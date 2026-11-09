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
/// A PortMaster install downloads up to a few hundred MB.
const PORT_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// PNG, JPEG or WebP, by their first bytes.
pub fn picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || (bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

/// A file inside `home` by a relative path: no `..`, no absolute path, no
/// link out of it, at most `limit` bytes. Its real path and size.
pub fn inside(home: &Path, relative: &str, limit: u64) -> Option<(PathBuf, u64)> {
    let plain = !relative.is_empty()
        && relative.len() <= 256
        && !relative.starts_with('/')
        && !relative.chars().any(char::is_control)
        && relative
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && part != ".");
    if !plain {
        return None;
    }
    let root = home.canonicalize().ok()?;
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > limit {
        return None;
    }
    // A linked folder on the way may still lead out.
    let real = path.canonicalize().ok()?;
    real.starts_with(&root).then_some((real, meta.len()))
}
/// A picture inside `home` the launcher will decode.
pub fn image(home: &Path, relative: &str) -> Option<PathBuf> {
    let (path, _) = inside(home, relative, IMAGE_BYTES)?;
    let mut head = [0u8; 12];
    fs::File::open(&path).ok()?.read_exact(&mut head).ok()?;
    picture(&head).then_some(path)
}

/// What turns a module's message into what the screens draw: its texts,
/// its folder for pictures and its icons.
pub struct Reader<'a> {
    pub installed: &'a Installed,
    pub home: &'a Path,
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
    fn image(&self, value: &Json) -> Option<PathBuf> {
        image(self.home, value.as_str()?)
    }
    fn item(&self, value: &Json) -> Option<Item> {
        Some(Item {
            id: name(&value["id"])?,
            title: self.text(&value["title"]),
            subtitle: self.text(&value["subtitle"]),
            badge: self.text(&value["badge"]),
            tone: Tone::parse(value["tone"].as_str().unwrap_or("")),
            image: self.image(&value["image"]),
        })
    }
    fn items(&self, value: &Json) -> Vec<Item> {
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
    /// Seconds since 1970.
    pub installed: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Port {
    pub name: String,
    /// Its launch script, `ports/<file>`, when the install brought one.
    pub game_key: String,
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
    systems: &[SystemDef],
    used: u64,
) -> Result<Game, Refusal> {
    check(home, params, allowed, systems, used)?.run()
}
/// Everything `rom.install` checks before it copies anything.
pub fn check(
    home: &Path,
    params: &Json,
    allowed: &[String],
    systems: &[SystemDef],
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
    let file = params["file"].as_str().ok_or((-32602, "file"))?;
    let (source, size) = inside(home, file, ROM_BYTES).ok_or((-32602, "file"))?;
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .ok_or((-32602, "extension"))?;
    if !def.extensions.iter().any(|e| e.to_lowercase() == ext) {
        return Err((-32602, "extension"));
    }
    if used + size > MODULE_BYTES {
        return Err((-32003, "too much installed"));
    }
    let cover = match params.get("cover").and_then(Json::as_str) {
        Some(cover) => Some(image(home, cover).ok_or((-32602, "cover"))?),
        None => None,
    };
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("");
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
/// A checked `rom.install`, for a worker to carry out.
pub struct Copy {
    def: SystemDef,
    source: PathBuf,
    size: u64,
    ext: String,
    name: String,
    cover: Option<PathBuf>,
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
        place(&def, (&source, size, &ext), &name, cover, &meta)
    }
}
fn place(
    def: &SystemDef,
    (source, size, ext): (&Path, u64, &str),
    name: &str,
    cover: Option<PathBuf>,
    meta: &Json,
) -> Result<Game, Refusal> {
    let root = &def.path;
    fs::create_dir_all(root).map_err(|_| (-32000, "cannot write"))?;
    let rom = free_name(root, name, ext);
    copy(source, &rom).map_err(|_| (-32000, "cannot write"))?;
    let cover = cover.and_then(|picture| {
        let images = root.join("media/images");
        let ext = picture
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
        let stem = rom.file_stem()?.to_str()?;
        let target = free_name(&images, stem, &format!(".{ext}"));
        fs::create_dir_all(&images).ok()?;
        copy(&picture, &target).ok().map(|()| target)
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
    let mut fields = vec![("name", name.to_owned())];
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
        game_key: crate::store::game_key(def, &rom).unwrap_or_default(),
        system: def.name.clone(),
        rom,
        cover,
        size,
        installed: super::now_secs(),
    })
}
/// The copy lands whole or not at all.
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    let mut source = fs::File::open(from)?;
    archr_io::atomic_publish(to, true, |file| io::copy(&mut source, file).map(|_| ()))
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
) -> Result<(), String> {
    let mut command = port_command(bin, install, name).ok_or("name")?;
    let mut child = command.spawn().map_err(|e| e.to_string())?;
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
                // SAFETY: signalling the process group the script leads.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                let _ = child.wait();
                return Err("stopped".into());
            }
        }
    };
    if status.success() {
        return Ok(());
    }
    // Its last words, once the reader caught up.
    thread::sleep(Duration::from_millis(50));
    let last = last.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Err(if last.is_empty() {
        "failed".into()
    } else {
        last
    })
}
/// The launch scripts in the ports folder now (its system's extensions).
pub fn scripts(ports: &SystemDef) -> Vec<PathBuf> {
    fs::read_dir(&ports.path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|path| {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let ext = format!(".{}", ext.to_lowercase());
            path.is_file() && ports.extensions.contains(&ext)
        })
        .collect()
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
        let home = dir.path().join("home");
        fs::create_dir_all(home.join("cache")).unwrap();
        fs::write(home.join("cache/A.png"), PNG).unwrap();
        fs::write(home.join("cache/A.txt"), b"hello world!").unwrap();
        fs::write(dir.path().join("Secret.png"), PNG).unwrap();
        symlink(dir.path().join("Secret.png"), home.join("Link.png")).unwrap();
        symlink(dir.path(), home.join("out")).unwrap();
        assert!(image(&home, "cache/A.png").is_some());
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
        ] {
            assert!(image(&home, bad).is_none(), "{bad}");
        }
        assert!(inside(&home, "cache/A.txt", 4).is_none(), "too big");
        assert!(picture(b"\xff\xd8\xff\xe0") && picture(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!picture(b"GIF89a"));
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
            ..SystemDef::default()
        }
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
        let allowed = vec!["gb".to_owned()];
        let params = json!({"system": "gb", "file": "downloads/tobu.gb",
            "name": "Tobu Tobu Girl", "cover": "Cover.png",
            "meta": {"developer": "tangram games", "description": "Up.", "year": 2017}});
        let game = install(&home, &params, &allowed, &systems, 0).unwrap();
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
        let again = install(&home, &params, &allowed, &systems, 1024).unwrap();
        assert_eq!(again.game_key, "gb/Tobu Tobu Girl (2).gb");
        let refused = |params: Json, allowed: &[String], used: u64| {
            install(&home, &params, allowed, &systems, used)
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
        assert_eq!(
            refused(with("cover", json!("downloads/tobu.gb")), &allowed, 0),
            "cover"
        );
        assert_eq!(
            refused(params.clone(), &allowed, MODULE_BYTES),
            "too much installed"
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
