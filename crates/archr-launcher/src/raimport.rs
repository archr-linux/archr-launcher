//! Import from RetroArch: a RetroArch setup copied to the card, filtered to
//! what is safe on this device, then applied in one go or not at all.
//!
//! Source (`roms/retroarch-import/`): `retroarch.cfg`, `config/<core>/`
//! (core and game overrides `.cfg`, core options `.opt`), `remaps/<core>/*.rmp`
//! and `shaders/**.slangp|.glslp`. Only allow-listed `retroarch.cfg` keys go
//! into the user's own retroarch.cfg (never system.cfg, so the launcher does
//! not impose them); paths, drivers, the device's video mode, credentials and
//! commands never do. Files are bounded, UTF-8, never symbolic links, and
//! overrides only for installed cores. Shader presets point at this device's
//! shader directory or are left out.
use archr_io::{atomic_publish, read_utf8, remove_durable, Symlinks::Reject};
use archr_ui::settings::raimport::Found;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

const CONFIG_LIMIT: u64 = 1024 * 1024;
const FILE_LIMIT: u64 = 256 * 1024;
const FILES_LIMIT: usize = 200;
const VALUE_LIMIT: usize = 512;
/// Shader trees a preset may have been saved against, on any system.
const SHADER_ROOTS: [&str; 6] = [
    "shaders_slang",
    "shaders_glsl",
    "slang-shaders",
    "glsl-shaders",
    "common-shaders",
    "shaders",
];

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
}

/// What an import would do: the screen's summary and the files to write.
#[derive(Debug, Default)]
pub struct Plan {
    pub found: Found,
    pub settings: Vec<(String, String)>,
    /// Destination and complete contents.
    pub files: Vec<(PathBuf, String)>,
}

/// Why a key or a file stays out, as catalog keys.
const BLOCKED: &str = "raimport.why_blocked";
const UNKNOWN: &str = "raimport.why_unknown";
const INCLUDE: &str = "raimport.why_include";
const BAD_FILE: &str = "raimport.why_file";
const NO_CORE: &str = "raimport.why_core";
const NO_SHADER: &str = "raimport.why_shader";

/// Never imported: where things live, which drivers run, the device's own
/// video mode, accounts, and anything that runs a command.
fn blocked(key: &str) -> bool {
    const EXACT: [&str; 12] = [
        "video_threaded",
        "video_refresh_rate",
        "video_monitor_index",
        "video_swap_interval",
        "video_hard_sync",
        "video_vsync",
        "video_shader",
        "video_filter",
        "input_overlay",
        "audio_device",
        "audio_dsp_plugin",
        "global_core_options",
    ];
    EXACT.contains(&key)
        || ["_directory", "_dir", "_path", "_cmd", "_url"]
            .iter()
            .any(|end| key.ends_with(end))
        || ["driver", "password", "token", "username"]
            .iter()
            .any(|word| key.contains(word))
        || [
            "video_fullscreen",
            "video_window",
            "video_context",
            "cheevos_",
            "netplay_",
            "ai_service",
            "libretro_",
            "core_updater",
            "input_player",
            "input_libretro_device",
            "input_osk_overlay",
        ]
        .iter()
        .any(|start| key.starts_with(start))
}

/// The retroarch.cfg keys worth carrying over (look, hotkeys, overlay
/// placement, latency, rewind, run-ahead, savestates, menu).
fn allowed(key: &str) -> bool {
    const EXACT: [&str; 26] = [
        "video_scale_integer",
        "video_scale_integer_overscale",
        "video_smooth",
        "video_ctx_scaling",
        "video_rotation",
        "aspect_ratio_index",
        "video_aspect_ratio",
        "video_aspect_ratio_auto",
        "video_shader_enable",
        "video_frame_delay",
        "video_frame_delay_auto",
        "video_font_enable",
        "video_font_size",
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
        "config_save_on_exit",
        "input_bind_hold",
    ];
    EXACT.contains(&key)
        || [
            "custom_viewport_",
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

/// RetroArch's hotkey binds (`input_menu_toggle_btn`, `input_exit_emulator`).
fn hotkey(key: &str) -> bool {
    let Some(name) = key.strip_prefix("input_") else {
        return false;
    };
    let name = ["_btn", "_axis", "_mbtn"]
        .iter()
        .find_map(|end| name.strip_suffix(end))
        .unwrap_or(name);
    [
        "enable_hotkey",
        "exit_emulator",
        "menu_toggle",
        "save_state",
        "load_state",
        "state_slot_increase",
        "state_slot_decrease",
        "toggle_fast_forward",
        "hold_fast_forward",
        "rewind",
        "pause_toggle",
        "reset",
        "screenshot",
        "fps_toggle",
        "shader_next",
        "shader_prev",
        "overlay_next",
        "toggle_slowmotion",
        "hold_slowmotion",
        "frame_advance",
        "audio_mute",
        "volume_up",
        "volume_down",
        "hotkey_block_delay",
        "menu_toggle_gamepad_combo",
        "quit_gamepad_combo",
    ]
    .contains(&name)
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
    /// Allowed entries of a RetroArch config, the rest listed as ignored.
    fn filter(
        &mut self,
        text: &str,
        item: &str,
        keep: impl Fn(&str) -> bool,
    ) -> Vec<(String, String)> {
        self.filter_with(text, item, blocked, keep)
    }
    fn filter_with(
        &mut self,
        text: &str,
        item: &str,
        blocked: impl Fn(&str) -> bool,
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

/// Everything the folder holds, sorted into what applies and what does not.
pub fn scan(roots: &Roots) -> Plan {
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
    plan
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
            let kept = plan.filter_with(
                &text,
                &item,
                |key| key.contains("joypad_index"),
                |key| key.starts_with("input_"),
            );
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

/// `shaders/**.slangp|.glslp`, at the same place under the shader directory.
fn scan_shaders(plan: &mut Plan, roots: &Roots) {
    let mut presets = Vec::new();
    let mut pending = vec![(PathBuf::new(), roots.source.join("shaders"))];
    while let Some((relative, dir)) = pending.pop() {
        for (name, path, kind) in entries(&dir) {
            let item = relative.join(&name);
            let shown = format!("shaders/{}", item.display());
            if !plain(&name) || kind.is_symlink() || relative.components().count() >= 4 {
                plan.ignore(shown, BAD_FILE);
            } else if kind.is_dir() {
                pending.push((item, path));
            } else if name.ends_with(".slangp") || name.ends_with(".glslp") {
                presets.push((item, path));
            } else {
                plan.ignore(shown, UNKNOWN);
            }
        }
    }
    presets.sort();
    let imported: BTreeSet<PathBuf> = presets.iter().map(|p| p.0.clone()).collect();
    for (item, path) in presets {
        let shown = format!("shaders/{}", item.display());
        match read(&path, FILE_LIMIT).map(|text| preset(&text, &item, roots, &imported)) {
            Some(Ok(text)) => {
                let to = roots.shaders.join(&item);
                if plan.write(to, shown, "raimport.file_preset".into(), text) {
                    plan.found.presets += 1;
                }
            }
            Some(Err(why)) => plan.ignore(shown, why),
            None => plan.ignore(shown, BAD_FILE),
        }
    }
}

/// A preset with every shader, texture and reference pointing at this
/// device's shader directory, or why it cannot.
fn preset(
    text: &str,
    item: &Path,
    roots: &Roots,
    imported: &BTreeSet<PathBuf>,
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
            let found = locate(target, here, roots, imported).ok_or(NO_SHADER)?;
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
                    let found = locate(&value, here, roots, imported).ok_or(NO_SHADER)?;
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

/// A shader file on this device for a path saved elsewhere: the part after a
/// known shader tree, else the path beside the preset; never outside.
fn locate(
    saved: &str,
    here: &Path,
    roots: &Roots,
    imported: &BTreeSet<PathBuf>,
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
    (fs::metadata(&path).is_ok_and(|m| m.is_file()) || imported.contains(&relative)).then_some(path)
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

/// Writes every file, or puts back what was there when one fails.
pub fn apply(plan: &Plan, roots: &Roots) -> Result<usize, String> {
    let mut writes = plan.files.clone();
    if !plan.settings.is_empty() {
        let path = roots.retroarch.join("retroarch.cfg");
        let current = read(&path, CONFIG_LIMIT).ok_or("raimport.no_retroarch_cfg")?;
        writes.push((path, merged(&current, &plan.settings)));
    }
    let mut done: Vec<(PathBuf, Option<String>)> = Vec::new();
    for (path, text) in &writes {
        let before = match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => read(path, CONFIG_LIMIT.max(FILE_LIMIT)),
            Ok(_) => {
                undo(&done);
                return Err("raimport.failed".into());
            }
            Err(_) => None,
        };
        if let Err(error) = publish(path, text) {
            log::warn!("Import: cannot write {}: {error}", path.display());
            undo(&done);
            return Err("raimport.failed".into());
        }
        done.push((path.clone(), before));
    }
    Ok(plan.settings.len() + plan.files.len())
}

fn publish(path: &Path, text: &str) -> io::Result<()> {
    let mode = fs::metadata(path).ok().map(|m| m.permissions());
    atomic_publish(path, true, |file| {
        if let Some(mode) = mode {
            file.set_permissions(mode)?;
        }
        file.write_all(text.as_bytes())
    })
}

fn undo(done: &[(PathBuf, Option<String>)]) {
    for (path, before) in done.iter().rev() {
        let result = match before {
            Some(text) => publish(path, text),
            None => remove_durable(path),
        };
        if let Err(error) = result {
            log::warn!("Import: cannot restore {}: {error}", path.display());
        }
    }
}

#[cfg(test)]
mod tests;
