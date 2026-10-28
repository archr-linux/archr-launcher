//! CRC32 + MD5 of a ROM in one bounded streaming pass.
use md5::{Digest as _, Md5};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

pub struct Digest {
    pub crc32: String,
    pub md5: String,
    pub size: u64,
}

/// Hex digests of the whole file; `Interrupted` if `cancel` is raised.
pub fn digest(path: &Path, cancel: &AtomicBool) -> io::Result<Digest> {
    let mut file = File::open(path)?;
    let (mut crc, mut md5, mut size) = (crc32fast::Hasher::new(), Md5::new(), 0);
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        if cancel.load(Relaxed) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        crc.update(&chunk[..n]);
        md5.update(&chunk[..n]);
        size += n as u64;
    }
    Ok(Digest {
        crc32: format!("{:08x}", crc.finalize()),
        md5: md5.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        size,
    })
}
