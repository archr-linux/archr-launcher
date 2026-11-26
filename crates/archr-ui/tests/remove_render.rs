//! The game menu's Delete and its question, for a game and for a port, in
//! Grafite and Ellegance at 640x480 and 1024x768, in Portuguese: every
//! frame draws, and the question changes the screen under it. With
//! ARCHR_DELETE_CAPTURE_DIR set the frames are saved there as PNGs.
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    i18n::I18n,
    layout::LISTA,
    render::Ui,
    state::{App, Button, DataSource, QuickItem, Removal, Screen},
    Theme,
};
use common::FakeData;
use std::path::{Path, PathBuf};

fn painted(root: &Path, name: &str, tint: [u8; 3]) -> PathBuf {
    let path = root.join(format!("{name}.png"));
    let mut img = image::RgbaImage::new(320, 240);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / 320., y as f32 / 240.);
        let c = tint.map(|t| (f32::from(t) * (0.45 + 0.55 * fx) + 60. * fy).min(255.) as u8);
        *px = image::Rgba([c[0], c[1], c[2], 255]);
    }
    img.save(&path).unwrap();
    path
}

fn game(path: &str, name: &str, image: Option<PathBuf>) -> Game {
    Game {
        path: path.into(),
        name: name.into(),
        image,
        video: None,
        favorite: false,
        last_played: None,
        year: Some(1995),
        players: None,
        details: Default::default(),
    }
}

fn library(pictures: &[PathBuf]) -> FakeData {
    let mut data = FakeData::default();
    let snes = [
        "Chrono Trigger",
        "Donkey Kong Country",
        "F-Zero",
        "Mega Man X",
        "Super Metroid",
    ];
    let ports = ["100 Lil Jumps", "A7Xpg", "Billy Frontier", "OldBits Hub"];
    for (system, fullname, names, ext) in [
        ("snes", "Super Nintendo", &snes[..], "sfc"),
        ("ports", "Ports", &ports[..], "sh"),
    ] {
        data.systems.push(SystemEntry {
            name: system.into(),
            fullname: fullname.into(),
            game_count: names.len() as u32,
            dir_mtime: 0,
            source_fingerprint: 0,
        });
        let games = names
            .iter()
            .enumerate()
            .map(|(n, name)| {
                let path = format!("/roms/{system}/{name}.{ext}");
                game(&path, name, Some(pictures[n % pictures.len()].clone()))
            })
            .collect();
        data.games.insert(system.into(), games);
    }
    data
}

fn removal(data: &FakeData, system: &str, game: usize, shown: &[&str], media: usize) -> Removal {
    let found = &data.games[system][game];
    Removal {
        system: system.into(),
        rom: found.path.clone(),
        name: found.name.clone(),
        port: system == "ports",
        shown: shown.iter().map(|s| (*s).to_owned()).collect(),
        media,
        paths: vec![found.path.clone()],
        on_action: false,
    }
}

fn theme(dir: &str) -> Theme {
    let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let default = Theme::load(&themes.join("default"));
    let mut theme = Theme::load_over(&themes.join(dir), &default);
    theme.active_view = LISTA.into();
    theme
}

#[test]
fn the_delete_row_and_its_question_draw_in_both_themes() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_DELETE_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let root = tempfile::tempdir().unwrap();
    let pictures: Vec<PathBuf> = [[70, 40, 160], [180, 60, 40], [40, 140, 90]]
        .iter()
        .enumerate()
        .map(|(n, tint)| painted(root.path(), &format!("p{n}"), *tint))
        .collect();
    let data = library(&pictures);
    let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales");
    for (label, dir) in [("grafite", "default"), ("ellegance", "ellegance")] {
        for size in [(640, 480), (1024, 768)] {
            let theme = theme(dir).fitted(size.0, size.1);
            let view = theme.view(&theme.active_view);
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.theme_dir = Some(root.path().to_path_buf());
            ui.default_theme_dir =
                Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"));
            ui.set_motion(archr_ui::motion::Motion::Off);
            ui.configure(&ttf, &creator, &theme, Some(root.path()))
                .unwrap();
            ui.catalog = I18n::load(&locales, Some("pt_BR"), None);
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            let mut covers =
                archr_ui::covers::CoverCache::new(&creator, root.path().join("covers"), 320, 240);
            let mut shot = |ui: &mut Ui, name: &str, app: &App| -> Vec<u8> {
                let cover = data
                    .games(match app.backdrop().0 {
                        Screen::Games { system, .. } => system,
                        _ => "snes",
                    })
                    .get(app.backdrop().1)
                    .and_then(|g| g.image.clone());
                for _ in 0..400 {
                    covers.request(&pictures);
                    covers.poll();
                    if covers.idle() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                for _ in 0..3 {
                    ui.tick();
                    ui.snap_animations();
                    let art = archr_ui::render::Art {
                        cover: cover.as_ref().and_then(|p| covers.peek(p)),
                        covers: Some(&covers),
                        ..Default::default()
                    };
                    ui.draw_with_settings(app, &data, &theme, art, None)
                        .unwrap();
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                let first = &pixels[..4];
                assert!(
                    pixels.chunks_exact(4).filter(|p| p != &first).count() > 1000,
                    "{label} {name} at {size:?} empty"
                );
                if let Some(out) = out.as_ref().filter(|_| !name.starts_with('_')) {
                    let file = out.join(format!("{label}-{name}-{}x{}.png", size.0, size.1));
                    image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                }
                pixels
            };
            for (system, at, shown, media, name) in [
                ("snes", 0, &["snes/Chrono Trigger.sfc"][..], 3, "jogo"),
                (
                    "ports",
                    3,
                    &["ports/OldBits Hub.sh", "ports/oldbits-hub/"][..],
                    0,
                    "port",
                ),
                ("ports", 1, &["ports/A7Xpg.sh"][..], 0, "port-sem-pasta"),
            ] {
                let mut app = App::new(10);
                app.set_view(&view);
                app.screen = Screen::games(system);
                app.cursor = at;
                app.handle(Button::Y, &data);
                let items = app.quick_menu_items(&data);
                assert_eq!(items.last(), Some(&QuickItem::Delete(system == "ports")));
                app.cursor = items.len() - 1;
                if name != "port-sem-pasta" {
                    shot(&mut ui, &format!("menu-{name}"), &app);
                }
                app.handle(Button::B, &data);
                let under = shot(&mut ui, &format!("_lista-{name}"), &app);
                app.removal = Some(removal(&data, system, at, shown, media));
                let asked = shot(&mut ui, &format!("apagar-{name}"), &app);
                assert_ne!(under, asked, "{label} {name}: the question shows");
            }
        }
    }
}
