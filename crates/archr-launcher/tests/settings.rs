use archr_launcher::{paths::Paths, settings::Backend};
use archr_ui::settings::Id;
use std::{fs, os::unix::fs::PermissionsExt};
use tempfile::{tempdir, TempDir};
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
fn suspend_sees_old_value_and_other_scripts_see_committed_value() {
    let (dir, backend) = fixture();
    script(&dir, "suspendmode", "case \"$(/bin/cat \"$ARCHR_LAUNCHER_ROOT/storage/.config/system/configs/system.cfg\")\" in *system.suspendmode=off*) exit 0;; *) exit 23;; esac");
    assert_eq!(backend.apply(Id::Suspend, "mem").unwrap(), "mem");
    script(&dir, "volume", "case \"$(/bin/cat \"$ARCHR_LAUNCHER_ROOT/storage/.config/system/configs/system.cfg\")\" in *audio.volume=65*) exit 0;; *) exit 24;; esac");
    backend.apply(Id::Volume, "65").unwrap();
}
#[test]
fn failure_reports_status_and_stderr_and_restores_value() {
    let (dir, backend) = fixture();
    script(&dir, "volume", "echo 'sink unavailable' >&2; exit 7");
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
    script(&dir, "volume", "/bin/sleep 0.2; exit 0");
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
    assert_eq!(runtime.state.pending, Some(Id::Volume));
    assert_eq!(runtime.state.value(Id::Volume), "60");
    runtime.handle(Button::B);
    assert_eq!(runtime.state.page(), Page::Root);
    while runtime.busy() {
        runtime.poll();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(runtime.state.value(Id::Volume), "65");
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
    fs::write(backlight.join("brightness"), "64").unwrap();
    fs::write(backlight.join("max_brightness"), "255").unwrap();
    let state = backend.load().unwrap();
    assert_eq!(state.value(Id::Volume), "50");
    assert_eq!(state.value(Id::Brightness), "25");
}
