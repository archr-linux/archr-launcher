//! The shelves drawn at 640x480, 720x720 and 480x320 with the test theme
//! (tests/themes/prateleiras) and with the shipped Ellegance: every tab and
//! the games grid fit and draw. With ARCHR_HOME_CAPTURE_DIR (the test
//! theme) or ARCHR_ELLEGANCE_CAPTURE_DIR (Ellegance) set, each frame is
//! saved there as a PNG. Ellegance's 640x480 frames are golden
//! (tests/golden/ellegance-640x480); ARCHR_GOLDEN_UPDATE=1 writes them again.
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
use archr_ui::layout::LISTA;
use std::path::{Path, PathBuf};

const SIZES: [(u32, u32); 3] = [(640, 480), (720, 720), (480, 320)];

/// SDL video runs one window at a time per process.
static SDL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One theme drawn on every screen: where its pictures go, the 640x480
/// frames kept for the golden comparison, and the time games were played.
/// Every frame is drawn at `NOW` (the launcher's clock pinned, in UTC), so
/// what prints a date or runs a timer reads the same on every run.
struct Run {
    theme: Theme,
    out: Option<PathBuf>,
    keep: Option<Vec<(String, Vec<u8>)>>,
    played: u64,
}
/// 2026-11-17 12:00 UTC: the moment every frame is drawn at.
const NOW: u64 = 1_794_916_800;
/// The launcher's clock stopped at `NOW` in UTC, and `instant` standing for
/// it: toasts are pushed against it.
fn pin(ui: &mut Ui, instant: std::time::Instant) {
    extern "C" {
        fn tzset();
    }
    std::env::set_var("TZ", "UTC");
    // SAFETY: tzset only re-reads TZ; the tests that draw run one at a
    // time (SDL), and nothing else reads the zone meanwhile.
    unsafe { tzset() };
    ui.pin_clock(instant, NOW);
}
/// Draws until every picture asked for has loaded, so a frame never
/// depends on how fast the workers were: `draw` asks, polls and draws, and
/// says whether its caches are idle.
fn settle(mut draw: impl FnMut() -> bool) {
    // Generous: the workers share the machine with the rest of the tests.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut round = 0;
    while std::time::Instant::now() < deadline {
        if draw() && round >= 2 {
            // Once more over what the last poll brought in.
            draw();
            return;
        }
        round += 1;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("pictures never finished loading");
}

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

fn game(name: &str, played: u64) -> Game {
    Game {
        path: format!("{name}.rom").into(),
        name: name.into(),
        image: None,
        video: None,
        favorite: false,
        year: Some(1996),
        players: Some("1-2".into()),
        last_played: Some(played),
        details: Default::default(),
    }
}
fn library(played: u64) -> Library {
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
            .insert(name.into(), games.iter().map(|g| game(g, played)).collect());
    }
    data.games.insert(
        "@favorites".into(),
        vec![
            game("Kirby Super Star", played),
            game("Super Metroid", played),
            game("Tetris", played),
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
fn load(dir: &Path, view: &str) -> Theme {
    let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let default = Theme::load(&themes.join("default"));
    let mut theme = Theme::load_over(dir, &default);
    theme.active_view = view.into();
    theme
}
fn theme() -> Theme {
    load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/themes/prateleiras"),
        "prateleiras",
    )
}
/// The shipped Ellegance, on the view a new install opens (its shelves).
fn ellegance() -> Theme {
    load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/ellegance"),
        "lista",
    )
}
#[test]
fn the_test_theme_asks_for_the_capsule_osd() {
    use archr_ui::screens::{OsdEdge, OsdShape};
    let osd = theme().screens.osd;
    assert_eq!(
        (osd.style, osd.position),
        (OsdShape::Capsule, OsdEdge::Right)
    );
    assert_eq!(osd.track.map(|c| c.a), Some(0xA6));
    assert_eq!(osd.radius, Some(0.0375));
}
fn recent(played: u64) -> Vec<Recent> {
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
        left: Some(played),
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
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    let mut run = Run {
        theme: theme(),
        out: std::env::var_os("ARCHR_HOME_CAPTURE_DIR").map(PathBuf::from),
        keep: None,
        played: 1_700_000_000,
    };
    shelves_draw(&mut run);
    every_screen_table_draws_at_every_size(&mut run);
}

/// Ellegance (themes/ellegance) on every screen at the three sizes, and its
/// 640x480 frames pixel for pixel against tests/golden/ellegance-640x480:
/// every one, those that print dates or run a timer too (the clock is
/// pinned, the pictures loaded before each frame is kept).
#[test]
fn ellegance_draws_every_screen() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    let mut run = Run {
        theme: ellegance(),
        out: std::env::var_os("ARCHR_ELLEGANCE_CAPTURE_DIR").map(PathBuf::from),
        keep: Some(Vec::new()),
        played: NOW - 2 * 86_400,
    };
    assert_eq!(run.theme.name.get("en"), Some("Ellegance"));
    assert!(run.theme.font_bold.is_some(), "Ellegance ships Inter Bold");
    shelves_draw(&mut run);
    every_screen_table_draws_at_every_size(&mut run);
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/ellegance-640x480");
    let update = std::env::var_os("ARCHR_GOLDEN_UPDATE").is_some();
    let mut changed = Vec::new();
    for (name, pixels) in run.keep.take().unwrap() {
        let path = golden.join(format!("{name}.png"));
        if update {
            std::fs::create_dir_all(&golden).unwrap();
            image::save_buffer(&path, &pixels, 640, 480, image::ColorType::Rgba8).unwrap();
            continue;
        }
        let expected = image::open(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .to_rgba8();
        let off = expected
            .as_raw()
            .chunks_exact(4)
            .zip(pixels.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        if off > 0 {
            let actual = std::env::temp_dir().join(format!("archr-golden-ellegance-{name}.png"));
            image::save_buffer(&actual, &pixels, 640, 480, image::ColorType::Rgba8).unwrap();
            changed.push(format!("{name}: {off} pixels (now {})", actual.display()));
        }
    }
    assert!(
        changed.is_empty(),
        "Ellegance 640x480 changed: {changed:#?}"
    );
}

fn shelves_draw(run: &mut Run) {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = run.out.clone();
    let mut keep = run.keep.take();
    let data = library(run.played);
    for size in SIZES {
        let theme = run.theme.fitted(size.0, size.1);
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
        ui.set_status(archr_ui::status::Status::frozen("14:32"));
        let instant = std::time::Instant::now();
        pin(&mut ui, instant);
        let mut app = App::new(10);
        app.set_view(&view);
        app.recent = recent(run.played);
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
            // Pictures load on workers: wait for every one of them.
            settle(|| {
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
                covers.idle() && pictures.idle() && ui.pictures_idle()
            });
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
            if let Some(keep) = keep.as_mut().filter(|_| size == (640, 480)) {
                keep.push((format!("home-{name}"), pixels));
            }
        };
        assert!(matches!(app.screen, Screen::Home { .. }));
        shot(&mut ui, "inicio", &app, None);
        // The showcase's buttons: More in focus, then its menu beside it.
        app.handle(Button::Up, &data);
        app.handle(Button::Right, &data);
        app.handle(Button::Right, &data);
        assert_eq!(app.home_hero, Some(2));
        shot(&mut ui, "inicio-destaque", &app, None);
        app.handle(Button::A, &data);
        assert!(matches!(app.screen, Screen::QuickMenu { game: Some(_) }));
        shot(&mut ui, "inicio-mais", &app, None);
        app.handle(Button::B, &data);
        app.handle(Button::Down, &data);
        assert_eq!(app.home_hero, None);
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
        // Pushed a second and a half before the pinned clock: its timer
        // stands at the same place on every run.
        ui.toasts.push(
            Toast::new(Kind::Info, Category::Updates, "Atualização disponível")
                .detail("3 pacotes do sistema estão prontos para instalar."),
            instant - std::time::Duration::from_millis(1500),
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
    run.keep = keep;
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
/// SDL lives on one thread: each theme's test runs this after its shelves.
fn every_screen_table_draws_at_every_size(run: &mut Run) {
    use archr_ui::{
        cheevos::{Fetch, Item as Cheevo, Sheet},
        keyboard::{Keyboard, Mode},
        resume::Boot,
        saves::{Kind as SaveKind, Progress, Save, Shot},
        settings::Id,
        state::{QuickItem, Search},
    };
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = run.out.clone();
    let mut keep = run.keep.take();
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
    let mut data = library(run.played);
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
    // Pictures of this week and before (the sections of [shots]).
    let now = NOW;
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
        let theme = run.theme.fitted(size.0, size.1);
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
        ui.set_status(archr_ui::status::Status::frozen("14:32"));
        pin(&mut ui, std::time::Instant::now());
        let mut covers =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("covers"), 320, 240);
        let mut thumbs =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("thumbs"), 160, 120);
        let mut pictures =
            archr_ui::covers::CoverCache::new(&creator, root.path().join("big"), size.0, size.1);
        let mut shot = |ui: &mut Ui, name: &str, app: &App, settings: Option<&Settings>| {
            let wanted: Vec<PathBuf> = pictures_at.clone();
            settle(|| {
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
                covers.idle() && thumbs.idle() && pictures.idle() && ui.pictures_idle()
            });
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
            if let Some(keep) = keep
                .as_mut()
                .filter(|_| size == (640, 480) && !name.starts_with('_'))
            {
                keep.push((format!("tela-{name}"), pixels));
            }
        };
        let fresh = || {
            let mut app = App::new(10);
            app.set_view(&view);
            app.recent = recent(run.played);
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
        // [app]: an app's catalog (API 4) with the focused item's picture.
        ui.app = Some(app_session(&pictures_at[0]));
        shot(&mut ui, "app", &fresh(), None);
        ui.app = None;
    }
    run.keep = keep;
}

/// An app (API 4) on its catalog: chips, four items with pictures, the
/// second one focused.
fn app_session(picture: &Path) -> archr_ui::apps::Session {
    use archr_ui::{
        apps::{Action, Chip, Item, Message, Screen as Page, Session},
        modules::Tone,
    };
    let mut session = Session::new("itchio", "Jogos do itch.io", true);
    let items = [
        "Opossum Country",
        "Dragon Diving",
        "Tobu Tobu Girl Deluxe",
        "Pixel Hop",
        "Blue Lines",
    ]
    .iter()
    .enumerate()
    .map(|(n, title)| Item {
        id: format!("g{n}"),
        title: (*title).into(),
        subtitle: "Game Boy · 2026".into(),
        badge: if n == 1 { "Instalado" } else { "Grátis" }.into(),
        tone: if n == 1 { Tone::Good } else { Tone::Info },
        image: Some(picture.to_path_buf()),
    })
    .collect();
    let chips = ["Todos", "Comprados", "GB", "GBC", "GBA", "NES"]
        .iter()
        .enumerate()
        .map(|(n, label)| Chip {
            id: format!("c{n}"),
            label: (*label).into(),
            selected: n == 0,
        })
        .collect();
    session.apply(Message::Show(Box::new(Page {
        id: "catalog".into(),
        title: "itch.io".into(),
        chips,
        items,
        y: Some(Action {
            label: "Populares".into(),
            action: "sort".into(),
            ..Action::default()
        }),
        footer: "270 jogos".into(),
        ..Page::default()
    })));
    session.top_mut().unwrap().cursor = 1;
    session
}

/// An app's catalog keeps the focused item's picture and rows of a usable
/// height in every view of the shipped themes, a grid or shelves included
/// (an app on Ellegance's grid once drew rows a fifth of the screen tall
/// and no picture at all).
#[test]
fn an_app_shows_its_pictures_in_every_view() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let root = tempfile::tempdir().unwrap();
    let picture = painted(root.path(), "app", (320, 240), [70, 40, 160]);
    let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    for dir in ["default", "ellegance"] {
        let base = load(&themes.join(dir), LISTA);
        for (id, _) in base.views() {
            let mut theme = base.clone();
            theme.active_view.clone_from(&id);
            let theme = theme.fitted(640, 480);
            let view = theme.view(&id);
            let art = view.app.art.expect("an app always has its picture");
            assert!(
                view.app.list.row < 0.12 && view.app.list.mark != archr_ui::layout::Mark::Grid,
                "{dir}/{id}: app rows {:?}",
                view.app.list
            );
            let (ttf, creator);
            let mut ui = Ui::new(640, 480, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.set_motion(archr_ui::motion::Motion::Off);
            ui.configure(&ttf, &creator, &theme, Some(root.path())).unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            let mut covers =
                archr_ui::covers::CoverCache::new(&creator, root.path().join("c"), 320, 240);
            ui.app = Some(app_session(&picture));
            let mut app = App::new(10);
            app.set_view(&view);
            let data = FakeData::default();
            settle(|| {
                covers.request(std::slice::from_ref(&picture));
                covers.poll();
                ui.snap_animations();
                let art = archr_ui::render::Art {
                    cover: covers.peek(&picture),
                    ..Default::default()
                };
                ui.draw_with_settings(&app, &data, &theme, art, None).unwrap();
                covers.idle()
            });
            let pixels = ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
            // The picture's light disc, inside the art's box.
            let r = archr_ui::render::pixels(art.rect, 640, 480);
            let disc = pixels.chunks_exact(4).enumerate().filter(|(n, p)| {
                let (x, y) = ((n % 640) as i32, (n / 640) as i32);
                r.contains_point((x, y)) && p[0] > 235 && p[1] > 220 && p[2] > 160 && p[2] < 200
            });
            assert!(disc.count() > 100, "{dir}/{id}: the app's picture is missing");
        }
    }
}

/// The brightest pixel of the footer band (its bottom tenth).
fn footer_light(pixels: &[u8], (w, h): (u32, u32)) -> u32 {
    let top = (h - h / 10) as usize;
    pixels[top * w as usize * 4..]
        .chunks_exact(4)
        .map(|p| (u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])) / 3)
        .max()
        .unwrap_or(0)
}

/// A layer over a screen (a question, an app's question or keyboard)
/// offers its own hints, as bright as a screen's: never left dimmed under
/// it, for both shipped themes.
#[test]
fn a_layer_keeps_its_hints_bright() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let root = tempfile::tempdir().unwrap();
    let picture = painted(root.path(), "app", (320, 240), [70, 40, 160]);
    let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    for dir in ["default", "ellegance"] {
        let theme = load(&themes.join(dir), LISTA).fitted(640, 480);
        let view = theme.view(LISTA);
        let (ttf, creator);
        let mut ui = Ui::new(640, 480, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        ui.set_motion(archr_ui::motion::Motion::Off);
        ui.configure(&ttf, &creator, &theme, Some(root.path())).unwrap();
        ui.set_status(archr_ui::status::Status::frozen("14:32"));
        let mut app = App::new(10);
        app.set_view(&view);
        let data = FakeData::default();
        let mut light = |ui: &mut Ui, settings: Option<&Settings>| {
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw_with_settings(&app, &data, &theme, Default::default(), settings)
                    .unwrap();
            }
            let pixels = ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
            footer_light(&pixels, (640, 480))
        };
        let mut settings = Settings::default();
        settings.loading = false;
        settings.open(Page::Wifi);
        settings.open(Page::Bluetooth);
        let plain = light(&mut ui, Some(&settings));
        settings.confirm_forget = Some("00:1A:2B:3C:4D:5E".into());
        let asked = light(&mut ui, Some(&settings));
        assert!(asked * 10 >= plain * 9, "{dir}: a question's hints dimmed ({asked} < {plain})");
        let mut session = app_session(&picture);
        ui.app = Some(session.clone());
        let screen = light(&mut ui, None);
        session.ask = Some(archr_ui::apps::Ask {
            text: "Baixar?".into(),
            ..Default::default()
        });
        ui.app = Some(session.clone());
        let asked = light(&mut ui, None);
        assert!(asked * 10 >= screen * 9, "{dir}: an app's question dims its hints ({asked} < {screen})");
        session.ask = None;
        session.keyboard = Some(archr_ui::keyboard::Keyboard::new(
            archr_ui::keyboard::Mode::Text,
            String::new(),
        ));
        ui.app = Some(session);
        let typing = light(&mut ui, None);
        assert!(typing * 10 >= screen * 9, "{dir}: an app's keyboard dims its hints ({typing} < {screen})");
        ui.app = None;
    }
}
