use std::process::Command;

#[test]
fn first_present_only_prepares_visible_rasters_and_fonts() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/emulationstation");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("es_systems.cfg"), "<systemList></systemList>").unwrap();
    let run = |experiments: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_archr-launcher"));
        command
            .env("ARCHR_LAUNCHER_ROOT", root.path())
            .env("ARCHR_LAUNCHER_TRACE", "1")
            .env("SDL_VIDEODRIVER", "offscreen")
            .args(["--windowed", "--script", "Y,B,QUIT"]);
        if let Some(value) = experiments {
            command.env("ARCHR_LAUNCHER_EXP", value);
        } else {
            command.env_remove("ARCHR_LAUNCHER_EXP");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stderr).unwrap()
    };
    let cold = run(None);
    let warm = run(None);
    let baseline = run(Some("none"));
    for trace in [&cold, &warm] {
        let before = trace.split("startup first present:").next().unwrap();
        assert!(!before.contains("SDL_image init"), "{before}");
        assert!(!before.contains("shadow texture"), "{before}");
        assert!(!before.contains("badge B"), "{before}");
        assert!(!before.contains("badge X"), "{before}");
        assert!(
            before.contains("face=Mono;"),
            "status clock is visible: {before}"
        );
        assert_eq!(trace.matches("startup first frame draw:").count(), 1);
        assert_eq!(trace.matches("startup first present:").count(), 1);
    }
    assert!(warm.contains("badge Y [PNG hit]"), "{warm}");
    assert!(warm.contains("badge A [PNG hit]"), "{warm}");
    assert!(baseline.contains("experiments: []"));
    let before = baseline.split("startup first present:").next().unwrap();
    assert!(before.contains("SDL_image init"));
    assert!(before.contains("shadow texture"));
    assert!(before.contains("badge X"));
    let cache = root.path().join("storage/.cache/archr-launcher");
    for (switches, badge, shadow) in [
        ("badgecache,lazyassets", true, false),
        ("shadowcache,lazyassets", false, true),
    ] {
        for entry in std::fs::read_dir(&cache).unwrap().flatten() {
            if entry.file_name().to_string_lossy().starts_with("raster-") {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }
        run(Some(switches));
        let names: Vec<_> = std::fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.iter().any(|name| name.contains("-badge-")), badge);
        assert_eq!(
            names.iter().any(|name| name.ends_with("-shadow.png")),
            shadow
        );
    }
}
