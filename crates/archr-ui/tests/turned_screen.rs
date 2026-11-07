mod common;
use archr_ui::{render::Ui, state::App, Theme};
use common::FakeData;

#[test]
fn a_portrait_panel_is_drawn_landscape_and_turned() {
    std::env::set_var("SDL_VIDEODRIVER", "dummy");
    std::env::set_var("ARCHR_ROTATE", "270");
    let ttf;
    let creator;
    let mut ui = Ui::new(320, 480, true).unwrap();
    std::env::remove_var("ARCHR_ROTATE");
    assert_eq!(ui.size(), (480, 320));
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin().fitted(480, 320);
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    ui.draw(&App::new(9), &FakeData::default(), &theme, None)
        .unwrap();
    // The frame was presented turned; drawing goes on in landscape.
    let pixels = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    assert_eq!(pixels.len(), 480 * 320 * 4);
}
