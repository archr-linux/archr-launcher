//! The launcher at 640x480 (4:3), 720x720 (1:1, R36 Plus) and 480x320
//! (3:2, a portrait panel drawn turned): every shipped theme's layout fits
//! each screen, `[aspect.*]` tables apply to their shape only, and 640x480
//! still draws the frames it always did (tests/golden/640x480).
mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    layout::{Layout, View},
    render::Ui,
    settings::{Page, Settings},
    state::{App, Button, Screen},
    Aspect, Rect01, Theme,
};
use common::FakeData;
use std::path::{Path, PathBuf};

const SIZES: [(u32, u32); 3] = [(640, 480), (720, 720), (480, 320)];

fn themes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
}
/// Every theme in themes/, over the shipped default.
fn shipped_themes() -> Vec<(String, Theme)> {
    let default = Theme::load(&themes_dir().join("default"));
    let mut themes: Vec<_> = std::fs::read_dir(themes_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|dir| dir.join("theme.toml").is_file())
        .map(|dir| {
            let name = dir.file_name().unwrap().to_string_lossy().into_owned();
            let theme = if name == "default" {
                default.clone()
            } else {
                Theme::load_over(&dir, &default)
            };
            (name, theme)
        })
        .collect();
    themes.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(themes.len() >= 2, "default and praia at least");
    themes
}

/// Screen pixels of a fraction rectangle, as the renderer rounds them.
fn px(rect: Rect01, (w, h): (u32, u32)) -> (i32, i32, i32, i32) {
    let r = archr_ui::render::pixels(rect, w, h);
    (r.x(), r.y(), r.right(), r.bottom())
}
fn inside(rect: Rect01, size: (u32, u32), what: &str) {
    let (left, top, right, bottom) = px(rect, size);
    assert!(
        left >= 0 && top >= 0 && right <= size.0 as i32 && bottom <= size.1 as i32,
        "{what} {rect:?} leaves the {size:?} screen"
    );
}
fn apart(a: Rect01, b: Rect01, size: (u32, u32), what: &str) {
    let (al, at, ar, ab) = px(a, size);
    let (bl, bt, br, bb) = px(b, size);
    assert!(
        ar <= bl || br <= al || ab <= bt || bb <= at,
        "{what} overlap at {size:?}: {a:?} and {b:?}"
    );
}
/// The list, picture, panel, text and dots of one screen: all on screen,
/// none of them over the list, and the list under the header.
fn screen_fits(theme: &Theme, layout: &Layout, size: (u32, u32), what: &str) {
    let list = layout.list.rect;
    inside(list, size, &format!("{what} list"));
    let header = theme.header_height * 480. * archr_ui::render::scale(size.0, size.1);
    let (_, top, _, _) = px(list, size);
    assert!(
        top as f32 >= header,
        "{what} list under the header at {size:?}"
    );
    for (name, rect) in [
        ("panel", layout.panel.map(|p| p.rect)),
        ("art", layout.art.map(|a| a.rect)),
        ("info", layout.info.map(|i| i.rect)),
        ("meta", layout.meta.map(|m| m.rect)),
        ("dots", layout.dots),
    ] {
        let Some(rect) = rect else { continue };
        inside(rect, size, &format!("{what} {name}"));
        apart(list, rect, size, &format!("{what} list and {name}"));
    }
    if let (Some(art), Some(meta)) = (layout.art, layout.meta) {
        apart(art.rect, meta.rect, size, &format!("{what} art and meta"));
    }
}

#[test]
fn every_shipped_theme_and_view_fits_every_screen() {
    for (name, theme) in shipped_themes() {
        for size in SIZES {
            let fitted = theme.fitted(size.0, size.1);
            for (id, _) in fitted.views() {
                let view: View = fitted.view(&id);
                for (screen, layout) in [("systems", &view.systems), ("games", &view.games)] {
                    screen_fits(&fitted, layout, size, &format!("{name} {id} {screen}"));
                }
            }
            let details = [
                ("shot", fitted.details_shot),
                ("info", fitted.details_info),
                ("title", fitted.details_title),
                ("subtitle", fitted.details_subtitle),
                ("text", fitted.details_text),
                ("actions", fitted.details_actions),
            ];
            for (n, (a, first)) in details.iter().enumerate() {
                inside(*first, size, &format!("{name} details {a}"));
                for (b, second) in &details[n + 1..] {
                    apart(*first, *second, size, &format!("{name} details {a}/{b}"));
                }
            }
            for (what, rect) in [
                ("switcher_card", fitted.switcher_card),
                ("switcher_info", fitted.switcher_info),
                ("resume_shot", fitted.resume_shot),
                ("resume_info", fitted.resume_info),
                ("saves_list", fitted.saves_list),
                ("saves_card", fitted.saves_card),
                ("shots_grid", fitted.shots_grid),
                ("achievements_list", fitted.achievements_list),
                ("start_widgets", fitted.start_widgets),
            ] {
                inside(rect, size, &format!("{name} {what}"));
            }
            apart(fitted.switcher_card, fitted.switcher_info, size, "switcher");
            apart(fitted.resume_shot, fitted.resume_info, size, "resume");
            apart(fitted.saves_list, fitted.saves_card, size, "saves");
        }
    }
}

#[test]
fn the_switcher_and_resume_pictures_stay_four_by_three_on_every_screen() {
    let theme = Theme::load(&themes_dir().join("default"));
    for size in SIZES {
        let fitted = theme.fitted(size.0, size.1);
        for rect in [fitted.switcher_card, fitted.resume_shot] {
            let (l, t, r, b) = px(rect, size);
            let ratio = (r - l) as f32 / (b - t) as f32;
            assert!(
                (1.3..=1.6).contains(&ratio),
                "{rect:?} is {ratio} wide at {size:?}"
            );
        }
    }
}

#[test]
fn aspect_tables_apply_to_their_shape_over_their_own_file() {
    assert_eq!(Aspect::of(720, 720), Aspect::Square);
    assert_eq!(Aspect::of(640, 480), Aspect::Classic);
    assert_eq!(Aspect::of(480, 320), Aspect::Wide);
    assert_eq!(Aspect::of(1280, 720), Aspect::Wide);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("theme.toml"),
        "margin = 0.05\n\
         switcher_card = { x = 0.1, y = 0.1, w = 0.5, h = 0.5 }\n\
         [view.lista.games]\nlist = { x = 0.4, w = 0.5 }\n\
         [aspect.square]\nmargin = 0.02\n\
         [aspect.square.view.lista.games]\nlist = { w = 0.3 }\n\
         [aspect.wide]\nfont_size = 0.03\n",
    )
    .unwrap();
    let theme = Theme::load(dir.path());
    let builtin = Theme::builtin();
    // 4:3: the file's own values; no aspect table applies.
    let classic = theme.fitted(640, 480);
    assert_eq!(classic, theme);
    assert_eq!(classic.margin, 0.05);
    // Square: its table over the file, key by key; views merge field by field.
    let square = theme.fitted(720, 720);
    assert_eq!(square.margin, 0.02);
    assert_eq!(square.font_size, builtin.font_size);
    let list = square.view("lista").games.list.rect;
    assert_eq!((list.x, list.w), (0.4, 0.3));
    // The file's own switcher wins over the default's square one.
    assert_eq!(square.switcher_card.w, 0.5);
    // Wide: only its key changes.
    let wide = theme.fitted(480, 320);
    assert_eq!((wide.font_size, wide.margin), (0.03, 0.05));
    // The default's square table reaches a theme that says nothing.
    let plain = Theme::load(&themes_dir().join("default")).fitted(720, 720);
    assert_eq!(plain.switcher_card.h, 0.46875);
    // Fitting again for the same screen changes nothing.
    assert_eq!(square.fitted(720, 720), square);
}

#[test]
fn a_square_screen_gives_lists_more_rows_not_bigger_ones() {
    let theme = Theme::load(&themes_dir().join("default"));
    let classic = theme.view("lista");
    let square = theme.fitted(720, 720).view("lista");
    // Same row size (a fraction of the 480 px reference), a taller list.
    assert_eq!(square.games.list.row, classic.games.list.row);
    let rows = |view: &View, (width, height): (u32, u32)| {
        let list = &view.games.list;
        let scale = archr_ui::render::scale(width, height);
        (list.rect.h * height as f32 / (list.row * 480. * scale)).floor()
    };
    assert_eq!(rows(&classic, (640, 480)), 10.);
    assert!(rows(&square, (720, 720)) >= 13.);
}

/// A small library with art-free systems, so frames are deterministic.
fn library() -> FakeData {
    let systems = [
        ("snes", "Super Nintendo", 9),
        ("gba", "Game Boy Advance", 3),
    ]
    .map(|(name, fullname, game_count)| SystemEntry {
        name: name.into(),
        fullname: fullname.into(),
        game_count,
        dir_mtime: 0,
        source_fingerprint: 0,
    })
    .to_vec();
    let games = [
        "Chrono Trigger",
        "Donkey Kong Country",
        "Mega Man X",
        "The Legend of Zelda: A Link to the Past",
    ]
    .iter()
    .map(|name| Game {
        path: format!("{name}.sfc").into(),
        name: (*name).into(),
        image: None,
        video: None,
        favorite: false,
        last_played: None,
        year: Some(1993),
        players: Some("1-2".into()),
        details: archr_index::Details {
            description: "Ataque os oito Mavericks e enfrente Sigma. ".repeat(6),
            genre: "Platform".into(),
            developer: "Capcom".into(),
            publisher: "Capcom".into(),
            rating: Some(80),
            ..Default::default()
        },
    })
    .collect();
    FakeData {
        systems,
        games: [("snes".into(), games)].into(),
        ..FakeData::default()
    }
}

/// Systems, games, details and two settings pages of `theme` at `size`.
fn frames(theme: &Theme, size: (u32, u32)) -> Vec<(&'static str, Vec<u8>)> {
    let root = tempfile::tempdir().unwrap();
    let ttf;
    let creator;
    let mut ui = Ui::new(size.0, size.1, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    // No console pictures and no status files: the same frame on any host.
    ui.theme_dir = Some(root.path().to_path_buf());
    ui.default_theme_dir = Some(themes_dir().join("default"));
    ui.set_motion(archr_ui::motion::Motion::Off);
    let theme = theme.fitted(size.0, size.1);
    ui.configure(&ttf, &creator, &theme, Some(root.path()))
        .unwrap();
    let data = library();
    let mut app = App::new(10);
    app.set_view(&theme.view(&theme.active_view));
    let mut shots = Vec::new();
    let mut shot = |ui: &mut Ui, name, app: &App, settings: Option<&Settings>| {
        for _ in 0..2 {
            ui.snap_animations();
            ui.draw_with_settings(app, &data, &theme, Default::default(), settings)
                .unwrap();
        }
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        shots.push((name, pixels));
    };
    shot(&mut ui, "systems", &app, None);
    app.handle(Button::A, &data);
    app.cursor = 2;
    shot(&mut ui, "games", &app, None);
    let games = std::mem::replace(
        &mut app.screen,
        Screen::Details {
            system: "snes".into(),
            game: 2,
        },
    );
    shot(&mut ui, "details", &app, None);
    app.screen = games;
    let mut settings = Settings::default();
    settings.loading = false;
    shot(&mut ui, "settings", &app, Some(&settings));
    settings.open(Page::Sound);
    shot(&mut ui, "settings-sound", &app, Some(&settings));
    shots
}

/// Pixels that differ by more than a hint of antialiasing, outside the
/// clock in the header's right corner.
fn changed(a: &[u8], b: &[u8], width: usize) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .enumerate()
        .filter(|(n, _)| !(n % width >= 520 && n / width < 40))
        .filter(|(_, (p, q))| p.iter().zip(*q).any(|(x, y)| x.abs_diff(*y) > 24))
        .count()
}

#[test]
fn every_screen_draws_at_every_size_and_640x480_matches_the_golden_frames() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    std::env::set_var("TZ", "UTC");
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/640x480");
    let update = std::env::var_os("ARCHR_GOLDEN_UPDATE").is_some();
    let default = Theme::load(&themes_dir().join("default"));
    for size in SIZES {
        let shots = frames(&default, size);
        for (name, pixels) in &shots {
            assert_eq!(pixels.len(), (size.0 * size.1 * 4) as usize, "{name}");
            // Something besides the background was drawn.
            let first = &pixels[..4];
            assert!(
                pixels.chunks_exact(4).filter(|p| p != &first).count() > 1000,
                "{name} at {size:?} is empty"
            );
        }
        if size != (640, 480) {
            continue;
        }
        for (name, pixels) in shots {
            let path = golden.join(format!("{name}.png"));
            if update {
                std::fs::create_dir_all(&golden).unwrap();
                image::save_buffer(&path, &pixels, 640, 480, image::ColorType::Rgba8).unwrap();
                continue;
            }
            let expected = image::open(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .to_rgba8();
            let off = changed(expected.as_raw(), &pixels, 640);
            // Font hinting may move a few edge pixels between FreeType
            // versions; a moved box or line moves thousands.
            assert!(
                off <= 640 * 480 / 500,
                "{name} at 640x480 changed in {off} pixels"
            );
        }
    }
}

/// Twenty systems, the first with forty games, all without art.
fn long_library() -> FakeData {
    let systems: Vec<SystemEntry> = (0..20)
        .map(|n| SystemEntry {
            name: format!("s{n:02}"),
            fullname: format!("System {n:02}"),
            game_count: 40,
            dir_mtime: 0,
            source_fingerprint: 0,
        })
        .collect();
    let games = (0..40)
        .map(|n| Game {
            path: format!("g{n:02}.rom").into(),
            name: format!("Game {n:02}"),
            image: None,
            video: None,
            favorite: false,
            last_played: None,
            year: None,
            players: None,
            details: Default::default(),
        })
        .collect();
    FakeData {
        systems,
        games: [("s00".into(), games)].into(),
        ..FakeData::default()
    }
}

/// Top of the selection pill in a fresh frame with `cursor` selected: the
/// first accent pixel down the middle of the list.
fn pill_top(theme: &Theme, size: (u32, u32), games: bool, cursor: usize) -> i32 {
    let ttf;
    let creator;
    let mut ui = Ui::new(size.0, size.1, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let root = tempfile::tempdir().unwrap();
    ui.theme_dir = Some(root.path().to_path_buf());
    ui.set_motion(archr_ui::motion::Motion::Off);
    ui.configure(&ttf, &creator, theme, Some(root.path()))
        .unwrap();
    let data = long_library();
    let view = theme.view(&theme.active_view);
    let mut app = App::new(10);
    app.set_view(&view);
    if games {
        app.handle(Button::A, &data);
    }
    app.cursor = cursor;
    ui.draw(&app, &data, theme, None).unwrap();
    let pixels = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    let list = archr_ui::render::pixels(view.screen(games).list.rect, size.0, size.1);
    let x = (list.x() + list.width() as i32 / 3) as u32;
    let accent = theme.highlight_bg;
    (list.y()..list.bottom())
        .find(|&y| {
            let at = ((y as u32 * size.0 + x) * 4) as usize;
            pixels[at..at + 3] == [accent.r, accent.g, accent.b]
        })
        .expect("the pill is on screen")
}

#[test]
fn l_and_r_page_by_the_rows_each_list_draws_at_every_size() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    for (name, theme) in shipped_themes() {
        for size in SIZES {
            for (id, _) in theme.views() {
                let mut theme = theme.fitted(size.0, size.1);
                theme.active_view = id.clone();
                let view = theme.view(&id);
                let mut app = App::new(10);
                app.set_view(&view);
                for (games, layout, nav) in [
                    (false, &view.systems, app.systems_nav),
                    (true, &view.games, app.games_nav),
                ] {
                    let list = &layout.list;
                    if size == (640, 480) {
                        // The pages 640x480 always had.
                        let before = (list.rect.h / (list.row + list.gap)).floor() as usize;
                        assert_eq!(nav.page, before.max(1), "{name} {id} games={games}");
                    }
                    if list.axis != archr_ui::layout::Axis::Vertical
                        || list.scroll != archr_ui::layout::Scroll::Edge
                    {
                        continue;
                    }
                    // The last row of a page is drawn where it belongs; the
                    // first of the next one makes the list scroll.
                    let top = archr_ui::render::pixels(list.rect, size.0, size.1).y();
                    let scale = archr_ui::render::scale(size.0, size.1);
                    let pitch = ((list.row * 480. * scale).round()
                        + (list.gap * 480. * scale).round()) as i32;
                    let what = format!("{name} {id} games={games} at {size:?}, page {}", nav.page);
                    let last = nav.page - 1;
                    assert_eq!(
                        pill_top(&theme, size, games, last),
                        top + last as i32 * pitch,
                        "{what}: row {last} is not drawn whole"
                    );
                    assert!(
                        pill_top(&theme, size, games, nav.page) < top + nav.page as i32 * pitch,
                        "{what}: row {} is drawn whole too",
                        nav.page
                    );
                }
            }
        }
    }
}
