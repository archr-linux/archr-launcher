//! API 4 on the host: what an app's screens may show, and the games and
//! PortMaster ports it may install (docs/modulos.md, section 10). Every path
//! a module names is relative to its own folder and checked here; what it
//! installed is recorded where it cannot write.
use super::registry::Installed;
use archr_index::SystemDef;
use archr_ui::apps::{
    Action, Ask, Change, Chip, Fact, Item, Layout, Message, Patch, Screen, Target, BUTTONS, CHIPS,
    FACTS, GALLERY, PAGE,
};
use archr_ui::modules::Tone;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, BufReader, Read},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

/// Pictures a screen shows, and a game the module installs, at most.
pub const IMAGE_BYTES: u64 = 4 << 20;
pub const ROM_BYTES: u64 = 64 << 20;
/// All the games one module installed, together.
pub const MODULE_BYTES: u64 = 2 << 30;
/// A module's description text, in characters.
const TEXT: usize = 4000;
/// The ledger of what each module installed, beside its data folders.
pub const LEDGER: &str = "installed.toml";
/// The system folder PortMaster's launch scripts live in.
pub const PORTS: &str = "ports";
/// Files the device itself would run (a shell, a JVM, a desktop entry),
/// never installed as a game, nor into a system that takes them: its
/// entries run as root. `.exe`/`.bat` of an emulated PC are fine.
pub const SCRIPTS: &[&str] = &[
    ".sh",
    ".bash",
    ".run",
    ".appimage",
    ".desktop",
    ".py",
    ".pl",
    ".jar",
];
/// The systems runemu.sh starts in a shell instead of an emulator.
const SHELLS: &[&str] = &["ports", "windows", "setup", "shell"];
/// A PortMaster install downloads up to a few hundred MB; stopped, the
/// script gets this long to take back what it began.
const PORT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
pub const PORT_GRACE: Duration = Duration::from_secs(15);

/// PNG, JPEG or WebP, by their first bytes.
pub fn picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || (bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

/// A regular file inside `home` by a relative path (no `..`, no absolute
/// path), opened one folder at a time without following any link, at most
/// `limit` bytes and with no other name. What is checked is the handle the
/// launcher then reads: renaming or linking afterwards changes nothing.
pub fn open_inside(home: &Path, relative: &str, limit: u64) -> Option<(fs::File, u64)> {
    use std::os::unix::fs::MetadataExt;
    let parts: Vec<&str> = relative.split('/').collect();
    let plain = !relative.is_empty()
        && relative.len() <= 256
        && !relative.chars().any(char::is_control)
        && parts
            .iter()
            .all(|part| !part.is_empty() && *part != ".." && *part != ".");
    if !plain {
        return None;
    }
    let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let mut dir = openat(libc::AT_FDCWD, home.as_os_str(), flags | libc::O_DIRECTORY)?;
    let (last, folders) = parts.split_last()?;
    for part in folders {
        dir = openat(dir.as_raw_fd(), part.as_ref(), flags | libc::O_DIRECTORY)?;
    }
    // Non-blocking: a FIFO never holds the host.
    let file = fs::File::from(openat(
        dir.as_raw_fd(),
        last.as_ref(),
        flags | libc::O_NONBLOCK,
    )?);
    let meta = file.metadata().ok()?;
    (meta.is_file() && meta.len() <= limit && meta.nlink() == 1).then_some((file, meta.len()))
}
fn openat(dir: RawFd, name: &std::ffi::OsStr, flags: i32) -> Option<OwnedFd> {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(name.as_bytes()).ok()?;
    // SAFETY: a valid C string and a directory descriptor (or AT_FDCWD).
    let fd = unsafe { libc::openat(dir, name.as_ptr(), flags) };
    // SAFETY: a descriptor openat just returned, owned from here on.
    (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) })
}
/// The bytes of a picture inside `home`: up to 4 MiB of PNG, JPEG or WebP.
pub fn picture_bytes(home: &Path, relative: &str) -> Option<Vec<u8>> {
    let (file, size) = open_inside(home, relative, IMAGE_BYTES)?;
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(IMAGE_BYTES + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= IMAGE_BYTES && picture(&bytes)).then_some(bytes)
}
/// A picture of the module's screens, copied from the handle into `store`
/// (the launcher's, out of the module's reach) under a name from its
/// content: the screens decode that copy, never the module's file.
pub fn image(
    home: &Path,
    relative: &str,
    store: &Path,
    pictures: &mut Pictures,
) -> Option<PathBuf> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::MetadataExt;
    let (file, size) = open_inside(home, relative, IMAGE_BYTES)?;
    let meta = file.metadata().ok()?;
    let seen = (
        meta.dev(),
        meta.ino(),
        size,
        meta.mtime(),
        meta.mtime_nsec(),
    );
    if let Some(path) = pictures.seen.get(&seen) {
        return Some(path.clone());
    }
    if pictures.seen.len() >= PICTURES || pictures.bytes + size > PICTURE_BYTES {
        log::warn!("An app's screens hold too many pictures: {relative} refused");
        return None;
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(IMAGE_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > IMAGE_BYTES || !picture(&bytes) {
        return None;
    }
    let digest = Sha256::digest(&bytes);
    let name: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
    let path = store.join(format!("{name}.{}", kind(&bytes)));
    if !path.is_file() {
        fs::create_dir_all(store).ok()?;
        archr_io::atomic_publish(&path, false, |file| io::Write::write_all(file, &bytes)).ok()?;
    }
    pictures.bytes += size;
    pictures.seen.insert(seen, path.clone());
    Some(path)
}
/// Pictures one app session copied, by the file they came from (device,
/// inode, size, change time): read once, and no more than `PICTURES` files
/// nor `PICTURE_BYTES` in all.
#[derive(Default)]
pub struct Pictures {
    seen: std::collections::HashMap<(u64, u64, u64, i64, i64), PathBuf>,
    bytes: u64,
}
const PICTURES: usize = 256;
const PICTURE_BYTES: u64 = 64 << 20;
/// The extension of a `picture`.
fn kind(bytes: &[u8]) -> &'static str {
    match bytes.first() {
        Some(0x89) => "png",
        Some(0xff) => "jpg",
        _ => "webp",
    }
}

/// What turns a module's message into what the screens draw: its texts,
/// its folder for pictures and its icons.
pub struct Reader<'a> {
    pub installed: &'a Installed,
    pub home: &'a Path,
    /// Where the pictures the screens show are copied, and what was.
    pub store: &'a Path,
    pub pictures: &'a mut Pictures,
    /// A Lucide name to the module's own unmodified SVG.
    pub icon: &'a mut dyn FnMut(&str) -> Option<Arc<str>>,
}
impl Reader<'_> {
    fn text(&self, value: &Json) -> String {
        self.installed.message(value).unwrap_or_default()
    }
    /// Text with its line breaks, as long as a description gets.
    fn long(&self, value: &Json) -> String {
        let text = match value {
            Json::String(text) => self.installed.tr(text),
            _ => return String::new(),
        };
        text.chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .take(TEXT)
            .collect()
    }
    fn image(&mut self, value: &Json) -> Option<PathBuf> {
        image(self.home, value.as_str()?, self.store, self.pictures)
    }
    fn item(&mut self, value: &Json) -> Option<Item> {
        Some(Item {
            id: name(&value["id"])?,
            title: self.text(&value["title"]),
            subtitle: self.text(&value["subtitle"]),
            badge: self.text(&value["badge"]),
            tone: Tone::parse(value["tone"].as_str().unwrap_or("")),
            image: self.image(&value["image"]),
        })
    }
    fn items(&mut self, value: &Json) -> Vec<Item> {
        list(value, PAGE).filter_map(|v| self.item(v)).collect()
    }
    fn chips(&self, value: &Json) -> Vec<Chip> {
        list(value, CHIPS)
            .filter_map(|v| {
                Some(Chip {
                    id: name(&v["id"])?,
                    label: self.text(&v["label"]),
                    selected: v["selected"] == true,
                })
            })
            .collect()
    }
    fn action(&mut self, value: &Json, id: &str) -> Option<Action> {
        let icon = value["icon"].as_str().and_then(&mut *self.icon);
        Some(Action {
            id: name(&value["id"]).unwrap_or_else(|| id.to_owned()),
            label: self.text(&value["label"]),
            icon,
            action: name(&value["action"])?,
            progress: percent(&value["progress"]),
        })
    }
    fn screen(&mut self, params: &Json) -> Option<Screen> {
        let layout = match params["layout"].as_str() {
            Some("collection") => Layout::Collection,
            Some("detail") => Layout::Detail,
            _ => return None,
        };
        let buttons = &params["buttons"];
        let (x, y, buttons) = match layout {
            Layout::Collection => (
                self.action(&buttons["x"], "x"),
                self.action(&buttons["y"], "y"),
                Vec::new(),
            ),
            Layout::Detail => {
                let list: Vec<Json> = list(buttons, BUTTONS).cloned().collect();
                let all = list
                    .iter()
                    .enumerate()
                    .filter_map(|(n, b)| self.action(b, &format!("b{n}")))
                    .collect();
                (None, None, all)
            }
        };
        Some(Screen {
            id: name(&params["id"])?,
            title: self.text(&params["title"]),
            layout,
            chips: self.chips(&params["chips"]),
            items: self.items(&params["items"]),
            more: params["more"] == true,
            x,
            y,
            footer: self.text(&params["footer"]),
            image: self.image(&params["image"]),
            gallery: list(&params["gallery"], GALLERY)
                .filter_map(|v| self.image(v))
                .collect(),
            subtitle: self.text(&params["subtitle"]),
            facts: list(&params["facts"], FACTS)
                .map(|f| Fact {
                    title: self.text(&f["title"]),
                    value: self.text(&f["value"]),
                    tone: Tone::parse(f["tone"].as_str().unwrap_or("")),
                })
                .collect(),
            text: self.long(&params["text"]),
            buttons,
            ..Screen::default()
        })
    }
    fn patch(&mut self, params: &Json) -> Patch {
        let given = |key: &str| params.get(key).filter(|v| !v.is_null());
        let change = Change {
            label: given("label")
                .or_else(|| given("title"))
                .map(|v| self.text(v)),
            subtitle: given("subtitle").map(|v| self.text(v)),
            badge: given("badge").map(|v| self.text(v)),
            tone: given("tone").map(|v| Tone::parse(v.as_str().unwrap_or(""))),
            image: given("image").and_then(|v| self.image(v)),
            // `null` takes the bar away; absent leaves it.
            progress: params.get("progress").map(percent),
            action: given("action").and_then(name),
        };
        let target = params["target"]
            .as_str()
            .and_then(|t| match t.split_once(':') {
                Some(("item", id)) => Some(Target::Item(id.to_owned())),
                Some(("button", id)) => Some(Target::Button(id.to_owned())),
                _ => None,
            });
        Patch {
            screen: name(&params["screen"]),
            append: self.items(&params["append"]),
            more: params["more"].as_bool(),
            chips: given("chips").map(|v| self.chips(v)),
            footer: given("footer").map(|v| self.text(v)),
            target: target.map(|t| (t, change)),
        }
    }
    /// A `screen.*` notification as the screens take it.
    pub fn message(&mut self, method: &str, params: &Json) -> Option<Message> {
        let text = |key: &str| {
            params
                .get(key)
                .filter(|v| !v.is_null())
                .map(|v| self.text(v))
        };
        Some(match method {
            "screen.show" => Message::Show(Box::new(self.screen(params)?)),
            "screen.patch" => Message::Patch(Box::new(self.patch(params))),
            "screen.toast" => Message::Toast(
                text("text").filter(|t| !t.is_empty())?,
                Tone::parse(params["tone"].as_str().unwrap_or("")),
            ),
            "screen.confirm" => Message::Confirm(Ask {
                id: name(&params["id"])?,
                text: text("text")?,
                yes: text("yes").unwrap_or_default(),
                no: text("no").unwrap_or_default(),
                on_yes: false,
                launcher: false,
            }),
            "screen.busy" => Message::Busy(text("text")),
            // `text: null` (or `offline: false`) takes the tag away.
            "screen.offline"
                if params["offline"] == false
                    || params["text"].is_null() && params.get("text").is_some() =>
            {
                Message::Offline(None)
            }
            "screen.offline" => Message::Offline(Some(text("text").unwrap_or_default())),
            _ => return None,
        })
    }
}
/// The first `most` entries of a JSON list.
fn list(value: &Json, most: usize) -> impl Iterator<Item = &Json> {
    value.as_array().into_iter().flatten().take(most)
}
/// The module's own short names: ids, actions.
fn name(value: &Json) -> Option<String> {
    value
        .as_str()
        .filter(|t| (1..=64).contains(&t.len()) && !t.chars().any(char::is_control))
        .map(str::to_owned)
}
fn percent(value: &Json) -> Option<u8> {
    value.as_f64().map(|p| p.clamp(0., 100.).round() as u8)
}

/// What each module installed: written by the launcher only, outside every
/// module's folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub modules: BTreeMap<String, Owned>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Owned {
    pub games: Vec<Game>,
    pub ports: Vec<Port>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Game {
    pub game_key: String,
    pub system: String,
    pub rom: PathBuf,
    pub cover: Option<PathBuf>,
    pub size: u64,
    /// When the copy was last changed (nanoseconds since 1970): a file
    /// changed since is not the game installed, and is not removed.
    pub mtime: u64,
    /// Seconds since 1970.
    pub installed: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Port {
    pub name: String,
    /// Its launch script, `ports/<file>`.
    pub game_key: String,
    /// What its files took after the install.
    pub size: u64,
    pub installed: u64,
}
impl Ledger {
    pub fn load(path: &Path) -> Self {
        archr_io::read_utf8(path, 1 << 20, archr_io::Symlinks::Reject)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let text = toml::to_string(self).map_err(io::Error::other)?;
        archr_io::atomic_publish(path, true, |file| {
            io::Write::write_all(file, text.as_bytes())
        })
    }
}
impl Owned {
    /// The games still on disk, and the bytes they take.
    pub fn present(&self) -> (Vec<&Game>, u64) {
        let games: Vec<&Game> = self.games.iter().filter(|g| g.rom.is_file()).collect();
        let bytes = games.iter().map(|g| g.size).sum();
        (games, bytes)
    }
    /// What its games and ports take, against `MODULE_BYTES`.
    pub fn used(&self) -> u64 {
        self.present().1 + self.ports.iter().map(|p| p.size).sum::<u64>()
    }
}
/// The game is still the file that was installed.
pub fn unchanged(game: &Game) -> bool {
    fs::symlink_metadata(&game.rom)
        .is_ok_and(|m| m.is_file() && m.len() == game.size && mtime(&m) == game.mtime)
}
fn mtime(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

/// A file name the user's folders take: no path, no control characters,
/// no leading dot, at most 96 characters.
pub fn safe_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    // Words of dots alone (`..`) go.
    let name = name
        .split_whitespace()
        .filter(|word| !word.chars().all(|c| c == '.'))
        .collect::<Vec<_>>()
        .join(" ");
    let name = name.trim_start_matches('.').trim();
    name.chars().take(96).collect::<String>().trim().to_owned()
}
/// `<name><ext>` in `dir`, or `<name> (2)<ext>`... when it is taken.
pub fn free_name(dir: &Path, name: &str, ext: &str) -> PathBuf {
    (1..)
        .map(|n| match n {
            1 => dir.join(format!("{name}{ext}")),
            n => dir.join(format!("{name} ({n}){ext}")),
        })
        .find(|path| fs::symlink_metadata(path).is_err())
        .expect("a free name")
}

type Refusal = (i64, &'static str);

/// `rom.install`: the module's file copied into the system's folder (the
/// first one, where a copy over the network also lands), its cover into
/// `media/images/`, and its entry in the gamelist. `used` is what the
/// module's games already take.
pub fn install(
    home: &Path,
    params: &Json,
    allowed: &[String],
    (systems, roms): (&[SystemDef], &Path),
    used: u64,
) -> Result<Game, Refusal> {
    check(home, params, allowed, (systems, roms), used)?.run()
}
/// Everything `rom.install` checks before it copies anything.
pub fn check(
    home: &Path,
    params: &Json,
    allowed: &[String],
    (systems, roms): (&[SystemDef], &Path),
    used: u64,
) -> Result<Copy, Refusal> {
    let system = params["system"].as_str().ok_or((-32602, "system"))?;
    if system == PORTS || !allowed.iter().any(|s| s == system) {
        return Err((-32001, "permission denied"));
    }
    let def = systems
        .iter()
        .find(|s| s.name == system)
        .ok_or((-32004, "no such system"))?;
    // Only a ROM folder of games: never one whose entries the device runs.
    let script = |ext: &str| SCRIPTS.iter().any(|s| s.eq_ignore_ascii_case(ext));
    // Only an emulated system: launched by runemu.sh, not in its shell.
    if !def.path.starts_with(roms)
        || !def.command.trim_start().starts_with("/usr/bin/runemu.sh")
        || SHELLS.contains(&def.name.as_str())
        || def.extensions.iter().any(|e| script(e))
    {
        return Err((-32001, "not a games folder"));
    }
    let file = params["file"].as_str().ok_or((-32602, "file"))?;
    let (source, size) = open_inside(home, file, ROM_BYTES).ok_or((-32602, "file"))?;
    let named = Path::new(file);
    let ext = named
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .ok_or((-32602, "extension"))?;
    if script(&ext) || !def.extensions.iter().any(|e| e.to_lowercase() == ext) {
        return Err((-32602, "extension"));
    }
    if used + size > MODULE_BYTES {
        return Err((-32003, "quota exceeded"));
    }
    // A cover that is not a sound picture is left out; the game is not.
    let cover = params["cover"]
        .as_str()
        .and_then(|cover| picture_bytes(home, cover));
    let stem = named.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let name = Some(safe_name(params["name"].as_str().unwrap_or("")))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| safe_name(stem));
    if name.is_empty() {
        return Err((-32602, "name"));
    }
    Ok(Copy {
        def: def.clone(),
        source,
        size,
        ext,
        name,
        cover,
        meta: params["meta"].clone(),
    })
}
/// A checked `rom.install`, for a worker to carry out: the game's file is
/// the handle opened while checking, the cover the bytes read then.
pub struct Copy {
    def: SystemDef,
    source: fs::File,
    size: u64,
    ext: String,
    name: String,
    cover: Option<Vec<u8>>,
    meta: Json,
}
impl Copy {
    pub fn run(self) -> Result<Game, Refusal> {
        let Self {
            def,
            source,
            size,
            ext,
            name,
            cover,
            meta,
        } = self;
        let root = &def.path;
        fs::create_dir_all(root).map_err(|_| (-32000, "cannot write"))?;
        let rom = publish(root, &name, &ext, |file| {
            let copied = io::copy(&mut (&source).take(size + 1), file)?;
            if copied == size {
                Ok(())
            } else {
                Err(io::Error::other("the file changed while it was copied"))
            }
        })
        .map_err(|_| (-32000, "cannot write"))?;
        let cover = cover.and_then(|bytes| {
            let images = root.join("media/images");
            let stem = rom.file_stem()?.to_str()?;
            fs::create_dir_all(&images).ok()?;
            let ext = format!(".{}", kind(&bytes));
            publish(&images, stem, &ext, |file| {
                io::Write::write_all(file, &bytes)
            })
            .ok()
        });
        let field = |key: &str, max: usize| -> String {
            meta[key]
                .as_str()
                .unwrap_or("")
                .chars()
                .filter(|c| *c == '\n' || !c.is_control())
                .take(max)
                .collect()
        };
        let mut fields = vec![("name", name.clone())];
        fields.push(("desc", field("description", TEXT)));
        fields.push(("developer", field("developer", 120)));
        if let Some(year) = meta["year"].as_u64().filter(|y| (1950..=2100).contains(y)) {
            fields.push(("releasedate", format!("{year}0101T000000")));
        }
        if let Some(cover) = &cover {
            fields.push(("image", cover.display().to_string()));
        }
        let update = archr_index::GameUpdate {
            rom: rom.clone(),
            fields,
        };
        if let Err(error) = archr_index::update_gamelist(root, &[update], true) {
            log::warn!("Cannot add {} to the gamelist: {error}", rom.display());
        }
        Ok(Game {
            mtime: fs::metadata(&rom).map_or(0, |m| mtime(&m)),
            game_key: crate::store::game_key(&def, &rom).unwrap_or_default(),
            system: def.name.clone(),
            rom,
            cover,
            size,
            installed: super::now_secs(),
        })
    }
}
/// Writes a new file `<name><ext>` in `dir` (or `<name> (2)<ext>`...),
/// whole, and never over one that is there, even one that appears while it
/// is written: it is renamed into place only where no file is.
fn publish(
    dir: &Path,
    name: &str,
    ext: &str,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    // A name of its own (time, process, try); what a crash left behind
    // over an hour ago goes.
    let hour = std::time::SystemTime::now() - Duration::from_secs(3600);
    for entry in fs::read_dir(dir)?.flatten() {
        let stale = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(".archr-install-") && n.ends_with(".tmp"))
            && entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t < hour);
        if stale {
            let _ = fs::remove_file(entry.path());
        }
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let (temp, mut file) = (0..100)
        .find_map(|n| {
            let temp = dir.join(format!(
                ".archr-install-{}-{nanos}-{n}.tmp",
                std::process::id()
            ));
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp);
            file.ok().map(|f| (temp, f))
        })
        .ok_or_else(|| io::Error::other("no temporary name"))?;
    let written = write(&mut file).and_then(|()| file.sync_all());
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    let from = std::ffi::CString::new(temp.as_os_str().as_bytes()).map_err(io::Error::other)?;
    for _ in 0..1000 {
        let to = free_name(dir, name, ext);
        let target = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(io::Error::other)?;
        // SAFETY: two valid C strings; renameat2 only reads them.
        let done = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                target.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if done == 0 {
            return Ok(to);
        }
        match io::Error::last_os_error().raw_os_error() {
            Some(libc::EEXIST) => continue,
            // A filesystem without it: the name was free a moment ago.
            Some(libc::EINVAL | libc::ENOSYS) if fs::symlink_metadata(&to).is_err() => {
                fs::rename(&temp, &to)?;
                return Ok(to);
            }
            _ => break,
        }
    }
    let _ = fs::remove_file(&temp);
    Err(io::Error::other("no name to write to"))
}
/// `rom.remove`: the ROM, its cover and its gamelist entry.
pub fn remove(game: &Game, systems: &[SystemDef]) {
    let _ = fs::remove_file(&game.rom);
    if let Some(cover) = &game.cover {
        let _ = fs::remove_file(cover);
    }
    if let Some(def) = systems.iter().find(|s| s.name == game.system) {
        let root = def.root_of(&game.rom);
        if let Err(error) = archr_index::remove_games(root, std::slice::from_ref(&game.rom)) {
            log::warn!(
                "Cannot take {} off the gamelist: {error}",
                game.rom.display()
            );
        }
    }
}

/// A PortMaster catalog file name: `^[a-z0-9][a-z0-9._-]*\.zip$`, up to 64.
pub fn port_ok(name: &str) -> bool {
    name.len() <= 64
        && name.strip_suffix(".zip").is_some_and(|stem| {
            stem.bytes()
                .next()
                .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                && stem
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        })
}
/// The system script that installs or uninstalls a port, never a command
/// of the module: `archr-port-install install|uninstall <name>`.
pub fn port_command(bin: &Path, install: bool, name: &str) -> Option<std::process::Command> {
    if !port_ok(name) {
        return None;
    }
    let mut command = std::process::Command::new(bin.join("archr-port-install"));
    command
        .arg(if install { "install" } else { "uninstall" })
        .arg(name)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    Some(command)
}
/// Runs the port script to its end, at most `PORT_TIMEOUT`, or until `stop`;
/// each `PROGRESS` line goes to `progress`. The error is the script's last
/// line on stderr.
pub fn run_port(
    bin: &Path,
    install: bool,
    name: &str,
    stop: &AtomicBool,
    progress: impl Fn(u8) + Send + 'static,
) -> Result<(), (i64, String)> {
    let mut command = port_command(bin, install, name).ok_or((-32602, "name".to_owned()))?;
    let mut child = command.spawn().map_err(|e| (-32000, e.to_string()))?;
    let last = Arc::new(Mutex::new(String::new()));
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(p) = port_progress(&line) {
                    progress(p);
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let last = Arc::clone(&last);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let line: String = line.chars().filter(|c| !c.is_control()).take(200).collect();
                if !line.trim().is_empty() {
                    *last.lock().unwrap_or_else(|e| e.into_inner()) = line;
                }
            }
        });
    }
    let deadline = Instant::now() + PORT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline && !stop.load(Ordering::Relaxed) => {
                thread::sleep(Duration::from_millis(100));
            }
            _ => {
                // TERM first: the script takes back a half install, for up
                // to `PORT_GRACE`; then the group is killed.
                let group = -(child.id() as i32);
                // SAFETY: signalling the process group the script leads.
                unsafe { libc::kill(group, libc::SIGTERM) };
                let grace = Instant::now() + PORT_GRACE;
                while matches!(child.try_wait(), Ok(None)) && Instant::now() < grace {
                    thread::sleep(Duration::from_millis(100));
                }
                // SAFETY: as above.
                unsafe { libc::kill(group, libc::SIGKILL) };
                let _ = child.wait();
                return Err((-32000, "stopped".into()));
            }
        }
    };
    // The script's own answers: 3 already installed, 4 not in the catalog.
    match status.code() {
        Some(0) => return Ok(()),
        Some(3) => return Err((-32005, "port already installed".into())),
        Some(4) => return Err((-32004, "not found".into())),
        _ => {}
    }
    // Its last words, once the reader caught up.
    thread::sleep(Duration::from_millis(50));
    let last = last.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Err((
        -32000,
        if last.is_empty() {
            "failed".into()
        } else {
            last
        },
    ))
}
/// What PortMaster installed for a catalog `name`, from the `port.json`
/// of a folder in `ports` that names it: its launch script and what its
/// files take. `None` when no port of that name is installed.
pub fn port_files(ports: &Path, name: &str) -> Option<(Option<PathBuf>, u64)> {
    for entry in fs::read_dir(ports).ok()?.flatten() {
        let json = entry.path().join("port.json");
        let Ok(text) = archr_io::read_utf8(&json, 256 << 10, archr_io::Symlinks::Reject) else {
            continue;
        };
        let Ok(port) = serde_json::from_str::<Json>(&text) else {
            continue;
        };
        if port["name"].as_str() != Some(name) {
            continue;
        }
        // Only names of the ports folder itself: never a path elsewhere.
        let items: Vec<PathBuf> = list(&port["items"], 64)
            .filter_map(Json::as_str)
            .map(|item| item.trim_end_matches('/'))
            .filter(|item| !item.is_empty() && !item.contains('/') && !item.starts_with('.'))
            .map(|item| ports.join(item))
            .collect();
        let script = items
            .iter()
            .find(|p| {
                p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sh"))
                    && fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
            })
            .cloned();
        let size = items.iter().map(|p| tree_size(p)).sum();
        return Some((script, size));
    }
    None
}
/// Bytes of a file, or of a folder's files, never through a link.
fn tree_size(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| tree_size(&e.path()))
            .sum(),
        Ok(m) if m.is_file() => m.len(),
        _ => 0,
    }
}
/// `PROGRESS <0-100>` lines of the script.
pub fn port_progress(line: &str) -> Option<u8> {
    let value: u32 = line.trim().strip_prefix("PROGRESS ")?.trim().parse().ok()?;
    Some(value.min(100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

    #[test]
    fn pictures_and_files_stay_inside_the_home() {
        let dir = tempfile::tempdir().unwrap();
        let (home, store) = (dir.path().join("home"), dir.path().join("store"));
        fs::create_dir_all(home.join("cache")).unwrap();
        fs::write(home.join("cache/A.png"), PNG).unwrap();
        fs::write(home.join("cache/A.txt"), b"hello world!").unwrap();
        fs::write(dir.path().join("Secret.png"), PNG).unwrap();
        symlink(dir.path().join("Secret.png"), home.join("Link.png")).unwrap();
        symlink(dir.path(), home.join("out")).unwrap();
        fs::write(home.join("cache/B.png"), PNG).unwrap();
        fs::hard_link(home.join("cache/B.png"), home.join("Twin.png")).unwrap();
        let mut pictures = Pictures::default();
        let copy = image(&home, "cache/A.png", &store, &mut pictures).unwrap();
        assert!(
            copy.starts_with(&store),
            "the screens read the launcher's copy"
        );
        assert_eq!(fs::read(&copy).unwrap(), PNG);
        for bad in [
            "cache/A.txt",
            "../Secret.png",
            "cache/../../Secret.png",
            "/etc/passwd",
            "Link.png",
            "out/Secret.png",
            "./cache/A.png",
            "",
            "cache/Missing.png",
            "Twin.png",
        ] {
            assert!(image(&home, bad, &store, &mut pictures).is_none(), "{bad}");
        }
        assert!(open_inside(&home, "cache/A.txt", 4).is_none(), "too big");
        // The same file again is not read again; past the session's cap,
        // nothing more is copied.
        fs::remove_file(&copy).unwrap();
        assert_eq!(
            image(&home, "cache/A.png", &store, &mut pictures),
            Some(copy.clone())
        );
        assert!(!copy.exists(), "known by its inode, not read");
        let mut full = Pictures {
            bytes: PICTURE_BYTES,
            ..Pictures::default()
        };
        assert!(image(&home, "cache/B.png", &store, &mut full).is_none());
        assert!(picture(b"\xff\xd8\xff\xe0") && picture(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!picture(b"GIF89a"));
    }

    /// The copy reads the handle opened while checking: a name swapped for
    /// a link afterwards copies nothing else.
    #[test]
    fn a_swap_after_the_check_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join("downloads")).unwrap();
        fs::write(home.join("downloads/a.gb"), b"game").unwrap();
        fs::write(dir.path().join("shadow"), b"root:secret").unwrap();
        let systems = [system(dir.path())];
        let roms = dir.path().join("roms");
        let params = json!({"system": "gb", "file": "downloads/a.gb", "name": "A"});
        let checked = check(&home, &params, &["gb".to_owned()], (&systems, &roms), 0).unwrap();
        fs::remove_file(home.join("downloads/a.gb")).unwrap();
        symlink(dir.path().join("shadow"), home.join("downloads/a.gb")).unwrap();
        let game = checked.run().unwrap();
        assert_eq!(fs::read(&game.rom).unwrap(), b"game");
        // Checked now, the link is refused.
        assert_eq!(
            check(&home, &params, &["gb".to_owned()], (&systems, &roms), 0).err(),
            Some((-32602, "file"))
        );
    }

    #[test]
    fn ports_are_known_by_their_port_json() {
        let dir = tempfile::tempdir().unwrap();
        let ports = dir.path();
        fs::create_dir_all(ports.join("tobu")).unwrap();
        fs::write(ports.join("tobu/data.bin"), vec![0u8; 100]).unwrap();
        fs::write(ports.join("Tobu Tobu Girl.sh"), b"#!/bin/sh\n").unwrap();
        fs::write(ports.join("Other.sh"), b"#!/bin/sh\n").unwrap();
        let json = json!({"name": "1tobu.zip", "items": ["Tobu Tobu Girl.sh", "tobu/", "../etc"]});
        fs::write(ports.join("tobu/port.json"), json.to_string()).unwrap();
        let (script, size) = port_files(ports, "1tobu.zip").unwrap();
        assert_eq!(script, Some(ports.join("Tobu Tobu Girl.sh")));
        assert!(size >= 100 + 10, "its folder and its script: {size}");
        assert!(port_files(ports, "2048.zip").is_none());
    }

    #[test]
    fn a_game_changed_since_its_install_is_not_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("A.gb");
        fs::write(&rom, b"rom").unwrap();
        let mut game = Game {
            rom: rom.clone(),
            size: 3,
            mtime: mtime(&fs::metadata(&rom).unwrap()),
            ..Game::default()
        };
        assert!(unchanged(&game));
        game.mtime += 1;
        assert!(!unchanged(&game));
        // Never over a file that is there, even one written meanwhile.
        let path = publish(dir.path(), "A", ".gb", |f| io::Write::write_all(f, b"new")).unwrap();
        assert_eq!(path, dir.path().join("A (2).gb"));
        assert_eq!(fs::read(&rom).unwrap(), b"rom");
    }

    #[test]
    fn names_are_safe_and_never_overwrite() {
        assert_eq!(safe_name("Tobu Tobu Girl"), "Tobu Tobu Girl");
        assert_eq!(safe_name("../../etc/passwd"), "etc passwd");
        assert_eq!(safe_name("a\nb\u{7}c: d?"), "abc d");
        assert_eq!(safe_name("..."), "");
        assert_eq!(safe_name(&"x".repeat(200)).len(), 96);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(free_name(dir.path(), "A", ".gb"), dir.path().join("A.gb"));
        fs::write(dir.path().join("A.gb"), b"").unwrap();
        fs::write(dir.path().join("A (2).gb"), b"").unwrap();
        assert_eq!(
            free_name(dir.path(), "A", ".gb"),
            dir.path().join("A (3).gb")
        );
    }

    #[test]
    fn ports_are_catalog_names_and_a_closed_command() {
        for good in ["1tobu.zip", "2048.zip", "a.b_c-d.zip"] {
            assert!(port_ok(good), "{good}");
        }
        for bad in [
            "Tobu.zip",
            ".zip",
            "-rf.zip",
            "a/b.zip",
            "2048.tar",
            "a zip.zip",
            "../x.zip",
            &format!("{}.zip", "a".repeat(61)),
        ] {
            assert!(!port_ok(bad), "{bad}");
        }
        let command = port_command(Path::new("/test/bin"), true, "2048.zip").unwrap();
        assert_eq!(command.get_program(), "/test/bin/archr-port-install");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, ["install", "2048.zip"]);
        let command = port_command(Path::new("/test/bin"), false, "2048.zip").unwrap();
        assert_eq!(command.get_args().next().unwrap(), "uninstall");
        assert!(port_command(Path::new("/bin"), true, "x;reboot.zip").is_none());
        assert_eq!(port_progress("PROGRESS 42\n"), Some(42));
        assert_eq!(port_progress("PROGRESS 420"), Some(100));
        assert_eq!(port_progress("downloading"), None);
    }

    fn system(root: &Path) -> SystemDef {
        SystemDef {
            name: "gb".into(),
            fullname: "Game Boy".into(),
            path: root.join("roms/gb"),
            extensions: vec![".gb".into(), ".zip".into()],
            command: RUNEMU.into(),
            ..SystemDef::default()
        }
    }
    const RUNEMU: &str = "/usr/bin/runemu.sh %ROM% -P%SYSTEM% --core=%CORE% --emulator=%EMULATOR%";

    /// The device's own catalog: every emulated system takes games; the
    /// ones whose entries the device runs take nothing.
    #[test]
    fn the_shipped_catalog_installs_into_emulated_systems_only() {
        let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/es_systems.cfg");
        let systems = archr_index::load_systems(&catalog).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let roms = Path::new("/storage/roms");
        let mut refused = Vec::new();
        for def in &systems {
            let ext = def.extensions.first().cloned().unwrap_or_default();
            let file = format!("game{ext}");
            fs::write(home.join(&file), b"rom").unwrap();
            let params = json!({"system": def.name, "file": file, "name": "Game"});
            let allowed = [def.name.clone()];
            if check(home, &params, &allowed, (&systems, roms), 0).is_err() {
                refused.push(def.name.as_str());
            }
        }
        refused.sort_unstable();
        for runner in [
            "idtech",
            "j2me",
            "moonlight",
            "music",
            "ports",
            "scummvm",
            "tools",
            "windows",
        ] {
            assert!(refused.contains(&runner), "{runner}: {refused:?}");
        }
        assert!(refused.len() <= 10, "{refused:?}");
        for emulated in ["gb", "gba", "nes", "megadrive", "pico-8", "psx"] {
            assert!(!refused.contains(&emulated), "{emulated}");
        }
        // A PC game is an .exe inside DOSBox: it goes in.
        fs::write(home.join("Game.exe"), b"MZ").unwrap();
        let pc = json!({"system": "pc", "file": "Game.exe"});
        assert!(check(home, &pc, &["pc".to_owned()], (&systems, roms), 0).is_ok());
    }

    #[test]
    fn install_checks_the_system_the_file_and_the_total() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join("downloads")).unwrap();
        fs::write(home.join("downloads/tobu.gb"), vec![0u8; 1024]).unwrap();
        fs::write(home.join("downloads/tobu.nes"), b"rom").unwrap();
        fs::write(home.join("Cover.png"), PNG).unwrap();
        let systems = [system(dir.path())];
        let roms = dir.path().join("roms");
        let allowed = vec!["gb".to_owned()];
        let params = json!({"system": "gb", "file": "downloads/tobu.gb",
            "name": "Tobu Tobu Girl", "cover": "Cover.png",
            "meta": {"developer": "tangram games", "description": "Up.", "year": 2017}});
        let game = install(&home, &params, &allowed, (&systems, &roms), 0).unwrap();
        assert_eq!(game.game_key, "gb/Tobu Tobu Girl.gb");
        assert_eq!(game.size, 1024);
        assert!(game.rom.is_file());
        let cover = game.cover.clone().unwrap();
        assert_eq!(
            cover,
            dir.path().join("roms/gb/media/images/Tobu Tobu Girl.png")
        );
        let gamelist = fs::read_to_string(dir.path().join("roms/gb/gamelist.xml")).unwrap();
        for expected in [
            "<name>Tobu Tobu Girl</name>",
            "<developer>tangram games</developer>",
            "<releasedate>20170101T000000</releasedate>",
            "<image>./media/images/Tobu Tobu Girl.png</image>",
        ] {
            assert!(gamelist.contains(expected), "{expected} in {gamelist}");
        }
        // The same game again: a name of its own, never over the first.
        let again = install(&home, &params, &allowed, (&systems, &roms), 1024).unwrap();
        assert_eq!(again.game_key, "gb/Tobu Tobu Girl (2).gb");
        let refused = |params: Json, allowed: &[String], used: u64| {
            install(&home, &params, allowed, (&systems, &roms), used)
                .unwrap_err()
                .1
        };
        let with = |key: &str, value: Json| {
            let mut p = params.clone();
            p[key] = value;
            p
        };
        assert_eq!(
            refused(with("system", json!("nes")), &allowed, 0),
            "permission denied"
        );
        assert_eq!(
            refused(params.clone(), &["nes".to_owned()], 0),
            "permission denied"
        );
        assert_eq!(
            refused(with("system", json!("ports")), &[PORTS.to_owned()], 0),
            "permission denied"
        );
        assert_eq!(
            refused(with("file", json!("downloads/tobu.nes")), &allowed, 0),
            "extension"
        );
        assert_eq!(
            refused(
                with("file", json!("../home/downloads/tobu.gb")),
                &allowed,
                0
            ),
            "file"
        );
        let bare = install(
            &home,
            &with("cover", json!("downloads/tobu.gb")),
            &allowed,
            (&systems, &roms),
            0,
        );
        assert_eq!(bare.unwrap().cover, None, "a bad cover is left out");
        // Never a script, nor into a folder of programs or outside the ROMs.
        fs::write(home.join("downloads/run.sh"), b"reboot").unwrap();
        let tools = SystemDef {
            name: "tools".into(),
            path: dir.path().join("roms/tools"),
            extensions: vec![".sh".into(), ".gb".into()],
            command: "%RUNCOMMAND%".into(),
            ..SystemDef::default()
        };
        let mut away = system(dir.path());
        away.name = "away".into();
        away.path = dir.path().join("elsewhere");
        let more = [systems[0].clone(), tools, away];
        let all = vec!["gb".to_owned(), "tools".to_owned(), "away".to_owned()];
        for (system, file) in [
            ("tools", "downloads/tobu.gb"),
            ("away", "downloads/tobu.gb"),
            ("gb", "downloads/run.sh"),
        ] {
            let p = json!({"system": system, "file": file});
            assert!(
                install(&home, &p, &all, (&more, &roms), 0).is_err(),
                "{system} {file}"
            );
        }
        assert_eq!(
            refused(params.clone(), &allowed, MODULE_BYTES),
            "quota exceeded"
        );
        let gone = Game {
            cover: Some(cover.clone()),
            ..game
        };
        remove(&gone, &systems);
        assert!(!gone.rom.exists() && !cover.exists());
        let gamelist = fs::read_to_string(dir.path().join("roms/gb/gamelist.xml")).unwrap();
        assert!(!gamelist.contains("Girl.gb</path>"), "{gamelist}");
        assert!(
            gamelist.contains("Tobu Tobu Girl (2).gb"),
            "the other stays"
        );
    }

    #[test]
    fn the_ledger_keeps_each_modules_games() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LEDGER);
        let rom = dir.path().join("A.gb");
        fs::write(&rom, b"rom").unwrap();
        let mut ledger = Ledger::default();
        let owned = ledger.modules.entry("demo".into()).or_default();
        owned.games.push(Game {
            game_key: "gb/A.gb".into(),
            rom: rom.clone(),
            size: 3,
            ..Game::default()
        });
        owned.games.push(Game {
            rom: dir.path().join("Gone.gb"),
            size: 5,
            ..Game::default()
        });
        owned.ports.push(Port {
            name: "2048.zip".into(),
            installed: 1,
            ..Port::default()
        });
        ledger.save(&path).unwrap();
        let back = Ledger::load(&path);
        assert_eq!(back, ledger);
        let (present, bytes) = back.modules["demo"].present();
        assert_eq!((present.len(), bytes), (1, 3), "only what is still there");
    }
}
