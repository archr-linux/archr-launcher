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
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/themes/cena/theme.toml");
    std::fs::copy(source, dir.join("theme.toml")).unwrap();
    for sub in [
        "arts",
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
        console(&dir.join(format!("systems/{system}.png")), hue);
    }
    art(&dir.join("arts/default.png"), (960, 720), [80, 80, 90]);
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
        said.retain(|w| !w.contains("outside the theme directory"));
        assert!(said.is_empty(), "{block}\n{said:#?}");
    }
    assert!(blocks >= 4, "{blocks} examples");
}

/// The test theme drawn at three sizes: both views, systems and games, a
/// system with its own element and a game without a logo; every frame shows
/// its art, the list in its place and the colors the theme gave it.
#[test]
fn free_screens_draw_at_every_size() {
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
