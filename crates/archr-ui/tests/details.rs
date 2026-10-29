mod common;
use archr_index::{cache::SystemEntry, Details, Game};
use archr_ui::{
    i18n::I18n,
    render::{Art, Ui},
    state::{App, Button, QuickItem},
    Theme,
};
use common::FakeData;
use sdl2::{pixels::PixelFormatEnum, surface::Surface};

const DESCRIPTION: &str = "Crono, Marle e Lucca atravessam eras — da pré-história a um futuro devastado — para impedir a chegada de Lavos, a criatura que destruirá o mundo em 1999. O sistema de batalha ativo permite técnicas combinadas entre os personagens, e as escolhas ao longo da jornada levam a mais de dez finais diferentes. Considerado um dos maiores RPGs de todos os tempos, reúne o criador de Dragon Quest, o artista Akira Toriyama e o produtor de Final Fantasy.";

fn data(rich: bool) -> FakeData {
    let details = if rich {
        Details {
            description: format!("{DESCRIPTION}\n\n{DESCRIPTION}"),
            genre: "RPG".into(),
            developer: "Square".into(),
            publisher: "Nintendo of America".into(),
            rating: Some(96),
            ..Default::default()
        }
    } else {
        Details::default()
    };
    let game = Game {
        path: "Chrono Trigger (USA).sfc".into(),
        name: "Chrono Trigger".into(),
        image: None,
        video: None,
        favorite: rich,
        last_played: rich.then_some(1_700_000_000),
        year: Some(1995),
        players: Some("1".into()),
        details,
    };
    let mut data = FakeData {
        systems: vec![SystemEntry {
            name: "snes".into(),
            fullname: "Super Nintendo".into(),
            game_count: 1,
            dir_mtime: 0,
            source_fingerprint: 0,
        }],
        ..Default::default()
    };
    data.games.insert("snes".into(), vec![game]);
    data
}

fn open(data: &FakeData) -> App {
    let mut app = App::new(8);
    app.handle(Button::A, data);
    app.handle(Button::Y, data);
    app.cursor = app
        .quick_menu_items(data)
        .iter()
        .position(|i| *i == QuickItem::Details)
        .unwrap();
    app.handle(Button::A, data);
    app.can_rescrape = true;
    app
}

/// Generated stand-ins for the scraped art: a gradient "screen" and a wide "logo".
fn surface(width: u32, height: u32, logo: bool) -> Surface<'static> {
    let mut surface = Surface::new(width, height, PixelFormatEnum::RGBA32).unwrap();
    let pitch = surface.pitch() as usize;
    surface.with_lock_mut(|bytes| {
        for y in 0..height as usize {
            for x in 0..width as usize {
                let at = y * pitch + x * 4;
                let px = if logo {
                    let band = (x / 24 + y / 12) % 2 == 0;
                    let inside = y > 6 && y < height as usize - 6;
                    match (inside, band) {
                        (false, _) => [0, 0, 0, 0],
                        (true, true) => [245, 192, 74, 255],
                        (true, false) => [122, 59, 47, 255],
                    }
                } else {
                    [
                        (40 + x * 80 / width as usize) as u8,
                        (58 + y * 20 / height as usize) as u8,
                        (85 + x * 40 / width as usize) as u8,
                        255,
                    ]
                };
                bytes[at..at + 4].copy_from_slice(&px);
            }
        }
    });
    surface
}

fn ui_with_fonts(
    theme: &Theme,
) -> (
    Ui<'static, 'static>,
    &'static sdl2::render::TextureCreator<sdl2::video::WindowContext>,
) {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf: &'static _ = Box::leak(Box::new(sdl2::ttf::init().unwrap()));
    let mut ui = Ui::new(640, 480, true).unwrap();
    let creator: &'static _ = Box::leak(Box::new(ui.canvas.texture_creator()));
    ui.configure(ttf, creator, theme, None).unwrap();
    (ui, creator)
}

fn theme() -> Theme {
    Theme::load(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"))
}

#[test]
fn details_render_in_every_locale_with_and_without_art_and_text() {
    let theme = theme();
    let (mut ui, creator) = ui_with_fonts(&theme);
    let shot = creator
        .create_texture_from_surface(surface(256, 224, false))
        .unwrap();
    let logo = creator
        .create_texture_from_surface(surface(320, 80, true))
        .unwrap();
    for language in ["en", "pt_BR", "es", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for rich in [true, false] {
            let data = data(rich);
            let mut app = open(&data);
            let art = Art {
                cover: None,
                shot: rich.then_some(&shot),
                logo: rich.then_some(&logo),
                badges: None,
                video: None,
            };
            ui.draw_with_settings(&app, &data, &theme, art, None)
                .unwrap();
            // The long description scrolls; the empty one does not.
            assert_eq!(ui.details_overflow > 0, rich, "{language}");
            app.details_overflow = ui.details_overflow;
            app.handle(Button::Down, &data);
            app.details_focus = 2;
            ui.draw_with_settings(&app, &data, &theme, art, None)
                .unwrap();
        }
    }
}

#[test]
#[ignore = "manual capture, ARCHR_DETAILS_CAPTURE_DIR required"]
fn capture_details() {
    let output = std::path::PathBuf::from(std::env::var_os("ARCHR_DETAILS_CAPTURE_DIR").unwrap());
    let theme = theme();
    let (mut ui, creator) = ui_with_fonts(&theme);
    let shot = creator
        .create_texture_from_surface(surface(256, 224, false))
        .unwrap();
    let logo = creator
        .create_texture_from_surface(surface(320, 80, true))
        .unwrap();
    for language in ["pt_BR", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, rich, scrolled) in [
            ("details", true, false),
            ("details-scrolled", true, true),
            ("details-bare", false, false),
        ] {
            let data = data(rich);
            let mut app = open(&data);
            let art = Art {
                cover: None,
                shot: rich.then_some(&shot),
                logo: rich.then_some(&logo),
                badges: None,
                video: None,
            };
            ui.draw_with_settings(&app, &data, &theme, art, None)
                .unwrap();
            if scrolled {
                app.details_overflow = ui.details_overflow;
                for _ in 0..3 {
                    app.handle(Button::Down, &data);
                }
                app.details_focus = 2;
            }
            ui.screenshot = Some(output.join(format!("{name}-{language}.png")));
            ui.draw_with_settings(&app, &data, &theme, art, None)
                .unwrap();
        }
    }
}
