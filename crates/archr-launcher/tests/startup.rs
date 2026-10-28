use std::process::Command;

#[test]
fn first_present_only_prepares_visible_rasters_and_fonts() {
    let populated = (0..16).map(|i| format!(
        "<system><name>s{i}</name><fullname>Система {i} — {} </fullname><path>/roms/s{i}</path><extension>.rom</extension></system>",
        "An extraordinarily long system title ".repeat(3)
    )).collect::<String>();
    for definitions in ["", populated.as_str()] {
        check_startup(definitions);
    }
}

fn check_startup(definitions: &str) {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/emulationstation");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("es_systems.cfg"),
        format!("<systemList>{definitions}</systemList>"),
    )
    .unwrap();
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
