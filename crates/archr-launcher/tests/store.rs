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
fn load_deduplicates_in_order_and_caps_recent_at_30() {
    let dir = tempdir().unwrap();
    let mut keys = vec![
        "snes/game0.zip".to_owned(),
        "snes/game1.zip".to_owned(),
        "snes/game0.zip".to_owned(),
        "snes/game1.zip".to_owned(),
    ];
    keys.extend((2..31).map(|n| format!("snes/game{n}.zip")));
    assert_eq!(keys.len(), 33);
    let list = keys
        .iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        dir.path().join("store.toml"),
        format!("favorites = [{list}]\nrecent = [{list}]\n"),
    )
    .unwrap();

    let store = Store::load(dir.path());
    let expected_favorites: Vec<_> = (0..31).map(|n| format!("snes/game{n}.zip")).collect();
    let expected_recent: Vec<_> = (0..30).map(|n| format!("snes/game{n}.zip")).collect();
    assert_eq!(store.favorites(), expected_favorites);
    assert_eq!(store.recent(), expected_recent);
    store.save(dir.path()).unwrap();
    let reloaded = Store::load(dir.path());
    assert_eq!(reloaded.favorites(), expected_favorites);
    assert_eq!(reloaded.recent(), expected_recent);
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

#[test]
fn invalid_loaded_store_cannot_be_overwritten() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.toml");
    for source in [
        b"not TOML [".to_vec(),
        vec![0xff],
        vec![b' '; 256 * 1024 + 1],
    ] {
        fs::write(&path, &source).unwrap();
        let mut store = Store::load(dir.path());
        store.toggle_favorite("nes/new.zip");
        assert!(store.save(dir.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), source);
    }
}

#[test]
fn save_accepts_exactly_256_kib_and_rejects_further_growth() {
    let dir = tempdir().unwrap();
    let mut store = Store::default();
    store.toggle_favorite("x");
    let overhead = toml::to_string(&store).unwrap().len() - 1;
    store.toggle_favorite("x");
    store.toggle_favorite(&"x".repeat(256 * 1024 - overhead));
    store.save(dir.path()).unwrap();
    let path = dir.path().join("store.toml");
    let previous = fs::read(&path).unwrap();
    assert_eq!(previous.len(), 256 * 1024);
    assert_eq!(Store::load(dir.path()).favorites(), store.favorites());
    store.toggle_favorite("one more");
    assert_eq!(
        store.save(dir.path()).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(fs::read(path).unwrap(), previous);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
