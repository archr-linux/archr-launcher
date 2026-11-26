//! Free screens (docs/temas.md, Telas livres) with the test theme
//! (tests/themes/cena): its two views, one in the shape of the approved
//! Solo mockup and one of the Albedo, read and drawn at 640x480, 720x720
//! and 1024x768. The pictures are drawn here, so nothing in the repository
//! is anyone else's art. With ARCHR_SCENE_CAPTURE_DIR set, every frame is
//! saved there as a PNG. Bad values in a theme are warnings, never a panic,
//! and every example of the docs chapter reads without a warning.
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    layout::{Kind, Mark, View},
    render::Ui,
    state::{App, Button, Screen},
    Theme,
};
use common::FakeData;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Once,
};

const SIZES: [(u32, u32); 3] = [(640, 480), (720, 720), (1024, 768)];
/// SDL is set up by one test at a time.
static SDL: std::sync::Mutex<()> = std::sync::Mutex::new(());

thread_local! { static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
struct Capture;
impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            WARNINGS.with(|w| w.borrow_mut().push(record.args().to_string()));
        }
    }
    fn flush(&self) {}
}
/// The warnings `run` logs on this thread.
fn warnings(run: impl FnOnce()) -> Vec<String> {
    static LOGGER: Capture = Capture;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    WARNINGS.with(|w| w.borrow_mut().clear());
    run();
    WARNINGS.with(|w| w.borrow_mut().drain(..).collect())
}

fn shipped(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../themes")
        .join(name)
}

/// The test theme in `dir`, with its pictures and fonts.
fn theme_folder(dir: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/themes/cena");
    std::fs::create_dir_all(dir.join("parts")).unwrap();
    for file in ["theme.toml", "parts/roda.toml"] {
        std::fs::copy(source.join(file), dir.join(file)).unwrap();
    }
    for sub in [
        "arts",
        "arts2",
        "logos",
        "systems",
        "icons",
        "fonts",
        "frames/classic",
        "frames/square",
        "frames/wide",
    ] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    let fonts = shipped("ellegance").join("fonts");
    std::fs::copy(fonts.join("Inter-Bold.ttf"), dir.join("fonts/Titulo.ttf")).unwrap();
    std::fs::copy(fonts.join("Inter-Regular.ttf"), dir.join("fonts/Corpo.ttf")).unwrap();
    for (system, hue) in [
        ("snes", [96, 72, 200]),
        ("megadrive", [30, 60, 190]),
        ("gba", [70, 30, 150]),
        ("n64", [40, 150, 90]),
    ] {
        art(&dir.join(format!("arts/{system}.png")), (960, 720), hue);
        // The "Dark" background option's art.
        art(
            &dir.join(format!("arts2/{system}.png")),
            (960, 720),
            hue.map(|c| c / 4),
        );
        console(&dir.join(format!("systems/{system}.png")), hue);
    }
    art(&dir.join("arts/default.png"), (960, 720), [80, 80, 90]);
    art(&dir.join("arts2/default.png"), (960, 720), [20, 20, 24]);
    // Logos for three systems, the Mega Drive's under the name it shares
    // and drawn: the Nintendo 64 falls back to its name or picture.
    logo(&dir.join("logos/snes.png"), [230, 60, 60]);
    logo(&dir.join("logos/gba.png"), [90, 110, 240]);
    std::fs::write(dir.join("logos/genesis.svg"), GENESIS).unwrap();
    for (shape, size) in [
        ("classic", (640, 480)),
        ("square", (640, 640)),
        ("wide", (854, 480)),
    ] {
        cut(
            &dir.join(format!("frames/{shape}/carousel-bg.png")),
            size,
            0.34,
        );
        cut(
            &dir.join(format!("frames/{shape}/gamelist-bg.png")),
            size,
            0.44,
        );
    }
    border(&dir.join("frames/borda.png"));
    star(&dir.join("icons/estrela.png"));
    calendar(&dir.join("icons/calendario.png"));
    console(&dir.join("icons/cartucho.png"), [120, 120, 130]);
}

/// The Mega Drive's logo, as a theme's SVG.
const GENESIS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 100"><rect x="4" y="14" width="292" height="72" rx="36" fill="#2E9BE6"/><circle cx="60" cy="50" r="24" fill="#FFFFFF"/></svg>"##;

fn save(img: image::RgbaImage, path: &Path) {
    img.save(path).unwrap();
}
/// A full-screen picture: two shades split by a diagonal, a sun and stripes.
fn art(path: &Path, (w, h): (u32, u32), hue: [u8; 3]) {
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let light = fx + 0.35 * fy < 0.62;
        let k = if light { 1.0 } else { 0.55 };
        let mut c = hue.map(|v| (f32::from(v) * k + 40. * (1. - fy)).min(255.) as u8);
        let (dx, dy) = (fx - 0.28, fy - 0.36);
        if dx * dx + dy * dy < 0.018 {
            c = [252, 214, 120];
        }
        if light && ((fx * 40.) as u32).is_multiple_of(7) && fy > 0.7 {
            c = c.map(|v| v.saturating_add(30));
        }
        *px = image::Rgba([c[0], c[1], c[2], 255]);
    }
    save(img, path);
}
/// A console-ish block: a rounded body, a screen and two buttons.
fn console(path: &Path, hue: [u8; 3]) {
    let (w, h) = (480, 480);
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let body = (0.12..0.88).contains(&fx) && (0.2..0.8).contains(&fy);
        let screen = (0.22..0.62).contains(&fx) && (0.3..0.62).contains(&fy);
        let (bx, by) = (fx - 0.74, fy - 0.46);
        let button = bx * bx + by * by < 0.0025;
        *px = if screen {
            image::Rgba([20, 24, 30, 255])
        } else if button {
            image::Rgba([240, 200, 60, 255])
        } else if body {
            let [r, g, b] = hue.map(|v| v.saturating_add(60));
            image::Rgba([r, g, b, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    save(img, path);
}
/// A wordless logo: a white pill with a colored disc.
fn logo(path: &Path, hue: [u8; 3]) {
    let (w, h) = (400, 140);
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let r = 0.5;
        let cx = fx.clamp(r * h as f32 / w as f32, 1. - r * h as f32 / w as f32);
        let (dx, dy) = ((fx - cx) * w as f32 / h as f32, fy - 0.5);
        let inside = dx * dx + dy * dy < r * r;
        let (ex, ey) = ((fx - 0.25) * w as f32 / h as f32, fy - 0.5);
        *px = if inside && ex * ex + ey * ey < 0.09 {
            image::Rgba([hue[0], hue[1], hue[2], 255])
        } else if inside {
            image::Rgba([250, 250, 250, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    save(img, path);
}
/// A frame over the screen: a dark band on the left (`band` of the width)
/// cut on a slant, a header bar and a hole where the content shows.
fn cut(path: &Path, (w, h): (u32, u32), band: f32) {
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let left = fx < band + 0.04 * (1. - fy);
        let header = fy < 0.11;
        let stripe = header && ((x + y) / 6).is_multiple_of(4);
        *px = if stripe {
            image::Rgba([40, 40, 44, 255])
        } else if header {
            image::Rgba([24, 24, 26, 245])
        } else if left {
            image::Rgba([18, 18, 20, 225])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    save(img, path);
}
/// A 24×24 nine-slice border: 3 px light edge, clear middle.
fn border(path: &Path) {
    let mut img = image::RgbaImage::new(24, 24);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let edge = x < 3 || y < 3 || x > 20 || y > 20;
        *px = if edge {
            image::Rgba([249, 196, 64, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    save(img, path);
}
/// A white five-pointed star, tinted by the theme.
fn star(path: &Path) {
    let side = 64;
    let points: Vec<(f32, f32)> = (0..10)
        .map(|n| {
            let angle = std::f32::consts::PI * (n as f32 / 5. - 0.5);
            let r = if n % 2 == 0 { 0.5 } else { 0.2 };
            (0.5 + r * angle.cos(), 0.52 + r * angle.sin())
        })
        .collect();
    let mut img = image::RgbaImage::new(side, side);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (fx, fy) = (
            (x as f32 + 0.5) / side as f32,
            (y as f32 + 0.5) / side as f32,
        );
        // Even-odd crossings of a ray to the right.
        let mut inside = false;
        for k in 0..points.len() {
            let ((x0, y0), (x1, y1)) = (points[k], points[(k + 1) % points.len()]);
            if (y0 > fy) != (y1 > fy) && fx < x0 + (fy - y0) / (y1 - y0) * (x1 - x0) {
                inside = !inside;
            }
        }
        *px = image::Rgba([255, 255, 255, if inside { 255 } else { 0 }]);
    }
    save(img, path);
}
/// A white calendar glyph, tinted by the theme.
fn calendar(path: &Path) {
    let mut img = image::RgbaImage::new(32, 32);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let frame = (3..29).contains(&x) && (5..29).contains(&y);
        let hollow = (6..26).contains(&x) && (12..26).contains(&y);
        *px = if frame && !hollow {
            image::Rgba([255, 255, 255, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    save(img, path);
}

fn game(name: &str) -> Game {
    Game {
        path: format!("{name}.rom").into(),
        name: name.into(),
        image: None,
        video: None,
        favorite: false,
        year: Some(1992),
        players: Some("1-2".into()),
        last_played: None,
        details: Default::default(),
    }
}
/// Four systems; the Mega Drive's games with pictures, descriptions and a
/// logo for one of them.
fn library(pictures: &Path) -> FakeData {
    let mut data = FakeData::default();
    let lists: [(&str, &str, &[&str]); 4] = [
        (
            "snes",
            "Super Nintendo",
            &[
                "Chrono Trigger",
                "F-Zero",
                "Super Metroid",
                "Yoshi's Island",
            ],
        ),
        (
            "megadrive",
            "Mega Drive",
            &[
                "Aladdin",
                "Castlevania: Bloodlines",
                "Comix Zone",
                "Ecco the Dolphin",
                "Gunstar Heroes",
                "Phantasy Star IV",
                "Shinobi III",
                "Sonic The Hedgehog 2",
                "Streets of Rage 2",
            ],
        ),
        (
            "gba",
            "Game Boy Advance",
            &["Metroid Fusion", "Advance Wars"],
        ),
        ("n64", "Nintendo 64", &["Super Mario 64"]),
    ];
    for (n, (name, fullname, games)) in lists.iter().enumerate() {
        data.systems.push(SystemEntry {
            name: (*name).into(),
            fullname: (*fullname).into(),
            game_count: games.len() as u32,
            dir_mtime: 0,
            source_fingerprint: 0,
        });
        let list = games
            .iter()
            .enumerate()
            .map(|(k, g)| {
                let mut game = game(g);
                let shot = pictures.join(format!("{name}-{k}.png"));
                let shade = |base: usize, step: usize, i: usize| ((base + step * i) % 256) as u8;
                art(&shot, (320, 224), [shade(60, 40, k), shade(90, 20, n), 160]);
                game.details.screenshot = Some(shot);
                let cover = pictures.join(format!("{name}-{k}-capa.png"));
                art(&cover, (150, 210), [180, shade(40, 30, k), 70]);
                game.image = Some(cover);
                game.details.description = format!(
                    "{g}: uma aventura de teste com fases rápidas, chefes grandes e um modo para dois jogadores. A descrição é longa de propósito, para quebrar linha e terminar em reticências quando não cabe."
                );
                game.details.developer = "Estúdio Teste".into();
                game.details.rating = Some(70 + 5 * (k % 5) as u8);
                game
            })
            .collect();
        data.games.insert((*name).into(), list);
    }
    let sonic = &mut data.games.get_mut("megadrive").unwrap()[7];
    sonic.details.logo = None;
    // Four and a half stars.
    sonic.details.rating = Some(85);
    let aladdin = &mut data.games.get_mut("megadrive").unwrap()[0];
    let mark = pictures.join("aladdin-logo.png");
    logo(&mark, [240, 180, 40]);
    aladdin.details.logo = Some(mark);
    aladdin.details.rating = None;
    data
}

fn load(dir: &Path, view: &str) -> Theme {
    let default = Theme::load(&shipped("default"));
    let mut theme = Theme::load_over(dir, &default);
    theme.active_view = view.into();
    theme
}

#[test]
fn the_test_theme_reads_without_warnings_and_places_its_list() {
    let dir = tempfile::tempdir().unwrap();
    theme_folder(dir.path());
    let mut theme = Theme::builtin();
    let said = warnings(|| theme = load(dir.path(), "albedo"));
    assert!(said.is_empty(), "{said:#?}");
    // Variables reach the top-level keys too.
    assert_eq!(
        (
            theme.highlight_bg.r,
            theme.highlight_bg.g,
            theme.highlight_bg.b
        ),
        (0xF9, 0xC4, 0x40)
    );
    for (w, h) in SIZES {
        let fitted = theme.fitted(w, h);
        let mut view = View::lista(&fitted);
        let said = warnings(|| view = fitted.view("albedo"));
        assert!(said.is_empty(), "{said:#?}");
        let games = view.games.scene.as_ref().expect("a free games screen");
        // The list element is the layout's list, sizes made fractions of 480.
        let list = &view.games.list;
        assert!((list.rect.w - 0.433).abs() < 1e-4 && (list.rect.y - 0.15).abs() < 1e-4);
        let tall = fitted.shape.reference.1;
        assert!(
            (list.row - 0.0775 * tall / 480.).abs() < 1e-4,
            "{}",
            list.row
        );
        // Ordered by z; `base` copied the systems' elements.
        let z: Vec<i32> = games.elements.iter().map(|e| e.z).collect();
        assert!(z.windows(2).all(|p| p[0] <= p[1]), "{z:?}");
        for id in ["bordas", "status", "dicas"] {
            let element = games.elements.iter().find(|e| e.id == id).unwrap();
            assert!(!matches!(element.kind, Kind::List(_)));
        }
        // `when.aspect` leaves elements out of other shapes.
        let square = games.elements.iter().any(|e| e.id == "posicao");
        assert_eq!(square, h == w, "{w}x{h}");
        // The fixed places are gone; the view's other screens keep theirs.
        assert!(view.games.art.is_none() && view.games.panel.is_none());
        assert!(view.systems.scene.is_some());
        // The anchor moved the box: its middle is at x, y.
        let shot = games.elements.iter().find(|e| e.id == "captura").unwrap();
        assert!((shot.rect.x + shot.rect.w / 2. - 0.717).abs() < 1e-4);
    }
    // A theme's other views, and themes without `el`, stay as they were.
    let lista = theme.view("lista");
    assert!(lista.games.scene.is_none() && lista.games.art.is_some());
    let solo = theme.view("solo");
    assert_eq!(solo.games.list.mark, Mark::Grid);
    assert_eq!(solo.games.list.columns, 3);
    // The stars and the facts, with their own pictures and lines.
    let kind = |view: &View, id: &str| {
        let scene = view.games.scene.as_ref().unwrap();
        scene
            .elements
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .kind
            .clone()
    };
    let Kind::Fields(fields) = kind(&solo, "ficha") else {
        panic!("fields")
    };
    assert_eq!(fields.rows.len(), 5);
    assert!(fields.rows[4].value.is_none() && fields.hide_empty);
    let Kind::Rating(stars) = kind(&theme.view("albedo"), "nota") else {
        panic!("rating")
    };
    assert!(stars.full.is_some() && stars.empty_color.is_some());
}

#[test]
fn bad_values_are_warnings_and_the_rest_still_draws() {
    let source = r##"
[vars]
cor = "#FF0000"
[view.x]
[[view.x.systems.el]]
kind = "mystery"
[[view.x.systems.el]]
kind = "image"
src = ["../../etc/passwd", "/abs.png"]
[[view.x.systems.el]]
kind = "text"
text = "${system.odd} ${system.name}"
rect = "everywhere"
anchor = "middle-ish"
opacity = "half"
rotate = 9000
font = "../fora.ttf"
color = "rosa"
size = 7
shadow = 3
when = { system = 5, mood = "happy" }
[[view.x.systems.el]]
kind = "text"
text = "never"
when = { aspect = "tall" }
[[view.x.systems.el]]
kind = "rect"
fill = "$nada"
gradient = { from = "red" }
radius = -1
[[view.x.systems.el]]
kind = "list"
rect = { x = -0.5, y = 0.1, w = 3, h = 0.5 }
row = "tall"
selected = { fill = 1, height = 0.1 }
mark = "wheel"
[[view.x.systems.el]]
kind = "list"
[[view.x.systems.el]]
kind = "hints"
style = "neon"
[[view.x.systems.el]]
kind = "status"
base = "nobody"
[[view.x.systems.el]]
kind = "frame"
image = "frames/b.png"
slice = "lots"
[[view.x.systems.el]]
kind = "text"
text = "ok"
color = "$cor"
[[view.x.systems.el]]
kind = "rating"
value = "game.name"
star = "/abs.png"
size = "big"
empty_opacity = 3
[[view.x.systems.el]]
kind = "fields"
rows = "none"
[[view.x.systems.el]]
kind = "fields"
rows = [5, { label = "a" }, { label = "b", rating = "game.year" }, { label = "c", value = "${game.genre}", extra = 1 }]
divider = "nope"
row = -1
[view.y.games]
el = "not a list"
[view.z.games]
el = []
"##;
    let table: toml::Table = toml::from_str(source).unwrap();
    let mut theme = Theme::builtin();
    let mut said = warnings(|| theme = Theme::parse(&table));
    let mut view = View::lista(&theme);
    said.extend(warnings(|| view = theme.view("x")));
    said.extend(warnings(|| {
        // An empty list keeps the view's own screen, as does one that is no list.
        assert!(theme.view("z").games.scene.is_none());
        assert!(theme.view("y").games.scene.is_none());
    }));
    let scene = view.systems.scene.as_ref().expect("the good elements stay");
    // The text with bad fields, the bad rect and list, and the plain text.
    let kinds: Vec<&str> = scene
        .elements
        .iter()
        .map(|e| match e.kind {
            Kind::Text(_) => "text",
            Kind::Fill(_) => "rect",
            Kind::List(_) => "list",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds.iter().filter(|k| **k == "list").count(), 1);
    assert!(kinds.contains(&"text") && kinds.contains(&"rect"));
    // The rating stays with the launcher's star; the second fields keeps
    // its two good lines.
    let fields: Vec<_> = scene
        .elements
        .iter()
        .filter_map(|e| match &e.kind {
            archr_ui::layout::Kind::Fields(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].rows.len(), 2);
    assert!(scene
        .elements
        .iter()
        .any(|e| matches!(&e.kind, archr_ui::layout::Kind::Rating(s) if s.full.is_none())));
    let ok = scene
        .elements
        .iter()
        .find_map(|e| match &e.kind {
            Kind::Text(t) if t.color.is_some() => Some(t.color.unwrap()),
            _ => None,
        })
        .unwrap();
    assert_eq!((ok.r, ok.g, ok.b), (255, 0, 0), "$cor filled in");
    let list = &view.systems.list;
    assert!(
        list.rect.x >= 0. && list.rect.x + list.rect.w <= 1.0001,
        "{list:?}"
    );
    // The second list is turned away whole: the first one's box stays.
    assert!(
        (list.rect.y - 0.1).abs() < 1e-4 && (list.rect.h - 0.5).abs() < 1e-4,
        "{list:?}"
    );
    for word in [
        "kind",
        "src",
        "system.odd",
        "rect",
        "anchor",
        "opacity",
        "rotate",
        "font",
        "color",
        "shadow",
        "aspect",
        "mood",
        "nada",
        "gradient",
        "radius",
        "row",
        "mark",
        "one list",
        "style",
        "base",
        "slice",
        "el must be",
        "only game.rating",
        "star",
        "size",
        "empty_opacity",
        "rows must be",
        "needs a value",
        "extra",
        "divider",
        "row = -1",
    ] {
        assert!(
            said.iter().any(|w| w.contains(word)),
            "no warning about {word}: {said:#?}"
        );
    }
}

#[test]
fn every_example_of_the_docs_chapter_reads_without_warnings() {
    let docs =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/temas.md"))
            .unwrap();
    let start = docs.find("\n## Telas livres").expect("the chapter");
    let end = docs[start + 1..]
        .find("\n## ")
        .map_or(docs.len(), |n| start + 1 + n);
    let chapter = &docs[start..end];
    let mut blocks = 0;
    for block in chapter.split("```toml\n").skip(1) {
        let block = block.split("```").next().unwrap();
        blocks += 1;
        let table: toml::Table = toml::from_str(block).unwrap_or_else(|e| panic!("{e}\n{block}"));
        let mut theme = Theme::builtin();
        let mut said = warnings(|| theme = Theme::parse(&table));
        for (id, _) in theme.views() {
            for (w, h) in SIZES {
                let fitted = theme.fitted(w, h);
                said.extend(warnings(|| drop(fitted.view(&id))));
            }
        }
        // A theme file that is not in the docs folder is no warning here.
        // Nor is an include, which only a theme folder can resolve.
        said.retain(|w| {
            !w.contains("outside the theme directory")
                && !w.contains("include needs the theme folder")
        });
        assert!(said.is_empty(), "{block}\n{said:#?}");
    }
    assert!(blocks >= 4, "{blocks} examples");
}

/// The test theme drawn at three sizes: both views, systems and games, a
/// system with its own element and a game without a logo; every frame shows
/// its art, the list in its place and the colors the theme gave it.
#[test]
fn free_screens_draw_at_every_size() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_SCENE_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cena");
    std::fs::create_dir_all(&folder).unwrap();
    theme_folder(&folder);
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let data = library(&pictures);
    let mut timings = Vec::new();
    for view_id in ["solo", "albedo"] {
        let theme = load(&folder, view_id);
        for size in SIZES {
            let theme = theme.fitted(size.0, size.1);
            let view = theme.view(&theme.active_view);
            let root = tempfile::tempdir().unwrap();
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.theme_dir = Some(folder.clone());
            ui.default_theme_dir = Some(shipped("default"));
            ui.set_motion(archr_ui::motion::Motion::Off);
            ui.configure(&ttf, &creator, &theme, Some(root.path()))
                .unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            ui.catalog = archr_ui::i18n::I18n::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales"),
                Some("pt_BR"),
                None,
            );
            ui.pin_clock(std::time::Instant::now(), 1_794_916_800);
            let mut covers =
                archr_ui::covers::CoverCache::new(&creator, root.path().join("covers"), 320, 240);
            let all: Vec<PathBuf> = data
                .games
                .values()
                .flatten()
                .filter_map(|g| g.image.clone())
                .collect();
            let mut app = App::new(10);
            app.set_view(&view);
            // The free games screen loads its own pictures.
            assert!(app.games_free());
            // `settle`: until every picture asked for has come; else the
            // first frame alone.
            let mut frame = |ui: &mut Ui, name: &str, app: &App, settle: bool| -> Vec<u8> {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
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
                    let began = std::time::Instant::now();
                    ui.draw_with_settings(app, &data, &theme, art, None)
                        .unwrap();
                    let took = began.elapsed();
                    if !settle {
                        break;
                    }
                    if covers.idle() && ui.pictures_idle() && round >= 3 {
                        timings.push((format!("{view_id}-{name}-{}x{}", size.0, size.1), took));
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "pictures never loaded"
                    );
                    round += 1;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                if let Some(out) = &out {
                    let file = out.join(format!("{view_id}-{name}-{}x{}.png", size.0, size.1));
                    image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                }
                pixels
            };
            let px = |pixels: &[u8], x: f32, y: f32| {
                let (x, y) = ((x * size.0 as f32) as usize, (y * size.1 as f32) as usize);
                let at = (y * size.0 as usize + x) * 4;
                [pixels[at], pixels[at + 1], pixels[at + 2]]
            };
            // How much of a region is near `rgb`.
            let share = |pixels: &[u8], (x0, y0, x1, y1): (f32, f32, f32, f32), rgb: [u8; 3]| {
                let (mut hit, mut all) = (0, 0);
                for y in (y0 * size.1 as f32) as usize..(y1 * size.1 as f32) as usize {
                    for x in (x0 * size.0 as f32) as usize..(x1 * size.0 as f32) as usize {
                        let at = (y * size.0 as usize + x) * 4;
                        all += 1;
                        if (0..3).all(|k| pixels[at + k].abs_diff(rgb[k]) < 12) {
                            hit += 1;
                        }
                    }
                }
                hit as f32 / all.max(1) as f32
            };
            let mut shot = |ui: &mut Ui, name: &str, app: &App| frame(ui, name, app, true);
            assert!(matches!(app.screen, Screen::Systems));
            let systems = shot(&mut ui, "sistemas", &app);
            app.handle(Button::Down, &data);
            // The next system's art was loaded with this one's: it shows on
            // the first frame.
            let first = frame(&mut ui, "sistemas-primeiro-quadro", &app, false);
            let megadrive = frame(&mut ui, "sistemas-megadrive", &app, true);
            app.handle(Button::A, &data);
            assert!(matches!(app.screen, Screen::Games { .. }));
            let games = frame(&mut ui, "jogos", &app, true);
            app.cursor = 7;
            let sonic = frame(&mut ui, "jogos-sem-logo", &app, true);
            // The Nintendo 64: no logo in the theme, the launcher's picture.
            app.handle(Button::B, &data);
            app.handle(Button::Down, &data);
            app.handle(Button::Down, &data);
            app.handle(Button::A, &data);
            assert!(matches!(&app.screen, Screen::Games { system, .. } if system == "n64"));
            let n64 = frame(&mut ui, "jogos-n64", &app, true);
            let red = [0xE8, 0x34, 0x3B];
            if view_id == "solo" {
                // The art fills the screen: its sun at the top left.
                let sun = px(&systems, 0.28, 0.36);
                assert!(sun[0] > 200 && sun[1] > 150, "{sun:?}");
                // The Mega Drive's own element, only on the Mega Drive.
                assert!(share(&megadrive, (0.04, 0.06, 0.24, 0.12), red) > 0.02);
                assert!(share(&systems, (0.04, 0.06, 0.24, 0.12), red) < 0.001);
                // The dark filter over the art on the games screen.
                let filtered = px(&games, 0.45, 0.06);
                assert!(filtered.iter().all(|c| *c < 140), "{filtered:?}");
                // The Mega Drive's art, already there on the first frame.
                let art = px(&first, 0.3, 0.55);
                assert!(art[2] > 180 && art[0] < 80, "{art:?}");
                // Its logo is the SVG of the name it shares, on both screens.
                let blue = [0x2E, 0x9B, 0xE6];
                assert!(share(&megadrive, (0.04, 0.74, 0.5, 0.9), blue) > 0.05);
                assert!(share(&games, (0.02, 0.02, 0.32, 0.14), blue) > 0.05);
                // The facts: a fourth line with the stars for Sonic (rated),
                // none for Aladdin (not rated); the empty genre left out.
                let row = (0.84, 0.81, 0.96, 0.86);
                assert!(share(&sonic, row, [255, 255, 255]) > 0.05);
                assert!(share(&games, row, [255, 255, 255]) < 0.01);
                let console = [100, 210, 150];
                assert!(share(&n64, (0.02, 0.02, 0.32, 0.14), console) > 0.05);
            } else {
                let accent = [0xF9, 0xC4, 0x40];
                // The selected row in the accent, in the list's box only.
                assert!(share(&games, (0.0, 0.15, 0.433, 0.925), accent) > 0.03);
                assert!(share(&sonic, (0.0, 0.15, 0.433, 0.925), accent) > 0.03);
                assert!(share(&systems, (0.0, 0.12, 0.333, 0.88), accent) > 0.03);
                // The theme's star in the accent, only for a rated game.
                let stars = (0.7667, 0.65, 0.9667, 0.675);
                assert!(share(&sonic, stars, accent) > 0.1);
                assert!(share(&games, stars, accent) < 0.01);
                // The nine-slice border runs around the screen.
                let edge = px(&systems, 0.5, 0.0);
                assert!(edge[0] > 230 && edge[2] < 100, "{edge:?}");
                // The turned name stands in a narrow tall strip.
                let turned = share(&systems, (0.30, 0.1, 0.36, 0.8), [255, 255, 255]);
                assert!(turned > 0.02, "{turned}");
            }
        }
    }
    for (name, took) in &timings {
        eprintln!("{name}: {took:?}");
    }
}

/// The two collection views read without a warning at every size, each
/// screen with its carousel or grid taking the d-pad by the one rule.
#[test]
fn collections_read_and_take_the_dpad_by_one_rule() {
    use archr_ui::layout::{Axis, Shape};
    let dir = tempfile::tempdir().unwrap();
    theme_folder(dir.path());
    let pictures = tempfile::tempdir().unwrap();
    let data = library(pictures.path());
    for (w, h) in SIZES {
        for id in ["carrossel", "roda"] {
            let theme = load(dir.path(), id).fitted(w, h);
            let mut view = View::lista(&theme);
            let said = warnings(|| view = theme.view(id));
            assert!(said.is_empty(), "{id} {w}x{h}: {said:#?}");
            let systems = view.systems.scene.as_ref().unwrap().collection().unwrap();
            assert!(matches!(
                systems.shape,
                Shape::Carousel {
                    axis: Axis::Vertical,
                    ..
                }
            ));
            assert!(systems.wrap);
            let games = view.games.scene.as_ref().unwrap().collection().unwrap();
            match (id, games.shape) {
                ("carrossel", Shape::Grid { columns, rows }) => assert_eq!((columns, rows), (3, 3)),
                ("roda", Shape::Carousel { axis, visible, .. }) => {
                    assert_eq!((axis, visible), (Axis::Horizontal, 5));
                    assert!(games.reflection > 0.);
                }
                other => panic!("{other:?}"),
            }
            let mut app = App::new(10);
            app.set_view(&view);
            // The grid is the scene's: no cover strip, no system change on L1/R1.
            assert!(app.games_grid().is_none());
            let press = |app: &mut App, b: Button, n: usize| {
                for _ in 0..n {
                    app.handle(b, &data);
                }
            };
            // Systems: a column of 4 that loops; ↑ from the first is the last.
            assert!(matches!(app.screen, Screen::Systems));
            press(&mut app, Button::Down, 3);
            assert_eq!(app.cursor, 3);
            press(&mut app, Button::Down, 1);
            assert_eq!(app.cursor, 0, "the loop comes round");
            press(&mut app, Button::Up, 1);
            assert_eq!(app.cursor, 3);
            // ←/→ and L1/R1 jump a page (the visible items), never round.
            press(&mut app, Button::L, 1);
            let page = if id == "roda" { 5 } else { 3 };
            assert_eq!(app.cursor, 3usize.saturating_sub(page));
            press(&mut app, Button::Right, 1);
            assert_eq!(app.cursor, 3);
            // The Mega Drive: 9 games.
            app.cursor = 1;
            press(&mut app, Button::A, 1);
            assert!(matches!(&app.screen, Screen::Games { system, .. } if system == "megadrive"));
            if id == "carrossel" {
                // ←/→ one, ↑/↓ a row, L1/R1 a page of nine; no loop here.
                press(&mut app, Button::Right, 2);
                assert_eq!(app.cursor, 2);
                press(&mut app, Button::Right, 1);
                assert_eq!(app.cursor, 3, "the next row's first");
                press(&mut app, Button::Down, 2);
                assert_eq!(app.cursor, 6, "the bottom row stops ↓");
                press(&mut app, Button::Up, 1);
                assert_eq!(app.cursor, 3);
                press(&mut app, Button::Left, 4);
                assert_eq!(app.cursor, 0, "no loop: the first stays");
                press(&mut app, Button::R, 1);
                assert_eq!(app.cursor, 8, "a page, as far as the last");
                press(&mut app, Button::L, 1);
                assert_eq!(app.cursor, 0);
            } else {
                // A row: ←/→ and ↑/↓ one, round the loop; L1/R1 five.
                press(&mut app, Button::Left, 1);
                assert_eq!(app.cursor, 8, "the loop comes round");
                press(&mut app, Button::Up, 2);
                assert_eq!(app.cursor, 6);
                press(&mut app, Button::Right, 3);
                assert_eq!(app.cursor, 0);
                press(&mut app, Button::R, 1);
                assert_eq!(app.cursor, 5);
                press(&mut app, Button::R, 1);
                assert_eq!(app.cursor, 8, "a page stops at the end");
            }
            // A, B, X and Y as always: B goes back to the systems.
            press(&mut app, Button::B, 1);
            assert!(matches!(app.screen, Screen::Systems));
            assert_eq!(app.cursor, 1);
        }
    }
}

#[test]
fn bad_collection_values_are_warnings() {
    let source = r##"
[view.x]
[[view.x.systems.el]]
kind = "carousel"
of = "apps"
axis = "diagonal"
visible = 99
wheel = 3
selected = "big"
item = 5
reflection = 4
columns = 2
[[view.x.systems.el]]
kind = "grid"
[[view.x.games.el]]
kind = "grid"
columns = 20
favorite = 7
selected = { frame = { image = "../x.png", slice = 8 }, glow = 1 }
item = { image = ["game.cover", "/abs.png"], logo = 3, size = "big" }
[[view.x.games.el]]
kind = "list"
"##;
    let table: toml::Table = toml::from_str(source).unwrap();
    let theme = Theme::parse(&table);
    let mut view = View::lista(&theme);
    let said = warnings(|| view = theme.view("x"));
    let systems = view.systems.scene.as_ref().unwrap().collection().unwrap();
    // The carousel stays, with its values limited or left as they were.
    match systems.shape {
        archr_ui::layout::Shape::Carousel { visible, wheel, .. } => {
            assert_eq!(visible, 15);
            assert!(wheel.is_none());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(systems.selected.scale, 1.);
    assert_eq!(systems.reflection, 1.);
    let games = view.games.scene.as_ref().unwrap();
    let grid = games.collection().unwrap();
    assert!(matches!(
        grid.shape,
        archr_ui::layout::Shape::Grid {
            columns: 8,
            rows: 3
        }
    ));
    assert_eq!(grid.item.image.len(), 1, "the unsafe path is left out");
    assert!(grid.selected.frame.is_none());
    assert!(
        !games
            .elements
            .iter()
            .any(|e| matches!(e.kind, Kind::List(_))),
        "one collection a screen"
    );
    for word in [
        "of = \"apps\"",
        "axis",
        "visible",
        "wheel",
        "selected must be",
        "item must be",
        "reflection",
        "columns",
        "favorite",
        "../x.png",
        "glow",
        "/abs.png",
        "logo",
        "one list, carousel or grid",
    ] {
        assert!(
            said.iter().any(|w| w.contains(word)),
            "no warning about {word}: {said:#?}"
        );
    }
}

/// The collections drawn at three sizes: settled, halfway through a glide
/// and settled again; the focused logo larger than the others, the grid's
/// frame on its focus, the wheel's items turned. With
/// ARCHR_SCENE_CAPTURE_DIR set, the frames are saved there; the time a
/// frame takes while the carousel glides is printed.
#[test]
fn collections_draw_and_glide_at_every_size() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_SCENE_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cena");
    std::fs::create_dir_all(&folder).unwrap();
    theme_folder(&folder);
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let mut data = library(&pictures);
    let megadrive = data.games.get_mut("megadrive").unwrap();
    megadrive[2].favorite = true;
    // Six more, for a grid that scrolls.
    for n in 0..6 {
        let mut more = megadrive[n].clone();
        more.name = format!("{} II", more.name);
        more.path = format!("more-{n}.rom").into();
        megadrive.push(more);
    }
    let mut timings = Vec::new();
    for view_id in ["carrossel", "roda"] {
        let theme = load(&folder, view_id);
        for size in SIZES {
            let theme = theme.fitted(size.0, size.1);
            let view = theme.view(&theme.active_view);
            let root = tempfile::tempdir().unwrap();
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.theme_dir = Some(folder.clone());
            ui.default_theme_dir = Some(shipped("default"));
            ui.set_motion(archr_ui::motion::Motion::On);
            ui.configure(&ttf, &creator, &theme, Some(root.path()))
                .unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            ui.catalog = archr_ui::i18n::I18n::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales"),
                Some("pt_BR"),
                None,
            );
            let mut app = App::new(10);
            app.set_view(&view);
            let mut clock = std::time::Instant::now();
            let unix = 1_794_916_800;
            // Draws at `clock`; `settle`: again until every picture is in,
            // then once more when the motion is over (2 s on). A frame's
            // time counts until the GPU has drawn it (one pixel read back).
            let frame = |ui: &mut Ui, name: &str, app: &App, clock, settle: bool| {
                ui.pin_clock(clock, unix);
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
                let mut round = 0;
                let mut took;
                loop {
                    ui.tick();
                    let began = std::time::Instant::now();
                    ui.draw_with_settings(app, &data, &theme, Default::default(), None)
                        .unwrap();
                    let one = sdl2::rect::Rect::new(0, 0, 1, 1);
                    ui.canvas
                        .read_pixels(one, sdl2::pixels::PixelFormatEnum::RGBA32)
                        .unwrap();
                    took = began.elapsed();
                    if !settle || ui.pictures_idle() && round >= 3 {
                        if settle {
                            ui.pin_clock(clock + std::time::Duration::from_secs(2), unix);
                            ui.draw_with_settings(app, &data, &theme, Default::default(), None)
                                .unwrap();
                        }
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "pictures never loaded"
                    );
                    round += 1;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                if let Some(out) = &out {
                    let file = out.join(format!("{view_id}-{name}-{}x{}.png", size.0, size.1));
                    image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                }
                (pixels, took)
            };
            let bright = |pixels: &[u8], (x0, y0, x1, y1): (f32, f32, f32, f32)| {
                let mut hit = 0;
                for y in (y0 * size.1 as f32) as usize..(y1 * size.1 as f32) as usize {
                    for x in (x0 * size.0 as f32) as usize..(x1 * size.0 as f32) as usize {
                        let at = (y * size.0 as usize + x) * 4;
                        if pixels[at..at + 3].iter().all(|c| *c > 235) {
                            hit += 1;
                        }
                    }
                }
                hit
            };
            let step = std::time::Duration::from_millis(1);
            let (systems, _) = frame(&mut ui, "sistemas", &app, clock, true);
            // A step down: the glide starts now and is halfway at 60 ms
            // (ease-out), then settles. Every frame of it is timed.
            app.handle(Button::Down, &data);
            clock += step;
            frame(&mut ui, "sistemas-inicio", &app, clock, false);
            let mut glide = Vec::new();
            for ms in [15, 30, 45, 60] {
                let (pixels, took) = frame(
                    &mut ui,
                    "sistemas-meio",
                    &app,
                    clock + std::time::Duration::from_millis(ms),
                    false,
                );
                glide.push(took);
                if ms == 60 {
                    // Halfway: neither where it was nor where it goes.
                    assert_ne!(pixels, systems);
                }
            }
            clock += std::time::Duration::from_secs(5);
            let (megadrive, _) = frame(&mut ui, "sistemas-megadrive", &app, clock, true);
            let mean = glide.iter().sum::<std::time::Duration>() / glide.len() as u32;
            timings.push((format!("{view_id}-glide-{}x{}", size.0, size.1), mean));
            if view_id == "carrossel" {
                // The focused logo (twice as large) is in the middle of the
                // column, on the right; the white of the SNES pill there
                // first, then the Mega Drive's name text (an SVG pill).
                let middle = (0.62, 0.36, 0.96, 0.54);
                assert!(bright(&systems, middle) > bright(&systems, (0.62, 0.1, 0.96, 0.2)));
                assert_ne!(bright(&megadrive, middle), bright(&systems, middle));
            } else {
                // The wheel's turned items lie in the left third only.
                assert!(bright(&systems, (0.0, 0.0, 0.34, 1.0)) > 0);
            }
            // The games: the grid's focus framed white, the row's focus
            // larger; three steps on (the third game is a favorite).
            // The screen slides in first.
            app.handle(Button::A, &data);
            clock += step;
            frame(&mut ui, "jogos-entrando", &app, clock, false);
            clock += std::time::Duration::from_secs(5);
            let (games, _) = frame(&mut ui, "jogos", &app, clock, true);
            for _ in 0..2 {
                app.handle(Button::Right, &data);
            }
            clock += std::time::Duration::from_secs(5);
            let (favorite, _) = frame(&mut ui, "jogos-favorito", &app, clock, true);
            if view_id == "carrossel" {
                // The white frame around the first cover.
                assert!(bright(&games, (0.0, 0.03, 0.24, 0.35)) > 20);
                // Three rows down: the grid scrolled a row, halfway first.
                for _ in 0..3 {
                    app.handle(Button::Down, &data);
                }
                clock += step;
                frame(&mut ui, "jogos-rolando-inicio", &app, clock, false);
                let half = clock + std::time::Duration::from_millis(40);
                frame(&mut ui, "jogos-rolando", &app, half, false);
                clock += std::time::Duration::from_secs(5);
                let (scrolled, _) = frame(&mut ui, "jogos-rolou", &app, clock, true);
                assert_ne!(scrolled, favorite);
            }
            assert_ne!(games, favorite);
        }
    }
    for (name, took) in &timings {
        eprintln!("{name}: {took:?} a frame");
    }
}

#[test]
fn bad_video_values_are_warnings() {
    let source = r##"
[view.x]
[[view.x.games.el]]
kind = "video"
src = ["/abs.mp4", "game.screenshot", "../x.mp4"]
[[view.x.games.el]]
kind = "video"
src = "game.video"
fallback = "game.screenshot"
mute = false
delay = 99
loop = "sim"
fit = "zoom"
volume = 1
[[view.x.games.el]]
kind = "video"
src = "videos/b.mp4"
"##;
    let table: toml::Table = toml::from_str(source).unwrap();
    let theme = Theme::parse(&table);
    let mut view = View::lista(&theme);
    let said = warnings(|| view = theme.view("x"));
    let scene = view.games.scene.as_ref().unwrap();
    // The one with a clip stays, its values limited or as they were.
    let videos: Vec<_> = scene
        .elements
        .iter()
        .filter_map(|e| match &e.kind {
            Kind::Video(v) => Some(v),
            _ => None,
        })
        .collect();
    assert_eq!(videos.len(), 1, "one video a screen");
    let video = videos[0];
    assert_eq!(video.delay, 10.);
    assert!(video.repeat);
    assert_eq!(video.fit, archr_ui::layout::ImageFit::Cover);
    assert!(video.still.is_some());
    for word in [
        "/abs.mp4",
        "game.screenshot",
        "../x.mp4",
        "names no video",
        "mute",
        "delay",
        "loop",
        "fit",
        "volume",
        "one video",
    ] {
        assert!(
            said.iter().any(|w| w.contains(word)),
            "no warning about {word}: {said:#?}"
        );
    }
}

/// The `video` element at three sizes: the clip it asks the host for (the
/// game's, or the system's under the name it shares), its still until the
/// host plays that clip, the frame in its box once it does, and never an
/// old clip's frame over a new item. A fake frame stands in for the
/// player: it is drawn the same way. With ARCHR_SCENE_CAPTURE_DIR set, the
/// frames are saved there.
#[test]
fn the_video_element_asks_for_its_clip_and_shows_its_frame_or_still() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_SCENE_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cena");
    std::fs::create_dir_all(folder.join("videos")).unwrap();
    theme_folder(&folder);
    // The Mega Drive's clip under the name it shares; only a file to find.
    let genesis = folder.join("videos/genesis.mp4");
    std::fs::write(&genesis, b"clip").unwrap();
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let mut data = library(&pictures);
    let aladdin = pictures.join("aladdin.mp4");
    std::fs::write(&aladdin, b"clip").unwrap();
    data.games.get_mut("megadrive").unwrap()[0].video = Some(aladdin.clone());
    let magenta = [255, 0, 255];
    for size in SIZES {
        let theme = load(&folder, "albedo").fitted(size.0, size.1);
        let view = theme.view(&theme.active_view);
        let root = tempfile::tempdir().unwrap();
        let (ttf, creator);
        let mut ui = Ui::new(size.0, size.1, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        ui.theme_dir = Some(folder.clone());
        ui.default_theme_dir = Some(shipped("default"));
        ui.set_motion(archr_ui::motion::Motion::Off);
        ui.configure(&ttf, &creator, &theme, Some(root.path()))
            .unwrap();
        ui.set_status(archr_ui::status::Status::frozen("14:32"));
        ui.pin_clock(std::time::Instant::now(), 1_794_916_800);
        // The player's frame: magenta, the size the element asked for.
        let fake = |(w, h): (u32, u32)| {
            let mut t = creator
                .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24, w, h)
                .unwrap();
            let bytes: Vec<u8> = magenta.repeat((w * h) as usize);
            t.update(None, &bytes, w as usize * 3).unwrap();
            t.set_blend_mode(sdl2::render::BlendMode::Blend);
            t
        };
        let mut app = App::new(10);
        app.set_view(&view);
        let frame = |ui: &mut Ui, app: &App, art: archr_ui::render::Art, name: &str| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
            let mut round = 0;
            loop {
                ui.tick();
                ui.snap_animations();
                ui.draw_with_settings(app, &data, &theme, art, None)
                    .unwrap();
                if ui.pictures_idle() && round >= 3 {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "pictures never loaded"
                );
                round += 1;
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let pixels = ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
            if let Some(out) = &out {
                let file = out.join(format!("video-{name}-{}x{}.png", size.0, size.1));
                image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8).unwrap();
            }
            pixels
        };
        let share = |pixels: &[u8], (x0, y0, x1, y1): (f32, f32, f32, f32)| {
            let (mut hit, mut all) = (0, 0);
            for y in (y0 * size.1 as f32) as usize..(y1 * size.1 as f32) as usize {
                for x in (x0 * size.0 as f32) as usize..(x1 * size.0 as f32) as usize {
                    let at = (y * size.0 as usize + x) * 4;
                    all += 1;
                    if (0..3).all(|k| pixels[at + k].abs_diff(magenta[k]) < 12) {
                        hit += 1;
                    }
                }
            }
            hit as f32 / all.max(1) as f32
        };
        let none = archr_ui::render::Art::default();
        // The SNES has no clip in the theme: nothing is asked for.
        frame(&mut ui, &app, none, "sistemas");
        assert_eq!(ui.scene_clip, None);
        // The Mega Drive's is found under the name it shares, in the box's
        // shape (0.5 x 0.8 of the screen), at most 640x480's pixels.
        app.handle(Button::Down, &data);
        frame(&mut ui, &app, none, "sistemas-megadrive");
        let clip = ui.scene_clip.clone().expect("the system's clip");
        assert_eq!(clip.file, genesis);
        assert_eq!(clip.fit, archr_ui::layout::ImageFit::Contain);
        let (w, h) = clip.size;
        assert!(w * h <= 640 * 480 + 1000, "{w}x{h}");
        let shape = (w as f32 / h as f32) / (0.5 * size.0 as f32 / (0.8 * size.1 as f32));
        assert!((shape - 1.).abs() < 0.02, "{w}x{h}");
        let system_box = (0.4667, 0.1, 0.9667, 0.9);
        let frame_of = fake(clip.size);
        let playing = archr_ui::render::Art {
            video: Some((&frame_of, 0.)),
            video_file: Some(&genesis),
            ..none
        };
        let shown = frame(&mut ui, &app, playing, "sistemas-video");
        assert!(share(&shown, system_box) > 0.6);
        // Into the games: Aladdin's clip, asked with its delay; its still
        // (the screen shot) until its frame comes.
        app.handle(Button::A, &data);
        let still = frame(&mut ui, &app, none, "jogos-antes");
        let clip = ui.scene_clip.clone().expect("the game's clip");
        assert_eq!(clip.file, aladdin);
        assert_eq!(clip.delay, std::time::Duration::from_secs(1));
        assert!(clip.repeat);
        // Below the logo and right of the cover, both drawn over it.
        let game_box = (0.62, 0.42, 0.95, 0.64);
        assert!(share(&still, game_box) < 0.01);
        let frame_of = fake(clip.size);
        let art = |file| archr_ui::render::Art {
            video: Some((&frame_of, 0.5)),
            video_file: Some(file),
            ..none
        };
        let playing = frame(&mut ui, &app, art(&aladdin), "jogos-video");
        assert!(share(&playing, game_box) > 0.9);
        // What a frame takes with the clip and with its still (until the
        // GPU has drawn it: one pixel read back), printed.
        let mut time = |art: archr_ui::render::Art| {
            let began = std::time::Instant::now();
            for _ in 0..20 {
                ui.draw_with_settings(&app, &data, &theme, art, None)
                    .unwrap();
                let one = sdl2::rect::Rect::new(0, 0, 1, 1);
                ui.canvas
                    .read_pixels(one, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
            }
            began.elapsed() / 20
        };
        let (with, without) = (time(art(&aladdin)), time(none));
        eprintln!(
            "video {}x{}: {with:?} a frame playing, {without:?} with the still",
            size.0, size.1
        );
        // Another clip's frame (the last item's) is never drawn here.
        let stale = frame(&mut ui, &app, art(&genesis), "jogos-outro");
        assert!(share(&stale, game_box) < 0.01);
        // A game without a clip asks for none; its screen shot shows.
        app.handle(Button::Down, &data);
        let other = frame(&mut ui, &app, art(&aladdin), "jogos-sem-video");
        assert_eq!(ui.scene_clip, None);
        assert!(share(&other, game_box) < 0.01);
        // Back on Aladdin, the clip is asked for again.
        app.handle(Button::Up, &data);
        frame(&mut ui, &app, none, "jogos-de-volta");
        assert_eq!(ui.scene_clip.as_ref().map(|c| &c.file), Some(&aladdin));
        // Its details inherit the games screen's clip, and play it.
        app.screen = Screen::Details {
            system: "megadrive".into(),
            game: 0,
        };
        frame(&mut ui, &app, none, "detalhes");
        let clip = ui.scene_clip.clone().expect("the details' clip");
        assert_eq!(clip.file, aladdin);
        let frame_of = fake(clip.size);
        let art = archr_ui::render::Art {
            video: Some((&frame_of, 0.5)),
            video_file: Some(&aladdin),
            ..none
        };
        let playing = frame(&mut ui, &app, art, "detalhes-video");
        assert!(share(&playing, game_box) > 0.9);
    }
}

/// The test theme's options, include and inherited view: the wheel comes
/// from parts/roda.toml, "solo-escuro" is Solo with its own changes, and the
/// options reach the top-level accent and the art the scene asks for.
#[test]
fn options_includes_and_inherited_views_read_without_warnings() {
    let dir = tempfile::tempdir().unwrap();
    theme_folder(dir.path());
    let mut theme = Theme::builtin();
    let said = warnings(|| theme = load(dir.path(), "solo-escuro"));
    assert!(said.is_empty(), "{said:#?}");
    let ids: Vec<String> = theme.views().into_iter().map(|v| v.0).collect();
    for id in ["roda", "solo-escuro"] {
        assert!(ids.iter().any(|v| v == id), "{id}: {ids:?}");
    }
    let options: Vec<&str> = theme.options().iter().map(|o| o.id.as_str()).collect();
    assert_eq!(options, ["cor", "fundo"]);
    assert_eq!(theme.options()[0].name.get("pt_BR"), Some("Tom do tema"));
    assert_eq!(theme.choice("cor"), Some("amarelo"));
    let rgb = |t: &Theme| (t.highlight_bg.r, t.highlight_bg.g, t.highlight_bg.b);
    assert_eq!(rgb(&theme), (0xF9, 0xC4, 0x40));
    // The included wheel reads as the theme.toml's own.
    let roda = theme.view("roda");
    assert!(roda.systems.scene.as_ref().unwrap().collection().is_some());
    // Solo inherited: the band removed, the list moved, a line added, the
    // rest as Solo has it.
    let element_ids = |view: &View| -> Vec<String> {
        let scene = view.systems.scene.as_ref().unwrap();
        scene.elements.iter().map(|e| e.id.clone()).collect()
    };
    let (solo, dark) = (theme.view("solo"), theme.view("solo-escuro"));
    let (mine, base) = (element_ids(&dark), element_ids(&solo));
    assert!(base.contains(&"faixa".to_owned()) && !mine.contains(&"faixa".to_owned()));
    assert!(mine.contains(&"herdado".to_owned()) && mine.contains(&"arte".to_owned()));
    assert_eq!(mine.len(), base.len());
    assert!((dark.systems.list.rect.x - 0.04).abs() < 1e-4);
    assert!((solo.systems.list.rect.x - 0.6).abs() < 1e-4);
    assert_eq!(dark.games, solo.games, "the screen it does not touch");
    // The choices: the accent at the top level, the art in the scene; the
    // other shapes keep them, an unknown one changes nothing.
    let chosen = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(o, c)| ((*o).to_owned(), (*c).to_owned()))
            .collect()
    };
    let mut purple = theme.clone();
    let said =
        warnings(|| purple = theme.with_options(&chosen(&[("cor", "roxo"), ("fundo", "escuro")])));
    assert!(said.is_empty(), "{said:#?}");
    assert_eq!(rgb(&purple), (0xA5, 0x6D, 0xE2));
    assert_eq!(purple.choice("fundo"), Some("escuro"));
    assert_eq!(purple.font, theme.font);
    let art = |t: &Theme| {
        let view = t.view("solo");
        let scene = view.systems.scene.unwrap();
        let element = scene.elements.into_iter().find(|e| e.id == "arte").unwrap();
        format!("{:?}", element.kind)
    };
    assert!(art(&purple).contains("arts2/") && !art(&theme).contains("arts2/"));
    assert_eq!(rgb(&purple.fitted(720, 720)), (0xA5, 0x6D, 0xE2));
    assert_eq!(purple.fitted(720, 720).choice("cor"), Some("roxo"));
    assert_eq!(
        theme.with_options(&chosen(&[("cor", "verde"), ("x", "y")])),
        theme
    );
    assert_eq!(purple.with_options(&[]), theme);
}

/// Choosing the dark background redraws the Solo systems screen with the
/// other art, and the purple accent reaches the inherited view's line.
#[test]
fn a_chosen_option_draws_its_art_and_color() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cena");
    std::fs::create_dir_all(&folder).unwrap();
    theme_folder(&folder);
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let data = library(&pictures);
    let size = (640, 480);
    let root = tempfile::tempdir().unwrap();
    let (ttf, creator);
    let mut ui = Ui::new(size.0, size.1, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    ui.theme_dir = Some(folder.clone());
    ui.default_theme_dir = Some(shipped("default"));
    ui.set_motion(archr_ui::motion::Motion::Off);
    ui.set_status(archr_ui::status::Status::frozen("14:32"));
    let mut draw = |theme: &Theme, name: &str| -> Vec<u8> {
        ui.configure(&ttf, &creator, theme, Some(root.path()))
            .unwrap();
        let mut app = App::new(10);
        app.set_view(&theme.view(&theme.active_view));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut round = 0;
        loop {
            ui.tick();
            ui.snap_animations();
            ui.draw_with_settings(&app, &data, theme, Default::default(), None)
                .unwrap();
            if ui.pictures_idle() && round >= 3 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "pictures never loaded"
            );
            round += 1;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        if let Some(out) = std::env::var_os("ARCHR_SCENE_CAPTURE_DIR").map(PathBuf::from) {
            std::fs::create_dir_all(&out).unwrap();
            let file = out.join(format!("opcoes-{name}-640x480.png"));
            image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8).unwrap();
        }
        pixels
    };
    let px = |pixels: &[u8], x: f32, y: f32| {
        let at = ((y * 480.) as usize * 640 + (x * 640.) as usize) * 4;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    let near = |pixels: &[u8], rgb: [u8; 3]| {
        let mut hit = 0;
        for y in (0.86 * 480.) as usize..(0.92 * 480.) as usize {
            for x in (0.04 * 640.) as usize..(0.54 * 640.) as usize {
                let at = (y * 640 + x) * 4;
                if (0..3).all(|k| pixels[at + k].abs_diff(rgb[k]) < 24) {
                    hit += 1;
                }
            }
        }
        hit
    };
    let theme = load(&folder, "solo");
    let pairs = |c: &str, f: &str| {
        vec![
            ("cor".to_owned(), c.to_owned()),
            ("fundo".to_owned(), f.to_owned()),
        ]
    };
    let light = draw(&theme, "claro");
    let dark = draw(&theme.with_options(&pairs("amarelo", "escuro")), "escuro");
    // The art's light half beside the sun: the Super Nintendo's violet,
    // a quarter as bright in the dark art.
    let (a, b) = (px(&light, 0.1, 0.5), px(&dark, 0.1, 0.5));
    assert!(a[2] > 180 && b[2] < 80, "{a:?} {b:?}");
    let mut inherited = load(&folder, "solo-escuro");
    let yellow = draw(&inherited, "herdado-amarelo");
    inherited = inherited.with_options(&pairs("roxo", "claro"));
    let purple = draw(&inherited, "herdado-roxo");
    let (y, p) = ([0xF9, 0xC4, 0x40], [0xA5, 0x6D, 0xE2]);
    let counts = [
        near(&yellow, y),
        near(&yellow, p),
        near(&purple, p),
        near(&purple, y),
    ];
    assert!(counts[0] > 100 && counts[1] < 10, "{counts:?}");
    assert!(counts[2] > 100 && counts[3] < 10, "{counts:?}");
}

/// Bad options, includes and inherited views are warnings; the rest of the
/// theme reads as it would without them.
#[test]
fn bad_options_includes_and_bases_are_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("tema");
    std::fs::create_dir_all(folder.join("parts")).unwrap();
    std::fs::write(dir.path().join("fora.toml"), "text = \"#000000\"\n").unwrap();
    std::fs::write(folder.join("parts/a.toml"), "include = \"parts/b.toml\"\nbackground = \"#010203\"\n[vars]\ncor = \"#00FF00\"\ntinta = \"#00FF00\"\n").unwrap();
    std::fs::write(
        folder.join("parts/b.toml"),
        "include = [\"parts/a.toml\"]\nbackground = \"#0A0B0C\"\nborder = \"#112233\"\n",
    )
    .unwrap();
    std::fs::write(folder.join("parts/ruim.toml"), "não é toml =\n").unwrap();
    std::fs::write(
        folder.join("theme.toml"),
        r##"
include = ["parts/a.toml", "../fora.toml", "/etc/passwd", "parts/falta.toml", "parts/ruim.toml"]
background = "#202020"
accent = "$cor"
text = "$tinta"

[options.sem_escolhas]
name = "Nada"

[options.uma]
choices.so = { name = "Só" }

[options.ruim]
default = "nenhuma"
extra = 1
choices.a = { name = "A", vars = { cor = "#FF0000", lista = [1] } }
choices.b = { name = "B", outro = 2 }

[options."com espaço"]
choices.a = {}
choices.b = {}

[view.filho]
base = "nenhuma"

[view.ciclo]
base = "ciclo"

[view.neto]
base = "filho"

[view.neto.systems]
remove = ["nada", 3]

[[view.neto.systems.el]]
id = "x"
kind = "rect"
fill = "#FFFFFF"
"##,
    )
    .unwrap();
    let default = Theme::load(&shipped("default"));
    let mut theme = Theme::builtin();
    let mut said = warnings(|| theme = Theme::load_over(&folder, &default));
    for id in ["filho", "ciclo", "neto"] {
        said.extend(warnings(|| drop(theme.view(id))));
    }
    // The include under the theme.toml: its own colour loses, the one it
    // alone gives (through b.toml, which cannot include a.toml again) stays.
    assert_eq!(theme.background.r, 0x20);
    assert_eq!((theme.border.r, theme.border.g), (0x11, 0x22));
    assert_eq!(theme.text.g, 0xFF, "a.toml's var reaches the theme");
    // One option is left: "ruim", defaulting to its first choice.
    let options: Vec<&str> = theme.options().iter().map(|o| o.id.as_str()).collect();
    assert_eq!(options, ["ruim"]);
    assert_eq!(theme.choice("ruim"), Some("a"));
    assert_eq!(theme.highlight_bg.r, 0xFF, "the default choice's var");
    for word in [
        "includes itself again",
        "\"../fora.toml\"",
        "\"/etc/passwd\"",
        "falta.toml",
        "ruim.toml",
        "options.sem_escolhas needs a choices table",
        "options.uma needs at least two choices",
        "options.ruim.default names no choice",
        "options.ruim.extra",
        "options.ruim.choices.a.vars.lista",
        "options.ruim.choices.b.outro",
        "only letters, digits",
        "view.filho.base names no other view",
        "view.ciclo.base names no other view",
        "no inherited element \"nada\"",
    ] {
        assert!(
            said.iter().any(|w| w.contains(word)),
            "no warning about {word}: {said:#?}"
        );
    }
    // The views still draw: "neto" is "filho" (the classic list) and its rect.
    let neto = theme.view("neto");
    assert_eq!(neto.systems.scene.as_ref().unwrap().elements.len(), 1);
}

#[test]
fn animations_read_and_bad_ones_are_warnings() {
    use archr_ui::layout::{Anim, Ease};
    let source = r##"
[view.x]
[[view.x.systems.el]]
id = "a"
kind = "text"
text = "${system.name}"
enter = { from = "left", distance = 0.1, fade = false, scale = 0.5, duration = 0.4, delay = 0.1, ease = "in-out" }
select = { from = "bottom" }
[[view.x.systems.el]]
id = "b"
kind = "image"
src = "x.png"
enter = { from = "diagonal", ease = "bounce", duration = 2, delay = 0.5, speed = 3 }
select = 5
[[view.x.systems.el]]
id = "c"
kind = "rect"
enter = { fade = false }
[[view.x.systems.el]]
kind = "list"
"##;
    let table: toml::Table = toml::from_str(source).unwrap();
    let theme = Theme::parse(&table);
    let mut view = View::lista(&theme);
    let said = warnings(|| view = theme.view("x"));
    let scene = view.systems.scene.as_ref().unwrap();
    let by = |id: &str| scene.elements.iter().find(|e| e.id == id).unwrap();
    assert_eq!(
        by("a").enter,
        Some(Anim {
            from: (-0.1, 0.),
            fade: false,
            scale: 0.5,
            duration: 0.4,
            delay: 0.1,
            ease: Ease::InOut,
        })
    );
    // `select` takes the list's time, fades and comes a little way.
    let select = by("a").select.unwrap();
    assert_eq!((select.from, select.fade), ((0., 0.04), true));
    assert!((select.duration - 0.18).abs() < 1e-6);
    // Too long is cut to 0.6 s in all; the wrong words keep the defaults.
    let b = by("b").enter.unwrap();
    assert_eq!(
        (b.from, b.ease, b.duration, b.delay),
        ((0., 0.), Ease::Out, 0.6, 0.)
    );
    assert!(by("b").select.is_none());
    assert!(by("c").enter.is_none(), "nothing to animate");
    for word in [
        "from must be",
        "ease must be",
        "duration",
        "delay + duration",
        "speed",
        "select must be a table",
        "neither moves",
    ] {
        assert!(
            said.iter().any(|w| w.contains(word)),
            "no warning about {word}: {said:#?}"
        );
    }
}

/// The Albedo view's animations at three sizes, with Animations on,
/// reduced and off: a frame partway through the screen's entrance differs
/// from the settled one and the settled one is the screen without motion;
/// a step down plays `select`, a second step right after shows at once
/// (the d-pad held); coming back from the games or Settings plays the
/// entrance again. With ARCHR_SCENE_CAPTURE_DIR set the frames are saved
/// there; the time a frame takes while elements move is printed.
#[test]
fn elements_enter_and_follow_the_focus_by_the_animations_setting() {
    use archr_ui::motion::Motion;
    use std::time::Duration;
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_SCENE_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cena");
    std::fs::create_dir_all(&folder).unwrap();
    theme_folder(&folder);
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let data = library(&pictures);
    let ms = Duration::from_millis;
    let mut timings = Vec::new();
    let theme = load(&folder, "albedo");
    for size in SIZES {
        let theme = theme.fitted(size.0, size.1);
        let view = theme.view(&theme.active_view);
        for motion in [Motion::On, Motion::Reduced, Motion::Off] {
            let root = tempfile::tempdir().unwrap();
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.theme_dir = Some(folder.clone());
            ui.default_theme_dir = Some(shipped("default"));
            ui.set_motion(motion);
            ui.configure(&ttf, &creator, &theme, Some(root.path()))
                .unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            ui.catalog = archr_ui::i18n::I18n::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales"),
                Some("pt_BR"),
                None,
            );
            let mut app = App::new(10);
            app.set_view(&view);
            let unix = 1_794_916_800;
            let t0 = std::time::Instant::now();
            let tag = format!("{}-{}x{}", motion.code(), size.0, size.1);
            // Draws at `clock`, again until every picture asked for is in
            // (the clock stands still meanwhile); the pixels, and the time
            // the first frame took until the GPU had drawn it.
            let frame = |ui: &mut Ui, name: &str, app: &App, clock| {
                ui.pin_clock(clock, unix);
                let deadline = std::time::Instant::now() + Duration::from_secs(60);
                let mut round = 0;
                let mut took = None;
                loop {
                    ui.tick();
                    let began = std::time::Instant::now();
                    ui.draw_with_settings(app, &data, &theme, Default::default(), None)
                        .unwrap();
                    let one = sdl2::rect::Rect::new(0, 0, 1, 1);
                    ui.canvas
                        .read_pixels(one, sdl2::pixels::PixelFormatEnum::RGBA32)
                        .unwrap();
                    took.get_or_insert(began.elapsed());
                    if ui.pictures_idle() && round >= 3 {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "pictures never loaded"
                    );
                    round += 1;
                    std::thread::sleep(ms(5));
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                if let Some(out) = &out {
                    let file = out.join(format!("animacao-{tag}-{name}.png"));
                    image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                }
                (pixels, took.unwrap())
            };
            // Right of the list (which glides and scrolls long names on
            // its own): the pictures, names and text that move here.
            let right = |pixels: &[u8]| -> Vec<u8> {
                let (w, h) = (size.0 as usize, size.1 as usize);
                (h / 10..h * 9 / 10)
                    .flat_map(|y| {
                        let row = &pixels[(y * w + w * 45 / 100) * 4..(y * w + w) * 4];
                        row.to_vec()
                    })
                    .collect()
            };
            // The screen opens: partway in, then settled.
            let mid = if motion == Motion::Reduced { 60 } else { 150 };
            let (opening, _) = frame(&mut ui, "sistemas-inicio", &app, t0);
            let (partway, took) = frame(&mut ui, "sistemas-entrada", &app, t0 + ms(mid));
            let (settled, _) = frame(&mut ui, "sistemas", &app, t0 + ms(700));
            if motion == Motion::On {
                timings.push((format!("entrada-{tag}"), took));
            }
            match motion {
                Motion::Off => {
                    assert!(
                        right(&opening) == right(&settled),
                        "{tag}: off, nothing moves"
                    );
                    assert!(right(&partway) == right(&settled));
                }
                _ => assert!(
                    right(&partway) != right(&settled),
                    "{tag}: the entrance shows"
                ),
            }
            if motion == Motion::Reduced {
                // A short fade only: done by 0.13 s.
                let (done, _) = frame(&mut ui, "sistemas-reduzida", &app, t0 + ms(130));
                assert!(
                    right(&done) == right(&settled),
                    "{tag}: reduced is a short fade"
                );
            }
            // A step down, at rest: the system's picture comes in.
            let t1 = t0 + ms(2000);
            app.handle(Button::Down, &data);
            frame(&mut ui, "sistemas-troca-inicio", &app, t1);
            let (partway, took) = frame(&mut ui, "sistemas-troca", &app, t1 + ms(80));
            let (settled, _) = frame(&mut ui, "sistemas-2", &app, t1 + ms(700));
            if motion == Motion::On {
                timings.push((format!("troca-{tag}"), took));
            }
            assert_eq!(
                right(&partway) != right(&settled),
                motion != Motion::Off,
                "{tag}: select plays with motion"
            );
            // Held: the next step 80 ms later shows at once.
            let t2 = t1 + ms(2000);
            app.handle(Button::Down, &data);
            frame(&mut ui, "sistemas-seguro-1", &app, t2);
            app.handle(Button::Down, &data);
            let (quick, _) = frame(&mut ui, "sistemas-seguro-2", &app, t2 + ms(80));
            let (rest, _) = frame(&mut ui, "sistemas-4", &app, t2 + ms(700));
            assert!(
                right(&quick) == right(&rest),
                "{tag}: a held d-pad shows each item at once"
            );
            // The games, and back: the systems come in again.
            let t3 = t2 + ms(2000);
            // The Mega Drive, with nine games.
            app.cursor = 1;
            frame(&mut ui, "sistemas-megadrive", &app, t3 - ms(1000));
            app.handle(Button::A, &data);
            frame(&mut ui, "jogos-inicio", &app, t3);
            let (partway, took) = frame(&mut ui, "jogos-entrada", &app, t3 + ms(mid));
            let (settled, _) = frame(&mut ui, "jogos", &app, t3 + ms(1000));
            assert_eq!(
                right(&partway) != right(&settled),
                motion != Motion::Off,
                "{tag}: games enter"
            );
            // The next game: its picture, logo, stars and text come in.
            app.handle(Button::Down, &data);
            frame(&mut ui, "jogos-troca-inicio", &app, t3 + ms(1100));
            let (partway, took2) = frame(&mut ui, "jogos-troca", &app, t3 + ms(1180));
            let (settled, _) = frame(&mut ui, "jogos-2", &app, t3 + ms(1900));
            assert_eq!(
                right(&partway) != right(&settled),
                motion != Motion::Off,
                "{tag}: the next game comes in"
            );
            if motion == Motion::On {
                timings.push((format!("jogos-entrada-{tag}"), took));
                timings.push((format!("jogos-troca-{tag}"), took2));
                // Eight more games: frames while they come in against
                // frames at rest, each until the GPU has drawn it.
                let timed = |ui: &mut Ui, app: &App, clock| {
                    ui.pin_clock(clock, unix);
                    let began = std::time::Instant::now();
                    ui.draw_with_settings(app, &data, &theme, Default::default(), None)
                        .unwrap();
                    let one = sdl2::rect::Rect::new(0, 0, 1, 1);
                    ui.canvas
                        .read_pixels(one, sdl2::pixels::PixelFormatEnum::RGBA32)
                        .unwrap();
                    began.elapsed()
                };
                let (mut moving, mut still) = (Vec::new(), Vec::new());
                for k in 0..8u64 {
                    let at = t3 + ms(2100 + 400 * k);
                    app.handle(Button::Down, &data);
                    frame(&mut ui, "jogos-medida", &app, at);
                    moving.push(timed(&mut ui, &app, at + ms(60)));
                    moving.push(timed(&mut ui, &app, at + ms(120)));
                    still.push(timed(&mut ui, &app, at + ms(350)));
                    still.push(timed(&mut ui, &app, at + ms(360)));
                }
                let median = |v: &mut Vec<Duration>| {
                    v.sort();
                    v[v.len() / 2]
                };
                timings.push((format!("jogos-troca-mediana-{tag}"), median(&mut moving)));
                timings.push((format!("jogos-parado-mediana-{tag}"), median(&mut still)));
                app.cursor = 0;
            }
            let t4 = t3 + ms(2000);
            app.handle(Button::B, &data);
            frame(&mut ui, "volta-inicio", &app, t4);
            let (partway, _) = frame(&mut ui, "volta-entrada", &app, t4 + ms(mid));
            let (settled, _) = frame(&mut ui, "volta", &app, t4 + ms(1000));
            assert_eq!(
                right(&partway) != right(&settled),
                motion != Motion::Off,
                "{tag}: back, again"
            );
            // Settings over it, then the screen again: it comes in anew.
            let t5 = t4 + ms(2000);
            let settings = archr_ui::settings::Settings::default();
            ui.pin_clock(t5, unix);
            ui.draw_with_settings(&app, &data, &theme, Default::default(), Some(&settings))
                .unwrap();
            frame(&mut ui, "volta-config-inicio", &app, t5 + ms(1));
            let (partway, _) = frame(&mut ui, "volta-config", &app, t5 + ms(mid));
            let (settled, _) = frame(&mut ui, "volta-config-fim", &app, t5 + ms(1000));
            assert_eq!(
                right(&partway) != right(&settled),
                motion != Motion::Off,
                "{tag}: after Settings"
            );
            // A press finishes every motion at once.
            let t6 = t5 + ms(2000);
            app.handle(Button::B, &data);
            app.handle(Button::A, &data);
            frame(&mut ui, "jogos-de-novo", &app, t6);
            ui.snap_animations();
            let (snapped, _) = frame(&mut ui, "jogos-pronto", &app, t6 + ms(10));
            let (settled, _) = frame(&mut ui, "jogos-fim", &app, t6 + ms(1000));
            assert!(
                right(&snapped) == right(&settled),
                "{tag}: a press snaps the entrance"
            );
        }
    }
    for (name, took) in &timings {
        eprintln!("frame {name}: {took:?}");
    }
}

/// The details screen as a free screen: Solo's own elements, Albedo's
/// inherited from its games screen less the list, and "solo-escuro"
/// inheriting the Solo's with the details' buttons; a bad value is a
/// warning and the rest still reads.
#[test]
fn details_screens_read_inherit_and_warn() {
    let dir = tempfile::tempdir().unwrap();
    theme_folder(dir.path());
    let theme = load(dir.path(), "solo");
    let ids = |view: &View| -> Vec<String> {
        let scene = view.details.as_ref().expect("a free details screen");
        scene.elements.iter().map(|e| e.id.clone()).collect()
    };
    for (w, h) in SIZES {
        let fitted = theme.fitted(w, h);
        for id in ["solo", "albedo", "solo-escuro"] {
            let mut view = View::lista(&fitted);
            let said = warnings(|| view = fitted.view(id));
            assert!(said.is_empty(), "{id} {w}x{h}: {said:#?}");
        }
        let solo = fitted.view("solo");
        let found = ids(&solo);
        for id in [
            "arte",
            "logo-sistema",
            "capa",
            "ficha",
            "video",
            "descricao",
            "dicas",
        ] {
            assert!(found.iter().any(|f| f == id), "{id}: {found:?}");
        }
        let albedo = fitted.view("albedo");
        let found = ids(&albedo);
        // The games screen's elements, less the list and the position.
        assert!(!found.iter().any(|f| f == "lista" || f == "posicao"));
        for id in [
            "fundo",
            "moldura",
            "titulo",
            "captura",
            "video",
            "logo-jogo",
            "ficha",
        ] {
            assert!(found.iter().any(|f| f == id), "{id}: {found:?}");
        }
        let scene = albedo.details.as_ref().unwrap();
        let element = |id: &str| scene.elements.iter().find(|e| e.id == id).unwrap();
        // The cover moved, its other fields kept; the description scrolls.
        let capa = element("capa");
        assert!((capa.rect.x - 0.03).abs() < 1e-4 && (capa.rect.y - 0.175).abs() < 1e-4);
        assert!(matches!(&capa.kind, Kind::Image(_)));
        assert!(matches!(&element("descricao").kind, Kind::Text(t) if t.scroll));
        assert!(matches!(&element("status").kind, Kind::Status(_)));
        // The games screen keeps its list and its own cover.
        let games = albedo.games.scene.as_ref().unwrap();
        assert!(games.elements.iter().any(|e| e.id == "lista"));
        let cover = games.elements.iter().find(|e| e.id == "capa").unwrap();
        assert!((cover.rect.x - 0.48).abs() < 1e-4);
        // Inherited with the view, with the buttons added.
        let dark = fitted.view("solo-escuro");
        let scene = dark.details.as_ref().unwrap();
        assert!(scene
            .elements
            .iter()
            .any(|e| matches!(e.kind, Kind::Actions)));
        let text = scene.elements.iter().find(|e| e.id == "descricao").unwrap();
        assert!(
            (text.rect.h - 0.14).abs() < 1e-4 && matches!(&text.kind, Kind::Text(t) if t.scroll)
        );
    }
    // Views without `details` keep the details of the `details_*` keys.
    assert!(theme.view("carrossel").details.is_none());
    assert!(theme.view("lista").details.is_none());

    let source = r##"
[[view.v.systems.el]]
kind = "list"

[[view.v.systems.el]]
kind = "actions"

[[view.v.games.el]]
id = "texto"
kind = "text"
text = "${game.description}"
wrap = true
long_text = "scroll"

[view.v.details]
base = "systems"
remove = ["nada"]
foo = 1

[[view.v.details.el]]
kind = "list"

[[view.v.details.el]]
kind = "text"
text = "${game.description}"
wrap = true
long_text = "scroll"

[[view.v.details.el]]
kind = "text"
text = "${game.name}"
wrap = true
long_text = "scroll"

[[view.v.details.el]]
kind = "text"
text = "${game.genre}"
long_text = "rolar"

[[view.v.details.el]]
kind = "text"
text = "${game.year}"
long_text = "scroll"

[[view.v.details.el]]
kind = "actions"
size = 2

[view.w.details]
base = "games"

[[view.w.details.el]]
kind = "rating"
"##;
    let table: toml::Table = toml::from_str(source).unwrap();
    let theme = Theme::parse(&table);
    let mut view = View::lista(&theme);
    let said = warnings(|| view = theme.view("v"));
    let expect = [
        "actions belong to the details screen",
        "only the details screen scrolls text",
        "details.foo",
        "base must be \"games\"",
        "no inherited element \"nada\"",
        "details screen has no list",
        "one text scrolls per screen",
        "long_text",
        "only wrapped text",
        "details.el[5].size",
    ];
    for words in expect {
        assert!(said.iter().any(|w| w.contains(words)), "{words}: {said:#?}");
    }
    // What was right still reads: the scrolling text and the buttons.
    let scene = view.details.as_ref().expect("the rest of the details");
    let scrolls = scene
        .elements
        .iter()
        .filter(|e| matches!(&e.kind, Kind::Text(t) if t.scroll))
        .count();
    assert_eq!(scrolls, 1);
    assert!(scene
        .elements
        .iter()
        .any(|e| matches!(e.kind, Kind::Actions)));
    // A games screen without elements: only the details' own.
    let said = warnings(|| view = theme.view("w"));
    assert!(
        said.iter()
            .any(|w| w.contains("games screen has no elements")),
        "{said:#?}"
    );
    assert_eq!(view.details.as_ref().unwrap().elements.len(), 1);
}

/// The details screens drawn at three sizes: the game's pictures, facts
/// and text where the theme put them, a long description scrolled with ↓,
/// the buttons of the details with their focus, and the classic details
/// untouched in a view without them.
#[test]
fn details_screens_draw_and_scroll_at_every_size() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let out = std::env::var_os("ARCHR_SCENE_CAPTURE_DIR").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cena");
    std::fs::create_dir_all(&folder).unwrap();
    theme_folder(&folder);
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let mut data = library(&pictures);
    // Streets of Rage 2: a description far longer than any box.
    let long = (1..=24)
        .map(|n| format!("Parágrafo {n} da história, com socos, chutes e fases na cidade."))
        .collect::<Vec<_>>()
        .join(" ");
    let rage = &mut data.games.get_mut("megadrive").unwrap()[8];
    rage.details.description = long;
    rage.details.genre = "Briga de rua".into();
    rage.last_played = Some(1_794_916_800 - 3 * 86_400);
    let accent = [0xF9, 0xC4, 0x40];
    for view_id in ["solo", "albedo", "solo-escuro", "lista"] {
        let theme = load(&folder, view_id);
        for size in SIZES {
            let theme = theme.fitted(size.0, size.1);
            let view = theme.view(&theme.active_view);
            let root = tempfile::tempdir().unwrap();
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.theme_dir = Some(folder.clone());
            ui.default_theme_dir = Some(shipped("default"));
            ui.set_motion(archr_ui::motion::Motion::Off);
            ui.configure(&ttf, &creator, &theme, Some(root.path()))
                .unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            ui.catalog = archr_ui::i18n::I18n::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales"),
                Some("pt_BR"),
                None,
            );
            ui.pin_clock(std::time::Instant::now(), 1_794_916_800);
            let mut app = App::new(10);
            app.set_view(&view);
            assert_eq!(app.details_free(), view_id != "lista");
            let frame = |ui: &mut Ui, app: &mut App, name: &str| -> Vec<u8> {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
                let mut round = 0;
                loop {
                    ui.tick();
                    ui.snap_animations();
                    let art = archr_ui::render::Art::default();
                    ui.draw_with_settings(app, &data, &theme, art, None)
                        .unwrap();
                    // As the host does after each frame.
                    app.details_overflow = ui.details_overflow;
                    if ui.pictures_idle() && round >= 3 {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "pictures never loaded"
                    );
                    round += 1;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                if let Some(out) = &out {
                    let file = out.join(format!("{view_id}-{name}-{}x{}.png", size.0, size.1));
                    image::save_buffer(file, &pixels, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                }
                pixels
            };
            let share = |pixels: &[u8], (x0, y0, x1, y1): (f32, f32, f32, f32), rgb: [u8; 3]| {
                let (mut hit, mut all) = (0, 0);
                for y in (y0 * size.1 as f32) as usize..(y1 * size.1 as f32) as usize {
                    for x in (x0 * size.0 as f32) as usize..(x1 * size.0 as f32) as usize {
                        let at = (y * size.0 as usize + x) * 4;
                        all += 1;
                        if (0..3).all(|k| pixels[at + k].abs_diff(rgb[k]) < 12) {
                            hit += 1;
                        }
                    }
                }
                hit as f32 / all.max(1) as f32
            };
            // How many pixels of a region differ between two frames.
            let changed = |a: &[u8], b: &[u8], (x0, y0, x1, y1): (f32, f32, f32, f32)| {
                let (mut hit, mut all) = (0, 0);
                for y in (y0 * size.1 as f32) as usize..(y1 * size.1 as f32) as usize {
                    for x in (x0 * size.0 as f32) as usize..(x1 * size.0 as f32) as usize {
                        let at = (y * size.0 as usize + x) * 4;
                        all += 1;
                        if (0..3).any(|k| a[at + k].abs_diff(b[k + at]) > 24) {
                            hit += 1;
                        }
                    }
                }
                hit as f32 / all.max(1) as f32
            };
            let details = |game: usize| Screen::Details {
                system: "megadrive".into(),
                game,
            };
            // Sonic: no logo (its name instead), four and a half stars.
            app.screen = details(7);
            let sonic = frame(&mut ui, &mut app, "detalhes");
            // Aladdin: its logo, no rating.
            app.screen = details(0);
            frame(&mut ui, &mut app, "detalhes-logo");
            // Streets of Rage 2: ↓ scrolls its description, never past the end.
            app.screen = details(8);
            app.details_scroll = 0;
            let top = frame(&mut ui, &mut app, "detalhes-longo");
            if view_id == "lista" {
                // The classic details scroll the same way.
                assert!(app.details_overflow > 0);
                continue;
            }
            let overflow = app.details_overflow;
            assert!(overflow > 0, "{view_id} {size:?}");
            app.handle(Button::Down, &data);
            app.handle(Button::Down, &data);
            assert_eq!(app.details_scroll, 2);
            let scrolled = frame(&mut ui, &mut app, "detalhes-rolado");
            for _ in 0..overflow + 3 {
                app.handle(Button::Down, &data);
            }
            assert_eq!(app.details_scroll, overflow);
            let end = frame(&mut ui, &mut app, "detalhes-fim");
            let (text, still) = if view_id == "albedo" {
                ((0.4667, 0.77, 0.9667, 0.92), (0.03, 0.18, 0.40, 0.6))
            } else {
                ((0.52, 0.70, 0.96, 0.82), (0.04, 0.18, 0.46, 0.54))
            };
            assert!(changed(&top, &scrolled, text) > 0.01, "{view_id} {size:?}");
            assert!(changed(&scrolled, &end, text) > 0.01, "{view_id} {size:?}");
            // Nothing else moves: the cover stays.
            assert!(changed(&top, &end, still) < 0.001, "{view_id} {size:?}");
            app.handle(Button::Up, &data);
            assert_eq!(app.details_scroll, overflow - 1);
            match view_id {
                "solo" => {
                    // The stars on the facts' fourth line (no genre).
                    let stars = share(&sonic, (0.2, 0.735, 0.46, 0.785), [255, 255, 255]);
                    assert!(stars > 0.03, "{stars}");
                }
                "albedo" => {
                    // No list: no row in the accent on the left.
                    assert!(share(&sonic, (0.005, 0.15, 0.028, 0.925), accent) < 0.01);
                    // The theme's star, in the accent, inherited.
                    assert!(share(&sonic, (0.7667, 0.65, 0.9667, 0.675), accent) > 0.1);
                    // From the games screen, the details come in (`enter`).
                    ui.set_motion(archr_ui::motion::Motion::On);
                    app.screen = Screen::Games {
                        system: "megadrive".into(),
                        folder: String::new(),
                    };
                    let art = archr_ui::render::Art::default();
                    ui.tick();
                    ui.draw_with_settings(&app, &data, &theme, art, None)
                        .unwrap();
                    ui.snap_animations();
                    ui.tick();
                    ui.draw_with_settings(&app, &data, &theme, art, None)
                        .unwrap();
                    assert!(!ui.animating());
                    app.screen = details(7);
                    ui.tick();
                    ui.draw_with_settings(&app, &data, &theme, art, None)
                        .unwrap();
                    assert!(ui.animating());
                }
                _ => {
                    // The buttons, Play lit in the accent; R1 lights Favorite.
                    let row = (0.52, 0.845, 0.96, 0.915);
                    let play = share(&sonic, row, accent);
                    assert!(play > 0.05, "{play}");
                    app.screen = details(7);
                    app.details_scroll = 0;
                    app.handle(Button::R, &data);
                    assert_eq!(app.details_focus, 1);
                    let favorite = frame(&mut ui, &mut app, "detalhes-favorito");
                    assert!(changed(&sonic, &favorite, row) > 0.05);
                    app.details_focus = 0;
                }
            }
        }
    }
}

/// A tiled frame over the screen (Albedo's scanlines: a 4x4 pattern) is
/// laid once in a texture and drawn in one copy; the pattern comes out the
/// same, tile after tile, with its transparent pixels still transparent.
#[test]
fn a_tiled_frame_keeps_its_pattern_in_one_copy() {
    let _sdl = SDL.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("xadrez");
    std::fs::create_dir_all(&folder).unwrap();
    // White and see-through in a 2x2 checker, over a red screen.
    let mut checker = image::RgbaImage::new(2, 2);
    for (x, y, p) in checker.enumerate_pixels_mut() {
        *p = if (x + y) % 2 == 0 {
            image::Rgba([255, 255, 255, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    checker.save(folder.join("xadrez.png")).unwrap();
    std::fs::write(
        folder.join("theme.toml"),
        "name = \"Xadrez\"\nbackground = \"#FF0000\"\n\
         [[view.xadrez.systems.el]]\nid = \"fill\"\nkind = \"fill\"\ncolor = \"#FF0000\"\n\
         rect = { h = 1.0, w = 1.0, x = 0.0, y = 0.0 }\nz = 0\n\
         [[view.xadrez.systems.el]]\nid = \"xadrez\"\nkind = \"frame\"\nimage = \"xadrez.png\"\n\
         rect = { h = 1.0, w = 1.0, x = 0.0, y = 0.0 }\nslice = 0\ntile = true\nz = 1\n",
    )
    .unwrap();
    let theme = load(&folder, "xadrez").fitted(640, 480);
    let view = theme.view("xadrez");
    let pictures = dir.path().join("pictures");
    std::fs::create_dir_all(&pictures).unwrap();
    let data = library(&pictures);
    let root = tempfile::tempdir().unwrap();
    let (ttf, creator);
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    ui.theme_dir = Some(folder.clone());
    ui.default_theme_dir = Some(shipped("default"));
    ui.set_motion(archr_ui::motion::Motion::Off);
    ui.configure(&ttf, &creator, &theme, Some(root.path()))
        .unwrap();
    let mut app = App::new(10);
    app.set_view(&view);
    app.screen = Screen::Systems;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let pixels = loop {
        ui.tick();
        ui.draw_with_settings(&app, &data, &theme, archr_ui::render::Art::default(), None)
            .unwrap();
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        // Twice: the second frame draws from the kept texture.
        if ui.pictures_idle()
            && pixels[(300 * 640 + 300) * 4] == 255
            && pixels[(300 * 640 + 300) * 4 + 1] == 255
        {
            ui.draw_with_settings(&app, &data, &theme, archr_ui::render::Art::default(), None)
                .unwrap();
            break ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
        }
        assert!(std::time::Instant::now() < deadline, "the frame never drew");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let px = |x: usize, y: usize| {
        let at = (y * 640 + x) * 4;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    // The checker's scale on screen: a 2x2 picture, k pixels per pixel.
    let k = (1..=8).find(|&k| px(300, 300) != px(300 + k, 300)).unwrap();
    for (x, y) in [(300, 300), (20, 40), (600, 420), (k * 5, k * 7)] {
        let (x, y) = (x - x % (2 * k), y - y % (2 * k));
        assert_eq!(px(x, y), [255, 255, 255], "white at {x},{y}");
        assert_eq!(px(x + k, y), [255, 0, 0], "see-through at {},{y}", x + k);
        assert_eq!(px(x, y + k), [255, 0, 0], "see-through at {x},{}", y + k);
        assert_eq!(
            px(x + k, y + k),
            [255, 255, 255],
            "white at {},{}",
            x + k,
            y + k
        );
    }
}
