//! Theme store engine: GitHub search, details, install and removal. Every
//! call blocks; the settings worker runs them on the store lane. The contract
//! for theme authors is in docs/temas.md ("Publicar na Loja de temas").
use archr_io::{atomic_publish, read_bytes, read_utf8, Symlinks};
use archr_scraper::{image_ext, Curl, Http};
use archr_ui::{
    settings::store::{allowed, Action, Card, Detail, Reply, Tab, TOPIC},
    Theme,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime},
};

/// Every hop must stay on these.
pub const HOSTS: &[&str] = &[
    "api.github.com",
    "codeload.github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
];
const API: &str = "https://api.github.com";
const RAW: &str = "https://raw.githubusercontent.com";
const CODELOAD: &str = "https://codeload.github.com";
/// Unauthenticated GitHub allows 10 searches a minute and 60 other calls an
/// hour: answers are kept this long.
const FRESH: Duration = Duration::from_secs(3600);
const JSON_LIMIT: u64 = 4 << 20;
const TOML_LIMIT: u64 = 64 << 10;
const IMAGE_LIMIT: u64 = 2 << 20;
pub const TARBALL_LIMIT: u64 = 25 << 20;
pub const EXTRACTED_LIMIT: u64 = 60 << 20;
const MAX_ENTRIES: usize = 5000;
const MAX_DEPTH: usize = 8;
const SCREENSHOTS: usize = 3;
/// Inside an installed theme: where it came from. Folders without one were not
/// installed by the store and are never touched by it.
const MANIFEST: &str = ".archr-store.toml";

/// Where a store theme came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    owner: String,
    repo: String,
    branch: String,
    reference: String,
    tree: String,
    date: String,
    stars: u64,
}
impl Manifest {
    fn read(dir: &Path) -> Option<Self> {
        let text = read_utf8(&dir.join(MANIFEST), 4096, Symlinks::Reject).ok()?;
        let manifest: Self = toml::from_str(&text).ok()?;
        (format!("{}-{}", manifest.owner, manifest.repo) == dir.file_name()?.to_str()?)
            .then_some(manifest)
    }
}

/// GitHub user or organisation.
fn owner_ok(name: &str) -> bool {
    (1..=39).contains(&name.len())
        && !name.starts_with('-')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
/// Repository, release tag or branch: no slash, no leading dot or dash.
fn name_ok(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && !name.starts_with(['.', '-'])
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn card_ok(card: &Card) -> bool {
    owner_ok(&card.owner) && name_ok(&card.repo) && name_ok(&card.branch)
}
/// `YYYY-MM-DD` from a GitHub timestamp.
fn day(value: &Value) -> String {
    let text = value.as_str().unwrap_or("");
    let day = text.get(..10).unwrap_or("");
    if day.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        day.to_owned()
    } else {
        String::new()
    }
}
fn now_day() -> String {
    let days = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Unpacks a GitHub tarball (one top folder) into `into`: regular files of the
/// allowed kinds only. Links, devices and the like are skipped; a path leaving
/// the archive refuses the whole theme. Nothing is ever executed.
pub fn extract(tarball: &[u8], into: &Path) -> Result<(), &'static str> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    let mut total = 0u64;
    let entries = archive.entries().map_err(|_| "store.invalid")?;
    for (n, entry) in entries.enumerate() {
        let mut entry = entry.map_err(|_| "store.invalid")?;
        if n >= MAX_ENTRIES {
            return Err("store.too_big");
        }
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions() || kind.is_pax_local_extensions() {
            continue;
        }
        let path = entry.path().map_err(|_| "store.invalid")?.into_owned();
        let mut parts = Vec::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_str().ok_or("store.invalid")?),
                Component::CurDir => {}
                _ => return Err("store.invalid"),
            }
        }
        // The top folder is GitHub's `<owner>-<repo>-<commit>`.
        let relative = parts.get(1..).unwrap_or_default().join("/");
        if !kind.is_file() || parts.len() > MAX_DEPTH + 1 || !allowed(&relative) {
            continue;
        }
        total = total.saturating_add(entry.header().size().map_err(|_| "store.invalid")?);
        if total > EXTRACTED_LIMIT {
            return Err("store.too_big");
        }
        let target = into.join(&relative);
        let fail = |_: io::Error| "store.failed";
        fs::create_dir_all(archr_io::parent_dir(&target)).map_err(fail)?;
        // create_new: never through a link or over an earlier entry.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(fail)?;
        io::copy(&mut (&mut entry).take(EXTRACTED_LIMIT), &mut file).map_err(fail)?;
    }
    Ok(())
}

/// Files under `dir` as theme-relative paths and sizes, links not followed.
fn files(dir: &Path) -> Vec<(String, u64)> {
    let mut found = Vec::new();
    let mut pending = vec![(dir.to_path_buf(), String::new(), 0)];
    while let Some((at, prefix, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten().take(MAX_ENTRIES) {
            let (Ok(kind), Some(name)) = (
                entry.file_type(),
                entry.file_name().to_str().map(str::to_owned),
            ) else {
                continue;
            };
            let path = format!("{prefix}{name}");
            if kind.is_dir() && depth < MAX_DEPTH {
                pending.push((entry.path(), format!("{path}/"), depth + 1));
            } else if kind.is_file() {
                found.push((path, entry.metadata().map_or(0, |m| m.len())));
            }
        }
    }
    found
}

pub struct Shop {
    http: Box<dyn Http>,
    /// Saved answers and pictures.
    cache: PathBuf,
    /// User themes: /storage/.config/archr-launcher/themes.
    themes: PathBuf,
    cancel: AtomicBool,
}

impl Shop {
    pub fn new(http: Box<dyn Http>, cache: PathBuf, themes: PathBuf) -> Self {
        Self {
            http,
            cache,
            themes,
            cancel: AtomicBool::new(false),
        }
    }
    /// The system `curl`, hardened as for the scraper, held to GitHub.
    pub fn production(cache: PathBuf, themes: PathBuf) -> Self {
        let http = Curl {
            tmp: cache.join("tmp"),
            user_agent: format!("archr-launcher/{}", env!("CARGO_PKG_VERSION")),
            hosts: HOSTS,
        };
        Self::new(Box::new(http), cache, themes)
    }
    /// The body, `None` for 404; failures as locale keys.
    fn get(&self, url: &str, limit: u64) -> Result<Option<Vec<u8>>, &'static str> {
        match self.http.get(url, limit, &self.cancel) {
            Ok(reply) if reply.status == 200 => Ok(Some(reply.body)),
            Ok(reply) if reply.status == 404 => Ok(None),
            Ok(reply) if matches!(reply.status, 403 | 429) && url.starts_with(API) => {
                Err("store.rate_limited")
            }
            Ok(_) => Err("store.failed"),
            Err(_) => Err("store.offline"),
        }
    }
    /// An API answer kept in `name` for an hour (`refresh` asks again); a 404
    /// is kept as an empty body. On failure the saved answer, however old.
    fn saved(
        &self,
        url: &str,
        name: &str,
        refresh: bool,
    ) -> (Option<(Vec<u8>, SystemTime)>, Option<&'static str>) {
        let path = self.cache.join(name);
        let saved = fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| Some((read_bytes(&path, JSON_LIMIT, Symlinks::Reject).ok()?, at)));
        let fresh = saved
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed().is_ok_and(|age| age < FRESH));
        if fresh && !refresh {
            return (saved, None);
        }
        match self.get(url, JSON_LIMIT) {
            Ok(body) => {
                let body = body.unwrap_or_default();
                let _ = fs::create_dir_all(archr_io::parent_dir(&path));
                if let Err(error) =
                    atomic_publish(&path, false, |file| io::Write::write_all(file, &body))
                {
                    log::warn!("Theme store cache: {error}");
                }
                (Some((body, SystemTime::now())), None)
            }
            Err(problem) => (saved, Some(problem)),
        }
    }
    /// A picture saved at `path`, downloading it once; only real images stay.
    fn picture(&self, url: &str, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_owned());
        }
        let body = self.get(url, IMAGE_LIMIT).ok()??;
        image_ext(&body)?;
        fs::create_dir_all(archr_io::parent_dir(path)).ok()?;
        atomic_publish(path, false, |file| io::Write::write_all(file, &body)).ok()?;
        Some(path.to_owned())
    }

    /// One tab from GitHub's search, saved for an hour.
    pub fn list(&self, tab: Tab, refresh: bool) -> Reply {
        let sort = if tab == Tab::Popular {
            "stars"
        } else {
            "updated"
        };
        let url = format!(
            "{API}/search/repositories?q=topic:{TOPIC}+archived:false&sort={sort}&order=desc&per_page=30"
        );
        let (saved, mut problem) = self.saved(&url, &format!("search-{sort}.json"), refresh);
        let (body, fetched) = saved.map_or((Vec::new(), None), |(body, at)| (body, Some(at)));
        let parsed: Option<Vec<Card>> = serde_json::from_slice::<Value>(&body).ok().map(|json| {
            json["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| Card {
                    owner: item["owner"]["login"].as_str().unwrap_or("").into(),
                    repo: item["name"].as_str().unwrap_or("").into(),
                    stars: item["stargazers_count"].as_u64().unwrap_or(0),
                    branch: item["default_branch"].as_str().unwrap_or("").into(),
                    pushed: day(&item["pushed_at"]),
                    ..Card::default()
                })
                .filter(card_ok)
                .collect()
        });
        if parsed.is_none() && problem.is_none() {
            problem = Some("store.failed");
        }
        Reply::Listed {
            tab,
            cards: parsed.unwrap_or_default(),
            problem,
            fetched,
        }
    }
    /// The card with its theme's name and cover, from the default branch,
    /// fetched again only after a new push.
    pub fn decorate(&self, card: &Card) -> Option<Card> {
        let dir = self.cache.join("cards").join(card.folder());
        let stamp = dir.join("pushed");
        let current = read_utf8(&stamp, 64, Symlinks::Reject).is_ok_and(|s| s == card.pushed);
        let (toml, preview) = (dir.join("theme.toml"), dir.join("preview.png"));
        let raw = format!("{RAW}/{}/{}/{}", card.owner, card.repo, card.branch);
        if !current {
            // Fetched first: offline, the saved card stays.
            let body = self.get(&format!("{raw}/theme.toml"), TOML_LIMIT).ok()??;
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).ok()?;
            atomic_publish(&toml, false, |file| io::Write::write_all(file, &body)).ok()?;
            self.picture(&format!("{raw}/preview.png"), &preview);
            atomic_publish(&stamp, false, |file| {
                io::Write::write_all(file, card.pushed.as_bytes())
            })
            .ok()?;
        }
        let table = toml::from_str(&read_utf8(&toml, TOML_LIMIT, Symlinks::Reject).ok()?).ok()?;
        Some(Card {
            name: Theme::parse(&table).name,
            preview: preview.is_file().then_some(preview),
            ..card.clone()
        })
    }
    /// What the latest release (or the default branch) holds; an installed
    /// theme is described from its folder when GitHub cannot be reached.
    pub fn detail(&self, card: &Card) -> Result<Detail, &'static str> {
        if !card_ok(card) {
            return Err("store.invalid");
        }
        self.remote(card)
            .or_else(|problem| self.local(card).ok_or(problem))
    }
    fn remote(&self, card: &Card) -> Result<Detail, &'static str> {
        let folder = card.folder();
        let repo = format!("{API}/repos/{}/{}", card.owner, card.repo);
        let saved = |url: String, name: String| match self.saved(&url, &name, false) {
            (Some((body, _)), _) => Ok(body),
            (None, problem) => Err(problem.unwrap_or("store.failed")),
        };
        let release = saved(
            format!("{repo}/releases/latest"),
            format!("{folder}/release.json"),
        )?;
        let release: Value = serde_json::from_slice(&release).unwrap_or(Value::Null);
        let (reference, date) = match release["tag_name"].as_str() {
            Some(tag) => (tag.to_owned(), day(&release["published_at"])),
            None => (card.branch.clone(), card.pushed.clone()),
        };
        if !name_ok(&reference) {
            return Err("store.invalid");
        }
        let tree = saved(
            format!("{repo}/git/trees/{reference}?recursive=1"),
            format!("{folder}/tree-{reference}.json"),
        )?;
        let tree: Value = serde_json::from_slice(&tree).map_err(|_| "store.failed")?;
        let sha = tree["sha"].as_str().unwrap_or("");
        if !name_ok(sha) {
            return Err("store.failed");
        }
        let files: Vec<(String, u64)> = tree["tree"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"] == "blob")
            .filter_map(|item| Some((item["path"].as_str()?.to_owned(), item["size"].as_u64()?)))
            .collect();
        let raw = format!("{RAW}/{}/{}/{reference}", card.owner, card.repo);
        let source = self
            .get(&format!("{raw}/theme.toml"), TOML_LIMIT)?
            .and_then(|body| String::from_utf8(body).ok())
            .ok_or("store.invalid")?;
        let mut detail = Detail::describe(card.clone(), &source, &files, env!("CARGO_PKG_VERSION"))
            .map_err(|_| "store.invalid")?;
        let pictures = self.cache.join("pictures").join(&folder).join(sha);
        let names = ["preview.png".to_owned()]
            .into_iter()
            .chain((1..=SCREENSHOTS).map(|n| format!("screenshots/{n}.png")));
        detail.images = names
            .filter(|name| files.iter().any(|(path, _)| path == name))
            .filter_map(|name| {
                let path = pictures.join(name.replace('/', "-"));
                self.picture(&format!("{raw}/{name}"), &path)
            })
            .collect();
        detail.date = date;
        detail.reference = reference;
        detail.tree = sha.to_owned();
        Ok(detail)
    }
    fn local(&self, card: &Card) -> Option<Detail> {
        let dir = self.themes.join(card.folder());
        let manifest = Manifest::read(&dir)?;
        let source = read_utf8(&dir.join("theme.toml"), TOML_LIMIT, Symlinks::Reject).ok()?;
        let mut detail = Detail::describe(
            card.clone(),
            &source,
            &files(&dir),
            env!("CARGO_PKG_VERSION"),
        )
        .ok()?;
        detail.images = [
            "preview.png",
            "screenshots/1.png",
            "screenshots/2.png",
            "screenshots/3.png",
        ]
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .collect();
        detail.date = manifest.date;
        detail.reference = manifest.reference;
        detail.tree = manifest.tree;
        Some(detail)
    }
    /// Themes the store installed: folder with content id, and their cards.
    pub fn installed(&self) -> Reply {
        let mut map = BTreeMap::new();
        let mut cards = Vec::new();
        for entry in fs::read_dir(&self.themes).into_iter().flatten().flatten() {
            let dir = entry.path();
            let Some(manifest) = Manifest::read(&dir) else {
                continue;
            };
            let name = read_utf8(&dir.join("theme.toml"), TOML_LIMIT, Symlinks::Reject)
                .ok()
                .and_then(|source| toml::from_str(&source).ok())
                .map(|table| Theme::parse(&table).name)
                .unwrap_or_default();
            let preview = dir.join("preview.png");
            let card = Card {
                owner: manifest.owner,
                repo: manifest.repo,
                name,
                stars: manifest.stars,
                branch: manifest.branch,
                pushed: manifest.date,
                preview: preview.is_file().then_some(preview),
            };
            map.insert(card.folder(), manifest.tree);
            cards.push(card);
        }
        cards.sort_by_key(|card| card.folder());
        Reply::Installed(map, cards)
    }
    /// Downloads the release (or branch) the details showed, checks it and
    /// swaps it into the themes folder in one rename.
    pub fn install(&self, detail: &Detail) -> Result<(), &'static str> {
        let card = &detail.card;
        if !card_ok(card) || !name_ok(&detail.reference) {
            return Err("store.invalid");
        }
        let target = self.themes.join(card.folder());
        let ours = Manifest::read(&target).is_some();
        if target.exists() && !ours {
            return Err("store.name_taken");
        }
        let url = format!(
            "{CODELOAD}/{}/{}/tar.gz/{}",
            card.owner, card.repo, detail.reference
        );
        let tarball = match self.http.get(&url, TARBALL_LIMIT, &self.cancel) {
            Ok(reply) if reply.status == 200 => reply.body,
            Ok(_) => return Err("store.failed"),
            Err(_) => return Err("store.download_failed"),
        };
        let fail = |_: io::Error| "store.failed";
        fs::create_dir_all(&self.themes).map_err(fail)?;
        let staging = tempfile::Builder::new()
            .prefix(".store-")
            .tempdir_in(&self.themes)
            .map_err(fail)?;
        extract(&tarball, staging.path())?;
        let source = read_utf8(
            &staging.path().join("theme.toml"),
            TOML_LIMIT,
            Symlinks::Reject,
        )
        .map_err(|_| "store.invalid")?;
        let files = files(staging.path());
        Detail::describe(card.clone(), &source, &files, "0").map_err(|_| "store.invalid")?;
        let manifest = Manifest {
            owner: card.owner.clone(),
            repo: card.repo.clone(),
            branch: card.branch.clone(),
            reference: detail.reference.clone(),
            tree: detail.tree.clone(),
            date: if detail.date.is_empty() {
                now_day()
            } else {
                detail.date.clone()
            },
            stars: card.stars,
        };
        let text = toml::to_string(&manifest).map_err(|_| "store.failed")?;
        fs::write(staging.path().join(MANIFEST), text).map_err(fail)?;
        // The old version moves aside (onto an empty folder) and goes away
        // only once the new one is in place.
        let old = tempfile::Builder::new()
            .prefix(".store-old-")
            .tempdir_in(&self.themes)
            .map_err(fail)?;
        if ours {
            fs::rename(&target, old.path()).map_err(fail)?;
        }
        if let Err(error) = fs::rename(staging.path(), &target) {
            if ours {
                let _ = fs::rename(old.path(), &target);
            }
            return Err(fail(error));
        }
        Ok(())
    }
    /// Deletes a theme the store installed; any other folder is refused.
    pub fn remove(&self, folder: &str) -> Result<(), &'static str> {
        let target = self.themes.join(folder);
        if folder.contains('/') || Manifest::read(&target).is_none() {
            return Err("store.failed");
        }
        let fail = |_: io::Error| "store.failed";
        let trash = tempfile::Builder::new()
            .prefix(".store-old-")
            .tempdir_in(&self.themes)
            .map_err(fail)?;
        fs::rename(&target, trash.path()).map_err(fail)
    }
    /// One request of the store lane. `active` is the saved theme, which
    /// cannot be removed; `queue` collects cards still to decorate.
    pub fn handle(&self, action: Action, active: &str, queue: &mut VecDeque<Card>) -> Vec<Reply> {
        match action {
            Action::List(Tab::Installed, _) => vec![self.installed()],
            Action::List(tab, refresh) => {
                let listed = self.list(tab, refresh);
                if let Reply::Listed { cards, .. } = &listed {
                    *queue = cards.iter().cloned().collect();
                }
                vec![self.installed(), listed]
            }
            Action::Open(card) => {
                vec![Reply::Detail(
                    card.folder(),
                    self.detail(&card).map(Box::new),
                )]
            }
            Action::Install(detail) => {
                let done = self.install(&detail).map(|()| "store.installed_ok");
                if let Err(problem) = done {
                    log::warn!(
                        "Theme store install of {} failed: {problem}",
                        detail.card.folder()
                    );
                }
                vec![self.installed(), Reply::Done(done)]
            }
            Action::Remove(folder) if folder == active => {
                vec![Reply::Done(Err("store.active_theme"))]
            }
            Action::Remove(folder) => {
                let done = self.remove(&folder).map(|()| "store.removed");
                vec![self.installed(), Reply::Done(done)]
            }
        }
    }
}
