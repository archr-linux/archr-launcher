//! The console's framebuffer (/dev/fb0) as the screen of a frame drawn by
//! SDL's software renderer: no GL, no Mesa. The maintenance boot has no GPU
//! module, and Mesa's software GL spends ~90 ms of CPU a frame on an RK3326.
//!
//! The console is put in graphics mode so its text does not draw over the
//! frame, and back in text mode on drop; after a crash the unit's TTYReset
//! does it, and the console shows the migration's log again.
use std::{fs::File, os::fd::AsRawFd};

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const KDSETMODE: libc::c_ulong = 0x4B3A;
const KD_TEXT: libc::c_ulong = 0;
const KD_GRAPHICS: libc::c_ulong = 1;

pub struct Framebuffer {
    console: File,
    map: *mut u8,
    len: usize,
    /// Bytes from one row to the next, and where the shown page starts.
    stride: usize,
    origin: usize,
    width: usize,
    height: usize,
    /// Clockwise quarter turns from the drawn frame to the panel (fbcon's).
    turns: u8,
}
impl Framebuffer {
    pub fn open() -> Result<Self, String> {
        let fail = |what: &str, e: std::io::Error| format!("{what}: {e}");
        let fb = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/fb0")
            .map_err(|e| fail("/dev/fb0", e))?;
        // fb_var_screeninfo: xres, yres, xres_virtual, yres_virtual,
        // xoffset, yoffset, bits_per_pixel, then fields not needed here.
        let mut var = [0u32; 40];
        // SAFETY: the kernel fills 160 bytes, the size of `var`.
        if unsafe { libc::ioctl(fb.as_raw_fd(), FBIOGET_VSCREENINFO, var.as_mut_ptr()) } != 0 {
            return Err(fail("FBIOGET_VSCREENINFO", std::io::Error::last_os_error()));
        }
        let [width, height, _, rows, x0, y0, bpp] = [0, 1, 2, 3, 4, 5, 6].map(|i| var[i] as usize);
        if bpp != 32 {
            return Err(format!("/dev/fb0: {bpp} bits per pixel"));
        }
        let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
        let stride = read("/sys/class/graphics/fb0/stride")
            .trim()
            .parse()
            .unwrap_or(width * 4);
        let turns = read("/sys/class/graphics/fbcon/rotate")
            .trim()
            .parse::<u8>()
            .unwrap_or(0)
            % 4;
        let len = stride * rows.max(height);
        // SAFETY: maps the framebuffer's memory; unmapped in Drop.
        let map = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fb.as_raw_fd(),
                0,
            )
        };
        if map == libc::MAP_FAILED {
            return Err(fail("mmap /dev/fb0", std::io::Error::last_os_error()));
        }
        let console = File::open("/dev/tty0").map_err(|e| fail("/dev/tty0", e))?;
        // SAFETY: KDSETMODE takes its value by argument.
        unsafe { libc::ioctl(console.as_raw_fd(), KDSETMODE, KD_GRAPHICS) };
        Ok(Self {
            console,
            map: map.cast(),
            len,
            stride,
            origin: y0 * stride + x0 * 4,
            width,
            height,
            turns,
        })
    }
    /// The size to draw at: the panel's, turned like the console.
    pub fn size(&self) -> (u32, u32) {
        let (w, h) = (self.width as u32, self.height as u32);
        if self.turns % 2 == 1 {
            (h, w)
        } else {
            (w, h)
        }
    }
    /// Shows a frame of `size()`, packed XRGB8888 rows `pitch` bytes apart.
    pub fn show(&mut self, pixels: &[u8], pitch: usize) {
        // SAFETY: the mapping is `len` bytes and lives as long as self.
        let out = unsafe { std::slice::from_raw_parts_mut(self.map, self.len) };
        blit(
            out,
            (self.origin, self.stride, self.turns),
            pixels,
            pitch,
            self.size(),
        );
    }
}
impl Drop for Framebuffer {
    fn drop(&mut self) {
        // SAFETY: as in open; then unmaps exactly what open mapped.
        unsafe {
            libc::ioctl(self.console.as_raw_fd(), KDSETMODE, KD_TEXT);
            libc::munmap(self.map.cast(), self.len);
        }
    }
}

/// Copies a `w`×`h` frame into `out`, turned clockwise `turns` times.
fn blit(
    out: &mut [u8],
    (origin, stride, turns): (usize, usize, u8),
    pixels: &[u8],
    pitch: usize,
    (w, h): (u32, u32),
) {
    let (w, h) = (w as usize, h as usize);
    for y in 0..h {
        let row = &pixels[y * pitch..y * pitch + w * 4];
        if turns == 0 {
            let at = origin + y * stride;
            out[at..at + w * 4].copy_from_slice(row);
            continue;
        }
        for (x, px) in row.chunks_exact(4).enumerate() {
            let (to_x, to_y) = match turns {
                1 => (h - 1 - y, x),
                2 => (w - 1 - x, h - 1 - y),
                _ => (y, w - 1 - x),
            };
            let at = origin + to_y * stride + to_x * 4;
            out[at..at + 4].copy_from_slice(px);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::blit;

    #[test]
    fn a_quarter_turn_puts_the_top_left_pixel_top_right() {
        // A 2×1 frame onto a 1-wide, 2-tall panel, 4-byte rows.
        let frame = [1, 1, 1, 1, 2, 2, 2, 2];
        let mut out = [0u8; 8];
        blit(&mut out, (0, 4, 1), &frame, 8, (2, 1));
        assert_eq!(out, [1, 1, 1, 1, 2, 2, 2, 2]);
        let mut out = [0u8; 8];
        blit(&mut out, (0, 4, 3), &frame, 8, (2, 1));
        assert_eq!(out, [2, 2, 2, 2, 1, 1, 1, 1]);
        let mut out = [0u8; 8];
        blit(&mut out, (0, 8, 2), &frame, 8, (2, 1));
        assert_eq!(out, [2, 2, 2, 2, 1, 1, 1, 1]);
    }
}
