use std::process::Command;

#[test]
fn first_present_only_prepares_visible_rasters_and_fonts() {
    let populated = (0..16).map(|i| format!(
        "<system><name>s{i}</name><fullname>Система {i} — {} </fullname><path>/roms/s{i}</path><extension>.rom</extension></system>",
        "An extraordinarily long system title ".repeat(3)
    )).collect::<String>();
    for definitions in ["", populated.as_str()] {
        check_startup(definitions, false);
    }
}

#[test]
fn first_present_prepares_fitted_names_without_the_deferred_panel() {
    check_startup(
        "<system><name>moto</name><fullname>MO/TO family (TO7, TO7/70, TO8, TO8D, TO9, TO9+, MO5 and MO6)</fullname><manufacturer>Thomson</manufacturer><release>1982</release><path>/roms/moto</path><extension>.rom</extension></system>",
        true,
    );
}

fn check_startup(definitions: &str, themed: bool) {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/archr");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("systems.cfg"),
        format!("<systemList>{definitions}</systemList>"),
    )
    .unwrap();
    if themed {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default");
        let theme = root.path().join("usr/share/archr-launcher/themes/default");
        std::fs::create_dir_all(theme.join("fonts")).unwrap();
        std::fs::copy(source.join("theme.toml"), theme.join("theme.toml")).unwrap();
        for entry in std::fs::read_dir(source.join("fonts")).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().is_some_and(|ext| ext == "ttf") {
                std::fs::copy(entry.path(), theme.join("fonts").join(entry.file_name())).unwrap();
            }
        }
    }
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
            .env("ARCHR_LAUNCHER_ROOT", root.path())
            .env("ARCHR_LAUNCHER_TRACE", "1")
            .env("SDL_VIDEODRIVER", "offscreen")
            .args(["--windowed", "--script", "Y,B,QUIT"])
            .output()
            .unwrap();
        let trace = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{trace}");
        let before = trace.split("startup first present:").next().unwrap();
        for absent in [
            "SDL_image init",
            "shadow texture",
            "icon rasterization",
            "badge A",
            "badge Y",
            "badge B",
            "badge X",
            "PNG hit",
            "experiments:",
        ] {
            assert!(!before.contains(absent), "{before}");
        }
        assert!(
            before.contains("face=Mono;"),
            "status clock is visible: {before}"
        );
        let after_join = before
            .split("startup worker join:")
            .nth(1)
            .expect("worker join");
        assert!(!after_join.contains("text rasterization"), "{after_join}");
        assert!(!after_join.contains("text measurement"), "{after_join}");
        assert!(!after_join.contains("font file open"), "{after_join}");
        assert_eq!(trace.matches("startup first frame draw:").count(), 1);
        assert_eq!(trace.matches("startup first present:").count(), 1);
        let cache = root.path().join("storage/.cache/archr-launcher");
        if cache.exists() {
            assert!(!std::fs::read_dir(cache)
                .unwrap()
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().starts_with("raster-")));
        }
    }
}

#[test]
fn the_launcher_forgets_the_running_game_when_it_starts() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/archr");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("systems.cfg"), "<systemList></systemList>").unwrap();
    let record = root.path().join("run/archr/current-game");
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::fs::write(&record, "/storage/roms/snes/Mario.sfc\n4242\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .args(["--windowed", "--script", "QUIT"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!record.exists());
}
