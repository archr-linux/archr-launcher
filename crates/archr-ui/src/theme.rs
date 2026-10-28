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
    /// Radii, heights, font size and margin are screen-height fractions.
    pub radius_item: f32,
    pub radius_card: f32,
    pub header_height: f32,
    pub footer_height: f32,
    pub row_height: f32,
    pub game_row_height: f32,
    pub selected_height: f32,
    pub game_selected_height: f32,
    pub font_size: f32,
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
                let table = toml::from_str(include_str!("../../../themes/default/theme.toml"))
                    .expect("bundled theme must be valid TOML");
                Self::from_table(&table, None, None)
            })
            .clone()
    }

    /// Apply each TOML key directly over defaults; a malformed field cannot
    /// discard valid neighbours. User themes never inherit from system themes.
    pub fn load(dir: &Path) -> Self {
        let theme = Self::builtin();
        let path = dir.join("theme.toml");
        let table = match read_utf8(&path, MAX_THEME_BYTES, Follow)
            .map_err(|e| e.to_string())
            .and_then(|source| toml::from_str(&source).map_err(|e| e.to_string()))
        {
            Ok(table) => table,
            Err(error) => {
                log::warn!(
                    "Cannot load theme {}: {error}; using builtin",
                    path.display()
                );
                return theme;
            }
        };
        Self::from_table(&table, Some(dir), Some(&theme))
    }
    fn from_table(table: &toml::Table, dir: Option<&Path>, defaults: Option<&Self>) -> Self {
        // One field list serves both the embedded theme and user overrides.
        // Only the embedded TOML must be complete and valid.
        macro_rules! token {
            ($name:ident, $parse:ident) => {
                parsed(table, stringify!($name), $parse)
                    .or_else(|| defaults.map(|theme| theme.$name))
                    .expect(concat!("invalid builtin theme field: ", stringify!($name)))
            };
        }
        let font = |name| field::<String>(table, name).and_then(|f| resolve_font(dir?, &f));
        Self {
            background: token!(background, color),
            text: token!(text, color),
            highlight_bg: token!(highlight_bg, color),
            highlight_text: token!(highlight_text, color),
            dim_text: token!(dim_text, color),
            surface: token!(surface, color),
            surface_elevated: token!(surface_elevated, color),
            border: token!(border, color),
            badge: token!(badge, color),
            text_secondary: token!(text_secondary, color),
            muted: token!(muted, color),
            accent_text_secondary: token!(accent_text_secondary, color),
            scrim: token!(scrim, color),
            radius_item: token!(radius_item, fraction),
            radius_card: token!(radius_card, fraction),
            header_height: token!(header_height, fraction),
            footer_height: token!(footer_height, fraction),
            row_height: token!(row_height, fraction),
            game_row_height: token!(game_row_height, fraction),
            selected_height: token!(selected_height, fraction),
            game_selected_height: token!(game_selected_height, fraction),
            font_size: token!(font_size, fraction),
            margin: token!(margin, fraction),
            list_width: token!(list_width, fraction),
            cover: token!(cover, rectangle),
            system_screen: token!(system_screen, system_screen),
            font: font("font"),
            font_semibold: font("font_semibold"),
            font_mono: font("font_mono"),
            font_mono_medium: font("font_mono_medium"),
        }
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

fn field<T: DeserializeOwned>(table: &toml::Table, name: &str) -> Option<T> {
    let alias = match name {
        "highlight_bg" => Some("accent"),
        "highlight_text" => Some("accent_text"),
        "dim_text" => Some("dim"),
        _ => None,
    };
    // A wrong-typed alias may fall back to its legacy spelling. A correctly
    // typed but invalid value still wins and falls back to the builtin field.
    for key in alias.into_iter().chain(std::iter::once(name)) {
        if let Some(value) = table.get(key) {
            match value.clone().try_into() {
                Ok(value) => return Some(value),
                Err(error) => log::warn!("Invalid theme field {key}: {error}; using builtin value"),
            }
        }
    }
    None
}

fn parsed<T: DeserializeOwned, U>(
    table: &toml::Table,
    name: &str,
    parse: impl FnOnce(T) -> Option<U>,
) -> Option<U> {
    let value = parse(field(table, name)?);
    if value.is_none() {
        log::warn!("Invalid theme field {name}; using builtin value");
    }
    value
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
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
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
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
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
            log::warn!("Theme font {value:?} is outside the theme directory or not an accessible file ({result:?}); using system default");
            None
        }
    }
}
