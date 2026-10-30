//! Module store: repositories with the `archr-launcher-module` topic and
//! their latest release (module.toml, preview.png, `<id>-<version>.tar.gz`),
//! the reviewed list the project publishes, and installs that check every
//! byte before anything lands in `modules/` (docs/modulos.md, "Loja").
use super::{
    manifest::{self, Manifest},
    registry::{self, Dirs, State},
    Input,
};
use crate::github::{name_ok, owner_ok, unpack, GitHub, Rules, API};
use archr_io::{read_bytes, Symlinks};
use archr_scraper::Http;
use archr_ui::modules::{Card, Offer, StoreAction, StoreReply, Tab};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::PathBuf,
    sync::mpsc,
};

pub const TOPIC: &str = "archr-launcher-module";
/// Every hop must stay on these: the API, and release downloads with their
/// redirects.
pub const HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
/// The project's repository whose latest release carries `reviewed.json`.
pub const REVIEWED: &str = "archr-linux/archr-modules";
/// The Ed25519 public key that signs the reviewed list
/// (keys/archr-modules-ed25519.pub; tools/sign-reviewed signs with its
/// private half, which never leaves the maintainer's machine).
pub const REVIEW_KEY: [u8; 32] = [
    0x63, 0x11, 0xf1, 0x62, 0x70, 0x1d, 0x99, 0x47, 0x1e, 0xa8, 0x8a, 0xc1, 0xd8, 0x43, 0x5f, 0xfc,
    0x7d, 0xd6, 0x40, 0x9c, 0x06, 0xd2, 0xc5, 0x94, 0xfd, 0x0a, 0x0e, 0xf1, 0x6d, 0x71, 0x3a, 0x71,
];
/// The oldest reviewed list this build accepts, even with nothing recorded
/// (a new device, a wiped `modules-reviewed.serial`): raise it at release
/// time to the serial then published.
pub const MIN_SERIAL: u64 = 1;
/// `reviewed.json.sig`: the Ed25519 signature of the whole `reviewed.json`,
/// 64 bytes as 128 hex digits (one line; spaces and line ends ignored).
pub fn signed(body: &[u8], signature: &[u8], key: &[u8; 32]) -> bool {
    let text: Vec<u8> = signature
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if text.len() != 128 || body.is_empty() {
        return false;
    }
    let mut raw = [0u8; 64];
    for (n, pair) in text.chunks(2).enumerate() {
        let Some(byte) = std::str::from_utf8(pair)
            .ok()
            .and_then(|h| u8::from_str_radix(h, 16).ok())
        else {
            return false;
        };
        raw[n] = byte;
    }
    let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(key) else {
        return false;
    };
    key.verify_strict(body, &ed25519_dalek::Signature::from_bytes(&raw))
        .is_ok()
}
const TARBALL_LIMIT: u64 = 8 << 20;
const EXTRACTED_LIMIT: u64 = 16 << 20;
const ENTRIES: usize = 500;
const TOML_LIMIT: u64 = 64 << 10;

/// What the host records after an install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    pub id: String,
    pub source: String,
    pub version: String,
    pub digest: String,
    pub granted: Vec<String>,
}

/// One release asset.
#[derive(Clone, Debug)]
struct Asset {
    url: String,
    size: u64,
    digest: String,
}

pub struct Shop {
    github: GitHub,
    dirs: Dirs,
    review_key: [u8; 32],
}
impl Shop {
    pub fn new(http: Box<dyn Http>, cache: PathBuf, dirs: Dirs) -> Self {
        Self {
            github: GitHub::new(http, cache),
            dirs,
            review_key: REVIEW_KEY,
        }
    }
    /// Another public key for the reviewed list: for tests only.
    pub fn with_review_key(mut self, key: [u8; 32]) -> Self {
        self.review_key = key;
        self
    }
    pub fn production(cache: PathBuf, dirs: Dirs) -> Self {
        Self::new(GitHub::curl(&cache, HOSTS), cache, dirs)
    }
    fn download(owner: &str, repo: &str, tag: &str, name: &str) -> String {
        format!("https://github.com/{owner}/{repo}/releases/download/{tag}/{name}")
    }
    /// The latest release's tag and assets (saved for an hour).
    fn release(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<(String, BTreeMap<String, Asset>), &'static str> {
        let json = self.github.json(
            &format!("{API}/repos/{owner}/{repo}/releases/latest"),
            &format!("{owner}-{repo}/release.json"),
        )?;
        let tag = json["tag_name"]
            .as_str()
            .filter(|t| name_ok(t))
            .ok_or("modules.invalid")?;
        let assets = json["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| {
                let name = a["name"].as_str().filter(|n| name_ok(n))?;
                let asset = Asset {
                    url: Self::download(owner, repo, tag, name),
                    size: a["size"].as_u64()?,
                    digest: a["digest"].as_str().unwrap_or("").to_owned(),
                };
                Some((name.to_owned(), asset))
            })
            .collect();
        Ok((tag.to_owned(), assets))
    }
    /// The reviewed list: repository and package digest of each reviewed
    /// release, from `reviewed.json` signed by the project's key
    /// (`reviewed.json.sig`). Unreachable: nothing is reviewed. Unsigned,
    /// badly signed or older than one already seen: nothing is reviewed
    /// either, and the key says why.
    fn reviewed(&self) -> Result<Vec<(String, String)>, &'static str> {
        let (owner, repo) = REVIEWED.split_once('/').expect("owner/repo");
        let Ok((_, mut assets)) = self.release(owner, repo) else {
            return Ok(Vec::new());
        };
        let Some(list) = assets.remove("reviewed.json") else {
            return Ok(Vec::new());
        };
        let fetch = |asset: &Asset, name: &str| {
            self.github
                .saved(&asset.url, name, false)
                .0
                .map(|(body, _)| body)
                .unwrap_or_default()
        };
        let body = fetch(&list, "reviewed.json");
        let signature = assets
            .remove("reviewed.json.sig")
            .map(|asset| fetch(&asset, "reviewed.json.sig"))
            .unwrap_or_default();
        if !signed(&body, &signature, &self.review_key) {
            log::warn!("The reviewed list is not signed by the ArchR key: no module is reviewed");
            return Err("modules.review_unsigned");
        }
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        // A list past its date, or older than one already seen (or than this
        // build knew), could bring back a revoked review.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if json["expires"]
            .as_u64()
            .is_none_or(|expires| expires <= now)
        {
            log::warn!("The reviewed list has expired: no module is reviewed");
            return Err("modules.review_expired");
        }
        let serial = json["serial"].as_u64().unwrap_or(0);
        let path = self.dirs.state.with_file_name("modules-reviewed.serial");
        let seen = read_bytes(&path, 64, Symlinks::Reject)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|t| t.trim().parse::<u64>().ok())
            .unwrap_or(0)
            .max(MIN_SERIAL);
        if serial < seen {
            log::warn!("The reviewed list {serial} is older than {seen}: ignored");
            return Err("modules.review_old");
        }
        if serial > seen {
            if let Err(error) = archr_io::atomic_publish(&path, false, |file| {
                std::io::Write::write_all(file, serial.to_string().as_bytes())
            }) {
                log::warn!("Cannot record the reviewed list's serial {serial}: {error}");
            }
        }
        Ok(json["modules"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                Some((
                    m["repo"].as_str()?.to_owned(),
                    m["digest"].as_str()?.to_owned(),
                ))
            })
            .collect())
    }
    /// A `.tar.gz` of the latest release carries one of `digests`. Asked
    /// only for repositories on the reviewed list (saved for an hour).
    fn current_reviewed(&self, owner: &str, repo: &str, digests: &[String]) -> bool {
        self.release(owner, repo).is_ok_and(|(_, assets)| {
            assets.iter().any(|(name, asset)| {
                name.ends_with(".tar.gz") && digests.iter().any(|d| same_digest(d, &asset.digest))
            })
        })
    }
    fn records(&self) -> State {
        State::load(&self.dirs.state)
    }

    /// One tab: reviewed ones first; Installed comes from the disk only.
    pub fn list(&self, tab: Tab, refresh: bool) -> StoreReply {
        if tab == Tab::Installed {
            return StoreReply::Listed {
                tab,
                cards: self.installed(),
                problem: None,
            };
        }
        let (repos, problem, _) = self.github.search(TOPIC, "stars", refresh);
        let (reviewed, unsigned) = match self.reviewed() {
            Ok(list) => (list, None),
            Err(key) => (Vec::new(), Some(key)),
        };
        let problem = problem.or(unsigned);
        let mut cards: Vec<Card> = repos
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                let source = format!("{}/{}", r.owner, r.repo);
                let digests: Vec<String> = reviewed
                    .iter()
                    .filter(|(repo, _)| *repo == source)
                    .map(|(_, digest)| digest.clone())
                    .collect();
                (r, digests)
            })
            .map(|(r, digests)| Card {
                // Reviewed only if a package of the current release is.
                reviewed: !digests.is_empty() && self.current_reviewed(&r.owner, &r.repo, &digests),
                digests,
                owner: r.owner,
                repo: r.repo,
                stars: r.stars,
                ..Card::default()
            })
            .filter(|c| tab == Tab::All || c.reviewed)
            .collect();
        cards.sort_by_key(|c| !c.reviewed);
        StoreReply::Listed {
            tab,
            cards,
            problem,
        }
    }
    /// Store modules on this device, as cards.
    fn installed(&self) -> Vec<Card> {
        self.records()
            .modules
            .iter()
            .filter_map(|(id, record)| {
                let (owner, repo) = record.source.split_once('/')?;
                let module = registry::Installed::read(&self.dirs.module(id), "en");
                let manifest = module.manifest.as_ref()?;
                Some(Card {
                    owner: owner.into(),
                    repo: repo.into(),
                    id: id.clone(),
                    name: text(&module, manifest.name.as_ref()),
                    description: text(&module, manifest.description.as_ref()),
                    icon: manifest.icon.clone().unwrap_or_default(),
                    version: manifest.version.clone(),
                    ..Card::default()
                })
            })
            .collect()
    }
    /// The card with the module's id, texts and version, from the latest
    /// release's module.toml (a plain download, no API call).
    pub fn decorate(&self, card: &Card) -> Option<Card> {
        if !owner_ok(&card.owner) || !name_ok(&card.repo) {
            return None;
        }
        let url = format!(
            "https://github.com/{}/{}/releases/latest/download/{}",
            card.owner,
            card.repo,
            manifest::FILE
        );
        let name = format!("{}-{}/latest-module.toml", card.owner, card.repo);
        let (Some((body, _)), _) = self.github.saved(&url, &name, false) else {
            return None;
        };
        let (manifest, _) = manifest::parse(std::str::from_utf8(&body).ok()?).ok()?;
        Some(Card {
            id: manifest.id.clone(),
            name: plain(manifest.name.as_ref()),
            description: plain(manifest.description.as_ref()),
            icon: manifest.icon.clone().unwrap_or_default(),
            version: manifest.version,
            ..card.clone()
        })
    }
    /// The permission page: what the latest release asks for, checked
    /// against its tag, its assets and this device.
    pub fn offer(&self, card: &Card) -> Result<Offer, &'static str> {
        let (manifest, tag, assets) = self.manifest(card)?;
        let package = assets
            .get(&format!("{}-{}.tar.gz", manifest.id, manifest.version))
            .ok_or("modules.no_package")?;
        let preview = assets.get("preview.png").and_then(|asset| {
            let path = self
                .github
                .cache
                .join(format!("{}-{}", card.owner, card.repo))
                .join(&tag)
                .join("preview.png");
            self.github.picture(&asset.url, &path)
        });
        let records = self.records();
        let record = records.modules.get(&manifest.id);
        let compatible = manifest::api_ok(manifest.api_version)
            && !archr_ui::settings::store::newer(&manifest.min_launcher, env!("CARGO_PKG_VERSION"));
        let blocked = match record {
            _ if !compatible => Some("modules.incompatible"),
            Some(r) if r.source.is_empty() && self.dirs.module(&manifest.id).exists() => {
                Some("modules.name_taken")
            }
            Some(r) if !r.source.is_empty() && r.source != card.source() => {
                Some("modules.other_source")
            }
            _ => None,
        };
        let granted = record.map(|r| r.granted.clone()).unwrap_or_default();
        let installed = record
            .filter(|r| r.source == card.source())
            .map(|r| r.version.clone());
        // Access permissions only, without what it adds to the screens.
        let access = |permissions: manifest::Permissions| {
            let mut bare = manifest.clone();
            bare.permissions = permissions;
            bare.tiles.clear();
            bare.shortcuts.clear();
            bare.widgets.clear();
            bare.quick.clear();
            bare.pages.clear();
            bare.mode = manifest::Mode::Oneshot;
            registry::permissions(&bare)
                .into_iter()
                .filter(|p| p.key != "modules.perm_none")
                .collect::<Vec<_>>()
        };
        let (added, kept) = if installed.is_some() {
            let new = manifest.permissions.without(&granted);
            let kept = manifest.permissions.only(&granted);
            let mut added = access(new);
            // New places on the screen and a process that stays open are
            // new too: the user sees them, and an update on its own never
            // brings them.
            let old = registry::Installed::read(&self.dirs.module(&manifest.id), "en").manifest;
            added.extend(new_surface(&manifest, old.as_ref()));
            (added, access(kept))
        } else {
            (Vec::new(), Vec::new())
        };
        let reviewed = self
            .reviewed()
            .unwrap_or_default()
            .iter()
            .any(|(repo, digest)| *repo == card.source() && same_digest(digest, &package.digest));
        Ok(Offer {
            card: Card {
                id: manifest.id.clone(),
                name: plain(manifest.name.as_ref()),
                description: plain(manifest.description.as_ref()),
                icon: manifest.icon.clone().unwrap_or_default(),
                version: manifest.version.clone(),
                reviewed,
                ..card.clone()
            },
            version: manifest.version.clone(),
            author: manifest.author.clone(),
            license: manifest.license.clone(),
            size: package.size,
            digest: package.digest.clone(),
            permissions: registry::permissions(&manifest),
            added,
            kept,
            preview,
            installed,
            reviewed,
            blocked,
        })
    }
    /// module.toml of the latest release: parsed, with its tag and assets.
    fn manifest(
        &self,
        card: &Card,
    ) -> Result<(Manifest, String, BTreeMap<String, Asset>), &'static str> {
        if !owner_ok(&card.owner) || !name_ok(&card.repo) {
            return Err("modules.invalid");
        }
        let (tag, assets) = self.release(&card.owner, &card.repo)?;
        let source = self.standalone(card, &tag, &assets)?;
        let (manifest, _) =
            manifest::parse(std::str::from_utf8(&source).map_err(|_| "modules.invalid")?)
                .map_err(|_| "modules.invalid")?;
        if tag != format!("v{}", manifest.version) {
            return Err("modules.bad_tag");
        }
        Ok((manifest, tag, assets))
    }
    /// The release's module.toml, kept with the tag it came from.
    fn standalone(
        &self,
        card: &Card,
        tag: &str,
        assets: &BTreeMap<String, Asset>,
    ) -> Result<Vec<u8>, &'static str> {
        let path = self
            .github
            .cache
            .join(format!("{}-{}", card.owner, card.repo))
            .join(tag)
            .join(manifest::FILE);
        if let Ok(bytes) = read_bytes(&path, TOML_LIMIT, Symlinks::Reject) {
            return Ok(bytes);
        }
        let asset = assets.get(manifest::FILE).ok_or("modules.no_package")?;
        let body = self
            .github
            .get(&asset.url, TOML_LIMIT)?
            .ok_or("store.failed")?;
        self.github.keep(&path, &body);
        Ok(body)
    }
    /// Downloads the package the offer showed, checks its digest and that it
    /// holds the very module.toml shown, and swaps it in; the version it
    /// replaces waits in `.previous-<id>` for the new one's first good run.
    pub fn install(&self, offer: &Offer) -> Result<Installed, &'static str> {
        if let Some(blocked) = offer.blocked {
            return Err(blocked);
        }
        let card = &offer.card;
        let (manifest, tag, assets) = self.manifest(card)?;
        if manifest.id != card.id || manifest.version != offer.version {
            return Err("modules.invalid");
        }
        let id = &manifest.id;
        let package = assets
            .get(&format!("{id}-{}.tar.gz", manifest.version))
            .ok_or("modules.no_package")?;
        // The package the user saw, not one published since.
        if !same_digest(&package.digest, &offer.digest) {
            return Err("modules.digest_mismatch");
        }
        let expected = package
            .digest
            .strip_prefix("sha256:")
            .filter(|d| d.len() == 64)
            .ok_or("modules.no_digest")?;
        if package.size > TARBALL_LIMIT {
            return Err("modules.too_big");
        }
        let tarball = match self.github.get(&package.url, TARBALL_LIMIT) {
            Ok(Some(body)) => body,
            Err("store.offline") => return Err("modules.download_failed"),
            _ => return Err("store.failed"),
        };
        let digest: String = Sha256::digest(&tarball)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if digest != expected {
            return Err("modules.digest_mismatch");
        }
        let fail = |_: std::io::Error| "store.failed";
        let modules = &self.dirs.modules;
        fs::create_dir_all(modules).map_err(fail)?;
        let staging = modules.join(format!(".staging-{id}"));
        let _ = fs::remove_dir_all(&staging);
        let unpacked = staging.join(id);
        fs::create_dir_all(&unpacked).map_err(fail)?;
        let result = (|| {
            let rules = Rules {
                top: Some(id),
                keep: &|_| true,
                strict: true,
                entries: ENTRIES,
                bytes: EXTRACTED_LIMIT,
                depth: 8,
                executable: Some(&manifest.entry),
            };
            unpack(&tarball, &unpacked, &rules).map_err(module_key)?;
            let shown = self.standalone(card, &tag, &assets)?;
            let packed = read_bytes(&unpacked.join(manifest::FILE), TOML_LIMIT, Symlinks::Reject)
                .map_err(|_| "modules.invalid")?;
            if packed != shown {
                return Err("modules.manifest_mismatch");
            }
            manifest::check(&unpacked).map_err(|errors| {
                log::warn!("Module {id} from the store: {}", errors.join("; "));
                "modules.invalid"
            })?;
            let (target, previous) = (self.dirs.module(id), self.dirs.previous(id));
            if target.exists() {
                let _ = fs::remove_dir_all(&previous);
                fs::rename(&target, &previous).map_err(fail)?;
            }
            fs::rename(&unpacked, &target).map_err(fail)
        })();
        let _ = fs::remove_dir_all(&staging);
        result?;
        Ok(Installed {
            id: id.clone(),
            source: card.source(),
            version: manifest.version.clone(),
            digest: package.digest.clone(),
            granted: manifest.permissions.tokens().into_iter().collect(),
        })
    }
    /// Module `id`, if its latest release is reviewed (its package's digest
    /// on the list), newer, and asks for nothing new: installed through the
    /// same checks as a manual install.
    pub fn auto(&self, id: &str) -> Option<Installed> {
        let records = self.records();
        let record = records.modules.get(id)?;
        // Off, failed or never allowed: the user decides about it first.
        if !record.enabled || record.failed || !record.consent {
            return None;
        }
        let (owner, repo) = record.source.split_once('/')?;
        let card = Card {
            owner: owner.into(),
            repo: repo.into(),
            id: id.to_owned(),
            ..Card::default()
        };
        let offer = self.offer(&card).ok()?;
        let newer = offer
            .installed
            .as_deref()
            .is_some_and(|v| archr_ui::settings::store::newer(&offer.version, v));
        if !(newer && offer.reviewed && offer.added.is_empty() && offer.blocked.is_none()) {
            return None;
        }
        // It failed here once and was rolled back.
        if same_digest(&offer.digest, &record.bad_digest) {
            return None;
        }
        self.install(&offer)
            .map_err(|problem| log::warn!("Automatic update of {id} failed: {problem}"))
            .ok()
    }
    fn handle(
        &self,
        action: StoreAction,
        queue: &mut VecDeque<Card>,
    ) -> (StoreReply, Option<Installed>) {
        match action {
            StoreAction::List(tab, refresh) => {
                let listed = self.list(tab, refresh);
                if let StoreReply::Listed { cards, .. } = &listed {
                    if tab != Tab::Installed {
                        *queue = cards.iter().cloned().collect();
                    }
                }
                (listed, None)
            }
            StoreAction::Open(card) => (
                StoreReply::Offer(card.source(), self.offer(&card).map(Box::new)),
                None,
            ),
            StoreAction::Auto(_) => (StoreReply::Done(Ok("")), None),
            StoreAction::Install(offer) => match self.install(&offer) {
                Ok(installed) => (
                    StoreReply::Done(Ok("modules.installed_ok")),
                    Some(installed),
                ),
                Err(problem) => {
                    log::warn!(
                        "Module store install of {} failed: {problem}",
                        offer.card.source()
                    );
                    (StoreReply::Done(Err(problem)), None)
                }
            },
        }
    }
}

/// Two `sha256:<hex>` digests name the same package.
pub fn same_digest(a: &str, b: &str) -> bool {
    let hex = |d: &str| d.strip_prefix("sha256:").unwrap_or(d).to_ascii_lowercase();
    !hex(a).is_empty() && hex(a) == hex(b)
}

/// What `new` shows or keeps running that `old` did not: one line per kind,
/// as the permission page names it. Without the old manifest, all of it.
fn new_surface(new: &Manifest, old: Option<&Manifest>) -> Vec<archr_ui::modules::Permission> {
    let ids = |list: &[archr_ui::modules::Entry]| -> Vec<String> {
        list.iter().map(|e| e.id.clone()).collect()
    };
    let grows = |new: Vec<String>, old: Option<Vec<String>>| {
        let old = old.unwrap_or_default();
        new.iter().any(|id| !old.contains(id))
    };
    let kinds = [
        (
            "modules.perm_tile",
            grows(ids(&new.tiles), old.map(|o| ids(&o.tiles))),
        ),
        (
            "modules.perm_shortcut",
            grows(ids(&new.shortcuts), old.map(|o| ids(&o.shortcuts))),
        ),
        (
            "modules.perm_widget",
            grows(ids(&new.widgets), old.map(|o| ids(&o.widgets))),
        ),
        (
            "modules.perm_quick",
            grows(
                new.quick.iter().map(|q| q.id.clone()).collect(),
                old.map(|o| o.quick.iter().map(|q| q.id.clone()).collect()),
            ),
        ),
        (
            "modules.perm_page",
            grows(
                new.pages.iter().map(|p| p.id.clone()).collect(),
                old.map(|o| o.pages.iter().map(|p| p.id.clone()).collect()),
            ),
        ),
        (
            "modules.perm_daemon",
            new.mode == manifest::Mode::Daemon
                && old.is_none_or(|o| o.mode != manifest::Mode::Daemon),
        ),
    ];
    kinds
        .into_iter()
        .filter(|(_, new)| *new)
        .map(|(key, _)| archr_ui::modules::Permission {
            key,
            detail: String::new(),
            sensitive: false,
        })
        .collect()
}

/// The unpacker speaks of themes; the module store has its own words.
fn module_key(key: &'static str) -> &'static str {
    match key {
        "store.invalid" => "modules.invalid",
        "store.too_big" => "modules.too_big",
        other => other,
    }
}

/// `name`/`description` without the module's catalog: text or a language
/// table; an `@key` has no text before the install.
fn plain(value: Option<&toml::Value>) -> String {
    match value {
        Some(toml::Value::String(text)) if !text.starts_with('@') => text.clone(),
        Some(table @ toml::Value::Table(_)) => {
            archr_ui::layout::Names::parse(Some(table), "module")
                .get("en")
                .unwrap_or_default()
                .to_owned()
        }
        _ => String::new(),
    }
}
fn text(module: &registry::Installed, value: Option<&toml::Value>) -> String {
    match value {
        Some(toml::Value::String(text)) => module.tr(text),
        other => plain(other),
    }
}

/// The store's own lane: requests first, then the cards still to fill in.
pub(super) fn lane(
    shop: &Shop,
    requests: &mpsc::Receiver<StoreAction>,
    results: &mpsc::Sender<Input>,
) {
    let mut queue = VecDeque::new();
    loop {
        let action = match requests.try_recv() {
            Ok(action) => Some(action),
            Err(mpsc::TryRecvError::Empty) if queue.is_empty() => match requests.recv() {
                Ok(action) => Some(action),
                Err(_) => return,
            },
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => return,
        };
        if let Some(StoreAction::Auto(id)) = &action {
            let answer = match shop.auto(id) {
                Some(installed) => (
                    StoreReply::Updated(installed.id.clone(), installed.version.clone()),
                    Some(installed),
                ),
                None => (StoreReply::Checked(id.clone()), None),
            };
            if results.send(Input::Store(Box::new(answer))).is_err() {
                return;
            }
            continue;
        }
        let (reply, installed) = match action {
            Some(action) => shop.handle(action, &mut queue),
            None => match queue.pop_front().and_then(|card| shop.decorate(&card)) {
                Some(card) => (StoreReply::Card(card), None),
                None => continue,
            },
        };
        if results
            .send(Input::Store(Box::new((reply, installed))))
            .is_err()
        {
            return;
        }
    }
}
