use crate::{
    api::{self, Game as Hit, Media as Art},
    creds::Credentials,
    hash::digest,
    http::{trusted, Curl, Http, NetError},
    systems::system_id,
    validate,
};
use archr_index::{clean_name, scan_system, Game, SystemDef};
use archr_io::atomic_publish;
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicUsize, Ordering::Relaxed},
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const API: &str = "https://api.screenscraper.fr/api2";
const SOFTNAME: &str = match option_env!("SCREENSCRAPER_SOFTNAME") {
    Some(name) => name,
    None => "ArchRLauncher",
};
const JOURNAL: &str = "scraper-journal.txt";
const MAX_THREADS: usize = 4;
const HASH_LIMIT: u64 = 256 << 20;
const IMAGE_LIMIT: u64 = 8 << 20;
const VIDEO_LIMIT: u64 = 48 << 20;
const NETWORK_GIVE_UP: u32 = 6;

/// Developer login, injected by the build environment in ES's own format
/// (`devid=...&devpassword=...`, appended verbatim to the query string).
const DEV_LOGIN: Option<&str> = option_env!("SCREENSCRAPER_DEV_LOGIN");

fn valid_dev(login: &str) -> bool {
    login.starts_with("devid=")
        && login.contains("&devpassword=")
        && login.bytes().all(|b| b.is_ascii_graphic() && b != b'"')
}

/// True when this build can scrape (developer login present at compile time).
pub fn available() -> bool {
    DEV_LOGIN.is_some_and(valid_dev)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    Box2d,
    Screenshot,
    Wheel,
    Video,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// System ids (`SystemDef::name`); empty means every mapped system.
    pub systems: Vec<String>,
    /// Skip games that already have a cover.
    pub only_missing: bool,
    /// Skip games already finished by an earlier (interrupted) run.
    pub resume: bool,
    pub media: Vec<Media>,
    /// Launcher language (`pt-BR`, `en`, ...): picks the preferred art region.
    pub language: String,
}

impl Options {
    /// Box art only (the library reads nothing else yet); only games without art; resumable.
    pub fn new(language: &str) -> Self {
        Self {
            systems: Vec::new(),
            only_missing: true,
            resume: true,
            media: vec![Media::Box2d],
            language: language.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub system: String,
    pub game: String,
    /// Games handled so far, out of `total` to process.
    pub done: usize,
    pub total: usize,
    pub scraped: usize,
    pub not_found: usize,
    pub failed: usize,
    /// Left out up front: already had art, or finished in an earlier run.
    pub skipped: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Completed,
    Cancelled,
    /// Daily request quota reached (or account closed for unknown ROMs).
    Quota,
    /// Developer or user login refused (HTTP 403).
    Auth,
    /// Service limited to eligible members (HTTP 401): not a wrong password.
    Restricted,
    /// This application is blacklisted or outdated (HTTP 426).
    Outdated,
    /// API closed by the service.
    Closed,
    /// Repeated network failures.
    Network,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub stop: Stop,
    pub progress: Progress,
}

#[derive(Debug)]
pub enum Event {
    Progress(Progress),
    Done(Report),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserInfo {
    pub threads: usize,
    pub requests_today: i64,
    pub max_requests_per_day: i64,
    pub max_requests_per_min: i64,
}

#[derive(Clone, Copy)]
pub struct Timing {
    /// Total gap between API requests, shared by all threads.
    pub spacing: Duration,
    /// Wait before retrying a throttled/failed request.
    pub retry: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            spacing: Duration::from_millis(1000),
            retry: Duration::from_secs(5),
        }
    }
}

pub struct Job {
    events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    /// Next pending event, never blocks.
    pub fn poll(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }
    /// Asks the worker to stop; a final `Event::Done` follows.
    pub fn cancel(&self) {
        self.cancel.store(true, Relaxed);
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub struct Engine {
    http: Arc<dyn Http>,
    dev: String,
    cache: PathBuf,
    creds: Option<Credentials>,
    pub timing: Timing,
}

enum Call {
    NotFound,
    Failed,
    Stop(Stop),
}

impl Engine {
    /// `None` when the build has no developer login: keep the feature disabled.
    pub fn production(cache: PathBuf, creds: Option<Credentials>) -> Option<Self> {
        let user_agent = format!("archr-launcher/{}", env!("CARGO_PKG_VERSION"));
        let http = Curl {
            tmp: cache.join("scraper-tmp"),
            user_agent,
        };
        DEV_LOGIN
            .filter(|login| valid_dev(login))
            .map(|login| Self::new(Arc::new(http), login, cache, creds))
    }

    pub fn new(http: Arc<dyn Http>, dev: &str, cache: PathBuf, creds: Option<Credentials>) -> Self {
        Self {
            http,
            dev: dev.to_owned(),
            cache,
            creds,
            timing: Timing::default(),
        }
    }

    /// Blocking: asks the service who the configured user is. Worker only.
    /// Fails unless the reply is a well-formed account block (for the
    /// configured user, when there is one).
    pub fn verify(&self) -> Result<UserInfo, Stop> {
        let never = AtomicBool::new(false);
        self.account(&Run::new(&never, self.timing.spacing))
    }

    /// Starts the background job. Never blocks the caller.
    pub fn start(self, systems: Vec<SystemDef>, options: Options) -> Job {
        let (send, events) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        thread::spawn(move || {
            let report = self.run(&systems, &options, &flag, &send);
            let _ = send.send(Event::Done(report));
        });
        Job { events, cancel }
    }

    fn url(&self, script: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{API}/{script}?{}&softname={}&output=xml",
            self.dev,
            encode(&format!("{SOFTNAME}-{}", env!("CARGO_PKG_VERSION")))
        );
        if let Some(c) = &self.creds {
            url += &format!(
                "&ssid={}&sspassword={}",
                encode(&c.user),
                encode(&c.password)
            );
        }
        for (key, value) in params {
            url += &format!("&{key}={}", encode(value));
        }
        url
    }

    fn account(&self, run: &Run) -> Result<UserInfo, Stop> {
        let body = match self.call(&self.url("ssuserInfos.php", &[]), 1 << 20, run) {
            Ok(body) => body,
            Err(Call::Stop(stop)) => return Err(stop),
            Err(Call::NotFound) => return Err(Stop::Closed),
            Err(Call::Failed) => return Err(Stop::Network),
        };
        let user = api::account(&body).ok_or(Stop::Closed)?;
        if self
            .creds
            .as_ref()
            .is_some_and(|c| !user.id.trim().eq_ignore_ascii_case(c.user.trim()))
        {
            return Err(Stop::Auth);
        }
        let count = |n: Option<i64>| n.unwrap_or(0).max(0);
        Ok(UserInfo {
            threads: count(user.maxthreads).max(1) as usize,
            requests_today: count(user.requeststoday),
            max_requests_per_day: count(user.maxrequestsperday),
            max_requests_per_min: count(user.maxrequestspermin),
        })
    }

    fn run(
        &self,
        systems: &[SystemDef],
        options: &Options,
        cancel: &AtomicBool,
        send: &Sender<Event>,
    ) -> Report {
        let mut progress = Progress::default();
        let ended = |stop, progress: Progress| Report { stop, progress };
        let mut run = Run::new(cancel, self.timing.spacing);
        let info = match self.account(&run) {
            Ok(info) => info,
            Err(stop) => return ended(stop, progress),
        };
        let threads = (info.threads).clamp(1, MAX_THREADS);
        if info.max_requests_per_min > 0 {
            run.spacing = Duration::from_millis(60_000 / info.max_requests_per_min as u64);
            *run.next_slot.lock().unwrap() = Instant::now() + run.spacing;
        }
        if info.max_requests_per_day > 0 {
            run.budget
                .store(info.max_requests_per_day - info.requests_today, Relaxed);
        }

        let regions = regions(&options.language);
        let signature = signature(options, &regions);
        let journal = self.cache.join(JOURNAL);
        let finished: HashSet<String> = match options.resume {
            true => fs::read_to_string(&journal)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect(),
            false => HashSet::new(),
        };
        let mut items = Vec::new();
        for system in systems {
            let wanted = options.systems.is_empty() || options.systems.contains(&system.name);
            let Some(id) = system_id(&system.name).filter(|_| wanted) else {
                continue;
            };
            for game in scan_system(system) {
                if cancel.load(Relaxed) {
                    return ended(Stop::Cancelled, progress);
                }
                let key = format!("{}\t{signature}", journal_key(system, &game));
                let needed = needed(options, system, &game);
                if needed.is_empty() || finished.contains(&key) {
                    progress.skipped += 1;
                } else {
                    items.push(Item {
                        system,
                        id,
                        game,
                        key,
                        needed,
                    });
                }
            }
        }
        progress.total = items.len();
        let _ = fs::create_dir_all(&self.cache);
        let journal = OpenOptions::new()
            .create(true)
            .append(true)
            .open(journal)
            .ok();
        let shared = Shared {
            progress: Mutex::new(progress),
            journal: Mutex::new(journal),
            next: AtomicUsize::new(0),
            reason: Mutex::new(None),
        };
        let _ = send.send(Event::Progress(shared.snapshot()));

        thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    while !cancel.load(Relaxed) {
                        let Some(item) = items.get(shared.next.fetch_add(1, Relaxed)) else {
                            break;
                        };
                        match self.scrape(&run, &regions, item) {
                            Err(reason) => {
                                shared.reason.lock().unwrap().get_or_insert(reason);
                                cancel.store(true, Relaxed);
                            }
                            Ok(outcome) => {
                                shared.record(item, outcome);
                                let _ = send.send(Event::Progress(shared.snapshot()));
                            }
                        }
                    }
                });
            }
        });
        let reason = *shared.reason.lock().unwrap();
        let reason = match (reason, cancel.load(Relaxed)) {
            (Some(reason), _) => reason,
            (None, true) => Stop::Cancelled,
            (None, false) => Stop::Completed,
        };
        ended(reason, shared.snapshot())
    }

    /// `Err` is a run-ending condition; per-game problems are `Ok(Failed)`.
    fn scrape(&self, run: &Run, regions: &[&str], item: &Item) -> Result<Outcome, Stop> {
        let sys = item.id.to_string();
        let name = item
            .game
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        // The hash lookup needs a digest and size; without one go by name.
        let mut found = None;
        if let Some(d) = rom_hashes(&item.game.path, run.cancel) {
            let size = d.size.to_string();
            let params = [
                ("systemeid", sys.as_str()),
                ("romtype", "rom"),
                ("romnom", &name),
                ("crc", &d.crc32),
                ("md5", &d.md5),
                ("romtaille", &size),
            ];
            match self.call(&self.url("jeuInfos.php", &params), 4 << 20, run) {
                Ok(body) => found = api::game(&body),
                Err(Call::NotFound) => {}
                Err(Call::Failed) => return Ok(Outcome::Failed),
                Err(Call::Stop(stop)) => return Err(stop),
            }
        }
        let hit = match found {
            Some(hit) => hit,
            None => {
                let stem = item.game.path.file_stem().unwrap_or_default();
                let query = clean_name(&stem.to_string_lossy());
                let url = self.url(
                    "jeuRecherche.php",
                    &[("systemeid", &sys), ("recherche", &query)],
                );
                match self.call(&url, 4 << 20, run) {
                    Ok(body) => match api::search(&body) {
                        Some(hit) => hit,
                        None => return Ok(Outcome::NotFound),
                    },
                    Err(Call::NotFound) => return Ok(Outcome::NotFound),
                    Err(Call::Failed) => return Ok(Outcome::Failed),
                    Err(Call::Stop(stop)) => return Err(stop),
                }
            }
        };
        self.download(run, regions, item, &hit)
    }

    /// Non-cover assets are written as they arrive; the cover (last) only if
    /// everything else worked, so a present cover means the game is complete.
    fn download(
        &self,
        run: &Run,
        regions: &[&str],
        item: &Item,
        hit: &Hit,
    ) -> Result<Outcome, Stop> {
        let stem = item.game.path.file_stem().unwrap_or_default();
        let (mut saved, mut failed) = (0, 0);
        for &kind in &item.needed {
            let (types, dir, limit): (&[&str], _, _) = match kind {
                Media::Box2d => (&["box-2D"], "images", IMAGE_LIMIT),
                Media::Screenshot => (&["ss"], "screenshots", IMAGE_LIMIT),
                Media::Wheel => (&["wheel", "wheel-hd"], "wheel", IMAGE_LIMIT),
                Media::Video => (&["video-normalized", "video"], "videos", VIDEO_LIMIT),
            };
            let Some(url) = pick(hit.medias(), types, regions) else {
                continue;
            };
            if kind == Media::Box2d && failed > 0 {
                break;
            }
            let body = match self.call(&url, limit, run) {
                Ok(body) => body,
                Err(Call::Stop(stop)) => return Err(stop),
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };
            let ext = match kind {
                Media::Video => validate::video(&body).then_some("mp4"),
                _ => validate::image(&body),
            };
            let Some(ext) = ext else {
                failed += 1;
                continue;
            };
            let mut file = stem.to_os_string();
            file.push(format!(".{ext}"));
            let dest = item.system.path.join("media").join(dir).join(file);
            match atomic_publish(&dest, false, |f| f.write_all(&body)) {
                Ok(()) => saved += 1,
                Err(_) => failed += 1,
            }
        }
        Ok(match (saved, failed) {
            (_, 1..) => Outcome::Failed,
            (0, 0) => Outcome::NotFound,
            _ => Outcome::Scraped,
        })
    }

    /// One API/media request with pacing, throttling retries and status mapping.
    /// Never logs the URL (it carries credentials).
    fn call(&self, url: &str, limit: u64, run: &Run) -> Result<Vec<u8>, Call> {
        for attempt in 0..3 {
            run.pace()?;
            match self.http.get(url, limit, run.cancel) {
                Ok(response) => match response.status {
                    200 => {
                        run.network.store(0, Relaxed);
                        return Ok(response.body);
                    }
                    404 => return Err(Call::NotFound),
                    401 => return Err(Call::Stop(Stop::Restricted)),
                    403 => return Err(Call::Stop(Stop::Auth)),
                    426 => return Err(Call::Stop(Stop::Outdated)),
                    430 | 431 => return Err(Call::Stop(Stop::Quota)),
                    423 => return Err(Call::Stop(Stop::Closed)),
                    429 | 500..=599 => {}
                    _ => return Err(Call::Failed),
                },
                Err(NetError::Cancelled) => return Err(Call::Stop(Stop::Cancelled)),
                Err(error) => {
                    log::debug!("scraper: network error {error:?}");
                    if run.network.fetch_add(1, Relaxed) + 1 >= NETWORK_GIVE_UP {
                        return Err(Call::Stop(Stop::Network));
                    }
                }
            }
            if attempt < 2 {
                run.sleep(self.timing.retry)?;
            }
        }
        Err(Call::Failed)
    }
}

struct Item<'a> {
    system: &'a SystemDef,
    id: u32,
    game: Game,
    key: String,
    /// Assets still to fetch, cover last.
    needed: Vec<Media>,
}

enum Outcome {
    Scraped,
    NotFound,
    Failed,
}

struct Shared {
    progress: Mutex<Progress>,
    journal: Mutex<Option<fs::File>>,
    next: AtomicUsize,
    reason: Mutex<Option<Stop>>,
}

impl Shared {
    fn snapshot(&self) -> Progress {
        self.progress.lock().unwrap().clone()
    }
    /// Only complete games are journaled; a failed asset is retried next run.
    fn record(&self, item: &Item, outcome: Outcome) {
        let mut p = self.progress.lock().unwrap();
        p.done += 1;
        p.system = item.system.name.clone();
        p.game = item.game.name.clone();
        match outcome {
            Outcome::Scraped => p.scraped += 1,
            Outcome::NotFound => p.not_found += 1,
            Outcome::Failed => return p.failed += 1,
        }
        if let Some(file) = self.journal.lock().unwrap().as_mut() {
            let _ = writeln!(file, "{}", item.key);
        }
    }
}

/// Per-run request state shared by the worker threads.
struct Run<'a> {
    cancel: &'a AtomicBool,
    /// Shared gap between requests (the account's per-minute limit, if known).
    spacing: Duration,
    /// Requests left today (i64::MAX if unknown).
    budget: AtomicI64,
    network: AtomicU32,
    next_slot: Mutex<Instant>,
}

impl<'a> Run<'a> {
    fn new(cancel: &'a AtomicBool, spacing: Duration) -> Self {
        Self {
            cancel,
            spacing,
            budget: AtomicI64::new(i64::MAX),
            network: AtomicU32::new(0),
            next_slot: Mutex::new(Instant::now()),
        }
    }

    /// Spends one request from the daily budget and waits for the shared slot.
    fn pace(&self) -> Result<(), Call> {
        if self.budget.fetch_sub(1, Relaxed) <= 0 {
            return Err(Call::Stop(Stop::Quota));
        }
        let wait = {
            let mut slot = self.next_slot.lock().unwrap();
            let now = Instant::now();
            let start = (*slot).max(now);
            *slot = start + self.spacing;
            start - now
        };
        self.sleep(wait)
    }

    fn sleep(&self, total: Duration) -> Result<(), Call> {
        let end = Instant::now() + total;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            if self.cancel.load(Relaxed) {
                return Err(Call::Stop(Stop::Cancelled));
            }
            thread::sleep(left.min(Duration::from_millis(100)));
        }
        Ok(())
    }
}

fn journal_key(system: &SystemDef, game: &Game) -> String {
    let relative = game.path.strip_prefix(&system.path).unwrap_or(&game.path);
    format!("{}/{}", system.name, relative.display())
}

/// What a finished game depends on: the asset set and the art region.
fn signature(options: &Options, regions: &[&str]) -> String {
    let mut kinds: Vec<String> = options.media.iter().map(|m| format!("{m:?}")).collect();
    kinds.sort();
    format!("{}@{}", kinds.join("+"), regions[0])
}

const fn folder(kind: Media) -> &'static str {
    match kind {
        Media::Box2d => "images",
        Media::Screenshot => "screenshots",
        Media::Wheel => "wheel",
        Media::Video => "videos",
    }
}

/// Requested assets still missing for `game` (all of them unless `only_missing`),
/// with the cover last.
fn needed(options: &Options, system: &SystemDef, game: &Game) -> Vec<Media> {
    let stem = game.path.file_stem().unwrap_or_default();
    let present = |kind: Media| match kind {
        Media::Box2d => game.image.is_some(),
        _ => ["png", "jpg", "mp4"].iter().any(|ext| {
            let mut file = stem.to_os_string();
            file.push(format!(".{ext}"));
            system
                .path
                .join("media")
                .join(folder(kind))
                .join(file)
                .is_file()
        }),
    };
    let mut kinds: Vec<Media> = options
        .media
        .iter()
        .copied()
        .filter(|&kind| !(options.only_missing && present(kind)))
        .collect();
    kinds.sort_by_key(|&kind| kind == Media::Box2d);
    kinds.dedup();
    kinds
}

/// Hashes only real files within the size cap; cue/m3u sheets and folders go by name.
fn rom_hashes(path: &Path, cancel: &AtomicBool) -> Option<crate::hash::Digest> {
    let meta = fs::metadata(path)
        .ok()
        .filter(|m| m.is_file() && m.len() <= HASH_LIMIT)?;
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let sheet = ["cue", "m3u", "gdi", "ccd", "sh"].contains(&ext.as_str());
    (!sheet && meta.len() > 0)
        .then(|| digest(path, cancel).ok())
        .flatten()
}

/// Preferred art regions for a launcher language, then ES's generic fallbacks.
fn regions(language: &str) -> Vec<&'static str> {
    let lang = language.to_ascii_lowercase();
    let mut out: Vec<&'static str> = match lang.split(['-', '_']).next().unwrap_or("") {
        "pt" => vec!["br", "pt"],
        "es" => vec!["es", "mx"],
        "ru" => vec!["ru"],
        _ => vec!["us", "uk"],
    };
    out.extend(["wor", "us", "eu", "jp", "ss", "cus", ""]);
    out
}

/// First media of the wanted `types` (in order) by region preference, on a
/// trusted host only: the URL embeds our login.
fn pick(medias: &[Art], types: &[&str], regions: &[&str]) -> Option<String> {
    types.iter().find_map(|kind| {
        regions.iter().find_map(|region| {
            medias
                .iter()
                .filter(|m| m.kind == *kind && m.region == *region)
                .find_map(|m| {
                    let url = m
                        .url
                        .trim()
                        .replace(' ', "%20")
                        .replace("#screenscraperserveur#", "https://www.screenscraper.fr/");
                    trusted(&url).then_some(url)
                })
        })
    })
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
