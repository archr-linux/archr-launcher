//! System › Error reports, the last report's text and the notice over the
//! Start screen, and the question of a port that needs panfrost, draw in
//! every language and at every screen size without a text the layout
//! cannot hold.
mod common;
use archr_ui::{
    i18n::I18n,
    motion::Motion,
    render::Ui,
    settings::{
        reports::{Facts, Last, Notice},
        Id, Page, Settings,
    },
    state::{App, Screen},
    Theme,
};
use common::FakeData;
use std::path::{Path, PathBuf};

struct Warnings;
static WARNINGS: Warnings = Warnings;
static SEEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
impl log::Log for Warnings {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        let text = record.args().to_string();
        if text.contains("unavailable in this layout") {
            SEEN.lock().unwrap().push(text);
        }
    }
    fn flush(&self) {}
}

const REPORT: &str = "ArchR error report
when: 2026-11-10 12:31:05 -0300
event: game
what: mupen64plus-arch
system: n64 (Nintendo 64)
game: Super Mario 64 (USA).z64
exit: 139 (SIGSEGV) after 42 s
signature: 7f3a9c21

--- command ---
runemu.sh /storage/roms/n64/Super\\ Mario\\ 64\\ \\(USA\\).z64 -Pn64 --core=mupen64plus-arch --emulator=mupen64plus

--- state ---
device: Game Console R36S
archr: 20261110 (0123456789, community)
kernel: 6.12.114
gpu: libmali
cpu: performance, 1416 MHz now, 1416 MHz max

--- run output (exec.log, last 300 lines) ---
Core: Loading 8MB RDRAM
Video: GLideN64 rev.4c2b1d
Video: gles 3.1 context ok
Input: r36s_Gamepad mapped
Core: Starting R4300 emulator
Video: texture cache full, flushing; a line long enough to wrap at the viewer's column limit
Segmentation fault
";

fn reports(page: Page) -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.open(Page::System);
    state.open(Page::ErrorReports);
    state.values.insert(Id::ErrorReports, "1".into());
    state.reports.facts = Some(Facts {
        queued: 1,
        sent_today: 2,
        last: Some(Last::new(
            "2026-11-10_12-31-05.txt".into(),
            true,
            "12:31".into(),
            REPORT,
        )),
    });
    if page == Page::ReportText {
        state.open(Page::ReportText);
    }
    state
}

/// The Start screen with the notice over it.
fn start() -> App {
    let mut app = App::new(9);
    app.screen = Screen::StartHub {
        row: 0,
        tile: 1,
        shortcut: 0,
    };
    app.reports_notice = Some(Notice::default());
    app
}
/// A port's games list with Ports-panfrost's question over it.
fn panfrost() -> App {
    let mut app = App::new(9);
    app.screen = Screen::games("ports");
    app.panfrost = Some("Bugdom 2: The Lost Gnome Kingdoms".into());
    app
}
#[test]
fn error_reports_draw_in_every_language_at_every_size() {
    log::set_logger(&WARNINGS).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let temp = tempfile::tempdir().unwrap();
    let themes: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let default = Theme::load(&themes.join("default"));
    let data = FakeData::default();
    for (width, height) in [(640, 480), (720, 720), (480, 320)] {
        let ttf;
        let creator;
        let mut ui = Ui::new(width, height, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        let theme = default.fitted(width, height);
        ui.theme_dir = Some(temp.path().to_path_buf());
        ui.default_theme_dir = Some(themes.join("default"));
        ui.configure(&ttf, &creator, &theme, Some(temp.path()))
            .unwrap();
        ui.set_motion(Motion::Off);
        for language in ["en", "pt_BR", "es", "ru"] {
            ui.catalog = I18n::load(Path::new("/nonexistent"), Some(language), None);
            for (name, app, state) in [
                ("ajustes", App::new(9), Some(reports(Page::ErrorReports))),
                ("ver", App::new(9), Some(reports(Page::ReportText))),
                ("aviso", start(), None),
                ("panfrost", panfrost(), None),
            ] {
                ui.screenshot = Some(
                    temp.path()
                        .join(format!("{name}-{language}-{width}x{height}.png")),
                );
                for _ in 0..2 {
                    ui.snap_animations();
                    ui.draw_with_settings(&app, &data, &theme, Default::default(), state.as_ref())
                        .unwrap();
                }
            }
        }
    }
    let seen = SEEN.lock().unwrap();
    assert!(seen.is_empty(), "{seen:#?}");
}
#[test]
fn the_panfrost_question_switches_on_a_and_closes_on_b() {
    use archr_ui::state::{Button, Outcome};
    let data = FakeData::default();
    let mut app = panfrost();
    assert_eq!(app.handle(Button::Down, &data), Outcome::None);
    assert!(app.panfrost.is_some(), "only A or B answer");
    assert_eq!(app.handle(Button::A, &data), Outcome::Panfrost);
    assert!(app.panfrost.is_none());
    let mut app = panfrost();
    assert_eq!(app.handle(Button::B, &data), Outcome::None);
    assert!(app.panfrost.is_none());
    assert_eq!(app.screen, Screen::games("ports"), "B stays on the list");
}
