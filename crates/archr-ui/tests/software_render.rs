use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    render::Ui,
    state::{App, DataSource},
    Theme,
};

struct Empty;
impl DataSource for Empty {
    fn systems(&self) -> &[SystemEntry] {
        &[]
    }
    fn games(&self, _: &str) -> &[Game] {
        &[]
    }
    fn origin(&self, _: &str, _: usize) -> Option<(String, usize)> {
        None
    }
    fn cores(&self, _: &str) -> Vec<(String, String)> {
        vec![]
    }
    fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
        None
    }
}

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
    ui.draw(&App::new(9), &Empty, &theme, None).unwrap();
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
