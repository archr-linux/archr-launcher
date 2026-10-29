//! module.toml and the package around it (docs/modulos.md, "Manifesto").
//! Unknown keys and wrong types are warnings and the field is dropped; only
//! what decides how and whether the module runs is fatal.
use archr_io::{read_bytes, read_utf8, Symlinks};
use archr_ui::modules::{Context, Entry, Page, Quick, Row, RowKind, Target};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
use toml::{Table, Value};

pub const API_VERSION: i64 = 1;
pub const FILE: &str = "module.toml";
/// Path aliases a module may read; never a free path.
pub const ALIASES: &[(&str, &str)] = &[
    ("roms", "/storage/roms"),
    ("bios", "/storage/roms/bios"),
    ("screenshots", "/storage/roms/screenshots"),
];
/// Settings `config_read` may list. Passwords, tokens and keys never are.
pub const READABLE: &[&str] = &[
    "system.language",
    "system.timezone",
    "system.cpugovernor",
    "system.hostname",
    "global.netplay.nickname",
    "global.retroachievements",
    "audio.volume",
    "display.brightness",
];
/// The events this launcher delivers.
pub const EVENTS: &[&str] = &[
    "launcher.started",
    "focus.system",
    "focus.game",
    "game.started",
    "game.ended",
];
/// Permissions of later API versions: asked for, they block the module.
const LATER: &[&str] = &["write", "config_write", "hooks", "system", "launch"];
/// Extension points of later API versions: ignored with a warning.
const LATER_KEYS: &[&str] = &["screens", "collections", "replace", "provider"];
const PAGES: usize = 8;
const ROWS: usize = 48;
const TEXT: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runtime {
    Sh,
    Bash,
    Native,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Daemon,
    Oneshot,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Permissions {
    pub net: bool,
    /// Aliases from `ALIASES`.
    pub read: Vec<String>,
    /// Keys from `READABLE`.
    pub config_read: Vec<String>,
}
impl Permissions {
    /// What a grant records, to tell which permissions an update adds.
    pub fn tokens(&self) -> BTreeSet<String> {
        let mut tokens: BTreeSet<String> = self.read.iter().map(|a| format!("read:{a}")).collect();
        tokens.extend(self.config_read.iter().map(|k| format!("config_read:{k}")));
        if self.net {
            tokens.insert("net".into());
        }
        tokens
    }
}

/// A parsed module.toml. Texts may be `@keys` of the module's locales.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub api_version: i64,
    pub min_launcher: String,
    /// Text, `@key` or a table by language.
    pub name: Option<Value>,
    pub description: Option<Value>,
    pub author: String,
    pub license: String,
    pub homepage: String,
    pub icon: Option<String>,
    pub runtime: Runtime,
    pub entry: String,
    pub mode: Mode,
    pub memory_mb: u32,
    pub permissions: Permissions,
    pub hooks: Vec<String>,
    pub tiles: Vec<Entry>,
    pub shortcuts: Vec<Entry>,
    pub quick: Vec<Quick>,
    pub pages: Vec<Page>,
}

/// `id` rules: `[a-z0-9-]{3,40}`.
pub fn id_ok(id: &str) -> bool {
    (3..=40).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
/// `x.y.z`, digits only.
pub fn semver_ok(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| (1..=6).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}
/// A Lucide name: lowercase words joined by dashes.
fn icon_ok(name: &str) -> bool {
    (1..=48).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Reads typed fields off a table, collecting warnings for the rest.
struct Reader<'a> {
    table: &'a Table,
    at: String,
    warnings: &'a mut Vec<String>,
}
impl Reader<'_> {
    fn check(&mut self, known: &[&str]) {
        for key in self.table.keys() {
            if LATER_KEYS.contains(&key.as_str()) {
                self.warnings
                    .push(format!("{}{key}: not supported by API 1, ignored", self.at));
            } else if !known.contains(&key.as_str()) {
                self.warnings
                    .push(format!("{}{key}: unknown key, ignored", self.at));
            }
        }
    }
    fn typed<T>(&mut self, key: &str, kind: &str, get: impl Fn(&Value) -> Option<T>) -> Option<T> {
        let value = self.table.get(key)?;
        let typed = get(value);
        if typed.is_none() {
            self.warnings
                .push(format!("{}{key}: expected {kind}, ignored", self.at));
        }
        typed
    }
    fn text(&mut self, key: &str) -> Option<String> {
        self.typed(key, "text", |v| {
            v.as_str()
                .filter(|s| s.chars().count() <= TEXT && !s.chars().any(char::is_control))
                .map(str::to_owned)
        })
    }
    fn texts(&mut self, key: &str) -> Vec<String> {
        self.typed(key, "a list of texts", |v| {
            v.as_array()?
                .iter()
                .map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
    }
    fn int(&mut self, key: &str) -> Option<i64> {
        self.typed(key, "an integer", Value::as_integer)
    }
    fn flag(&mut self, key: &str) -> Option<bool> {
        self.typed(key, "true or false", Value::as_bool)
    }
    fn tables(&mut self, key: &str) -> Vec<Table> {
        self.typed(key, "a list of tables", |v| {
            v.as_array()?
                .iter()
                .map(|t| t.as_table().cloned())
                .collect()
        })
        .unwrap_or_default()
    }
    fn table(&mut self, key: &str) -> Table {
        self.typed(key, "a table", |v| v.as_table().cloned())
            .unwrap_or_default()
    }
}
fn reader<'a>(table: &'a Table, at: &str, warnings: &'a mut Vec<String>) -> Reader<'a> {
    Reader {
        table,
        at: at.to_owned(),
        warnings,
    }
}

/// The manifest and its warnings, or the fatal problems.
pub fn parse(source: &str) -> Result<(Manifest, Vec<String>), Vec<String>> {
    let table: Table = toml::from_str(source).map_err(|e| vec![format!("module.toml: {e}")])?;
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut top = reader(&table, "", &mut warnings);
    top.check(&[
        "id",
        "version",
        "api_version",
        "min_launcher",
        "name",
        "description",
        "author",
        "license",
        "homepage",
        "icon",
        "run",
        "limits",
        "permissions",
        "hooks",
        "start",
        "quick",
        "settings",
    ]);
    let id = top.text("id").filter(|id| id_ok(id));
    let version = top.text("version").filter(|v| semver_ok(v));
    let api_version = top.int("api_version");
    let min_launcher = top
        .text("min_launcher")
        .filter(|v| semver_ok(v))
        .unwrap_or_default();
    let author = top.text("author").unwrap_or_default();
    let license = top.text("license").unwrap_or_default();
    let homepage = top.text("homepage").unwrap_or_default();
    let icon = top.text("icon").filter(|i| icon_ok(i));
    let run = top.table("run");
    let limits = top.table("limits");
    let permissions = top.table("permissions");
    let hooks = top.tables("hooks");
    let start = top.table("start");
    let quick = top.tables("quick");
    let settings = top.table("settings");
    for (ok, problem) in [
        (id.is_some(), "id: required, [a-z0-9-]{3,40}"),
        (version.is_some(), "version: required, x.y.z"),
        (api_version.is_some(), "api_version: required integer"),
    ] {
        if !ok {
            errors.push(problem.to_owned());
        }
    }
    let mut run = reader(&run, "run.", &mut warnings);
    run.check(&["runtime", "entry", "mode"]);
    let runtime = match run.text("runtime").as_deref() {
        Some("sh") => Some(Runtime::Sh),
        Some("bash") => Some(Runtime::Bash),
        Some("native") => Some(Runtime::Native),
        _ => None,
    };
    let mode = match run.text("mode").as_deref() {
        Some("daemon") => Some(Mode::Daemon),
        Some("oneshot") => Some(Mode::Oneshot),
        _ => None,
    };
    let entry = run.text("entry").filter(|e| {
        !e.is_empty()
            && !e.starts_with(['/', '.'])
            && e.split('/').all(|part| !part.is_empty() && part != "..")
    });
    for (ok, problem) in [
        (runtime.is_some(), "run.runtime: sh, bash or native"),
        (mode.is_some(), "run.mode: daemon or oneshot"),
        (entry.is_some(), "run.entry: a file inside the module"),
    ] {
        if !ok {
            errors.push(problem.to_owned());
        }
    }
    let mut limits = reader(&limits, "limits.", &mut warnings);
    limits.check(&["memory_mb"]);
    let memory_mb = limits
        .int("memory_mb")
        .map_or(32, |m| m.clamp(8, 64) as u32);
    let permissions = read_permissions(&permissions, &mut warnings, &mut errors);
    let hooks = hooks
        .iter()
        .filter_map(|hook| {
            let mut hook = reader(hook, "hooks.", &mut warnings);
            hook.check(&["event"]);
            let event = hook.text("event")?;
            if !EVENTS.contains(&event.as_str()) {
                hook.warnings
                    .push(format!("hooks.event: {event} is not delivered by API 1"));
                return None;
            }
            Some(event)
        })
        .collect();
    let mut start = reader(&start, "start.", &mut warnings);
    start.check(&["tiles", "shortcuts"]);
    let (tiles, shortcuts) = (start.tables("tiles"), start.tables("shortcuts"));
    let mut settings = reader(&settings, "settings.", &mut warnings);
    settings.check(&["pages"]);
    let pages: Vec<Page> = settings
        .tables("pages")
        .iter()
        .take(PAGES)
        .filter_map(|page| read_page(page, &mut warnings))
        .collect();
    let rows: usize = pages.iter().map(|p| p.rows.len()).sum();
    if rows > ROWS {
        errors.push(format!("settings: {rows} rows, at most {ROWS}"));
    }
    let entries = |list: &[Table], at: &str, warnings: &mut Vec<String>| -> Vec<Entry> {
        list.iter()
            .filter_map(|t| read_entry(t, at, &pages, warnings))
            .collect()
    };
    let tiles = entries(&tiles, "start.tiles.", &mut warnings);
    let shortcuts = entries(&shortcuts, "start.shortcuts.", &mut warnings);
    let quick = quick
        .iter()
        .filter_map(|q| read_quick(q, &mut warnings))
        .collect();
    let (name, description) = (
        table.get("name").cloned(),
        table.get("description").cloned(),
    );
    match (id, version, api_version, runtime, entry, mode) {
        (Some(id), Some(version), Some(api_version), Some(runtime), Some(entry), Some(mode))
            if errors.is_empty() =>
        {
            Ok((
                Manifest {
                    id,
                    version,
                    api_version,
                    min_launcher,
                    name,
                    description,
                    author,
                    license,
                    homepage,
                    icon,
                    runtime,
                    entry,
                    mode,
                    memory_mb,
                    permissions,
                    hooks,
                    tiles,
                    shortcuts,
                    quick,
                    pages,
                },
                warnings,
            ))
        }
        _ => Err(errors),
    }
}

fn read_permissions(
    table: &Table,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> Permissions {
    for key in table.keys() {
        if LATER.contains(&key.as_str()) {
            errors.push(format!(
                "permissions.{key}: not available in this launcher yet"
            ));
        } else if !["net", "read", "config_read"].contains(&key.as_str()) {
            errors.push(format!("permissions.{key}: unknown permission"));
        }
    }
    let mut reader = reader(table, "permissions.", warnings);
    let net = reader.flag("net").unwrap_or(false);
    let mut list = |key: &str, allowed: &[&str]| -> Vec<String> {
        let values = reader.texts(key);
        for value in &values {
            if !allowed.contains(&value.as_str()) {
                errors.push(format!("permissions.{key}: {value} is not allowed"));
            }
        }
        values
    };
    let aliases: Vec<&str> = ALIASES.iter().map(|(a, _)| *a).collect();
    let read = list("read", &aliases);
    let config_read = list("config_read", READABLE);
    Permissions {
        net,
        read,
        config_read,
    }
}

fn read_entry(
    table: &Table,
    at: &str,
    pages: &[Page],
    warnings: &mut Vec<String>,
) -> Option<Entry> {
    let mut entry = reader(table, at, warnings);
    entry.check(&["id", "label", "icon", "open", "action"]);
    let id = entry.text("id").filter(|id| key_ok(id))?;
    let label = entry.text("label").unwrap_or_default();
    let icon = entry.text("icon").filter(|i| icon_ok(i));
    let target = match (entry.text("open"), entry.text("action")) {
        (Some(open), _) => match pages.iter().position(|p| p.id == open) {
            Some(page) => Target::Page(page as u8),
            None => {
                entry
                    .warnings
                    .push(format!("{at}open: no page {open}, entry ignored"));
                return None;
            }
        },
        (None, Some(action)) => Target::Action(action),
        (None, None) => {
            entry
                .warnings
                .push(format!("{at}{id}: needs open or action, ignored"));
            return None;
        }
    };
    // The icon name rides in `icon` until the registry swaps in the SVG.
    Some(Entry {
        id,
        label,
        icon: icon.map(|i| i.as_str().into()),
        target,
    })
}

fn read_quick(table: &Table, warnings: &mut Vec<String>) -> Option<Quick> {
    let mut quick = reader(table, "quick.", warnings);
    quick.check(&["id", "label", "context", "systems", "action"]);
    let context = match quick.text("context").as_deref() {
        Some("game") => Context::Game,
        Some("system") => Context::System,
        _ => {
            quick
                .warnings
                .push("quick.context: game or system, item ignored".into());
            return None;
        }
    };
    Some(Quick {
        id: quick.text("id")?,
        label: quick.text("label").unwrap_or_default(),
        context,
        systems: quick.texts("systems"),
        action: quick.text("action")?,
    })
}

fn read_page(table: &Table, warnings: &mut Vec<String>) -> Option<Page> {
    let mut page = reader(table, "settings.pages.", warnings);
    page.check(&["id", "title", "rows"]);
    let id = page.text("id")?;
    let title = page.text("title").unwrap_or_default();
    let rows = page.tables("rows");
    let rows = rows
        .iter()
        .filter_map(|row| read_row(row, warnings))
        .collect();
    Some(Page { id, title, rows })
}

fn read_row(table: &Table, warnings: &mut Vec<String>) -> Option<Row> {
    let mut row = reader(table, "settings.pages.rows.", warnings);
    row.check(&[
        "type", "label", "subtitle", "section", "key", "default", "choices", "labels", "min",
        "max", "step", "action", "confirm", "id", "value", "secret",
    ]);
    let kind = row.text("type")?;
    let key = || row_key(table);
    let flag = |row: &mut Reader| {
        row.flag("default")
            .map_or("0", |on| if on { "1" } else { "0" })
            .to_owned()
    };
    let (kind, default) = match kind.as_str() {
        "toggle" => (RowKind::Toggle { key: key()? }, flag(&mut row)),
        "check" => (RowKind::Check { key: key()? }, flag(&mut row)),
        "value" => {
            let choices = row.texts("choices");
            let mut labels = row.texts("labels");
            labels.resize(choices.len(), String::new());
            let default = row
                .text("default")
                .filter(|d| choices.contains(d))
                .or_else(|| choices.first().cloned())?;
            let key = key()?;
            (
                RowKind::Value {
                    key,
                    choices,
                    labels,
                },
                default,
            )
        }
        "slider" => {
            let (min, max) = (row.int("min").unwrap_or(0), row.int("max").unwrap_or(100));
            let step = row.int("step").unwrap_or(1).max(1);
            if min > max || (max - min) / step > 200 {
                row.warnings
                    .push("settings.pages.rows: slider range too wide, ignored".into());
                return None;
            }
            let choices: Vec<String> = (min..=max)
                .step_by(step as usize)
                .map(|n| n.to_string())
                .collect();
            let default = row
                .int("default")
                .map(|d| d.to_string())
                .filter(|d| choices.contains(d))
                .unwrap_or_else(|| choices[0].clone());
            let labels = choices.clone();
            (
                RowKind::Value {
                    key: key()?,
                    choices,
                    labels,
                },
                default,
            )
        }
        "action" => (
            RowKind::Action {
                action: row.text("action")?,
                confirm: row.text("confirm"),
            },
            String::new(),
        ),
        "text" => (
            RowKind::Text {
                id: row.text("id")?,
            },
            row.text("value").unwrap_or_default(),
        ),
        "keyboard" => (
            RowKind::Keyboard {
                key: key()?,
                max: row.int("max").map_or(64, |m| m.clamp(1, 64) as u8),
                secret: row.flag("secret").unwrap_or(false),
            },
            row.text("default").unwrap_or_default(),
        ),
        other => {
            row.warnings.push(format!(
                "settings.pages.rows: type {other} unknown, ignored"
            ));
            return None;
        }
    };
    Some(Row {
        label: row.text("label").unwrap_or_default(),
        subtitle: row.text("subtitle"),
        section: row.text("section"),
        kind,
        default,
    })
}
/// Setting keys and entry ids: short ASCII words.
fn key_ok(key: &str) -> bool {
    (1..=40).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}
/// A settings key: what `settings.set` and the JSON file use.
fn row_key(table: &Table) -> Option<String> {
    table
        .get("key")
        .and_then(Value::as_str)
        .filter(|k| key_ok(k))
        .map(str::to_owned)
}

/// The module's `locales/<language>.toml`: flat text keys.
pub fn locale(dir: &Path, language: &str) -> Option<Table> {
    let text = read_utf8(
        &dir.join("locales").join(format!("{language}.toml")),
        256 << 10,
        Symlinks::Reject,
    )
    .ok()?;
    let table: Table = toml::from_str(&text).ok()?;
    Some(
        table
            .into_iter()
            .filter(|(_, v)| v.is_str())
            .collect::<Table>(),
    )
}

/// The SVG is one of Lucide 0.468.0's icons, unmodified (first 16 bytes of
/// each SHA-256, sorted).
pub fn lucide(svg: &[u8]) -> bool {
    static TABLE: &[u8] = include_bytes!("lucide.bin");
    let digest = Sha256::digest(svg);
    TABLE
        .chunks_exact(16)
        .collect::<Vec<_>>()
        .binary_search(&&digest[..16])
        .is_ok()
}

/// Everything `--check-module` and an install verify in a module folder.
pub fn check(dir: &Path) -> Result<(Manifest, Vec<String>), Vec<String>> {
    let source = read_utf8(&dir.join(FILE), 64 << 10, Symlinks::Reject)
        .map_err(|e| vec![format!("{FILE}: {e}")])?;
    let (manifest, warnings) = parse(&source)?;
    let mut errors = Vec::new();
    if dir.file_name().and_then(|n| n.to_str()) != Some(manifest.id.as_str()) {
        errors.push(format!("the folder must be named {}", manifest.id));
    }
    let entry = dir.join(&manifest.entry);
    if !fs::symlink_metadata(&entry).is_ok_and(|m| m.is_file()) {
        errors.push(format!("run.entry: {} is not a file", manifest.entry));
    }
    if locale(dir, "en").is_none() {
        errors.push("locales/en.toml: required".into());
    }
    let icons: BTreeSet<&str> = manifest
        .icon
        .iter()
        .map(String::as_str)
        .chain(
            manifest
                .tiles
                .iter()
                .chain(&manifest.shortcuts)
                .filter_map(|e| e.icon.as_deref()),
        )
        .collect();
    for icon in icons {
        let path = dir.join("icons").join(format!("{icon}.svg"));
        match read_bytes(&path, 16 << 10, Symlinks::Reject) {
            Ok(svg) if lucide(&svg) => {}
            Ok(_) => errors.push(format!("icons/{icon}.svg: not an unmodified Lucide icon")),
            Err(_) => errors.push(format!("icons/{icon}.svg: missing")),
        }
    }
    if errors.is_empty() {
        Ok((manifest, warnings))
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
id = "bateria-detalhada"
version = "1.0.0"
api_version = 1
name = "@name"
icon = "battery-full"
color = "red"
[run]
runtime = "bash"
entry = "main.sh"
mode = "daemon"
[limits]
memory_mb = 500
[permissions]
read = ["roms"]
[[hooks]]
event = "focus.game"
[[hooks]]
event = "launch.before"
[[start.tiles]]
id = "bateria"
label = "@name"
open = "main"
[[start.tiles]]
id = "lost"
open = "nowhere"
[[settings.pages]]
id = "main"
title = "@name"
[[settings.pages.rows]]
type = "slider"
key = "brilho"
min = 0
max = 100
step = 25
default = 50
[[settings.pages.rows]]
type = "toggle"
key = "watts"
default = true
[[settings.pages.rows]]
type = "radio"
"#;

    #[test]
    fn a_good_manifest_parses_with_warnings_for_what_is_dropped() {
        let (manifest, warnings) = parse(GOOD).unwrap();
        assert_eq!(manifest.id, "bateria-detalhada");
        assert_eq!(manifest.memory_mb, 64, "clamped");
        assert_eq!(
            manifest.permissions.tokens(),
            ["read:roms".to_owned()].into()
        );
        assert_eq!(manifest.hooks, ["focus.game"]);
        assert_eq!(manifest.tiles.len(), 1);
        assert_eq!(manifest.tiles[0].target, Target::Page(0));
        let rows = &manifest.pages[0].rows;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].default, "50");
        assert!(matches!(&rows[0].kind, RowKind::Value { choices, .. } if choices.len() == 5));
        assert_eq!(rows[1].default, "1");
        for expected in ["color", "launch.before", "nowhere", "radio"] {
            assert!(
                warnings.iter().any(|w| w.contains(expected)),
                "{expected}: {warnings:?}"
            );
        }
    }

    #[test]
    fn identity_run_and_permissions_are_fatal() {
        let errors = parse("id = \"X\"\nversion = \"1\"\n[permissions]\nhooks = [\"launch\"]\nfoo = 1\n[permissions.x]\n")
            .unwrap_err();
        for expected in [
            "id:",
            "version:",
            "api_version:",
            "run.runtime",
            "run.entry",
            "hooks",
            "foo",
            "x",
        ] {
            assert!(
                errors.iter().any(|e| e.contains(expected)),
                "{expected}: {errors:?}"
            );
        }
        let python = GOOD.replace("runtime = \"bash\"", "runtime = \"python3\"");
        assert!(parse(&python).is_err(), "only shell and native code");
        let escape = GOOD.replace("main.sh", "../../bin/sh");
        assert!(parse(&escape).is_err());
        let secret = GOOD.replace("read = [\"roms\"]", "config_read = [\"wifi.key\"]");
        assert!(parse(&secret).is_err());
    }

    #[test]
    fn only_unmodified_lucide_icons_pass() {
        let icons = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/icons");
        let svg = fs::read(icons.join("cpu.svg")).unwrap();
        assert!(lucide(&svg));
        let mut changed = svg.clone();
        changed.extend_from_slice(b" ");
        assert!(!lucide(&changed));
    }
}
