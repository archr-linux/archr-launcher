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

/// The largest picture worth keeping: what the launcher decodes on the
/// largest screen it has (1024x768), one and a half times over.
pub const MAX_PICTURE: (u32, u32) = (1536, 1152);
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

/// Prepares `out` for the store; the files left and their bytes.
pub fn prepare(
    out: &Path,
    written: &[(String, String)],
    notes: &mut BTreeSet<String>,
) -> Result<(usize, u64), String> {
    let patterns = named(written);
    let mut gone = 0;
    for (rel, path) in files(out) {
        let keep = KEPT.contains(&rel.as_str())
            || READ_BY_NAME.iter().any(|d| rel.starts_with(d))
            || patterns.iter().any(|p| matches(p, &rel));
        if !keep {
            let _ = std::fs::remove_file(&path);
            gone += 1;
        }
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
            && shrink(&path)
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

/// A picture made no larger than `MAX_PICTURE` (its shape kept) and
/// recompressed; kept as it was when that is not smaller. True when it
/// changed. Needs ImageMagick on the PC.
fn shrink(path: &Path) -> bool {
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
    let (w, h) = MAX_PICTURE;
    let mut cmd = Command::new("magick");
    cmd.arg(path)
        .args(["-resize", &format!("{w}x{h}>"), "-strip"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if lower.ends_with(".png") {
        cmd.args(["-define", "png:compression-level=9"]);
    } else {
        cmd.args(["-quality", "86"]);
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
}
