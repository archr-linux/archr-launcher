//! Import from RetroArch: a RetroArch setup copied to the card, filtered to
//! what is safe on this device, then applied in one go or not at all.
//!
//! Source (`roms/retroarch-import/`, or the one folder in it when a whole
//! RetroArch folder was copied there): `retroarch.cfg`, `config/<core>/`
//! (core and game overrides `.cfg`, core options `.opt`), `remaps/<core>/*.rmp`,
//! `shaders/**` (presets, shader sources and their textures) and `overlays/**`
//! (overlay `.cfg` files and their images); also `roms/shaders/`,
//! `roms/overlays/` and each system's `roms/<system>/shaders|overlays/`,
//! where many keep them (those land under `<system>/`). Only allow-listed `retroarch.cfg`
//! keys go into the user's own retroarch.cfg (never system.cfg, so the
//! launcher does not impose them); paths, drivers, the device's video mode,
//! credentials and commands never do. Files are bounded, never symbolic
//! links, configs UTF-8, and overrides only for installed cores. Shader
//! presets point at this device's shader directory or are left out; shader
//! sources, textures and overlays are copied as they are, never over a file
//! the system ships.
use archr_io::{atomic_publish, read_bytes, read_utf8, remove_durable, Symlinks::Reject};
use archr_ui::settings::raimport::Found;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

const CONFIG_LIMIT: u64 = 1024 * 1024;
const FILE_LIMIT: u64 = 256 * 1024;
const FILES_LIMIT: usize = 200;
/// Files copied as they are (shader sources, textures, overlays): each, how
/// many, and all together.
const COPY_LIMIT: u64 = 16 * 1024 * 1024;
const COPIES_LIMIT: usize = 2000;
const COPIES_TOTAL: u64 = 256 * 1024 * 1024;
/// Folders below shaders/ and overlays/.
const DEPTH: usize = 8;
const VALUE_LIMIT: usize = 512;
/// Lines the list page shows of each kind; the rest is one "+N" line.
const LIST_LIMIT: usize = 200;
/// Shader trees a preset may have been saved against, on any system.
const SHADER_ROOTS: [&str; 6] = [
    "shaders_slang",
    "shaders_glsl",
    "slang-shaders",
    "glsl-shaders",
    "common-shaders",
    "shaders",
];
/// Copied as they are: what presets load, and what overlays are made of.
const SHADER_FILES: [&str; 9] = [
    "slang", "glsl", "h", "inc", "png", "jpg", "jpeg", "bmp", "tga",
];
const OVERLAY_FILES: [&str; 6] = ["cfg", "png", "jpg", "jpeg", "bmp", "tga"];

/// Where everything is read and written; the host passes the real paths.
#[derive(Clone, Debug)]
pub struct Roots {
    pub source: PathBuf,
    /// `~/.config/retroarch`: retroarch.cfg and `config/` (rgui_config_directory).
    pub retroarch: PathBuf,
    /// input_remapping_directory.
    pub remaps: PathBuf,
    /// video_shader_dir, the merged shader tree.
    pub shaders: PathBuf,
    /// libretro_directory: `<name>_libretro.so` beside its `.info`.
    pub cores: PathBuf,
    /// The system's shader trees under video_shader_dir.
    pub factory: Vec<PathBuf>,
    /// overlay_directory, and the system's overlays under it.
    pub overlays: PathBuf,
    pub factory_overlays: PathBuf,
    /// The ROMs folder: the user's own roms/overlays and roms/shaders, and
    /// roms/<system>/overlays and roms/<system>/shaders, read like the
    /// import folder's.
    pub roms: PathBuf,
}

/// What an import would do: the screen's summary and the files to write.
#[derive(Debug, Default)]
pub struct Plan {
    pub found: Found,
    pub settings: Vec<(String, String)>,
    /// Destination and complete contents.
    pub files: Vec<(PathBuf, String)>,
    /// Destination and the file copied there as it is.
    pub copies: Vec<(PathBuf, PathBuf)>,
    copied: u64,
    /// Every destination a copy was considered for, taken or not: the first
    /// folder that has a file decides, a later one never replaces it.
    claimed: BTreeSet<PathBuf>,
}

/// Why a key or a file stays out, as catalog keys.
const BLOCKED: &str = "raimport.why_blocked";
const UNKNOWN: &str = "raimport.why_unknown";
const INCLUDE: &str = "raimport.why_include";
const BAD_FILE: &str = "raimport.why_file";
const NO_CORE: &str = "raimport.why_core";
const NO_SHADER: &str = "raimport.why_shader";
const FACTORY: &str = "raimport.why_factory";
const SAME: &str = "raimport.why_same";
const TWICE: &str = "raimport.why_twice";

/// Never imported, whatever the file: where things live, which drivers run,
/// accounts, and anything that runs a command or calls a server. Anything
/// else outside the allow-list is left out anyway.
fn blocked(key: &str) -> bool {
    ["_dir", "_path", "_cmd", "_url"]
        .iter()
        .any(|end| key.ends_with(end))
        || ["_directory", "driver", "password", "token", "username"]
            .iter()
            .any(|word| key.contains(word))
}

/// The retroarch.cfg keys worth carrying over (look, menu combos, overlay
/// placement, latency, rewind, run-ahead, savestates, menu). Not the custom
/// viewport, rotation, frame delay or font size, sized for another screen
/// and CPU, nor "save on exit", which stays the user's choice here.
fn allowed(key: &str) -> bool {
    const EXACT: [&str; 21] = [
        "video_scale_integer",
        "video_scale_integer_overscale",
        "video_smooth",
        "video_ctx_scaling",
        "aspect_ratio_index",
        "video_aspect_ratio",
        "video_aspect_ratio_auto",
        "video_shader_enable",
        "video_frame_delay_auto",
        "video_font_enable",
        "audio_latency",
        "audio_sync",
        "audio_volume",
        "audio_rate_control_delta",
        "fps_show",
        "framecount_show",
        "memory_show",
        "statistics_show",
        "fastforward_ratio",
        "slowmotion_ratio",
        "pause_nonactive",
    ];
    EXACT.contains(&key)
        || [
            "video_viewport_bias_",
            "video_msg_",
            "input_overlay_",
            "menu_",
            "rewind_",
            "run_ahead_",
            "savestate_",
            "auto_",
        ]
        .iter()
        .any(|start| key.starts_with(start))
        || hotkey(key)
}

/// Hotkey settings that mean the same on any pad. Button and axis numbers
/// (`input_*_btn`, `_axis`, `_mbtn`) belong to the other device's pad.
fn hotkey(key: &str) -> bool {
    [
        "input_menu_toggle_gamepad_combo",
        "input_quit_gamepad_combo",
        "input_hotkey_block_delay",
    ]
    .contains(&key)
}

/// One `key = "value"` line, or None for blanks and comments. An include
/// pulls in another file and is refused.
enum Line {
    Entry(String, String),
    Include,
    Bad,
}
fn parse(line: &str) -> Option<Line> {
    let line = line.trim();
    if line.starts_with("#include") {
        return Some(Line::Include);
    }
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let Some((key, value)) = line.split_once('=') else {
        return Some(Line::Bad);
    };
    let key = key.trim();
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let good = !key.is_empty()
        && key.len() <= 96
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
        && value.len() <= VALUE_LIMIT
        && !value.contains('"')
        && !value.chars().any(char::is_control);
    Some(if good {
        Line::Entry(key.into(), value.into())
    } else {
        Line::Bad
    })
}

/// A regular, bounded, UTF-8 file; a symbolic link is refused, never followed.
fn read(path: &Path, limit: u64) -> Option<String> {
    read_utf8(path, limit, Reject).ok()
}

/// A plain name inside the import folder: no separators, no "..", no hidden.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\'])
        && name.chars().all(|c| !c.is_control())
}

/// Entries of a directory that is not a symbolic link, sorted.
fn entries(dir: &Path) -> Vec<(String, PathBuf, fs::FileType)> {
    let real = fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir());
    let mut list: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .filter(|_| real)
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            Some((name, entry.path(), entry.file_type().ok()?))
        })
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list
}

/// `corename` of every core whose library is installed.
fn installed_cores(cores: &Path) -> BTreeSet<String> {
    entries(cores)
        .into_iter()
        .filter_map(|(name, path, _)| {
            name.strip_suffix("_libretro.so")?;
            let info = read(&path.with_extension("info"), FILE_LIMIT)?;
            info.lines().find_map(|line| match parse(line)? {
                Line::Entry(key, value) if key == "corename" => Some(value),
                _ => None,
            })
        })
        .collect()
}

impl Plan {
    fn ignore(&mut self, item: impl Into<String>, why: &str) {
        self.found.ignored.push((item.into(), why.into()));
    }
    /// Whether the file was taken; past the limit it is listed as ignored.
    fn write(&mut self, to: PathBuf, item: String, detail: String, text: String) -> bool {
        if self.files.len() >= FILES_LIMIT {
            self.ignore(item, BAD_FILE);
            return false;
        }
        self.found.apply.push((item, detail));
        self.files.push((to, text));
        true
    }
    /// A file to copy as it is, within the limits, unless the destination
    /// already holds the same bytes.
    fn copy(&mut self, to: PathBuf, from: PathBuf, item: String, size: u64) -> bool {
        if !self.claimed.insert(to.clone()) {
            self.ignore(item, TWICE);
            return false;
        }
        if same(&from, &to, size) {
            self.ignore(item, SAME);
            return false;
        }
        if size > COPY_LIMIT
            || self.copies.len() >= COPIES_LIMIT
            || self.copied + size > COPIES_TOTAL
        {
            self.ignore(item, BAD_FILE);
            return false;
        }
        self.copied += size;
        self.found.apply.push((item, "raimport.file_copy".into()));
        self.copies.push((to, from));
        true
    }
    /// Allowed entries of a RetroArch config, the rest listed as ignored.
    fn filter(
        &mut self,
        text: &str,
        item: &str,
        keep: impl Fn(&str) -> bool,
    ) -> Vec<(String, String)> {
        let mut kept = BTreeMap::new();
        let mut include = false;
        for line in text.lines() {
            match parse(line) {
                None => {}
                Some(Line::Include) => include = true,
                Some(Line::Bad) => self.ignore(format!("{item}: {}", short(line)), BAD_FILE),
                // A path from another system means nothing here.
                Some(Line::Entry(key, value))
                    if blocked(&key) || value.starts_with(['/', '~']) || value.contains(":\\") =>
                {
                    self.ignore(key, BLOCKED)
                }
                Some(Line::Entry(key, _)) if !keep(&key) => self.ignore(key, UNKNOWN),
                Some(Line::Entry(key, value)) => {
                    kept.insert(key, value);
                }
            }
        }
        if include {
            self.ignore(format!("{item}: #include"), INCLUDE);
        }
        kept.into_iter().collect()
    }
}

fn short(line: &str) -> String {
    line.trim().chars().take(40).collect()
}
fn config_text(entries: &[(String, String)]) -> String {
    entries
        .iter()
        .map(|(key, value)| format!("{key} = \"{value}\"\n"))
        .collect()
}

/// Where the setup is: the import folder, or the one folder in it when a
/// whole RetroArch folder was copied there.
fn origin(source: &Path) -> PathBuf {
    const KNOWN: [&str; 5] = ["retroarch.cfg", "config", "remaps", "shaders", "overlays"];
    if KNOWN
        .iter()
        .any(|name| fs::symlink_metadata(source.join(name)).is_ok())
    {
        return source.to_path_buf();
    }
    let folders: Vec<_> = entries(source)
        .into_iter()
        .filter(|(name, _, kind)| kind.is_dir() && plain(name))
        .collect();
    match folders.as_slice() {
        [(_, path, _)] => path.clone(),
        _ => source.to_path_buf(),
    }
}

/// Everything the folder holds, sorted into what applies and what does not.
pub fn scan(roots: &Roots) -> Plan {
    let roots = &Roots {
        source: origin(&roots.source),
        ..roots.clone()
    };
    let mut plan = Plan::default();
    let source = &roots.source;
    let config = source.join("retroarch.cfg");
    if fs::symlink_metadata(&config).is_ok() {
        match read(&config, CONFIG_LIMIT) {
            Some(text) => {
                let before = plan.found.ignored.len();
                plan.settings = plan.filter(&text, "retroarch.cfg", allowed);
                plan.found.settings = plan.settings.len();
                plan.found.skipped = plan.found.ignored.len() - before;
                for (key, value) in &plan.settings {
                    plan.found.apply.push((key.clone(), value.clone()));
                }
            }
            None => plan.ignore("retroarch.cfg", BAD_FILE),
        }
    }
    let cores = installed_cores(&roots.cores);
    scan_overrides(&mut plan, roots, &cores);
    scan_remaps(&mut plan, roots, &cores);
    scan_shaders(&mut plan, roots);
    scan_overlays(&mut plan, roots);
    cap(&mut plan.found.apply);
    cap(&mut plan.found.ignored);
    plan
}

fn cap(list: &mut Vec<(String, String)>) {
    if list.len() > LIST_LIMIT {
        let more = list.len() - (LIST_LIMIT - 1);
        list.truncate(LIST_LIMIT - 1);
        list.push((format!("+{more}"), format!("raimport.more|count={more}")));
    }
}

/// `config/<core>/`: overrides (`.cfg`, same keys as retroarch.cfg) and core
/// options (`.opt`, any option but the blocked ones).
fn scan_overrides(plan: &mut Plan, roots: &Roots, cores: &BTreeSet<String>) {
    for (core, dir, kind) in entries(&roots.source.join("config")) {
        if !kind.is_dir() || !plain(&core) {
            plan.ignore(format!("config/{core}"), BAD_FILE);
            continue;
        }
        if !cores.contains(&core) {
            plan.ignore(format!("config/{core}"), NO_CORE);
            continue;
        }
        let mut used = false;
        for (name, path, kind) in entries(&dir) {
            let item = format!("config/{core}/{name}");
            let options = name.ends_with(".opt");
            if !(name.ends_with(".cfg") || options) {
                plan.ignore(item, UNKNOWN);
                continue;
            }
            let text = (kind.is_file() && plain(&name))
                .then(|| read(&path, FILE_LIMIT))
                .flatten();
            let Some(text) = text else {
                plan.ignore(item, BAD_FILE);
                continue;
            };
            let kept = if options {
                plan.filter(&text, &item, |_| true)
            } else {
                plan.filter(&text, &item, allowed)
            };
            if kept.is_empty() {
                continue;
            }
            let to = roots.retroarch.join("config").join(&core).join(&name);
            let detail = format!("raimport.file_settings|count={}", kept.len());
            if plan.write(to, item, detail, config_text(&kept)) {
                plan.found.overrides += 1;
                used = true;
            }
        }
        if used {
            plan.found.cores.push(core);
        }
    }
}

/// `remaps/<core>/*.rmp`: button remaps, never which pad is which player.
fn scan_remaps(plan: &mut Plan, roots: &Roots, cores: &BTreeSet<String>) {
    for (core, dir, kind) in entries(&roots.source.join("remaps")) {
        if !kind.is_dir() || !plain(&core) || !cores.contains(&core) {
            plan.ignore(
                format!("remaps/{core}"),
                if kind.is_dir() { NO_CORE } else { BAD_FILE },
            );
            continue;
        }
        for (name, path, kind) in entries(&dir) {
            let item = format!("remaps/{core}/{name}");
            let text = (kind.is_file() && plain(&name) && name.ends_with(".rmp"))
                .then(|| read(&path, FILE_LIMIT))
                .flatten();
            let Some(text) = text else {
                plan.ignore(item, BAD_FILE);
                continue;
            };
            // Remaps are player binds by nature; which pad is which stays.
            let kept = plan.filter(&text, &item, |key| {
                key.starts_with("input_") && !key.contains("joypad_index")
            });
            if kept.is_empty() {
                continue;
            }
            let to = roots.remaps.join(&core).join(&name);
            let detail = format!("raimport.file_settings|count={}", kept.len());
            if plan.write(to, item, detail, config_text(&kept)) {
                plan.found.remaps += 1;
            }
        }
    }
}

/// Regular files under `dir`, sorted, as (path below `dir`, path, size).
/// Links, odd names and folders deeper than DEPTH are listed as ignored.
fn walk(plan: &mut Plan, dir: &Path, label: &str) -> Vec<(PathBuf, PathBuf, u64)> {
    let mut files = Vec::new();
    let mut pending = vec![(PathBuf::new(), dir.to_path_buf())];
    while let Some((relative, dir)) = pending.pop() {
        for (name, path, kind) in entries(&dir) {
            let item = relative.join(&name);
            if !plain(&name) || kind.is_symlink() || relative.components().count() >= DEPTH {
                plan.ignore(format!("{label}/{}", item.display()), BAD_FILE);
            } else if kind.is_dir() {
                pending.push((item, path));
            } else if kind.is_file() {
                let size = fs::symlink_metadata(&path).map_or(u64::MAX, |m| m.len());
                files.push((item, path, size));
            } else {
                plan.ignore(format!("{label}/{}", item.display()), BAD_FILE);
            }
        }
    }
    files.sort();
    files
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Whether `to` already holds the bytes of `from`.
fn same(from: &Path, to: &Path, size: u64) -> bool {
    fs::symlink_metadata(to).is_ok_and(|m| m.is_file() && m.len() == size)
        && matches!(
            (read_bytes(from, COPY_LIMIT, Reject), read_bytes(to, COPY_LIMIT, Reject)),
            (Ok(a), Ok(b)) if a == b
        )
}

/// Where shaders and overlays come from, as (folder, name the list shows,
/// subfolder they land in): the import folder's, the user's own
/// roms/<name>/, then roms/<system>/<name>/ of each system folder (one
/// level, never deeper into the ROMs), which lands under <system>/ so two
/// systems never meet.
fn sources(roots: &Roots, name: &str) -> Vec<(PathBuf, String, PathBuf)> {
    let mut list = vec![
        (roots.source.join(name), name.to_owned(), PathBuf::new()),
        (
            roots.roms.join(name),
            format!("roms/{name}"),
            PathBuf::new(),
        ),
    ];
    for (system, dir, kind) in entries(&roots.roms) {
        let skip = ["shaders", "overlays", "retroarch-import"].contains(&system.as_str());
        if kind.is_dir() && plain(&system) && !skip {
            let folder = dir.join(name);
            if fs::symlink_metadata(&folder).is_ok_and(|m| m.is_dir()) {
                list.push((
                    folder,
                    format!("roms/{system}/{name}"),
                    PathBuf::from(&system),
                ));
            }
        }
    }
    list
}

/// `shaders/**` and roms/shaders/**, at the same place under the shader
/// directory: sources and textures as they are, then presets pointed at
/// this device.
fn scan_shaders(plan: &mut Plan, roots: &Roots) {
    let mut presets = Vec::new();
    let mut incoming = BTreeSet::new();
    for (dir, label, under) in sources(roots, "shaders") {
        for (item, path, size) in walk(plan, &dir, &label) {
            let shown = format!("{label}/{}", item.display());
            let item = under.join(item);
            let kind = extension(&item);
            if kind == "slangp" || kind == "glslp" {
                presets.push((item, path, shown));
            } else if !SHADER_FILES.contains(&kind.as_str()) {
                plan.ignore(shown, UNKNOWN);
            } else if roots.factory.iter().any(|dir| dir.join(&item).exists()) {
                // The system's own file stays; presets find it there.
                plan.ignore(shown, FACTORY);
            } else if plan.copy(roots.shaders.join(&item), path, shown, size) {
                plan.found.shader_files += 1;
                incoming.insert(item);
            }
        }
    }
    let mut taken = BTreeSet::new();
    for (item, path, shown) in presets {
        // Never hide a preset the system ships under the same name.
        if roots.factory.iter().any(|dir| dir.join(&item).exists()) {
            plan.ignore(shown, FACTORY);
            continue;
        }
        if taken.contains(&item) {
            plan.ignore(shown, TWICE);
            continue;
        }
        match read(&path, FILE_LIMIT).map(|text| preset(&text, &item, roots, &incoming)) {
            Some(Ok(text)) => {
                let to = roots.shaders.join(&item);
                if plan.write(to, shown, "raimport.file_preset".into(), text) {
                    plan.found.presets += 1;
                    taken.insert(item);
                }
            }
            Some(Err(why)) => plan.ignore(shown, why),
            None => plan.ignore(shown, BAD_FILE),
        }
    }
}

/// `overlays/**` and roms/overlays/** as they are, at the same place under
/// the overlay directory.
fn scan_overlays(plan: &mut Plan, roots: &Roots) {
    for (dir, label, under) in sources(roots, "overlays") {
        for (item, path, size) in walk(plan, &dir, &label) {
            let shown = format!("{label}/{}", item.display());
            let item = under.join(item);
            if !OVERLAY_FILES.contains(&extension(&item).as_str()) {
                plan.ignore(shown, UNKNOWN);
            } else if roots.factory_overlays.join(&item).exists() {
                plan.ignore(shown, FACTORY);
            } else if plan.copy(roots.overlays.join(&item), path, shown, size) {
                plan.found.overlays += 1;
            }
        }
    }
}

/// A preset with every shader, texture and reference pointing at this
/// device's shader directory, or why it cannot.
fn preset(
    text: &str,
    item: &Path,
    roots: &Roots,
    incoming: &BTreeSet<PathBuf>,
) -> Result<String, &'static str> {
    let textures: Vec<String> = text
        .lines()
        .find_map(|line| match parse(line)? {
            Line::Entry(key, value) if key == "textures" => Some(value),
            _ => None,
        })
        .map(|names| names.split(';').map(str::to_owned).collect())
        .unwrap_or_default();
    let here = item.parent().unwrap_or(Path::new(""));
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(target) = trimmed.strip_prefix("#reference") {
            let target = target.trim().trim_matches('"');
            let found = locate(target, here, roots, incoming).ok_or(NO_SHADER)?;
            out.push_str(&format!("#reference \"{}\"\n", found.display()));
            continue;
        }
        match parse(line) {
            None => out.push_str(&format!("{trimmed}\n")),
            Some(Line::Include | Line::Bad) => return Err(BAD_FILE),
            Some(Line::Entry(key, value)) => {
                let file = key
                    .strip_prefix("shader")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                    || textures.contains(&key);
                let value = if file {
                    let found = locate(&value, here, roots, incoming).ok_or(NO_SHADER)?;
                    found.display().to_string()
                } else {
                    value
                };
                out.push_str(&format!("{key} = \"{value}\"\n"));
            }
        }
    }
    Ok(out)
}

/// A shader file on this device, or coming with the import, for a path saved
/// elsewhere: the part after a known shader tree, else the path beside the
/// preset; never outside.
fn locate(
    saved: &str,
    here: &Path,
    roots: &Roots,
    incoming: &BTreeSet<PathBuf>,
) -> Option<PathBuf> {
    let saved = saved.replace('\\', "/");
    let parts: Vec<&str> = saved.split('/').collect();
    let tail = parts
        .iter()
        .rposition(|part| SHADER_ROOTS.contains(part))
        .map(|at| parts[at + 1..].join("/"));
    let relative = match tail {
        Some(tail) => normal(Path::new(&tail))?,
        None if !saved.starts_with('/') => normal(&here.join(&saved))?,
        None => return None,
    };
    let path = roots.shaders.join(&relative);
    (incoming.contains(&relative) || fs::metadata(&path).is_ok_and(|m| m.is_file())).then_some(path)
}

/// A relative path with ".." resolved, or None when it leaves its root.
fn normal(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(name) => out.push(name),
            Component::CurDir => {}
            Component::ParentDir if out.pop() => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// The user's retroarch.cfg with the imported keys replaced or added.
fn merged(current: &str, settings: &[(String, String)]) -> String {
    let mut left: BTreeMap<&str, &str> = settings
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut out = String::with_capacity(current.len());
    for line in current.lines() {
        let key = line.split_once('=').map(|(k, _)| k.trim());
        match key.and_then(|k| left.remove_entry(k)) {
            Some((key, value)) => out.push_str(&format!("{key} = \"{value}\"\n")),
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    for (key, value) in left {
        out.push_str(&format!("{key} = \"{value}\"\n"));
    }
    out
}

/// What a destination held before the import: None when it did not exist.
type Before = Option<Vec<u8>>;

/// What goes to a destination: text made here, or a file copied as it is.
enum Source {
    Text(String),
    File(PathBuf),
}

/// A destination's bytes, read before anything is written. Anything that is
/// not a readable regular file stops the import.
fn before(path: &Path) -> Result<Before, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Ok(meta) if meta.is_file() => {
            read_bytes(path, COPY_LIMIT, Reject)
                .map(Some)
                .map_err(|error| {
                    log::warn!("Import: cannot read {}: {error}", path.display());
                    "raimport.failed".to_owned()
                })
        }
        _ => Err("raimport.failed".into()),
    }
}

/// Saves what every destination holds, then writes each file; when one
/// write fails, the earlier ones get their saved bytes back (a file that did
/// not exist is removed, one that existed is never removed).
pub fn apply(plan: &Plan, roots: &Roots) -> Result<usize, String> {
    let mut writes: Vec<(PathBuf, Source, Before)> = Vec::new();
    if !plan.settings.is_empty() {
        let path = roots.retroarch.join("retroarch.cfg");
        let saved = before(&path)?.ok_or("raimport.no_retroarch_cfg")?;
        let current = String::from_utf8(saved.clone()).map_err(|_| "raimport.failed")?;
        let text = merged(&current, &plan.settings);
        writes.push((path, Source::Text(text), Some(saved)));
    }
    for (path, text) in &plan.files {
        writes.push((path.clone(), Source::Text(text.clone()), before(path)?));
    }
    for (path, from) in &plan.copies {
        writes.push((path.clone(), Source::File(from.clone()), before(path)?));
    }
    for (n, (path, source, _)) in writes.iter().enumerate() {
        let written = match source {
            Source::Text(text) => publish(path, text.as_bytes()),
            Source::File(from) => {
                read_bytes(from, COPY_LIMIT, Reject).and_then(|bytes| publish(path, &bytes))
            }
        };
        if let Err(error) = written {
            log::warn!("Import: cannot write {}: {error}", path.display());
            undo(&writes[..n]);
            return Err("raimport.failed".into());
        }
    }
    Ok(plan.settings.len() + plan.files.len() + plan.copies.len())
}

fn publish(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mode = fs::metadata(path).ok().map(|m| m.permissions());
    atomic_publish(path, true, |file| {
        if let Some(mode) = mode {
            file.set_permissions(mode)?;
        }
        file.write_all(bytes)
    })
}

fn undo(done: &[(PathBuf, Source, Before)]) {
    for (path, _, before) in done.iter().rev() {
        let result = match before {
            Some(bytes) => publish(path, bytes),
            None => remove_durable(path),
        };
        if let Err(error) = result {
            log::warn!("Import: cannot restore {}: {error}", path.display());
        }
    }
}

#[cfg(test)]
mod tests;
