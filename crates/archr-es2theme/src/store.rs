//! A converted theme made ready for the theme store (`--store`): only the
//! files its TOML names, every picture no larger than a 1024x768 screen
//! draws it (the launcher decodes at most one and a half times the screen),
//! recompressed, and its clips re-encoded small or, past the clip budget,
//! left out for the stills the screens already have. A theme that keeps a
//! clip asks for the launcher whose store installs clips (0.6.1).
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use toml::Value;

/// The largest screen the launcher draws (1024x768): no picture needs to
/// be larger than to cover it.
pub const SCREEN: (f32, f32) = (1024., 768.);
/// Room over the box a picture is drawn in: a focused item grows, a
/// picture may be cut to cover.
const MARGIN: f32 = 1.25;
/// The theme's clips together, re-encoded; past it they are left out.
pub const CLIP_BUDGET: u64 = 10 << 20;
/// One clip, as the store keeps it (the launcher's `CLIP_LIMIT`).
pub const CLIP_LIMIT: u64 = 8 << 20;
/// Files every theme keeps, named or not.
const KEPT: &[&str] = &["theme.toml", "CONVERSAO.md", "preview.png"];
/// Folders the launcher reads by itself (`system.logo`, `system.image`...).
const READ_BY_NAME: &[&str] = &[
    "logos/",
    "systems/",
    "videos/",
    "backgrounds/",
    "sounds/",
    "original/",
    "parts/",
    "screenshots/",
];

/// The system names an `es_systems.cfg` lists (`<name>snes</name>`), or a
/// plain list, one per line.
pub fn system_names(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    if text.contains("<name>") {
        for part in text.split("<name>").skip(1) {
            if let Some((name, _)) = part.split_once("</name>") {
                found.insert(name.trim().to_lowercase());
            }
        }
    } else {
        found.extend(
            text.lines()
                .map(|l| l.trim().to_lowercase())
                .filter(|l| !l.is_empty()),
        );
    }
    found
}

/// Prepares `out` for the store; the files left and their bytes.
pub fn prepare(
    out: &Path,
    written: &[(String, String)],
    systems: Option<&BTreeSet<String>>,
    notes: &mut BTreeSet<String>,
) -> Result<(usize, u64), String> {
    let drawn = drawn(written);
    let mut patterns = named(written);
    patterns.extend(drawn.iter().map(|(p, _)| p.clone()));
    let mut gone = 0;
    let mut other = 0;
    for (rel, path) in files(out) {
        let mut keep = KEPT.contains(&rel.as_str())
            || READ_BY_NAME.iter().any(|d| rel.starts_with(d))
            || patterns.iter().any(|p| matches(p, &rel));
        // A picture of a system the console does not run (the theme's own
        // list is ES-DE's) is left out.
        let media = [".png", ".jpg", ".jpeg", ".webp", ".svg", ".mp4"]
            .iter()
            .any(|e| rel.to_lowercase().ends_with(e));
        if let (true, true, Some(systems)) = (keep, media, systems) {
            let names = names_of(&patterns, &rel);
            if !names.is_empty() && !names.iter().any(|n| wanted(systems, n)) {
                keep = false;
                other += 1;
            }
        }
        if !keep {
            let _ = std::fs::remove_file(&path);
            gone += 1;
        }
    }
    if other > 0 {
        notes.insert(format!(
            "Loja: {other} imagens de sistemas que o Arch R não tem ficaram de fora"
        ));
    }
    if gone > 0 {
        notes.insert(format!(
            "Loja: {gone} arquivos que o tema não usa ficaram de fora (escolhas além das 24 de uma opção, variantes não convertidas)"
        ));
    }
    let mut smaller = 0;
    for (rel, path) in files(out) {
        let lower = rel.to_lowercase();
        if [".png", ".jpg", ".jpeg", ".webp"]
            .iter()
            .any(|e| lower.ends_with(e))
            && !rel.starts_with("original/")
            && shrink(&path, cap(&drawn, &rel))
        {
            smaller += 1;
        }
    }
    if smaller > 0 {
        notes.insert(format!(
            "Loja: {smaller} imagens reduzidas ao que uma tela de 1024x768 mostra e recomprimidas"
        ));
    }
    let clips: Vec<(String, PathBuf)> = files(out)
        .into_iter()
        .filter(|(rel, _)| rel.to_lowercase().ends_with(".mp4"))
        .collect();
    for (_, path) in &clips {
        reencode(path);
    }
    let total: u64 = clips.iter().map(|(_, p)| size(p)).sum();
    let too_big = clips.iter().any(|(_, p)| size(p) > CLIP_LIMIT);
    if !clips.is_empty() && (total > CLIP_BUDGET || too_big) {
        for (_, path) in &clips {
            let _ = std::fs::remove_file(path);
        }
        notes.insert(format!(
            "Loja: os {} vídeos do tema ({:.1} MB) passam do orçamento de {} MB e ficaram de fora; as telas mostram a imagem parada de cada um",
            clips.len(),
            total as f64 / 1_048_576.,
            CLIP_BUDGET >> 20
        ));
    } else if !clips.is_empty() {
        min_launcher(out, "0.6.1")?;
        notes.insert(format!(
            "Loja: {} vídeo(s) re-codificado(s) para 854x480 ({:.1} MB); o tema pede o launcher 0.6.1, cuja Loja instala vídeos",
            clips.len(),
            total as f64 / 1_048_576.
        ));
    }
    let left = files(out);
    let bytes = left.iter().map(|(_, p)| size(p)).sum();
    Ok((left.len(), bytes))
}

fn size(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |m| m.len())
}

/// Every file under `dir`, as a path from it and on disk.
pub fn files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(at) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(rel) = path.strip_prefix(dir) {
                found.push((rel.to_string_lossy().replace('\\', "/"), path));
            }
        }
    }
    found.sort();
    found
}

/// The paths the TOML files name: every text value with a file's shape,
/// data (`${system}`) standing for any name.
fn named(written: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (_, text) in written {
        let Ok(table) = toml::from_str::<toml::Table>(text) else {
            continue;
        };
        walk(&Value::Table(table), &mut out);
    }
    out.sort();
    out.dedup();
    out
}
fn walk(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let s = s.trim_start_matches("./");
            if s.contains('.') && !s.contains(' ') && !s.starts_with('#') {
                out.push(s.to_owned());
            }
        }
        Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
        Value::Table(t) => t.values().for_each(|v| walk(v, out)),
        _ => {}
    }
}
/// The system names a per-system file stands for: what `${...}` takes in
/// the patterns that name it, or the file's name in `logos/` and
/// `systems/`. Empty for a file of the theme's own.
fn names_of(patterns: &[String], rel: &str) -> Vec<String> {
    if patterns.iter().any(|p| !p.contains("${") && p == rel) {
        return Vec::new();
    }
    let mut names: Vec<String> = patterns
        .iter()
        // A file pattern: `${...}` inside a path with an extension after it.
        .filter(|p| p.contains("${") && p.rsplit_once('}').is_some_and(|(_, e)| e.starts_with('.')))
        .filter_map(|p| capture(p, rel))
        .collect();
    for dir in ["logos/", "systems/"] {
        if let Some(name) = rel.strip_prefix(dir).filter(|n| !n.contains('/')) {
            names.push(name.rsplit_once('.').map_or(name, |(s, _)| s).to_owned());
        }
    }
    names
}
/// What one `${...}` of `pattern` takes in `rel`, when it matches.
fn capture(pattern: &str, rel: &str) -> Option<String> {
    let start = pattern.find("${")?;
    let end = start + pattern[start..].find('}')? + 1;
    let (before, after) = (&pattern[..start], &pattern[end..]);
    if after.contains("${") || !matches(pattern, rel) {
        return None;
    }
    let middle = rel.strip_prefix(before)?.strip_suffix(after)?;
    Some(middle.to_owned())
}
/// A system the console runs, or a name one of them shares its pictures
/// with, or a collection's or the theme's fallback.
fn wanted(systems: &BTreeSet<String>, name: &str) -> bool {
    let lower = name.to_lowercase();
    systems.contains(&lower)
        || systems
            .iter()
            .any(|s| archr_ui::render::shared_system(s) == Some(lower.as_str()))
        || lower.starts_with('_')
        || lower.starts_with("auto-")
        || [
            "default",
            "fallback",
            "collection",
            "favorites",
            "recent",
            "lastplayed",
            "all",
            "allgames",
            "itchio",
            "modules",
            "emulators",
        ]
        .iter()
        .any(|k| lower.contains(k))
}

/// `rel` is a file `pattern` names (`${...}` stands for one name).
fn matches(pattern: &str, rel: &str) -> bool {
    let mut pieces = Vec::new();
    let mut rest = pattern;
    while let Some(start) = rest.find("${") {
        pieces.push(&rest[..start]);
        rest = rest[start..]
            .find('}')
            .map_or("", |e| &rest[start + e + 1..]);
    }
    pieces.push(rest);
    if pieces.len() == 1 {
        return pattern == rel;
    }
    let mut at = 0;
    for (n, piece) in pieces.iter().enumerate() {
        if n == 0 {
            if !rel.starts_with(piece) {
                return false;
            }
            at = piece.len();
        } else if n == pieces.len() - 1 {
            return rel.len() >= at + piece.len()
                && rel.ends_with(piece)
                && !rel[at..rel.len() - piece.len()].contains('/');
        } else {
            let Some(found) = rel[at..].find(piece) else {
                return false;
            };
            if rel[at..at + found].contains('/') {
                return false;
            }
            at += found + piece.len();
        }
    }
    true
}

/// The size a file must keep: enough to cover the largest box the theme
/// draws it in on a 1024x768 screen (with `MARGIN`), never more than to
/// cover the screen; the screen when a box is not known.
fn cap(drawn: &[(String, Option<(f32, f32)>)], rel: &str) -> (u32, u32) {
    let mut best: Option<(f32, f32)> = None;
    for (pattern, size) in drawn {
        if !matches(pattern, rel) {
            continue;
        }
        match size {
            None => return (SCREEN.0 as u32, SCREEN.1 as u32),
            Some((w, h)) => {
                let (bw, bh) = best.unwrap_or((0., 0.));
                best = Some((bw.max(*w), bh.max(*h)));
            }
        }
    }
    let Some((w, h)) = best else {
        return (SCREEN.0 as u32, SCREEN.1 as u32);
    };
    (
        (w * MARGIN).clamp(16., SCREEN.0).ceil() as u32,
        (h * MARGIN).clamp(16., SCREEN.1).ceil() as u32,
    )
}

/// The pictures the screens' elements draw, as patterns, with the largest
/// box each is drawn in on a 1024x768 screen (None: not known, or a frame,
/// which keeps the screen's size). A variable counts with every value it
/// takes (shapes, choices, systems).
fn drawn(written: &[(String, String)]) -> Vec<(String, Option<(f32, f32)>)> {
    let mut all = toml::Table::new();
    for (_, text) in written {
        if let Ok(t) = toml::from_str::<toml::Table>(text) {
            merge(&mut all, t);
        }
    }
    let mut vars: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    let mut add = |t: Option<&Value>| {
        if let Some(Value::Table(t)) = t {
            for (k, v) in t {
                vars.entry(k.clone()).or_default().push(v.clone());
            }
        }
    };
    add(all.get("vars"));
    for key in ["aspect", "system"] {
        if let Some(Value::Table(t)) = all.get(key) {
            for one in t.values() {
                add(one.get("vars"));
            }
        }
    }
    if let Some(Value::Table(options)) = all.get("options") {
        for o in options.values() {
            if let Some(Value::Table(choices)) = o.get("choices") {
                for c in choices.values() {
                    add(c.get("vars"));
                }
            }
        }
    }
    let values = |v: Option<&Value>| -> Vec<Value> {
        match v {
            Some(Value::String(s)) if s.starts_with('$') && !s.starts_with("${") => {
                vars.get(&s[1..]).cloned().unwrap_or_default()
            }
            Some(Value::Array(a)) => a.clone(),
            Some(v) => vec![v.clone()],
            None => Vec::new(),
        }
    };
    let number = |v: Option<&Value>| -> Option<f32> {
        let vs = values(v);
        if vs.is_empty() {
            return None;
        }
        vs.iter()
            .map(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .collect::<Option<Vec<f64>>>()
            .map(|n| n.into_iter().fold(0., f64::max) as f32)
    };
    let sources = |v: Option<&Value>| -> Vec<String> {
        let mut out = Vec::new();
        for one in values(v) {
            for s in values(Some(&one)) {
                if let Some(s) = s.as_str() {
                    out.extend(data_files(s));
                }
            }
        }
        out
    };
    let mut out = Vec::new();
    let Some(Value::Table(views)) = all.get("view") else {
        return out;
    };
    for view in views.values() {
        for screen in ["systems", "games", "details"] {
            let Some(Value::Array(els)) = view.get(screen).and_then(|s| s.get("el")) else {
                continue;
            };
            for e in els {
                let Some(t) = e.as_table() else { continue };
                let rect = t.get("rect");
                let rw = number(rect.and_then(|r| r.get("w")));
                let rh = number(rect.and_then(|r| r.get("h")));
                let boxed = rw.zip(rh).map(|(w, h)| (w * SCREEN.0, h * SCREEN.1));
                let scale = number(t.get("selected").and_then(|s| s.get("scale")))
                    .unwrap_or(1.)
                    .max(1.);
                let kind = t.get("kind").and_then(Value::as_str).unwrap_or("");
                match kind {
                    "image" | "video" => {
                        let all = sources(t.get("src"))
                            .into_iter()
                            .chain(sources(t.get("fallback")));
                        for s in all {
                            out.push((s, boxed));
                        }
                    }
                    "carousel" | "grid" => {
                        let item = t.get("item");
                        let iw = number(item.and_then(|i| i.get("w")));
                        let ih = number(item.and_then(|i| i.get("h")));
                        let cell = match (iw, ih) {
                            (Some(w), Some(h)) => Some((w * SCREEN.0, h * SCREEN.1)),
                            _ if kind == "grid" => {
                                let cols = number(t.get("columns")).unwrap_or(1.).max(1.);
                                let rows = number(t.get("rows")).unwrap_or(1.).max(1.);
                                boxed.map(|(w, h)| (w / cols, h / rows))
                            }
                            _ => boxed,
                        };
                        let cell = cell.map(|(w, h)| (w * scale, h * scale));
                        for key in ["image", "logo"] {
                            for s in sources(item.and_then(|i| i.get(key))) {
                                out.push((s, cell));
                            }
                        }
                    }
                    "frame" => {
                        for s in sources(t.get("image")) {
                            out.push((s, None));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}
fn merge(base: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}
/// A source's files: a theme path, or the folders the launcher's data
/// reads (`system.logo` is `logos/<system>.*`).
fn data_files(s: &str) -> Vec<String> {
    let folder = match s {
        "system.logo" => "logos",
        "system.image" => "systems",
        s if s.starts_with("game.") || s.starts_with("system.") => return Vec::new(),
        s => return vec![s.trim_start_matches("./").to_owned()],
    };
    ["png", "svg", "webp", "jpg"]
        .iter()
        .map(|e| format!("{folder}/${{system}}.{e}"))
        .collect()
}

/// A picture made no larger than needed to cover `cap` (its shape kept)
/// and recompressed; kept as it was when that is not smaller. True when it
/// changed. Needs ImageMagick on the PC.
fn shrink(path: &Path, cap: (u32, u32)) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    let before = size(path);
    if before < 24 << 10 {
        return false;
    }
    let tmp = path.with_extension(format!(
        "store.{}",
        path.extension()
            .map_or("png".into(), |e| e.to_string_lossy().into_owned())
    ));
    let (w, h) = cap;
    let mut cmd = Command::new("magick");
    cmd.arg(path)
        .args(["-resize", &format!("{w}x{h}^>"), "-strip"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if lower.ends_with(".png") {
        cmd.args(["-define", "png:compression-level=9"]);
    } else {
        cmd.args(["-quality", "78"]);
    }
    let made = cmd.arg(&tmp).status().is_ok_and(|s| s.success());
    if made && size(&tmp) > 0 && size(&tmp) < before {
        std::fs::rename(&tmp, path).is_ok()
    } else {
        let _ = std::fs::remove_file(&tmp);
        false
    }
}

/// A clip re-encoded at most 854x480, no sound (the launcher plays clips
/// mute, at most 640x480), when that is smaller. Needs ffmpeg on the PC.
fn reencode(path: &Path) {
    let before = size(path);
    let tmp = path.with_extension("store.mp4");
    let made = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(path)
        .args([
            "-an",
            "-vf",
            "scale='min(854,iw)':'min(480,ih)':force_original_aspect_ratio=decrease,scale=trunc(iw/2)*2:trunc(ih/2)*2",
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "26",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(&tmp)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if made && size(&tmp) > 0 && size(&tmp) < before {
        let _ = std::fs::rename(&tmp, path);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// `min_launcher` in theme.toml raised to `version`.
fn min_launcher(out: &Path, version: &str) -> Result<(), String> {
    let path = out.join("theme.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let text: String = text
        .lines()
        .map(|l| {
            if l.starts_with("min_launcher = ") {
                format!("min_launcher = \"{version}\"")
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_one_name_per_data() {
        assert!(matches("arts/${system}.jpg", "arts/snes.jpg"));
        assert!(!matches("arts/${system}.jpg", "arts/a/snes.jpg"));
        assert!(matches("a/${x}/b-${y}.png", "a/c/b-d.png"));
        assert!(matches("fonts/A.ttf", "fonts/A.ttf"));
        assert!(!matches("fonts/A.ttf", "fonts/B.ttf"));
    }

    #[test]
    fn pictures_of_systems_the_console_lacks_go() {
        let systems: BTreeSet<String> = ["snes", "megadrive"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let patterns = vec!["art/${system}.jpg".to_owned(), "art/frame.png".to_owned()];
        let keep = |rel: &str| {
            let names = names_of(&patterns, rel);
            names.is_empty() || names.iter().any(|n| wanted(&systems, n))
        };
        assert!(keep("art/snes.jpg"));
        // The Mega Drive shares the Genesis's pictures.
        assert!(keep("art/genesis.jpg"));
        assert!(keep("art/_default.jpg"));
        assert!(keep("art/frame.png"));
        assert!(!keep("art/xbox360.jpg"));
        assert!(!keep("logos/xbox360.svg"));
        // A text with data is no file pattern.
        assert!(names_of(&["${game.name}".to_owned()], "theme.toml").is_empty());
        assert_eq!(
            system_names("<system><name>snes</name></system>"),
            ["snes".to_owned()].into()
        );
    }

    #[test]
    fn a_picture_keeps_what_its_largest_box_needs() {
        let drawn = vec![
            ("cards/${system}.webp".to_owned(), Some((200., 150.))),
            ("cards/${system}.webp".to_owned(), Some((100., 300.))),
            ("bg/${system}.jpg".to_owned(), None),
        ];
        assert_eq!(cap(&drawn, "cards/snes.webp"), (250, 375));
        assert_eq!(cap(&drawn, "bg/snes.jpg"), (1024, 768));
        assert_eq!(cap(&drawn, "other.png"), (1024, 768));
    }
}
