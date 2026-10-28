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
            if page == Page::Sound {
                state.cursor = 3;
                state.values.insert(Id::UiSound, "1".into());
            }
            ui.settings = Some(state);
            ui.draw(&App::new(9), &FakeData::default(), &theme, None)
                .unwrap();
            if page == Page::Sound {
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                let pixel = |x: usize, y: usize| &pixels[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3];
                assert_eq!(
                    pixel(575, 268),
                    &[255, 255, 255],
                    "white selected switch track"
                );
                assert_eq!(
                    pixel(591, 268),
                    &[30, 136, 255],
                    "accent selected switch knob"
                );
            }
        }
    }
}
