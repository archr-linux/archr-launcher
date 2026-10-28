use archr_launcher::{paths::Paths, settings::Backend};
use archr_ui::settings::Id;
use std::{fs, os::unix::fs::PermissionsExt};
use tempfile::{tempdir, TempDir};
// The shared process runner retries ETXTBSY from parallel fixture scripts.
fn fixture() -> (TempDir, Backend) {
    let dir = tempdir().unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
    fs::create_dir_all(paths.at("/usr/bin")).unwrap();
    fs::create_dir_all(paths.at("/sys/power")).unwrap();
    fs::write(paths.at("/sys/power/state"), "freeze mem disk").unwrap();
    fs::write(&paths.config, "system.suspendmode=off\naudio.volume=60\n").unwrap();
    let backend = Backend::new(paths);
    (dir, backend)
}
fn script(dir: &TempDir, name: &str, body: &str) {
    let path = dir.path().join("usr/bin").join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
#[test]
fn scripts_are_single_owners_and_runtime_only_operations_publish_after_success() {
    let (dir, backend) = fixture();
    script(&dir, "suspendmode", "case \"$(/bin/cat \"$ARCHR_LAUNCHER_ROOT/storage/.config/system/configs/system.cfg\")\" in *system.suspendmode=off*) printf 'system.suspendmode=mem\n' > \"$ARCHR_LAUNCHER_ROOT/storage/.config/system/configs/system.cfg\";; *) exit 23;; esac");
    assert_eq!(backend.apply(Id::Suspend, "mem").unwrap(), "mem");
    script(&dir, "pactl", "case \"$(/bin/cat \"$ARCHR_LAUNCHER_ROOT/storage/.config/system/configs/system.cfg\")\" in *audio.volume=65*) exit 24;; *) exit 0;; esac");
    backend.apply(Id::Volume, "65").unwrap();
}
#[test]
fn failure_reports_status_and_stderr_without_publishing_value() {
    let (dir, backend) = fixture();
    script(&dir, "volume", "false; exit 0"); // Legacy helper masks the runtime error.
    script(&dir, "pactl", "echo 'sink unavailable' >&2; exit 7");
    let error = backend.apply(Id::Volume, "65").unwrap_err();
    assert!(
        error.contains('7') && error.contains("sink unavailable"),
        "{error}"
    );
    assert!(
        fs::read_to_string(dir.path().join("storage/.config/system/configs/system.cfg"))
            .unwrap()
            .contains("audio.volume=60")
    );
}
#[test]
fn invalid_values_never_reach_scripts_or_config() {
    let (_dir, backend) = fixture();
    for (id, value) in [
        (Id::Volume, "101"),
        (Id::Brightness, "0"),
        (Id::Gpu, "bad"),
        (Id::Governor, "bad; reboot"),
        (Id::Language, "xx"),
        (Id::Timezone, "../../etc/passwd"),
    ] {
        assert!(backend.apply(id, value).is_err());
    }
}
#[test]
fn load_parses_comma_timezones_and_includes_current_suspend_mode() {
    let (dir, backend) = fixture();
    script(
        &dir,
        "timeinfo",
        "printf 'Europe/London,\\nAmerica/Sao_Paulo,\\n'",
    );
    script(&dir, "set-audio", "printf 'Speakers\\nHeadphones\\n'");
    let state = backend.load().unwrap();
    assert!(state.options[&Id::Timezone].contains(&"Europe/London".into()));
    assert!(state.options[&Id::Timezone].contains(&"UTC".into()));
    assert!(state.options[&Id::Suspend].contains(&"off".into()));
}

#[test]
fn inactivity_keys_store_minutes_and_default_to_never() {
    let (_dir, backend) = fixture();
    let state = backend.load().unwrap();
    for (id, key) in [
        (Id::Idle, "idle.timeout"),
        (Id::IdleGame, "idle.timeout.game"),
    ] {
        assert_eq!(id.key(), key);
        assert_eq!(state.value(id), "0");
        assert_eq!(state.options[&id], ["0", "1", "3", "5", "10", "15", "30"]);
        for value in ["0", "1", "3", "5", "10", "15", "30"] {
            assert_eq!(backend.apply(id, value).unwrap(), value);
            assert_eq!(backend.load().unwrap().value(id), value);
        }
        assert!(backend.apply(id, "60").is_err());
    }
}

#[test]
fn worker_does_not_block_input_and_commits_only_after_success() {
    use archr_launcher::{
        settings::Runtime,
        state::{Button, Outcome},
    };
    use archr_ui::settings::Page;
    use std::time::{Duration, Instant};
    let (dir, _) = fixture();
    script(&dir, "set-audio", "printf 'Speakers\\n'");
    script(&dir, "timeinfo", "printf 'UTC,\\n'");
    script(
        &dir,
        "pactl",
        "if [ \"$1\" = -- ]; then /bin/sleep 0.2; fi; exit 0",
    );
    let mut runtime = Runtime::new(Paths::new(Some(dir.path().to_owned())));
    runtime.open();
    let deadline = Instant::now() + Duration::from_secs(3);
    while runtime.state.loading {
        runtime.poll();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    runtime.state.open(Page::Sound);
    let began = Instant::now();
    assert_eq!(runtime.handle(Button::R), Outcome::None);
    assert!(began.elapsed() < Duration::from_millis(50));
    // Sliders are optimistic: no "applying" state, the close gate still waits.
    assert_eq!(runtime.state.pending, None);
    assert_eq!(runtime.state.value(Id::Volume), "65");
    assert!(runtime.mutations_pending());
    runtime.handle(Button::B);
    assert_eq!(runtime.state.page(), Page::Root);
    while runtime.busy() {
        runtime.poll();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(runtime.state.value(Id::Volume), "65");
    assert!(!runtime.mutations_pending());
}

#[test]
fn load_uses_live_volume_and_first_backlight_percent() {
    let (dir, backend) = fixture();
    script(
        &dir,
        "pactl",
        "printf 'Volume: front-left: 32768 / 50%% / -18.06 dB\\n'",
    );
    let backlight = dir.path().join("sys/class/backlight/panel");
    fs::create_dir_all(&backlight).unwrap();
    // Sysfs reports a page-sized st_size even for short numeric attributes.
    fs::write(backlight.join("brightness"), format!("{:<4096}", "64\n")).unwrap();
    fs::write(
        backlight.join("max_brightness"),
        format!("{:<4096}", "255\n"),
    )
    .unwrap();
    let state = backend.load().unwrap();
    assert_eq!(state.value(Id::Volume), "50");
    assert_eq!(state.value(Id::Brightness), "25");
}

#[test]
fn load_normalizes_stored_sliders_and_rejects_nonregular_probes() {
    let (dir, backend) = fixture();
    let cfg = dir.path().join("storage/.config/system/configs/system.cfg");
    fs::write(&cfg, "audio.volume=32767\ndisplay.brightness=-500\n").unwrap();
    fs::create_dir_all(dir.path().join("proc")).unwrap();
    let fifo = std::ffi::CString::new(dir.path().join("proc/cpuinfo").to_str().unwrap()).unwrap();
    // SAFETY: a valid NUL-terminated fixture path, no retained pointer.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let began = std::time::Instant::now();
    let state = backend.load().unwrap();
    assert!(began.elapsed() < std::time::Duration::from_secs(1));
    assert_eq!(state.value(Id::Volume), "100");
    assert_eq!(state.value(Id::Brightness), "5");
}

#[test]
fn information_distinguishes_active_driver_and_detects_hardware_and_metrics() {
    use std::os::unix::fs::symlink;
    let (dir, backend) = fixture();
    let root = dir.path();
    for (path, value) in [
        (
            "proc/cpuinfo",
            "processor : 0\nmodel name : Example x86 CPU\n",
        ),
        ("proc/uptime", "7380.0 9000.0"),
        ("sys/class/thermal/thermal_zone0/temp", "43000"),
        ("sys/class/graphics/fb0/virtual_size", "640,480"),
        (
            "sys/devices/system/cpu/cpufreq/policy0/scaling_governor",
            "performance",
        ),
    ] {
        let p = root.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, value).unwrap();
    }
    let display = root.join("sys/class/drm/card0/device/driver");
    fs::create_dir_all(display.parent().unwrap()).unwrap();
    symlink("/drivers/rockchip-drm", display).unwrap();
    let driver = root.join("sys/class/drm/renderD128/device/driver");
    fs::create_dir_all(driver.parent().unwrap()).unwrap();
    symlink("/drivers/panfrost", &driver).unwrap();
    let info = backend.load().unwrap().info;
    assert!(info.cards[0]
        .detail
        .rows
        .iter()
        .any(|(key, value)| *key == "settings.uptime_label"
            && value == "settings.duration_hours|hours=2|minutes=3"));
    assert_eq!(info.cards.len(), 6);
    assert_eq!(info.cards[0].detail.heading, "Example x86 CPU");
    assert!(info.cards[0]
        .detail
        .rows
        .iter()
        .any(|(_, v)| v == "performance"));
    assert!(info.cards[0].lines.iter().any(|l| l.contains("43.0 °C")));
    assert!(info.cards[1]
        .detail
        .rows
        .iter()
        .any(|(_, v)| v == "panfrost"));
    assert!(info.cards[1]
        .detail
        .rows
        .iter()
        .any(|(_, v)| v == "libmali"));
    assert!(info.cards[1]
        .detail
        .rows
        .iter()
        .any(|(_, v)| v == "640 × 480"));
    fs::remove_file(driver).unwrap();
    assert!(backend.load().unwrap().info.cards[1]
        .detail
        .rows
        .iter()
        .any(|(k, v)| *k == "settings.active_driver" && v.is_empty()));
    assert!(!info
        .cards
        .iter()
        .flat_map(|c| &c.lines)
        .any(|l| l.contains("Cortex-A35") || l.contains("Mali-G31")));
}

#[test]
fn brightness_publishes_once_after_success_and_keeps_config_on_sysfs_failure() {
    let (dir, backend) = fixture();
    let panel = dir.path().join("sys/class/backlight/panel");
    fs::create_dir_all(&panel).unwrap();
    fs::write(panel.join("max_brightness"), format!("{:<4096}", "255\n")).unwrap();
    fs::write(panel.join("brightness"), "128").unwrap();
    script(&dir, "brightness", "exit 29"); // Must never invoke the owning legacy helper.
    assert_eq!(backend.apply(Id::Brightness, "75").unwrap(), "75");
    assert_eq!(fs::read_to_string(panel.join("brightness")).unwrap(), "191");
    fs::remove_file(panel.join("brightness")).unwrap();
    fs::create_dir(panel.join("brightness")).unwrap();
    assert!(backend
        .apply(Id::Brightness, "80")
        .unwrap_err()
        .contains("brightness"));
    assert_eq!(backend.load().unwrap().value(Id::Brightness), "75");
}

#[test]
fn information_collects_detail_rows_and_core_deltas_without_commands() {
    let (dir, backend) = fixture();
    for (path, value) in [
        ("proc/cpuinfo", "processor : 0\nHardware : Rockchip RK3326\n"),
        ("proc/stat", "cpu 10 0 10 80 0 0 0 0\ncpu0 10 0 10 80 0 0 0 0\n"),
        ("proc/meminfo", "MemTotal: 1000000 kB\nMemAvailable: 750000 kB\nCached: 50000 kB\nSwapTotal: 100000 kB\nSwapFree: 90000 kB\n"),
        ("proc/uptime", "3540.0 0.0"),
        ("proc/net/route", "Iface Destination Gateway Flags\nwlan0 00000000 0103A8C0 0003\n"),
        ("etc/resolv.conf", "nameserver 1.1.1.1\n"),
        ("proc/sys/kernel/hostname", "archr-test"),
        ("sys/devices/system/cpu/cpufreq/policy0/scaling_cur_freq", "1008000"),
        ("sys/devices/system/cpu/cpufreq/policy0/scaling_min_freq", "408000"),
        ("sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq", "1512000"),
    ] {
        let p = dir.path().join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, value).unwrap();
    }
    let first = backend.load().unwrap().info;
    assert_eq!(first.cards[0].detail.rows[0].1, "@1008 MHz");
    fs::write(
        dir.path().join("proc/stat"),
        "cpu 20 0 20 100 0 0 0 0\ncpu0 20 0 20 100 0 0 0 0\n",
    )
    .unwrap();
    let second = backend.load().unwrap().info;
    assert_eq!(second.cards[0].detail.meters[0].percent, Some(50));
    assert_eq!(second.cards[2].percent, Some(25));
    assert!(second.cards[4]
        .detail
        .rows
        .iter()
        .any(|(_, v)| v == "1.1.1.1"));
    assert!(second.cards[4]
        .detail
        .rows
        .iter()
        .any(|(_, v)| v == "192.168.3.1"));
    assert!(second.cards[0].lines[0].contains("Rockchip RK3326"));
}

#[test]
fn cpu_hotplug_preserves_core_identity_and_reappearing_core_has_no_delta() {
    let (dir, backend) = fixture();
    let path = dir.path().join("proc/stat");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "cpu0 10 0 10 80
cpu2 90 0 10 100
",
    )
    .unwrap();
    backend.load().unwrap();
    fs::write(
        &path,
        "cpu2 140 0 10 150
",
    )
    .unwrap();
    let info = backend.load().unwrap().info;
    assert_eq!(info.cards[0].detail.meters[0].label, "CPU2");
    assert_eq!(info.cards[0].detail.meters[0].percent, Some(50));
    fs::write(
        &path,
        "cpu0 30 0 10 100
cpu2 190 0 10 200
",
    )
    .unwrap();
    let info = backend.load().unwrap().info;
    assert_eq!(info.cards[0].detail.meters[0].percent, None);
    assert_eq!(info.cards[0].detail.meters[1].percent, Some(50));
}
#[test]
fn arm64_identity_uses_compatible_and_cpu_part_without_hardware_field() {
    let (dir, backend) = fixture();
    for (path, value) in [
        (
            "proc/cpuinfo",
            "processor : 0
CPU part : 0xd03
",
        ),
        (
            "sys/firmware/devicetree/base/compatible",
            "board,example\0allwinner,sun50i-h616\0",
        ),
    ] {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    let info = backend.load().unwrap().info;
    assert!(info.cards[0].detail.heading.contains("SUN50I-H616"));
    assert!(info.cards[0].detail.subtitle.contains("Cortex-A53"));
    assert!(info.cards[0]
        .detail
        .subtitle
        .ends_with(&format!("|bits={}", usize::BITS)));
}

#[test]
fn battery_formats_integer_current_and_capacity_without_a_restart_timer() {
    let (dir, backend) = fixture();
    for (name, value) in [
        ("capacity", "83"),
        ("status", "Discharging"),
        ("voltage_now", "4100000"),
        ("current_now", "650000"),
        ("charge_full_design", "3200000"),
    ] {
        let path = dir.path().join("sys/class/power_supply/battery").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    let info = backend.load().unwrap().info;
    let rows = &info.cards[5].detail.rows;
    for (key, expected) in [
        ("settings.voltage", "@4.10 V"),
        ("settings.current", "@650 mA"),
        ("settings.design_capacity", "@3200 mAh"),
    ] {
        assert!(rows.iter().any(|(k, v)| *k == key && v == expected));
    }
    assert!(!rows.iter().any(|(k, _)| *k == "settings.on_battery"));
}
