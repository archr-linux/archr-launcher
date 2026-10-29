use archr_index::{cache::SystemEntry, Game};
use archr_ui::state::{App, Button, DataSource, Outcome, Screen, SessionAction};

struct Data {
    systems: Vec<SystemEntry>,
    recent: usize,
}
impl DataSource for Data {
    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }
    fn games(&self, _: &str) -> &[Game] {
        &[]
    }
    fn collection_len(&self, name: &str) -> usize {
        if name == "@recent" {
            self.recent
        } else {
            0
        }
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
        recent: 1,
        systems: if tools {
            vec![SystemEntry {
                name: "tools".into(),
                fullname: "Tools".into(),
                game_count: 3,
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
fn tiles_open_recent_games_and_tools() {
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
    app.handle(Button::Start, &data(true));
    app.handle(Button::R, &data(true));
    app.handle(Button::R, &data(true));
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
    assert_eq!(app.handle(Button::A, &data), Outcome::ToggleWifi);
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
    assert_eq!(app.handle(Button::A, &data), Outcome::Settings);
}
#[test]
fn focus_never_lands_on_a_disabled_tile() {
    let none = Data {
        recent: 0,
        ..data(false)
    };
    let mut app = App::new(8);
    app.handle(Button::Start, &none);
    let tile = |app: &App| match app.screen {
        Screen::StartHub { tile, .. } => tile,
        _ => unreachable!(),
    };
    assert_eq!(tile(&app), 1, "opens on the first offered tile");
    for button in [Button::R, Button::R, Button::L, Button::L] {
        app.handle(button, &none);
        assert_eq!(tile(&app), 1);
    }
    let all = data(true);
    app.handle(Button::R, &all);
    assert_eq!(tile(&app), 2);
    app.handle(Button::L, &all);
    app.handle(Button::L, &all);
    assert_eq!(tile(&app), 0);
}

#[test]
fn module_tiles_and_shortcuts_join_the_start_menu() {
    use archr_ui::modules::{Entry, Module, Request, Target, View};
    let entry = |id: &str, target| Entry {
        id: id.into(),
        label: id.into(),
        icon: None,
        target,
    };
    let mut app = App::new(8);
    app.modules = std::sync::Arc::new(View {
        modules: vec![Module {
            id: "bateria".into(),
            enabled: true,
            granted: true,
            tiles: vec![entry("t", Target::Page(0))],
            shortcuts: vec![entry("s", Target::Action("refresh".into()))],
            ..Module::default()
        }],
        ..View::default()
    });
    let data = data(true);
    app.handle(Button::Start, &data);
    for _ in 0..5 {
        app.handle(Button::R, &data);
    }
    assert!(
        matches!(app.screen, Screen::StartHub { tile: 3, .. }),
        "{:?}",
        app.screen
    );
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Module(Request::Open {
            module: "bateria".into(),
            page: 0
        })
    );
    app.handle(Button::Down, &data);
    for _ in 0..3 {
        app.handle(Button::R, &data);
    }
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Module(Request::Invoke {
            module: "bateria".into(),
            action: "refresh".into(),
            call: None
        })
    );
    // Suspend moved one place right and still suspends.
    app.handle(Button::R, &data);
    assert_eq!(
        app.handle(Button::A, &data),
        Outcome::Action(SessionAction::Suspend)
    );
}

#[test]
fn a_module_item_joins_the_quick_menu_before_suspend() {
    use archr_ui::modules::{Context, Module, Quick, Request, View};
    use archr_ui::state::QuickItem;
    let mut app = App::new(8);
    app.modules = std::sync::Arc::new(View {
        modules: vec![Module {
            id: "perfil-cpu".into(),
            enabled: true,
            granted: true,
            quick: vec![Quick {
                id: "q".into(),
                label: "Perfil".into(),
                context: Context::System,
                systems: Vec::new(),
                action: "sugerir".into(),
            }],
            ..Module::default()
        }],
        ..View::default()
    });
    let data = data(true);
    app.handle(Button::Y, &data);
    let items = app.quick_menu_items(&data);
    let at = items
        .iter()
        .position(|i| *i == QuickItem::Module(0))
        .unwrap();
    assert_eq!(items[at + 1], QuickItem::Suspend);
    app.cursor = at;
    let Outcome::Module(Request::Invoke {
        module,
        action,
        call,
    }) = app.handle(Button::A, &data)
    else {
        panic!("no module call");
    };
    assert_eq!(
        (module.as_str(), action.as_str()),
        ("perfil-cpu", "sugerir")
    );
    assert!(call.is_some());
    assert_eq!(app.screen, Screen::Systems);
}
