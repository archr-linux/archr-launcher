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

/// Glass (docs/temas.md, Estilo): a translucent fill, a 1 px border, a
/// 1 px highlight along the top and, behind it, the background blurred by
/// `blur` (a size; 0 for none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub fill: Rgba,
    pub border: Rgba,
    pub highlight: Rgba,
    pub blur: f32,
}
/// How the focused item stands out: the filled pill, or grown, raised and
/// ringed (`lift`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusStyle {
    Pill,
    Lift,
}
/// The ring around a lifted focus: its color and reference pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub color: Rgba,
    pub width: f32,
}
/// Motion along a fixed curve (`easing`), or a critically damped spring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionStyle {
    Tween,
    Spring,
}
/// A fill shading from `from` to `to`; `angle` 0 runs left to right, 90
/// top to bottom (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    pub from: Rgba,
    pub to: Rgba,
    pub angle: f32,
}

/// The shape of a screen, which picks a theme's `[aspect.<name>]` table:
/// square up to 6:5, wide from 29:20 (3:2, 16:9), classic between (4:3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    Square,
    Classic,
    Wide,
}
impl Aspect {
    pub fn of(width: u32, height: u32) -> Self {
        let ratio = width as f32 / height.max(1) as f32;
        if ratio < 1.2 {
            Self::Square
        } else if ratio > 1.45 {
            Self::Wide
        } else {
            Self::Classic
        }
    }
    /// `square`, `classic` or `wide`, as the theme names it.
    pub fn name(self) -> &'static str {
        self.key()
    }
    fn key(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Classic => "classic",
            Self::Wide => "wide",
        }
    }
}

/// The screen a theme was fitted to: its aspect and its size in reference
/// pixels (at least 640×480; 640×640 on a square screen, 720×480 on 3:2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub aspect: Aspect,
    pub reference: (f32, f32),
    /// The screen itself, in pixels.
    pub screen: (u32, u32),
}
impl Shape {
    pub const CLASSIC: Self = Self {
        aspect: Aspect::Classic,
        reference: (640., 480.),
        screen: (640, 480),
    };
    pub fn of(width: u32, height: u32) -> Self {
        let scale = crate::render::scale(width, height);
        Self {
            aspect: Aspect::of(width, height),
            reference: (width as f32 / scale, height as f32 / scale),
            screen: (width, height),
        }
    }
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
    /// The bold (700) face; without it bold text takes the semibold one.
    pub font_bold: Option<PathBuf>,
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
    /// Failure ink (a system update that did not go through).
    pub danger: Rgba,
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
    /// Start menu: the row of module widgets; tiles and shortcuts make room.
    pub start_widgets: Rect01,
    /// Start menu: where its 640x480 composition (title, tiles, shortcuts)
    /// is centered; the whole screen by default.
    pub start_area: Rect01,
    /// Achievements screen: progress bar and counts, and the list.
    pub achievements_progress: Rect01,
    pub achievements_list: Rect01,
    /// Saves screen: the list of states and the selected state's card.
    pub saves_list: Rect01,
    pub saves_card: Rect01,
    /// Screenshots screen: the grid of pictures.
    pub shots_grid: Rect01,
    /// Switcher (Alternador): the selected game's picture; the neighbours'
    /// size and top (`w`, `h`, `y`) and their gap to it (`x`); and the name,
    /// line and dots.
    pub switcher_card: Rect01,
    pub switcher_side: Rect01,
    pub switcher_info: Rect01,
    /// Resume at power on (Retomar-boot): the picture, and the texts, bar
    /// and buttons under it.
    pub resume_shot: Rect01,
    pub resume_info: Rect01,
    /// Style (docs/temas.md, Estilo): every corner radius times `corners`
    /// (0 = square), panel borders `border_width` reference pixels thick,
    /// the shadow under panels, console pictures scaled without smoothing
    /// (`art_scaling = "pixel"`) and motion in steps (`easing = "steps"`).
    pub corners: f32,
    pub border_width: f32,
    pub shadow: ShadowStyle,
    /// The soft shadow's color, reach beyond the box and fall, as sizes
    /// (fractions of 480); None draws it as the launcher always has.
    pub shadow_color: Option<Rgba>,
    pub shadow_blur: Option<f32>,
    pub shadow_offset: Option<f32>,
    /// The glass material, when the theme describes one.
    pub glass: Option<Glass>,
    /// The focused item: pill or lift, how much a lifted one grows (1 to
    /// 1.15) and its ring (the accent, 2 px, when the theme gives none).
    pub focus: FocusStyle,
    pub focus_scale: f32,
    pub focus_ring: Option<Ring>,
    pub motion: MotionStyle,
    /// Space between the letters of titles (text of `TITLE_SIZE` reference
    /// pixels and up), in em: negative tightens, as large display type
    /// wants. 0 keeps the font's own spacing.
    pub title_tracking: f32,
    /// Space between the letters of the small capitals over a title (the
    /// shelves' kicker, "2 DAYS AGO" over a card), in em. 0 keeps the
    /// font's own spacing.
    pub caps_tracking: f32,
    /// The browsing screens' background shaded instead of plain.
    pub gradient: Option<Gradient>,
    /// One table per screen (`[toast]`, `[hints]`, `[status]`, `[dialog]`).
    pub screens: crate::screens::Screens,
    pub pixel_art: bool,
    /// Theme pictures (background, frames, decorations) without smoothing.
    pub pixel_images: bool,
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
    /// How glyphs fit the pixel grid: "mono" (the default, approved on the
    /// device: stems snapped both ways), "light" (only heights snapped,
    /// the font's own widths and spacing) or "none".
    pub font_hinting: crate::text::Hinting,
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
    /// `[vars]`: the builtin's, then the theme's own, already written into
    /// every value that names one.
    vars: toml::Table,
    /// `[app]` tables in the order they apply (the builtin's, then the
    /// theme's own): an app's screens (API 4), whatever the view.
    app: Vec<toml::Table>,
    /// The screen this theme is laid out for.
    pub shape: Shape,
    /// `[options.<id>]`: what the user may choose in Appearance, the
    /// inherited ones first.
    options: Vec<ThemeOption>,
    /// The user's choices that are not the option's default, already
    /// written into `vars`.
    chosen: Vec<(String, String)>,
    /// The user's Appearance choices that reshape the theme's screens.
    pub user: UserLook,
    /// Each theme.toml this one is built from, builtin first, with its
    /// folder: `fitted` lays them out again for another screen.
    sources: Sources,
}

/// `[options.<id>]`: one choice of the theme's, made in Appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeOption {
    pub id: String,
    pub name: Names,
    /// The choice in effect until the user makes one.
    pub default: String,
    pub choices: Vec<ThemeChoice>,
}
/// `choices.<id> = { name, vars }`: its vars go over the theme's `[vars]`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeChoice {
    pub id: String,
    pub name: Names,
    vars: toml::Table,
}
/// A theme offers at most this many options.
pub const MAX_OPTIONS: usize = 12;
/// Choices one option offers at most.
const MAX_CHOICES: usize = 24;
/// Included files nest at most this deep (theme.toml is 0).
const MAX_INCLUDE_DEPTH: u8 = 4;

/// Appearance choices laid over every theme (none is a theme.toml key).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UserLook {
    /// The number beside the battery icon.
    pub battery_percent: bool,
    /// Box art on the games screen; off gives the list the whole width.
    pub game_art: bool,
    /// Box art width as a screen-width fraction, over the view's.
    pub art_width: Option<f32>,
    /// Box art corner radius in reference pixels, over the view's.
    pub art_radius: Option<f32>,
    /// Switcher: the selected game's picture fills the screen behind it,
    /// under a curtain of this opacity (0-100); None keeps the background.
    pub switcher_curtain: Option<u8>,
    /// Footer hints as plain letters, without the round buttons.
    pub text_hints: bool,
    /// The games list over the system's background picture, when it has one.
    pub system_background: bool,
}
impl Default for UserLook {
    fn default() -> Self {
        Self {
            battery_percent: true,
            game_art: true,
            art_width: None,
            art_radius: None,
            switcher_curtain: None,
            text_hints: false,
            system_background: true,
        }
    }
}

/// The theme.toml files a theme is built from, with their folders. Where a
/// theme came from is not how it looks: themes compare by their fields.
#[derive(Clone, Debug, Default)]
struct Sources(Vec<(toml::Table, Option<PathBuf>)>);
impl PartialEq for Sources {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Theme {
    /// The compiled-in default, independent of installed theme files.
    pub fn builtin() -> Self {
        static BUILTIN: OnceLock<Theme> = OnceLock::new();
        BUILTIN
            .get_or_init(|| {
                let table = toml::from_str(include_str!("../../../themes/default/theme.toml"))
                    .expect("bundled theme must be valid TOML");
                Self::from_table(&table, None, None, Shape::CLASSIC, &[])
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
        let Some(mut table) = read_table(&path) else {
            return theme;
        };
        let mut open = vec![path.canonicalize().unwrap_or(path)];
        included(&mut table, dir, &mut open);
        let mut theme = Self::from_table(&table, Some(dir), Some(&theme), theme.shape, &[]);
        if theme.name.get("en").is_none() {
            let folder = dir.file_name().unwrap_or_default().to_string_lossy();
            theme.name = Names::plain(&folder);
        }
        theme
    }
    /// A theme.toml's table over the builtin, without its folder (fonts unresolved).
    pub fn parse(table: &toml::Table) -> Self {
        Self::from_table(table, None, Some(&Self::builtin()), Shape::CLASSIC, &[])
    }
    /// This theme laid out for a `width`×`height` screen: every theme.toml
    /// it is built from again, each with its `[aspect.<shape>]` table over
    /// it. Fonts, name and view stay as they are.
    pub fn fitted(&self, width: u32, height: u32) -> Self {
        let shape = Shape::of(width, height);
        if shape == self.shape {
            return self.clone();
        }
        let mut fitted = self.rebuilt(shape, &self.chosen);
        fitted.font.clone_from(&self.font);
        fitted.font_semibold.clone_from(&self.font_semibold);
        fitted.font_bold.clone_from(&self.font_bold);
        fitted.font_mono.clone_from(&self.font_mono);
        fitted.font_mono_medium.clone_from(&self.font_mono_medium);
        fitted
    }
    /// The theme with the user's choices of its options (option id, choice
    /// id); an option left out keeps its default, an unknown one is
    /// ignored. Fonts the choices leave alone stay as they are.
    pub fn with_options(&self, chosen: &[(String, String)]) -> Self {
        let chosen: Vec<(String, String)> = self
            .options
            .iter()
            .filter_map(|option| {
                let (_, choice) = chosen.iter().find(|(id, _)| *id == option.id)?;
                (*choice != option.default && option.choices.iter().any(|c| c.id == *choice))
                    .then(|| (option.id.clone(), choice.clone()))
            })
            .collect();
        if chosen == self.chosen {
            return self.clone();
        }
        let mut next = self.rebuilt(self.shape, &chosen);
        // Fonts come from the folder again: those the choices did not
        // change keep the ones in use (a remapped root, a user's face).
        let same = self.rebuilt(self.shape, &self.chosen);
        macro_rules! keep {
            ($($font:ident),*) => {$(
                if next.$font == same.$font {
                    next.$font.clone_from(&self.$font);
                }
            )*};
        }
        keep!(font, font_semibold, font_bold, font_mono, font_mono_medium);
        next
    }
    /// The options the theme offers, in the order Appearance shows them.
    pub fn options(&self) -> &[ThemeOption] {
        &self.options
    }
    /// The choice in effect for the option `id` (its default until chosen).
    pub fn choice(&self, id: &str) -> Option<&str> {
        let option = self.options.iter().find(|o| o.id == id)?;
        Some(
            self.chosen
                .iter()
                .find(|(o, _)| o == id)
                .map_or(option.default.as_str(), |(_, c)| c.as_str()),
        )
    }
    /// Every theme.toml it is built from again, for `shape` and with
    /// `chosen`; name, view and the user's look stay.
    fn rebuilt(&self, shape: Shape, chosen: &[(String, String)]) -> Self {
        let mut rebuilt: Option<Self> = None;
        for (table, dir) in &self.sources.0 {
            rebuilt = Some(Self::from_table(
                table,
                dir.as_deref(),
                rebuilt.as_ref(),
                shape,
                chosen,
            ));
        }
        let mut rebuilt = rebuilt.unwrap_or_else(|| self.clone());
        rebuilt.name = self.name.clone();
        rebuilt.active_view.clone_from(&self.active_view);
        rebuilt.user = self.user;
        rebuilt
    }
    fn from_table(
        source: &toml::Table,
        dir: Option<&Path>,
        defaults: Option<&Self>,
        shape: Shape,
        chosen: &[(String, String)],
    ) -> Self {
        let mut sources = defaults.map_or_else(Sources::default, |d| d.sources.clone());
        sources.0.push((source.clone(), dir.map(Path::to_path_buf)));
        let mut merged = for_aspect(source, shape.aspect);
        if merged.remove("include").is_some() {
            log::warn!("Theme include needs the theme folder; ignored");
        }
        let options = options(&merged, defaults);
        let vars = vars(&merged, defaults, &options, chosen);
        crate::layout::substitute(&mut merged, &vars);
        crate::screens::flatten(&mut merged);
        let table = &merged;
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
        // Each family (regular with semibold and bold, mono with medium)
        // comes whole from the theme when it names any face, else from the
        // defaults.
        fn family<const N: usize>(
            table: &toml::Table,
            keys: [&'static str; N],
            font: impl Fn(&'static str) -> Option<PathBuf>,
            inherited: Option<[Option<PathBuf>; N]>,
        ) -> [Option<PathBuf>; N] {
            if keys.iter().any(|key| table.contains_key(*key)) {
                keys.map(font)
            } else {
                inherited.unwrap_or_else(|| std::array::from_fn(|_| None))
            }
        }
        let [font_regular, font_semibold, font_bold] = family(
            table,
            ["font", "font_semibold", "font_bold"],
            font,
            defaults.map(|d| [d.font.clone(), d.font_semibold.clone(), d.font_bold.clone()]),
        );
        let [font_mono, font_mono_medium] = family(
            table,
            ["font_mono", "font_mono_medium"],
            font,
            defaults.map(|d| [d.font_mono.clone(), d.font_mono_medium.clone()]),
        );
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
            danger: token!(danger, color),
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
            start_widgets: token!(start_widgets, rectangle),
            start_area: token!(start_area, rectangle),
            achievements_progress: token!(achievements_progress, rectangle),
            achievements_list: token!(achievements_list, rectangle),
            saves_list: token!(saves_list, rectangle),
            saves_card: token!(saves_card, rectangle),
            shots_grid: token!(shots_grid, rectangle),
            switcher_card: token!(switcher_card, rectangle),
            switcher_side: token!(switcher_side, rectangle),
            switcher_info: token!(switcher_info, rectangle),
            resume_shot: token!(resume_shot, rectangle),
            resume_info: token!(resume_info, rectangle),
            corners: token!(corners, unit),
            border_width: token!(border_width, border_width),
            shadow: token!(shadow, shadow),
            shadow_color: optional(
                table,
                "shadow_color",
                color,
                defaults.and_then(|d| d.shadow_color),
            ),
            shadow_blur: optional(
                table,
                "shadow_blur",
                shadow_blur,
                defaults.and_then(|d| d.shadow_blur),
            ),
            shadow_offset: optional(
                table,
                "shadow_offset",
                shadow_offset,
                defaults.and_then(|d| d.shadow_offset),
            ),
            glass: match table.get("glass") {
                Some(value) => glass(value, defaults.and_then(|d| d.glass)),
                None => defaults.and_then(|d| d.glass),
            },
            focus: optional(table, "focus", focus, defaults.map(|d| d.focus))
                .unwrap_or(FocusStyle::Pill),
            focus_scale: optional(
                table,
                "focus_scale",
                focus_scale,
                defaults.map(|d| d.focus_scale),
            )
            .unwrap_or(FOCUS_SCALE),
            focus_ring: match table.get("focus_ring") {
                Some(value) => ring(value),
                None => defaults.and_then(|d| d.focus_ring),
            },
            motion: optional(table, "motion", motion, defaults.map(|d| d.motion))
                .unwrap_or(MotionStyle::Tween),
            title_tracking: optional(
                table,
                "title_tracking",
                title_tracking,
                defaults.map(|d| d.title_tracking),
            )
            .unwrap_or(0.),
            caps_tracking: optional(
                table,
                "caps_tracking",
                caps_tracking,
                defaults.map(|d| d.caps_tracking),
            )
            .unwrap_or(0.),
            screens: crate::screens::Screens::read(
                table,
                defaults.map(|d| d.screens).unwrap_or_default(),
            ),
            gradient: match table.get("gradient") {
                Some(value) => gradient(value, "gradient"),
                None => defaults.and_then(|d| d.gradient),
            },
            pixel_art: token!(pixel_art = "art_scaling", art_scaling),
            pixel_images: token!(pixel_images = "image_scaling", art_scaling),
            steps: token!(steps = "easing", easing),
            font_scale: token!(font_scale, font_scale),
            font_grid: token!(font_grid, font_grid),
            font_min: token!(font_min, font_min),
            font_smoothing: token!(font_smoothing, some),
            font_hinting: parsed(table, "font_hinting", hinting)
                .or_else(|| defaults.map(|theme| theme.font_hinting))
                .unwrap_or_default(),
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
            font_bold,
            font_mono,
            font_mono_medium,
            name: Names::parse(table.get("name"), "name"),
            active_view: LISTA.into(),
            user: UserLook::default(),
            views: view_tables(table, defaults),
            vars,
            app: app_tables(table, defaults),
            shape,
            options,
            chosen: chosen.to_vec(),
            sources,
        }
    }
    /// The layout `id` describes, or the classic one when the theme has no such view.
    pub fn view(&self, id: &str) -> View {
        let (id, _) = self
            .views
            .iter()
            .find(|(view, _)| view == id)
            .unwrap_or(&self.views[0]);
        let mut view = View::lista(self);
        let classic = view.games.clone();
        view.id = id.clone();
        for table in &self.view_tables(id, 0) {
            view.overlay(table, &format!("view.{id}"));
        }
        view.games.fit_art(&self.user);
        view.app = crate::layout::app_layout(classic, &self.app);
        view
    }
    /// The tables of the view `id` in the order they apply. A view with
    /// `base = "<other>"` is the other one's tables made one, with its own
    /// laid over: an element with an id the other has changes only the
    /// fields it writes, `remove = [...]` in a screen takes inherited
    /// elements away.
    fn view_tables(&self, id: &str, depth: u8) -> Vec<toml::Table> {
        let tables = self
            .views
            .iter()
            .find(|(view, _)| view == id)
            .map_or(&[][..], |(_, tables)| tables.as_slice());
        let Some(base) = tables.iter().rev().find_map(|t| t.get("base")) else {
            return tables.to_vec();
        };
        let at = format!("view.{id}.base");
        let own = tables.iter().map(|t| {
            let mut t = t.clone();
            t.remove("base");
            t
        });
        let base = base.as_str().filter(|base| {
            let known = *base != id && self.views.iter().any(|(v, _)| v == base);
            if !known {
                log::warn!("Theme {at} names no other view of this theme; ignored");
            } else if depth >= MAX_INCLUDE_DEPTH {
                log::warn!("Theme {at}: views inherit at most {MAX_INCLUDE_DEPTH} deep; ignored");
            }
            known && depth < MAX_INCLUDE_DEPTH
        });
        let Some(base) = base else {
            return own.collect();
        };
        let mut view = toml::Table::new();
        for table in self.view_tables(base, depth + 1) {
            merge(&mut view, &table);
        }
        for table in own {
            inherit(&mut view, &table, &format!("view.{id}"));
        }
        vec![view]
    }
    /// The theme's glass, or a light one for a panel of glass in a theme
    /// that describes none.
    pub fn glass_or_default(&self) -> Glass {
        self.glass.unwrap_or_else(|| {
            glass(&toml::Value::Table(toml::Table::new()), None).expect("default glass")
        })
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
    "danger",
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
    "font_bold",
    "font_mono",
    "font_mono_medium",
    "name",
    "view",
    "app",
    "sounds",
    "details_shot",
    "details_info",
    "details_title",
    "details_subtitle",
    "details_text",
    "details_actions",
    "start_widgets",
    "start_area",
    "achievements_progress",
    "achievements_list",
    "saves_list",
    "saves_card",
    "shots_grid",
    "switcher_card",
    "switcher_side",
    "switcher_info",
    "resume_shot",
    "resume_info",
    "corners",
    "border_width",
    "shadow",
    "shadow_color",
    "shadow_blur",
    "shadow_offset",
    "glass",
    "focus",
    "focus_scale",
    "focus_ring",
    "motion",
    "title_tracking",
    "caps_tracking",
    "gradient",
    "toast",
    "hints",
    "status",
    "header",
    "dialog",
    "details",
    "achievements",
    "saves",
    "shots",
    "switcher",
    "resume",
    "start",
    "menu",
    "search",
    "keyboard",
    "osd",
    "settings",
    "art_scaling",
    "image_scaling",
    "easing",
    "font_scale",
    "font_grid",
    "font_min",
    "font_smoothing",
    "font_hinting",
    "text_outline",
    "background_image",
    "frames",
    "aspect",
    "vars",
    "options",
    "include",
    // Store metadata (docs/temas.md, Publicar na Loja de temas).
    "description",
    "author",
    "version",
    "license",
    "min_launcher",
];

/// `table` with its `[aspect.<aspect>]` table laid over it, key by key:
/// tables merge, any other value replaces.
fn for_aspect(table: &toml::Table, aspect: Aspect) -> toml::Table {
    let mut merged = table.clone();
    let Some(aspects) = merged.remove("aspect") else {
        return merged;
    };
    let Some(aspects) = aspects.as_table() else {
        log::warn!("Theme key aspect must hold [aspect.<shape>] tables; ignored");
        return merged;
    };
    for key in aspects.keys() {
        if !["square", "classic", "wide"].contains(&key.as_str()) {
            log::warn!("Unknown theme aspect {key}; ignored");
        }
    }
    match aspects.get(aspect.key()) {
        Some(toml::Value::Table(over)) => merge(&mut merged, over),
        Some(_) => log::warn!("Theme aspect.{} must be a table; ignored", aspect.key()),
        None => {}
    }
    merged
}
fn merge(base: &mut toml::Table, over: &toml::Table) {
    for (key, value) in over {
        match (base.get_mut(key), value) {
            (Some(toml::Value::Table(base)), toml::Value::Table(over)) => merge(base, over),
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

/// The inherited `[vars]` with this theme's own over them, then the vars
/// of each option's choice in effect over both.
fn vars(
    table: &toml::Table,
    defaults: Option<&Theme>,
    options: &[ThemeOption],
    chosen: &[(String, String)],
) -> toml::Table {
    let mut vars = defaults.map_or_else(toml::Table::new, |d| d.vars.clone());
    match table.get("vars") {
        None => {}
        Some(toml::Value::Table(own)) => vars.extend(scalars(own, "vars")),
        Some(_) => log::warn!("Theme key vars must be a table; ignored"),
    }
    for option in options {
        let id = chosen
            .iter()
            .find(|(o, _)| *o == option.id)
            .map_or(&option.default, |(_, c)| c);
        if let Some(choice) = option.choices.iter().find(|c| c.id == *id) {
            vars.extend(choice.vars.clone());
        }
    }
    vars
}
/// The text, number and true/false values of `table`; others are logged.
fn scalars(table: &toml::Table, at: &str) -> toml::Table {
    table
        .iter()
        .filter(|(name, value)| {
            let scalar = !matches!(value, toml::Value::Table(_) | toml::Value::Array(_));
            if !scalar {
                log::warn!("Theme {at}.{name} must be text, a number or true/false; ignored");
            }
            scalar
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// The inherited `[options]` with this theme's own over them, by id.
fn options(table: &toml::Table, defaults: Option<&Theme>) -> Vec<ThemeOption> {
    let mut options = defaults.map_or_else(Vec::new, |d| d.options.clone());
    let own = match table.get("options") {
        None => return options,
        Some(toml::Value::Table(own)) => own,
        Some(_) => {
            log::warn!("Theme key options must hold [options.<id>] tables; ignored");
            return options;
        }
    };
    for (id, value) in own {
        let Some(option) = theme_option(id, value) else {
            continue;
        };
        match options.iter().position(|o| o.id == *id) {
            Some(old) => options[old] = option,
            None if options.len() < MAX_OPTIONS => options.push(option),
            None => {
                log::warn!("Theme options.{id}: a theme has at most {MAX_OPTIONS} options; ignored")
            }
        }
    }
    options
}
fn theme_option(id: &str, value: &toml::Value) -> Option<ThemeOption> {
    let at = format!("options.{id}");
    let key_shaped = |id: &str| {
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    if !key_shaped(id) {
        log::warn!("Theme {at}: an option id has only letters, digits, _ and -; ignored");
        return None;
    }
    let Some(table) = value.as_table() else {
        log::warn!("Theme {at} must be a table; ignored");
        return None;
    };
    for key in table.keys() {
        if !["name", "default", "choices"].contains(&key.as_str()) {
            log::warn!("Unknown theme key {at}.{key}; ignored");
        }
    }
    let Some(listed) = table.get("choices").and_then(toml::Value::as_table) else {
        log::warn!("Theme {at} needs a choices table; ignored");
        return None;
    };
    let mut choices = Vec::new();
    for (choice, value) in listed {
        let at = format!("{at}.choices.{choice}");
        let Some(value) = value.as_table().filter(|_| key_shaped(choice)) else {
            log::warn!("Theme {at} must be a table under a plain id; ignored");
            continue;
        };
        if choices.len() == MAX_CHOICES {
            log::warn!("Theme {at}: an option has at most {MAX_CHOICES} choices; ignored");
            break;
        }
        for key in value.keys() {
            if !["name", "vars"].contains(&key.as_str()) {
                log::warn!("Unknown theme key {at}.{key}; ignored");
            }
        }
        let vars = match value.get("vars") {
            None => toml::Table::new(),
            Some(toml::Value::Table(vars)) => scalars(vars, &format!("{at}.vars")),
            Some(_) => {
                log::warn!("Theme {at}.vars must be a table; ignored");
                toml::Table::new()
            }
        };
        let mut name = Names::parse(value.get("name"), &at).truncated(40);
        if name.get("en").is_none() {
            name = Names::plain(choice);
        }
        choices.push(ThemeChoice {
            id: choice.clone(),
            name,
            vars,
        });
    }
    if choices.len() < 2 {
        log::warn!("Theme {at} needs at least two choices; ignored");
        return None;
    }
    let default = match table.get("default").and_then(toml::Value::as_str) {
        Some(default) if choices.iter().any(|c| c.id == default) => default.to_owned(),
        found => {
            if found.is_some() || table.contains_key("default") {
                log::warn!("Theme {at}.default names no choice; the first one");
            }
            choices[0].id.clone()
        }
    };
    let mut name = Names::parse(table.get("name"), &at).truncated(40);
    if name.get("en").is_none() {
        name = Names::plain(id);
    }
    Some(ThemeOption {
        id: id.to_owned(),
        name,
        default,
        choices,
    })
}

/// A theme.toml (or a file it includes) as a table; None with a warning.
fn read_table(path: &Path) -> Option<toml::Table> {
    match read_utf8(path, MAX_THEME_BYTES, Follow)
        .map_err(|e| e.to_string())
        .and_then(|source| toml::from_str(&source).map_err(|e| e.to_string()))
    {
        Ok(table) => Some(table),
        Err(error) => {
            log::warn!(
                "Cannot load theme {}: {error}; using the default",
                path.display()
            );
            None
        }
    }
}
/// `include = [...]`: each file (inside the theme folder) under `table`,
/// as if its keys were written there; `table` wins, and of two included
/// files the later one. `open` holds the files being read, theme.toml
/// first: one that includes itself again is turned away.
fn included(table: &mut toml::Table, dir: &Path, open: &mut Vec<PathBuf>) {
    let Some(listed) = table.remove("include") else {
        return;
    };
    let names: Vec<&str> = match &listed {
        toml::Value::String(one) => vec![one.as_str()],
        toml::Value::Array(many) => many.iter().filter_map(toml::Value::as_str).collect(),
        _ => Vec::new(),
    };
    if names.is_empty() {
        log::warn!("Theme include must name files (\"parts/a.toml\" or a list); ignored");
        return;
    }
    let mut under = toml::Table::new();
    for name in names {
        let Some(path) = resolve_file(dir, name) else {
            continue;
        };
        if open.contains(&path) {
            log::warn!("Theme include {name:?} includes itself again; ignored");
            continue;
        }
        if open.len() > usize::from(MAX_INCLUDE_DEPTH) {
            log::warn!("Theme include {name:?}: files include others at most {MAX_INCLUDE_DEPTH} deep; ignored");
            continue;
        }
        let Some(mut part) = read_table(&path) else {
            continue;
        };
        open.push(path);
        included(&mut part, dir, open);
        open.pop();
        merge(&mut under, &part);
    }
    merge(&mut under, table);
    *table = under;
}

/// A view's own table over the one it inherits (`Theme::view_tables`).
fn inherit(view: &mut toml::Table, own: &toml::Table, at: &str) {
    for (key, value) in own {
        let screen = matches!(key.as_str(), "systems" | "games");
        match (view.get_mut(key), value) {
            (Some(toml::Value::Table(base)), toml::Value::Table(over)) if screen => {
                inherit_screen(base, over, &format!("{at}.{key}"));
            }
            (None, toml::Value::Table(over)) if screen => {
                let mut base = toml::Table::new();
                inherit_screen(&mut base, over, &format!("{at}.{key}"));
                view.insert(key.clone(), toml::Value::Table(base));
            }
            (Some(toml::Value::Table(base)), toml::Value::Table(over)) => merge(base, over),
            _ => {
                view.insert(key.clone(), value.clone());
            }
        }
    }
}
fn inherit_screen(screen: &mut toml::Table, own: &toml::Table, at: &str) {
    let id = |e: &toml::Value| e.get("id").and_then(toml::Value::as_str).map(str::to_owned);
    let mut elements = match screen.remove("el") {
        Some(toml::Value::Array(elements)) => elements,
        _ => Vec::new(),
    };
    if let Some(remove) = own.get("remove") {
        let names: Vec<&str> = remove
            .as_array()
            .map(|names| names.iter().filter_map(toml::Value::as_str).collect())
            .unwrap_or_default();
        if names.is_empty() {
            log::warn!("Theme {at}.remove must list element ids; ignored");
        }
        for name in names {
            let before = elements.len();
            elements.retain(|e| id(e).as_deref() != Some(name));
            if elements.len() == before {
                log::warn!("Theme {at}.remove: no inherited element {name:?}");
            }
        }
    }
    match own.get("el") {
        None => {}
        Some(toml::Value::Array(over)) => {
            for element in over {
                let same = id(element).and_then(|wanted| {
                    elements
                        .iter_mut()
                        .find(|e| id(e).as_deref() == Some(&wanted))
                });
                match (same, element) {
                    (Some(toml::Value::Table(base)), toml::Value::Table(fields)) => {
                        base.extend(fields.clone());
                    }
                    _ => elements.push(element.clone()),
                }
            }
        }
        // Not a list: the screen's own reading says so.
        Some(other) => {
            screen.insert("el".into(), other.clone());
            elements.clear();
        }
    }
    for (key, value) in own {
        match (key.as_str(), screen.get_mut(key), value) {
            ("el" | "remove", ..) => {}
            (_, Some(toml::Value::Table(base)), toml::Value::Table(over)) => merge(base, over),
            _ => {
                screen.insert(key.clone(), value.clone());
            }
        }
    }
    if !elements.is_empty() {
        screen.insert("el".into(), toml::Value::Array(elements));
    }
}

/// The inherited `[app]` tables, with this theme's own after them.
fn app_tables(table: &toml::Table, defaults: Option<&Theme>) -> Vec<toml::Table> {
    let mut tables = defaults.map_or_else(Vec::new, |d| d.app.clone());
    match table.get("app") {
        None => {}
        Some(toml::Value::Table(own)) => {
            if tables.last() != Some(own) {
                tables.push(own.clone());
            }
        }
        Some(_) => log::warn!("Theme key app must be a table; ignored"),
    }
    tables
}

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

/// A key without a builtin value: the theme's, else the inherited one.
fn optional<T: DeserializeOwned, U>(
    table: &toml::Table,
    name: &str,
    parse: impl FnOnce(T) -> Option<U>,
    inherited: Option<U>,
) -> Option<U> {
    if table.contains_key(name) {
        parsed(table, name, parse).or(inherited)
    } else {
        inherited
    }
}
/// How much a lifted focus grows when the theme does not say.
pub const FOCUS_SCALE: f32 = 1.06;
fn focus(value: String) -> Option<FocusStyle> {
    choice(
        &value,
        &[("pill", FocusStyle::Pill), ("lift", FocusStyle::Lift)],
    )
}
fn focus_scale(value: f32) -> Option<f32> {
    (1.0..=1.15).contains(&value).then_some(value)
}
/// Text this large (reference pixels) and up is a title: `title_tracking`
/// spaces its letters.
pub const TITLE_SIZE: f32 = 20.;
fn title_tracking(value: f32) -> Option<f32> {
    (-0.1..=0.1).contains(&value).then_some(value)
}
fn caps_tracking(value: f32) -> Option<f32> {
    (0.0..=0.3).contains(&value).then_some(value)
}
fn motion(value: String) -> Option<MotionStyle> {
    choice(
        &value,
        &[
            ("tween", MotionStyle::Tween),
            ("spring", MotionStyle::Spring),
        ],
    )
}
/// The fields of a small style table: each `parse`d, warned about and left
/// out when wrong; unknown fields warned about.
fn style_table<'t>(value: &'t toml::Value, at: &str, keys: &[&str]) -> Option<&'t toml::Table> {
    let Some(table) = value.as_table() else {
        log::warn!("Theme {at} must be a table; ignored");
        return None;
    };
    for key in table.keys().filter(|k| !keys.contains(&k.as_str())) {
        log::warn!("Unknown theme key {at}.{key}; ignored");
    }
    Some(table)
}
fn style_color(table: &toml::Table, at: &str, key: &str) -> Option<Rgba> {
    let value = table.get(key)?;
    let parsed = value.as_str().and_then(|c| color(c.to_owned()));
    if parsed.is_none() {
        log::warn!("Theme {at}.{key} must be a color like \"#RRGGBB\"; ignored");
    }
    parsed
}
fn style_number(table: &toml::Table, at: &str, key: &str, range: (f32, f32)) -> Option<f32> {
    let value = table.get(key)?;
    let parsed = number(value).filter(|n| (range.0..=range.1).contains(n));
    if parsed.is_none() {
        log::warn!(
            "Theme {at}.{key} must be a number from {} to {}; ignored",
            range.0,
            range.1
        );
    }
    parsed
}
/// `glass = { fill, border, highlight, blur }`: what it leaves out comes
/// from the inherited glass, else from light defaults.
fn glass(value: &toml::Value, inherited: Option<Glass>) -> Option<Glass> {
    let Some(t) = style_table(value, "glass", &["fill", "border", "highlight", "blur"]) else {
        return inherited;
    };
    let white = |a| Rgba {
        r: 255,
        g: 255,
        b: 255,
        a,
    };
    let base = inherited.unwrap_or(Glass {
        fill: white(0x1F),
        border: white(0x33),
        highlight: white(0x40),
        blur: 0.,
    });
    Some(Glass {
        fill: style_color(t, "glass", "fill").unwrap_or(base.fill),
        border: style_color(t, "glass", "border").unwrap_or(base.border),
        highlight: style_color(t, "glass", "highlight").unwrap_or(base.highlight),
        blur: style_number(t, "glass", "blur", (0., 0.1)).unwrap_or(base.blur),
    })
}
/// `focus_ring = { color, width }`, width 0 to 8 reference pixels.
fn ring(value: &toml::Value) -> Option<Ring> {
    let t = style_table(value, "focus_ring", &["color", "width"])?;
    let color = style_color(t, "focus_ring", "color");
    let width = style_number(t, "focus_ring", "width", (0., 8.));
    match (color, width) {
        (Some(color), width) => Some(Ring {
            color,
            width: width.unwrap_or(2.),
        }),
        _ => {
            log::warn!("Theme focus_ring needs a color; ignored");
            None
        }
    }
}
/// `{ from, to, angle }`: both colors, the angle in degrees (0 when left out).
pub(crate) fn gradient(value: &toml::Value, at: &str) -> Option<Gradient> {
    let t = style_table(value, at, &["from", "to", "angle"])?;
    let angle = match t.get("angle") {
        None => Some(0.),
        Some(_) => style_number(t, at, "angle", (-360., 360.)),
    };
    match (style_color(t, at, "from"), style_color(t, at, "to"), angle) {
        (Some(from), Some(to), Some(angle)) => Some(Gradient { from, to, angle }),
        _ => {
            log::warn!("Theme {at} must be {{ from, to, angle }}; ignored");
            None
        }
    }
}
fn shadow_blur(value: f32) -> Option<f32> {
    (0.0..=0.1).contains(&value).then_some(value)
}
fn shadow_offset(value: f32) -> Option<f32> {
    (-0.05..=0.05).contains(&value).then_some(value)
}
pub(crate) fn color(value: String) -> Option<Rgba> {
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
fn hinting(value: String) -> Option<crate::text::Hinting> {
    use crate::text::Hinting;
    match value.as_str() {
        "mono" => Some(Hinting::Mono),
        "light" => Some(Hinting::Light),
        "none" => Some(Hinting::None),
        _ => None,
    }
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
pub(crate) fn sides(value: Option<&toml::Value>, max: f32) -> Option<[f32; 4]> {
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
        .flat_map(|view| ["systems", "games", "home"].map(|screen| view.get(screen)))
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
