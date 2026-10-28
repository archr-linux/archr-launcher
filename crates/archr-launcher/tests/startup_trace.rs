use std::{process::Command, time::Duration};

fn startup(trace: Option<&str>) -> String {
    let root = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_archr-launcher"));
    command
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .env_remove("ARCHR_LAUNCHER_TRACE")
        .env("ARCHR_LAUNCHER_EXP", "none")
        .args(["--windowed", "--script", "Y,B,QUIT"]);
    if let Some(value) = trace {
        command.env("ARCHR_LAUNCHER_TRACE", value);
    }
    let result = command.output().unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(result.status.success(), "{stderr}");
    stderr
}

#[test]
fn startup_trace_is_opt_in() {
    for value in [None, Some("0"), Some("true"), Some("")] {
        let stderr = startup(value);
        assert!(!stderr.contains("archr-launcher: startup "), "{stderr}");
        assert!(stderr.contains("archr-launcher: first frame "));
    }
}

#[test]
fn startup_trace_reports_ordered_phases_and_first_events_once() {
    let stderr = startup(Some("1"));
    let phases: Vec<_> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("archr-launcher: startup "))
        .map(|line| {
            let (phase, elapsed) = line.rsplit_once(": ").expect("phase timestamp");
            let phase = if phase.starts_with("badge ") {
                phase.split(" [").next().unwrap()
            } else {
                phase
            };
            let ms: u64 = elapsed.strip_suffix(" ms").unwrap().parse().unwrap();
            (phase, Duration::from_millis(ms))
        })
        .collect();
    assert!(!phases.is_empty(), "{stderr}");
    assert!(phases.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    let position = |name| {
        phases
            .iter()
            .position(|(phase, _)| *phase == name)
            .unwrap_or_else(|| panic!("missing {name}: {stderr}"))
    };
    let ordered = [
        "args/paths",
        "system.cfg read",
        "locale selection",
        "SDL_Init video",
        "window creation",
        "renderer / GL context creation",
        "SDL_image init",
        "TTF_Init",
        "theme load",
        "first text rasterization",
        "badge A",
        "badge B",
        "badge X",
        "badge Y",
        "shadow texture",
        "status initialization",
        "overview cache read",
        "system definitions read",
        "first frame draw",
        "first present",
        "SDL joystick init",
        "SDL gamecontroller init",
    ];
    let positions: Vec<_> = ordered.into_iter().map(position).collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    for name in ["first text rasterization", "first present"] {
        assert_eq!(phases.iter().filter(|(phase, _)| *phase == name).count(), 1);
    }
    assert!(phases[..position("first present")]
        .iter()
        .any(|(phase, _)| phase.starts_with("font file open [") && phase.contains("ok=true")));
}
