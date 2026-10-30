mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    i18n::I18n,
    render::{Art, Ui},
    saves::Shot,
    state::{App, Screen},
    Theme,
};
use common::FakeData;
use sdl2::{pixels::PixelFormatEnum, surface::Surface};

fn data() -> FakeData {
    let game = Game {
        path: "Chrono Trigger (USA).sfc".into(),
        name: "Chrono Trigger".into(),
        image: None,
        video: None,
        favorite: false,
        last_played: None,
        year: None,
        players: None,
        details: Default::default(),
    };
    let mut data = FakeData {
        systems: vec![SystemEntry {
            name: "snes".into(),
            fullname: "Super Nintendo".into(),
            game_count: 1,
            dir_mtime: 0,
            source_fingerprint: 0,
        }],
        ..Default::default()
    };
    data.games.insert("snes".into(), vec![game]);
    data
}

fn app() -> App {
    let mut app = App::new(8);
    app.screen = Screen::Shot {
        system: "snes".into(),
        game: 0,
    };
    app.progress.shots = vec![Shot {
        path: "a.png".into(),
        taken: 1_700_000_000,
        bytes: 1000,
        size: Some((640, 480)),
        details: false,
    }];
    app
}

/// A bright picture, near white with a faint diagonal.
fn bright() -> Surface<'static> {
    let mut surface = Surface::new(640, 480, PixelFormatEnum::RGBA32).unwrap();
    let pitch = surface.pitch() as usize;
    surface.with_lock_mut(|bytes| {
        for y in 0..480 {
            for x in 0..640 {
                let v = 235 + ((x + y) / 90) as u8;
                bytes[y * pitch + x * 4..][..4].copy_from_slice(&[v, v, v - 8, 255]);
            }
        }
    });
    surface
}

fn themes() -> [(&'static str, Theme); 2] {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    ["default", "praia"].map(|n| (n, Theme::load(&root.join(n))))
}

fn render(output: &std::path::Path, language: &str) -> Vec<(String, Vec<u8>)> {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf: &'static _ = Box::leak(Box::new(sdl2::ttf::init().unwrap()));
    let mut ui = Ui::new(640, 480, true).unwrap();
    let creator: &'static _ = Box::leak(Box::new(ui.canvas.texture_creator()));
    let picture = creator.create_texture_from_surface(bright()).unwrap();
    let (data, app) = (data(), app());
    let mut out = Vec::new();
    for (name, theme) in themes() {
        ui.configure(ttf, creator, &theme, None).unwrap();
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        let art = Art {
            picture: Some(&picture),
            ..Art::default()
        };
        let file = output.join(format!("shot-{name}-{language}.png"));
        ui.screenshot = Some(file.clone());
        ui.draw_with_settings(&app, &data, &theme, art, None)
            .unwrap();
        out.push((name.to_string(), std::fs::read(file).unwrap()));
    }
    out
}

#[test]
fn title_and_date_sit_on_a_plate_over_a_bright_picture() {
    // ARCHR_SHOT_CAPTURE_DIR keeps the pictures to look at.
    let temp = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("ARCHR_SHOT_CAPTURE_DIR")
        .map_or_else(|| temp.path().to_path_buf(), Into::into);
    for language in ["pt_BR", "ru"] {
        for (name, png) in render(&dir, language) {
            let image = image::load_from_memory(&png).unwrap().to_rgba8();
            // Beside the text, left of the counter pill: plate, not picture.
            let [r, g, b, _] = image.get_pixel(400, 6).0;
            assert!(
                u32::from(r) + u32::from(g) + u32::from(b) < 3 * 140,
                "{name} {language}: {r},{g},{b}"
            );
            // Below the plate the picture is untouched.
            let [r, ..] = image.get_pixel(400, 200).0;
            assert!(r > 220, "{name} {language}");
        }
    }
}
