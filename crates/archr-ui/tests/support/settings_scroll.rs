use super::*;
#[test]
fn variable_heights_keep_selection_and_section_visible_and_snap_on_page_change() {
    let now = Instant::now();
    let mut scroll = Scroll::default();
    assert_eq!(scroll.offset(Page::System, 50., 90., 600., now), 0.);
    assert_eq!(scroll.offset(Page::System, 533., 579., 600., now), 0.);
    assert!(scroll.tween.active(now));
    let settled = scroll.offset(Page::System, 533., 579., 600., now + crate::motion::SELECT);
    assert_eq!(settled, 151.);
    assert!(533. - settled >= VIEW_TOP && 579. - settled <= VIEW_BOTTOM);
    let section = Placement {
        top: 180.,
        height: 46.,
        section: Some("settings.system"),
    };
    scroll.tween.retarget(180., now);
    scroll.tween.snap();
    scroll.offset(
        Page::System,
        section.selected_top(),
        section.bottom(),
        600.,
        now,
    );
    assert_eq!(
        scroll.tween.target, 109.,
        "scrolling up reveals the section header"
    );
    assert_eq!(scroll.offset(Page::Region, 533., 579., 600., now), 151.);
    assert!(
        !scroll.tween.active(now),
        "page changes snap to the selected row, not zero"
    );
    assert_eq!(scroll.offset(Page::Sound, 50., 90., 90., now), 0.);
}
#[test]
fn compact_placements_use_real_heights_and_three_pixel_gaps() {
    let rows = Page::Sound.rows();
    let positions = placements(Page::Sound, rows);
    assert_eq!(positions[0].top, VIEW_TOP);
    for (row, place) in rows.iter().zip(&positions) {
        assert_eq!(place.height, if row.subtitle.is_some() { 46. } else { 40. });
    }
    for pair in positions.windows(2) {
        assert_eq!(pair[1].top - pair[0].bottom(), ROW_GAP);
    }
}
#[test]
#[ignore = "manual shared renderer capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_long_page_scrolled_in_both_languages() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_SETTINGS_CAPTURE_DIR").expect("capture directory"),
    );
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::load(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"),
    );
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    for language in ["pt_BR", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        let mut state = Settings::default();
        state.loading = false;
        state.open(Page::System);
        let keys = [
            "settings.gpu",
            "settings.suspend",
            "settings.governor",
            "settings.turbo",
            "settings.volume",
            "settings.brightness",
            "settings.audio",
            "settings.language",
            "settings.timezone",
            "settings.idle",
            "settings.idle_game",
        ];
        let rows: Vec<_> = keys
            .into_iter()
            .enumerate()
            .map(|(n, label)| crate::settings::Row {
                id: Id::Governor,
                label,
                subtitle: (n % 3 == 0).then_some("settings.instant"),
                kind: Kind::Value,
                section: None,
            })
            .collect();
        let mut positions = placements(Page::System, &rows);
        for (i, place) in positions.iter_mut().enumerate() {
            place.top += (i / 4 + 1) as f32 * SECTION_HEIGHT;
            if i % 4 == 0 {
                place.section = Some("settings.system");
            }
        }
        ui.canvas.set_draw_color(color(theme.background));
        ui.canvas.clear();
        state.cursor = 2;
        ui.settings_page(&state, &theme).unwrap();
        state.cursor = 10;
        let last = *positions.last().unwrap();
        ui.settings_scroll.offset(
            Page::System,
            last.selected_top(),
            last.bottom(),
            last.bottom(),
            ui.frame.now,
        );
        ui.settings_scroll.tween.snap();
        ui.canvas.set_draw_color(color(theme.background));
        ui.canvas.fill_rect(Rect::new(16, 50, 608, 384)).unwrap();
        View {
            ui: &mut ui,
            theme: &theme,
        }
        .rows_at(&state, &rows, &positions, RIGHT)
        .unwrap();
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        let at = (410 * 640 + 30) * 4;
        assert_eq!(&pixels[at..at + 3], &[30, 136, 255]);
        image::save_buffer(
            output.join(format!("long-scrolled-{language}.png")),
            &pixels,
            640,
            480,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
