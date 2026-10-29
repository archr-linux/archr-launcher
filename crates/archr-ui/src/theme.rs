use crate::layout::{Names, View, LISTA};
use archr_io::{read_utf8, Symlinks::Follow};
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

const MAX_THEME_BYTES: u64 = 64 * 1024;
/// The installed default theme: what other themes do not bring (fonts,
/// console pictures, sounds, icons) comes from here.
pub const DEFAULT_THEME_DIR: &str = "/usr/share/archr-launcher/themes/default";

/// Renderer-independent color, with straight (unpremultiplied) alpha.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Rectangle in screen fractions: x/w relative to width, y/h to height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct Rect01 {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A theme picture stretched over a box as a nine-slice frame: corners kept
/// whole, edges and middle stretched or repeated (docs/temas.md, Molduras).
#[derive(Clone, Debug, PartialEq)]
pub struct Frame9 {
    pub image: PathBuf,
    /// Image pixels kept whole at the left, top, right and bottom edges.
    pub slice: [u32; 4],
    /// Reference screen pixels (480 high) per image pixel.
    pub scale: f32,
    /// Reference pixels the frame reaches beyond its box: left, top, right, bottom.
    pub pad: [f32; 4],
    /// Edges and middle repeat instead of stretching.
    pub tile: bool,
}

/// A ring around text drawn straight over the background, and the ink of
/// that text (its own color when none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub color: Rgba,
    /// Reference pixels (480 high).
    pub width: f32,
    pub ink: Option<Rgba>,
}

/// Boxes a theme can dress with a frame.
pub const FRAMES: &[&str] = &[
    "header",
    "footer",
    "list",
    "row",
    "selected",
    "panel",
    "art",
    "button",
    "button_selected",
    "badge",
    "tile",
    "tile_selected",
];

/// What a raised panel casts: a blurred shadow, a solid offset block, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowStyle {
    Soft,
    Hard,
    None,
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
    /// Positive and cautionary status ink (e.g. matching / different ROM).
    pub success: Rgba,
    pub warning: Rgba,
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
    /// Game details screen: screen shot, info table, logo or name, the line
    /// under it, description and action buttons.
    pub details_shot: Rect01,
    pub details_info: Rect01,
    pub details_title: Rect01,
    pub details_subtitle: Rect01,
    pub details_text: Rect01,
    pub details_actions: Rect01,
    /// Achievements screen: progress bar and counts, and the list.
    pub achievements_progress: Rect01,
    pub achievements_list: Rect01,
    /// Style (docs/temas.md, Estilo): every corner radius times `corners`
    /// (0 = square), panel borders `border_width` reference pixels thick,
    /// the shadow under panels, console pictures scaled without smoothing
    /// (`art_scaling = "pixel"`) and motion in steps (`easing = "steps"`).
    pub corners: f32,
    pub border_width: f32,
    pub shadow: ShadowStyle,
    pub pixel_art: bool,
    pub steps: bool,
    /// Text: every size times `font_scale`, then snapped to a multiple of
    /// `font_grid` reference pixels (0: free); `font_smoothing = false`
    /// draws glyphs without anti-aliasing. `text_outline` (color, reference
    /// pixels) rings the text drawn straight over the background.
    pub font_scale: f32,
    pub font_grid: f32,
    /// No text is drawn smaller than this, in reference pixels: what does
    /// not fit at it wraps instead.
    pub font_min: f32,
    pub font_smoothing: bool,
    pub text_outline: Option<Outline>,
    /// Pictures of the browsing screens: the background and the frames.
    pub background_image: Option<PathBuf>,
    pub frames: Vec<(String, Frame9)>,
    /// Decoration images named in the views, resolved inside the theme
    /// folder when it loads; an image that escapes it is left out.
    decor_files: Vec<(String, PathBuf)>,
    /// Shown in Appearance; the folder name when the theme gives none.
    pub name: Names,
    /// Id of the view the screens are drawn with (an Appearance choice).
    pub active_view: String,
    /// `[view.<id>]` tables in the order they apply: the builtin's, then the
    /// theme's own. "lista" always comes first.
    views: Vec<(String, Vec<toml::Table>)>,
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
        Self::load_over(dir, &Self::builtin())
    }
    /// The theme in `dir`, taking what it does not say from `base` (the
    /// installed default theme, whose fonts the builtin cannot know).
    pub fn load_over(dir: &Path, base: &Self) -> Self {
        let theme = base.clone();
        let path = dir.join("theme.toml");
        let table = match read_utf8(&path, MAX_THEME_BYTES, Follow)
            .map_err(|e| e.to_string())
            .and_then(|source| toml::from_str(&source).map_err(|e| e.to_string()))
        {
            Ok(table) => table,
            Err(error) => {
                log::warn!(
                    "Cannot load theme {}: {error}; using the default",
                    path.display()
                );
                return theme;
            }
        };
        let mut theme = Self::from_table(&table, Some(dir), Some(&theme));
        if theme.name.get("en").is_none() {
            let folder = dir.file_name().unwrap_or_default().to_string_lossy();
            theme.name = Names::plain(&folder);
        }
        theme
    }
    /// A theme.toml's table over the builtin, without its folder (fonts unresolved).
    pub fn parse(table: &toml::Table) -> Self {
        Self::from_table(table, None, Some(&Self::builtin()))
    }
    fn from_table(table: &toml::Table, dir: Option<&Path>, defaults: Option<&Self>) -> Self {
        // One field list serves both the embedded theme and user overrides.
        // Only the embedded TOML must be complete and valid.
        macro_rules! token {
            ($name:ident, $parse:ident) => {
                token!($name = stringify!($name), $parse)
            };
            ($name:ident = $key:expr, $parse:ident) => {
                parsed(table, $key, $parse)
                    .or_else(|| defaults.map(|theme| theme.$name))
                    .expect(concat!("invalid builtin theme field: ", stringify!($name)))
            };
        }
        let font = |name: &str| field::<String>(table, name).and_then(|f| resolve_file(dir?, &f));
        // Each family (regular with semibold, mono with medium) comes whole
        // from the theme when it names either face, else from the defaults.
        let family = |keys: [&'static str; 2], inherited: fn(&Self) -> [Option<PathBuf>; 2]| {
            if keys.iter().any(|key| table.contains_key(*key)) {
                keys.map(font)
            } else {
                defaults.map_or([None, None], inherited)
            }
        };
        let [font_regular, font_semibold] = family(["font", "font_semibold"], |d| {
            [d.font.clone(), d.font_semibold.clone()]
        });
        let [font_mono, font_mono_medium] = family(["font_mono", "font_mono_medium"], |d| {
            [d.font_mono.clone(), d.font_mono_medium.clone()]
        });
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
            success: token!(success, color),
            warning: token!(warning, color),
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
            details_shot: token!(details_shot, rectangle),
            details_info: token!(details_info, rectangle),
            details_title: token!(details_title, rectangle),
            details_subtitle: token!(details_subtitle, rectangle),
            details_text: token!(details_text, rectangle),
            details_actions: token!(details_actions, rectangle),
            achievements_progress: token!(achievements_progress, rectangle),
            achievements_list: token!(achievements_list, rectangle),
            corners: token!(corners, unit),
            border_width: token!(border_width, border_width),
            shadow: token!(shadow, shadow),
            pixel_art: token!(pixel_art = "art_scaling", art_scaling),
            steps: token!(steps = "easing", easing),
            font_scale: token!(font_scale, font_scale),
            font_grid: token!(font_grid, font_grid),
            font_min: token!(font_min, font_min),
            font_smoothing: token!(font_smoothing, some),
            text_outline: match table.get("text_outline") {
                Some(value) => text_outline(value),
                None => defaults.and_then(|d| d.text_outline),
            },
            background_image: match field::<String>(table, "background_image") {
                Some(path) => dir.and_then(|dir| resolve_file(dir, &path)),
                None => defaults.and_then(|d| d.background_image.clone()),
            },
            decor_files: decor_files(table, dir, defaults),
            frames: match table.get("frames") {
                Some(value) => frames(value, dir),
                None => defaults.map_or_else(Vec::new, |d| d.frames.clone()),
            },
            font: font_regular,
            font_semibold,
            font_mono,
            font_mono_medium,
            name: Names::parse(table.get("name"), "name"),
            active_view: LISTA.into(),
            views: view_tables(table, defaults),
        }
    }
    /// The layout `id` describes, or the classic one when the theme has no such view.
    pub fn view(&self, id: &str) -> View {
        let (id, tables) = self
            .views
            .iter()
            .find(|(view, _)| view == id)
            .unwrap_or(&self.views[0]);
        let mut view = View::lista(self);
        view.id = id.clone();
        for table in tables {
            view.overlay(table, &format!("view.{id}"));
        }
        view
    }
    /// The file of the decoration image `image`, if it is inside the theme.
    pub fn decor_file(&self, image: &str) -> Option<&Path> {
        self.decor_files
            .iter()
            .find(|(name, _)| name == image)
            .map(|(_, path)| path.as_path())
    }
    /// The frame the theme gives the box `name`, if any.
    pub fn frame(&self, name: &str) -> Option<&Frame9> {
        self.frames.iter().find(|(n, _)| n == name).map(|(_, f)| f)
    }
    /// Ids of the views this theme offers, with their names.
    pub fn views(&self) -> Vec<(String, Names)> {
        self.views
            .iter()
            .map(|(id, tables)| {
                let name = tables.iter().rev().find_map(|t| t.get("name"));
                (id.clone(), Names::parse(name, &format!("view.{id}")))
            })
            .collect()
    }
}

/// Top-level keys the loader reads; anything else is logged and ignored.
pub const KNOWN_KEYS: &[&str] = &[
    "background",
    "text",
    "highlight_bg",
    "accent",
    "highlight_text",
    "accent_text",
    "dim_text",
    "dim",
    "surface",
    "surface_elevated",
    "border",
    "badge",
    "text_secondary",
    "muted",
    "accent_text_secondary",
    "scrim",
    "success",
    "warning",
    "radius_item",
    "radius_card",
    "header_height",
    "footer_height",
    "row_height",
    "game_row_height",
    "selected_height",
    "game_selected_height",
    "font_size",
    "margin",
    "list_width",
    "cover",
    "system_screen",
    "font",
    "font_semibold",
    "font_mono",
    "font_mono_medium",
    "name",
    "view",
    "sounds",
    "details_shot",
    "details_info",
    "details_title",
    "details_subtitle",
    "details_text",
    "details_actions",
    "achievements_progress",
    "achievements_list",
    "corners",
    "border_width",
    "shadow",
    "art_scaling",
    "easing",
    "font_scale",
    "font_grid",
    "font_min",
    "font_smoothing",
    "text_outline",
    "background_image",
    "frames",
    // Store metadata (docs/temas.md, Publicar na Loja de temas).
    "description",
    "author",
    "version",
    "license",
    "min_launcher",
];

/// The inherited views, with this theme's tables laid over them.
fn view_tables(table: &toml::Table, defaults: Option<&Theme>) -> Vec<(String, Vec<toml::Table>)> {
    for key in table.keys().filter(|k| !KNOWN_KEYS.contains(&k.as_str())) {
        log::warn!("Unknown theme key {key}; ignored");
    }
    let mut views =
        defaults.map_or_else(|| vec![(LISTA.to_owned(), Vec::new())], |d| d.views.clone());
    let declared = match table.get("view") {
        None => return views,
        Some(toml::Value::Table(declared)) => declared,
        Some(_) => {
            log::warn!("Theme key view must hold [view.<id>] tables; ignored");
            return views;
        }
    };
    for (id, value) in declared {
        let Some(own) = value.as_table() else {
            log::warn!("Theme view.{id} must be a table; ignored");
            continue;
        };
        let index = views.iter().position(|(v, _)| v == id).unwrap_or_else(|| {
            views.push((id.clone(), Vec::new()));
            views.len() - 1
        });
        // A theme that repeats the builtin's own table adds nothing.
        if views[index].1.last() != Some(own) {
            views[index].1.push(own.clone());
        }
    }
    views
}

/// User themes take precedence as a whole; fields inherit from the builtin,
/// not from the identically named system theme.
pub fn resolve_theme(user_dir: &Path, system_dir: &Path, name: &str) -> Theme {
    theme_dir(user_dir, system_dir, name).map_or_else(Theme::builtin, |dir| Theme::load(&dir))
}

/// Directory of the theme `resolve_theme` would load, if any.
pub fn theme_dir(user_dir: &Path, system_dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.starts_with('.') || name.contains('/') || name.contains("..") {
        log::warn!("Invalid theme name {name:?}; using builtin");
        return None;
    }
    let found = [user_dir, system_dir]
        .into_iter()
        .map(|root| root.join(name))
        .find(|dir| dir.join("theme.toml").is_file());
    if found.is_none() {
        log::warn!("Theme {name:?} not found; using builtin");
    }
    found
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
fn unit(value: f32) -> Option<f32> {
    (0.0..=1.0).contains(&value).then_some(value)
}
fn border_width(value: f32) -> Option<f32> {
    (1.0..=8.0).contains(&value).then_some(value)
}
/// `value` among `options`, ignoring case.
fn choice<T: Copy>(value: &str, options: &[(&str, T)]) -> Option<T> {
    options
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(value))
        .map(|(_, choice)| *choice)
}
fn some<T>(value: T) -> Option<T> {
    Some(value)
}
fn font_scale(value: f32) -> Option<f32> {
    (0.5..=2.0).contains(&value).then_some(value)
}
fn font_min(value: f32) -> Option<f32> {
    (6.0..=32.0).contains(&value).then_some(value)
}
fn font_grid(value: f32) -> Option<f32> {
    (0.0..=32.0).contains(&value).then_some(value)
}
fn text_outline(value: &toml::Value) -> Option<Outline> {
    let parsed = value.as_table().and_then(|t| {
        let ink = |key| t.get(key)?.as_str().and_then(|c| color(c.to_owned()));
        let outline = Outline {
            color: ink("color")?,
            width: number(t.get("width")?)?,
            ink: ink("ink"),
        };
        let ink_ok = t.get("ink").is_none() || outline.ink.is_some();
        ((0.0..=4.0).contains(&outline.width) && ink_ok).then_some(outline)
    });
    if parsed.is_none() {
        log::warn!("Theme text_outline must be {{ color, width = 0..4, ink }}; ignored");
    }
    parsed.filter(|outline| outline.width > 0.)
}
fn number(value: &toml::Value) -> Option<f32> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
        .map(|n| n as f32)
        .filter(|n| n.is_finite())
}
/// One number for all four sides, or `[left, top, right, bottom]`.
fn sides(value: Option<&toml::Value>, max: f32) -> Option<[f32; 4]> {
    let all = match value {
        None => return Some([0.; 4]),
        Some(toml::Value::Array(list)) if list.len() == 4 => {
            let mut sides = [0.; 4];
            for (side, value) in sides.iter_mut().zip(list) {
                *side = number(value)?;
            }
            sides
        }
        Some(value) => [number(value)?; 4],
    };
    all.iter().all(|n| (0.0..=max).contains(n)).then_some(all)
}
/// Every `decor` image of the theme's views, resolved like fonts and frames.
fn decor_files(
    table: &toml::Table,
    dir: Option<&Path>,
    defaults: Option<&Theme>,
) -> Vec<(String, PathBuf)> {
    let mut found = defaults.map_or_else(Vec::new, |d| d.decor_files.clone());
    let Some(dir) = dir else { return found };
    let views = table.get("view").and_then(toml::Value::as_table);
    let images = views
        .into_iter()
        .flat_map(|views| views.values())
        .filter_map(toml::Value::as_table)
        .flat_map(|view| ["systems", "games"].map(|screen| view.get(screen)))
        .flatten()
        .filter_map(|screen| screen.get("decor")?.as_array())
        .flatten()
        .filter_map(|decor| decor.get("image")?.as_str());
    for image in images {
        if found.iter().any(|(name, _)| name == image) {
            continue;
        }
        if let Some(path) = resolve_file(dir, image) {
            found.push((image.to_owned(), path));
        }
    }
    found
}
/// `[frames.<name>]`: image, slice, scale, pad and tile, for the known boxes.
fn frames(value: &toml::Value, dir: Option<&Path>) -> Vec<(String, Frame9)> {
    let Some(table) = value.as_table() else {
        log::warn!("Theme frames must hold [frames.<name>] tables; ignored");
        return Vec::new();
    };
    let mut found = Vec::new();
    for (name, value) in table {
        if !FRAMES.contains(&name.as_str()) {
            log::warn!("Unknown theme frame {name}; ignored");
            continue;
        }
        let frame = value.as_table().and_then(|t| {
            let image = resolve_file(dir?, t.get("image")?.as_str()?)?;
            let slice = sides(t.get("slice"), 256.)?.map(|n| n as u32);
            let scale = t.get("scale").map_or(Some(1.), number)?;
            let pad = sides(t.get("pad"), 64.)?;
            let tile = t.get("tile").map_or(Some(false), toml::Value::as_bool)?;
            (0.25..=8.0).contains(&scale).then_some(Frame9 {
                image,
                slice,
                scale,
                pad,
                tile,
            })
        });
        match frame {
            Some(frame) => found.push((name.clone(), frame)),
            None => log::warn!("Theme frames.{name} is invalid or its image is missing; ignored"),
        }
    }
    found
}
fn shadow(value: String) -> Option<ShadowStyle> {
    choice(
        &value,
        &[
            ("soft", ShadowStyle::Soft),
            ("hard", ShadowStyle::Hard),
            ("none", ShadowStyle::None),
        ],
    )
}
fn art_scaling(value: String) -> Option<bool> {
    choice(&value, &[("smooth", false), ("pixel", true)])
}
fn easing(value: String) -> Option<bool> {
    choice(&value, &[("smooth", false), ("steps", true)])
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
/// A file inside the theme folder (fonts, pictures); None with a warning.
pub(crate) fn resolve_file(dir: &Path, value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        log::warn!("Invalid theme file path {value:?}; ignored");
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
            log::warn!("Theme file {value:?} is outside the theme directory or not an accessible file ({result:?}); ignored");
            None
        }
    }
}
