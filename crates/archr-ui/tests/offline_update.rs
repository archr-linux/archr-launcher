//! System › Updates with an update file on the card: its row (newer and
//! not newer), the question, the run and its failures draw in every
//! language and at every screen size without a text the layout cannot
//! hold. ARCHR_OFFLINE_CAPTURE_DIR keeps the pictures.
mod common;
use archr_ui::{
    i18n::I18n,
    motion::Motion,
    render::Ui,
    settings::{
        updates::{Card, Progress, Reboot, Run, Stage},
        Id, Settings,
    },
    state::App,
    state::Button,
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

const FILE: &str = "/storage/roms/update/archr-update-20261113.1.tar";

/// The Updates page with no network and the card's file found.
fn page(newer: bool) -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.info.version = "20261112".into();
    state.open_updates();
    state.updates.failed = Some("network".into());
    state.updates.card_found(Some(Card {
        file: FILE.into(),
        id: "20261113.1".into(),
        newer,
        changelog: [
            "- [launcher] new feature: update from the card",
            "- [system] fix the battery reading at 0%",
            "- [system] N64 games in .7z open again",
            "- [launcher] fix the clock after a restart",
        ]
        .map(String::from)
        .to_vec(),
    }));
    state.cursor = state
        .visible_rows()
        .iter()
        .position(|r| r.id == Id::UpdateCard)
        .expect("the card's row");
    state
}
fn run(run: Run) -> Settings {
    let mut state = page(true);
    state.updates.from_card = Some(FILE.into());
    state.updates.run = Some(run);
    state
}
fn failed(reason: &str) -> Settings {
    run(Run::Failed {
        reason: reason.into(),
        line: String::new(),
        changed: false,
    })
}
fn screens() -> Vec<(&'static str, Settings)> {
    let mut confirm = page(true);
    confirm.handle(Button::A);
    assert!(confirm.updates.confirm_card);
    vec![
        ("cartao", page(true)),
        ("cartao-antigo", page(false)),
        ("cartao-confirmar", confirm),
        (
            "cartao-preparando",
            run(Run::Working(Progress {
                stage: Stage::Extract,
                percent: 40,
                ..Default::default()
            })),
        ),
        (
            "cartao-instalando",
            run(Run::Working(Progress {
                stage: Stage::Install,
                package: "archr-launcher".into(),
                current: 3,
                count: 7,
                percent: 42,
                ..Default::default()
            })),
        ),
        (
            "cartao-concluida",
            run(Run::Done {
                reboot: Some(Reboot::Launcher),
                from: "20261112".into(),
                to: "20261113.1".into(),
                count: 7,
            }),
        ),
        ("cartao-invalido", failed("offline-invalid")),
        ("cartao-nao-novo", failed("offline-old")),
        ("cartao-incompleto", failed("offline-incomplete")),
    ]
}
#[test]
fn the_card_update_draws_in_every_language_at_every_size() {
    log::set_logger(&WARNINGS).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let temp = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("ARCHR_OFFLINE_CAPTURE_DIR")
        .map_or_else(|| temp.path().to_path_buf(), Into::into);
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
            for (name, state) in screens() {
                ui.screenshot = Some(dir.join(format!("{name}-{language}-{width}x{height}.png")));
                for _ in 0..2 {
                    ui.snap_animations();
                    ui.draw_with_settings(
                        &App::new(9),
                        &data,
                        &theme,
                        Default::default(),
                        Some(&state),
                    )
                    .unwrap();
                }
            }
        }
    }
    let seen = SEEN.lock().unwrap();
    assert!(seen.is_empty(), "{seen:#?}");
}
