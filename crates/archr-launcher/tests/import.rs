use archr_index::Game;
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
    assert!(store.import_games("nes", &games));
    let key = "nes//storage/roms/nes/Game.nes";
    assert!(store.is_favorite(key));
    assert_eq!(store.recent(), &[key]);
    store.toggle_favorite(key);
    store.save(dir.path()).unwrap();
    let mut store = Store::load(dir.path());
    assert!(!store.import_games("nes", &games));
    assert!(!store.is_favorite(key));
}
