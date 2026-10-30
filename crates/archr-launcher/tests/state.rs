#[path = "../../archr-ui/tests/common/mod.rs"]
mod common;
use common::FakeData;

use archr_index::{cache::SystemEntry, Game};
use archr_launcher::state::{App, Button, Outcome, QuickItem, Screen, SessionAction};

impl FakeData {
    fn library() -> Self {
        let mut data = Self::default();
        for (name, count) in [("nes", 3), ("empty", 0), ("snes", 12)] {
            data.systems.push(SystemEntry {
                name: name.into(),
                fullname: name.to_uppercase(),
                game_count: count,
                dir_mtime: 0,
                source_fingerprint: 0,
            });
            data.games.insert(name.into(), games(count as usize));
        }
        data.cores.insert(
            "snes".into(),
            vec![pair("retroarch", "snes9x"), pair("standalone", "native")],
        );
        data.current
            .insert(("snes".into(), 7), pair("retroarch", "snes9x"));
        data
    }

    fn collections(&mut self) {
        self.games.insert("@favorites".into(), games(2));
        self.games.insert("@recent".into(), games(1));
        self.origins
            .insert(("@favorites".into(), 0), ("snes".into(), 7));
        self.origins
            .insert(("@favorites".into(), 1), ("nes".into(), 2));
        self.origins
            .insert(("@recent".into(), 0), ("snes".into(), 7));
    }
}

fn games(count: usize) -> Vec<Game> {
    (0..count)
        .map(|i| Game {
            path: format!("game-{i}.zip").into(),
            name: format!("Game {i}"),
            image: None,
            video: None,
            favorite: false,
            year: None,
            players: None,
            last_played: None,
            details: Default::default(),
        })
        .collect()
}

fn pair(emulator: &str, core: &str) -> (String, String) {
    (emulator.into(), core.into())
}

fn in_games(system: &str, cursor: usize) -> App {
    let mut app = App::new(5);
    app.screen = Screen::Games {
        system: system.into(),
    };
    app.cursor = cursor;
    app
}

#[test]
fn a_on_system_enters_games() {
    let data = FakeData::library();
    let mut app = App::new(5);
    assert_eq!(app.screen, Screen::Systems);
    app.handle(Button::Down, &data);
    assert_eq!(app.handle(Button::A, &data), Outcome::None);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "snes".into()
        }
    );
    assert_eq!(app.cursor, 0);
}

#[test]
fn b_on_games_returns_to_same_system_row() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = App::new(5);
    for _ in 0..3 {
        app.handle(Button::Down, &data);
    }
    app.handle(Button::A, &data);
    for _ in 0..7 {
        app.handle(Button::Down, &data);
    }
    assert_eq!(app.cursor, 7);
    assert_eq!(app.handle(Button::B, &data), Outcome::None);
    assert_eq!(app.screen, Screen::Systems);
    assert_eq!(app.cursor, 3);
}

#[test]
fn a_on_game_launches() {
    let data = FakeData::library();
    let mut app = in_games("snes", 7);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Launch {
            system: "snes".into(),
            game: 7
        }
    );
    assert_eq!(app.cursor, 7);
}

#[test]
fn x_toggles_favorite() {
    let data = FakeData::library();
    let mut app = in_games("nes", 2);
    assert_eq!(
        app.handle(Button::X, &data),
        Outcome::ToggleFavorite {
            system: "nes".into(),
            game: 2
        }
    );
}

#[test]
fn l_r_move_by_page_and_clamp() {
    let data = FakeData::library();
    let mut app = in_games("snes", 0);
    for (button, cursor) in [
        (Button::L, 0),
        (Button::R, 5),
        (Button::R, 10),
        (Button::R, 11),
        (Button::R, 11),
        (Button::L, 6),
        (Button::L, 1),
        (Button::L, 0),
    ] {
        assert_eq!(app.handle(button, &data), Outcome::None);
        assert_eq!(app.cursor, cursor);
    }
}

#[test]
fn system_pages_use_visible_rows() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = App::new(2);
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, 2);
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, 3);
    app.handle(Button::L, &data);
    assert_eq!(app.cursor, 1);
}

#[test]
fn up_down_clamp_instead_of_wrapping() {
    let data = FakeData::library();
    for mut app in [App::new(5), in_games("nes", 0)] {
        app.handle(Button::Up, &data);
        assert_eq!(app.cursor, 0);
        for _ in 0..20 {
            app.handle(Button::Down, &data);
        }
        let last = if app.screen == Screen::Systems { 1 } else { 2 };
        assert_eq!(app.cursor, last);
    }
}

#[test]
fn cursor_clamped_when_list_empty() {
    let data = FakeData::default();
    for mut app in [App::new(5), in_games("missing", 99)] {
        app.cursor = 99;
        for button in [
            Button::A,
            Button::X,
            Button::Up,
            Button::Down,
            Button::L,
            Button::R,
        ] {
            assert_eq!(app.handle(button, &data), Outcome::None);
            assert_eq!(app.cursor, 0);
        }
    }
}

#[test]
fn stale_cursor_clamps_before_launch() {
    let data = FakeData::library();
    let mut app = in_games("nes", usize::MAX);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Launch {
            system: "nes".into(),
            game: 2
        }
    );
    assert_eq!(app.cursor, 2);
}

#[test]
fn page_bounds_cover_overflow_and_zero() {
    let data = FakeData::library();
    for (size, right, left) in [(usize::MAX, 11, 0), (0, 7, 7)] {
        let mut app = in_games("snes", 7);
        app.games_nav.page = size;
        for (button, expected) in [(Button::R, right), (Button::L, left)] {
            assert_eq!(app.handle(button, &data), Outcome::None);
            assert_eq!(app.cursor, expected);
        }
    }
}

#[test]
fn collections_listed_before_systems() {
    let mut data = FakeData::library();
    data.collections();
    assert_eq!(
        App::system_rows(&data),
        ["@favorites", "@recent", "nes", "snes"]
    );
}

#[test]
fn empty_collections_and_zero_game_systems_are_hidden() {
    let mut data = FakeData::library();
    assert_eq!(App::system_rows(&data), ["nes", "snes"]);
    data.collections();
    data.games.remove("@favorites");
    assert_eq!(App::system_rows(&data), ["@recent", "nes", "snes"]);
    data.games.remove("@recent");
    data.games.insert("@favorites".into(), games(1));
    assert_eq!(App::system_rows(&data), ["@favorites", "nes", "snes"]);
}

#[test]
fn collections_launch_and_favorite_resolve_real_origin() {
    let mut data = FakeData::library();
    data.collections();
    for (system, row, real_system, real_game) in [
        ("@favorites", 0, "snes", 7),
        ("@favorites", 1, "nes", 2),
        ("@recent", 0, "snes", 7),
    ] {
        let mut app = in_games(system, row);
        assert_eq!(
            app.handle(Button::A, &data),
            Outcome::Launch {
                system: real_system.into(),
                game: real_game
            }
        );
        assert_eq!(
            app.handle(Button::X, &data),
            Outcome::ToggleFavorite {
                system: real_system.into(),
                game: real_game
            }
        );
    }
}

#[test]
fn collection_entry_and_back_preserve_pseudo_system_row() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = App::new(5);
    app.handle(Button::Down, &data);
    app.handle(Button::A, &data);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "@recent".into()
        }
    );
    app.handle(Button::B, &data);
    assert_eq!(app.screen, Screen::Systems);
    assert_eq!(app.cursor, 1);
}

#[test]
fn missing_origin_makes_game_actions_noops() {
    let mut data = FakeData::library();
    data.collections();
    data.origins.clear();
    let mut app = in_games("@favorites", 1);
    for button in [Button::A, Button::X, Button::Y] {
        assert_eq!(app.handle(button, &data), Outcome::None);
        assert_eq!(
            app.screen,
            Screen::Games {
                system: "@favorites".into()
            }
        );
        assert_eq!(app.cursor, 1);
    }
}

#[test]
fn y_opens_quick_menu_and_b_closes() {
    let data = FakeData::library();
    let mut app = in_games("snes", 7);
    assert_eq!(app.handle(Button::Y, &data), Outcome::None);
    assert_eq!(
        app.screen,
        Screen::QuickMenu {
            game: Some(("snes".into(), 7))
        }
    );
    assert_eq!(app.cursor, 0);
    app.handle(Button::Down, &data);
    assert_eq!(app.handle(Button::B, &data), Outcome::None);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "snes".into()
        }
    );
    assert_eq!(app.cursor, 7);
}

#[test]
fn quick_menu_from_systems_works_even_without_systems() {
    for data in [FakeData::default(), FakeData::library()] {
        let mut app = App::new(5);
        app.handle(Button::Down, &data);
        let cursor = app.cursor;
        app.handle(Button::Y, &data);
        assert_eq!(app.screen, Screen::QuickMenu { game: None });
        assert_eq!(
            app.quick_menu_items(&data),
            [
                QuickItem::Settings,
                QuickItem::Move,
                QuickItem::Suspend,
                QuickItem::Reboot,
                QuickItem::Shutdown
            ]
        );
        app.handle(Button::Down, &data);
        app.handle(Button::B, &data);
        assert_eq!(app.screen, Screen::Systems);
        assert_eq!(app.cursor, cursor);
    }
}

#[test]
fn settings_remains_accessible_alongside_order_actions() {
    let mut data = FakeData::library();
    for order in [vec![], vec!["snes".to_owned(), "nes".to_owned()]] {
        data.order = order;
        let mut app = App::new(5);
        app.handle(Button::Y, &data);
        assert_eq!(app.quick_menu_items(&data)[0], QuickItem::Settings);
        assert_eq!(app.handle(Button::A, &data), Outcome::Settings);
        assert_eq!(app.screen, Screen::Systems);
    }
}

#[test]
fn quick_menu_collection_uses_real_game_and_restores_collection_cursor() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = App::new(5);
    app.handle(Button::A, &data);
    app.handle(Button::Y, &data);
    assert_eq!(
        app.screen,
        Screen::QuickMenu {
            game: Some(("snes".into(), 7))
        }
    );
    assert_eq!(
        app.quick_menu_items(&data),
        [QuickItem::Core, QuickItem::Details]
    );
    app.handle(Button::B, &data);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "@favorites".into()
        }
    );
    app.handle(Button::B, &data);
    assert_eq!(app.screen, Screen::Systems);
    assert_eq!(app.cursor, 0);
}

#[test]
fn quick_menu_tools_and_ports_hide_core_and_power_lives_in_the_system_menu() {
    let mut data = FakeData::library();
    for system in ["tools", "ports"] {
        data.games.insert(system.into(), games(1));
    }
    data.cores
        .insert("ports".into(), vec![pair("portmaster", "portmaster")]);
    for system in ["snes", "tools", "ports"] {
        let mut app = in_games(system, 0);
        app.handle(Button::Y, &data);
        // Game menus open the details; only snes has a core choice.
        let items = app.quick_menu_items(&data);
        assert_eq!(items.len(), 1 + usize::from(system == "snes"), "{system}");
        assert!(!items.contains(&QuickItem::Suspend), "{system}");
        assert!(!items.contains(&QuickItem::Reboot), "{system}");
        assert!(!items.contains(&QuickItem::Shutdown), "{system}");
    }
    let mut app = App::new(5);
    app.handle(Button::Y, &data);
    for (index, action) in [
        SessionAction::Suspend,
        SessionAction::Reboot,
        SessionAction::Shutdown,
    ]
    .into_iter()
    .enumerate()
    {
        app.cursor = index + 2;
        assert_eq!(app.handle(Button::A, &data), Outcome::Action(action));
        assert!(matches!(app.screen, Screen::QuickMenu { .. }));
    }
}

#[test]
fn quick_menu_up_down_clamp() {
    let data = FakeData::library();
    let mut app = in_games("snes", 7);
    app.handle(Button::Y, &data);
    app.handle(Button::Up, &data);
    assert_eq!(app.cursor, 0);
    for _ in 0..10 {
        app.handle(Button::Down, &data);
    }
    assert_eq!(app.cursor, 1);
}

fn open_details(data: &FakeData, system: &str, game: usize) -> App {
    let mut app = in_games(system, game);
    app.handle(Button::Y, data);
    app.cursor = app
        .quick_menu_items(data)
        .iter()
        .position(|item| *item == QuickItem::Details)
        .unwrap();
    app.handle(Button::A, data);
    app
}

#[test]
fn details_open_from_the_game_menu_and_go_back_to_the_list() {
    let data = FakeData::library();
    let mut app = open_details(&data, "snes", 7);
    assert_eq!(
        app.screen,
        Screen::Details {
            system: "snes".into(),
            game: 7
        }
    );
    // The menu over the details offers no second "Details".
    app.handle(Button::Y, &data);
    assert!(!app.quick_menu_items(&data).contains(&QuickItem::Details));
    app.handle(Button::B, &data);
    assert!(matches!(app.screen, Screen::Details { .. }));
    app.handle(Button::B, &data);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "snes".into()
        }
    );
    assert_eq!(app.cursor, 7);
}

#[test]
fn details_buttons_play_favorite_and_scrape_again_only_when_allowed() {
    let data = FakeData::library();
    let mut app = open_details(&data, "snes", 7);
    let game = || ("snes".to_owned(), 7);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Launch {
            system: game().0,
            game: game().1
        }
    );
    app.handle(Button::R, &data);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::ToggleFavorite {
            system: game().0,
            game: game().1
        }
    );
    app.handle(Button::R, &data);
    app.handle(Button::R, &data);
    assert_eq!(app.details_focus, 2);
    assert_eq!(app.handle(Button::A, &data), Outcome::None);
    app.can_rescrape = true;
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Rescrape {
            system: game().0,
            game: game().1
        }
    );
    app.rescraping = true;
    assert_eq!(app.handle(Button::A, &data), Outcome::None);
}

#[test]
fn details_text_scrolls_within_the_reported_overflow() {
    let data = FakeData::library();
    let mut app = open_details(&data, "snes", 7);
    app.handle(Button::Down, &data);
    assert_eq!(app.details_scroll, 0, "nothing to scroll");
    app.details_overflow = 2;
    for _ in 0..5 {
        app.handle(Button::Down, &data);
    }
    assert_eq!(app.details_scroll, 2);
    app.handle(Button::Up, &data);
    assert_eq!(app.details_scroll, 1);
    // A re-sorted system moves the game: the screen and the list follow it.
    app.retarget_details(3);
    assert_eq!(
        app.screen,
        Screen::Details {
            system: "snes".into(),
            game: 3
        }
    );
    app.handle(Button::B, &data);
    assert_eq!(app.cursor, 3);
}

#[test]
fn core_cycles_with_shoulders_and_emits_real_identity() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = in_games("@recent", 0);
    app.handle(Button::Y, &data);
    for (button, emulator, core) in [
        (Button::R, "standalone", "native"),
        (Button::R, "retroarch", "snes9x"),
        (Button::L, "standalone", "native"),
        (Button::L, "retroarch", "snes9x"),
    ] {
        assert_eq!(
            app.handle(button, &data),
            Outcome::SetCore {
                system: "snes".into(),
                game: 7,
                emulator: emulator.into(),
                core: core.into()
            }
        );
        data.current
            .insert(("snes".into(), 7), pair(emulator, core));
        assert_eq!(app.selected_core(), (emulator, core));
        assert_eq!(app.cursor, 0);
    }
    assert_eq!(app.handle(Button::A, &data), Outcome::None);
}

#[test]
fn core_cycle_handles_missing_current_and_empty_choices() {
    let mut data = FakeData::library();
    data.current.clear();
    let mut app = in_games("snes", 7);
    app.handle(Button::Y, &data);
    assert_eq!(app.selected_core(), ("", ""));
    for (button, emulator, core) in [
        (Button::R, "retroarch", "snes9x"),
        (Button::L, "standalone", "native"),
    ] {
        assert_eq!(
            app.handle(button, &data),
            Outcome::SetCore {
                system: "snes".into(),
                game: 7,
                emulator: emulator.into(),
                core: core.into()
            }
        );
    }
    data.cores.clear();
    for button in [Button::L, Button::R] {
        assert_eq!(app.handle(button, &data), Outcome::None);
    }
}

#[test]
fn shoulders_on_non_core_menu_items_do_nothing() {
    let data = FakeData::library();
    for mut app in [App::new(5), in_games("snes", 7)] {
        app.handle(Button::Y, &data);
        app.handle(Button::Down, &data);
        for button in [Button::L, Button::R] {
            assert_eq!(app.handle(button, &data), Outcome::None);
            assert_eq!(app.cursor, 1);
        }
    }
}

#[test]
fn empty_games_cannot_open_game_menu() {
    let data = FakeData::default();
    let mut app = in_games("missing", 0);
    assert_eq!(app.handle(Button::Y, &data), Outcome::None);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "missing".into()
        }
    );
}

#[test]
fn reserved_and_unmapped_buttons_do_nothing() {
    let data = FakeData::library();
    for mut app in [App::new(5), in_games("snes", 7)] {
        let screen = app.screen.clone();
        let cursor = app.cursor;
        assert_eq!(app.handle(Button::Select, &data), Outcome::None);
        assert_eq!(app.screen, screen);
        assert_eq!(app.cursor, cursor);
        if matches!(screen, Screen::Games { .. }) {
            assert_eq!(app.handle(Button::Start, &data), Outcome::None);
            assert_eq!(app.screen, screen);
        }
        app.handle(Button::Y, &data);
        let screen = app.screen.clone();
        for button in [Button::Start, Button::Select, Button::X, Button::Y] {
            assert_eq!(app.handle(button, &data), Outcome::None);
            assert_eq!(app.screen, screen);
            assert_eq!(app.cursor, 0);
        }
    }
    let mut app = App::new(5);
    for button in [Button::B, Button::X] {
        assert_eq!(app.handle(button, &data), Outcome::None);
        assert_eq!(app.screen, Screen::Systems);
    }
}

fn assert_core_sequence_without_data_changes(button: Button, expected: &[(&str, &str)]) {
    let mut data = FakeData::library();
    data.cores.insert(
        "snes".into(),
        vec![
            pair("retroarch", "snes9x"),
            pair("standalone", "native"),
            pair("retroarch", "bsnes"),
        ],
    );
    let mut app = in_games("snes", 7);
    app.handle(Button::Y, &data);
    let mut outcomes = Vec::new();
    let mut displayed = Vec::new();
    for _ in expected {
        outcomes.push(app.handle(button, &data));
        displayed.push(pair(app.selected_core().0, app.selected_core().1));
    }
    for (outcome, &(emulator, core)) in outcomes.into_iter().zip(expected) {
        assert_eq!(
            outcome,
            Outcome::SetCore {
                system: "snes".into(),
                game: 7,
                emulator: emulator.into(),
                core: core.into(),
            }
        );
    }
    for (item, &(emulator, core)) in displayed.into_iter().zip(expected) {
        assert_eq!(item, pair(emulator, core));
    }
}

#[test]
fn consecutive_shoulders_cycle_all_cores_with_unchanged_data() {
    for (button, expected) in [
        (
            Button::R,
            [
                ("standalone", "native"),
                ("retroarch", "bsnes"),
                ("retroarch", "snes9x"),
                ("standalone", "native"),
            ],
        ),
        (
            Button::L,
            [
                ("retroarch", "bsnes"),
                ("standalone", "native"),
                ("retroarch", "snes9x"),
                ("retroarch", "bsnes"),
            ],
        ),
    ] {
        assert_core_sequence_without_data_changes(button, &expected);
    }
}

#[test]
fn quick_menu_preserves_the_collection_backdrop_and_cursor() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = in_games("@favorites", 1);
    app.handle(Button::Y, &data);
    assert_eq!(
        app.backdrop(),
        (
            &Screen::Games {
                system: "@favorites".into()
            },
            1
        )
    );
    app.handle(Button::B, &data);
    assert_eq!(app.backdrop(), (&app.screen, 1));
}

fn ordered(data: &FakeData) -> Vec<String> {
    App::system_rows(data)
}

#[test]
fn saved_order_wins_then_new_systems_follow_and_removed_ids_are_ignored() {
    let mut data = FakeData::library();
    data.collections();
    data.order = ["snes", "gone", "@recent", "snes"].map(String::from).into();
    assert_eq!(ordered(&data), ["snes", "@recent", "@favorites", "nes"]);
    data.order.clear();
    assert_eq!(ordered(&data), ["@favorites", "@recent", "nes", "snes"]);
}

fn start_move(data: &FakeData, cursor: usize) -> App {
    let mut app = App::new(5);
    app.cursor = cursor;
    app.handle(Button::Y, data);
    app.handle(Button::Down, data);
    assert_eq!(app.quick_menu_items(data)[app.cursor], QuickItem::Move);
    assert_eq!(app.handle(Button::A, data), Outcome::None);
    assert_eq!(app.screen, Screen::Systems);
    app
}

#[test]
fn move_swaps_rows_with_the_cursor_and_confirm_publishes_the_order() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = start_move(&data, 1);
    assert_eq!(app.moving(), Some(("@recent", 2, 4)));
    app.handle(Button::Down, &data);
    app.handle(Button::Down, &data);
    app.handle(Button::Down, &data);
    assert_eq!(app.cursor, 3, "clamped at the last row");
    app.handle(Button::Up, &data);
    app.handle(Button::L, &data);
    app.handle(Button::Start, &data);
    assert_eq!(app.screen, Screen::Systems, "other buttons are ignored");
    assert_eq!(app.rows(&data), ["@favorites", "nes", "@recent", "snes"]);
    assert_eq!(ordered(&data), ["@favorites", "@recent", "nes", "snes"]);
    let expected = ["@favorites", "nes", "@recent", "snes"].map(String::from);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::SetOrder(expected.into())
    );
    assert_eq!((app.moving(), app.cursor), (None, 2));
}

#[test]
fn cancel_restores_the_previous_order_and_cursor() {
    let data = FakeData::library();
    let mut app = start_move(&data, 0);
    app.handle(Button::Down, &data);
    assert_eq!(app.handle(Button::B, &data), Outcome::None);
    assert_eq!((app.moving(), app.cursor), (None, 0));
    assert_eq!(app.rows(&data), ["nes", "snes"]);
}

#[test]
fn restore_item_exists_only_with_a_custom_order_and_keeps_the_cursor_on_its_row() {
    let mut data = FakeData::library();
    let mut app = App::new(5);
    app.handle(Button::Y, &data);
    assert!(!app
        .quick_menu_items(&data)
        .contains(&QuickItem::RestoreOrder));
    data.order = ["snes", "nes"].map(String::from).into();
    app.handle(Button::B, &data);
    app.handle(Button::Y, &data);
    assert_eq!(app.quick_menu_items(&data)[2], QuickItem::RestoreOrder);
    app.handle(Button::Down, &data);
    app.handle(Button::Down, &data);
    assert_eq!(app.handle(Button::A, &data), Outcome::SetOrder(Vec::new()));
    assert_eq!((app.screen.clone(), app.cursor), (Screen::Systems, 1));
}

/// An app following `view` of the builtin theme.
fn in_view(view: &str) -> App {
    let mut app = App::new(1);
    app.set_view(&archr_ui::Theme::builtin().view(view));
    app
}

#[test]
fn the_carousel_steps_one_system_with_left_and_right() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = in_view("vitrine");
    // The d-pad and the shoulders both send L/R.
    for (button, cursor) in [
        (Button::R, 1),
        (Button::R, 2),
        (Button::L, 1),
        (Button::Down, 2),
        (Button::Up, 1),
        (Button::R, 2),
        (Button::R, 3),
        (Button::R, 3),
        (Button::L, 2),
    ] {
        assert_eq!(app.handle(button, &data), Outcome::None);
        assert_eq!(app.cursor, cursor, "after {button:?}");
    }
    // Its games screen is a vertical list: L/R page through it.
    app.cursor = 3;
    app.handle(Button::A, &data);
    let page = app.games_nav.page;
    assert!(page > 1);
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, page);
    app.handle(Button::Down, &data);
    assert_eq!(app.cursor, page + 1);
    app.handle(Button::L, &data);
    assert_eq!(app.cursor, 1);
}

#[test]
fn a_vertical_list_pages_by_what_its_layout_shows() {
    let data = FakeData::library();
    let mut app = in_view("lista");
    let page = app.games_nav.page;
    assert!((5..20).contains(&page), "a screenful of games: {page}");
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, 1, "two systems: a page reaches the last");
    app.handle(Button::L, &data);
    assert_eq!(app.cursor, 0);
    app.cursor = 1;
    app.handle(Button::A, &data);
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, page.min(11));
    app.handle(Button::L, &data);
    assert_eq!(app.cursor, 0);
}

#[test]
fn changing_the_view_changes_the_navigation() {
    let theme = archr_ui::Theme::builtin();
    let mut app = in_view("lista");
    let lista = app.systems_nav;
    app.set_view(&theme.view("vitrine"));
    assert_ne!(app.systems_nav, lista);
    app.set_view(&theme.view("lista"));
    assert_eq!(app.systems_nav, lista);
}

#[test]
fn the_carousel_moves_a_lifted_system_with_left_and_right() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = start_move(&data, 1);
    app.set_view(&archr_ui::Theme::builtin().view("vitrine"));
    app.handle(Button::R, &data);
    assert_eq!(app.rows(&data), ["@favorites", "nes", "@recent", "snes"]);
    app.handle(Button::L, &data);
    app.handle(Button::L, &data);
    assert_eq!(app.rows(&data), ["@recent", "@favorites", "nes", "snes"]);
    assert_eq!(app.cursor, 0);
}

#[test]
fn achievements_open_from_the_game_menu_only_when_signed_in_and_page_with_l_r() {
    use archr_ui::cheevos::{Fetch, Item, Sheet};
    let data = FakeData::library();
    let mut app = in_games("nes", 1);
    app.handle(Button::Y, &data);
    assert!(!app
        .quick_menu_items(&data)
        .contains(&QuickItem::Achievements));
    app.handle(Button::B, &data);
    app.cheevos = true;
    app.handle(Button::Y, &data);
    let items = app.quick_menu_items(&data);
    assert_eq!(items, [QuickItem::Details, QuickItem::Achievements]);
    app.cursor = 1;
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Achievements {
            system: "nes".into(),
            game: 1
        }
    );
    assert_eq!(
        app.screen,
        Screen::Achievements {
            system: "nes".into(),
            game: 1
        }
    );
    assert_eq!(app.sheet.fetch, Fetch::Loading);
    app.sheet = Sheet {
        fetch: Fetch::Fresh,
        hardcore: false,
        items: vec![Item::default(); 14],
    };
    app.sheet_page = 6;
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, 6);
    app.handle(Button::R, &data);
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, 13, "stops at the last one");
    app.handle(Button::Up, &data);
    app.handle(Button::L, &data);
    assert_eq!(app.cursor, 6);
    // The details menu offers it too, and B goes back where it came from.
    app.handle(Button::B, &data);
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "nes".into()
        }
    );
    assert_eq!(app.cursor, 1);
    let mut details = open_details(&data, "nes", 1);
    details.cheevos = true;
    details.handle(Button::Y, &data);
    assert!(details
        .quick_menu_items(&data)
        .contains(&QuickItem::Achievements));
}

#[test]
fn mangohud_row_steps_through_the_systems_choices_for_this_game() {
    let data = FakeData::library();
    let mut app = in_games("snes", 7);
    app.handle(Button::Y, &data);
    assert!(!app.quick_menu_items(&data).contains(&QuickItem::Mangohud));
    app.handle(Button::B, &data);
    app.mangohud = true;
    app.handle(Button::Y, &data);
    assert_eq!(
        app.quick_menu_items(&data),
        [QuickItem::Core, QuickItem::Details, QuickItem::Mangohud]
    );
    app.handle(Button::Down, &data);
    app.handle(Button::Down, &data);
    for (button, value) in [
        (Button::R, "1"),
        (Button::R, "0"),
        (Button::R, ""),
        (Button::L, "0"),
    ] {
        assert_eq!(
            app.handle(button, &data),
            Outcome::SetMangohud {
                system: "snes".into(),
                game: 7,
                value: value.into()
            }
        );
        assert_eq!(app.selected_mangohud().0, value);
    }
    assert_eq!(app.handle(Button::A, &data), Outcome::None);
}

mod progress {
    use super::*;
    use archr_ui::saves::{Blocked, Kind, Progress, Save, Shot};

    fn save(slot: Option<u16>, kind: Kind) -> Save {
        Save {
            path: format!(
                "/s/Game 7.state{}",
                slot.map_or(".auto".into(), |n| n.to_string())
            )
            .into(),
            picture: None,
            slot,
            saved: 100,
            bytes: 1,
            kind,
            emulator: "RetroArch".into(),
            playable: true,
            backup: Vec::new(),
        }
    }
    fn shot(n: u64) -> Shot {
        Shot {
            path: format!("/shots/Game 7-2610{n:02}-000000.png").into(),
            taken: n,
            bytes: 1,
            size: None,
            details: n == 1,
        }
    }
    fn found(saves: usize, shots: u64) -> Progress {
        Progress {
            game: Some(("snes".into(), 7)),
            saves: (0..saves as u16)
                .map(|n| save(Some(n), Kind::RetroArch))
                .collect(),
            shots: (0..shots).map(shot).collect(),
        }
    }
    /// The game menu of snes game 7, after the host answered.
    fn menu(data: &FakeData, progress: Progress) -> App {
        let mut app = in_games("snes", 7);
        app.handle(Button::Y, data);
        assert_eq!(app.scan, Some(("snes".into(), 7)), "the host is asked");
        app.set_progress(progress, data);
        app
    }

    #[test]
    fn the_game_menu_offers_saves_and_screenshots_only_when_there_are_some() {
        let data = FakeData::library();
        let app = menu(&data, found(2, 3));
        let items = app.quick_menu_items(&data);
        assert_eq!(
            &items[..4],
            [
                QuickItem::Core,
                QuickItem::Details,
                QuickItem::Saves,
                QuickItem::Shots
            ]
        );
        let app = menu(&data, found(0, 0));
        assert!(!app.quick_menu_items(&data).contains(&QuickItem::Saves));
        // Another game's answer shows nothing here.
        let mut other = found(2, 2);
        other.game = Some(("snes".into(), 6));
        let app = menu(&data, other);
        assert!(!app.quick_menu_items(&data).contains(&QuickItem::Shots));
    }

    #[test]
    fn the_selected_row_stays_put_when_the_rows_arrive() {
        let data = FakeData::library();
        let mut app = in_games("snes", 7);
        app.handle(Button::Y, &data);
        app.handle(Button::Down, &data);
        assert_eq!(app.quick_menu_items(&data)[app.cursor], QuickItem::Details);
        app.handle(Button::Down, &data);
        let before = app.quick_menu_items(&data)[app.cursor];
        app.set_progress(found(1, 1), &data);
        assert_eq!(app.quick_menu_items(&data)[app.cursor], before);
    }

    #[test]
    fn playing_from_a_state_asks_first_and_deleting_starts_on_cancel() {
        let data = FakeData::library();
        let mut app = menu(&data, found(3, 0));
        app.cursor = 2;
        app.handle(Button::A, &data);
        assert_eq!(
            app.screen,
            Screen::Saves {
                system: "snes".into(),
                game: 7
            }
        );
        app.handle(Button::Down, &data);
        assert_eq!(app.handle(Button::A, &data), Outcome::None, "a question");
        assert!(app.confirm.as_ref().is_some_and(|c| c.on_action));
        assert_eq!(
            app.handle(Button::A, &data),
            Outcome::PlayFrom {
                system: "snes".into(),
                game: 7,
                state: "/s/Game 7.state1".into(),
            }
        );
        // Delete: Cancel has the focus; A there only closes the question.
        app.handle(Button::X, &data);
        assert!(app.confirm.as_ref().is_some_and(|c| !c.on_action));
        assert_eq!(app.handle(Button::A, &data), Outcome::None);
        assert!(app.confirm.is_none());
        app.handle(Button::X, &data);
        app.handle(Button::R, &data);
        assert!(matches!(
            app.handle(Button::A, &data),
            Outcome::Delete { paths, .. } if paths == [std::path::PathBuf::from("/s/Game 7.state1")]
        ));
        // Nothing left: the screen closes.
        app.set_progress(found(0, 0), &data);
        assert_eq!(
            app.screen,
            Screen::Games {
                system: "snes".into()
            }
        );
    }

    #[test]
    fn hardcore_and_netplay_keep_retroarch_states_from_starting_the_game() {
        let data = FakeData::library();
        for blocked in [Blocked::Hardcore, Blocked::Netplay] {
            let mut app = menu(&data, found(1, 0));
            app.blocked = Some(blocked);
            app.cursor = 2;
            app.handle(Button::A, &data);
            app.handle(Button::A, &data);
            assert!(app.confirm.is_none(), "{blocked:?}");
            assert!(!app.can_play(&app.progress.saves[0]));
            // A standalone emulator's state is not RetroArch's to refuse.
            assert!(app.can_play(&save(Some(1), Kind::Standalone)));
        }
    }

    #[test]
    fn the_grid_moves_by_rows_and_opens_one_screenshot() {
        let data = FakeData::library();
        let mut app = menu(&data, found(0, 6));
        app.shots_columns = 4;
        app.cursor = 2;
        app.handle(Button::A, &data);
        app.handle(Button::R, &data);
        app.handle(Button::Down, &data);
        assert_eq!(app.cursor, 5, "the next row, clamped to the last");
        app.handle(Button::Down, &data);
        assert_eq!(app.cursor, 5);
        app.handle(Button::Up, &data);
        assert_eq!(app.cursor, 1);
        app.handle(Button::A, &data);
        assert!(matches!(app.screen, Screen::Shot { .. }));
        // Shot 1 is already the details picture; shot 2 is not.
        assert_eq!(app.handle(Button::Y, &data), Outcome::None);
        app.handle(Button::R, &data);
        assert!(
            matches!(app.handle(Button::Y, &data), Outcome::UseShot { path, .. } if path.ends_with("Game 7-261002-000000.png"))
        );
        app.handle(Button::B, &data);
        assert!(matches!(app.screen, Screen::Shots { .. }));
        assert_eq!(app.cursor, 2, "back on the picture last shown");
    }
}
