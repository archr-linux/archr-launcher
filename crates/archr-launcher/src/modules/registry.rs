//! What is installed and what the user decided (modules.toml), the settings
//! files, and the `View` the screens draw, in the launcher's language.
use super::manifest::{self, Manifest, Mode, ALIASES};
use archr_io::{atomic_publish, read_bytes, read_utf8, Symlinks};
use archr_ui::modules::{Module, Permission, RowKind, Status};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};
use toml::{Table, Value};

/// Where modules live; every path under the launcher's own folder.
#[derive(Clone, Debug)]
pub struct Dirs {
    /// `modules/<id>/`: the code, root-owned.
    pub modules: PathBuf,
    /// modules.toml: one record per module.
    pub state: PathBuf,
    /// `module-settings/<id>.json`, written only by the launcher.
    pub settings: PathBuf,
    /// `module-data/<id>/`, the module's own writable folder.
    pub data: PathBuf,
}
impl Dirs {
    pub fn new(base: &Path) -> Self {
        Self {
            modules: base.join("modules"),
            state: base.join("modules.toml"),
            settings: base.join("module-settings"),
            data: base.join("module-data"),
        }
    }
    pub fn module(&self, id: &str) -> PathBuf {
        self.modules.join(id)
    }
    pub fn previous(&self, id: &str) -> PathBuf {
        self.modules.join(format!(".previous-{id}"))
    }
    pub fn settings_file(&self, id: &str) -> PathBuf {
        self.settings.join(format!("{id}.json"))
    }
    /// The values of `secret` keyboard rows, apart: 0600, the module's own.
    pub fn secrets_file(&self, id: &str) -> PathBuf {
        self.settings.join(format!("{id}.secret.json"))
    }
    pub fn data_dir(&self, id: &str) -> PathBuf {
        self.data.join(id)
    }
}

/// The user's decisions about one module.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// `owner/repo` of the store release; empty for a local folder.
    pub source: String,
    pub version: String,
    /// Permission tokens the user accepted (`Permissions::tokens`).
    pub granted: Vec<String>,
    /// The user saw the permission page and accepted it.
    pub consent: bool,
    pub enabled: bool,
    /// Switched off after repeated failures, until the user switches it on.
    pub failed: bool,
    /// SHA-256 of the installed package, for the reviewed badge.
    pub digest: String,
    /// When it last ran, and when the store last updated it on its own
    /// (seconds since 1970; 0 = never).
    pub used: u64,
    pub auto_updated: u64,
}
/// Module uids, which are also their gids: above useradd's UID_MAX (60000)
/// and systemd-homed's 60001–60513, below DynamicUser's 61184–65519.
pub const UIDS: std::ops::Range<u32> = 60600..61100;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub modules: BTreeMap<String, Record>,
    /// Each module's own uid. Kept when the module is removed with its data
    /// kept, so the files still belong to it if it comes back.
    pub uids: BTreeMap<String, u32>,
    /// When the store was last asked for automatic updates (seconds since
    /// 1970).
    pub checked: u64,
}
impl State {
    /// The module's uid: the one it has, else the first free one from a
    /// hash of its id; `None` once the range is full.
    pub fn uid(&mut self, id: &str) -> Option<u32> {
        let taken = |uids: &BTreeMap<String, u32>, uid: u32| {
            !UIDS.contains(&uid) || uids.iter().any(|(k, v)| k != id && *v == uid)
        };
        match self.uids.get(id) {
            Some(&uid) if !taken(&self.uids, uid) => return Some(uid),
            _ => self.uids.remove(id),
        };
        let span = UIDS.end - UIDS.start;
        // FNV-1a: the same first choice on every build.
        let hash = id.bytes().fold(0x811c_9dc5_u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        });
        let uid = (0..span)
            .map(|i| UIDS.start + (hash % span + i) % span)
            .find(|&uid| !taken(&self.uids, uid))?;
        self.uids.insert(id.to_owned(), uid);
        Some(uid)
    }
    pub fn load(path: &Path) -> Self {
        read_utf8(path, 256 << 10, Symlinks::Reject)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let text = toml::to_string(self).map_err(io::Error::other)?;
        atomic_publish(path, true, |file| {
            io::Write::write_all(file, text.as_bytes())
        })
    }
}

/// One folder of `modules/`.
#[derive(Clone, Debug)]
pub struct Installed {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Option<Manifest>,
    /// Why it cannot run.
    pub problem: String,
    pub compatible: bool,
    /// The module's catalog in the launcher's language, then English.
    pub catalog: (Table, Table),
}
impl Installed {
    pub fn read(dir: &Path, language: &str) -> Self {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let (manifest, problem) = match manifest::check(dir) {
            Ok((manifest, warnings)) => {
                for warning in warnings {
                    log::warn!("Module {id}: {warning}");
                }
                (Some(manifest), String::new())
            }
            Err(errors) => {
                log::warn!("Module {id} is not valid: {}", errors.join("; "));
                (None, errors.join("\n"))
            }
        };
        let compatible = manifest.as_ref().is_some_and(|m| {
            manifest::api_ok(m.api_version)
                && !archr_ui::settings::store::newer(&m.min_launcher, env!("CARGO_PKG_VERSION"))
        });
        let file = language.replace('-', "_");
        let short = file.split('_').next().unwrap_or("en");
        let local = manifest::locale(dir, &file)
            .or_else(|| manifest::locale(dir, short))
            .unwrap_or_default();
        Self {
            id,
            dir: dir.to_owned(),
            manifest,
            problem,
            compatible,
            catalog: (local, manifest::locale(dir, "en").unwrap_or_default()),
        }
    }
    /// `@key` from the module's catalog; other text as it is.
    pub fn tr(&self, text: &str) -> String {
        let Some(key) = text.strip_prefix('@') else {
            return text.to_owned();
        };
        [&self.catalog.0, &self.catalog.1]
            .iter()
            .find_map(|table| table.get(key).and_then(Value::as_str))
            .unwrap_or(key)
            .to_owned()
    }
    /// A `ui.update`/`ui.notify` value: text, `@key`, or `{key, args}`.
    pub fn message(&self, value: &Json) -> Option<String> {
        let text = match value {
            Json::String(text) => self.tr(text),
            Json::Object(map) => {
                let mut text = self.tr(map.get("key")?.as_str()?);
                for (name, arg) in map
                    .get("args")
                    .and_then(Json::as_object)
                    .into_iter()
                    .flatten()
                {
                    let arg = match arg {
                        Json::String(s) => s.clone(),
                        Json::Number(n) => n.to_string(),
                        _ => continue,
                    };
                    text = text.replace(&format!("{{{name}}}"), &arg);
                }
                text
            }
            _ => return None,
        };
        Some(text.chars().filter(|c| !c.is_control()).take(200).collect())
    }
    /// `name` or `description`: `@key`, text, or a table by language.
    fn names(&self, value: Option<&Value>, language: &str) -> String {
        match value {
            Some(Value::Table(_)) => archr_ui::layout::Names::parse(value, "module")
                .get(language)
                .unwrap_or_default()
                .to_owned(),
            Some(Value::String(text)) => self.tr(text),
            _ => String::new(),
        }
    }
}

/// Every module folder, by id; staging and previous versions are skipped.
pub fn scan(dirs: &Dirs, language: &str) -> Vec<Installed> {
    let mut found: Vec<Installed> = fs::read_dir(&dirs.modules)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| Installed::read(&e.path(), language))
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// What a module asks for, as the permission page lists it.
pub fn permissions(manifest: &Manifest) -> Vec<Permission> {
    let line = |key, detail: &str, sensitive| Permission {
        key,
        detail: detail.to_owned(),
        sensitive,
    };
    let granted = &manifest.permissions;
    let mut list = Vec::new();
    if granted.net {
        list.push(line("modules.perm_net", "modules.perm_net_detail", true));
    }
    for alias in &granted.read {
        let path = ALIASES.iter().find(|(a, _)| a == alias).map_or("", |a| a.1);
        let key = match alias.as_str() {
            "roms" => "modules.perm_read_roms",
            "bios" => "modules.perm_read_bios",
            _ => "modules.perm_read_screenshots",
        };
        // Reading and the internet together could send the files away.
        list.push(line(key, path, granted.net));
    }
    if !granted.config_read.is_empty() {
        list.push(line(
            "modules.perm_config_read",
            &granted.config_read.join(", "),
            false,
        ));
    }
    if !granted.config_write.is_empty() {
        // It changes what the user chose.
        list.push(line(
            "modules.perm_config_write",
            &granted.config_write.join(", "),
            true,
        ));
    }
    if granted.hooks.iter().any(|h| h == "launch") {
        list.push(line(
            "modules.perm_hooks_launch",
            "modules.perm_hooks_launch_detail",
            false,
        ));
    }
    if granted.system.iter().any(|s| s == "cpu-profile") {
        list.push(line(
            "modules.perm_cpu_profile",
            "modules.perm_cpu_profile_detail",
            false,
        ));
    }
    if granted.launch {
        list.push(line(
            "modules.perm_launch",
            "modules.perm_launch_detail",
            false,
        ));
    }
    // The amber ones first: a list cut short never hides them.
    list.sort_by_key(|p| !p.sensitive);
    if list.is_empty() {
        list.push(line("modules.perm_none", "modules.perm_none_detail", false));
    }
    for (used, key) in [
        (!manifest.tiles.is_empty(), "modules.perm_tile"),
        (!manifest.shortcuts.is_empty(), "modules.perm_shortcut"),
        (!manifest.widgets.is_empty(), "modules.perm_widget"),
        (!manifest.quick.is_empty(), "modules.perm_quick"),
        (!manifest.pages.is_empty(), "modules.perm_page"),
        (manifest.mode == Mode::Daemon, "modules.perm_daemon"),
    ] {
        if used {
            list.push(line(key, "", false));
        }
    }
    list
}

/// Saved settings, string-typed for the screens; defaults fill the gaps.
pub fn settings(dirs: &Dirs, manifest: &Manifest) -> BTreeMap<String, String> {
    let load = |path: &Path| -> serde_json::Map<String, Json> {
        read_bytes(path, 64 << 10, Symlinks::Reject)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    };
    let saved = load(&dirs.settings_file(&manifest.id));
    let secrets = load(&dirs.secrets_file(&manifest.id));
    let mut values = BTreeMap::new();
    for row in manifest.pages.iter().flat_map(|p| &p.rows) {
        let Some(key) = row.kind.key() else { continue };
        let saved = if secret(&row.kind) { &secrets } else { &saved };
        let value = match saved.get(key) {
            Some(Json::Bool(on)) => if *on { "1" } else { "0" }.to_owned(),
            Some(Json::String(text)) => text.clone(),
            Some(Json::Number(n)) => n.to_string(),
            _ => row.default.clone(),
        };
        values.insert(key.to_owned(), value);
    }
    values
}

/// The JSON a module reads: toggles and checks as booleans, sliders as
/// numbers, the rest as text.
pub fn settings_json(manifest: &Manifest, values: &BTreeMap<String, String>) -> Json {
    let mut map = serde_json::Map::new();
    for row in manifest.pages.iter().flat_map(|p| &p.rows) {
        let Some(key) = row.kind.key() else { continue };
        let value = values.get(key).unwrap_or(&row.default);
        let json = match &row.kind {
            RowKind::Toggle { .. } | RowKind::Check { .. } => Json::Bool(value == "1"),
            RowKind::Value {
                labels, choices, ..
            } if labels == choices && value.parse::<i64>().is_ok() => {
                Json::from(value.parse::<i64>().unwrap_or(0))
            }
            _ => Json::String(value.clone()),
        };
        map.insert(key.to_owned(), json);
    }
    Json::Object(map)
}

fn secret(kind: &RowKind) -> bool {
    matches!(kind, RowKind::Keyboard { secret: true, .. })
}
/// Whether the module has `secret` rows, and so a secrets file.
pub fn has_secrets(manifest: &Manifest) -> bool {
    manifest
        .pages
        .iter()
        .flat_map(|p| &p.rows)
        .any(|r| secret(&r.kind))
}

/// Writes what a module reads: its settings (0644, the launcher's) and the
/// `secret` values apart (0600, handed to `owner`, the module's uid).
pub fn save_settings(
    dirs: &Dirs,
    manifest: &Manifest,
    values: &BTreeMap<String, String>,
    owner: Option<u32>,
) -> io::Result<()> {
    let all = settings_json(manifest, values);
    let hidden: Vec<&str> = manifest
        .pages
        .iter()
        .flat_map(|p| &p.rows)
        .filter(|r| secret(&r.kind))
        .filter_map(|r| r.kind.key())
        .collect();
    let part = |secrets: bool| -> Json {
        let map = all.as_object().into_iter().flatten();
        map.filter(|(k, _)| hidden.contains(&k.as_str()) == secrets)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<serde_json::Map<_, _>>()
            .into()
    };
    write_json(&dirs.settings_file(&manifest.id), &part(false), 0o644, None)?;
    let path = dirs.secrets_file(&manifest.id);
    if hidden.is_empty() {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    write_json(&path, &part(true), 0o600, owner)
}
/// Mode and owner are set before the content is written: a secret is never
/// readable by anyone else, not even for a moment.
fn write_json(path: &Path, json: &Json, mode: u32, owner: Option<u32>) -> io::Result<()> {
    use std::os::unix::fs::{fchown, PermissionsExt};
    let text = json.to_string();
    atomic_publish(path, true, |file| {
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        if let Some(uid) = owner {
            fchown(&*file, Some(uid), Some(uid))?;
        }
        io::Write::write_all(file, text.as_bytes())
    })
}

/// One module for the screens, texts resolved.
pub fn module(
    dirs: &Dirs,
    installed: &Installed,
    record: Option<&Record>,
    language: &str,
) -> Module {
    let record = record.cloned().unwrap_or_default();
    let Some(manifest) = &installed.manifest else {
        return Module {
            id: installed.id.clone(),
            name: installed.id.clone(),
            source: record.source,
            status: Status::Invalid,
            problem: installed.problem.clone(),
            ..Module::default()
        };
    };
    let tr = |text: &str| installed.tr(text);
    let svg = |name: &str| -> Option<Arc<str>> {
        let path = installed.dir.join("icons").join(format!("{name}.svg"));
        read_utf8(&path, 16 << 10, Symlinks::Reject)
            .ok()
            .map(Into::into)
    };
    let entries = |list: &[archr_ui::modules::Entry]| {
        list.iter()
            .map(|e| archr_ui::modules::Entry {
                id: e.id.clone(),
                label: tr(&e.label),
                icon: e.icon.as_deref().or(manifest.icon.as_deref()).and_then(svg),
                target: e.target.clone(),
            })
            .collect()
    };
    let granted = record.consent
        && manifest
            .permissions
            .tokens()
            .iter()
            .all(|t| record.granted.contains(t));
    let status = if record.failed {
        Status::Failed
    } else if !installed.compatible {
        Status::Incompatible
    } else {
        Status::Ready
    };
    Module {
        id: manifest.id.clone(),
        name: Some(installed.names(manifest.name.as_ref(), language))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| manifest.id.clone()),
        description: installed.names(manifest.description.as_ref(), language),
        author: manifest.author.clone(),
        version: manifest.version.clone(),
        license: manifest.license.clone(),
        icon: manifest.icon.as_deref().and_then(svg),
        enabled: record.enabled,
        granted,
        status,
        source: record.source.clone(),
        reviewed: false,
        update: None,
        permissions: permissions(manifest),
        problem: if installed.compatible {
            String::new()
        } else {
            "modules.incompatible".into()
        },
        tiles: entries(&manifest.tiles),
        shortcuts: entries(&manifest.shortcuts),
        widgets: entries(&manifest.widgets),
        digest: record.digest.clone(),
        used: record.used,
        auto_updated: record.auto_updated,
        failures: 0,
        quick: manifest
            .quick
            .iter()
            .map(|q| archr_ui::modules::Quick {
                label: tr(&q.label),
                ..q.clone()
            })
            .collect(),
        pages: manifest
            .pages
            .iter()
            .map(|page| archr_ui::modules::Page {
                id: page.id.clone(),
                title: tr(&page.title),
                rows: page
                    .rows
                    .iter()
                    .map(|row| archr_ui::modules::Row {
                        label: tr(&row.label),
                        subtitle: row.subtitle.as_deref().map(tr),
                        section: row.section.as_deref().map(tr),
                        kind: match &row.kind {
                            RowKind::Value {
                                key,
                                choices,
                                labels,
                            } => RowKind::Value {
                                key: key.clone(),
                                choices: choices.clone(),
                                labels: labels
                                    .iter()
                                    .zip(choices)
                                    .map(|(l, c)| if l.is_empty() { c.clone() } else { tr(l) })
                                    .collect(),
                            },
                            RowKind::Action { action, confirm } => RowKind::Action {
                                action: action.clone(),
                                confirm: confirm.as_deref().map(tr),
                            },
                            other => other.clone(),
                        },
                        default: row.default.clone(),
                    })
                    .collect(),
            })
            .collect(),
        settings: settings(dirs, manifest),
    }
}

/// The record a module gets on its first sight: a local folder starts off
/// and without consent; the screens ask before it runs.
pub fn record<'a>(state: &'a mut State, id: &str) -> &'a mut Record {
    state.modules.entry(id.to_owned()).or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uids_stay_put_across_restarts_and_never_collide() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("modules.toml");
        let mut state = State::default();
        let alpha = state.uid("alpha").unwrap();
        let beta = state.uid("beta").unwrap();
        assert!(UIDS.contains(&alpha) && UIDS.contains(&beta) && alpha != beta);
        state.save(&path).unwrap();
        let mut loaded = State::load(&path);
        assert_eq!(
            (loaded.uid("alpha"), loaded.uid("beta")),
            (Some(alpha), Some(beta))
        );
        // Someone else already holds alpha's first choice: the next one.
        let mut taken = State::default();
        taken.uids.insert("other".into(), alpha);
        let moved = taken.uid("alpha").unwrap();
        assert_ne!(moved, alpha);
        assert_eq!(taken.uid("alpha"), Some(moved));
        // A duplicate or out-of-range record is given a fresh uid.
        let mut broken = State::default();
        broken.uids.insert("a".into(), 61000);
        broken.uids.insert("b".into(), 61000);
        broken.uids.insert("c".into(), 1000);
        let (a, b, c) = (broken.uid("a"), broken.uid("b"), broken.uid("c"));
        assert!(a != b && UIDS.contains(&c.unwrap()));
        // The range is finite.
        let mut full = State::default();
        for n in 0..UIDS.len() {
            assert!(full.uid(&format!("m{n}")).is_some());
        }
        assert_eq!(full.uid("one-more"), None);
        let mut seen: Vec<u32> = full.uids.values().copied().collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), UIDS.len());
    }

    #[test]
    fn secret_values_go_to_their_own_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = Dirs::new(dir.path());
        fs::create_dir_all(dirs.module("sec")).unwrap();
        fs::write(
            dirs.module("sec").join("module.toml"),
            "id = \"sec\"\nversion = \"1.0.0\"\napi_version = 1\nname = \"X\"\n\
             [run]\nruntime = \"sh\"\nentry = \"main.sh\"\nmode = \"oneshot\"\n\
             [[settings.pages]]\nid = \"p\"\n\
             [[settings.pages.rows]]\ntype = \"keyboard\"\nkey = \"pin\"\nsecret = true\n\
             [[settings.pages.rows]]\ntype = \"keyboard\"\nkey = \"name\"\n",
        )
        .unwrap();
        fs::write(dirs.module("sec").join("main.sh"), "").unwrap();
        fs::create_dir_all(dirs.module("sec").join("locales")).unwrap();
        fs::write(dirs.module("sec").join("locales/en.toml"), "").unwrap();
        let (manifest, _) = manifest::check(&dirs.module("sec")).unwrap();
        let values = BTreeMap::from([("pin".into(), "1234".into()), ("name".into(), "ana".into())]);
        save_settings(&dirs, &manifest, &values, None).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            fs::read_to_string(dirs.settings_file("sec")).unwrap(),
            r#"{"name":"ana"}"#
        );
        assert_eq!(mode(&dirs.settings_file("sec")), 0o644);
        assert_eq!(
            fs::read_to_string(dirs.secrets_file("sec")).unwrap(),
            r#"{"pin":"1234"}"#
        );
        assert_eq!(mode(&dirs.secrets_file("sec")), 0o600);
        assert_eq!(settings(&dirs, &manifest), values);
    }

    #[test]
    fn amber_permissions_come_first() {
        let source = "id = \"x-y\"\nversion = \"1.0.0\"\napi_version = 2\n\
            [run]\nruntime = \"sh\"\nentry = \"main.sh\"\nmode = \"oneshot\"\n\
            [permissions]\nconfig_read = [\"system.language\"]\nlaunch = true\n\
            config_write = [\"system.cpugovernor\"]\n";
        let (manifest, _) = manifest::parse(source).unwrap();
        let keys: Vec<&str> = permissions(&manifest).iter().map(|p| p.key).collect();
        assert_eq!(
            keys,
            [
                "modules.perm_config_write",
                "modules.perm_config_read",
                "modules.perm_launch"
            ]
        );
    }

    #[test]
    fn texts_come_from_the_module_catalog_with_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let module = dir.path().join("teste");
        fs::create_dir_all(module.join("locales")).unwrap();
        fs::write(
            module.join("locales/pt_BR.toml"),
            "\"st\" = \"{pct} % · {w} W\"\n",
        )
        .unwrap();
        fs::write(module.join("locales/en.toml"), "\"other\" = \"Other\"\n").unwrap();
        let installed = Installed::read(&module, "pt_BR");
        let value = serde_json::json!({"key": "@st", "args": {"pct": 87, "w": "1.9"}});
        assert_eq!(installed.message(&value).unwrap(), "87 % · 1.9 W");
        assert_eq!(installed.tr("@other"), "Other");
        assert_eq!(installed.tr("@missing"), "missing");
        assert_eq!(installed.tr("plain"), "plain");
        assert!(installed.manifest.is_none(), "no module.toml");
    }
}
