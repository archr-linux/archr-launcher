mod common;
use archr_ui::{
    i18n::I18n,
    netplay::{Effect, Group, Join, Lobby, Rom, Room},
    render::Ui,
    state::{App, Button, Outcome, QuickItem, Screen},
    Theme,
};
use common::FakeData;

fn room(group: Group, game: &str, rom: Rom, join: bool) -> Room {
    Room {
        group,
        game: game.into(),
        host: "marcos".into(),
        place: "BR".into(),
        core: "snes9x".into(),
        relay: String::new(),
        latency_ms: Some(38),
        locked: game.starts_with('S'),
        rom,
        join: join.then(|| Join {
            game: game.into(),
            system: "snes".into(),
            rom: "/roms/snes/a.sfc".into(),
            emulator: "retroarch".into(),
            core: "snes9x".into(),
            ip: "203.0.113.9".into(),
            port: 55435,
            spectate: false,
        }),
    }
}
fn sample() -> Vec<Room> {
    vec![
        room(Group::Lan, "Pokémon Red", Rom::Same, true),
        room(Group::Online, "Super Mario World", Rom::Same, true),
        room(
            Group::Online,
            "Street Fighter II Turbo",
            Rom::Different,
            true,
        ),
        room(Group::Unavailable, "Tekken 3", Rom::NoCore, false),
    ]
}

#[test]
fn navigation_menu_refresh_and_join() {
    let mut lobby = Lobby::default();
    assert!(lobby.loading);
    assert_eq!(
        lobby.handle(Button::Y),
        Effect::None,
        "no refresh while loading"
    );
    lobby.set(sample(), false);
    lobby.handle(Button::Down);
    lobby.handle(Button::A);
    assert_eq!(lobby.menu, Some(0));
    lobby.handle(Button::Down);
    let Effect::Join(join) = lobby.handle(Button::A) else {
        panic!("expected a join");
    };
    assert!(join.spectate && join.game == "Super Mario World" && lobby.menu.is_none());
    lobby.handle(Button::A);
    let Effect::Join(join) = lobby.handle(Button::A) else {
        panic!("expected a join");
    };
    assert!(!join.spectate);
    // A room without a local game opens no menu.
    lobby.handle(Button::Down);
    lobby.handle(Button::Down);
    assert_eq!(lobby.cursor, 3);
    lobby.handle(Button::A);
    assert!(lobby.menu.is_none());
    assert_eq!(lobby.handle(Button::Y), Effect::Refresh);
    assert!(lobby.loading);
    lobby.set(Vec::new(), true);
    assert_eq!((lobby.cursor, lobby.failed), (0, true));
    assert_eq!(lobby.handle(Button::B), Effect::Close);
}

#[test]
fn quick_menu_offers_netplay_only_when_enabled() {
    let data = FakeData::default();
    let mut app = App::new(10);
    app.screen = Screen::QuickMenu { game: None };
    assert!(!app.quick_menu_items(&data).contains(&QuickItem::Netplay));
    app.netplay = true;
    let items = app.quick_menu_items(&data);
    let at = items.iter().position(|i| *i == QuickItem::Netplay).unwrap();
    app.cursor = at;
    assert_eq!(app.handle(Button::A, &data), Outcome::OpenNetplay);
    assert!(matches!(app.screen, Screen::Systems));
}

#[test]
fn every_locale_renders_the_lobby_and_menu_without_warnings() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let (app, data) = (App::new(10), FakeData::default());
    let shots = std::env::var_os("ARCHR_NETPLAY_SHOTS").map(std::path::PathBuf::from);
    for language in ["en", "pt_BR", "es", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        let mut lobby = Lobby::default();
        ui.lobby = Some(Lobby::default());
        ui.draw(&app, &data, &theme, None).unwrap(); // loading
        lobby.set(sample(), false);
        ui.lobby = Some(lobby);
        ui.draw(&app, &data, &theme, None).unwrap();
        if let Some(dir) = &shots {
            ui.screenshot = Some(dir.join(format!("lobby-{language}.png")));
            ui.draw(&app, &data, &theme, None).unwrap();
        }
        ui.lobby.as_mut().unwrap().handle(Button::A);
        ui.lobby.as_mut().unwrap().handle(Button::Down);
        if let Some(dir) = &shots {
            ui.screenshot = Some(dir.join(format!("menu-{language}.png")));
        }
        ui.draw(&app, &data, &theme, None).unwrap();
        ui.lobby = Some(Lobby {
            failed: true,
            loading: false,
            ..Lobby::default()
        });
        ui.draw(&app, &data, &theme, None).unwrap();
    }
}

#[test]
fn theme_carries_status_colours() {
    let theme = Theme::builtin();
    assert_eq!(
        (theme.success.r, theme.success.g, theme.success.b),
        (0x3F, 0xB9, 0x50)
    );
    assert_eq!(
        (theme.warning.r, theme.warning.g, theme.warning.b),
        (0xD2, 0x99, 0x22)
    );
}
