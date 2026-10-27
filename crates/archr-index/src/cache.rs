//! Versioned, best-effort index caches. Overview loading never scans ROMs.
//!
//! Both files start with `ALAX` and a little-endian u32 version, followed by a
//! serde/bincode 1.3 payload (fixed-width integers, little endian). System game
//! payloads start with the u64 fingerprint, then the game vector. Reads and
//! decoding are size-limited and reject truncated payloads and trailing bytes.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use bincode::Options;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{scan_system, Game, SystemDef};

const MAGIC: &[u8; 4] = b"ALAX";
const HEADER_BYTES: u64 = 8;
pub const CACHE_VERSION: u32 = 1;
/// Maximum overview file size, including the header (1 MiB).
pub const MAX_OVERVIEW_BYTES: u64 = 1024 * 1024;
/// Maximum per-system cache file size, including the header (32 MiB).
pub const MAX_SYSTEM_CACHE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overview {
    pub systems: Vec<SystemEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemEntry {
    pub name: String,
    pub fullname: String,
    pub game_count: u32,
    /// Maximum modification time in nanoseconds since the Unix epoch across the
    /// system directory, gamelist.xml, and supported media directories. Obtained
    /// with stat calls only; nested ROM directory changes are not detected in v1.
    pub dir_mtime: u64,
}

#[derive(Serialize, Deserialize)]
struct GameCache {
    dir_mtime: u64,
    games: Vec<Game>,
}

/// Load only the small overview. Missing, incompatible, or corrupt caches miss.
pub fn load_overview(cache_dir: &Path) -> Option<Overview> {
    read_cache(&cache_dir.join("overview.bin"), MAX_OVERVIEW_BYTES)
}

/// Scan every system, including empty/missing ones (which receive a zero count).
/// This is background work, not part of the startup path.
pub fn build_overview(systems: &[SystemDef]) -> Overview {
    Overview {
        systems: systems
            .iter()
            .map(|sys| SystemEntry {
                name: sys.name.clone(),
                fullname: sys.fullname.clone(),
                dir_mtime: fingerprint(&sys.path).unwrap_or(0),
                game_count: u32::try_from(scan_system(sys).len()).unwrap_or(u32::MAX),
            })
            .collect(),
    }
}

/// Atomically replace the overview, creating the cache directory if necessary.
pub fn save_overview(cache_dir: &Path, overview: &Overview) -> io::Result<()> {
    write_cache(
        &cache_dir.join("overview.bin"),
        overview,
        MAX_OVERVIEW_BYTES,
    )
}

/// Load cached games when the directory/gamelist/media fingerprint matches.
/// Otherwise scan and best-effort rewrite the cache; write errors never hide ROMs.
/// Changes inside nested ROM subfolders are not detected by the v1 fingerprint.
pub fn load_games(cache_dir: &Path, sys: &SystemDef) -> Vec<Game> {
    let path = cache_dir.join("systems").join(system_filename(&sys.name));
    let dir_mtime = fingerprint(&sys.path);
    if let Some(current) = dir_mtime {
        if let Some(cached) = read_cache::<GameCache>(&path, MAX_SYSTEM_CACHE_BYTES) {
            if cached.dir_mtime == current {
                return cached.games;
            }
        }
    }

    let games = scan_system(sys);
    if let Some(dir_mtime) = dir_mtime {
        let cached = GameCache { dir_mtime, games };
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
    let file = fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length > max_bytes {
        return None;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(length).ok()?);
    // Metadata is only a snapshot: bound the read even if the file grows.
    file.take(max_bytes.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    if u64::try_from(bytes.len()).ok()? > max_bytes {
        return None;
    }
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
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cache path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut writer = io::BufWriter::new(temp.as_file_mut());
        writer.write_all(MAGIC)?;
        writer.write_all(&CACHE_VERSION.to_le_bytes())?;
        codec(max_bytes - HEADER_BYTES)
            .serialize_into(&mut writer, value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writer.flush()?;
    }
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
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
fn fingerprint(system_dir: &Path) -> Option<u64> {
    let root = fs::metadata(system_dir).ok()?;
    if !root.is_dir() {
        return None;
    }
    let mut newest = root.modified().ok()?;
    for relative in [
        "gamelist.xml",
        "media/images",
        "media/covers",
        "media/boxart",
        "media/thumbnails",
        "media/videos",
        "images",
        "videos",
    ] {
        match fs::metadata(system_dir.join(relative)) {
            Ok(metadata) => newest = newest.max(metadata.modified().ok()?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    newest
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()
}
