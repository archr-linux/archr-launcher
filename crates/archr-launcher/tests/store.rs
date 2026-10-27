use std::{fs, io::Read};

use archr_launcher::store::Store;
use tempfile::tempdir;

#[test]
fn favorites_roundtrip() {
    let root = tempdir().unwrap();
    let dir = root.path().join("config/launcher");
    let mut store = Store::load(&dir);
    let relative = "snes/folder/Tom's Game.zip";
    let absolute = "nes//storage/roms/nes/Game.zip";
    store.toggle_favorite(relative);
    store.toggle_favorite(absolute);
    assert!(store.is_favorite(relative));
    store.toggle_favorite(relative);
    assert!(!store.is_favorite(relative));
    store.push_recent(relative);
    store.push_recent(absolute);
    store.save(&dir).unwrap();

    let loaded = Store::load(&dir);
    assert_eq!(loaded.favorites(), &[absolute]);
    assert_eq!(loaded.recent(), &[absolute, relative]);
    let data: toml::Table =
        toml::from_str(&fs::read_to_string(dir.join("store.toml")).unwrap()).unwrap();
    assert_eq!(
        data["favorites"].as_array().unwrap()[0].as_str(),
        Some(absolute)
    );
    assert_eq!(data["recent"].as_array().unwrap().len(), 2);
}

#[test]
fn recent_dedups_and_caps_at_30() {
    let dir = tempdir().unwrap();
    let mut store = Store::load(dir.path());
    for n in 0..35 {
        store.push_recent(&format!("snes/game{n}.zip"));
    }
    store.push_recent("snes/game20.zip");
    assert_eq!(store.recent().len(), 30);
    assert_eq!(store.recent()[0], "snes/game20.zip");
    assert_eq!(store.recent()[1], "snes/game34.zip");
    assert_eq!(store.recent().last().unwrap(), "snes/game5.zip");
    assert_eq!(
        store
            .recent()
            .iter()
            .filter(|k| *k == "snes/game20.zip")
            .count(),
        1
    );
    store.save(dir.path()).unwrap();
    assert_eq!(Store::load(dir.path()).recent(), store.recent());
}

#[test]
fn missing_invalid_oversized_and_nonregular_store_are_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.toml");
    let missing = Store::load(dir.path());
    assert!(missing.favorites().is_empty());
    assert!(missing.recent().is_empty());
    for source in [
        b"not TOML [".to_vec(),
        vec![0xff],
        b"favorites = 3\nrecent = []".to_vec(),
        vec![b' '; 256 * 1024 + 1],
    ] {
        fs::write(&path, source).unwrap();
        let loaded = Store::load(dir.path());
        assert!(loaded.favorites().is_empty());
        assert!(loaded.recent().is_empty());
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(Store::load(dir.path()).favorites().is_empty());
}

#[test]
fn store_enforces_size_limit_at_boundary() {
    let dir = tempdir().unwrap();
    let mut source = "favorites = ['snes/game.zip']\nrecent = []\n".to_string();
    source.extend(std::iter::repeat_n(' ', 256 * 1024 - source.len()));
    fs::write(dir.path().join("store.toml"), &source).unwrap();
    assert!(Store::load(dir.path()).is_favorite("snes/game.zip"));
    source.push(' ');
    fs::write(dir.path().join("store.toml"), source).unwrap();
    assert!(Store::load(dir.path()).favorites().is_empty());
}

#[test]
fn save_replaces_without_truncating_open_old_store() {
    let dir = tempdir().unwrap();
    let mut store = Store::load(dir.path());
    store.toggle_favorite("snes/old.zip");
    store.save(dir.path()).unwrap();
    let mut old = fs::File::open(dir.path().join("store.toml")).unwrap();
    store.toggle_favorite("snes/new.zip");
    store.save(dir.path()).unwrap();
    let mut old_source = String::new();
    old.read_to_string(&mut old_source).unwrap();
    assert!(!old_source.contains("new.zip"));
    assert!(Store::load(dir.path()).is_favorite("snes/new.zip"));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn save_errors_clean_up_temporary_file() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("store.toml")).unwrap();
    assert!(Store::load(dir.path()).save(dir.path()).is_err());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn store_fifo_is_rejected_without_opening() {
    let dir = tempdir().unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(dir.path().join("store.toml"))
        .status()
        .unwrap()
        .success());
    let store = Store::load(dir.path());
    assert!(store.favorites().is_empty());
    assert!(store.recent().is_empty());
}
