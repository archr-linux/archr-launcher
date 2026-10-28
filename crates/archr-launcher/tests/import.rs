use archr_index::{Game, SystemDef};
use archr_launcher::store::Store;
use std::path::PathBuf;

#[test]
fn metadata_is_imported_once_without_resurrecting_removed_favorites() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::default();
    let games = [Game {
        path: PathBuf::from("/storage/roms/nes/Game.nes"),
        name: "Game".into(),
        image: None,
        video: None,
        favorite: true,
        year: None,
        players: None,
        last_played: Some(123),
    }];
    let system = SystemDef {
        name: "nes".into(),
        fullname: "Nintendo".into(),
        path: PathBuf::from("/storage/roms/nes"),
        extensions: vec![".nes".into()],
        emulators: vec![],
    };
    assert!(store.import_games(&system, &games));
    let key = "nes/Game.nes";
    assert!(store.is_favorite(key));
    assert_eq!(store.recent(), &[key]);
    store.toggle_favorite(key);
    store.save(dir.path()).unwrap();
    let mut store = Store::load(dir.path());
    assert!(!store.import_games(&system, &games));
    assert!(!store.is_favorite(key));
}

#[test]
fn migration_preserves_unknown_paths_and_merges_legacy_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.toml");
    let system = SystemDef {
        name: "n64".into(),
        fullname: "Nintendo 64".into(),
        path: "/storage/roms/n64".into(),
        extensions: vec![".zip".into()],
        emulators: vec![],
    };
    let cases = [
        (
            "n64//storage/roms/n64/Super Mario 64 (USA).zip",
            "n64/Super Mario 64 (USA).zip",
        ),
        ("n64//storage/roms/n64/one/Game.zip", "n64/one/Game.zip"),
        ("n64//storage/roms/n64/two/Game.zip", "n64/two/Game.zip"),
        ("n64//storage/roms/n64/link/Game.zip", "n64/link/Game.zip"),
        (
            "n64//storage/roms/n64-other/Game.zip",
            "n64//storage/roms/n64-other/Game.zip",
        ),
        (
            "missing//storage/roms/missing/Game.zip",
            "missing//storage/roms/missing/Game.zip",
        ),
    ];
    for (old, expected) in cases {
        std::fs::write(&path, format!("favorites = [{old:?}, {expected:?}]\nrecent = [{old:?}, {expected:?}]\nimported_systems = ['n64']\n[imported_times]\n{old:?} = 123\n")).unwrap();
        let mut store = Store::load(dir.path());
        assert_eq!(
            store.migrate(std::slice::from_ref(&system)),
            old != expected
        );
        assert_eq!(store.favorites(), [expected]);
        assert_eq!(store.recent(), [expected]);
        assert!(store.is_favorite(expected));
        store.save(dir.path()).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        let value: toml::Value = toml::from_str(&first).unwrap();
        assert_eq!(value["imported_times"][expected].as_integer(), Some(123));
        assert!(!store.migrate(std::slice::from_ref(&system)));
        store.save(dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        assert!(!store.import_games(&system, &[]));
        store.toggle_favorite(expected);
        assert!(store.favorites().is_empty());
    }
}
