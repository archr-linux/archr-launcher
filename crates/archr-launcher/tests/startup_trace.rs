use std::{process::Command, time::Duration};

fn startup(trace: Option<&str>) -> String {
    let root = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_archr-launcher"));
    command
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .env_remove("ARCHR_LAUNCHER_TRACE")
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
    let position = |name| {
        phases
            .iter()
            .position(|(phase, _)| *phase == name)
            .unwrap_or_else(|| panic!("missing {name}: {stderr}"))
    };
    for ordered in [
        vec![
            "args/paths",
            "TTF_Init",
            "SDL_Init video",
            "window creation",
            "renderer / GL context creation",
            "worker join",
            "CPU surfaces upload",
            "status initialization",
            "first frame draw",
            "first present",
            "SDL joystick init",
            "SDL gamecontroller init",
        ],
        vec![
            "TTF_Init",
            "system.cfg read",
            "locale selection",
            "theme load",
            "overview cache read",
            "system definitions read",
            "first text rasterization",
            "badge Y",
            "badge A",
            "CPU assets ready",
            "worker join",
        ],
    ] {
        let positions: Vec<_> = ordered.into_iter().map(position).collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{stderr}"
        );
        assert!(
            positions
                .windows(2)
                .all(|pair| phases[pair[0]].1 <= phases[pair[1]].1),
            "{stderr}"
        );
    }
    let after_join = stderr
        .split("startup worker join:")
        .nth(1)
        .unwrap()
        .split("startup first present:")
        .next()
        .unwrap();
    assert!(!after_join.contains("text rasterization"), "{after_join}");
    assert!(!after_join.contains("font file open"), "{after_join}");
    for name in ["first text rasterization", "first present"] {
        assert_eq!(phases.iter().filter(|(phase, _)| *phase == name).count(), 1);
    }
    assert!(phases[..position("first present")]
        .iter()
        .any(|(phase, _)| phase.starts_with("font file open [") && phase.contains("ok=true")));
}

#[test]
fn enabled_wifi_and_hub_navigation_do_not_rasterize_icons_before_present() {
    let root = tempfile::tempdir().unwrap();
    let radio = root.path().join("sys/class/rfkill/rfkill0");
    std::fs::create_dir_all(&radio).unwrap();
    std::fs::write(radio.join("type"), "wlan\n").unwrap();
    std::fs::write(radio.join("soft"), "0\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("ARCHR_LAUNCHER_TRACE", "1")
        .env("SDL_VIDEODRIVER", "offscreen")
        .args([
            "--windowed",
            "--script",
            "WAIT,START,WAIT,B,START,WAIT,B,QUIT",
        ])
        .output()
        .unwrap();
    let trace = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{trace}");
    let (before, _) = trace
        .split_once("startup first present:")
        .expect("first present");
    assert!(!before.contains("icon rasterization"), "{before}");
}
