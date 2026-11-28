//! Converted themes drawn by the launcher, as the free screens' captures
//! are (crates/archr-ui/tests/scene_render.rs): the systems, games and
//! details screens of every view at 640x480, 720x720 and 1024x768.
//!
//! The themes come from outside the repository (their art is theirs):
//! `ARCHR_ES2THEME_RENDER` lists converted theme folders, separated by
//! `:`, and `ARCHR_ES2THEME_SHOTS` is where the PNGs go. Without them the
//! test draws a small theme converted from the XML written here.
//! `ARCHR_ES2THEME_CHOICES=<opção>=<escolha>,...` draws them with those
//! choices of their options; `ARCHR_ES2THEME_VIEW=<id>` (or `default`)
//! draws one view.
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    render::Ui,
    state::{App, Button, DataSource, Screen},
    Theme,
};
use std::path::{Path, PathBuf};

const SIZES: [(u32, u32); 3] = [(640, 480), (720, 720), (1024, 768)];

struct Data {
    systems: Vec<SystemEntry>,
    games: Vec<(String, Vec<Game>)>,
    order: Vec<String>,
}
impl DataSource for Data {
    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }
    fn games(&self, system: &str) -> &[Game] {
        self.games
            .iter()
            .find(|(s, _)| s == system)
            .map_or(&[], |(_, g)| g.as_slice())
    }
    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        self.games(system).get(game).map(|_| (system.into(), game))
    }
    fn system_order(&self) -> &[String] {
        &self.order
    }
    fn cores(&self, _: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
        None
    }
}

/// A game's screenshot at its console's own resolution: a tiled sky,
/// a HUD bar on top, blocks and a floor, a one-pixel frame (a crop or a
/// stretch shows at once).
fn shot(path: &Path, (w, h): (u32, u32), hue: [u8; 3]) {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let mut c = hue.map(|v| (f32::from(v) * (0.55 + 0.45 * (1. - fy))) as u8);
        if (x / 16 + y / 16) % 2 == 0 && fy < 0.7 {
            c = c.map(|v| v.saturating_add(18));
        }
        if fy < 0.09 {
            c = [20, 20, 30];
            if (x / 6) % 3 == 0 && fy > 0.03 && fy < 0.06 && fx < 0.4 {
                c = [240, 240, 240];
            }
        }
        if fy > 0.82 {
            c = if (x / 8) % 2 == 0 {
                [150, 90, 40]
            } else {
                [120, 70, 30]
            };
        }
        let (dx, dy) = (fx - 0.45, fy - 0.62);
        if dx.abs() < 0.04 && dy.abs() < 0.12 {
            c = [230, 60, 50];
        }
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            c = [255, 255, 255];
        }
        image::Rgba([c[0], c[1], c[2], 255])
    });
    img.save(path).unwrap();
}
/// A box cover in its region's shape: a frame, a title band and a hero.
fn cover(path: &Path, (w, h): (u32, u32), hue: [u8; 3]) {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let mut c = hue.map(|v| (f32::from(v) * (0.5 + 0.5 * fy)) as u8);
        if fy < 0.16 {
            c = [235, 235, 235];
            if fy > 0.05 && fy < 0.11 && fx > 0.1 && fx < 0.9 && (x / 7) % 4 != 3 {
                c = [30, 30, 40];
            }
        }
        let (dx, dy) = (fx - 0.5, (fy - 0.58) * h as f32 / w as f32);
        if dx * dx + dy * dy < 0.06 {
            c = [250, 200, 80];
        }
        if x < 3 || y < 3 || x + 3 >= w || y + 3 >= h {
            c = [10, 10, 10];
        }
        image::Rgba([c[0], c[1], c[2], 255])
    });
    img.save(path).unwrap();
}
/// A game's wheel logo: letters in a capsule on transparency.
fn logo(path: &Path, (w, h): (u32, u32), hue: [u8; 3]) {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let inside = (fy - 0.5).abs() < 0.42 && (fx - 0.5).abs() < 0.48;
        if !inside {
            return image::Rgba([0, 0, 0, 0]);
        }
        let letter = (fy - 0.5).abs() < 0.22 && (x / 18) % 3 != 2 && fx > 0.1 && fx < 0.9;
        let c = if letter { [255, 255, 255] } else { hue };
        image::Rgba([c[0], c[1], c[2], 255])
    });
    img.save(path).unwrap();
}

fn library(pictures: &Path) -> Data {
    // Id, name, cover and screenshot sizes, games (the long titles too).
    type System<'a> = (&'a str, &'a str, (u32, u32), (u32, u32), &'a [&'a str]);
    let lists: [System; 12] = [
        (
            "snes",
            "Super Nintendo",
            (420, 300),
            (256, 224),
            &[
                "Chrono Trigger",
                "F-Zero",
                "Super Metroid",
                "The Legend of Zelda: A Link to the Past",
                "Yoshi's Island",
            ],
        ),
        (
            "megadrive",
            "Mega Drive",
            (300, 420),
            (320, 224),
            &[
                "Aladdin",
                "Castlevania: Bloodlines",
                "Comix Zone",
                "Ecco the Dolphin",
                "Gunstar Heroes",
                "Phantasy Star IV: The End of the Millennium",
                "Sonic The Hedgehog",
                "Shinobi III: Return of the Ninja Master",
                "Streets of Rage 2",
                "Sonic The Hedgehog 2",
                "Vectorman",
                "Golden Axe",
                "Altered Beast",
                "Earthworm Jim",
                "Ristar",
                "ToeJam & Earl",
                "Thunder Force IV",
                "Rocket Knight Adventures",
                "Contra: Hard Corps",
                "Dynamite Headdy",
                "Landstalker",
                "Kid Chameleon",
                "Mega Turrican",
                "Strider",
            ],
        ),
        (
            "gba",
            "Game Boy Advance",
            (320, 320),
            (240, 160),
            &["Metroid Fusion", "Advance Wars", "Golden Sun"],
        ),
        (
            "n64",
            "Nintendo 64",
            (420, 300),
            (320, 240),
            &["Super Mario 64", "Star Fox 64"],
        ),
        (
            "psx",
            "PlayStation",
            (400, 400),
            (320, 240),
            &["Crash Bandicoot"],
        ),
        ("nes", "Nintendo", (300, 420), (256, 240), &["Mega Man 2"]),
        ("gb", "Game Boy", (300, 300), (160, 144), &["Tetris"]),
        (
            "gbc",
            "Game Boy Color",
            (300, 300),
            (160, 144),
            &["Wario Land 3"],
        ),
        (
            "mastersystem",
            "Master System",
            (300, 420),
            (256, 192),
            &["Alex Kidd"],
        ),
        (
            "gamegear",
            "Game Gear",
            (300, 420),
            (160, 144),
            &["Shinobi"],
        ),
        (
            "pcengine",
            "PC Engine",
            (300, 300),
            (256, 224),
            &["Bonk's Adventure"],
        ),
        ("segacd", "Mega-CD", (400, 400), (320, 224), &["Sonic CD"]),
    ];
    let mut data = Data {
        systems: Vec::new(),
        games: Vec::new(),
        order: Vec::new(),
    };
    for (n, (name, full, box_size, screen, games)) in lists.iter().enumerate() {
        data.systems.push(SystemEntry {
            name: (*name).into(),
            fullname: (*full).into(),
            game_count: games.len() as u32,
            dir_mtime: 0,
            source_fingerprint: 0,
        });
        let list = games
            .iter()
            .enumerate()
            .map(|(k, g)| {
                let shade = |base: usize, step: usize, i: usize| ((base + step * i) % 256) as u8;
                let hue = [shade(60, 47, k), shade(90, 31, n), shade(160, 23, k + n)];
                let screenshot = pictures.join(format!("{name}-{k}.png"));
                shot(&screenshot, *screen, hue);
                let boxart = pictures.join(format!("{name}-{k}-capa.png"));
                cover(&boxart, *box_size, [hue[2], hue[0], hue[1]]);
                // Two games in three have a wheel logo.
                let wheel = (k % 3 != 2).then(|| {
                    let p = pictures.join(format!("{name}-{k}-logo.png"));
                    logo(&p, (400, 150), [hue[1], 40, hue[0]]);
                    p
                });
                Game {
                    path: format!("{g}.rom").into(),
                    name: (*g).into(),
                    image: Some(boxart),
                    video: None,
                    favorite: k == 1,
                    last_played: Some(1_794_916_800 - 2 * 86_400),
                    year: Some(1989 + (k % 8) as u16),
                    players: Some(if k % 2 == 0 { "1-2" } else { "1" }.into()),
                    details: archr_index::Details {
                        description: format!(
                            "{g} é um clássico de {full}: fases rápidas, chefes enormes e um segredo em cada canto. A história continua de onde o primeiro parou, com músicas marcantes e um modo para dois jogadores."
                        ),
                        genre: "Plataforma".into(),
                        developer: "Sega Technical Institute".into(),
                        publisher: "Sega".into(),
                        rating: Some(70 + 5 * (k % 5) as u8),
                        screenshot: Some(screenshot),
                        logo: wheel,
                    },
                }
            })
            .collect();
        data.games.push(((*name).into(), list));
    }
    data
}

/// Every view of the theme in `dir`, its three screens at every size,
/// saved in `shots` (when given) as `<prefix>-<view>-<screen>-<w>x<h>.png`.
fn draw(dir: &Path, prefix: &str, shots: Option<&Path>) {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let tmp = tempfile::tempdir().unwrap();
    let pictures = tmp.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let data = library(&pictures);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let default = Theme::load(&manifest.join("../../themes/default"));
    let mut loaded = Theme::load_over(dir, &default);
    // `ARCHR_ES2THEME_CHOICES=opção=escolha,...`: the theme's options chosen.
    if let Ok(choices) = std::env::var("ARCHR_ES2THEME_CHOICES") {
        let chosen: Vec<(String, String)> = choices
            .split(',')
            .filter_map(|c| c.split_once('='))
            .map(|(o, c)| (o.trim().to_owned(), c.trim().to_owned()))
            .collect();
        loaded = loaded.with_options(&chosen);
    }
    let own: Vec<String> = default.views().into_iter().map(|(id, _)| id).collect();
    // `ARCHR_ES2THEME_VIEW=<id>` draws that view alone; `default`, the one
    // the theme opens with.
    let only = std::env::var("ARCHR_ES2THEME_VIEW").ok().map(|v| {
        if v == "default" {
            loaded.default_view.clone()
        } else {
            v
        }
    });
    for (view_id, _) in loaded.views() {
        if own.contains(&view_id) || only.as_ref().is_some_and(|v| *v != view_id) {
            continue;
        }
        let mut theme = loaded.clone();
        theme.active_view.clone_from(&view_id);
        for size in SIZES {
            let theme = theme.fitted(size.0, size.1);
            let view = theme.view(&theme.active_view);
            let root = tempfile::tempdir().unwrap();
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.theme_dir = Some(dir.to_path_buf());
            ui.default_theme_dir = Some(manifest.join("../../themes/default"));
            ui.set_motion(archr_ui::motion::Motion::Off);
            ui.configure(&ttf, &creator, &theme, Some(root.path()))
                .unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            ui.catalog =
                archr_ui::i18n::I18n::load(&manifest.join("../../locales"), Some("pt_BR"), None);
            ui.pin_clock(std::time::Instant::now(), 1_794_916_800);
            let mut covers =
                archr_ui::covers::CoverCache::new(&creator, root.path().join("covers"), 320, 240);
            // The covers of the system drawn (all of them would not fit the
            // cache and never settle).
            let all: Vec<PathBuf> = data
                .games
                .iter()
                .filter(|(s, _)| s == "megadrive")
                .flat_map(|(_, g)| g.iter().filter_map(|g| g.image.clone()))
                .take(8)
                .collect();
            let mut app = App::new(10);
            app.set_view(&view);
            let mut frame = |ui: &mut Ui, app: &mut App, name: &str| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
                let mut round = 0;
                loop {
                    for chunk in all.chunks(5) {
                        covers.request(chunk);
                    }
                    covers.poll();
                    ui.tick();
                    ui.snap_animations();
                    let art = archr_ui::render::Art {
                        covers: Some(&covers),
                        ..Default::default()
                    };
                    ui.draw_with_settings(app, &data, &theme, art, None)
                        .unwrap();
                    app.details_overflow = ui.details_overflow;
                    if covers.idle() && ui.pictures_idle() && round >= 3 {
                        break;
                    }
                    // A theme whose screen holds more pictures than the
                    // launcher's budget keeps loading them: said, not fatal
                    // for a theme from outside (the small one must settle).
                    if std::time::Instant::now() >= deadline {
                        assert!(prefix != "pequeno", "pictures never loaded");
                        eprintln!("pictures never settled: {prefix} {view_id} {name} {size:?}");
                        break;
                    }
                    round += 1;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                // Something is drawn: not one flat color.
                let first = &pixels[..4];
                assert!(
                    pixels.chunks(4).any(|p| p != first),
                    "{prefix} {view_id} {name} is blank"
                );
                if let Some(shots) = shots {
                    let file = shots.join(format!(
                        "{prefix}-{view_id}-{name}-{}x{}.png",
                        size.0, size.1
                    ));
                    image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                }
            };
            // The Mega Drive: the second system (set, not stepped to: in a
            // grid of systems ↓ goes to the next row).
            app.cursor = 1;
            frame(&mut ui, &mut app, "sistemas");
            app.handle(Button::A, &data);
            assert!(matches!(app.screen, Screen::Games { .. }));
            app.cursor = 6;
            frame(&mut ui, &mut app, "jogos");
            app.screen = Screen::Details {
                system: "megadrive".into(),
                game: 6,
            };
            frame(&mut ui, &mut app, "detalhes");
        }
    }
}

#[test]
fn converted_themes_draw_at_every_size() {
    let shots = std::env::var_os("ARCHR_ES2THEME_SHOTS").map(PathBuf::from);
    if let Some(shots) = &shots {
        std::fs::create_dir_all(shots).unwrap();
    }
    match std::env::var_os("ARCHR_ES2THEME_RENDER") {
        Some(dirs) => {
            for dir in std::env::split_paths(&dirs) {
                let prefix = dir.file_name().unwrap().to_string_lossy().into_owned();
                draw(&dir, &prefix, shots.as_deref());
            }
        }
        None => {
            let tmp = tempfile::tempdir().unwrap();
            let src = tmp.path().join("pequeno");
            let out = tmp.path().join("convertido");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(src.join("theme.xml"), SMALL).unwrap();
            let c = archr_es2theme::convert(&src, &out, &Default::default()).unwrap();
            assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
            draw(&out, "pequeno", shots.as_deref());
        }
    }
}

const SMALL: &str = r#"<theme>
  <formatVersion>7</formatVersion>
  <view name="system">
    <carousel name="systemcarousel"><type>vertical</type><pos>0 0</pos><size>0.4 1</size><maxLogoCount>3</maxLogoCount></carousel>
    <text name="systemInfo"><pos>0.5 0.9</pos><color>FFFFFF</color><fontSize>0.04</fontSize></text>
    <text name="logoText"><color>FFCC00</color><fontSize>0.06</fontSize></text>
  </view>
  <view name="detailed">
    <textlist name="gamelist"><pos>0.55 0.15</pos><size>0.42 0.7</size><selectorColor>3060C0</selectorColor><primaryColor>FFFFFF</primaryColor><fontSize>0.04</fontSize></textlist>
    <image name="md_image"><pos>0.25 0.4</pos><origin>0.5 0.5</origin><maxSize>0.45 0.45</maxSize></image>
    <text name="md_description"><pos>0.03 0.7</pos><size>0.45 0.2</size><color>DDDDDD</color><fontSize>0.03</fontSize></text>
    <rating name="md_rating"><pos>0.03 0.65</pos><size>0 0.04</size><color>FFCC00</color></rating>
    <helpsystem name="help"><pos>0.02 0.94</pos><textColor>FFFFFF</textColor></helpsystem>
  </view>
</theme>"#;
