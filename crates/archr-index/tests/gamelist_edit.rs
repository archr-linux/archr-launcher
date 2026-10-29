use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use archr_index::{scan_system, update_gamelist, GameUpdate, SystemDef};

fn system(path: &Path) -> SystemDef {
    SystemDef {
        name: "snes".into(),
        fullname: "Super Nintendo".into(),
        path: path.to_owned(),
        extensions: vec![".sfc".into()],
        ..Default::default()
    }
}

fn update(rom: &Path, fields: &[(&'static str, &str)]) -> GameUpdate {
    GameUpdate {
        rom: rom.to_owned(),
        fields: fields.iter().map(|(t, v)| (*t, (*v).to_owned())).collect(),
    }
}

const EXISTING: &str = r#"<?xml version="1.0"?>
<!-- edited by hand -->
<gameList>
  <folder><path>./hacks</path><name>Hacks</name></folder>
  <game id="7" source="ScreenScraper">
    <path>./Chrono.sfc</path>
    <name>My Chrono</name>
    <desc></desc>
    <favorite>true</favorite>
    <custom><nested a="1">kept</nested></custom>
    <thumbnail>./missing.png</thumbnail>
  </game>
  <game><path>./Other.sfc</path><playcount>4</playcount></game>
</gameList>
"#;

#[test]
fn merge_preserves_other_games_unknown_fields_and_user_edits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in ["Chrono.sfc", "Other.sfc", "New Game.sfc"] {
        fs::write(root.join(name), []).unwrap();
    }
    fs::create_dir_all(root.join("media/images")).unwrap();
    fs::write(root.join("media/images/Chrono.png"), []).unwrap();
    fs::write(root.join("gamelist.xml"), EXISTING).unwrap();
    fs::set_permissions(root.join("gamelist.xml"), fs::Permissions::from_mode(0o664)).unwrap();

    let cover = root.join("media/images/Chrono.png");
    update_gamelist(
        root,
        &[
            update(
                &root.join("Chrono.sfc"),
                &[
                    ("name", "Chrono Trigger"),
                    ("desc", "Time <travel> & more"),
                    ("genre", "RPG"),
                    ("thumbnail", cover.to_str().unwrap()),
                ],
            ),
            update(
                &root.join("New Game.sfc"),
                &[("name", "New"), ("rating", "0.8")],
            ),
        ],
        false,
    )
    .unwrap();

    let text = fs::read_to_string(root.join("gamelist.xml")).unwrap();
    // Untouched content survives verbatim.
    for kept in [
        "<!-- edited by hand -->",
        "<folder><path>./hacks</path><name>Hacks</name></folder>",
        "<game id=\"7\" source=\"ScreenScraper\">",
        "<name>My Chrono</name>",
        "<favorite>true</favorite>",
        "<custom><nested a=\"1\">kept</nested></custom>",
        "<playcount>4</playcount>",
    ] {
        assert!(text.contains(kept), "{kept} missing from\n{text}");
    }
    // Empty fields and paths to missing files are filled; edits are not.
    assert!(!text.contains("Chrono Trigger"));
    assert!(text.contains("<desc>Time &lt;travel&gt; &amp; more</desc>"));
    assert!(text.contains("<genre>RPG</genre>"));
    assert!(text.contains("<thumbnail>./media/images/Chrono.png</thumbnail>"));
    assert!(!text.contains("missing.png"));
    assert!(text.contains("<path>./New Game.sfc</path>"));
    assert_eq!(
        fs::metadata(root.join("gamelist.xml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o664
    );

    // The result is a gamelist the index reads back.
    let games = scan_system(&system(root));
    let chrono = games.iter().find(|g| g.name == "My Chrono").unwrap();
    assert!(chrono.favorite);
    assert_eq!(chrono.details.description, "Time <travel> & more");
    assert_eq!(chrono.image, Some(cover));
    let new = games.iter().find(|g| g.name == "New").unwrap();
    assert_eq!(new.details.rating, Some(80));
}

#[test]
fn forced_merge_overwrites_and_keeps_one_field_per_tag() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("Chrono.sfc"), []).unwrap();
    fs::write(root.join("gamelist.xml"), EXISTING).unwrap();
    let rom = root.join("Chrono.sfc");
    update_gamelist(root, &[update(&rom, &[("name", "Chrono Trigger")])], true).unwrap();
    update_gamelist(root, &[update(&rom, &[("name", "Chrono Trigger")])], true).unwrap();
    let text = fs::read_to_string(root.join("gamelist.xml")).unwrap();
    assert_eq!(text.matches("<name>").count(), 2, "{text}");
    assert!(text.contains("<name>Chrono Trigger</name>"));
    assert!(text.contains("<name>Hacks</name>"));
}

#[test]
fn creates_a_missing_gamelist_and_refuses_to_replace_a_broken_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let rom = root.join("sub/Game.sfc");
    update_gamelist(root, &[update(&rom, &[("name", "Game")])], false).unwrap();
    let text = fs::read_to_string(root.join("gamelist.xml")).unwrap();
    assert!(text.starts_with("<?xml"));
    assert!(text.contains("<path>./sub/Game.sfc</path>"), "{text}");
    assert!(text.trim_end().ends_with("</gameList>"));

    let broken = "<gameList><game><path>./Game.sfc</path>";
    fs::write(root.join("gamelist.xml"), broken).unwrap();
    assert!(update_gamelist(root, &[update(&rom, &[("name", "Game")])], false).is_err());
    assert_eq!(
        fs::read_to_string(root.join("gamelist.xml")).unwrap(),
        broken
    );
}

#[test]
fn writes_are_atomic_and_leave_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("gamelist.xml"), "<gameList/>").unwrap();
    let before = fs::metadata(root.join("gamelist.xml")).unwrap();
    let rom = root.join("Game.sfc");
    update_gamelist(root, &[update(&rom, &[("genre", "RPG")])], false).unwrap();
    // A rename publishes a new inode instead of rewriting the old file in place.
    use std::os::unix::fs::MetadataExt;
    let after = fs::metadata(root.join("gamelist.xml")).unwrap();
    assert_ne!(before.ino(), after.ino());
    let names: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["gamelist.xml"]);
    assert!(fs::read_to_string(root.join("gamelist.xml"))
        .unwrap()
        .contains("<genre>RPG</genre>"));
}
