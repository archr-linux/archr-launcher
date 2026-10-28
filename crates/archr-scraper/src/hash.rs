//! CRC32 + MD5 of a ROM in one bounded streaming pass.
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
        md5: md5.finish().iter().map(|b| format!("{b:02x}")).collect(),
        size,
    })
}

/// RFC 1321. Only used to identify ROMs; not a security primitive.
pub struct Md5 {
    state: [u32; 4],
    block: [u8; 64],
    fill: usize,
    length: u64,
}

const SHIFT: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];

impl Md5 {
    pub fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            block: [0; 64],
            fill: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length += data.len() as u64;
        while !data.is_empty() {
            let take = data.len().min(64 - self.fill);
            self.block[self.fill..self.fill + take].copy_from_slice(&data[..take]);
            self.fill += take;
            data = &data[take..];
            if self.fill == 64 {
                self.compress();
                self.fill = 0;
            }
        }
    }

    pub fn finish(mut self) -> [u8; 16] {
        let bits = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.fill != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_le_bytes());
        let mut out = [0; 16];
        for (bytes, word) in out.chunks_mut(4).zip(self.state) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        out
    }

    fn compress(&mut self) {
        let words: Vec<u32> = self
            .block
            .chunks(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let [mut a, mut b, mut c, mut d] = self.state;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let k = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32;
            let rotated = a
                .wrapping_add(f)
                .wrapping_add(k)
                .wrapping_add(words[g])
                .rotate_left(SHIFT[(i / 16) * 4 + i % 4]);
            (a, d, c) = (d, c, b);
            b = b.wrapping_add(rotated);
        }
        for (s, v) in self.state.iter_mut().zip([a, b, c, d]) {
            *s = s.wrapping_add(v);
        }
    }
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}
