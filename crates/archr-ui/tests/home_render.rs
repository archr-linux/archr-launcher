//! The shelves drawn at 640x480, 720x720 and 480x320 with the test theme
//! (tests/themes/prateleiras): every tab and the games grid fit and draw.
//! With ARCHR_HOME_CAPTURE_DIR set, each frame is saved there as a PNG.
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    apps::Row,
    render::Ui,
    resume::Recent,
    state::{App, Button, DataSource, Screen},
    Rect01, Theme,
};
use common::FakeData;
use std::path::{Path, PathBuf};

const SIZES: [(u32, u32); 3] = [(640, 480), (720, 720), (480, 320)];

/// A library with an app (API 4) beside the systems.
struct Library(FakeData, Vec<Row>);
impl DataSource for Library {
    fn systems(&self) -> &[SystemEntry] {
        self.0.systems()
    }
    fn games(&self, system: &str) -> &[Game] {
        self.0.games(system)
    }
    fn collection_len(&self, name: &str) -> usize {
        self.0.collection_len(name)
    }
    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        self.0.origin(system, game)
    }
    fn cores(&self, system: &str) -> Vec<(String, String)> {
        self.0.cores(system)
    }
    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)> {
        self.0.current_core(system, game)
    }
    fn apps(&self) -> &[Row] {
        &self.1
    }
}

fn game(name: &str) -> Game {
    Game {
        path: format!("{name}.rom").into(),
        name: name.into(),
        image: None,
        video: None,
        favorite: false,
        year: Some(1996),
        players: Some("1-2".into()),
        last_played: Some(1_700_000_000),
        details: Default::default(),
    }
}
fn library() -> Library {
    let mut data = FakeData::default();
    let names = [
        "Kirby Super Star",
        "Chrono Trigger",
        "Donkey Kong Country",
        "F-Zero",
        "Mega Man X",
        "Super Mario World",
        "Super Metroid",
        "The Legend of Zelda: A Link to the Past",
        "Yoshi's Island",
    ];
    for (name, fullname, games) in [
        ("snes", "Super Nintendo", &names[..]),
        ("psp", "PSP", &["God of War: Chains of Olympus"][..]),
        ("gb", "Game Boy", &["Fairy-chan DEMO", "Tetris"][..]),
        ("gba", "Game Boy Advance", &["Metroid Fusion"][..]),
        ("megadrive", "Mega Drive", &["Sonic the Hedgehog"][..]),
        ("n64", "Nintendo 64", &["Super Mario 64"][..]),
    ] {
        data.systems.push(SystemEntry {
            name: name.into(),
            fullname: fullname.into(),
            game_count: games.len() as u32,
            dir_mtime: 0,
            source_fingerprint: 0,
        });
        data.games
            .insert(name.into(), games.iter().map(|g| game(g)).collect());
    }
    data.games.insert(
        "@favorites".into(),
        vec![
            game("Kirby Super Star"),
            game("Super Metroid"),
            game("Tetris"),
        ],
    );
    for (n, origin) in [("snes", 0), ("snes", 6), ("gb", 1)].iter().enumerate() {
        data.origins
            .insert(("@favorites".into(), n), (origin.0.into(), origin.1));
    }
    let app = Row {
        key: "@app:itchio".into(),
        name: "Jogos do itch.io".into(),
        summary: "Jogos gratuitos e os que você comprou no itch.io.".into(),
        ..Row::default()
    };
    Library(data, vec![app])
}
fn theme() -> Theme {
    let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let default = Theme::load(&themes.join("default"));
    let mut theme = Theme::load_over(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/themes/prateleiras"),
        &default,
    );
    theme.active_view = "prateleiras".into();
    theme
}
fn recent() -> Vec<Recent> {
    [
        ("snes", 0, "Kirby Super Star"),
        ("psp", 0, "God of War: Chains of Olympus"),
        ("gb", 0, "Fairy-chan DEMO"),
    ]
    .iter()
    .map(|(system, game, name)| Recent {
        system: (*system).into(),
        game: *game,
        name: (*name).into(),
        left: Some(1_700_000_000),
        ..Recent::default()
    })
    .collect()
}

fn inside(rect: Rect01, (w, h): (u32, u32), what: &str) {
    let r = archr_ui::render::pixels(rect, w, h);
    assert!(
        r.x() >= 0 && r.y() >= 0 && r.right() <= w as i32 && r.bottom() <= h as i32,
        "{what} {rect:?} leaves the {w}x{h} screen"
    );
}

#[test]
fn the_shelves_fit_and_draw_at_every_size() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_HOME_CAPTURE_DIR").map(PathBuf::from);
    let data = library();
    for size in SIZES {
        let theme = theme().fitted(size.0, size.1);
        let view = theme.view(&theme.active_view);
        let home = view.home.clone().expect("the test view has shelves");
        for (what, rect) in [
            ("tabs", home.tabs_rect),
            ("hero", home.hero.rect),
            ("hero art", home.hero.art),
            ("heading", home.heading),
            ("library", home.library.rect),
            ("games list", view.games.list.rect),
            ("strip", view.games.strip.unwrap()),
        ] {
            inside(rect, size, what);
        }
        let root = tempfile::tempdir().unwrap();
        let (ttf, creator);
        let mut ui = Ui::new(size.0, size.1, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        ui.theme_dir = Some(root.path().to_path_buf());
        ui.default_theme_dir =
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"));
        ui.set_motion(archr_ui::motion::Motion::Off);
        ui.configure(&ttf, &creator, &theme, Some(root.path()))
            .unwrap();
        let mut app = App::new(10);
        app.set_view(&view);
        app.recent = recent();
        // A picture for the first recent game (where it was left) and its
        // cover: the showcase blurs it behind the screen.
        let picture = root.path().join("kirby.png");
        let mut img = image::RgbaImage::new(160, 120);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgba([(x * 255 / 160) as u8, 90, (y * 255 / 120) as u8, 255]);
        }
        img.save(&picture).unwrap();
        app.recent[0].picture = Some(picture.clone());
        let mut pictures =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("thumbs"), size.0, size.1);
        let mut covers =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("covers"), 320, 240);
        let mut shot = |ui: &mut Ui, name: &str, app: &App| {
            // Pictures load on workers: with captures, wait for them.
            let rounds = if out.is_some() { 40 } else { 2 };
            for _ in 0..rounds {
                let (cover_paths, picture_paths) = app.home_paths(&data);
                covers.request(&cover_paths);
                pictures.request(&picture_paths);
                pictures.request(std::slice::from_ref(&picture));
                covers.poll();
                pictures.poll();
                ui.tick();
                ui.snap_animations();
                let art = archr_ui::render::Art {
                    covers: Some(&covers),
                    pictures: Some(&pictures),
                    ..Default::default()
                };
                ui.draw_with_settings(app, &data, &theme, art, None)
                    .unwrap();
                if rounds > 2 {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
            let pixels = ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
            let first = &pixels[..4];
            assert!(
                pixels.chunks_exact(4).filter(|p| p != &first).count() > 1000,
                "{name} at {size:?} is empty"
            );
            if let Some(out) = &out {
                let file = out.join(format!("home-{name}-{}x{}.png", size.0, size.1));
                image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8).unwrap();
            }
        };
        assert!(matches!(app.screen, Screen::Home { .. }));
        shot(&mut ui, "inicio", &app);
        app.handle(Button::Down, &data);
        app.handle(Button::Right, &data);
        shot(&mut ui, "inicio-sistemas", &app);
        app.handle(Button::Down, &data);
        shot(&mut ui, "inicio-app", &app);
        app.handle(Button::R, &data);
        shot(&mut ui, "biblioteca", &app);
        app.handle(Button::A, &data);
        app.handle(Button::Right, &data);
        app.handle(Button::Right, &data);
        app.handle(Button::Right, &data);
        shot(&mut ui, "jogos", &app);
        app.handle(Button::B, &data);
        app.handle(Button::R, &data);
        shot(&mut ui, "favoritos", &app);
        app.handle(Button::Y, &data);
        shot(&mut ui, "favoritos-menu", &app);
    }
}
