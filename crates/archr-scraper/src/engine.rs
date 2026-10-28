use crate::{
    creds::Credentials,
    hash::digest,
    http::{Curl, Http, NetError},
    systems::system_id,
    xml::{parse, El},
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
    /// Box art, screenshot and wheel; video off; only games without art; resumable.
    pub fn new(language: &str) -> Self {
        Self {
            systems: Vec::new(),
            only_missing: true,
            resume: true,
            media: vec![Media::Box2d, Media::Screenshot, Media::Wheel],
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
    /// Developer or user login refused.
    Auth,
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
    pub fn verify(&self) -> Result<UserInfo, Stop> {
        let never = AtomicBool::new(false);
        let run = Run::new(&never, 1);
        match self.call(&self.url("ssuserInfos.php", &[]), 1 << 20, &run) {
            Ok(body) => Ok(user_info(&body)),
            Err(Call::Stop(stop)) => Err(stop),
            Err(_) => Err(Stop::Network),
        }
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

    fn run(
        &self,
        systems: &[SystemDef],
        options: &Options,
        cancel: &AtomicBool,
        send: &Sender<Event>,
    ) -> Report {
        let mut progress = Progress::default();
        let stop = |stop, progress: Progress| Report { stop, progress };
        let mut run = Run::new(cancel, 1);
        let info = match self.call(&self.url("ssuserInfos.php", &[]), 1 << 20, &run) {
            Ok(body) => user_info(&body),
            Err(Call::Stop(s)) => return stop(s, progress),
            Err(_) => UserInfo::default(), // unknown level: assume the minimum
        };
        run.threads = info.threads.clamp(1, MAX_THREADS);
        if info.max_requests_per_day > 0 {
            run.budget
                .store(info.max_requests_per_day - info.requests_today, Relaxed);
        }

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
                    return stop(Stop::Cancelled, progress);
                }
                let key = journal_key(system, &game);
                if (options.only_missing && game.image.is_some()) || finished.contains(&key) {
                    progress.skipped += 1;
                } else {
                    items.push(Item {
                        system,
                        id,
                        game,
                        key,
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

        let regions = regions(&options.language);
        thread::scope(|scope| {
            for _ in 0..run.threads {
                scope.spawn(|| {
                    while !cancel.load(Relaxed) {
                        let Some(item) = items.get(shared.next.fetch_add(1, Relaxed)) else {
                            break;
                        };
                        let outcome = self.scrape(&run, options, &regions, item);
                        match outcome {
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
        stop(reason, shared.snapshot())
    }

    /// `Err` is a run-ending condition; per-game problems are `Ok(Failed)`.
    fn scrape(
        &self,
        run: &Run,
        options: &Options,
        regions: &[&str],
        item: &Item,
    ) -> Result<Outcome, Stop> {
        let sys = item.id.to_string();
        let name = item
            .game
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let mut params = vec![
            ("systemeid", sys.as_str()),
            ("romtype", "rom"),
            ("romnom", &name),
        ];
        let hashes = rom_hashes(&item.game.path, run.cancel);
        let size;
        if let Some(d) = &hashes {
            size = d.size.to_string();
            params.extend([
                ("crc", d.crc32.as_str()),
                ("md5", &d.md5),
                ("romtaille", &size),
            ]);
        }
        let found = match self.call(&self.url("jeuInfos.php", &params), 4 << 20, run) {
            Ok(body) => parse(&body).and_then(|root| root.take("jeu")),
            Err(Call::NotFound) => None,
            Err(Call::Failed) => return Ok(Outcome::Failed),
            Err(Call::Stop(stop)) => return Err(stop),
        };
        let game = match found {
            Some(game) => game,
            None => {
                let query = clean_name(
                    &item
                        .game
                        .path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy(),
                );
                let url = self.url(
                    "jeuRecherche.php",
                    &[("systemeid", &sys), ("recherche", &query)],
                );
                match self.call(&url, 4 << 20, run) {
                    Ok(body) => match parse(&body).and_then(first_game) {
                        Some(game) => game,
                        None => return Ok(Outcome::NotFound),
                    },
                    Err(Call::NotFound) => return Ok(Outcome::NotFound),
                    Err(Call::Failed) => return Ok(Outcome::Failed),
                    Err(Call::Stop(stop)) => return Err(stop),
                }
            }
        };
        let medias: Vec<&El> = game
            .child("medias")
            .map_or(vec![], |m| m.children("media").collect());
        let stem = item.game.path.file_stem().unwrap_or_default();
        let (mut saved, mut failed) = (0, 0);
        for &kind in &options.media {
            let (types, dir, limit): (&[&str], _, _) = match kind {
                Media::Box2d => (&["box-2D"], "images", IMAGE_LIMIT),
                Media::Screenshot => (&["ss"], "screenshots", IMAGE_LIMIT),
                Media::Wheel => (&["wheel", "wheel-hd"], "wheel", IMAGE_LIMIT),
                Media::Video => (&["video-normalized", "video"], "videos", VIDEO_LIMIT),
            };
            let Some(url) = pick(&medias, types, regions) else {
                continue;
            };
            match self.call(&url, limit, run) {
                Ok(body) => {
                    let Some(ext) = sniff(&body, kind == Media::Video) else {
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
                Err(Call::Stop(stop)) => return Err(stop),
                Err(_) => failed += 1,
            }
        }
        Ok(match (saved, failed) {
            (0, 0) => Outcome::NotFound,
            (0, _) => Outcome::Failed,
            _ => Outcome::Scraped,
        })
    }

    /// One API/media request with pacing, throttling retries and status mapping.
    /// Never logs the URL (it carries credentials).
    fn call(&self, url: &str, limit: u64, run: &Run) -> Result<Vec<u8>, Call> {
        for attempt in 0..3 {
            run.pace(self.timing.spacing)?;
            let retry = match self.http.get(url, limit, run.cancel) {
                Ok(response) => match response.status {
                    200 => {
                        run.network.store(0, Relaxed);
                        return Ok(response.body);
                    }
                    404 => return Err(Call::NotFound),
                    401 | 403 => return Err(Call::Stop(Stop::Auth)),
                    430 | 431 => return Err(Call::Stop(Stop::Quota)),
                    423 => return Err(Call::Stop(Stop::Closed)),
                    429 | 500..=599 => true,
                    _ => return Err(Call::Failed),
                },
                Err(NetError::Cancelled) => return Err(Call::Stop(Stop::Cancelled)),
                Err(error) => {
                    log::debug!("scraper: network error {error:?}");
                    if run.network.fetch_add(1, Relaxed) + 1 >= NETWORK_GIVE_UP {
                        return Err(Call::Stop(Stop::Network));
                    }
                    true
                }
            };
            if retry && attempt < 2 {
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
    threads: usize,
    /// Requests left today (negative budget never starts: i64::MAX if unknown).
    budget: AtomicI64,
    network: AtomicU32,
    next_slot: Mutex<Instant>,
}

impl<'a> Run<'a> {
    fn new(cancel: &'a AtomicBool, threads: usize) -> Self {
        Self {
            cancel,
            threads,
            budget: AtomicI64::new(i64::MAX),
            network: AtomicU32::new(0),
            next_slot: Mutex::new(Instant::now()),
        }
    }

    /// Spends one request from the daily budget and waits for the shared slot.
    fn pace(&self, spacing: Duration) -> Result<(), Call> {
        if self.budget.fetch_sub(1, Relaxed) <= 0 {
            return Err(Call::Stop(Stop::Quota));
        }
        let wait = {
            let mut slot = self.next_slot.lock().unwrap();
            let now = Instant::now();
            let start = (*slot).max(now);
            *slot = start + spacing;
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

fn user_info(body: &[u8]) -> UserInfo {
    let number = |el: &El, name: &str| {
        el.child(name)
            .and_then(|n| n.text.trim().parse::<i64>().ok())
    };
    let Some(root) = parse(body) else {
        return UserInfo::default();
    };
    let Some(user) = root.child("ssuser") else {
        return UserInfo::default();
    };
    UserInfo {
        threads: number(user, "maxthreads").unwrap_or(1).max(1) as usize,
        requests_today: number(user, "requeststoday").unwrap_or(0),
        max_requests_per_day: number(user, "maxrequestsperday").unwrap_or(0),
    }
}

/// `jeuRecherche` answers `<Data><jeux><jeu id=..>`; empty `<jeu/>` means no match.
fn first_game(root: El) -> Option<El> {
    let mut jeux = root.take("jeux")?;
    let at = jeux.kids.iter().position(|k| k.attr("id").is_some())?;
    Some(jeux.kids.swap_remove(at))
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

/// First media of the wanted `types` (in order) by region preference. Only
/// https URLs on screenscraper.fr are followed: they embed our login.
fn pick(medias: &[&El], types: &[&str], regions: &[&str]) -> Option<String> {
    types.iter().find_map(|kind| {
        regions.iter().find_map(|region| {
            medias
                .iter()
                .filter(|m| m.attr("type") == Some(kind))
                .find(|m| m.attr("region").unwrap_or("") == *region)
                .and_then(|m| trusted_url(m.text.trim()))
        })
    })
}

fn trusted_url(raw: &str) -> Option<String> {
    let url = raw
        .replace(' ', "%20")
        .replace("#screenscraperserveur#", "https://www.screenscraper.fr/");
    let host = url.strip_prefix("https://")?.split(['/', '?']).next()?;
    (host == "screenscraper.fr" || host.ends_with(".screenscraper.fr")).then_some(url)
}

/// Extension by content: the service answers errors as text with status 200.
fn sniff(body: &[u8], video: bool) -> Option<&'static str> {
    match body {
        [0x89, b'P', b'N', b'G', ..] if !video => Some("png"),
        [0xFF, 0xD8, 0xFF, ..] if !video => Some("jpg"),
        [_, _, _, _, b'f', b't', b'y', b'p', ..] if video => Some("mp4"),
        _ => None,
    }
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
