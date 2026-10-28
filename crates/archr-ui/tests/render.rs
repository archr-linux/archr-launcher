mod common;
use archr_index::{cache::SystemEntry, Game};
use archr_ui::{
    render::Ui,
    state::{App, Screen},
    Theme,
};
use common::FakeData as Data;
#[test]
fn grafite_layout_motion_glyphs_and_untrusted_text_render_offscreen() {
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
    assert_eq!(pixel(8, 432), &[28, 35, 44], "footer divider");
    ui.snap_animations();
    app.cursor = 1;
    ui.draw(&app, &data, &theme, None).unwrap();
    assert!(ui.animating(), "selection starts its 90ms slide");
    ui.snap_animations();
    ui.draw(&app, &data, &theme, None).unwrap();
    assert!(
        !ui.animating(),
        "new input can finish the previous target immediately"
    );
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
        system: Some("nes".into()),
        game: Some(0),
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
        assert!(
            (ink_x - center).abs() <= 0.25,
            "horizontal glyph center {ink_x} vs {center}"
        );
        assert!(
            (ink_y - 455.5).abs() <= 0.25,
            "vertical glyph center {ink_y} vs 455.5"
        );
    }
}
