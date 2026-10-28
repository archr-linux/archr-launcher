use std::fs;
use std::path::{Path, PathBuf};

use archr_index::{clean_name, scan_system, SystemDef};

fn system(path: &Path) -> SystemDef {
    SystemDef {
        name: "snes".into(),
        fullname: "Super Nintendo".into(),
        path: path.to_owned(),
        extensions: vec![".sfc".into(), ".smc".into()],
        emulators: Vec::new(),
        ..Default::default()
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/roms/snes")
}

fn touch(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, []).unwrap();
    path
}

#[test]
fn scan_filters_by_extension_case_insensitive() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Mario.SFC");
    touch(directory.path(), "Zelda.smc");
    touch(directory.path(), "notes.txt");
    touch(directory.path(), "Other.zip");
    let games = scan_system(&system(directory.path()));

    assert_eq!(games.len(), 2);
    assert_eq!(games[0].path, directory.path().join("Mario.SFC"));
    assert_eq!(games[1].path, directory.path().join("Zelda.smc"));
}

#[test]
fn gamelist_paths_resolved_relative_to_system() {
    let games = scan_system(&system(&fixture()));
    let game = games
        .iter()
        .find(|game| game.name == "Super Mario World Deluxe")
        .unwrap();

    assert_eq!(game.path, fixture().join("Super Mario World (USA) [!].SFC"));
    assert_eq!(game.image, Some(fixture().join("media/images/mario.png")));
    assert_eq!(game.video, Some(fixture().join("media/videos/mario.mp4")));
    assert!(game.favorite);
    assert_eq!(game.last_played, Some(1_704_164_645));
}

#[test]
fn fallback_media_then_images_folder() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    let media = touch(directory.path(), "media/images/Game.png");
    let legacy = touch(directory.path(), "images/Game-image.jpg");

    assert_eq!(
        scan_system(&system(directory.path()))[0].image,
        Some(media.clone())
    );
    fs::remove_file(media).unwrap();
    assert_eq!(
        scan_system(&system(directory.path()))[0].image,
        Some(legacy)
    );
}

#[test]
fn clean_names_preserve_text_and_nonempty_fallbacks() {
    for (input, expected) in [
        ("Super Mario World (USA) [!]", "Super Mario World"),
        ("  Game (USA) II [Rev A]  ", "Game II"),
        ("Game (Region (USA)) [a[b]]", "Game"),
        ("(USA) [!]", "(USA) [!]"),
        ("Café 日本語 [!]", "Café 日本語"),
        ("Game (unfinished", "Game (unfinished"),
    ] {
        assert_eq!(clean_name(input), expected, "{input:?}");
    }
    for input in ["", "   "] {
        assert!(!clean_name(input).is_empty());
    }
}

#[test]
fn scan_recurses_three_directories_and_skips_hidden_and_media_entries() {
    let directory = tempfile::tempdir().unwrap();
    for name in [
        "Root.sfc",
        "one/First.sfc",
        "one/two/Second.sfc",
        "one/two/three/Third.sfc",
        "one/two/three/four/Too Deep.sfc",
        ".hidden.sfc",
        ".hidden/Hidden.sfc",
        "media/Bad.sfc",
        "images/Bad.sfc",
        "videos/Bad.sfc",
        "manuals/Bad.sfc",
        "downloaded_images/Bad.sfc",
        "one/media/Bad.sfc",
    ] {
        touch(directory.path(), name);
    }
    let games = scan_system(&system(directory.path()));
    let names: Vec<_> = games.iter().map(|game| game.name.as_str()).collect();
    assert_eq!(names, ["First", "Root", "Second", "Third"]);
}

#[test]
fn extension_matching_directories_are_games_not_containers() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("Game.ps3")).unwrap();
    touch(directory.path(), "Playlist.m3u/Nested.sfc");
    let mut sys = system(directory.path());
    sys.extensions.extend([".ps3".into(), ".m3u".into()]);
    let games = scan_system(&sys);
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].path, directory.path().join("Game.ps3"));
    assert_eq!(games[1].path, directory.path().join("Playlist.m3u"));
}

#[cfg(unix)]
#[test]
fn symlinks_follow_one_hop_without_loops_or_chains() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    touch(outside.path(), "Real.sfc");
    touch(outside.path(), "nested/Other.sfc");
    symlink("../Real.sfc", outside.path().join("nested/Second Hop.sfc")).unwrap();
    symlink(
        outside.path().join("Real.sfc"),
        directory.path().join("Direct.sfc"),
    )
    .unwrap();
    symlink("Direct.sfc", directory.path().join("Chain.sfc")).unwrap();
    symlink(".", directory.path().join("loop")).unwrap();
    symlink(outside.path(), directory.path().join("linked")).unwrap();
    symlink("absent", directory.path().join("Broken.sfc")).unwrap();
    let games = scan_system(&system(directory.path()));
    let paths: Vec<_> = games
        .iter()
        .map(|game| game.path.strip_prefix(directory.path()).unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            Path::new("Direct.sfc"),
            Path::new("linked/nested/Other.sfc"),
            Path::new("linked/Real.sfc")
        ]
    );
}

#[test]
fn sort_uses_display_name_case_insensitively_then_path() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["z.sfc", "a.sfc", "b.sfc", "c.sfc"] {
        touch(directory.path(), name);
    }
    fs::write(directory.path().join("gamelist.xml"),
        "<gameList><game><path>z.sfc</path><name>alpha</name></game><game><path>a.sfc</path><name>Beta</name></game><game><path>b.sfc</path><name>ALPHA</name></game></gameList>")
        .unwrap();
    let games = scan_system(&system(directory.path()));
    let names: Vec<_> = games.iter().map(|game| game.name.as_str()).collect();
    assert_eq!(names, ["ALPHA", "alpha", "Beta", "c"]);
    assert_eq!(games[0].path, directory.path().join("b.sfc"));
}

#[test]
fn gamelist_image_thumbnail_and_video_references_precede_folder_media() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    let image = touch(directory.path(), "chosen.png");
    let thumbnail = touch(directory.path(), "thumb.jpg");
    let video = touch(directory.path(), "chosen.mp4");
    let fallback_image = touch(directory.path(), "media/images/Game.png");
    let fallback_video = touch(directory.path(), "media/videos/Game.mp4");
    fs::write(directory.path().join("gamelist.xml"), format!(
        "<gameList><game><path>{}</path><image>{}</image><thumbnail>./thumb.jpg</thumbnail><video>{}</video></game></gameList>",
        directory.path().join("Game.sfc").display(), image.display(), video.display()))
        .unwrap();
    let game = scan_system(&system(directory.path())).remove(0);
    assert_eq!(game.image, Some(image.clone()));
    assert_eq!(game.video, Some(video.clone()));
    fs::remove_file(image).unwrap();
    assert_eq!(
        scan_system(&system(directory.path()))[0].image,
        Some(thumbnail.clone())
    );
    fs::remove_file(thumbnail).unwrap();
    fs::remove_file(video).unwrap();
    let game = scan_system(&system(directory.path())).remove(0);
    assert_eq!(game.image, Some(fallback_image));
    assert_eq!(game.video, Some(fallback_video));
}

#[test]
fn folder_media_priority_and_all_supported_suffixes() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    let candidates = [
        "media/images/Game.png",
        "media/images/Game.jpg",
        "media/images/Game.jpeg",
        "media/covers/Game.png",
        "media/boxart/Game.jpg",
        "media/thumbnails/Game.jpeg",
        "images/Game.png",
        "images/Game.jpg",
        "images/Game.jpeg",
        "images/Game-image.png",
        "images/Game-image.jpg",
        "images/Game-image.jpeg",
    ];
    for candidate in candidates {
        touch(directory.path(), candidate);
    }
    for candidate in candidates {
        let expected = directory.path().join(candidate);
        assert_eq!(
            scan_system(&system(directory.path()))[0].image,
            Some(expected.clone()),
            "candidate: {candidate}"
        );
        fs::remove_file(expected).unwrap();
    }
    assert_eq!(scan_system(&system(directory.path()))[0].image, None);
}

#[test]
fn video_falls_back_to_legacy_scraper_folder() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    let media = touch(directory.path(), "media/videos/Game.mp4");
    let legacy = touch(directory.path(), "videos/Game-video.mp4");
    assert_eq!(
        scan_system(&system(directory.path()))[0].video,
        Some(media.clone())
    );
    fs::remove_file(media).unwrap();
    assert_eq!(
        scan_system(&system(directory.path()))[0].video,
        Some(legacy)
    );
}

#[test]
fn gamelist_metadata_decodes_text_ignores_unknowns_and_drops_stale_entries() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    fs::write(directory.path().join("gamelist.xml"),
        "<?xml version=\"1.0\"?><gameList><metadata><game><path>Game.sfc</path><name>Wrong</name></game></metadata><game><path>./Game.sfc</path><metadata><name>Wrong</name></metadata><name>Café &amp; <![CDATA[Games & More]]></name><favorite>false</favorite><lastplayed>invalid</lastplayed><desc>ignored</desc></game><game><path>Gone.sfc</path><name>Gone</name></game></gameList>")
        .unwrap();
    let games = scan_system(&system(directory.path()));
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].name, "Café & Games & More");
    assert!(!games[0].favorite);
    assert_eq!(games[0].last_played, None);
}

#[test]
fn empty_gamelist_name_uses_clean_name() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game (USA).sfc");
    fs::write(
        directory.path().join("gamelist.xml"),
        "<gameList><game><path>./Game (USA).sfc</path><name/></game></gameList>",
    )
    .unwrap();
    assert_eq!(scan_system(&system(directory.path()))[0].name, "Game");
}

#[test]
fn malformed_xml_is_ignored_atomically() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    let image = touch(directory.path(), "media/images/Game.png");
    for xml in [
        "",
        "<wrongRoot/>",
        "<gameList/><gameList/>",
        "<gameList><game><path>Game.sfc</path><name>Wrong</name></game>",
        "<gameList><game><path>Game.sfc</path><name>&undefined;</name></game></gameList>",
        "<gameList><game><path>Game.sfc</path><name>Wrong</name></game><game></gameList>",
        "<gameList><game><path>Game.sfc</path><name>Wrong</name></game>unexpected</gameList>",
        "<gameList><game bad=unquoted><path>Game.sfc</path><name>Wrong</name></game></gameList>",
        "<gameList><game><path>Game.sfc</path><name>Wrong</name><favorite>true</favorite><lastplayed>20240102T030405</lastplayed></game><!-- invalid -- comment --></gameList>",
        "<gameList><game><path>Game.sfc</path><name>Wrong</name><favorite>true</favorite></game><game>",
    ] {
        fs::write(directory.path().join("gamelist.xml"), xml).unwrap();
        let games = scan_system(&system(directory.path()));
        assert_eq!(games.len(), 1, "{xml}");
        assert_eq!(games[0].name, "Game", "{xml}");
        assert_eq!(games[0].image.as_ref(), Some(&image), "{xml}");
        assert!(!games[0].favorite, "{xml}");
        assert_eq!(games[0].last_played, None, "{xml}");
    }
}

#[test]
fn lastplayed_parses_utc_and_rejects_invalid_calendar_values() {
    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Game.sfc");
    for (text, expected) in [
        ("19700101T000000", Some(0)),
        ("20000229T000000", Some(951_782_400)),
        ("20240102T030405", Some(1_704_164_645)),
        ("21000301T000000", Some(4_107_542_400)),
        ("19691231T235959", None),
        ("20230229T000000", None),
        ("21000229T000000", None),
        ("20241301T000000", None),
        ("20240001T000000", None),
        ("20240100T000000", None),
        ("20240431T000000", None),
        ("20240101T240000", None),
        ("20240101T006000", None),
        ("20240101T000060", None),
        ("20240101X000000", None),
        ("2024-01-01", None),
        ("", None),
    ] {
        fs::write(directory.path().join("gamelist.xml"), format!(
            "<gameList><game><path>Game.sfc</path><lastplayed>{text}</lastplayed></game></gameList>"))
            .unwrap();
        assert_eq!(
            scan_system(&system(directory.path()))[0].last_played,
            expected,
            "timestamp: {text}"
        );
    }
}

#[cfg(unix)]
#[test]
fn gamelist_keeps_distinct_metadata_for_symlink_aliases() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    touch(directory.path(), "Real.sfc");
    symlink("Real.sfc", directory.path().join("Alias.sfc")).unwrap();
    fs::write(directory.path().join("gamelist.xml"),
        "<gameList><game><path>Real.sfc</path><name>Original</name></game><game><path>Alias.sfc</path><name>Alias</name></game></gameList>")
        .unwrap();
    let games = scan_system(&system(directory.path()));
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].name, "Alias");
    assert_eq!(games[0].path, directory.path().join("Alias.sfc"));
    assert_eq!(games[1].name, "Original");
    assert_eq!(games[1].path, directory.path().join("Real.sfc"));
}

#[test]
fn home_paths_use_home_not_the_system_parent() {
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set for this test"));
    // No environment mutation: concurrent tests can safely share the real HOME.
    let directory = tempfile::tempdir_in(&home).unwrap();
    let roms = directory.path().join("roms/snes");
    touch(&roms, "Game.sfc");
    let image = touch(directory.path(), "cover.png");
    let video = touch(directory.path(), "clip.mp4");
    let home_relative = directory.path().strip_prefix(&home).unwrap().display();
    fs::write(roms.join("gamelist.xml"), format!(
        "<gameList><game><path>~/{home_relative}/roms/snes/Game.sfc</path><name>From Home</name><image>~/{home_relative}/cover.png</image><video>~/{home_relative}/clip.mp4</video></game></gameList>"))
        .unwrap();
    let games = scan_system(&system(&roms));
    assert_eq!(games[0].name, "From Home");
    assert_eq!(games[0].image, Some(image));
    assert_eq!(games[0].video, Some(video));
}

#[test]
fn missing_system_dir_yields_empty_not_error() {
    let directory = tempfile::tempdir().unwrap();
    assert!(scan_system(&system(&directory.path().join("missing"))).is_empty());
}

#[test]
fn gamelist_supplies_optional_year_and_players_for_cover_caption() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "Kirby.sfc");
    fs::write(dir.path().join("gamelist.xml"), "<gameList><game><path>./Kirby.sfc</path><releasedate>19960321T000000</releasedate><players>1-2</players></game></gameList>").unwrap();
    let games = scan_system(&system(dir.path()));
    assert_eq!(games[0].year, Some(1996));
    assert_eq!(games[0].players.as_deref(), Some("1-2"));
    fs::write(dir.path().join("gamelist.xml"), "<gameList><game><path>./Kirby.sfc</path><releasedate>oops</releasedate><players></players></game></gameList>").unwrap();
    let games = scan_system(&system(dir.path()));
    assert_eq!(games[0].year, None);
    assert_eq!(games[0].players, None);
}

#[test]
fn oversized_gamelist_discards_all_metadata() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "Game.sfc");
    let path = dir.path().join("gamelist.xml");
    let mut source = "<gameList><game><path>Game.sfc</path><name>Imported</name><favorite>true</favorite></game></gameList>".to_owned();
    // Valid XML whitespace ensures size, not syntax, rejects the document.
    source.extend(std::iter::repeat_n(
        ' ',
        32 * 1024 * 1024 + 1 - source.len(),
    ));
    fs::write(&path, source).unwrap();
    let games = scan_system(&system(dir.path()));
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].name, "Game");
    assert!(!games[0].favorite);
}
