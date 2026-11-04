//! System › Diagnostics and Games › BIOS draw in every language without a
//! text the layout cannot hold. ARCHR_DIAG_CAPTURE_DIR keeps the pictures
//! (pt_BR and ru) to compare with the mockups.
mod common;
use archr_ui::{
    i18n::I18n,
    motion::Motion,
    render::Ui,
    settings::{bios, diag, Id, Page, Settings},
    state::App,
    Theme,
};
use common::FakeData;
use std::time::{Duration, SystemTime};

struct Warnings;
static WARNINGS: Warnings = Warnings;
static SEEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
impl log::Log for Warnings {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        // Fixtures have no installed icons; only layout warnings count.
        let text = record.args().to_string();
        if text.contains("unavailable in this layout") {
            SEEN.lock().unwrap().push(text);
        }
    }
    fn flush(&self) {}
}

const BIOS: &str = r#"{"systems":[
 {"id":"psx","name":"PlayStation","cores":"pcsx_rearmed · DuckStation","verdict":"builtin","free":"psx","free_installed":false,
  "files":[{"file":"scph5501.bin","md5":"490f666e1afb15b7362b406ed1cea246","state":"missing"},
           {"file":"scph1001.bin","md5":"dc2b9bf8da62ec93e868cfd29f0d067d","state":"missing"}]},
 {"id":"gba","name":"Game Boy Advance","cores":"gpSP · mGBA","verdict":"ok","free":null,"free_installed":false,
  "files":[{"file":"gba_bios.bin","md5":"a860e8c0b6d573d191e4ec7db1b1e4f6","state":"ok"}]},
 {"id":"nds","name":"Nintendo DS","cores":"DraStic · melonDS","verdict":"builtin","free":null,"free_installed":false,"files":[]},
 {"id":"dreamcast","name":"Dreamcast","cores":"Flycast","verdict":"optional","free":null,"free_installed":false,"files":[]},
 {"id":"segacd","name":"Sega CD","cores":"Genesis Plus GX","verdict":"missing","free":null,"free_installed":false,
  "files":[{"file":"bios_CD_U.bin","md5":"854b9150240a198070150e4566ae1290","state":"missing"}]},
 {"id":"neogeo","name":"Neo Geo","cores":"FBNeo","verdict":"missing","free":null,"free_installed":false,"files":[]},
 {"id":"pcenginecd","name":"PC Engine CD","cores":"Beetle PCE Fast","verdict":"bad","free":null,"free_installed":false,
  "files":[{"file":"syscard3.pce","md5":"38179df8f4ac870017db21ebcbf53114","state":"bad"}]}]}"#;

fn diagnostics() -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.open(Page::System);
    state.open(Page::Diagnostics);
    state.values.insert(Id::DebugMode, "1".into());
    state.diag.facts = Some(diag::Facts::parse(
        "PSTORE=ramoops\nCRASHES=0\n",
        Some(diag::Report {
            name: "archr-relatorio-20261030-2236.zip".into(),
            bytes: 1_468_006,
            at: SystemTime::now() - Duration::from_secs(150),
        }),
    ));
    state.cursor = 2;
    state
}
fn bios_page() -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.open(Page::Games);
    state.open(Page::Bios);
    state.bios.systems = Some(bios::parse(BIOS).unwrap());
    state
}

#[test]
fn diagnostics_and_bios_draw_in_every_language() {
    log::set_logger(&WARNINGS).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let temp = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("ARCHR_DIAG_CAPTURE_DIR")
        .map_or_else(|| temp.path().to_path_buf(), Into::into);
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    ui.set_motion(Motion::Off);
    let mut question = bios_page();
    question.bios.confirm = true;
    let mut msx = bios_page();
    msx.bios.systems.as_mut().unwrap()[0] = bios::parse(
        r#"{"systems":[{"id":"msx","name":"MSX","cores":"blueMSX · fMSX","verdict":"free","free":"msx","free_installed":true,
         "files":[{"file":"MSX.ROM","md5":"364a1a579fe5cb8dba54519bcfcdac0d","state":"free"}]}]}"#,
    )
    .unwrap()
    .remove(0);
    let mut running = diagnostics();
    running.diag.running = true;
    for language in ["en", "pt_BR", "es", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in [
            ("diagnostico", diagnostics()),
            ("diagnostico-gerando", running.clone()),
            ("bios", bios_page()),
            ("bios-livre", question.clone()),
            ("bios-msx", msx.clone()),
        ] {
            ui.screenshot = Some(dir.join(format!("{name}-{language}.png")));
            ui.draw_with_settings(
                &App::new(9),
                &FakeData::default(),
                &theme,
                Default::default(),
                Some(&state),
            )
            .unwrap();
        }
    }
    let seen = SEEN.lock().unwrap();
    assert!(seen.is_empty(), "{seen:#?}");
}
