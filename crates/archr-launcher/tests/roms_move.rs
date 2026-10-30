use std::{fs, process::Command};

/// The pixel at (x, y) of a captured frame, RGB.
fn pixel(frame: &std::path::Path, x: usize, y: usize) -> [u8; 3] {
    use sdl2::image::LoadSurface;
    let surface = sdl2::surface::Surface::from_file(frame)
        .unwrap()
        .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    let pitch = surface.pitch() as usize;
    surface.with_lock(|bytes| {
        let at = y * pitch + x * 4;
        [bytes[at], bytes[at + 1], bytes[at + 2]]
    })
}

#[test]
fn the_screen_shows_the_progress_file_and_writes_nothing_on_storage() {
    let root = tempfile::tempdir().unwrap();
    let work = root.path().join("run/archr/roms-migrate");
    fs::create_dir_all(&work).unwrap();
    fs::write(
        work.join("progress"),
        "STAGE=copying\nPROGRESS=2/4\nENTRY=nes\nBYTES=500/1000\nLEFT=90\n",
    )
    .unwrap();
    let frame = root.path().join("frame.png");
    let result = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .env("ARCHR_LAUNCHER_SCREENSHOT", &frame)
        .args(["--windowed", "--roms-migrate-screen"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    // Half the bar is the accent, the other half the track (Grafite).
    let bar = (480 - 245) / 2 + 44 + 25 + 18 + 3 * 14 + 8 + 5;
    assert_eq!(pixel(&frame, 160, bar), [0x1E, 0x88, 0xFF]);
    assert_eq!(pixel(&frame, 480, bar), [0x1C, 0x23, 0x2C]);
    // STORAGE is unmounted during the migration: nothing may live there.
    assert!(!root.path().join("storage").exists());
}
