//! The shelves drawn at 640x480, 720x720 and 480x320 with the test theme
//! (tests/themes/prateleiras): every tab and the games grid fit and draw.
//! With ARCHR_HOME_CAPTURE_DIR set, each frame is saved there as a PNG.
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    apps::Row,
    render::Ui,
    resume::Recent,
    settings::{Page, Settings},
    state::{App, Button, DataSource, Screen},
    toast::{Category, Kind, Toast},
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
        let mut shot = |ui: &mut Ui, name: &str, app: &App, settings: Option<&Settings>| {
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
                ui.draw_with_settings(app, &data, &theme, art, settings)
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
        shot(&mut ui, "inicio", &app, None);
        app.handle(Button::Down, &data);
        app.handle(Button::Right, &data);
        shot(&mut ui, "inicio-sistemas", &app, None);
        app.handle(Button::Down, &data);
        shot(&mut ui, "inicio-app", &app, None);
        app.handle(Button::R, &data);
        shot(&mut ui, "biblioteca", &app, None);
        app.handle(Button::A, &data);
        app.handle(Button::Right, &data);
        app.handle(Button::Right, &data);
        app.handle(Button::Right, &data);
        shot(&mut ui, "jogos", &app, None);
        app.handle(Button::B, &data);
        app.handle(Button::R, &data);
        shot(&mut ui, "favoritos", &app, None);
        app.handle(Button::Y, &data);
        shot(&mut ui, "favoritos-menu", &app, None);
        // [toast]: a notice at the top, on glass.
        app.handle(Button::B, &data);
        ui.toasts.push(
            Toast::new(Kind::Info, Category::Updates, "Atualização disponível")
                .detail("3 pacotes do sistema estão prontos para instalar."),
            std::time::Instant::now(),
        );
        shot(&mut ui, "aviso", &app, None);
        // [dialog]: a question with its icon and answers side by side.
        let mut settings = Settings::default();
        settings.loading = false;
        settings.open(Page::Wifi);
        settings.open(Page::Bluetooth);
        settings.cursor = 2;
        settings.confirm_forget = Some("00:1A:2B:3C:4D:5E".into());
        shot(&mut ui, "dialogo", &app, Some(&settings));
    }
    every_screen_table_draws_at_every_size();
}

/// A picture of `w`×`h` in a shade of its own, saved under `root`.
fn painted(root: &Path, name: &str, (w, h): (u32, u32), tint: [u8; 3]) -> PathBuf {
    let path = root.join(format!("{name}.png"));
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let mut c = [0u8; 3];
        for k in 0..3 {
            c[k] = (f32::from(tint[k]) * (0.45 + 0.55 * fx) + 60. * fy).min(255.) as u8;
        }
        // A light disc, so a blur or a crop shows.
        let (dx, dy) = (fx - 0.62, fy - 0.42);
        if dx * dx + dy * dy < 0.03 {
            c = [250, 236, 180];
        }
        *px = image::Rgba([c[0], c[1], c[2], 255]);
    }
    img.save(&path).unwrap();
    path
}

/// Every screen the theme's tables restyle (docs/temas.md, Uma tabela por
/// tela), drawn with the test theme at the three sizes. With
/// ARCHR_HOME_CAPTURE_DIR set, each frame is saved there as tela-*.png.
/// SDL lives on one thread: the shelves' test runs this after its own.
fn every_screen_table_draws_at_every_size() {
    use archr_ui::{
        cheevos::{Fetch, Item as Cheevo, Sheet},
        keyboard::{Keyboard, Mode},
        resume::Boot,
        saves::{Kind as SaveKind, Progress, Save, Shot},
        settings::Id,
        state::{QuickItem, Search},
    };
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_HOME_CAPTURE_DIR").map(PathBuf::from);
    let root = tempfile::tempdir().unwrap();
    let pictures_at: Vec<PathBuf> = [
        [70, 40, 160],
        [180, 60, 40],
        [40, 140, 90],
        [30, 90, 170],
        [150, 120, 30],
        [120, 40, 120],
    ]
    .iter()
    .enumerate()
    .map(|(n, tint)| painted(root.path(), &format!("p{n}"), (320, 240), *tint))
    .collect();
    let mut data = library();
    for (n, game) in data.0.games.get_mut("snes").unwrap().iter_mut().enumerate() {
        game.image = Some(pictures_at[n % pictures_at.len()].clone());
    }
    {
        let kirby = &mut data.0.games.get_mut("snes").unwrap()[0];
        kirby.details.genre = "Plataforma".into();
        kirby.details.developer = "HAL Laboratory".into();
        kirby.details.description = "Kirby volta em oito aventuras: engula os inimigos, copie seus poderes e jogue com um amigo no modo cooperativo.".into();
        kirby.details.screenshot = Some(pictures_at[1].clone());
    }
    let now = 1_700_000_000u64;
    let saves: Vec<Save> = (0..4)
        .map(|n| Save {
            path: root.path().join(format!("state{n}")),
            picture: Some(pictures_at[n].clone()),
            slot: (n > 0).then_some(n as u16),
            saved: now - 86_400 * (n as u64 * 2 + 1),
            bytes: 1 << 20,
            kind: SaveKind::RetroArch,
            emulator: "RetroArch".into(),
            playable: true,
            backup: Vec::new(),
        })
        .collect();
    let shots: Vec<Shot> = (0..7)
        .map(|n| Shot {
            path: pictures_at[n % pictures_at.len()].clone(),
            taken: now - 86_400 * [0, 1, 3, 9, 12, 30, 40][n],
            bytes: 1 << 18,
            size: Some((320, 240)),
            details: false,
        })
        .collect();
    let sheet = Sheet {
        fetch: Fetch::Fresh,
        hardcore: true,
        items: (0..7)
            .map(|n| Cheevo {
                title: [
                    "Primeira estrela",
                    "Gourmet",
                    "Sem dano",
                    "Arena",
                    "Colecionador",
                    "Velocista",
                    "Lenda",
                ][n]
                    .into(),
                description: "Termine a fase sem perder uma vida.".into(),
                points: [5, 10, 10, 25, 25, 50, 100][n],
                unlocked: n < 2,
                badge: None,
            })
            .collect(),
    };
    for size in SIZES {
        let theme = theme().fitted(size.0, size.1);
        let view = theme.view(&theme.active_view);
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
        let mut covers =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("covers"), 320, 240);
        let mut thumbs =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("thumbs"), 160, 120);
        let mut pictures =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("big"), size.0, size.1);
        let mut shot = |ui: &mut Ui, name: &str, app: &App, settings: Option<&Settings>| {
            let rounds = if out.is_some() { 40 } else { 2 };
            let wanted: Vec<PathBuf> = pictures_at.clone();
            for _ in 0..rounds {
                let (small, large) = app.progress_paths();
                covers.request(&wanted);
                thumbs.request(&small);
                thumbs.request(&wanted);
                pictures.request(&large);
                pictures.request(&wanted);
                covers.poll();
                thumbs.poll();
                pictures.poll();
                ui.tick();
                ui.snap_animations();
                let art = archr_ui::render::Art {
                    cover: covers.peek(&pictures_at[0]),
                    shot: covers.peek(&pictures_at[1]),
                    covers: Some(&covers),
                    thumbs: Some(&thumbs),
                    pictures: Some(&pictures),
                    picture: large.first().and_then(|p| pictures.peek(p)),
                    ..Default::default()
                };
                ui.draw_with_settings(app, &data, &theme, art, settings)
                    .unwrap();
                if rounds > 2 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
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
            // A name with '_' draws only (what lies behind the next one).
            if let Some(out) = out.as_ref().filter(|_| !name.starts_with('_')) {
                let file = out.join(format!("tela-{name}-{}x{}.png", size.0, size.1));
                image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8).unwrap();
            }
        };
        let fresh = || {
            let mut app = App::new(10);
            app.set_view(&view);
            app.recent = recent();
            for (n, r) in app.recent.iter_mut().enumerate() {
                r.picture = Some(pictures_at[n].clone());
            }
            app.updates = 3;
            app
        };
        let snes = |game| ("snes".to_owned(), game);
        // [start]
        let mut app = fresh();
        shot(&mut ui, "_inicio", &app, None);
        app.handle(Button::Start, &data);
        assert!(
            matches!(app.screen, Screen::StartHub { .. }),
            "{:?}",
            app.screen
        );
        shot(&mut ui, "start", &app, None);
        app.handle(Button::Down, &data);
        shot(&mut ui, "start-atalhos", &app, None);
        // [menu]: a recent game's on the shelves, then a game's on the grid
        // with the focus on its first row.
        let mut app = fresh();
        app.handle(Button::Y, &data);
        assert!(matches!(app.screen, Screen::QuickMenu { game: Some(_) }));
        assert_eq!(app.quick_row(&data), 3, "Play, Favorite and Details");
        assert_eq!(app.quick_menu_items(&data)[0], QuickItem::Play);
        shot(&mut ui, "menu", &app, None);
        let mut app = fresh();
        app.handle(Button::R, &data);
        app.handle(Button::A, &data);
        app.handle(Button::Right, &data);
        app.handle(Button::Y, &data);
        app.handle(Button::Right, &data);
        assert_eq!(app.cursor, 1, "across the quick actions");
        app.handle(Button::Down, &data);
        assert_eq!(app.cursor, 3, "down to the first row");
        shot(&mut ui, "menu-lista", &app, None);
        // [details]
        let mut app = fresh();
        app.screen = Screen::Details {
            system: "snes".into(),
            game: 0,
        };
        shot(&mut ui, "detalhes", &app, None);
        // [achievements]
        let mut app = fresh();
        app.screen = Screen::Achievements {
            system: "snes".into(),
            game: 0,
        };
        app.sheet = sheet.clone();
        shot(&mut ui, "conquistas", &app, None);
        // [saves] and [shots], with their questions.
        let mut app = fresh();
        app.screen = Screen::Saves {
            system: "snes".into(),
            game: 0,
        };
        app.set_progress(
            Progress {
                game: Some(snes(0)),
                saves: saves.clone(),
                shots: shots.clone(),
                blocked: None,
            },
            &data,
        );
        shot(&mut ui, "saves", &app, None);
        app.handle(Button::X, &data);
        shot(&mut ui, "saves-apagar", &app, None);
        let mut app = fresh();
        app.screen = Screen::Shots {
            system: "snes".into(),
            game: 0,
        };
        app.set_progress(
            Progress {
                game: Some(snes(0)),
                saves: saves.clone(),
                shots: shots.clone(),
                blocked: None,
            },
            &data,
        );
        shot(&mut ui, "capturas", &app, None);
        // [switcher] and [resume]
        let mut app = fresh();
        app.switcher = app.recent.clone();
        app.cursor = 1;
        app.screen = Screen::Switcher;
        shot(&mut ui, "alternador", &app, None);
        let mut app = fresh();
        app.boot = Some(Boot {
            game: app.recent[0].clone(),
            ready: true,
            power_off: true,
            progress: 0.4,
        });
        app.screen = Screen::Resume;
        shot(&mut ui, "retomar", &app, None);
        // [search] and [keyboard]
        let mut app = fresh();
        app.search = Some(Search {
            keyboard: Keyboard::new(Mode::Text, "Super".into()),
            results: false,
        });
        app.screen = Screen::Search {
            system: "snes".into(),
        };
        shot(&mut ui, "busca", &app, None);
        let mut settings = Settings::default();
        settings.loading = false;
        settings.open(Page::Wifi);
        settings.open(Page::Keyboard);
        settings.keyboard = Some(Keyboard::new(Mode::Secret, "senha123".into()));
        settings.keyboard_target = Some(Id::WifiSwitch);
        settings.wifi_ssid = "Casa".into();
        shot(&mut ui, "teclado", &fresh(), Some(&settings));
    }
}
