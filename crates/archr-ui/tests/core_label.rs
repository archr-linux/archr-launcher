mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    i18n::I18n,
    state::{App, Button, QuickItem},
};
use common::FakeData;

/// One game of `system` with `cores`, its game menu open on the core row.
fn open(system: &str, cores: &[(&str, &str)], current: (&str, &str)) -> (App, FakeData) {
    let mut data = FakeData {
        systems: vec![SystemEntry {
            name: system.into(),
            fullname: system.into(),
            game_count: 1,
            dir_mtime: 0,
            source_fingerprint: 0,
        }],
        ..Default::default()
    };
    data.games.insert(
        system.into(),
        vec![Game {
            path: "Game.iso".into(),
            name: "Game".into(),
            image: None,
            video: None,
            favorite: false,
            last_played: None,
            year: None,
            players: None,
            details: Default::default(),
        }],
    );
    let pairs = cores.iter().map(|(e, c)| (e.to_string(), c.to_string()));
    data.cores.insert(system.into(), pairs.collect());
    data.current.insert(
        (system.into(), 0),
        (current.0.to_string(), current.1.to_string()),
    );
    let mut app = App::new(8);
    app.handle(Button::A, &data);
    app.handle(Button::Y, &data);
    app.cursor = app
        .quick_menu_items(&data)
        .iter()
        .position(|i| *i == QuickItem::Core)
        .expect("a core row");
    (app, data)
}

fn english() -> I18n {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales");
    I18n::load(&dir, Some("en"), None)
}

#[test]
fn a_core_says_whether_it_is_retroarch_or_standalone_when_a_system_has_both() {
    let catalog = english();
    let psp = [("ppsspp", "ppsspp"), ("retroarch", "ppsspp")];
    let (mut app, data) = open("psp", &psp, ("ppsspp", "ppsspp"));
    assert_eq!(app.core_label(&catalog), "ppsspp (standalone)");
    app.handle(Button::R, &data);
    assert_eq!(app.core_label(&catalog), "ppsspp (RetroArch)");
}

#[test]
fn a_system_with_one_kind_shows_the_core_alone() {
    let catalog = english();
    let snes = [("retroarch", "snes9x"), ("retroarch", "snes9x2010")];
    let (app, _) = open("snes", &snes, ("retroarch", "snes9x"));
    assert_eq!(app.core_label(&catalog), "snes9x");
}
