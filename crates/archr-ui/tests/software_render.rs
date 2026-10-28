mod common;
use archr_ui::{render::Ui, state::App, Theme};
use common::FakeData;

#[test]
fn software_renderer_keeps_text_when_texture_limits_are_unspecified() {
    std::env::set_var("SDL_VIDEODRIVER", "dummy");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    ui.draw(&App::new(9), &FakeData::default(), &theme, None)
        .unwrap();
    let pixels = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    let text_pixels = pixels
        .chunks_exact(4)
        .filter(|p| p[..3].iter().all(|v| *v > 200))
        .count();
    assert!(
        text_pixels > 50,
        "header/footer text disappeared with SDL's unspecified texture limit"
    );
}
