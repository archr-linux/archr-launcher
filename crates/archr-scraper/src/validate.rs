//! Structural checks for downloaded media, without decoding it: complete
//! stream, sane dimensions. The service answers errors as text with status 200.

const MAX_SIDE: u32 = 8192;
const MAX_PIXELS: u64 = 16 << 20;

fn sane(width: u32, height: u32) -> bool {
    (1..=MAX_SIDE).contains(&width)
        && (1..=MAX_SIDE).contains(&height)
        && u64::from(width) * u64::from(height) <= MAX_PIXELS
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// File extension when `bytes` is a complete PNG or JPEG.
pub fn image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let ok = bytes.len() > 33
            && &bytes[12..16] == b"IHDR"
            && sane(be32(&bytes[16..]), be32(&bytes[20..]))
            && bytes.ends_with(b"\0\0\0\0IEND\xAEB`\x82");
        return ok.then_some("png");
    }
    jpeg(bytes).then_some("jpg")
}

fn jpeg(bytes: &[u8]) -> bool {
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |at| at + 1);
    if !bytes.starts_with(&[0xFF, 0xD8]) || !bytes[..end].ends_with(&[0xFF, 0xD9]) {
        return false;
    }
    let mut at = 2;
    while at + 4 <= end {
        if bytes[at] != 0xFF {
            return false;
        }
        let marker = bytes[at + 1];
        let length = usize::from(bytes[at + 2]) << 8 | usize::from(bytes[at + 3]);
        match marker {
            0xFF => at += 1,
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                let frame = &bytes[at + 4..end.min(at + 2 + length)];
                let (height, width) = match frame {
                    [_, h1, h0, w1, w0, ..] => (
                        u32::from(*h1) << 8 | u32::from(*h0),
                        u32::from(*w1) << 8 | u32::from(*w0),
                    ),
                    _ => return false,
                };
                return sane(width, height);
            }
            0xDA | 0xD9 => return false, // scan before any frame header
            _ if length < 2 => return false,
            _ => at += 2 + length,
        }
    }
    false
}

/// True for an MP4 whose top-level boxes tile the file exactly and include `moov`.
pub fn video(bytes: &[u8]) -> bool {
    let (mut at, mut moov, mut ftyp) = (0usize, false, false);
    while at + 8 <= bytes.len() {
        let size = match be32(&bytes[at..]) {
            0 => bytes.len() - at,
            1 => return false,
            n => n as usize,
        };
        if size < 8 || at + size > bytes.len() {
            return false;
        }
        match &bytes[at + 4..at + 8] {
            b"ftyp" => ftyp = true,
            b"moov" => moov = true,
            _ => {}
        }
        at += size;
    }
    ftyp && moov && at == bytes.len()
}
