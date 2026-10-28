use archr_index::{cache::SystemEntry, Game};
use archr_ui::state::{App, Button, DataSource, Outcome, Screen, SessionAction};

struct Data {
    systems: Vec<SystemEntry>,
}
impl DataSource for Data {
    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }
    fn games(&self, _: &str) -> &[Game] {
        &[]
    }
    fn origin(&self, _: &str, _: usize) -> Option<(String, usize)> {
        None
    }
    fn cores(&self, _: &str) -> Vec<(String, String)> {
        vec![]
    }
    fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
        None
    }
}
fn data(tools: bool) -> Data {
    Data {
        systems: if tools {
            vec![SystemEntry {
                name: "tools".into(),
                fullname: "Tools".into(),
                game_count: 0,
                dir_mtime: 0,
                source_fingerprint: 0,
            }]
        } else {
            vec![]
        },
    }
}
#[test]
fn start_and_b_close_hub_without_losing_system_cursor() {
    let mut app = App::new(8);
    app.cursor = 3;
    app.handle(Button::Start, &data(false));
    assert!(matches!(app.screen, Screen::StartHub { .. }));
    app.handle(Button::Start, &data(false));
    assert_eq!(app.screen, Screen::Systems);
    assert_eq!(app.cursor, 3);
    app.handle(Button::Start, &data(false));
    app.handle(Button::B, &data(false));
    assert_eq!(app.screen, Screen::Systems);
}
#[test]
fn tiles_open_recent_games_and_only_configured_tools() {
    let mut app = App::new(8);
    app.handle(Button::Start, &data(false));
    app.handle(Button::A, &data(false));
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "@recent".into()
        }
    );
    app.handle(Button::B, &data(false));
    assert_eq!(app.screen, Screen::Systems);
    app.handle(Button::Start, &data(false));
    app.handle(Button::R, &data(false));
    app.handle(Button::A, &data(false));
    assert_eq!(app.screen, Screen::Systems);
    app.handle(Button::Start, &data(false));
    app.handle(Button::R, &data(false));
    app.handle(Button::R, &data(false));
    app.handle(Button::A, &data(false));
    assert!(matches!(app.screen, Screen::StartHub { .. }));
    app.handle(Button::A, &data(true));
    assert_eq!(
        app.screen,
        Screen::Games {
            system: "tools".into()
        }
    );
}
#[test]
fn shortcuts_remember_columns_and_toggle_from_snapshot() {
    let mut app = App::new(8);
    let data = data(false);
    app.handle(Button::Start, &data);
    app.handle(Button::Down, &data);
    app.handle(Button::R, &data);
    app.handle(Button::R, &data);
    assert_eq!(
        app.handle_with_wifi(Button::A, &data, false),
        Outcome::ToggleWifi(true)
    );
    assert_eq!(
        app.handle_with_wifi(Button::A, &data, true),
        Outcome::ToggleWifi(false)
    );
    app.handle(Button::Up, &data);
    assert!(matches!(
        app.screen,
        Screen::StartHub {
            row: 0,
            tile: 0,
            shortcut: 2
        }
    ));
    app.handle(Button::Down, &data);
    app.handle(Button::R, &data);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Action(SessionAction::Suspend)
    );
    app.handle(Button::L, &data);
    app.handle(Button::L, &data);
    app.handle(Button::L, &data);
    assert_eq!(app.handle(Button::A, &data), Outcome::None);
}
