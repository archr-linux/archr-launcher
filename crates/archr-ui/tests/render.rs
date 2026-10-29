mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    motion::Motion,
    render::{prepare_first_frame, Ui},
    startup::StartupTrace,
    state::{App, Button, Screen},
    Theme,
};
use common::FakeData as Data;
static SDL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn grafite_layout_motion_glyphs_and_untrusted_text_render_offscreen() {
    let _guard = SDL_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let mut theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let data = Data {
        systems: vec![SystemEntry {
            name: "nes".into(),
            fullname: "Nin\0tendo".into(),
            game_count: 1,
            dir_mtime: 0,
            source_fingerprint: 0,
        }],
        games: [(
            "nes".into(),
            vec![Game {
                path: "A.nes".into(),
                name: "Ga\0me".into(),
                image: None,
                video: None,
                favorite: false,
                year: None,
                players: None,
                details: Default::default(),
                last_played: None,
            }],
        )]
        .into(),
        cores: [(
            "nes".into(),
            ["a", "b"]
                .map(|core| ("retroarch".into(), core.into()))
                .into(),
        )]
        .into(),
        ..Data::default()
    };
    let mut app = App::new(10);
    ui.draw(&app, &data, &theme, None).unwrap();
    let rgba = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    let pixel = |x: usize, y: usize| &rgba[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3];
    assert_eq!(pixel(0, 0), &[11, 15, 20]);
    assert_eq!(pixel(16, 50), &[11, 15, 20], "pill corner stays rounded");
    assert_eq!(
        pixel(28, 56),
        &[30, 136, 255],
        "pill begins below the 44px header"
    );
    assert_eq!(
        pixel(8, 432),
        &[11, 15, 20],
        "footer waits for first present"
    );
    assert_eq!(
        pixel(380, 240),
        &[11, 15, 20],
        "panel must wait for first present"
    );
    ui.draw(&app, &data, &theme, None).unwrap();
    let settled = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    let divider = (432 * 640 + 8) * 4;
    assert_eq!(
        &settled[divider..divider + 3],
        &[28, 35, 44],
        "footer appears on the next frame"
    );
    let panel = (240 * 640 + 380) * 4;
    assert_eq!(
        &settled[panel..panel + 3],
        &[17, 22, 28],
        "panel appears on the next frame"
    );
    ui.snap_animations();
    app.cursor = 1;
    ui.draw(&app, &data, &theme, None).unwrap();
    assert!(ui.animating(), "selection starts its slide");
    ui.snap_animations();
    ui.draw(&app, &data, &theme, None).unwrap();
    assert!(
        !ui.animating(),
        "new input can finish the previous target immediately"
    );
    // Still pictures from here: screens and menus arrive at once.
    ui.set_motion(Motion::Off);
    app.cursor = 0;
    theme =
        Theme::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"));
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    app.screen = Screen::Games {
        system: "nes".into(),
    };
    ui.draw(&app, &data, &theme, None).unwrap();
    let rgba = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    assert_button_ink_is_centered(&rgba);
    ui.banner = Some("Failed\0message".into());
    ui.draw(&app, &data, &theme, None).unwrap();
    ui.banner = None;
    app.screen = Screen::QuickMenu {
        game: Some(("nes".into(), 0)),
    };
    ui.draw(&app, &data, &theme, None).unwrap();
    let rgba = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    assert!(
        rgba[2] <= 14,
        "quick-menu backdrop needs roughly 72% scrim opacity"
    );
    let offset = (140 * 640 + 170) * 4;
    assert_eq!(
        &rgba[offset..offset + 3],
        &[30, 136, 255],
        "quick-menu core row matches the approved y=134 layout"
    );
    ui.set_motion(Motion::On);
    let scroll_data = Data {
        systems: data.systems.clone(),
        games: [(
            "nes".into(),
            (0..64)
                .map(|i| {
                    let mut game = data.games["nes"][0].clone();
                    game.name = format!("Game {i}");
                    game.path = format!("game-{i}.nes").into();
                    game
                })
                .collect(),
        )]
        .into(),
        ..Data::default()
    };
    app.screen = Screen::Games {
        system: "nes".into(),
    };
    app.cursor = 0;
    ui.draw(&app, &scroll_data, &theme, None).unwrap();
    ui.snap_animations();
    app.cursor = 63;
    ui.draw(&app, &scroll_data, &theme, None).unwrap();
    assert!(
        ui.animating(),
        "scroll and pill animate rather than changing pages abruptly"
    );
    ui.snap_animations();
    ui.draw(&app, &scroll_data, &theme, None).unwrap();
    let rgba = ui
        .canvas
        .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
        .unwrap();
    let offset = (390 * 640 + 28) * 4;
    assert_eq!(
        &rgba[offset..offset + 3],
        &[30, 136, 255],
        "last row remains visible above the footer"
    );
}

fn assert_button_ink_is_centered(rgba: &[u8]) {
    let rgb = |x: usize, y: usize| &rgba[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3];
    // At the top of each 20px circle there is no glyph. Locate the four badges
    // from their fill rather than baking in text-dependent footer widths.
    let mut centers = Vec::new();
    let mut x = 0;
    while x < 640 {
        if rgb(x, 447) == [28, 37, 48] {
            let start = x;
            while x < 640 && rgb(x, 447) == [28, 37, 48] {
                x += 1;
            }
            centers.push((start + x - 1) as f32 / 2.);
        } else {
            x += 1;
        }
    }
    assert_eq!(centers.len(), 4);
    for (letter, center) in ['A', 'B', 'X', 'Y'].into_iter().zip(centers) {
        let mut bounds = (640, 480, 0, 0);
        for y in 446..466 {
            for x in (center as usize - 9)..=(center as usize + 9) {
                if rgb(x, y)[0] > 28 || rgb(x, y)[1] > 37 || rgb(x, y)[2] > 48 {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x);
                    bounds.3 = bounds.3.max(y);
                }
            }
        }
        assert!(bounds.2 >= bounds.0, "button has visible ink");
        let ink_x = (bounds.0 + bounds.2) as f32 / 2.;
        let ink_y = (bounds.1 + bounds.3) as f32 / 2.;
        println!(
            "BADGE_FINAL_CENTER {letter} dx={:.3}px dy={:.3}px",
            ink_x - center,
            ink_y - 455.5
        );
        // Hinted ink sits on whole pixels: at most half a pixel off when its
        // width parity differs from the circle's, never resampled to fit.
        assert!(
            (ink_x - center).abs() <= 0.5,
            "horizontal glyph center {ink_x} vs {center}"
        );
        assert!(
            (ink_y - 455.5).abs() <= 0.5,
            "vertical glyph center {ink_y} vs 455.5"
        );
    }
}

#[test]
fn first_hub_present_has_icons() {
    let _guard = SDL_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let data = Data::default();
    let mut app = App::new(10);
    app.screen = Screen::StartHub {
        row: 0,
        tile: 0,
        shortcut: 0,
    };
    let shots = tempfile::tempdir().unwrap();
    let shot = std::env::var_os("ARCHR_ICON_SCREENSHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| shots.path().to_path_buf())
        .join("hub-first-present.png");
    ui.screenshot = Some(shot.clone());
    ui.draw(&app, &data, &theme, None).unwrap();
    let first = image::open(&shot).unwrap().to_rgba8().into_raw();
    let icon = (130..178).flat_map(|y| (126..174).map(move |x| (y * 640 + x) * 4));
    assert!(
        icon.into_iter()
            .any(|at| first[at..at + 3] == [255, 255, 255]),
        "first hub present must show the Recent icon"
    );
}

#[test]
fn first_systems_present_has_prepared_collection_icons() {
    let _guard = SDL_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    let mut data = Data::default();
    data.counts.insert("@favorites".into(), 1);
    let catalog = archr_ui::i18n::I18n::default();
    let icons = tempfile::tempdir().unwrap();
    let prepared = prepare_first_frame(
        &ttf,
        &theme,
        (None, None, icons.path()),
        &catalog,
        &data,
        (640, 480),
        StartupTrace::default(),
    )
    .unwrap();
    ui.configure_prepared(&creator, prepared, None).unwrap();
    ui.catalog = catalog;
    let shots = tempfile::tempdir().unwrap();
    let shot = std::env::var_os("ARCHR_ICON_SCREENSHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| shots.path().to_path_buf())
        .join("systems-first-present.png");
    ui.screenshot = Some(shot.clone());
    ui.draw(&App::new(10), &data, &theme, None).unwrap();
    let first = image::open(&shot).unwrap().to_rgba8().into_raw();
    let icon = (50..105).flat_map(|y| (25..60).map(move |x| (y * 640 + x) * 4));
    assert!(
        icon.into_iter()
            .any(|at| first[at..at + 3] == [255, 255, 255]),
        "first systems present must show the prepared collection icon"
    );
}

#[test]
fn move_mode_draws_from_prepared_icons_without_touching_the_filesystem() {
    let _guard = SDL_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    let data = Data {
        systems: vec![SystemEntry {
            name: "nes".into(),
            fullname: "Nintendo".into(),
            game_count: 1,
            dir_mtime: 0,
            source_fingerprint: 0,
        }],
        ..Data::default()
    };
    let catalog = archr_ui::i18n::I18n::default();
    // A solid square stands in for the installed arrow: it is only on screen
    // if the icon was read at startup, since the directory is gone before drawing.
    let icons = tempfile::tempdir().unwrap();
    std::fs::write(
        icons.path().join("arrow-up-down.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect width="24" height="24" fill="currentColor"/></svg>"#,
    )
    .unwrap();
    let prepared = prepare_first_frame(
        &ttf,
        &theme,
        (None, None, icons.path()),
        &catalog,
        &data,
        (640, 480),
        StartupTrace::default(),
    )
    .unwrap();
    drop(icons);
    ui.configure_prepared(&creator, prepared, None).unwrap();
    ui.catalog = catalog;
    let mut app = App::new(10);
    app.handle(Button::Y, &data);
    app.handle(Button::Down, &data);
    app.handle(Button::A, &data);
    assert!(app.moving().is_some());
    let shots = tempfile::tempdir().unwrap();
    let shot = shots.path().join("move.png");
    ui.draw(&app, &data, &theme, None).unwrap();
    ui.screenshot = Some(shot.clone());
    ui.draw(&app, &data, &theme, None).unwrap();
    let frame = image::open(&shot).unwrap().to_rgba8().into_raw();
    let at = (72 * 640 + 44) * 4;
    assert_eq!(
        frame[at..at + 3],
        [255, 255, 255],
        "the lifted row icon must come from startup preparation"
    );
}

#[test]
fn theme_views_place_the_list_and_the_art() {
    let _guard = SDL_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let themes = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let data = Data {
        systems: ["gba", "snes", "n64"]
            .map(|name| SystemEntry {
                name: name.into(),
                fullname: name.to_uppercase(),
                game_count: 3,
                dir_mtime: 0,
                source_fingerprint: 0,
            })
            .to_vec(),
        ..Data::default()
    };
    let app = App::new(10);
    let mut shot = |theme: &Theme| {
        ui.configure(&ttf, &creator, theme, None).unwrap();
        for _ in 0..2 {
            ui.snap_animations();
            ui.draw(&app, &data, theme, None).unwrap();
        }
        let rgba = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        move |x: usize, y: usize| rgba[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3].to_vec()
    };
    // The example theme moves the yellow selection to the right, the panel left.
    let beach = Theme::load(&themes.join("praia"));
    let pixel = shot(&beach);
    assert_eq!(pixel(450, 70), [0xFF, 0xC9, 0x3C], "selection on the right");
    assert_eq!(pixel(100, 300), [0x0F, 0x30, 0x57], "panel on the left");
    assert_ne!(pixel(30, 70), [0xFF, 0xC9, 0x3C]);
    // Vitrine: the first system is a large tile in the middle, ringed in the accent.
    let mut vitrine = Theme::builtin();
    vitrine.active_view = "vitrine".into();
    let pixel = shot(&vitrine);
    let edge = 320 - 92;
    assert_eq!(
        pixel(edge, 188),
        [0x1E, 0x88, 0xFF],
        "accent ring of the centre tile"
    );
    assert_ne!(pixel(40, 70), [0x1E, 0x88, 0xFF], "no list pill");
}

#[test]
fn motion_levels_decide_what_moves() {
    let _guard = SDL_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let game = |name: &str| Game {
        path: format!("{name}.nes").into(),
        name: name.into(),
        image: None,
        video: None,
        favorite: false,
        year: None,
        players: None,
        last_played: None,
        details: Default::default(),
    };
    let data = Data {
        systems: ["nes", "snes"]
            .map(|name| SystemEntry {
                name: name.into(),
                fullname: name.into(),
                game_count: 2,
                dir_mtime: 0,
                source_fingerprint: 0,
            })
            .to_vec(),
        games: [("nes".into(), vec![game("a"), game("b")])].into(),
        ..Data::default()
    };
    // What animates after each gesture, for one motion level.
    let mut gestures = |motion: Motion| {
        ui.set_motion(motion);
        let mut app = App::new(10);
        let mut moved = Vec::new();
        let step = |ui: &mut Ui, app: &mut App, button: Option<Button>| {
            ui.snap_animations();
            if let Some(button) = button {
                app.handle(button, &data);
            }
            ui.draw(app, &data, &theme, None).unwrap();
            ui.animating()
        };
        step(&mut ui, &mut app, None);
        step(&mut ui, &mut app, None);
        step(&mut ui, &mut app, Some(Button::Down));
        // Has the pill already reached the second row?
        let rgba = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        let at = (120 * 640 + 28) * 4;
        moved.push(rgba[at..at + 3] != [30, 136, 255]);
        app.cursor = 0;
        step(&mut ui, &mut app, None);
        moved.push(step(&mut ui, &mut app, Some(Button::A)));
        moved.push(step(&mut ui, &mut app, Some(Button::Y)));
        moved
    };
    // Selection glide, systems to games (slide or fade), quick menu.
    assert_eq!(gestures(Motion::On), [true, true, true]);
    assert_eq!(
        gestures(Motion::Reduced),
        [false, true, true],
        "fades only, nothing travels"
    );
    assert_eq!(gestures(Motion::Off), [false, false, false]);
    // Input never waits: snapping ends whatever was moving.
    ui.set_motion(Motion::On);
    ui.start_opening();
    let app = App::new(10);
    ui.draw(&app, &data, &theme, None).unwrap();
    assert!(ui.animating());
    ui.snap_animations();
    ui.draw(&app, &data, &theme, None).unwrap();
    assert!(!ui.animating());
}
