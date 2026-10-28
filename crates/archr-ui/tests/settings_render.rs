mod common;
use archr_ui::{
    i18n::I18n,
    render::Ui,
    settings::{Id, Page, Settings},
    state::App,
    Theme,
};
use common::FakeData;
struct Warnings(std::sync::Mutex<Vec<String>>);
static WARNINGS: Warnings = Warnings(std::sync::Mutex::new(Vec::new()));
impl log::Log for Warnings {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
    }
    fn flush(&self) {}
}
#[test]
fn all_locales_and_pages_render_and_selected_on_toggle_has_contrast() {
    log::set_logger(&WARNINGS).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    for language in ["en", "pt_BR", "es", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for page in [
            Page::Root,
            Page::Sound,
            Page::System,
            Page::Region,
            Page::Info,
            Page::InfoDetail(0),
            Page::InfoDetail(1),
            Page::InfoDetail(2),
            Page::InfoDetail(3),
            Page::InfoDetail(4),
            Page::InfoDetail(5),
            Page::Soon("settings.network"),
        ] {
            let mut state = Settings::default();
            if page != Page::Root {
                state.open(page);
            }
            state.loading = false;
            if matches!(page, Page::Info | Page::InfoDetail(_)) {
                state.info.device = "R36S".into();
                state.info.kernel = "6.6.63-archr".into();
                state.info.version = "2.1.1".into();
                state.info.cards = [
                    "settings.processor",
                    "settings.graphics",
                    "settings.memory",
                    "settings.storage",
                    "settings.network",
                    "settings.battery",
                ]
                .into_iter()
                .map(|_| archr_ui::settings::InfoCard {
                    lines: vec!["640 × 480".into(), "panfrost".into()],
                    percent: Some(70),
                    detail: archr_ui::settings::InfoDetail {
                        rows: vec![
                            ("settings.design_capacity", "@3200 mAh".into()),
                            ("settings.min_max", "@408 · @1512 MHz".into()),
                        ],
                        ..Default::default()
                    },
                })
                .collect();
            }
            if page == Page::Sound {
                state.values.insert(Id::Idle, "5".into());
            }

            if page == Page::System {
                state.cursor = 3;
                state.values.insert(Id::Turbo, "1".into());
            }
            ui.draw_with_settings(
                &App::new(9),
                &FakeData::default(),
                &theme,
                None,
                Some(&state),
            )
            .unwrap();
            if page == Page::Root {
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                let rgb = |x: usize, y: usize| &pixels[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3];
                assert_eq!(
                    rgb(24, 110),
                    &[11, 15, 20],
                    "unselected rows have no background or border"
                );
                for y in 86..89 {
                    assert_eq!(rgb(100, y), &[11, 15, 20], "three-pixel inter-row gap");
                }
            }
            if matches!(page, Page::Root | Page::Sound) {
                state.error = Some((None, "diagnostic без потери отклика ".repeat(2048)));
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                let before = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                state.error = Some((None, "different private technical details".into()));
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                let after = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                assert!(
                    before == after,
                    "technical details must never enter the banner"
                );
            }
            if page == Page::Sound {
                state.error = None;
                let name = "W".repeat(264);
                assert_eq!(name.chars().count(), 264);
                state.values.insert(Id::Audio, name);
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                let unavailable = ui
                    .canvas
                    .read_pixels(
                        Some(sdl2::rect::Rect::new(352, 136, 228, 40)),
                        sdl2::pixels::PixelFormatEnum::RGBA32,
                    )
                    .unwrap();
                state.values.insert(Id::Audio, String::new());
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                assert!(unavailable == ui.canvas.read_pixels(Some(sdl2::rect::Rect::new(352,136,228,40)),sdl2::pixels::PixelFormatEnum::RGBA32).unwrap(),"unrenderable external data must show localized unavailable, never terminate or truncate");
                state.values.insert(Id::Audio, "W".repeat(60));
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                assert!(
                    unavailable
                        != ui
                            .canvas
                            .read_pixels(
                                Some(sdl2::rect::Rect::new(352, 136, 228, 40)),
                                sdl2::pixels::PixelFormatEnum::RGBA32
                            )
                            .unwrap(),
                    "reallocated row width must allow complete multi-line values"
                );
            }
            if page == Page::Info {
                for card in &mut state.info.cards {
                    card.lines = vec!["x".into()];
                    card.percent = None;
                }
                state.info.cards[0].lines = ["a", "b", "c", "d", "e"].map(str::to_owned).to_vec();
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                let complete = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                state.info.cards[0].lines.clear();
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
                assert!(
                    complete
                        != ui
                            .canvas
                            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                            .unwrap(),
                    "use all available card space before falling back"
                );
                state.info.device = "W".repeat(264);
                state.info.cards[0].lines = vec!["W\n".repeat(264)];
                state.info.cards[4].lines = vec!["W".repeat(264)];
                ui.draw_with_settings(
                    &App::new(9),
                    &FakeData::default(),
                    &theme,
                    None,
                    Some(&state),
                )
                .unwrap();
            }
            if page == Page::System {
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                let pixel = |x: usize, y: usize| &pixels[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3];
                assert_eq!(
                    pixel(580, 199),
                    &[255, 255, 255],
                    "white selected switch track"
                );
                assert_eq!(
                    pixel(600, 199),
                    &[30, 136, 255],
                    "accent selected switch knob"
                );
            }
        }
    }
    assert!(
        WARNINGS
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains(&"W".repeat(264))),
        "full original data must be logged"
    );
}
