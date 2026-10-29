//! What the theme and module stores share: GitHub calls with an hour of
//! cache, pictures checked as images, and a tarball unpacker that writes
//! regular files only. Every call blocks; store lanes run them.
use archr_io::{atomic_publish, read_bytes, Symlinks};
use archr_scraper::{image_ext, Curl, Http};
use serde_json::Value;
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime},
};

pub const API: &str = "https://api.github.com";
/// Unauthenticated GitHub allows 10 searches a minute and 60 other calls an
/// hour: answers are kept this long.
const FRESH: Duration = Duration::from_secs(3600);
pub const JSON_LIMIT: u64 = 4 << 20;
const IMAGE_LIMIT: u64 = 2 << 20;

/// GitHub user or organisation.
pub fn owner_ok(name: &str) -> bool {
    (1..=39).contains(&name.len())
        && !name.starts_with('-')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
/// Repository, release tag or branch: no slash, no leading dot or dash.
pub fn name_ok(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && !name.starts_with(['.', '-'])
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
/// `YYYY-MM-DD` from a GitHub timestamp.
pub fn day(value: &Value) -> String {
    let text = value.as_str().unwrap_or("");
    let day = text.get(..10).unwrap_or("");
    if day.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        day.to_owned()
    } else {
        String::new()
    }
}

/// One search result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub repo: String,
    pub stars: u64,
    pub branch: String,
    pub pushed: String,
}

pub struct GitHub {
    http: Box<dyn Http>,
    /// Saved answers and pictures.
    pub cache: PathBuf,
    pub cancel: AtomicBool,
}
impl GitHub {
    pub fn new(http: Box<dyn Http>, cache: PathBuf) -> Self {
        Self {
            http,
            cache,
            cancel: AtomicBool::new(false),
        }
    }
    /// The system `curl`, hardened as for the scraper, held to `hosts`.
    pub fn curl(cache: &Path, hosts: &'static [&'static str]) -> Box<dyn Http> {
        Box::new(Curl {
            tmp: cache.join("tmp"),
            user_agent: format!("archr-launcher/{}", env!("CARGO_PKG_VERSION")),
            hosts,
        })
    }
    /// The body, `None` for 404; failures as locale keys.
    pub fn get(&self, url: &str, limit: u64) -> Result<Option<Vec<u8>>, &'static str> {
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
    pub fn saved(
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
                self.keep(&path, &body);
                (Some((body, SystemTime::now())), None)
            }
            Err(problem) => (saved, Some(problem)),
        }
    }
    /// A saved answer that only fails when there is none.
    pub fn json(&self, url: &str, name: &str) -> Result<Value, &'static str> {
        match self.saved(url, name, false) {
            (Some((body, _)), _) => serde_json::from_slice(&body).map_err(|_| "store.failed"),
            (None, problem) => Err(problem.unwrap_or("store.failed")),
        }
    }
    pub fn keep(&self, path: &Path, body: &[u8]) {
        let _ = fs::create_dir_all(archr_io::parent_dir(path));
        if let Err(error) = atomic_publish(path, false, |file| io::Write::write_all(file, body)) {
            log::warn!("Store cache: {error}");
        }
    }
    /// A picture saved at `path`, downloading it once; only real images stay.
    pub fn picture(&self, url: &str, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_owned());
        }
        let body = self.get(url, IMAGE_LIMIT).ok()??;
        image_ext(&body)?;
        fs::create_dir_all(archr_io::parent_dir(path)).ok()?;
        atomic_publish(path, false, |file| io::Write::write_all(file, &body)).ok()?;
        Some(path.to_owned())
    }
    /// Repositories with `topic`, by `sort` (stars or updated), saved for an
    /// hour; the problem keeps the saved list, if any.
    pub fn search(
        &self,
        topic: &str,
        sort: &str,
        refresh: bool,
    ) -> (Option<Vec<Repo>>, Option<&'static str>, Option<SystemTime>) {
        let url = format!(
            "{API}/search/repositories?q=topic:{topic}+archived:false&sort={sort}&order=desc&per_page=30"
        );
        let (saved, problem) = self.saved(&url, &format!("search-{sort}.json"), refresh);
        let (body, fetched) = saved.map_or((Vec::new(), None), |(body, at)| (body, Some(at)));
        let repos = serde_json::from_slice::<Value>(&body).ok().map(|json| {
            json["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| Repo {
                    owner: item["owner"]["login"].as_str().unwrap_or("").into(),
                    repo: item["name"].as_str().unwrap_or("").into(),
                    stars: item["stargazers_count"].as_u64().unwrap_or(0),
                    branch: item["default_branch"].as_str().unwrap_or("").into(),
                    pushed: day(&item["pushed_at"]),
                })
                .filter(|r| owner_ok(&r.owner) && name_ok(&r.repo) && name_ok(&r.branch))
                .collect()
        });
        let problem = problem.or_else(|| repos.is_none().then_some("store.failed"));
        (repos, problem, fetched)
    }
}

/// What a tarball may hold and how it is written.
pub struct Rules<'a> {
    /// The single top folder's name; `None` accepts any (GitHub's).
    pub top: Option<&'a str>,
    /// Kept paths (relative to the top folder); others are skipped.
    pub keep: &'a dyn Fn(&str) -> bool,
    /// Links, devices and the like refuse the whole archive instead of
    /// being skipped.
    pub strict: bool,
    pub entries: usize,
    pub bytes: u64,
    pub depth: usize,
    /// The one file written executable (0755); every other file is 0644.
    pub executable: Option<&'a str>,
}

/// Unpacks `tarball` (one top folder) into `into`: regular files only. A path
/// leaving the archive refuses it all. Nothing is ever executed.
pub fn unpack(tarball: &[u8], into: &Path, rules: &Rules) -> Result<(), &'static str> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    let mut total = 0u64;
    let entries = archive.entries().map_err(|_| "store.invalid")?;
    for (n, entry) in entries.enumerate() {
        let mut entry = entry.map_err(|_| "store.invalid")?;
        if n >= rules.entries {
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
        if rules.top.is_some_and(|top| parts.first() != Some(&top)) {
            return Err("store.invalid");
        }
        let relative = parts.get(1..).unwrap_or_default().join("/");
        let regular = kind.is_file() || (kind.is_dir() && rules.strict);
        if rules.strict && !regular {
            return Err("store.invalid");
        }
        if !kind.is_file() || parts.len() > rules.depth + 1 || !(rules.keep)(&relative) {
            continue;
        }
        total = total.saturating_add(entry.header().size().map_err(|_| "store.invalid")?);
        if total > rules.bytes {
            return Err("store.too_big");
        }
        let target = into.join(&relative);
        let fail = |_: io::Error| "store.failed";
        fs::create_dir_all(archr_io::parent_dir(&target)).map_err(fail)?;
        let mode = if rules.executable == Some(relative.as_str()) {
            0o755
        } else {
            0o644
        };
        // create_new: never through a link or over an earlier entry.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&target)
            .map_err(fail)?;
        io::copy(&mut (&mut entry).take(rules.bytes), &mut file).map_err(fail)?;
    }
    Ok(())
}
