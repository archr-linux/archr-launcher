//! The Modules row (Modulos-inicio, Modulos-grade and their Ellegance
//! boards): which modules it lists, that it is gone with none, that A opens
//! a module's page and B there comes back to the list, and Y its details.
//! The frames draw with Grafite at 640x480 and Ellegance at 640x480,
//! 720x720 and 480x320; with ARCHR_MODULES_CAPTURE_DIR set they are saved
//! there as PNGs.
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    i18n::I18n,
    layout::{ShelfKind, Tab},
    modules::{self, Module, Page as ModulePage, Request, Status, View},
    render::Ui,
    settings::{Effect, Page, Settings},
    state::{App, Button, DataSource, Item, Outcome, Screen},
    Theme,
};
use common::FakeData;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// SDL video runs one window at a time per process.
static SDL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const GAUGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/></svg>"#;

fn icon(name: &str) -> Arc<str> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/icons");
    std::fs::read_to_string(dir.join(format!("{name}.svg")))
        .unwrap()
        .into()
}

fn page() -> ModulePage {
    ModulePage {
        id: "main".into(),
        title: "Main".into(),
        rows: Vec::new(),
        detail: false,
        hint: None,
        aside: Vec::new(),
        aside_note: None,
    }
}
fn module(id: &str, name: &str, description: &str, icon: Option<Arc<str>>) -> Module {
    Module {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        author: "ArchR <contato@archr.dev>".into(),
        version: "1.0.0".into(),
        icon,
        enabled: true,
        granted: true,
        pages: vec![page()],
        ..Module::default()
    }
}
/// The three modules of the boards, as their manifests say them, and those
/// the row leaves out: off, not allowed yet, failed, without a page, and an
/// app (it has a row of its own).
fn view() -> View {
    let mut desempenho = module(
        "desempenho",
        "Desempenho e chip",
        "Mostra ao vivo o chip, os clocks e a temperatura deste console, define tetos \
         de CPU e GPU para todos os jogos e as tensões a partir do próximo boot e mede \
         CPU, memória e GPU, com um histórico de notas.",
        Some(GAUGE.into()),
    );
    desempenho.version = "0.3.0".into();
    let off = Module {
        enabled: false,
        ..module("off", "Off", "", None)
    };
    let asking = Module {
        granted: false,
        ..module("asking", "Asking", "", None)
    };
    let failed = Module {
        status: Status::Failed,
        ..module("failed", "Failed", "", None)
    };
    let pageless = Module {
        pages: Vec::new(),
        ..module("pageless", "Pageless", "", None)
    };
    let app = Module {
        app: Some(archr_ui::apps::Row::default()),
        ..module("itchio", "itch.io", "", None)
    };
    View {
        modules: vec![
            off,
            desempenho,
            asking,
            module(
                "bateria-detalhada",
                "Bateria detalhada",
                "Carga, potência, tensão e saúde da bateria no menu Start.",
                Some(icon("battery-full")),
            ),
            failed,
            pageless,
            app,
            module(
                "perfil-cpu",
                "Perfil de CPU",
                "Sugere um governador de CPU para cada sistema e mostra o atual.",
                Some(icon("cpu")),
            ),
        ],
        ..View::default()
    }
}

/// The boards' systems, Tools as tools, and the Modules row as the
/// launcher puts it in (`modules::collection`).
struct Library(FakeData);
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
    fn is_tools(&self, system: &str) -> bool {
        system == "tools"
    }
}
fn game(name: &str) -> Game {
    Game {
        path: format!("{name}.rom").into(),
        name: name.into(),
        image: None,
        video: None,
        favorite: false,
        last_played: None,
        year: None,
        players: None,
        details: Default::default(),
    }
}
fn library(view: &View) -> Library {
    let mut data = FakeData::default();
    for (name, fullname, games) in [
        ("pico-8", "PICO-8", 4),
        ("ports", "Ports", 37),
        ("psp", "PlayStation Portable", 3),
        ("scummvm", "ScummVM", 2),
        ("tools", "Tools", 16),
    ] {
        data.systems.push(SystemEntry {
            name: name.into(),
            fullname: fullname.into(),
            game_count: games,
            dir_mtime: 0,
            source_fingerprint: 0,
        });
        let list = (0..games)
            .map(|n| game(&format!("{fullname} {n}")))
            .collect();
        data.games.insert(name.into(), list);
    }
    let listed = view.listed_games();
    modules::collection(&mut data.systems, listed.len());
    if !listed.is_empty() {
        data.games.insert(modules::SYSTEM.into(), listed);
    }
    Library(data)
}
fn names(data: &Library) -> Vec<String> {
    data.games(modules::SYSTEM)
        .iter()
        .map(|g| g.name.clone())
        .collect()
}

#[test]
fn the_row_lists_the_modules_that_are_on_with_a_page() {
    let view = view();
    let data = library(&view);
    assert_eq!(
        names(&data),
        ["Desempenho e chip", "Bateria detalhada", "Perfil de CPU"]
    );
    let rows = App::system_rows(&data);
    assert_eq!(rows.last().map(String::as_str), Some(modules::SYSTEM));
    assert_eq!(rows[rows.len() - 2], "tools", "right after Tools");
    let entry = data.systems().last().unwrap();
    assert_eq!(entry.game_count, 3);
    // Each row stands for its module.
    for game in data.games(modules::SYSTEM) {
        let module = view.listed_module(game).unwrap();
        assert_eq!(module.name, game.name);
    }
}

#[test]
fn with_no_such_module_there_is_no_row() {
    let mut view = view();
    for module in &mut view.modules {
        module.enabled = false;
    }
    let data = library(&view);
    assert!(data.games(modules::SYSTEM).is_empty());
    assert!(!App::system_rows(&data).iter().any(|r| r == modules::SYSTEM));
    // Not on the shelves either.
    let mut app = App::new(10);
    app.set_view(&ellegance().view("lista"));
    for tab in [Tab::Home, Tab::Library] {
        for shelf in app.shelves(tab, &data) {
            assert!(!shelf.items.contains(&Item::System(modules::SYSTEM.into())));
        }
    }
}

#[test]
fn the_shelves_show_it_after_tools_among_the_systems() {
    let data = library(&view());
    let mut app = App::new(10);
    app.set_view(&ellegance().view("lista"));
    let shelves = app.shelves(Tab::Home, &data);
    let systems = shelves
        .iter()
        .find(|s| s.kind == ShelfKind::Systems)
        .unwrap();
    let last = systems.items.len() - 1;
    assert_eq!(systems.items[last], Item::System(modules::SYSTEM.into()));
    assert_eq!(systems.items[last - 1], Item::System("tools".into()));
    for shelf in shelves.iter().filter(|s| s.kind == ShelfKind::Collections) {
        assert!(!shelf.items.contains(&Item::System(modules::SYSTEM.into())));
    }
}

/// The list as the systems screen opens it.
fn open_list(app: &mut App, data: &Library) {
    app.cursor = App::system_rows(data)
        .iter()
        .position(|r| r == modules::SYSTEM)
        .unwrap();
    assert_eq!(app.handle(Button::A, data), Outcome::None);
    assert_eq!(app.screen, Screen::games(modules::SYSTEM));
}

#[test]
fn a_opens_the_page_and_b_there_comes_back_to_the_list() {
    let view = Arc::new(view());
    let data = library(&view);
    let mut app = App::new(10);
    open_list(&mut app, &data);
    app.handle(Button::Down, &data);
    let outcome = app.handle(Button::A, &data);
    assert_eq!(
        outcome,
        Outcome::Module(Request::Open {
            module: "bateria-detalhada".into(),
            page: 0,
        })
    );
    // The launcher opens Settings on that page.
    let mut settings = Settings::default();
    settings.loading = false;
    settings.set_modules(view.clone());
    settings.open_module_page("bateria-detalhada", 0);
    assert_eq!(settings.page(), Page::Module(3, 0));
    assert_eq!(settings.handle(Button::B), Effect::Close);
    // Under Settings the list waits where it was.
    assert_eq!(app.screen, Screen::games(modules::SYSTEM));
    assert_eq!(app.cursor, 1);
    // B on the list goes back to the systems, on the row.
    app.handle(Button::B, &data);
    assert_eq!(app.screen, Screen::Systems);
    assert_eq!(App::system_rows(&data)[app.cursor], modules::SYSTEM);
}

#[test]
fn y_opens_the_details_and_b_there_comes_back_too() {
    let view = Arc::new(view());
    let data = library(&view);
    let mut app = App::new(10);
    open_list(&mut app, &data);
    assert_eq!(
        app.handle(Button::Y, &data),
        Outcome::Module(Request::Details("desempenho".into()))
    );
    assert_eq!(app.screen, Screen::games(modules::SYSTEM), "no menu opens");
    assert_eq!(app.handle(Button::X, &data), Outcome::None);
    let mut settings = Settings::default();
    settings.loading = false;
    settings.set_modules(view.clone());
    settings.open_module_info("desempenho");
    assert_eq!(settings.page(), Page::ModuleInfo);
    // Open, from the details, then back through them.
    assert_eq!(settings.handle(Button::A), Effect::None);
    assert_eq!(settings.page(), Page::Module(1, 0));
    assert_eq!(settings.handle(Button::B), Effect::None);
    assert_eq!(settings.page(), Page::ModuleInfo);
    assert_eq!(settings.handle(Button::B), Effect::Close);
}

#[test]
fn the_list_follows_the_modules_and_closes_when_empty() {
    let mut view = view();
    let data = library(&view);
    let mut app = App::new(10);
    open_list(&mut app, &data);
    app.handle(Button::Down, &data);
    app.handle(Button::Down, &data);
    assert_eq!(app.cursor, 2);
    // Perfil de CPU switched off: the cursor stays on a module.
    view.modules[7].enabled = false;
    let data = library(&view);
    app.modules_changed(&data);
    assert_eq!(app.cursor, 1);
    for module in &mut view.modules {
        module.enabled = false;
    }
    let data = library(&view);
    app.modules_changed(&data);
    assert_eq!(app.screen, Screen::Systems);
    assert!(app.cursor < App::system_rows(&data).len());
}

fn themes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
}
fn ellegance() -> Theme {
    let default = Theme::load(&themes().join("default"));
    let mut theme = Theme::load_over(&themes().join("ellegance"), &default);
    theme.active_view = "lista".into();
    theme
}

/// Frames of the boards with `theme` at `size`, in Portuguese as drawn there.
fn frames(theme: &Theme, size: (u32, u32), out: Option<&Path>, tag: &str) {
    let root = tempfile::tempdir().unwrap();
    let (ttf, creator);
    let mut ui = Ui::new(size.0, size.1, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    ui.theme_dir = Some(root.path().to_path_buf());
    ui.default_theme_dir = Some(themes().join("default"));
    ui.set_motion(archr_ui::motion::Motion::Off);
    ui.catalog = I18n::load(Path::new("/nonexistent"), Some("pt_BR"), None);
    let theme = theme.fitted(size.0, size.1);
    ui.configure(&ttf, &creator, &theme, Some(root.path()))
        .unwrap();
    ui.set_status(archr_ui::status::Status::frozen("14:32"));
    let view = Arc::new(view());
    let data = library(&view);
    let mut app = App::new(10);
    app.set_view(&theme.view(&theme.active_view));
    app.modules = view.clone();
    app.recent = [("psp", 0), ("ports", 1), ("psp", 2), ("pico-8", 3)]
        .iter()
        .map(|(system, game)| archr_ui::resume::Recent {
            system: (*system).into(),
            game: *game,
            name: data.games(system)[*game].name.clone(),
            ..Default::default()
        })
        .collect();
    let shot = |ui: &mut Ui, name: &str, app: &App| {
        for _ in 0..3 {
            ui.tick();
            ui.snap_animations();
            ui.draw_with_settings(app, &data, &theme, Default::default(), None)
                .unwrap();
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
        if let Some(out) = out {
            let file = out.join(format!("{tag}-{name}-{}x{}.png", size.0, size.1));
            image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8).unwrap();
        }
    };
    if app.home.is_some() {
        let shelves = app.shelves(Tab::Home, &data);
        let shelf = shelves
            .iter()
            .position(|s| s.kind == ShelfKind::Systems)
            .unwrap();
        let item = shelves[shelf].items.len() - 1;
        app.screen = Screen::Home {
            tab: Tab::Home,
            shelf,
            item,
        };
        shot(&mut ui, "inicio", &app);
        app.handle(Button::A, &data);
    } else {
        app.cursor = App::system_rows(&data)
            .iter()
            .position(|r| r == modules::SYSTEM)
            .unwrap();
        shot(&mut ui, "inicio", &app);
        app.handle(Button::A, &data);
    }
    assert_eq!(app.screen, Screen::games(modules::SYSTEM));
    shot(&mut ui, "grade", &app);
    app.handle(Button::Right, &data);
    app.handle(Button::Down, &data);
    shot(&mut ui, "grade-2", &app);
}

#[test]
fn the_boards_draw_in_both_themes() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_MODULES_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let grafite = Theme::load(&themes().join("default"));
    frames(&grafite, (640, 480), out.as_deref(), "grafite");
    for size in [(640, 480), (720, 720), (480, 320)] {
        frames(&ellegance(), size, out.as_deref(), "ellegance");
    }
}
