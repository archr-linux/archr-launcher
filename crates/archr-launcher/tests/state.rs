use std::collections::HashMap;

use archr_index::{cache::SystemEntry, Game};
use archr_launcher::state::{App, Button, DataSource, Outcome, QuickItem, Screen, SessionAction};

#[derive(Default)]
struct FakeData {
    systems: Vec<SystemEntry>,
    games: HashMap<String, Vec<Game>>,
    origins: HashMap<(String, usize), (String, usize)>,
    cores: HashMap<String, Vec<(String, String)>>,
    current: HashMap<(String, usize), (String, String)>,
}

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

impl DataSource for FakeData {
    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }

    fn games(&self, system: &str) -> &[Game] {
        self.games.get(system).map(Vec::as_slice).unwrap_or(&[])
    }

    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        if system.starts_with('@') {
            self.origins.get(&(system.into(), game)).cloned()
        } else {
            self.games(system).get(game).map(|_| (system.into(), game))
        }
    }

    fn cores(&self, system: &str) -> Vec<(String, String)> {
        self.cores.get(system).cloned().unwrap_or_default()
    }

    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)> {
        self.current.get(&(system.into(), game)).cloned()
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
fn page_movement_cannot_overflow() {
    let data = FakeData::library();
    let mut app = in_games("snes", 7);
    app.page_size = usize::MAX;
    app.handle(Button::R, &data);
    assert_eq!(app.cursor, 11);
    app.handle(Button::L, &data);
    assert_eq!(app.cursor, 0);
}

#[test]
fn zero_page_size_does_not_move_cursor() {
    let data = FakeData::library();
    let mut app = in_games("snes", 7);
    app.page_size = 0;
    for button in [Button::L, Button::R] {
        app.handle(button, &data);
        assert_eq!(app.cursor, 7);
    }
}

#[test]
fn collections_listed_before_systems() {
    let mut data = FakeData::library();
    data.collections();
    let app = App::new(5);
    assert_eq!(
        app.system_rows(&data),
        ["@favorites", "@recent", "nes", "snes"]
    );
}

#[test]
fn empty_collections_and_zero_game_systems_are_hidden() {
    let mut data = FakeData::library();
    let app = App::new(5);
    assert_eq!(app.system_rows(&data), ["nes", "snes"]);
    data.collections();
    data.games.remove("@favorites");
    assert_eq!(app.system_rows(&data), ["@recent", "nes", "snes"]);
    data.games.remove("@recent");
    data.games.insert("@favorites".into(), games(1));
    assert_eq!(app.system_rows(&data), ["@favorites", "nes", "snes"]);
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
            system: Some("snes".into()),
            game: Some(7)
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
        assert_eq!(
            app.screen,
            Screen::QuickMenu {
                system: None,
                game: None
            }
        );
        assert_eq!(
            app.quick_menu_items(&data),
            [
                QuickItem::Settings,
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
fn quick_menu_collection_uses_real_game_and_restores_collection_cursor() {
    let mut data = FakeData::library();
    data.collections();
    let mut app = App::new(5);
    app.handle(Button::A, &data);
    app.handle(Button::Y, &data);
    assert_eq!(
        app.screen,
        Screen::QuickMenu {
            system: Some("snes".into()),
            game: Some(7)
        }
    );
    assert_eq!(
        app.quick_menu_items(&data),
        [
            QuickItem::Core {
                emulator: "retroarch".into(),
                core: "snes9x".into()
            },
            QuickItem::Settings,
            QuickItem::Suspend,
            QuickItem::Reboot,
            QuickItem::Shutdown
        ]
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
fn quick_menu_actions_emit_session_outcomes() {
    let data = FakeData::library();
    for with_game in [false, true] {
        let mut app = if with_game {
            in_games("snes", 7)
        } else {
            App::new(5)
        };
        app.handle(Button::Y, &data);
        for (index, action) in [
            SessionAction::OpenSettings,
            SessionAction::Suspend,
            SessionAction::Reboot,
            SessionAction::Shutdown,
        ]
        .into_iter()
        .enumerate()
        {
            app.cursor = index + usize::from(with_game);
            assert_eq!(app.handle(Button::A, &data), Outcome::Action(action));
            assert!(matches!(app.screen, Screen::QuickMenu { .. }));
        }
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
    assert_eq!(app.cursor, 4);
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
        assert_eq!(
            app.quick_menu_items(&data)[0],
            QuickItem::Core {
                emulator: emulator.into(),
                core: core.into()
            }
        );
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
    assert_eq!(
        app.quick_menu_items(&data)[0],
        QuickItem::Core {
            emulator: String::new(),
            core: String::new()
        }
    );
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
        for button in [Button::Start, Button::Select] {
            assert_eq!(app.handle(button, &data), Outcome::None);
            assert_eq!(app.screen, screen);
            assert_eq!(app.cursor, cursor);
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
        displayed.push(app.quick_menu_items(&data)[0].clone());
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
        assert_eq!(
            item,
            QuickItem::Core {
                emulator: emulator.into(),
                core: core.into(),
            }
        );
    }
}

#[test]
fn consecutive_r_cycles_all_cores_and_wraps_with_unchanged_data() {
    assert_core_sequence_without_data_changes(
        Button::R,
        &[
            ("standalone", "native"),
            ("retroarch", "bsnes"),
            ("retroarch", "snes9x"),
            ("standalone", "native"),
        ],
    );
}

#[test]
fn consecutive_l_cycles_all_cores_and_wraps_with_unchanged_data() {
    assert_core_sequence_without_data_changes(
        Button::L,
        &[
            ("retroarch", "bsnes"),
            ("standalone", "native"),
            ("retroarch", "snes9x"),
            ("retroarch", "bsnes"),
        ],
    );
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
