//! The game hash RetroAchievements identifies a ROM by, following rcheevos
//! for the systems people play on this device: cartridge systems (bare or in
//! a one-file zip), arcade by name and PlayStation from a cue sheet or ISO.
//! CHD, PBP, 7z and the CD systems besides PlayStation are not hashed.
use archr_io::{read_bytes, Symlinks::Follow};
use md5::{Digest, Md5};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// rcheevos hashes at most this much of any file.
const LIMIT: u64 = 64 << 20;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Whole,
    Nes,
    Snes,
    Pce,
    Atari7800,
    Lynx,
    N64,
    Arcade,
    Psx,
}

fn method(system: &str) -> Option<Method> {
    use Method::*;
    Some(match system {
        "nes" | "nesh" | "famicom" | "fds" => Nes,
        "snes" | "snesh" | "sfc" => Snes,
        "pcengine" | "tg16" | "supergrafx" => Pce,
        "atari7800" => Atari7800,
        "atarilynx" | "lynx" => Lynx,
        "n64" => N64,
        "arcade" | "mame" | "fbn" | "fbneo" | "cps1" | "cps2" | "cps3" | "neogeo" => Arcade,
        "psx" => Psx,
        "gb" | "gbh" | "gbc" | "gbch" | "gba" | "gbah" | "genesis" | "genh" | "megadrive"
        | "megadriveh" | "megadrive-japan" | "mastersystem" | "gamegear" | "ggh" | "sg-1000"
        | "sega32x" | "atari2600" | "colecovision" | "intellivision" | "megaduck" | "ngp"
        | "ngpc" | "pokemini" | "vectrex" | "virtualboy" | "wonderswan" | "wonderswancolor" => {
            Whole
        }
        _ => return None,
    })
}

/// The launcher can identify games of this system.
pub fn supported(system: &str) -> bool {
    method(system).is_some()
}

fn hex(md5: Md5) -> String {
    md5.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn md5(bytes: &[u8]) -> String {
    let mut md5 = Md5::new();
    md5.update(bytes);
    hex(md5)
}

/// Lowercase hex MD5 of `path` as RetroAchievements knows it, or `None`
/// when this system or file format is not hashed here.
pub fn rom_hash(system: &str, path: &Path) -> io::Result<Option<String>> {
    let Some(method) = method(system) else {
        return Ok(None);
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match method {
        Method::Arcade => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            return Ok((!stem.is_empty()).then(|| md5(stem.as_bytes())));
        }
        Method::Psx => return psx(path, &ext, true),
        _ => {}
    }
    let data = match ext.as_str() {
        "zip" => unzip_first(path)?,
        "7z" | "rar" | "chd" | "cue" | "m3u" => return Ok(None),
        _ => read_bytes(path, LIMIT, Follow)?,
    };
    Ok(Some(cartridge(method, data)))
}

fn cartridge(method: Method, mut data: Vec<u8>) -> String {
    let starts = |prefix: &[u8], at: usize| data.get(at..at + prefix.len()) == Some(prefix);
    let skip = match method {
        Method::Nes if starts(b"NES\x1a", 0) || starts(b"FDS\x1a", 0) => 16,
        Method::Snes if data.len() % 0x2000 == 512 => 512,
        Method::Pce if data.len() & 512 != 0 => 512,
        Method::Atari7800 if starts(b"ATARI7800", 1) => 128,
        Method::Lynx if starts(b"LYNX\0", 0) => 64,
        _ => 0,
    };
    if method == Method::N64 {
        // rcheevos hashes the big-endian (z64) byte order.
        match data.first() {
            Some(0x37) => data.chunks_exact_mut(2).for_each(|w| w.swap(0, 1)),
            Some(0x40) => data.chunks_exact_mut(4).for_each(|w| w.reverse()),
            _ => {}
        }
    }
    md5(data.get(skip..).unwrap_or_default())
}

fn le16(bytes: &[u8], at: usize) -> usize {
    bytes
        .get(at..at + 2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize)
}
fn le32(bytes: &[u8], at: usize) -> u64 {
    bytes.get(at..at + 4).map_or(0, |b| {
        u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    })
}

fn bad(message: &str) -> io::Error {
    archr_io::invalid_data(message)
}

/// The first file of a zip, as RetroArch hands a one-ROM archive to the core.
/// Stored and deflated entries only; nothing past `LIMIT` is inflated.
fn unzip_first(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let tail_len = len.min(22 + 0xFFFF);
    file.seek(SeekFrom::Start(len - tail_len))?;
    let mut tail = Vec::new();
    file.by_ref().take(tail_len).read_to_end(&mut tail)?;
    let end = tail
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or_else(|| bad("not a zip"))?;
    let (count, size, offset) = (
        le16(&tail, end + 10),
        le32(&tail, end + 12),
        le32(&tail, end + 16),
    );
    if size > 1 << 20 {
        return Err(bad("zip directory too large"));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut directory = Vec::new();
    file.by_ref().take(size).read_to_end(&mut directory)?;
    let mut at = 0;
    for _ in 0..count {
        if directory.get(at..at + 4) != Some(b"PK\x01\x02") {
            break;
        }
        let name_len = le16(&directory, at + 28);
        let entry_len = 46 + name_len + le16(&directory, at + 30) + le16(&directory, at + 32);
        let is_dir = directory
            .get(at + 46..at + 46 + name_len)
            .is_some_and(|n| n.ends_with(b"/"));
        if !is_dir {
            let (method, packed, plain, local) = (
                le16(&directory, at + 10),
                le32(&directory, at + 20),
                le32(&directory, at + 24),
                le32(&directory, at + 42),
            );
            if packed > LIMIT || plain > LIMIT {
                return Err(bad("zip entry too large"));
            }
            let mut header = [0u8; 30];
            file.seek(SeekFrom::Start(local))?;
            file.read_exact(&mut header)?;
            if &header[..4] != b"PK\x03\x04" {
                return Err(bad("bad zip entry"));
            }
            let skip = (le16(&header, 26) + le16(&header, 28)) as i64;
            file.seek(SeekFrom::Current(skip))?;
            let packed_data = file.take(packed);
            let mut out = Vec::new();
            match method {
                0 => packed_data.take(plain).read_to_end(&mut out)?,
                8 => flate2::read::DeflateDecoder::new(packed_data)
                    .take(plain)
                    .read_to_end(&mut out)?,
                _ => return Err(bad("unsupported zip compression")),
            };
            if out.len() as u64 != plain {
                return Err(bad("truncated zip entry"));
            }
            return Ok(out);
        }
        at += entry_len;
    }
    Err(bad("empty zip"))
}

/// User data of a disc image: 2048-byte sectors, raw (2352) ones included.
struct Disc {
    file: File,
    sector: u64,
    offset: u64,
}

impl Disc {
    fn read(&mut self, lba: u64) -> io::Result<[u8; 2048]> {
        let mut data = [0u8; 2048];
        self.file
            .seek(SeekFrom::Start(lba * self.sector + self.offset))?;
        self.file.read_exact(&mut data)?;
        Ok(data)
    }

    /// Extent and size of a file or directory, by `\`-separated ISO 9660
    /// path; names compare without case and without the `;1` version.
    fn find(&mut self, path: &str) -> io::Result<Option<(u64, u64)>> {
        let root = self.read(16)?;
        let (mut extent, mut size) = (le32(&root, 156 + 2), le32(&root, 156 + 10));
        for name in path.split('\\').filter(|n| !n.is_empty()) {
            let mut found = None;
            'sectors: for lba in extent..extent + size.div_ceil(2048).min(64) {
                let sector = self.read(lba)?;
                let mut at = 0;
                while at < 2048 && sector[at] != 0 {
                    let record = &sector[at..(at + sector[at] as usize).min(2048)];
                    let name_len = record.get(32).copied().unwrap_or(0) as usize;
                    let entry = record.get(33..33 + name_len).unwrap_or_default();
                    let entry = entry.split(|&b| b == b';').next().unwrap_or_default();
                    if entry.eq_ignore_ascii_case(name.as_bytes()) {
                        found = Some((le32(record, 2), le32(record, 10)));
                        break 'sectors;
                    }
                    at += sector[at] as usize;
                }
            }
            let Some(next) = found else {
                return Ok(None);
            };
            (extent, size) = next;
        }
        Ok(Some((extent, size)))
    }
}

/// The disc image a cue sheet or playlist points at, with its sector layout.
fn disc(path: &Path, ext: &str, playlist: bool) -> io::Result<Option<(PathBuf, u64, u64)>> {
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok(match ext {
        "iso" => Some((path.to_owned(), 2048, 0)),
        "cue" => {
            let sheet = archr_io::read_utf8(path, 64 << 10, Follow)?;
            let mut bin = None;
            let mut layout = None;
            for line in sheet.lines().map(str::trim) {
                let upper = line.to_ascii_uppercase();
                if bin.is_none() && upper.starts_with("FILE ") {
                    let name = line[5..].trim();
                    let name = match name.strip_prefix('"') {
                        Some(quoted) => quoted.split('"').next().unwrap_or(""),
                        None => name.split_whitespace().next().unwrap_or(""),
                    };
                    bin = Some(dir.join(name));
                } else if layout.is_none() && upper.starts_with("TRACK ") {
                    layout = Some(if upper.contains("MODE2/2352") {
                        (2352, 24)
                    } else if upper.contains("MODE1/2352") {
                        (2352, 16)
                    } else {
                        (2048, 0)
                    });
                }
            }
            bin.zip(layout)
                .map(|(bin, (sector, offset))| (bin, sector, offset))
        }
        "m3u" if playlist => {
            let list = archr_io::read_utf8(path, 64 << 10, Follow)?;
            let Some(first) = list
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
            else {
                return Ok(None);
            };
            let first = dir.join(first);
            let ext = first
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            disc(&first, &ext, false)?
        }
        _ => None,
    })
}

/// rcheevos `rc_hash_psx`: the boot executable named by SYSTEM.CNF (or
/// PSX.EXE), hashed as its name followed by its contents.
fn psx(path: &Path, ext: &str, playlist: bool) -> io::Result<Option<String>> {
    let Some((bin, sector, offset)) = disc(path, ext, playlist)? else {
        return Ok(None);
    };
    let mut disc = Disc {
        file: File::open(bin)?,
        sector,
        offset,
    };
    let mut name = String::new();
    if let Some((extent, _)) = disc.find("SYSTEM.CNF")? {
        let config = disc.read(extent)?;
        let config = String::from_utf8_lossy(&config);
        for line in config.lines() {
            let Some(rest) = line.trim_start().strip_prefix("BOOT") else {
                continue;
            };
            let Some(value) = rest.trim_start().strip_prefix('=') else {
                continue;
            };
            let value = value.trim_start();
            let value = value.strip_prefix("cdrom:").unwrap_or(value);
            name = value
                .trim_start_matches('\\')
                .split(|c: char| c.is_whitespace() || c == ';')
                .next()
                .unwrap_or("")
                .chars()
                .take(63)
                .collect();
            break;
        }
    }
    let mut found = Some(&name)
        .filter(|n| !n.is_empty())
        .map(|n| disc.find(n))
        .transpose()?
        .flatten();
    if found.is_none() {
        name = "PSX.EXE".into();
        found = disc.find(&name)?;
    }
    let Some((extent, listed)) = found else {
        return Ok(None);
    };
    let first = disc.read(extent)?;
    let size = if first.starts_with(b"PS-X EX") {
        le32(&first, 28) + 2048
    } else {
        listed
    }
    .min(LIMIT);
    let mut md5 = Md5::new();
    md5.update(name.as_bytes());
    let mut left = size;
    let mut lba = extent;
    while left > 0 {
        // A short image ends the file, as rcheevos stops at a short read.
        let data = if lba == extent {
            first
        } else {
            match disc.read(lba) {
                Ok(data) => data,
                Err(_) => break,
            }
        };
        let take = left.min(2048) as usize;
        md5.update(&data[..take]);
        left -= take as u64;
        lba += 1;
    }
    Ok(Some(hex(md5)))
}
