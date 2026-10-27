use archr_index::{cache::load_games_with_mapper, scan_system_with_mapper, SystemDef};
use std::{fs, path::Path};
#[test]
fn root_is_applied_before_absolute_gamelist_rom_and_media_checks() {
    let root = tempfile::tempdir().unwrap();
    let roms = root.path().join("storage/roms/nes");
    fs::create_dir_all(&roms).unwrap();
    fs::create_dir_all(root.path().join("storage/art")).unwrap();
    fs::write(roms.join("A.nes"), []).unwrap();
    let image = root.path().join("storage/art/SPECIAL.png");
    fs::write(&image, []).unwrap();
    fs::write(roms.join("gamelist.xml"), "<gameList><game><path>/storage/roms/nes/A.nes</path><name>Special</name><image>/storage/art/SPECIAL.png</image></game></gameList>").unwrap();
    let sys = SystemDef {
        name: "nes".into(),
        fullname: "Nintendo".into(),
        path: roms,
        extensions: vec![".nes".into()],
        emulators: vec![],
    };
    let mapper = |path: &Path| {
        if path.is_absolute() && !path.starts_with(root.path()) {
            root.path().join(path.strip_prefix("/").unwrap())
        } else {
            path.to_owned()
        }
    };
    let games = scan_system_with_mapper(&sys, &mapper);
    assert_eq!(games[0].name, "Special");
    assert_eq!(games[0].image.as_ref(), Some(&image));
    let cache = root.path().join("cache");
    assert_eq!(load_games_with_mapper(&cache, &sys, &mapper), games);
    assert_eq!(load_games_with_mapper(&cache, &sys, &mapper), games);
}
