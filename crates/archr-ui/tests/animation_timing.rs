//! Manual wall-time sample of animated draw + present on the PC offscreen backend.
mod common;
use archr_index::cache::SystemEntry;
use archr_ui::{render::Ui, state::App, Theme};
use common::FakeData as Data;
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
#[ignore = "manual PC animation timing; run with --ignored --nocapture"]
fn selection_animation_frame_time() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let root = tempfile::tempdir().unwrap();
    let battery = root.path().join("sys/class/power_supply/battery");
    fs::create_dir_all(&battery).unwrap();
    fs::write(battery.join("capacity"), "87").unwrap();
    fs::write(battery.join("status"), "Discharging").unwrap();
    let theme =
        Theme::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"));
    let data = Data {
        systems: [
            ("gba", "Game Boy Advance", 4),
            ("snes", "Super Nintendo", 10),
            ("genesis", "Mega Drive", 3),
            ("n64", "Nintendo 64", 3),
            ("psx", "PlayStation", 3),
            ("gb", "Game Boy", 2),
            ("nes", "Nintendo Entertainment System", 2),
        ]
        .into_iter()
        .map(|(name, fullname, count)| SystemEntry {
            name: name.into(),
            fullname: fullname.into(),
            game_count: count,
            dir_mtime: 0,
            source_fingerprint: 0,
        })
        .collect(),
        counts: [("@favorites".into(), 12), ("@recent".into(), 8)].into(),
        ..Data::default()
    };
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    ui.configure(&ttf, &creator, &theme, Some(root.path()))
        .unwrap();
    let mut app = App::new(ui.page_size(&theme));
    ui.draw(&app, &data, &theme, None).unwrap();
    let mut samples = Vec::new();
    for gesture in 0..24 {
        ui.snap_animations();
        app.cursor = 3 + gesture % 2;
        ui.tick();
        ui.draw(&app, &data, &theme, None).unwrap();
        while ui.animating() {
            // Offscreen has no monitor vblank: explicitly sample at ~60 Hz.
            std::thread::sleep(Duration::from_millis(16));
            let began = Instant::now();
            ui.tick();
            ui.draw(&app, &data, &theme, None).unwrap();
            let micros = began.elapsed().as_micros();
            if gesture >= 4 && ui.animating() {
                samples.push(micros);
            }
        }
    }
    samples.sort_unstable();
    assert!(samples.len() >= 50);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "ANIMATION_TIMING profile={profile} samples={} median_us={} p95_us={} max_us={}",
        samples.len(),
        samples[samples.len() / 2],
        samples[(samples.len() * 95 / 100).min(samples.len() - 1)],
        samples[samples.len() - 1]
    );
}
