use std::{fs, process::Command};

#[test]
fn windowed_script_navigates_and_publishes_game() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/emulationstation");
    fs::create_dir_all(&config).unwrap();
    let systems = ["3do", "amiga", "nes"];
    let mut xml = String::from("<systemList>");
    for system in systems {
        let dir = root.path().join(format!("storage/roms/{system}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Game.rom"), []).unwrap();
        xml.push_str(&format!("<system><name>{system}</name><fullname>{system} full</fullname><path>/storage/roms/{system}</path><extension>.rom</extension><emulators><emulator name=\"retroarch\"><cores><core default=\"true\">test</core></cores></emulator></emulators></system>"));
    }
    xml.push_str("</systemList>");
    fs::write(config.join("es_systems.cfg"), xml).unwrap();
    let run = root.path().join("run/archr-launcher");
    fs::create_dir_all(&run).unwrap();
    fs::write(run.join("last-exit"), "7\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .env("ARCHR_LAUNCHER_SCREENSHOT", root.path().join("frame.png"))
        .args(["--windowed", "--script", "A,SNAPSHOT,B,Y,B,A,Y,R,B,X,A"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let png = fs::read(root.path().join("frame.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert!(png.len() > 2000, "frame should contain rendered text");
    let next = fs::read_to_string(run.join("next")).unwrap();
    assert!(
        next.starts_with(&format!(
            "game '{}/usr/bin/runemu.sh' ",
            root.path().display()
        )),
        "{next}"
    );
    assert!(next.contains("-P3do"));
    assert!(next.contains(root.path().to_str().unwrap()));
    assert!(next.contains("--emulator=retroarch"));
    assert!(next.contains("--core=test"));
    assert!(!run.join("last-exit").exists());
    let store = fs::read_to_string(
        root.path()
            .join("storage/.config/archr-launcher/store.toml"),
    )
    .unwrap();
    assert!(store.contains("3do/"));
    let cfg = fs::read_to_string(
        root.path()
            .join("storage/.config/system/configs/system.cfg"),
    )
    .unwrap();
    assert!(cfg.contains("3do[\"Game.rom\"].core=test"));
}

#[test]
fn missing_systems_still_allow_settings_and_fallback_is_logged() {
    let root = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .args(["--windowed", "--script", "Y,A"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.path().join("run/archr-launcher/next")).unwrap(),
        "action settings\n"
    );
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("using builtin"),
        "theme fallback should reach the log"
    );
}

#[test]
fn snapshot_after_launch_button_captures_the_opening_overlay() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/emulationstation");
    let roms = root.path().join("storage/roms/snes");
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(&roms).unwrap();
    fs::write(roms.join("Kirby.sfc"), []).unwrap();
    fs::write(config.join("es_systems.cfg"), "<systemList><system><name>snes</name><fullname>Super Nintendo</fullname><path>/storage/roms/snes</path><extension>.sfc</extension><emulators><emulator name=\"retroarch\"><cores><core>snes9x</core></cores></emulator></emulators></system></systemList>").unwrap();
    use sdl2::image::SaveSurface;
    let mut cover =
        sdl2::surface::Surface::new(210, 280, sdl2::pixels::PixelFormatEnum::RGBA32).unwrap();
    cover
        .fill_rect(None, sdl2::pixels::Color::RGB(200, 0, 0))
        .unwrap();
    cover.save(roms.join("cover.png")).unwrap();
    fs::write(
        roms.join("gamelist.xml"),
        "<gameList><game><path>./Kirby.sfc</path><image>./cover.png</image></game></gameList>",
    )
    .unwrap();
    let frame = root.path().join("opening.png");
    let result = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .env("ARCHR_LAUNCHER_SCREENSHOT", &frame)
        .args(["--windowed", "--script", "A,WAIT,WAIT,A,SNAPSHOT"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    use sdl2::image::LoadSurface;
    let surface = sdl2::surface::Surface::from_file(&frame)
        .unwrap()
        .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    surface.with_lock(|bytes| {
        let offset = 220 * surface.pitch() as usize + 140 * 4;
        assert_eq!(
            &bytes[offset..offset + 3],
            &[22, 28, 35],
            "opening overlay uses the elevated card"
        );
        let offset = 100 * surface.pitch() as usize + 500 * 4;
        assert!(
            bytes[offset] > 60 && bytes[offset + 1] < 10,
            "terminal feedback must keep the loaded cover under its scrim"
        );
    });
}
