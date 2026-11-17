//! The shelves (`Screen::Home`): what they hold and how the buttons move.
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    layout::{ShelfKind, Tab},
    resume::Recent,
    state::{App, Button, Item, Outcome, Screen},
    Theme,
};
use common::FakeData;

fn game(name: &str) -> Game {
    Game {
        name: name.into(),
        path: format!("/roms/{name}.rom").into(),
        image: None,
        video: None,
        favorite: false,
        year: None,
        players: None,
        last_played: None,
        details: Default::default(),
    }
}
fn library() -> FakeData {
    let mut data = FakeData::default();
    for (name, count) in [("snes", 6), ("gba", 3), ("n64", 2)] {
        data.systems.push(SystemEntry {
            name: name.into(),
            fullname: name.to_uppercase(),
            game_count: count,
            dir_mtime: 0,
            source_fingerprint: 0,
        });
        data.games.insert(
            name.into(),
            (0..count).map(|n| game(&format!("{name}{n}"))).collect(),
        );
    }
    // Two favorites: one on the SNES, one on the N64.
    data.games
        .insert("@favorites".into(), vec![game("snes2"), game("n641")]);
    data.origins
        .insert(("@favorites".into(), 0), ("snes".into(), 2));
    data.origins
        .insert(("@favorites".into(), 1), ("n64".into(), 1));
    data
}
fn theme(source: &str) -> Theme {
    Theme::parse(&toml::from_str(source).unwrap())
}
/// An app on a view with shelves, and two recent games.
fn shelves() -> App {
    let theme = theme("[view.lista]\nhome = true");
    let mut app = App::new(5);
    app.set_view(&theme.view("lista"));
    app.recent = (0..2)
        .map(|n| Recent {
            system: "snes".into(),
            game: n,
            name: format!("snes{n}"),
            entry: Some("--entryslot=auto".into()),
            ..Recent::default()
        })
        .collect();
    app
}
fn at(app: &App) -> (Tab, usize, usize) {
    match app.screen {
        Screen::Home { tab, shelf, item } => (tab, shelf, item),
        ref other => panic!("not on the shelves: {other:?}"),
    }
}

#[test]
fn a_view_with_home_rests_on_the_shelves_and_one_without_on_the_systems() {
    let mut app = shelves();
    assert_eq!(at(&app), (Tab::Home, 0, 0));
    assert!(app.at_root());
    app.set_view(&Theme::builtin().view("lista"));
    assert_eq!(app.screen, Screen::Systems, "no shelves: back to the list");
    app.set_view(&theme("[view.lista]\nhome = true").view("lista"));
    assert_eq!(at(&app), (Tab::Home, 0, 0));
}

#[test]
fn shelves_hold_what_there_is_and_empty_ones_go() {
    let data = library();
    let mut app = shelves();
    let home = app.shelves(Tab::Home, &data);
    let kinds: Vec<_> = home.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [ShelfKind::Continue, ShelfKind::Systems],
        "no apps: no shelf"
    );
    assert_eq!(home[0].items, [Item::Recent(0), Item::Recent(1)]);
    assert_eq!(home[1].items[0], Item::System("snes".into()));
    let favorites = app.shelves(Tab::Favorites, &data);
    let systems: Vec<_> = favorites.iter().map(|s| s.system.as_deref()).collect();
    assert_eq!(systems, [Some("snes"), Some("n64")], "one shelf per system");
    assert_eq!(app.shelves(Tab::Library, &data)[0].items.len(), 3);
    app.recent.clear();
    assert_eq!(app.shelves(Tab::Home, &data)[0].kind, ShelfKind::Systems);
}

#[test]
fn arrows_walk_a_shelf_up_and_down_change_shelf_shoulders_change_tab() {
    let data = library();
    let mut app = shelves();
    app.handle(Button::Right, &data);
    assert_eq!(at(&app), (Tab::Home, 0, 1));
    app.handle(Button::Right, &data);
    assert_eq!(at(&app), (Tab::Home, 0, 1), "the end of the shelf");
    app.handle(Button::Down, &data);
    assert_eq!(at(&app), (Tab::Home, 1, 1), "the focus keeps its place");
    app.handle(Button::Down, &data);
    assert_eq!(at(&app), (Tab::Home, 1, 1), "the last shelf");
    app.handle(Button::Left, &data);
    app.handle(Button::Up, &data);
    assert_eq!(at(&app), (Tab::Home, 0, 0));
    app.handle(Button::R, &data);
    assert_eq!(at(&app), (Tab::Library, 0, 0));
    // The Library is a grid of three columns: ↓ goes a row, as far as there is one.
    app.handle(Button::Right, &data);
    app.handle(Button::Down, &data);
    assert_eq!(at(&app), (Tab::Library, 0, 1));
    app.handle(Button::R, &data);
    app.handle(Button::R, &data);
    assert_eq!(at(&app).0, Tab::Favorites, "no tab past the last");
    app.handle(Button::L, &data);
    assert_eq!(at(&app).0, Tab::Library);
    // B rests here.
    app.handle(Button::B, &data);
    assert_eq!(at(&app).0, Tab::Library);
}

#[test]
fn a_opens_and_plays_and_screens_opened_here_come_back_here() {
    let data = library();
    let mut app = shelves();
    // A recent game continues where it was left, as the switcher does.
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Continue {
            system: "snes".into(),
            game: 0,
            entry: Some("--entryslot=auto".into()),
        }
    );
    // A system opens its games; B comes back to the same place.
    app.handle(Button::Down, &data);
    app.handle(Button::Right, &data);
    app.handle(Button::A, &data);
    assert_eq!(app.screen, Screen::games("gba"));
    app.handle(Button::B, &data);
    assert_eq!(at(&app), (Tab::Home, 1, 1));
    // A favorite plays; Y opens its game menu, and B closes it back here.
    app.handle(Button::R, &data);
    app.handle(Button::R, &data);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Launch {
            system: "snes".into(),
            game: 2,
        }
    );
    app.handle(Button::Y, &data);
    assert!(matches!(app.screen, Screen::QuickMenu { .. }));
    assert!(matches!(app.backdrop().0, Screen::Home { .. }));
    app.handle(Button::B, &data);
    assert_eq!(at(&app).0, Tab::Favorites);
    // Start opens the Start menu, which closes back to the shelves.
    app.handle(Button::Start, &data);
    assert!(matches!(app.screen, Screen::StartHub { .. }));
    app.handle(Button::B, &data);
    assert_eq!(at(&app).0, Tab::Favorites);
}

#[test]
fn a_games_grid_steps_a_game_a_row_and_a_system() {
    let data = library();
    let theme = theme(
        "[view.lista]\nhome = true\n\
         [view.lista.games]\nlist = { mark = 'grid', columns = 4 }\n\
         strip = { x = 0.05, y = 0.22, w = 0.9, h = 0.06 }",
    );
    let mut app = App::new(5);
    app.set_view(&theme.view("lista"));
    app.handle(Button::Down, &data);
    app.handle(Button::A, &data);
    assert_eq!(app.screen, Screen::games("snes"));
    app.handle(Button::Right, &data);
    assert_eq!(app.cursor, 1);
    app.handle(Button::Down, &data);
    assert_eq!(app.cursor, 5, "a row is four games");
    app.handle(Button::Down, &data);
    assert_eq!(app.cursor, 5, "no row below");
    app.handle(Button::Up, &data);
    assert_eq!(app.cursor, 1);
    app.handle(Button::R, &data);
    assert_eq!((app.screen.clone(), app.cursor), (Screen::games("gba"), 0));
    app.handle(Button::B, &data);
    assert!(
        matches!(app.screen, Screen::Home { .. }),
        "back to the shelves"
    );
}

#[test]
fn the_recent_menu_offers_to_forget_and_the_paths_follow_the_shelves() {
    let mut data = library();
    let mut app = shelves();
    app.recent[0].picture = Some("/shots/snes0.png".into());
    data.games.get_mut("snes").unwrap()[2].image = Some("/covers/snes2.png".into());
    let (covers, pictures) = app.home_paths(&data);
    assert_eq!(pictures, [std::path::PathBuf::from("/shots/snes0.png")]);
    assert!(covers.is_empty(), "the Home tab here shows no game covers");
    app.handle(Button::R, &data);
    app.handle(Button::R, &data);
    let (covers, _) = app.home_paths(&data);
    assert_eq!(covers, [std::path::PathBuf::from("/covers/snes2.png")]);
    app.handle(Button::L, &data);
    app.handle(Button::L, &data);
    app.handle(Button::Y, &data);
    let items = app.quick_menu_items(&data);
    assert!(items.contains(&archr_ui::state::QuickItem::ForgetRecent));
}
