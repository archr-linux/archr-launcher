use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    render::Ui,
    state::{App, DataSource, Screen},
    Theme,
};
struct Data {
    systems: Vec<SystemEntry>,
    games: Vec<Game>,
}
impl DataSource for Data {
    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }
    fn games(&self, _: &str) -> &[Game] {
        &self.games
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
fn untrusted_nul_in_system_and_game_text_renders_without_panicking() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let data = Data {
        systems: vec![SystemEntry {
            name: "nes".into(),
            fullname: "Nin\0tendo".into(),
            game_count: 1,
            dir_mtime: 0,
            source_fingerprint: 0,
        }],
        games: vec![Game {
            path: "A.nes".into(),
            name: "Ga\0me".into(),
            image: None,
            video: None,
            favorite: false,
            last_played: None,
        }],
    };
    let mut app = App::new(10);
    ui.draw(&app, &data, &theme, None).unwrap();
    app.screen = Screen::Games {
        system: "nes".into(),
    };
    ui.banner = Some("Failed\0message".into());
    ui.draw(&app, &data, &theme, None).unwrap();
}
