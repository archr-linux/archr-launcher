use archr_io::{read_utf8, Symlinks::Follow};
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

const MAX_THEME_BYTES: u64 = 64 * 1024;

/// Renderer-independent color, with straight (unpremultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Rectangle in screen fractions: x/w relative to width, y/h to height.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Rect01 {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemScreen {
    List,
    /// Accepted for compatibility; the v1 renderer presents this as a list.
    Carousel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub background: Rgba,
    pub text: Rgba,
    pub highlight_bg: Rgba,
    pub highlight_text: Rgba,
    pub dim_text: Rgba,
    pub font: Option<PathBuf>,
    pub font_semibold: Option<PathBuf>,
    pub font_mono: Option<PathBuf>,
    pub font_mono_medium: Option<PathBuf>,
    pub surface: Rgba,
    pub surface_elevated: Rgba,
    pub border: Rgba,
    pub badge: Rgba,
    pub text_secondary: Rgba,
    pub muted: Rgba,
    pub accent_text_secondary: Rgba,
    pub scrim: Rgba,
    /// Radii and heights, as fractions of screen height.
    pub radius_item: f32,
    pub radius_card: f32,
    pub header_height: f32,
    pub footer_height: f32,
    pub row_height: f32,
    pub game_row_height: f32,
    pub selected_height: f32,
    pub game_selected_height: f32,

    /// Fraction of screen height.
    pub font_size: f32,
    /// Fraction of screen height.
    pub margin: f32,
    /// Fraction of screen width.
    pub list_width: f32,
    pub cover: Rect01,
    pub system_screen: SystemScreen,
}

impl Theme {
    /// The compiled-in default, independent of installed theme files.
    pub fn builtin() -> Self {
        static BUILTIN: OnceLock<Theme> = OnceLock::new();
        BUILTIN
            .get_or_init(|| {
                let raw = RawTheme::parse(include_str!("../../../themes/default/theme.toml"))
                    .expect("bundled theme must be valid TOML");
                Self {
                    background: color(raw.background.expect("builtin background"))
                        .expect("builtin background color"),
                    text: color(raw.text.expect("builtin text")).expect("builtin text color"),
                    highlight_bg: color(raw.highlight_bg.expect("builtin highlight_bg"))
                        .expect("builtin highlight_bg color"),
                    highlight_text: color(raw.highlight_text.expect("builtin highlight_text"))
                        .expect("builtin highlight_text color"),
                    dim_text: color(raw.dim_text.expect("builtin dim_text"))
                        .expect("builtin dim_text color"),
                    font: None,
                    font_semibold: None,
                    font_mono: None,
                    font_mono_medium: None,
                    surface: color(raw.surface.expect("builtin surface")).expect("builtin color"),
                    surface_elevated: color(
                        raw.surface_elevated.expect("builtin surface_elevated"),
                    )
                    .expect("builtin color"),
                    border: color(raw.border.expect("builtin border")).expect("builtin color"),
                    badge: color(raw.badge.expect("builtin badge")).expect("builtin color"),
                    text_secondary: color(raw.text_secondary.expect("builtin text_secondary"))
                        .expect("builtin color"),
                    muted: color(raw.muted.expect("builtin muted")).expect("builtin color"),
                    accent_text_secondary: color(
                        raw.accent_text_secondary
                            .expect("builtin accent_text_secondary"),
                    )
                    .expect("builtin color"),
                    scrim: color(raw.scrim.expect("builtin scrim")).expect("builtin color"),
                    radius_item: fraction(raw.radius_item.expect("builtin radius_item"))
                        .expect("builtin dimension"),
                    radius_card: fraction(raw.radius_card.expect("builtin radius_card"))
                        .expect("builtin dimension"),
                    header_height: fraction(raw.header_height.expect("builtin header_height"))
                        .expect("builtin dimension"),
                    footer_height: fraction(raw.footer_height.expect("builtin footer_height"))
                        .expect("builtin dimension"),
                    row_height: fraction(raw.row_height.expect("builtin row_height"))
                        .expect("builtin dimension"),
                    game_row_height: fraction(
                        raw.game_row_height.expect("builtin game_row_height"),
                    )
                    .expect("builtin dimension"),
                    selected_height: fraction(
                        raw.selected_height.expect("builtin selected_height"),
                    )
                    .expect("builtin dimension"),
                    game_selected_height: fraction(
                        raw.game_selected_height
                            .expect("builtin game_selected_height"),
                    )
                    .expect("builtin dimension"),

                    font_size: fraction(raw.font_size.expect("builtin font_size"))
                        .expect("builtin font_size in (0, 1]"),
                    margin: fraction(raw.margin.expect("builtin margin"))
                        .expect("builtin margin in (0, 1]"),
                    list_width: fraction(raw.list_width.expect("builtin list_width"))
                        .expect("builtin list_width in (0, 1]"),
                    cover: rectangle(raw.cover.expect("builtin cover"))
                        .expect("builtin cover within screen"),
                    system_screen: system_screen(raw.system_screen.expect("builtin system_screen"))
                        .expect("builtin system_screen variant"),
                }
            })
            .clone()
    }

    /// Load `dir/theme.toml`, inheriting missing or invalid fields individually.
    /// Syntax, I/O, oversized, or non-regular-file errors return the entire
    /// builtin theme with one warning.
    pub fn load(dir: &Path) -> Self {
        let mut theme = Self::builtin();
        let path = dir.join("theme.toml");
        let source = match read_utf8(&path, MAX_THEME_BYTES, Follow) {
            Ok(source) => source,
            Err(error) => {
                log::warn!(
                    "Cannot read theme {}: {error}; using builtin",
                    path.display()
                );
                return theme;
            }
        };
        let raw = match RawTheme::parse(&source) {
            Ok(raw) => raw,
            Err(error) => {
                log::warn!(
                    "Cannot parse theme {}: {error}; using builtin",
                    path.display()
                );
                return theme;
            }
        };
        apply("background", raw.background, &mut theme.background, color);
        apply("text", raw.text, &mut theme.text, color);
        apply(
            "highlight_bg",
            raw.highlight_bg,
            &mut theme.highlight_bg,
            color,
        );
        apply(
            "highlight_text",
            raw.highlight_text,
            &mut theme.highlight_text,
            color,
        );
        apply("dim_text", raw.dim_text, &mut theme.dim_text, color);
        apply("font_size", raw.font_size, &mut theme.font_size, fraction);
        apply("margin", raw.margin, &mut theme.margin, fraction);
        apply(
            "list_width",
            raw.list_width,
            &mut theme.list_width,
            fraction,
        );
        apply("cover", raw.cover, &mut theme.cover, rectangle);
        apply(
            "system_screen",
            raw.system_screen,
            &mut theme.system_screen,
            system_screen,
        );
        apply("surface", raw.surface, &mut theme.surface, color);
        apply(
            "surface_elevated",
            raw.surface_elevated,
            &mut theme.surface_elevated,
            color,
        );
        apply("border", raw.border, &mut theme.border, color);
        apply("badge", raw.badge, &mut theme.badge, color);
        apply(
            "text_secondary",
            raw.text_secondary,
            &mut theme.text_secondary,
            color,
        );
        apply("muted", raw.muted, &mut theme.muted, color);
        apply(
            "accent_text_secondary",
            raw.accent_text_secondary,
            &mut theme.accent_text_secondary,
            color,
        );
        apply("scrim", raw.scrim, &mut theme.scrim, color);
        apply(
            "radius_item",
            raw.radius_item,
            &mut theme.radius_item,
            fraction,
        );
        apply(
            "radius_card",
            raw.radius_card,
            &mut theme.radius_card,
            fraction,
        );
        apply(
            "header_height",
            raw.header_height,
            &mut theme.header_height,
            fraction,
        );
        apply(
            "footer_height",
            raw.footer_height,
            &mut theme.footer_height,
            fraction,
        );
        apply(
            "row_height",
            raw.row_height,
            &mut theme.row_height,
            fraction,
        );
        apply(
            "game_row_height",
            raw.game_row_height,
            &mut theme.game_row_height,
            fraction,
        );
        apply(
            "selected_height",
            raw.selected_height,
            &mut theme.selected_height,
            fraction,
        );
        apply(
            "game_selected_height",
            raw.game_selected_height,
            &mut theme.game_selected_height,
            fraction,
        );
        theme.font = raw.font.and_then(|font| resolve_font(dir, &font));
        theme.font_semibold = raw.font_semibold.and_then(|font| resolve_font(dir, &font));
        theme.font_mono = raw.font_mono.and_then(|font| resolve_font(dir, &font));
        theme.font_mono_medium = raw
            .font_mono_medium
            .and_then(|font| resolve_font(dir, &font));

        theme
    }
}

/// User themes take precedence as a whole; fields inherit from the builtin,
/// not from the identically named system theme.
pub fn resolve_theme(user_dir: &Path, system_dir: &Path, name: &str) -> Theme {
    if name.is_empty() || name.starts_with('.') || name.contains('/') || name.contains("..") {
        log::warn!("Invalid theme name {name:?}; using builtin");
        return Theme::builtin();
    }
    for root in [user_dir, system_dir] {
        let dir = root.join(name);
        if dir.join("theme.toml").is_file() {
            return Theme::load(&dir);
        }
    }
    log::warn!("Theme {name:?} not found; using builtin");
    Theme::builtin()
}

// Decode each known key separately so a valid TOML value with the wrong type
// cannot cause deserialization of unrelated keys to fail. Unknown keys stay
// in the intermediate table and are deliberately ignored.
struct RawTheme {
    background: Option<String>,
    text: Option<String>,
    highlight_bg: Option<String>,
    highlight_text: Option<String>,
    dim_text: Option<String>,
    font: Option<String>,
    font_semibold: Option<String>,
    font_mono: Option<String>,
    font_mono_medium: Option<String>,
    surface: Option<String>,
    surface_elevated: Option<String>,
    border: Option<String>,
    badge: Option<String>,
    text_secondary: Option<String>,
    muted: Option<String>,
    accent_text_secondary: Option<String>,
    scrim: Option<String>,
    radius_item: Option<f32>,
    radius_card: Option<f32>,
    header_height: Option<f32>,
    footer_height: Option<f32>,
    row_height: Option<f32>,
    game_row_height: Option<f32>,
    selected_height: Option<f32>,
    game_selected_height: Option<f32>,

    font_size: Option<f32>,
    margin: Option<f32>,
    list_width: Option<f32>,
    cover: Option<Rect01>,
    system_screen: Option<String>,
}

impl RawTheme {
    fn parse(source: &str) -> Result<Self, toml::de::Error> {
        let table: toml::Table = toml::from_str(source)?;
        Ok(Self {
            background: field(&table, "background"),
            text: field(&table, "text"),
            highlight_bg: field(&table, "accent").or_else(|| field(&table, "highlight_bg")),
            highlight_text: field(&table, "accent_text")
                .or_else(|| field(&table, "highlight_text")),
            dim_text: field(&table, "dim").or_else(|| field(&table, "dim_text")),
            font: field(&table, "font"),
            font_semibold: field(&table, "font_semibold"),
            font_mono: field(&table, "font_mono"),
            font_mono_medium: field(&table, "font_mono_medium"),
            surface: field(&table, "surface"),
            surface_elevated: field(&table, "surface_elevated"),
            border: field(&table, "border"),
            badge: field(&table, "badge"),
            text_secondary: field(&table, "text_secondary"),
            muted: field(&table, "muted"),
            accent_text_secondary: field(&table, "accent_text_secondary"),
            scrim: field(&table, "scrim"),
            radius_item: field(&table, "radius_item"),
            radius_card: field(&table, "radius_card"),
            header_height: field(&table, "header_height"),
            footer_height: field(&table, "footer_height"),
            row_height: field(&table, "row_height"),
            game_row_height: field(&table, "game_row_height"),
            selected_height: field(&table, "selected_height"),
            game_selected_height: field(&table, "game_selected_height"),

            font_size: field(&table, "font_size"),
            margin: field(&table, "margin"),
            list_width: field(&table, "list_width"),
            cover: field(&table, "cover"),
            system_screen: field(&table, "system_screen"),
        })
    }
}

fn field<T: DeserializeOwned>(table: &toml::Table, name: &str) -> Option<T> {
    let Some(value) = table.get(name) else {
        if name != "font" {
            log::warn!("Missing theme field {name}; using builtin value");
        }
        return None;
    };
    match value.clone().try_into() {
        Ok(value) => Some(value),
        Err(error) => {
            log::warn!("Invalid theme field {name}: {error}; using builtin value");
            None
        }
    }
}

fn apply<T, U>(name: &str, raw: Option<T>, target: &mut U, parse: impl FnOnce(T) -> Option<U>) {
    if let Some(raw) = raw {
        match parse(raw) {
            Some(value) => *target = value,
            None => log::warn!("Invalid theme field {name}; using builtin value"),
        }
    }
}

fn color(value: String) -> Option<Rgba> {
    let hex = value.strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(Rgba {
        r: u8::from_str_radix(&hex[0..2], 16).ok()?,
        g: u8::from_str_radix(&hex[2..4], 16).ok()?,
        b: u8::from_str_radix(&hex[4..6], 16).ok()?,
        a: if hex.len() == 8 {
            u8::from_str_radix(&hex[6..8], 16).ok()?
        } else {
            255
        },
    })
}

fn fraction(value: f32) -> Option<f32> {
    (value.is_finite() && value > 0.0 && value <= 1.0).then_some(value)
}

fn rectangle(rect: Rect01) -> Option<Rect01> {
    ([rect.x, rect.y, rect.w, rect.h]
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        && rect.x + rect.w <= 1.0
        && rect.y + rect.h <= 1.0)
        .then_some(rect)
}

fn system_screen(value: String) -> Option<SystemScreen> {
    if value.eq_ignore_ascii_case("list") {
        Some(SystemScreen::List)
    } else if value.eq_ignore_ascii_case("carousel") {
        Some(SystemScreen::Carousel)
    } else {
        None
    }
}

fn resolve_font(dir: &Path, value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        log::warn!("Invalid theme font path {value:?}; using system default");
        return None;
    }
    let canonical_dir = match dir.canonicalize() {
        Ok(dir) => dir,
        Err(error) => {
            log::warn!("Cannot resolve theme font directory: {error}; using system default");
            return None;
        }
    };
    match canonical_dir.join(path).canonicalize() {
        Ok(resolved) if resolved.starts_with(&canonical_dir) && resolved.is_file() => {
            Some(resolved)
        }
        result => {
            log::warn!(
                "Theme font {value:?} is outside the theme directory or not an accessible file ({result:?}); using system default"
            );
            None
        }
    }
}
