//! The console's framebuffer (/dev/fb0) as the screen of a frame drawn by
//! SDL's software renderer: no GL, no Mesa. The maintenance boot has no GPU
//! module, and Mesa's software GL spends ~90 ms of CPU a frame on an RK3326.
//!
//! Frames are written to the device, not mapped: the kernel checks every
//! offset, and the only unsafe code left is four ioctls. The console is put
//! in graphics mode so its text does not draw over the frame, and back in
//! text mode on drop; after a crash the unit's TTYReset does it, and the
//! console shows the migration's log again.
use std::{
    fs::File,
    os::{fd::AsRawFd, unix::fs::FileExt},
};

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;
const KDSETMODE: libc::c_ulong = 0x4B3A;
const KD_TEXT: libc::c_ulong = 0;
const KD_GRAPHICS: libc::c_ulong = 1;

/// linux/fb.h's fb_fix_screeninfo; only the length and the row size are read.
#[repr(C)]
#[derive(Default)]
struct FixInfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    kind: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

pub struct Framebuffer {
    fb: File,
    console: File,
    /// Bytes from one row to the next, and where the shown page starts.
    stride: u64,
    origin: u64,
    width: usize,
    height: usize,
    /// Clockwise quarter turns from the drawn frame to the panel (fbcon's).
    turns: u8,
    /// The frame turned to the panel, rows `width * 4` bytes apart.
    panel: Vec<u8>,
}
impl Framebuffer {
    pub fn open() -> Result<Self, String> {
        let fail = |what: &str, e: std::io::Error| format!("{what}: {e}");
        let fb = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/fb0")
            .map_err(|e| fail("/dev/fb0", e))?;
        // fb_var_screeninfo, 40 u32: xres, yres, xres_virtual, yres_virtual,
        // xoffset, yoffset, bits_per_pixel, grayscale, then the red, green
        // and blue bitfields (offset, length, msb_right).
        let mut var = [0u32; 40];
        let mut fix = FixInfo::default();
        // SAFETY: each ioctl fills a struct of the size it is given.
        let read = unsafe {
            libc::ioctl(fb.as_raw_fd(), FBIOGET_VSCREENINFO, var.as_mut_ptr()) == 0
                && libc::ioctl(fb.as_raw_fd(), FBIOGET_FSCREENINFO, &mut fix) == 0
        };
        if !read {
            return Err(fail("FBIOGET_*SCREENINFO", std::io::Error::last_os_error()));
        }
        let geometry = Geometry {
            size: (var[0], var[1]),
            offset: (var[4], var[5]),
            bpp: var[6],
            rgb_offsets: (var[8], var[11], var[14]),
            stride: fix.line_length,
            len: fix.smem_len,
        };
        let origin = geometry.check().map_err(|e| format!("/dev/fb0: {e}"))?;
        let turns = std::fs::read_to_string("/sys/class/graphics/fbcon/rotate")
            .ok()
            .and_then(|turns| turns.trim().parse::<u8>().ok())
            .unwrap_or(0)
            % 4;
        let console = File::open("/dev/tty0").map_err(|e| fail("/dev/tty0", e))?;
        // SAFETY: KDSETMODE takes its value by argument.
        unsafe { libc::ioctl(console.as_raw_fd(), KDSETMODE, KD_GRAPHICS) };
        let (width, height) = (var[0] as usize, var[1] as usize);
        Ok(Self {
            fb,
            console,
            stride: fix.line_length.into(),
            origin,
            width,
            height,
            turns,
            panel: vec![0; width * height * 4],
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
    pub fn show(&mut self, pixels: &[u8], pitch: usize) -> std::io::Result<()> {
        let size = self.size();
        blit(&mut self.panel, self.turns, pixels, pitch, size);
        let row = self.width * 4;
        if self.stride == row as u64 {
            return self.fb.write_all_at(&self.panel, self.origin);
        }
        for (y, line) in self.panel.chunks_exact(row).enumerate() {
            self.fb
                .write_all_at(line, self.origin + y as u64 * self.stride)?;
        }
        Ok(())
    }
}
impl Drop for Framebuffer {
    fn drop(&mut self) {
        // SAFETY: as in open.
        unsafe { libc::ioctl(self.console.as_raw_fd(), KDSETMODE, KD_TEXT) };
    }
}

/// What the driver says of the shown page.
struct Geometry {
    size: (u32, u32),
    offset: (u32, u32),
    bpp: u32,
    /// Bit offsets of red, green and blue in a pixel.
    rgb_offsets: (u32, u32, u32),
    stride: u32,
    len: u32,
}
impl Geometry {
    /// Where the shown page starts, when it is XRGB8888 and fits the memory.
    fn check(&self) -> Result<u64, String> {
        let Self {
            size: (w, h),
            offset: (x0, y0),
            ..
        } = *self;
        if self.bpp != 32 || self.rgb_offsets != (16, 8, 0) {
            return Err(format!("not XRGB8888 ({} bits per pixel)", self.bpp));
        }
        let (stride, row) = (u64::from(self.stride), u64::from(w) * 4);
        if w == 0 || h == 0 || stride < row {
            return Err(format!("{w}x{h}, rows of {stride} bytes"));
        }
        let origin = u64::from(y0) * stride + u64::from(x0) * 4;
        if origin + (u64::from(h) - 1) * stride + row > u64::from(self.len) {
            return Err(format!("{w}x{h} at {x0},{y0} past {} bytes", self.len));
        }
        Ok(origin)
    }
}

/// Copies a `w`×`h` frame into the packed `out`, turned clockwise `turns`
/// times.
fn blit(out: &mut [u8], turns: u8, pixels: &[u8], pitch: usize, (w, h): (u32, u32)) {
    let (w, h) = (w as usize, h as usize);
    // The panel's row: the frame's, or its column when a quarter turn.
    let stride = if turns % 2 == 1 { h * 4 } else { w * 4 };
    for y in 0..h {
        let row = &pixels[y * pitch..y * pitch + w * 4];
        if turns == 0 {
            out[y * stride..y * stride + w * 4].copy_from_slice(row);
            continue;
        }
        for (x, px) in row.chunks_exact(4).enumerate() {
            let (to_x, to_y) = match turns {
                1 => (h - 1 - y, x),
                2 => (w - 1 - x, h - 1 - y),
                _ => (y, w - 1 - x),
            };
            let at = to_y * stride + to_x * 4;
            out[at..at + 4].copy_from_slice(px);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blit, Geometry};

    #[test]
    fn a_quarter_turn_puts_the_top_left_pixel_top_right() {
        // A 2×1 frame onto a 1-wide, 2-tall panel, 4-byte rows.
        let frame = [1, 1, 1, 1, 2, 2, 2, 2];
        let mut out = [0u8; 8];
        blit(&mut out, 1, &frame, 8, (2, 1));
        assert_eq!(out, [1, 1, 1, 1, 2, 2, 2, 2]);
        let mut out = [0u8; 8];
        blit(&mut out, 3, &frame, 8, (2, 1));
        assert_eq!(out, [2, 2, 2, 2, 1, 1, 1, 1]);
        let mut out = [0u8; 8];
        blit(&mut out, 2, &frame, 8, (2, 1));
        assert_eq!(out, [2, 2, 2, 2, 1, 1, 1, 1]);
        // A padded frame row: only `w` pixels of it are copied.
        let frame = [1, 1, 1, 1, 9, 9, 9, 9, 2, 2, 2, 2, 9, 9, 9, 9];
        let mut out = [0u8; 8];
        blit(&mut out, 0, &frame, 8, (1, 2));
        assert_eq!(out, [1, 1, 1, 1, 2, 2, 2, 2]);
    }
    #[test]
    fn only_an_xrgb_page_inside_the_memory_is_drawn() {
        // The R36S: 640×480, double buffered, the second page shown.
        let r36s = Geometry {
            size: (640, 480),
            offset: (0, 480),
            bpp: 32,
            rgb_offsets: (16, 8, 0),
            stride: 2560,
            len: 2560 * 960,
        };
        assert_eq!(r36s.check(), Ok(2560 * 480));
        let past = Geometry {
            offset: (0, 481),
            ..r36s
        };
        assert!(past.check().is_err());
        let short_rows = Geometry {
            stride: 2556,
            offset: (0, 0),
            ..past
        };
        assert!(short_rows.check().is_err());
        let bgr = Geometry {
            rgb_offsets: (0, 8, 16),
            ..short_rows
        };
        assert!(bgr.check().is_err());
        let rgb565 = Geometry {
            bpp: 16,
            rgb_offsets: (11, 5, 0),
            ..bgr
        };
        assert!(rgb565.check().is_err());
        let empty = Geometry {
            size: (0, 0),
            bpp: 32,
            rgb_offsets: (16, 8, 0),
            ..rgb565
        };
        assert!(empty.check().is_err());
    }
}
