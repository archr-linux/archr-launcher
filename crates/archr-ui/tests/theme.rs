use archr_ui::{resolve_theme, Rect01, Rgba, SystemScreen, Theme};
use std::{fs, path::Path};
use tempfile::TempDir;

fn write_theme(dir: &Path, source: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("theme.toml"), source).unwrap();
}

fn load(source: &str) -> Theme {
    let dir = TempDir::new().unwrap();
    write_theme(dir.path(), source);
    Theme::load(dir.path())
}

#[test]
fn builtin_parses() {
    let theme = Theme::builtin();
    assert_eq!(theme.system_screen, SystemScreen::List);
    assert_eq!(theme.font, None);
    assert!(theme.background.r < 32 && theme.background.g < 32 && theme.background.b < 32);
    assert!(theme.text.r > 220 && theme.text.g > 220 && theme.text.b > 220);
    assert!((theme.font_size - 17.0 / 480.0).abs() < 0.01);
    assert!((theme.margin - 16.0 / 480.0).abs() < 0.01);
    assert!((theme.list_width - 378.0 / 640.0).abs() < 0.01);
    assert!(theme.cover.x >= theme.list_width);
    assert!(theme.cover.x + theme.cover.w <= 1.0);
    assert!(theme.cover.y + theme.cover.h <= 1.0);
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default");
    let mut installed = Theme::load(&shipped);
    for font in [
        &mut installed.font,
        &mut installed.font_semibold,
        &mut installed.font_mono,
        &mut installed.font_mono_medium,
    ] {
        assert!(font.as_ref().is_some_and(|p| p.is_file()));
        *font = None;
    }
    assert_eq!(installed, theme);
}

#[test]
fn invalid_color_field_falls_back_only_that_field() {
    let mut expected = Theme::builtin();
    expected.text = Rgba {
        r: 0x12,
        g: 0x34,
        b: 0x56,
        a: 0x78,
    };
    expected.margin = 0.1;
    assert_eq!(
        load("background = 'not-a-color'\ntext = '#12345678'\nmargin = 0.1"),
        expected
    );
}

#[test]
fn missing_dir_returns_builtin() {
    let dir = TempDir::new().unwrap();
    assert_eq!(Theme::load(&dir.path().join("missing")), Theme::builtin());
    assert_eq!(Theme::load(dir.path()), Theme::builtin());
}

#[test]
fn user_dir_wins_over_system_dir() {
    let dir = TempDir::new().unwrap();
    let user = dir.path().join("user");
    let system = dir.path().join("system");
    write_theme(&user.join("night"), "margin = 0.2");
    write_theme(&system.join("night"), "margin = 0.3");
    assert_eq!(resolve_theme(&user, &system, "night").margin, 0.2);
}

#[test]
fn system_dir_used_when_user_file_missing() {
    let dir = TempDir::new().unwrap();
    let user = dir.path().join("user");
    let system = dir.path().join("system");
    fs::create_dir_all(user.join("night")).unwrap();
    write_theme(&system.join("night"), "margin = 0.3");
    assert_eq!(resolve_theme(&user, &system, "night").margin, 0.3);
    assert_eq!(resolve_theme(&user, &system, "missing"), Theme::builtin());
}

#[test]
fn invalid_user_theme_does_not_mix_with_system_theme() {
    let dir = TempDir::new().unwrap();
    let user = dir.path().join("user");
    let system = dir.path().join("system");
    write_theme(&user.join("night"), "margin = [");
    write_theme(&system.join("night"), "margin = 0.3");
    assert_eq!(resolve_theme(&user, &system, "night"), Theme::builtin());
}

#[test]
fn unsafe_theme_names_return_builtin() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_theme(root, "margin = 0.2");
    write_theme(&root.join("night"), "margin = 0.3");
    write_theme(&root.join(".hidden"), "margin = 0.4");
    for name in [
        "",
        ".",
        "..",
        "../night",
        "night/",
        "night/..",
        "night/../night",
        ".hidden",
        "night..old",
    ] {
        assert_eq!(
            resolve_theme(root, root, name),
            Theme::builtin(),
            "{name:?}"
        );
    }
    assert_eq!(
        resolve_theme(root, root, root.to_str().unwrap()),
        Theme::builtin()
    );
}

#[test]
fn colors_support_rgb_rgba_and_reject_malformed_values() {
    let valid = load("background = '#aBcDeF'\ntext = '#01234567'\nhighlight_bg = '#010203'\nhighlight_text = '#040506'\ndim_text = '#070809'");
    assert_eq!(
        valid.background,
        Rgba {
            r: 0xab,
            g: 0xcd,
            b: 0xef,
            a: 255
        }
    );
    assert_eq!(
        valid.text,
        Rgba {
            r: 1,
            g: 0x23,
            b: 0x45,
            a: 0x67
        }
    );
    assert_eq!(
        valid.highlight_bg,
        Rgba {
            r: 1,
            g: 2,
            b: 3,
            a: 255
        }
    );
    assert_eq!(
        valid.highlight_text,
        Rgba {
            r: 4,
            g: 5,
            b: 6,
            a: 255
        }
    );
    assert_eq!(
        valid.dim_text,
        Rgba {
            r: 7,
            g: 8,
            b: 9,
            a: 255
        }
    );
    for value in [
        "''",
        "'#abc'",
        "'abcdef'",
        "'#1234567'",
        "'#gg0000'",
        "'#é1234'",
        "42",
        "[]",
        "{}",
        "true",
    ] {
        let mut expected = Theme::builtin();
        expected.margin = 0.2;
        assert_eq!(
            load(&format!("background = {value}\nmargin = 0.2")),
            expected,
            "{value}"
        );
    }
}

#[test]
fn invalid_types_fall_back_independently() {
    let mut expected = Theme::builtin();
    expected.text = Rgba {
        r: 1,
        g: 2,
        b: 3,
        a: 255,
    };
    assert_eq!(load("background = false\ntext = '#010203'\nhighlight_bg = 1\nhighlight_text = []\ndim_text = {}\nfont = 9\nfont_size = 'large'\nmargin = []\nlist_width = true\ncover = 'right'\nsystem_screen = 7"), expected);
}

#[test]
fn dimensions_reject_nonfinite_zero_negative_and_above_one() {
    for field in ["font_size", "margin", "list_width"] {
        for value in ["0.0", "-0.1", "1.01", "nan", "+inf", "-inf", "1e99"] {
            let mut expected = Theme::builtin();
            expected.system_screen = SystemScreen::Carousel;
            assert_eq!(
                load(&format!("{field} = {value}\nsystem_screen = 'carousel'")),
                expected,
                "{field} = {value}"
            );
        }
    }
    let theme = load("font_size = 1.0\nmargin = 0.001\nlist_width = 0.8");
    assert_eq!(
        (theme.font_size, theme.margin, theme.list_width),
        (1.0, 0.001, 0.8)
    );
}

#[test]
fn cover_is_validated_as_one_field() {
    for cover in [
        "{ x = -0.1, y = 0.0, w = 0.2, h = 0.2 }",
        "{ x = 0.9, y = 0.0, w = 0.2, h = 0.2 }",
        "{ x = 0.0, y = 0.9, w = 0.2, h = 0.2 }",
        "{ x = 0.0, y = 0.0, w = -0.2, h = 0.2 }",
        "{ x = 0.0, y = 0.0, w = 0.2, h = nan }",
        "{ x = inf, y = 0.0, w = 0.2, h = 0.2 }",
        "{ x = 0.0, y = 1.1, w = 0.0, h = 0.0 }",
        "{ x = 0.0, y = 0.0, w = 1.1, h = 0.0 }",
        "{ x = 'bad', y = 0.0, w = 0.2, h = 0.2 }",
        "{ x = 0.0, y = 0.0, w = 0.2 }",
    ] {
        let mut expected = Theme::builtin();
        expected.margin = 0.2;
        assert_eq!(
            load(&format!("cover = {cover}\nmargin = 0.2")),
            expected,
            "{cover}"
        );
    }
    assert_eq!(
        load("cover = { x = 0.5, y = 0.25, w = 0.5, h = 0.75 }").cover,
        Rect01 {
            x: 0.5,
            y: 0.25,
            w: 0.5,
            h: 0.75
        }
    );
    assert_eq!(
        load("cover = { x = 1.0, y = 1.0, w = 0.0, h = 0.0 }").cover,
        Rect01 {
            x: 1.0,
            y: 1.0,
            w: 0.0,
            h: 0.0
        }
    );
}

#[test]
fn system_screen_is_case_insensitive_with_fallback() {
    assert_eq!(
        load("system_screen = 'CaRoUsEl'").system_screen,
        SystemScreen::Carousel
    );
    assert_eq!(
        load("system_screen = 'LIST'").system_screen,
        SystemScreen::List
    );
    let mut expected = Theme::builtin();
    expected.margin = 0.2;
    assert_eq!(load("system_screen = 'grid'\nmargin = 0.2"), expected);
}

#[test]
fn font_resolves_only_existing_relative_files() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("fonts")).unwrap();
    fs::write(dir.path().join("fonts/menu.ttf"), b"font fixture").unwrap();
    write_theme(dir.path(), "font = 'fonts/menu.ttf'");
    let font = Theme::load(dir.path()).font.unwrap();
    assert!(font.is_absolute());
    assert_eq!(
        font,
        fs::canonicalize(dir.path().join("fonts/menu.ttf")).unwrap()
    );
    for path in [
        "missing.ttf",
        "fonts",
        "",
        "../outside.ttf",
        "fonts/../fonts/menu.ttf",
    ] {
        write_theme(dir.path(), &format!("font = '{path}'\nmargin = 0.2"));
        let theme = Theme::load(dir.path());
        assert_eq!(theme.font, None, "{path}");
        assert_eq!(theme.margin, 0.2);
    }
    let absolute = dir.path().join("fonts/menu.ttf");
    write_theme(dir.path(), &format!("font = '{}'", absolute.display()));
    assert_eq!(Theme::load(dir.path()).font, None);
}

#[test]
fn relative_theme_dir_still_returns_absolute_font_path() {
    let dir = tempfile::Builder::new()
        .prefix("theme-test-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    fs::write(dir.path().join("menu.ttf"), b"font fixture").unwrap();
    write_theme(dir.path(), "font = 'menu.ttf'");
    let cwd = std::env::current_dir().unwrap();
    let relative = dir.path().strip_prefix(&cwd).unwrap();
    assert_eq!(
        Theme::load(relative).font,
        Some(fs::canonicalize(dir.path().join("menu.ttf")).unwrap())
    );
}

#[test]
fn unknown_keys_are_ignored() {
    let mut expected = Theme::builtin();
    expected.margin = 0.2;
    expected.cover = Rect01 {
        x: 0.5,
        y: 0.0,
        w: 0.5,
        h: 1.0,
    };
    assert_eq!(load("future = ['new']\nmargin = 0.2\ncover = { x = 0.5, y = 0.0, w = 0.5, h = 1.0, future = true }\n[extras]\nanything = true"), expected);
}

#[test]
fn oversized_theme_returns_builtin() {
    let mut source = String::from("margin = 0.2\n#");
    source.push_str(&" ".repeat(64 * 1024 - source.len()));
    assert_eq!(load(&source).margin, 0.2);
    source.push(' ');
    assert_eq!(load(&source), Theme::builtin());
}

#[cfg(unix)]
#[test]
fn font_symlinks_cannot_escape_theme_dir() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let theme_dir = root.path().join("night");
    let outside = root.path().join("night-external");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("menu.ttf"), b"external font").unwrap();
    write_theme(&theme_dir, "font = 'linked.ttf'\nmargin = 0.2");
    symlink(outside.join("menu.ttf"), theme_dir.join("linked.ttf")).unwrap();
    symlink(&outside, theme_dir.join("linked-dir")).unwrap();
    for font in ["linked.ttf", "linked-dir/menu.ttf"] {
        write_theme(&theme_dir, &format!("font = '{font}'\nmargin = 0.2"));
        let theme = Theme::load(&theme_dir);
        assert_eq!(theme.font, None, "{font}");
        assert_eq!(theme.margin, 0.2);
    }
}

#[cfg(unix)]
#[test]
fn font_symlinks_within_theme_and_symlinked_theme_dir_are_allowed() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let theme_dir = root.path().join("night");
    write_theme(&theme_dir, "font = 'linked.ttf'");
    fs::write(theme_dir.join("menu.ttf"), b"local font").unwrap();
    symlink("menu.ttf", theme_dir.join("linked.ttf")).unwrap();
    let alias = root.path().join("alias");
    symlink(&theme_dir, &alias).unwrap();
    assert_eq!(
        Theme::load(&alias).font,
        Some(fs::canonicalize(theme_dir.join("menu.ttf")).unwrap())
    );
}

#[test]
fn syntax_errors_and_invalid_utf8_return_builtin() {
    assert_eq!(load("margin = 0.2\ntext = ["), Theme::builtin());
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("theme.toml"), [0xff]).unwrap();
    assert_eq!(Theme::load(dir.path()), Theme::builtin());
}

#[test]
fn warnings_report_invalid_fields_and_one_syntax_error() {
    use std::{cell::RefCell, sync::Once};
    thread_local! { static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
    struct Capture;
    impl log::Log for Capture {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                WARNINGS.with(|warnings| warnings.borrow_mut().push(record.args().to_string()));
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Capture = Capture;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    WARNINGS.with(|warnings| warnings.borrow_mut().clear());
    load("margin = 0.2\ntext = [");
    WARNINGS.with(|warnings| assert_eq!(warnings.borrow().len(), 1));
    WARNINGS.with(|warnings| warnings.borrow_mut().clear());
    load(&format!("margin = 0.2\n#{}", " ".repeat(64 * 1024)));
    WARNINGS.with(|warnings| assert_eq!(warnings.borrow().len(), 1));
    WARNINGS.with(|warnings| warnings.borrow_mut().clear());
    load("background = 'bad'\nfont_size = 'bad'\nmargin = nan\nlist_width = 2.0\ncover = {}\nsystem_screen = 'bad'\nfont = '../bad.ttf'");
    WARNINGS.with(|warnings| {
        for field in [
            "background",
            "font_size",
            "margin",
            "list_width",
            "cover",
            "system_screen",
            "font",
        ] {
            assert!(
                warnings
                    .borrow()
                    .iter()
                    .any(|warning| warning.contains(field)),
                "no warning for {field}"
            );
        }
    });
}

#[test]
fn grafite_tokens_inherit_independently_and_legacy_accent_still_works() {
    let theme = load("surface = '#010203'\nsurface_elevated = false\nradius_item = 0.03\nheader_height = nan\naccent = '#112233'\nfont_mono = '../escape.ttf'");
    assert_eq!(
        theme.surface,
        Rgba {
            r: 1,
            g: 2,
            b: 3,
            a: 255
        }
    );
    assert_eq!(theme.surface_elevated, Theme::builtin().surface_elevated);
    assert_eq!(theme.radius_item, 0.03);
    assert_eq!(theme.header_height, Theme::builtin().header_height);
    assert_eq!(
        theme.highlight_bg,
        Rgba {
            r: 0x11,
            g: 0x22,
            b: 0x33,
            a: 255
        }
    );
    assert_eq!(theme.font_mono, None);
    assert_eq!(
        load("highlight_bg = '#123456'").highlight_bg,
        Rgba {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 255
        }
    );
}
