//! A theme's own options (docs/temas.md, Telas livres, Opções do tema) in
//! Settings › Appearance: one row per option while they are few, a page of
//! their own when there are more; L/R choose, the preview takes the choice
//! at once and leaving the page hands it to the launcher, no restart. The
//! option rows of Grafite and Ellegance are golden at 640x480 and 1024x768
//! (tests/golden/theme-options, `ARCHR_GOLDEN_UPDATE=1` writes them again);
//! ARCHR_OPTIONS_CAPTURE_DIR keeps every frame.
mod common;
use archr_ui::{
    i18n::I18n,
    render::Ui,
    settings::{Effect, Id, Page, Settings},
    state::{App, Button},
    Theme,
};
use common::FakeData;
use std::path::{Path, PathBuf};

fn shipped(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../themes")
        .join(name)
}

/// Options that change the accent (`accent = "$acento"`) and a number;
/// `more` adds two, to pass the three the Appearance page holds.
fn options(accent: &str, more: bool) -> String {
    let mut text = format!(
        r##"
[vars]
acento = "{accent}"
raio = 8

[options.cor]
name = {{ en = "Tint", pt_BR = "Tom" }}
default = "original"
choices.original = {{ name = {{ en = "Original", pt_BR = "Original" }} }}
choices.roxo = {{ name = {{ en = "Purple", pt_BR = "Roxo" }}, vars = {{ acento = "#A56DE2" }} }}
choices.verde = {{ name = {{ en = "Green", pt_BR = "Verde" }}, vars = {{ acento = "#2EAD5B" }} }}

[options.cantos]
name = {{ en = "Corners", pt_BR = "Cantos" }}
default = "redondos"
choices.redondos = {{ name = {{ en = "Round", pt_BR = "Redondos" }} }}
choices.retos = {{ name = {{ en = "Square", pt_BR = "Retos" }}, vars = {{ raio = 0 }} }}
"##
    );
    if more {
        for id in ["extra1", "extra2"] {
            text.push_str(&format!(
                "\n[options.{id}]\nchoices.a = {{ name = \"A\" }}\nchoices.b = {{ name = \"B\" }}\n"
            ));
        }
    }
    text
}

/// A shipped theme in `dir` with its accent made a variable and the test
/// options added; its fonts come along.
fn theme_with_options(name: &str, dir: &Path, more: bool) -> Theme {
    let source = std::fs::read_to_string(shipped(name).join("theme.toml")).unwrap();
    let line = source
        .lines()
        .find(|l| l.starts_with("accent = "))
        .unwrap()
        .to_owned();
    let accent = line.split('"').nth(1).unwrap().to_owned();
    let text = source.replacen(&line, "accent = \"$acento\"", 1) + &options(&accent, more);
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    std::fs::write(dir.join("theme.toml"), text).unwrap();
    for font in std::fs::read_dir(shipped(name).join("fonts")).unwrap() {
        let font = font.unwrap();
        std::fs::copy(font.path(), dir.join("fonts").join(font.file_name())).unwrap();
    }
    let default = Theme::load(&shipped("default"));
    if name == "default" {
        Theme::load(dir)
    } else {
        Theme::load_over(dir, &default)
    }
}

fn catalog() -> I18n {
    I18n::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales"),
        Some("pt_BR"),
        None,
    )
}

/// Settings on Appearance with `theme`, the cursor on `id`.
fn appearance(theme: &Theme, id: Id) -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.values.insert(Id::Language, "pt_BR".into());
    let folder = theme.name.get("en").unwrap_or_default().to_lowercase();
    state.theme_names.insert(folder.clone(), theme.name.clone());
    state.values.insert(Id::Theme, folder);
    state.base_theme = theme.clone();
    state.open(Page::Appearance);
    state.cursor = state
        .visible_rows()
        .iter()
        .position(|r| r.id == id)
        .unwrap();
    state
}

#[test]
fn the_options_are_rows_of_appearance_and_a_draft_until_it_is_left() {
    let dir = tempfile::tempdir().unwrap();
    let theme = theme_with_options("default", dir.path(), false);
    let catalog = catalog();
    let mut state = appearance(&theme, Id::ThemeOption(0));
    let ids: Vec<Id> = state.visible_rows().iter().map(|r| r.id).collect();
    // Under the view, in the order of their ids; no link to a page.
    let view = ids.iter().position(|id| *id == Id::View).unwrap();
    assert_eq!(
        ids[view + 1..view + 3],
        [Id::ThemeOption(0), Id::ThemeOption(1)]
    );
    assert!(!ids.contains(&Id::ThemeOption(2)) && !ids.contains(&Id::ThemeOptionsLink));
    let rows = state.visible_rows();
    assert_eq!(state.label(&rows[view + 1]), "Cantos");
    assert_eq!(state.label(&rows[view + 2]), "Tom");
    assert_eq!(
        state.display_value(Id::ThemeOption(1), &catalog),
        "Original"
    );
    // R on the colour: the preview's theme takes it at once.
    state.cursor = view + 2;
    assert_eq!(state.handle(Button::R), Effect::None);
    assert_eq!(state.display_value(Id::ThemeOption(1), &catalog), "Roxo");
    assert_eq!(state.base_theme.highlight_bg.r, 0xA5);
    assert_eq!(state.base_theme.choice("cor"), Some("roxo"));
    // Leaving saves it; the launcher's theme follows without a restart,
    // at any size.
    assert_eq!(state.handle(Button::B), Effect::Appearance);
    let look = state.appearance();
    assert!(look.options.contains(&("cor".into(), "roxo".into())));
    let applied = look.apply(&theme.fitted(1024, 768));
    assert_eq!(applied.highlight_bg.r, 0xA5);
    assert_eq!(applied.choice("cantos"), Some("redondos"));
    // Restore puts the theme's defaults back.
    let mut again = appearance(&state.base_theme, Id::Restore);
    again.handle(Button::A);
    assert_eq!(again.base_theme.choice("cor"), Some("original"));
    assert_eq!(again.base_theme.highlight_bg, theme.highlight_bg);
    // A theme without options shows no row for them.
    let plain = appearance(&Theme::load(&shipped("default")), Id::View);
    let rows = plain.visible_rows();
    assert!(!rows
        .iter()
        .any(|r| matches!(r.id, Id::ThemeOption(_) | Id::ThemeOptionsLink)));
    // The key each choice is saved under, per theme folder.
    use archr_ui::appearance::option_key;
    assert_eq!(
        option_key("albedo", "cor").as_deref(),
        Some("launcher.theme_option.albedo.cor")
    );
    assert_eq!(option_key("meu tema", "cor"), None);
}

#[test]
fn more_than_three_options_get_a_page() {
    let dir = tempfile::tempdir().unwrap();
    let theme = theme_with_options("default", dir.path(), true);
    assert_eq!(theme.options().len(), 4);
    let mut state = appearance(&theme, Id::ThemeOptionsLink);
    assert!(!state
        .visible_rows()
        .iter()
        .any(|r| matches!(r.id, Id::ThemeOption(_))));
    state.handle(Button::A);
    assert_eq!(state.page(), Page::ThemeOptions);
    let rows = state.visible_rows();
    assert_eq!(rows.len(), 4);
    assert_eq!(state.label(&rows[3]), "extra2");
    state.cursor = 3;
    assert_eq!(state.handle(Button::R), Effect::None);
    assert_eq!(state.base_theme.choice("extra2"), Some("b"));
    // Back to Appearance, then out: one save for both pages.
    assert_eq!(state.handle(Button::B), Effect::None);
    assert_eq!(state.page(), Page::Appearance);
    assert_eq!(state.handle(Button::B), Effect::Appearance);
    assert!(state
        .appearance()
        .options
        .contains(&("extra2".into(), "b".into())));
}

/// The option row in Grafite and Ellegance, golden; R redraws the page
/// with the new choice and its colour, the launcher still running.
#[test]
fn the_option_rows_draw_in_grafite_and_ellegance() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/theme-options");
    let update = std::env::var_os("ARCHR_GOLDEN_UPDATE").is_some();
    let out = std::env::var_os("ARCHR_OPTIONS_CAPTURE_DIR").map(PathBuf::from);
    let mut changed = Vec::new();
    for (name, folder) in [("grafite", "default"), ("ellegance", "ellegance")] {
        let dir = tempfile::tempdir().unwrap();
        let theme = theme_with_options(folder, dir.path(), false);
        for size in [(640, 480), (1024, 768)] {
            let theme = theme.fitted(size.0, size.1);
            let (ttf, creator);
            let mut ui = Ui::new(size.0, size.1, true).unwrap();
            ttf = sdl2::ttf::init().unwrap();
            creator = ui.canvas.texture_creator();
            ui.set_motion(archr_ui::motion::Motion::Off);
            ui.configure(&ttf, &creator, &theme, None).unwrap();
            ui.set_status(archr_ui::status::Status::frozen("14:32"));
            ui.catalog = catalog();
            let mut state = appearance(&theme, Id::ThemeOption(1));
            let frame = |ui: &mut Ui, state: &Settings, label: &str| -> Vec<u8> {
                for _ in 0..2 {
                    ui.snap_animations();
                    ui.draw_with_settings(
                        &App::new(9),
                        &FakeData::default(),
                        &theme,
                        Default::default(),
                        Some(state),
                    )
                    .unwrap();
                }
                let pixels = ui
                    .canvas
                    .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                    .unwrap();
                let file = format!("{name}-{label}-{}x{}.png", size.0, size.1);
                if let Some(out) = &out {
                    std::fs::create_dir_all(out).unwrap();
                    image::save_buffer(
                        out.join(&file),
                        &pixels,
                        size.0,
                        size.1,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
                pixels
            };
            let before = frame(&mut ui, &state, "opcoes");
            let file = format!("{name}-{}x{}.png", size.0, size.1);
            let path = golden.join(&file);
            if update {
                std::fs::create_dir_all(&golden).unwrap();
                image::save_buffer(&path, &before, size.0, size.1, image::ColorType::Rgba8)
                    .unwrap();
            } else {
                let expected = image::open(&path)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                    .to_rgba8();
                let off = expected
                    .as_raw()
                    .chunks_exact(4)
                    .zip(before.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count();
                if off > 0 {
                    let actual = std::env::temp_dir().join(format!("archr-golden-{file}"));
                    image::save_buffer(&actual, &before, size.0, size.1, image::ColorType::Rgba8)
                        .unwrap();
                    changed.push(format!("{file}: {off} pixels (now {})", actual.display()));
                }
            }
            state.handle(Button::R);
            let after = frame(&mut ui, &state, "opcoes-roxo");
            let purple = after
                .chunks_exact(4)
                .filter(|p| {
                    p[0].abs_diff(0xA5) < 8 && p[1].abs_diff(0x6D) < 8 && p[2].abs_diff(0xE2) < 8
                })
                .count();
            assert!(purple > 50, "{name} {size:?}: {purple} purple pixels");
            assert_ne!(before, after, "{name} {size:?}");
        }
    }
    assert!(changed.is_empty(), "theme options changed: {changed:#?}");
}

/// A choice may change the fonts; one that does not keeps the faces in use
/// (the launcher remaps the default one to its root).
#[test]
fn fonts_follow_a_choice_and_stay_otherwise() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("fonts")).unwrap();
    let inter = shipped("ellegance").join("fonts");
    for (from, to) in [("Inter-Regular.ttf", "A.ttf"), ("Inter-Bold.ttf", "B.ttf")] {
        std::fs::copy(inter.join(from), dir.path().join("fonts").join(to)).unwrap();
    }
    std::fs::write(
        dir.path().join("theme.toml"),
        r##"
font = "$letra"
[vars]
letra = "fonts/A.ttf"
[options.letra]
choices.a = {}
choices.b = { vars = { letra = "fonts/B.ttf" } }
[options.outra]
choices.x = {}
choices.y = {}
"##,
    )
    .unwrap();
    let mut theme = Theme::load(dir.path());
    let name = |t: &Theme| t.font.as_ref().unwrap().file_name().unwrap().to_owned();
    assert_eq!(name(&theme), "A.ttf");
    let pick = |o: &str, c: &str| vec![(o.to_owned(), c.to_owned())];
    assert_eq!(name(&theme.with_options(&pick("letra", "b"))), "B.ttf");
    theme.font = Some(PathBuf::from("/root/fonts/A.ttf"));
    let other = theme.with_options(&pick("outra", "y"));
    assert_eq!(other.font, theme.font);
}
