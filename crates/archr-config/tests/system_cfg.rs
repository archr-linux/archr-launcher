use archr_config::{rom_key, SystemCfg};
use std::fs;
use std::io::Read;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn get_prefers_game_over_system_over_global() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    fs::write(
        &path,
        "global.core=a\nsnes.core=b\nsnes[\"x.sfc\"].core=c\n",
    )
    .unwrap();

    let cfg = SystemCfg::load(&path).unwrap();
    assert_eq!(cfg.get("snes", Some("x.sfc"), "core"), Some("c".into()));
    assert_eq!(cfg.get("snes", None, "core"), Some("b".into()));
    assert_eq!(cfg.get("snes", Some("other.sfc"), "core"), Some("b".into()));
    assert_eq!(cfg.get("nes", None, "core"), Some("a".into()));
    assert_eq!(cfg.get("nes", Some("x.nes"), "core"), Some("a".into()));
    assert_eq!(cfg.get("nes", None, "emulator"), None);
}

#[test]
fn set_preserves_other_lines_and_order() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(
        &path,
        "# Keep this comment\r\n\r\nglobal.core=a\nsnes.core=old\r\nunknown line\nnes.core=z",
    )
    .unwrap();

    SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# Keep this comment\r\n\r\nglobal.core=a\nsnes.core=new\r\nunknown line\nnes.core=z"
    );
}

#[test]
fn set_rereads_under_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(&path, "snes.core=old\n").unwrap();
    let snapshot = SystemCfg::load(&path).unwrap();

    let status = Command::new("sh")
        .arg("-c")
        .arg("printf 'nes.core=other-process\n' >> \"$1\"")
        .arg("writer")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());

    SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();

    assert_eq!(snapshot.get("snes", None, "core"), Some("old".into()));
    assert_eq!(snapshot.get("nes", None, "core"), None);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "snes.core=new\nnes.core=other-process\n"
    );
}

#[test]
fn rom_key_quotes_name() {
    assert_eq!(
        rom_key("snes", "Chrono Trigger (USA).sfc", "core"),
        "snes[\"Chrono Trigger (USA).sfc\"].core"
    );
}

#[test]
fn stale_lock_with_nonexistent_pid_is_taken_over() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    assert!(!std::path::Path::new("/proc/4294967295").exists());
    fs::write(&path, "snes.core=old\n").unwrap();
    fs::write(&lock, "4294967295\n").unwrap();

    SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=new\n");
    assert!(!lock.exists());
}

#[test]
fn old_invalid_pid_lock_is_taken_over() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(&path, "snes.core=old\n").unwrap();
    for contents in [b"not-a-pid\n".as_slice(), b"", b"\xff"] {
        fs::write(&lock, contents).unwrap();
        fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(5))
            .unwrap();
        SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();
        assert!(!lock.exists());
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=new\n");
}

#[test]
fn lock_file_is_removed_after_set_and_save() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(&path, "snes.core=old\n").unwrap();

    SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();

    assert!(!lock.exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn lock_file_is_removed_on_load_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.cfg");
    let lock = dir.path().join(".system.cfg.lock");

    let error = SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(!lock.exists());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn set_appends_missing_key_after_unterminated_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(&path, "# unchanged\nsnes.core=old").unwrap();

    SystemCfg::set_and_save(&path, &lock, "nes.core", "new").unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# unchanged\nsnes.core=old\nnes.core=new\n"
    );
}

#[test]
fn keys_are_literal_and_values_can_contain_equals() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(
        &path,
        "snes[\"a=b[1].sfc\"].core=old\nsnes[\"axb1.sfc\"].core=other\n",
    )
    .unwrap();

    SystemCfg::set_and_save(&path, &lock, "snes[\"a=b[1].sfc\"].core", "x=y").unwrap();

    let cfg = SystemCfg::load(&path).unwrap();
    assert_eq!(
        cfg.get("snes", Some("a=b[1].sfc"), "core"),
        Some("x=y".into())
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "snes[\"a=b[1].sfc\"].core=x=y\nsnes[\"axb1.sfc\"].core=other\n"
    );
}

#[test]
fn set_replaces_file_instead_of_truncating_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(&path, "snes.core=old\n").unwrap();
    let mut old_file = fs::File::open(&path).unwrap();

    SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();

    let mut old_contents = String::new();
    old_file.read_to_string(&mut old_contents).unwrap();
    assert_eq!(old_contents, "snes.core=old\n");
    assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=new\n");
}

#[cfg(unix)]
#[test]
fn set_preserves_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join(".system.cfg.lock");
    fs::write(&path, "snes.core=old\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

    SystemCfg::set_and_save(&path, &lock, "snes.core", "new").unwrap();

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}
