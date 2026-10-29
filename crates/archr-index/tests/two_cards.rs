//! A system read from two cards: /storage/roms/<system> and the second
//! card's folder, each with its own gamelist and media.
use std::fs;
use std::path::{Path, PathBuf};

use archr_index::cache::{build_overview, load_games};
use archr_index::{scan_system, update_gamelist, GameUpdate, SystemDef};

fn touch(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, []).unwrap();
    path
}

/// (tempdir, system) with `sd1/snes` and `sd2/snes` as its two roots.
fn two_cards() -> (tempfile::TempDir, SystemDef) {
    let dir = tempfile::tempdir().unwrap();
    let sys = SystemDef {
        name: "snes".into(),
        path: dir.path().join("sd1/snes"),
        external: Some(dir.path().join("sd2/snes")),
        extensions: vec![".sfc".into()],
        ..Default::default()
    };
    (dir, sys)
}

#[test]
fn both_cards_are_listed_with_their_own_gamelist_and_media() {
    let (_dir, sys) = two_cards();
    let sd2 = sys.external.clone().unwrap();
    touch(&sys.path, "Alpha.sfc");
    touch(&sd2, "Mario (Ü) [!].sfc");
    touch(&sd2, "media/images/Mario (Ü) [!].png");
    fs::write(
        sd2.join("gamelist.xml"),
        "<gameList><game><path>./Mario (Ü) [!].sfc</path><name>Super Mario World</name></game></gameList>",
    )
    .unwrap();

    let games = scan_system(&sys);

    assert_eq!(games.len(), 2);
    assert_eq!(games[0].name, "Alpha");
    assert_eq!(games[1].name, "Super Mario World");
    assert_eq!(
        games[1].image,
        Some(sd2.join("media/images/Mario (Ü) [!].png"))
    );
    // Each game knows its card.
    assert_eq!(sys.root_of(&games[0].path), sys.path);
    assert_eq!(sys.root_of(&games[1].path), sd2);
}

#[test]
fn a_rom_on_both_cards_is_listed_once_from_the_first() {
    let (_dir, sys) = two_cards();
    let sd2 = sys.external.clone().unwrap();
    touch(&sys.path, "sub/Same.sfc");
    touch(&sd2, "sub/Same.sfc");
    touch(&sd2, "Only2.sfc");

    let games = scan_system(&sys);

    let paths: Vec<_> = games.iter().map(|game| game.path.clone()).collect();
    assert_eq!(
        paths,
        [sd2.join("Only2.sfc"), sys.path.join("sub/Same.sfc")]
    );
}

#[test]
fn a_missing_card_or_folder_is_just_empty() {
    let (_dir, mut sys) = two_cards();
    touch(&sys.path, "Alpha.sfc");
    // The second card's folder for this system does not exist.
    assert_eq!(scan_system(&sys).len(), 1);
    // Only the second card has the system.
    let sd2 = sys.external.clone().unwrap();
    touch(&sd2, "Beta.sfc");
    fs::remove_dir_all(&sys.path).unwrap();
    assert_eq!(scan_system(&sys)[0].path, sd2.join("Beta.sfc"));
    // Card pulled out: back to the first card alone.
    sys.external = None;
    assert!(scan_system(&sys).is_empty());
}

#[test]
fn the_cache_tells_the_cards_apart() {
    let (dir, mut sys) = two_cards();
    let cache = dir.path().join("cache");
    let sd2 = sys.external.take().unwrap();
    touch(&sys.path, "Alpha.sfc");
    touch(&sd2, "Beta.sfc");

    assert_eq!(load_games(&cache, &sys).len(), 1);
    // The card arrives: the first card's cached list is not reused.
    sys.external = Some(sd2.clone());
    assert_eq!(load_games(&cache, &sys).len(), 2);
    assert_eq!(
        build_overview(std::slice::from_ref(&sys)).systems[0].game_count,
        2
    );
    // A change on the second card alone invalidates it too.
    touch(&sd2, "Gamma.sfc");
    assert_eq!(load_games(&cache, &sys).len(), 3);
    // And it leaves again.
    sys.external = None;
    assert_eq!(load_games(&cache, &sys).len(), 1);
}

#[test]
fn gamelist_updates_go_to_the_card_holding_the_rom() {
    let (_dir, sys) = two_cards();
    let sd2 = sys.external.clone().unwrap();
    touch(&sys.path, "Alpha.sfc");
    let rom = touch(&sd2, "Beta.sfc");
    let cover = touch(&sd2, "media/images/Beta.png");

    let root = sys.root_of(&rom);
    let update = GameUpdate {
        rom: rom.clone(),
        fields: vec![
            ("name", "Beta Quest".into()),
            ("image", cover.display().to_string()),
        ],
    };
    update_gamelist(root, &[update], false).unwrap();

    assert!(!sys.path.join("gamelist.xml").exists());
    let xml = fs::read_to_string(sd2.join("gamelist.xml")).unwrap();
    assert!(xml.contains("<path>./Beta.sfc</path>"), "{xml}");
    assert!(
        xml.contains("<image>./media/images/Beta.png</image>"),
        "{xml}"
    );
    let games = scan_system(&sys);
    assert_eq!(games[1].name, "Beta Quest");
    assert_eq!(games[1].image, Some(cover));
}
