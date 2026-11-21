//! Settings › Launcher, the host's side: what the page reads (counts, the
//! cover cache, the version, the log) and the file work of its actions. The
//! library's own changes (resync, prune, recents, restore) live in Library.
use crate::{library::Library, paths::Paths};
use archr_ui::settings::launcher::Facts;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// The session's log: the launcher's messages and its children's.
pub const LOG: &str = "/var/log/archr-launcher-session.log";
/// The end of the log read for the viewer, and its lines at most.
const LOG_TAIL: u64 = 64 * 1024;
const LOG_LINES: usize = 400;

pub fn facts(library: &Library, paths: &Paths) -> Facts {
    Facts {
        games: library.game_total(),
        missing: library.missing_games().len(),
        cover_bytes: dir_bytes(&paths.cache.join("thumbs")),
        hidden: library.hidden_count(),
        recent: library.recent_count(),
        synced: fs::metadata(paths.cache.join(archr_index::cache::OVERVIEW_FILE))
            .and_then(|meta| meta.modified())
            .ok(),
        version: version(paths),
    }
}

/// The thumbnails' folder emptied; the lists make them again as they show.
pub fn clear_covers(paths: &Paths) {
    let thumbs = paths.cache.join("thumbs");
    if let Err(error) = fs::remove_dir_all(&thumbs) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Cannot clear {}: {error}", thumbs.display());
        }
    }
}

/// The last lines of the session's log, newest last.
pub fn read_log(paths: &Paths) -> Vec<String> {
    let Ok(mut file) = fs::File::open(paths.at(LOG)) else {
        return Vec::new();
    };
    let len = file.metadata().map_or(0, |meta| meta.len());
    let start = len.saturating_sub(LOG_TAIL);
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err()
        || file.take(LOG_TAIL).read_to_end(&mut bytes).is_err()
    {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.lines().collect();
    // Cut mid-file: the first line is a piece of one.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let skip = lines.len().saturating_sub(LOG_LINES);
    lines[skip..].iter().map(|line| clean(line)).collect()
}

/// No control characters reach the screen; tabs become spaces.
fn clean(line: &str) -> String {
    line.chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect()
}

/// The installed package's short version: the commit of
/// `archr-launcher-1:0.git20261120T192915.1241a58-1`, else the crate's.
pub fn version(paths: &Paths) -> String {
    fs::read_dir(paths.at("/var/lib/pacman/local"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .find_map(|name| short_version(&name))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned())
}

fn short_version(entry: &str) -> Option<String> {
    let rest = entry.strip_prefix("archr-launcher-")?;
    // archr-launcher-<epoch:>pkgver-pkgrel: a digit, not another package.
    if !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let pkgver = rest.rsplit_once('-')?.0;
    let pkgver = pkgver.split_once(':').map_or(pkgver, |(_, v)| v);
    Some(match pkgver.rsplit_once('.') {
        Some((head, commit)) if head.contains("git") => commit.to_owned(),
        _ => pkgver.to_owned(),
    })
}

fn dir_bytes(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |meta| meta.len()),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_the_packaged_commit() {
        assert_eq!(
            short_version("archr-launcher-1:0.git20261120T192915.1241a58-1").as_deref(),
            Some("1241a58")
        );
        assert_eq!(
            short_version("archr-launcher-0.4.0-2").as_deref(),
            Some("0.4.0")
        );
        assert_eq!(short_version("archr-launcher-themes-1.0-1"), None);
        assert_eq!(short_version("archr-1.0-1"), None);
    }

    #[test]
    fn the_log_tail_keeps_whole_lines_newest_last() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_path_buf()));
        let log = paths.at(LOG);
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        let text: String = (0..5000).map(|n| format!("line {n}\tok\x07\n")).collect();
        fs::write(&log, text).unwrap();
        let lines = read_log(&paths);
        assert_eq!(lines.len(), LOG_LINES);
        assert_eq!(lines.last().unwrap(), "line 4999 ok");
        assert!(lines.iter().all(|line| line.starts_with("line ")));
        assert!(read_log(&Paths::new(Some(root.path().join("none")))).is_empty());
    }

    #[test]
    fn the_cover_cache_is_measured_and_cleared() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_path_buf()));
        let apps = paths.cache.join("thumbs/apps");
        fs::create_dir_all(&apps).unwrap();
        fs::write(paths.cache.join("thumbs/A.png"), [0; 100]).unwrap();
        fs::write(apps.join("B.png"), [0; 50]).unwrap();
        assert_eq!(dir_bytes(&paths.cache.join("thumbs")), 150);
        clear_covers(&paths);
        assert_eq!(dir_bytes(&paths.cache.join("thumbs")), 0);
        clear_covers(&paths);
    }
}
