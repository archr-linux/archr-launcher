mod common;
use archr_ui::{
    i18n::I18n,
    render::Ui,
    settings::{Id, Page, Settings},
    state::App,
    Theme,
};
use common::FakeData;
#[test]
fn all_locales_and_pages_render_and_selected_on_toggle_has_contrast() {
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
            Page::Soon("settings.network"),
        ] {
            let mut state = Settings::default();
            if page != Page::Root {
                state.open(page);
            }
            state.loading = false;
            if page == Page::Info {
                state.info.device = "R36S".into();
                state.info.kernel = "6.6.63-archr".into();
                state.info.version = "2.1.1".into();
                state.info.uptime = "2:03".into();
                state.info.cards = [
                    "settings.processor",
                    "settings.graphics",
                    "settings.memory",
                    "settings.storage",
                    "settings.network",
                    "settings.battery",
                ]
                .into_iter()
                .map(|title| archr_ui::settings::InfoCard {
                    title,
                    lines: vec!["640 × 480".into(), "settings.after_reboot|panfrost".into()],
                    percent: Some(70),
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
            }
            if page == Page::System {
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                let pixel = |x: usize, y: usize| &pixels[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3];
                assert_eq!(
                    pixel(570, 289),
                    &[255, 255, 255],
                    "white selected switch track"
                );
                assert_eq!(
                    pixel(590, 289),
                    &[30, 136, 255],
                    "accent selected switch knob"
                );
            }
        }
    }
}
