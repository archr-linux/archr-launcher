use serde::{de::DeserializeOwned, Deserialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

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
    /// Syntax or I/O errors return the entire builtin theme with one warning.
    pub fn load(dir: &Path) -> Self {
        let mut theme = Self::builtin();
        let path = dir.join("theme.toml");
        let source = match fs::read_to_string(&path) {
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
        theme.font = raw.font.and_then(|font| resolve_font(dir, &font));
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
            highlight_bg: field(&table, "highlight_bg"),
            highlight_text: field(&table, "highlight_text"),
            dim_text: field(&table, "dim_text"),
            font: field(&table, "font"),
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
    match dir.join(path).canonicalize() {
        Ok(resolved) if resolved.is_file() => Some(resolved),
        result => {
            log::warn!(
                "Theme font {value:?} is not an accessible file ({result:?}); using system default"
            );
            None
        }
    }
}
