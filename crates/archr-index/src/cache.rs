//! Versioned, best-effort index caches. Overview loading never scans ROMs.
//!
//! Both files start with `ALAX` and a little-endian u32 version, followed by a
//! serde/bincode 1.3 payload (fixed-width integers, little endian). System game
//! payloads start with the u64 fingerprint, then the game vector. Reads and
//! decoding are size-limited and reject truncated payloads and trailing bytes.

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use archr_io::{atomic_publish, read_bytes, Symlinks::Follow};
use bincode::Options;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{scan_system_with_mapper, Game, SystemDef};

const MAGIC: &[u8; 4] = b"ALAX";
const HEADER_BYTES: u64 = 8;
pub const CACHE_VERSION: u32 = 4;
/// Maximum overview file size, including the header (1 MiB).
pub const MAX_OVERVIEW_BYTES: u64 = 1024 * 1024;
/// Maximum per-system cache file size, including the header (32 MiB).
pub const MAX_SYSTEM_CACHE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overview {
    pub systems: Vec<SystemEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemEntry {
    pub name: String,
    pub fullname: String,
    pub game_count: u32,
    /// Latest source modification time in nanoseconds, from stat calls only.
    pub dir_mtime: u64,
    /// Also covers the system path and extensions, without changing dir_mtime.
    pub source_fingerprint: u64,
}

#[derive(Serialize, Deserialize)]
struct GameCache {
    source_fingerprint: u64,
    games: Vec<Game>,
}

/// Load only the small overview. Missing, incompatible, or corrupt caches miss.
pub fn load_overview(cache_dir: &Path) -> Option<Overview> {
    read_cache(&cache_dir.join("overview.bin"), MAX_OVERVIEW_BYTES)
}

/// Scan every system, including empty/missing ones (which receive a zero count).
/// This is background work, not part of the startup path.
pub fn build_overview(systems: &[SystemDef]) -> Overview {
    refresh_overview(&Overview::default(), systems).0
}

/// Stat unchanged systems without scanning. Definition/path/extension changes,
/// added/removed systems and changed directory/media mtimes invalidate entries.
pub fn refresh_overview(old: &Overview, systems: &[SystemDef]) -> (Overview, bool) {
    refresh_overview_with_mapper(old, systems, &Path::to_owned)
}
pub fn refresh_overview_with_mapper(
    old: &Overview,
    systems: &[SystemDef],
    mapper: &dyn Fn(&Path) -> PathBuf,
) -> (Overview, bool) {
    let overview = Overview {
        systems: systems
            .iter()
            .map(|sys| {
                let previous = old.systems.iter().find(|entry| entry.name == sys.name);
                let current = fingerprint(sys);
                let count = previous
                    .filter(|entry| current.map(|(_, hash)| hash) == Some(entry.source_fingerprint))
                    .map(|entry| entry.game_count)
                    .unwrap_or_else(|| {
                        u32::try_from(scan_system_with_mapper(sys, mapper).len())
                            .unwrap_or(u32::MAX)
                    });
                SystemEntry {
                    name: sys.name.clone(),
                    fullname: sys.fullname.clone(),
                    game_count: count,
                    dir_mtime: current.map_or(0, |(newest, _)| newest),
                    source_fingerprint: current.map_or(0, |(_, hash)| hash),
                }
            })
            .collect(),
    };
    let changed = overview != *old;
    (overview, changed)
}

/// Atomically replace the overview, creating the cache directory if necessary.
pub fn save_overview(cache_dir: &Path, overview: &Overview) -> io::Result<()> {
    let path = cache_dir.join("overview.bin");
    write_cache(&path, overview, MAX_OVERVIEW_BYTES)
}

/// Load cached games when the directory/gamelist/media fingerprint matches.
/// Otherwise scan and best-effort rewrite the cache; write errors never hide ROMs.
/// Changes confined to nested ROM subfolders still need a parent/gamelist update.
pub fn load_games(cache_dir: &Path, sys: &SystemDef) -> Vec<Game> {
    load_games_with_mapper(cache_dir, sys, &Path::to_owned)
}
pub fn load_games_with_mapper(
    cache_dir: &Path,
    sys: &SystemDef,
    mapper: &dyn Fn(&Path) -> PathBuf,
) -> Vec<Game> {
    let path = cache_dir.join("systems").join(system_filename(&sys.name));
    let source_fingerprint = fingerprint(sys).map(|(_, hash)| hash);
    if let Some(current) = source_fingerprint {
        if let Some(cached) = read_cache::<GameCache>(&path, MAX_SYSTEM_CACHE_BYTES) {
            if cached.source_fingerprint == current {
                return cached.games;
            }
        }
    }

    let games = scan_system_with_mapper(sys, mapper);
    if let Some(source_fingerprint) = source_fingerprint {
        let cached = GameCache {
            source_fingerprint,
            games,
        };
        // A read-only/full/unavailable cache is not an indexing failure.
        let _ = write_cache(&path, &cached, MAX_SYSTEM_CACHE_BYTES);
        cached.games
    } else {
        games
    }
}

fn codec(limit: u64) -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .reject_trailing_bytes()
        .with_limit(limit)
}

fn read_cache<T: DeserializeOwned>(path: &Path, max_bytes: u64) -> Option<T> {
    let bytes = read_bytes(path, max_bytes, Follow).ok()?;
    if bytes.get(..4)? != MAGIC || bytes.get(4..8)? != CACHE_VERSION.to_le_bytes() {
        return None;
    }
    let payload = bytes.get(8..)?;
    let mut reader = io::Cursor::new(payload);
    // bincode 1.3 ignores limits for slice deserialization. The reader API
    // enforces them before allocating strings; it needs an explicit EOF check.
    let value = codec(max_bytes.checked_sub(HEADER_BYTES)?)
        .deserialize_from(&mut reader)
        .ok()?;
    (reader.position() == u64::try_from(payload.len()).ok()?).then_some(value)
}

fn write_cache<T: Serialize>(path: &Path, value: &T, max_bytes: u64) -> io::Result<()> {
    atomic_publish(path, false, |file| {
        let mut writer = io::BufWriter::new(file);
        writer.write_all(MAGIC)?;
        writer.write_all(&CACHE_VERSION.to_le_bytes())?;
        codec(max_bytes - HEADER_BYTES)
            .serialize_into(&mut writer, value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writer.flush()
    })
}

/// Encode every unsafe UTF-8 byte, including `%`, so sanitization cannot cause
/// name collisions or produce traversal/absolute paths. Plain names stay plain.
fn system_filename(name: &str) -> PathBuf {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut filename = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') {
            filename.push(char::from(byte));
        } else {
            filename.push('%');
            filename.push(char::from(HEX[usize::from(byte >> 4)]));
            filename.push(char::from(HEX[usize::from(byte & 0xf)]));
        }
    }
    if filename.is_empty() {
        filename.push('%');
    }
    filename.push_str(".bin");
    PathBuf::from(filename)
}

/// A fixed number of stat calls, never directory listings. Missing optional
/// paths are ignored; other metadata errors disable cache reuse rather than
/// turning an unreadable fingerprint into an apparently valid one.
fn fingerprint(sys: &SystemDef) -> Option<(u64, u64)> {
    if !fs::metadata(&sys.path).ok()?.is_dir() {
        return None;
    }
    let mut hash = DefaultHasher::new();
    sys.path.hash(&mut hash);
    sys.extensions.hash(&mut hash);
    let mut newest = 0;
    for relative in [
        "",
        "gamelist.xml",
        "media/images",
        "media/covers",
        "media/boxart",
        "media/thumbnails",
        "media/videos",
        "images",
        "videos",
    ] {
        let stamp = match fs::metadata(sys.path.join(relative)) {
            Ok(metadata) => {
                let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
                let nanos = u64::try_from(modified.as_nanos()).ok()?;
                newest = newest.max(nanos);
                Some((nanos, metadata.is_file().then_some(metadata.len())))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && !relative.is_empty() => None,
            Err(_) => return None,
        };
        stamp.hash(&mut hash);
    }
    Some((newest, hash.finish()))
}
