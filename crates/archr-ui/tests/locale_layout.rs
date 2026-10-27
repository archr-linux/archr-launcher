use archr_ui::i18n::I18n;
use std::path::Path;

#[test]
fn shipped_fonts_cover_cyrillic_and_phase_one_labels_fit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ttf = sdl2::ttf::init().unwrap();
    for name in [
        "Inter-Regular.ttf",
        "Inter-SemiBold.ttf",
        "JetBrainsMono-Regular.ttf",
        "JetBrainsMono-Medium.ttf",
    ] {
        let font = ttf
            .load_font(root.join("themes/default/fonts").join(name), 15)
            .unwrap();
        for c in "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдеёжзийклмнопрстуфхцчшщъыьэюя".chars()
        {
            assert!(font.find_glyph(c).is_some(), "{name}: missing {c}");
        }
    }
    let font = ttf
        .load_font(root.join("themes/default/fonts/Inter-Regular.ttf"), 13)
        .unwrap();
    let semibold = ttf
        .load_font(root.join("themes/default/fonts/Inter-SemiBold.ttf"), 15)
        .unwrap();
    for code in ["en", "pt_BR", "es", "ru"] {
        let i = I18n::load(&root.join("locales"), Some(code), None);
        // Real footer allocation: 24px side margins, 20px badge, 6px gap,
        // and 16px between hints. No text may be truncated or leave the screen.
        for keys in [
            &["hint.open", "hint.menu"][..],
            &["hint.play", "hint.back", "hint.favorite", "hint.menu"][..],
            &["hint.choose", "hint.close"][..],
        ] {
            let width: u32 = keys
                .iter()
                .map(|key| font.size_of(&i.text(key)).unwrap().0 + 26)
                .sum::<u32>()
                + 16 * (keys.len() as u32 - 1);
            assert!(width <= 592, "{code}: footer {width}px");
        }
        for key in [
            "menu.title",
            "menu.settings",
            "menu.suspend",
            "menu.reboot",
            "menu.shutdown",
        ] {
            let width = semibold.size_of(&i.text(key)).unwrap().0;
            assert!(width <= 292, "{code}: {key}: {width}px");
        }
        // The default core takes at most half of the 292px content column;
        // the left label shares the row with L/R and three 8px gaps.
        let default_width = semibold.size_of(&i.text("menu.default")).unwrap().0;
        assert!(
            default_width <= 146,
            "{code}: default core {default_width}px"
        );
        let core_width = semibold.size_of(&i.text("menu.core")).unwrap().0;
        assert!(
            core_width + default_width + 2 * 10 + 3 * 8 <= 292,
            "{code}: core row"
        );
    }
}
