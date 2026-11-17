//! One table per screen (docs/temas.md, Uma tabela por tela): `[toast]`,
//! `[hints]`, `[status]`, `[dialog]` and the tables that gather the older
//! loose keys (`[details]`, `[achievements]`, `[saves]`, `[shots]`,
//! `[switcher]`, `[resume]`, `[start]`). Every key is optional; what a
//! theme leaves out is inherited, and the defaults draw as the launcher
//! always has.
use crate::theme::{Rect01, Rgba};
use toml::{Table, Value};

/// Where the toasts stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// The user's choice (Appearance), the launcher's default.
    User,
    TopLeft,
    Top,
    TopRight,
}
/// The shape behind a toast's icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    Circle,
    Square,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToastStyle {
    pub position: Corner,
    /// The card's width, a size (of 480).
    pub width: f32,
    pub icon: Badge,
    /// The card on the theme's glass instead of its surface.
    pub glass: bool,
}
impl Default for ToastStyle {
    fn default() -> Self {
        Self {
            position: Corner::User,
            width: 276. / 480.,
            icon: Badge::Circle,
            glass: false,
        }
    }
}

/// Footer hints: on the footer as a bar, or gathered in a capsule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintsStyle {
    Bar,
    Capsule,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hints {
    pub style: HintsStyle,
    pub position: Side,
    /// A's hint lit (its badge in the text colour, its label too) and the
    /// others in the secondary text, instead of every one muted.
    pub lit: bool,
}
impl Default for Hints {
    fn default() -> Self {
        Self {
            style: HintsStyle::Bar,
            position: Side::Right,
            lit: false,
        }
    }
}

/// The clock and battery pill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusStyle {
    pub glass: bool,
    pub battery: bool,
    pub radios: bool,
    /// The clock in the monospaced face (the default) or the semibold one.
    pub mono: bool,
    /// Where the pill stands: its right edge at the box's, centered on its
    /// height. None: at the right of the header, as always.
    pub rect: Option<Rect01>,
}
impl Default for StatusStyle {
    fn default() -> Self {
        Self {
            glass: false,
            battery: true,
            radios: true,
            mono: true,
            rect: None,
        }
    }
}

/// Questions and alerts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DialogStyle {
    /// Width and corner radius, sizes (of 480); None keeps the launcher's.
    pub width: Option<f32>,
    pub radius: Option<f32>,
    /// A disc with an icon above the question.
    pub icon: bool,
    /// The answers as buttons side by side (`row`), the launcher's hints
    /// otherwise.
    pub buttons_row: bool,
    pub glass: bool,
}

/// The Start menu: its tiles and shortcuts (`"tiles"`, as always), or the
/// control center (`"control"`): the hour large, blocks of glass, round
/// shortcuts and the focused item's caption, over the screen it opened on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StartStyle {
    pub control: bool,
    /// Sizes (of 480): the blocks' corners, the round shortcuts and the hour.
    pub radius: f32,
    pub shortcut: f32,
    pub clock: f32,
    /// Where the control center's blocks, its round shortcuts and the hour
    /// (with the network and battery level with it) stand; None: laid out
    /// from the top as always.
    pub tiles: Option<Rect01>,
    pub shortcuts: Option<Rect01>,
    pub clock_at: Option<Rect01>,
}
impl Default for StartStyle {
    fn default() -> Self {
        Self {
            control: false,
            radius: 26. / 480.,
            shortcut: 52. / 480.,
            clock: 44. / 480.,
            tiles: None,
            shortcuts: None,
            clock_at: None,
        }
    }
}

/// The game menu and the quick menu: a list in a panel (`"list"`, as
/// always) or a card of glass beside the focused item (`"context"`), with a
/// game's quick actions on top, sections and destructive actions in red.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuStyle {
    pub context: bool,
    /// Beside the focused item (`"focus"`) or in the middle (`"center"`).
    pub anchor_focus: bool,
    /// Sizes (of 480): the card's width, a row's height and the corners.
    pub width: f32,
    pub row: f32,
    pub radius: f32,
    /// The box the card stands in (its width, and its height at most),
    /// over `anchor`.
    pub rect: Option<Rect01>,
}
impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            context: false,
            anchor_focus: true,
            width: 300. / 480.,
            row: 38. / 480.,
            radius: 26. / 480.,
            rect: None,
        }
    }
}

/// The game's details: the theme's rectangles (as always), or a showcase
/// over the whole screen (`style = "hero"`) with the game's picture
/// blurred behind, its title large and, with `shelf`, more of its system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailsStyle {
    pub hero: bool,
    pub shelf: bool,
}
impl Default for DetailsStyle {
    fn default() -> Self {
        Self {
            hero: false,
            shelf: true,
        }
    }
}

/// A game's achievements: the progress as a bar (as always) or a ring with
/// the count inside; the list's rows, badges and its group of glass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AchievementsStyle {
    pub ring: bool,
    /// The list in a rounded group of glass.
    pub glass: bool,
    /// Sizes (of 480): a row, the gap between rows, a badge and its corners.
    pub row: f32,
    pub gap: f32,
    pub badge: f32,
    pub badge_radius: f32,
}
impl Default for AchievementsStyle {
    fn default() -> Self {
        Self {
            ring: false,
            glass: false,
            row: 50. / 480.,
            gap: 2. / 480.,
            badge: 36. / 480.,
            badge_radius: 8. / 480.,
        }
    }
}

/// A game's saves: the list and its card (as always), or the selected
/// state's picture large in `preview` with its list in a group of glass
/// (`style = "preview"`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavesStyle {
    pub preview: bool,
    /// Sizes (of 480): a row of the list and its picture's width.
    pub row: f32,
    pub thumb: f32,
}
impl Default for SavesStyle {
    fn default() -> Self {
        Self {
            preview: false,
            row: 56. / 480.,
            thumb: 56. / 480.,
        }
    }
}

/// A game's screenshots: the grid's columns (0: as many as fit), gap and
/// corners, and the pictures grouped by when they were taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotsStyle {
    pub sections: bool,
    pub columns: u32,
    pub gap: f32,
    pub radius: f32,
}
impl Default for ShotsStyle {
    fn default() -> Self {
        Self {
            sections: false,
            columns: 0,
            gap: 12. / 480.,
            radius: 8. / 480.,
        }
    }
}

/// The switcher's cards side by side (as always) or in depth (`cards =
/// "depth"`): the selected one large and outlined, its neighbours smaller,
/// dimmed and leaving the screen, over its picture blurred.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwitcherStyle {
    pub depth: bool,
}

/// The resume at power on: its picture and texts in their rectangles (as
/// always), or the title large in the middle over the picture blurred
/// (`style = "title"`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResumeStyle {
    pub title: bool,
    /// Where the countdown's bar stands; None: under the texts.
    pub progress: Option<Rect01>,
    /// The game's picture behind; None: blurred behind the title, none
    /// behind the picture.
    pub backdrop: Option<Backdrop>,
}
/// A picture behind a screen: blurred by `blur` (a size, of 480; 0 keeps
/// it sharp) and showing through the background at `opacity` (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub blur: f32,
    pub opacity: f32,
}
impl Default for Backdrop {
    /// As the title style always drew it: blurred, two fifths through.
    fn default() -> Self {
        Self {
            blur: 24. / 480.,
            opacity: 105. / 255.,
        }
    }
}

/// The list's search: the field and plain rows (as always), or a field of
/// glass with the count inside, the results in a group of glass with their
/// covers and the term in bold, and the keyboard docked on a sheet
/// (`style = "field"`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchStyle {
    pub field: bool,
    /// Sizes (of 480): a result's row and the field's height.
    pub row: f32,
    pub height: f32,
    /// Where the field and the results stand; None: under the header,
    /// and from under the field to the keyboard.
    pub field_at: Option<Rect01>,
    pub results: Option<Rect01>,
    /// The keyboard docked at the bottom edge, or right under the results'
    /// box (`dock = "results"`, on a card of its own).
    pub dock_results: bool,
}
impl Default for SearchStyle {
    fn default() -> Self {
        Self {
            field: false,
            row: 38. / 480.,
            height: 40. / 480.,
            field_at: None,
            results: None,
            dock_results: false,
        }
    }
}

/// The on-screen keyboard: its keys (height, gap, corners, in sizes of
/// 480) and, with `sheet`, the editing on a sheet of glass with Cancel and
/// the confirmation at its top; the focused key light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyboardStyle {
    pub sheet: bool,
    pub key_h: f32,
    pub key_gap: f32,
    pub key_radius: f32,
}
impl Default for KeyboardStyle {
    fn default() -> Self {
        Self {
            sheet: false,
            key_h: 34. / 480.,
            key_gap: 6. / 480.,
            key_radius: 8. / 480.,
        }
    }
}

/// The volume and brightness OSD's shape (archr-osd).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OsdShape {
    /// The dark pill with the light fill, as always.
    #[default]
    Pill,
    /// A taller capsule of dark glass with a light rim.
    Capsule,
}
/// The screen edge the OSD stands at: centered on it, upright at the
/// sides and lying down at the top and bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OsdEdge {
    #[default]
    Right,
    Left,
    Top,
    Bottom,
}
/// `[osd]`: the volume and brightness OSD. The launcher hands it to
/// archr-osd (/run/archr-launcher/osd-theme.toml); what a theme leaves out
/// draws as always.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OsdStyle {
    pub style: OsdShape,
    pub position: OsdEdge,
    /// The level's colour; None: the OSD's own.
    pub fill: Option<Rgba>,
    /// Behind the level; None: the OSD's own.
    pub track: Option<Rgba>,
    /// Corner radius, a size (of 480); None: the shape's own.
    pub radius: Option<f32>,
}

/// The Settings pages in the order of their root, by the ids the theme's
/// `[settings.icon_colors]` names them with.
pub const SETTINGS_PAGES: [&str; 9] = [
    "network",
    "sound",
    "controls",
    "games",
    "region",
    "appearance",
    "achievements",
    "system",
    "storage",
];
/// The squares behind the root's icons (`icon = "tile"`) when the theme
/// gives no colour: one hue per page, dark enough for a white icon.
const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 255 }
}
const TILE_COLORS: [Rgba; 9] = [
    rgb(0x0A, 0x84, 0xFF),
    rgb(0xFF, 0x37, 0x5F),
    rgb(0x30, 0xB0, 0x50),
    rgb(0xFF, 0x7A, 0x00),
    rgb(0x5E, 0x5C, 0xE6),
    rgb(0xBF, 0x5A, 0xF2),
    rgb(0xD9, 0x8E, 0x04),
    rgb(0x6E, 0x6E, 0x76),
    rgb(0x1E, 0x9B, 0xD7),
];
/// The root of Settings as a sidebar: where it stands (`x`, `w`, fractions
/// of the 640-wide page), of glass or plain, and its title's size (of 480).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarStyle {
    pub x: f32,
    pub w: f32,
    pub glass: bool,
    pub title: f32,
}
impl Default for SidebarStyle {
    fn default() -> Self {
        Self {
            x: 14. / 640.,
            w: 214. / 640.,
            glass: true,
            title: 22. / 480.,
        }
    }
}
/// `[settings]`: the root as the list it always was or a sidebar with the
/// focused page's summary beside it (`layout`), the root's icons plain or
/// white on coloured squares (`icon = "tile"`), and the rows of every page
/// flat or in rounded groups (`groups = "inset"`). A theme that asks for
/// any of them has the rest of its style (glass, lifted focus, motion) in
/// Settings too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsStyle {
    pub sidebar: bool,
    pub bar: SidebarStyle,
    /// A root entry's height, a size (of 480).
    pub row: f32,
    pub tiles: bool,
    pub inset: bool,
    /// A page's title on a line of its own under the "‹ Parent" capsule,
    /// large (`title = "below"`), instead of beside it.
    pub title_below: bool,
    /// By `SETTINGS_PAGES`; None: the launcher's colour for that page.
    pub icon_colors: [Option<Rgba>; 9],
}
impl Default for SettingsStyle {
    fn default() -> Self {
        Self {
            sidebar: false,
            bar: SidebarStyle::default(),
            row: 36. / 480.,
            tiles: false,
            inset: false,
            title_below: false,
            icon_colors: [None; 9],
        }
    }
}
impl SettingsStyle {
    /// Whether the theme reshapes Settings at all; if not, they draw as
    /// they always have, whatever the rest of the theme says.
    pub fn themed(&self) -> bool {
        self.sidebar || self.tiles || self.inset
    }
    /// The focused row lit with a light fill and a ring, not the accent.
    pub fn light_focus(&self) -> bool {
        self.sidebar || self.inset
    }
    /// The square behind the root icon of page `n` (`SETTINGS_PAGES`).
    pub fn tile(&self, n: usize) -> Rgba {
        self.icon_colors
            .get(n)
            .copied()
            .flatten()
            .unwrap_or(TILE_COLORS[n % TILE_COLORS.len()])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Screens {
    pub toast: ToastStyle,
    pub hints: Hints,
    pub status: StatusStyle,
    pub dialog: DialogStyle,
    pub start: StartStyle,
    pub menu: MenuStyle,
    pub details: DetailsStyle,
    pub achievements: AchievementsStyle,
    pub saves: SavesStyle,
    pub shots: ShotsStyle,
    pub switcher: SwitcherStyle,
    pub resume: ResumeStyle,
    pub search: SearchStyle,
    pub keyboard: KeyboardStyle,
    pub osd: OsdStyle,
    pub settings: SettingsStyle,
}

/// Rectangles the screen tables place, as (table, key, old loose key).
pub const RECTS: &[(&str, &str, &str)] = &[
    ("details", "shot", "details_shot"),
    ("details", "info", "details_info"),
    ("details", "title", "details_title"),
    ("details", "subtitle", "details_subtitle"),
    ("details", "text", "details_text"),
    ("details", "actions", "details_actions"),
    ("achievements", "progress", "achievements_progress"),
    ("achievements", "list", "achievements_list"),
    ("saves", "list", "saves_list"),
    ("saves", "preview", "saves_card"),
    ("shots", "grid", "shots_grid"),
    ("switcher", "card", "switcher_card"),
    ("switcher", "side", "switcher_side"),
    ("switcher", "info", "switcher_info"),
    ("resume", "shot", "resume_shot"),
    ("resume", "info", "resume_info"),
    ("start", "area", "start_area"),
    ("start", "widgets", "start_widgets"),
];
/// Tables a theme may have at its top.
pub const TABLES: &[&str] = &[
    "toast",
    "hints",
    "status",
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
];

/// `table` with each screen table's rectangle under its old loose key, so
/// both spellings are read alike; the table's wins when both are given.
pub(crate) fn flatten(table: &mut Table) {
    for (screen, key, old) in RECTS {
        let rect = table
            .get(*screen)
            .and_then(Value::as_table)
            .and_then(|t| t.get(*key))
            // A word (`progress = "ring"`) is a style, not a place.
            .filter(|v| v.is_table())
            .cloned();
        if let Some(rect) = rect {
            if table.contains_key(*old) {
                log::warn!("Theme {screen}.{key} and {old} both given; {screen}.{key} is used");
            }
            table.insert((*old).to_owned(), rect);
        }
    }
    for screen in TABLES {
        let Some(value) = table.get(*screen) else {
            continue;
        };
        let Some(t) = value.as_table() else {
            log::warn!("Theme [{screen}] must be a table; ignored");
            continue;
        };
        let own: Vec<&str> = RECTS
            .iter()
            .filter(|(s, _, _)| s == screen)
            .map(|(_, k, _)| *k)
            .chain(style_keys(screen).iter().copied())
            .collect();
        for key in t.keys().filter(|k| !own.contains(&k.as_str())) {
            log::warn!("Unknown theme key {screen}.{key}; ignored");
        }
    }
}
fn style_keys(screen: &str) -> &'static [&'static str] {
    match screen {
        "toast" => &["position", "width", "icon", "style"],
        "hints" => &["style", "position", "lit"],
        "status" => &["style", "battery", "radios", "clock", "rect"],
        "dialog" => &["width", "radius", "icon", "buttons", "style"],
        "start" => &["style", "radius", "shortcut", "clock", "tiles", "shortcuts"],
        "menu" => &["style", "anchor", "width", "row", "radius", "rect"],
        "details" => &["style", "shelf"],
        "achievements" => &["style", "row", "gap", "badge", "badge_radius"],
        "saves" => &["style", "row", "thumb"],
        "shots" => &["sections", "columns", "gap", "radius"],
        "switcher" => &["cards"],
        "resume" => &["style", "progress", "backdrop"],
        "search" => &["style", "row", "height", "field", "results", "dock"],
        "keyboard" => &["sheet", "key"],
        "osd" => &["style", "position", "fill", "track", "radius"],
        "settings" => &[
            "layout",
            "sidebar",
            "row",
            "icon",
            "groups",
            "title",
            "icon_colors",
        ],
        _ => &[],
    }
}

impl Screens {
    /// The tables of `table` over `inherited`.
    pub(crate) fn read(table: &Table, inherited: Self) -> Self {
        let mut own = inherited;
        let get = |screen: &str| table.get(screen).and_then(Value::as_table);
        if let Some(t) = get("toast") {
            let r = Reader("toast", t);
            own.toast.position = r.pick(
                "position",
                own.toast.position,
                &[
                    ("top-left", Corner::TopLeft),
                    ("top", Corner::Top),
                    ("top-right", Corner::TopRight),
                ],
            );
            own.toast.width = r.number("width", own.toast.width, 0.3, 1.2);
            own.toast.icon = r.pick(
                "icon",
                own.toast.icon,
                &[
                    ("circle", Badge::Circle),
                    ("square", Badge::Square),
                    ("none", Badge::None),
                ],
            );
            own.toast.glass = r.pick(
                "style",
                own.toast.glass,
                &[("glass", true), ("plain", false)],
            );
        }
        if let Some(t) = get("hints") {
            let r = Reader("hints", t);
            own.hints.style = r.pick(
                "style",
                own.hints.style,
                &[("bar", HintsStyle::Bar), ("capsule", HintsStyle::Capsule)],
            );
            own.hints.position = r.pick(
                "position",
                own.hints.position,
                &[
                    ("left", Side::Left),
                    ("center", Side::Center),
                    ("right", Side::Right),
                ],
            );
            own.hints.lit = r.flag("lit", own.hints.lit);
        }
        if let Some(t) = get("status") {
            let r = Reader("status", t);
            own.status.glass = r.pick(
                "style",
                own.status.glass,
                &[("glass", true), ("plain", false)],
            );
            own.status.battery = r.flag("battery", own.status.battery);
            own.status.radios = r.flag("radios", own.status.radios);
            own.status.mono = r.pick("clock", own.status.mono, &[("mono", true), ("text", false)]);
            own.status.rect = r.rect("rect").or(own.status.rect);
        }
        if let Some(t) = get("dialog") {
            let r = Reader("dialog", t);
            if t.contains_key("width") {
                own.dialog.width = Some(r.number("width", 1., 0.5, 1.2));
            }
            if t.contains_key("radius") {
                own.dialog.radius = Some(r.number("radius", 0.03, 0., 0.1));
            }
            own.dialog.icon = r.flag("icon", own.dialog.icon);
            own.dialog.buttons_row = r.pick(
                "buttons",
                own.dialog.buttons_row,
                &[("row", true), ("hints", false)],
            );
            own.dialog.glass = r.pick(
                "style",
                own.dialog.glass,
                &[("glass", true), ("plain", false)],
            );
        }
        if let Some(t) = get("start") {
            let r = Reader("start", t);
            own.start.control = r.pick(
                "style",
                own.start.control,
                &[("tiles", false), ("control", true)],
            );
            own.start.radius = r.number("radius", own.start.radius, 0., 0.1);
            own.start.shortcut = r.number("shortcut", own.start.shortcut, 0.06, 0.16);
            // The hour: its size, or its place with its size inside.
            match t.get("clock") {
                Some(Value::Table(clock)) => {
                    own.start.clock_at = r.rect("clock").or(own.start.clock_at);
                    // Warnings name the key as the theme wrote it.
                    let at = ["start", "clock"].join(".");
                    let size = Reader(&at, clock);
                    own.start.clock = size.number("size", own.start.clock, 0.04, 0.16);
                }
                _ => own.start.clock = r.number("clock", own.start.clock, 0.04, 0.16),
            }
            own.start.tiles = r.rect("tiles").or(own.start.tiles);
            own.start.shortcuts = r.rect("shortcuts").or(own.start.shortcuts);
        }
        if let Some(t) = get("menu") {
            let r = Reader("menu", t);
            own.menu.context = r.pick(
                "style",
                own.menu.context,
                &[("list", false), ("context", true)],
            );
            own.menu.anchor_focus = r.pick(
                "anchor",
                own.menu.anchor_focus,
                &[("focus", true), ("center", false)],
            );
            own.menu.width = r.number("width", own.menu.width, 0.4, 1.0);
            own.menu.row = r.number("row", own.menu.row, 0.06, 0.12);
            own.menu.radius = r.number("radius", own.menu.radius, 0., 0.1);
            own.menu.rect = r.rect("rect").or(own.menu.rect);
        }
        if let Some(t) = get("details") {
            let r = Reader("details", t);
            own.details.hero = r.pick(
                "style",
                own.details.hero,
                &[("panels", false), ("hero", true)],
            );
            own.details.shelf = r.flag("shelf", own.details.shelf);
        }
        if let Some(t) = get("achievements") {
            let r = Reader("achievements", t);
            // `progress` is the bar's place, or a word, or a place with
            // its `style`.
            let progress = match t.get("progress") {
                Some(Value::Table(at)) => at.get("style").map(|_| Reader("achievements", at)),
                Some(_) => Some(Reader("achievements", t)),
                None => None,
            };
            let key = if matches!(t.get("progress"), Some(Value::Table(_))) {
                "style"
            } else {
                "progress"
            };
            if let Some(progress) = progress {
                own.achievements.ring = progress.pick(
                    key,
                    own.achievements.ring,
                    &[("bar", false), ("ring", true)],
                );
            }
            own.achievements.glass = r.pick(
                "style",
                own.achievements.glass,
                &[("plain", false), ("glass", true)],
            );
            let a = &mut own.achievements;
            a.row = r.number("row", a.row, 0.06, 0.2);
            a.gap = r.number("gap", a.gap, 0., 0.05);
            a.badge = r.number("badge", a.badge, 0.03, 0.15);
            a.badge_radius = r.number("badge_radius", a.badge_radius, 0., 0.075);
        }
        if let Some(t) = get("saves") {
            let r = Reader("saves", t);
            own.saves.preview = r.pick(
                "style",
                own.saves.preview,
                &[("list", false), ("preview", true)],
            );
            own.saves.row = r.number("row", own.saves.row, 0.06, 0.2);
            own.saves.thumb = r.number("thumb", own.saves.thumb, 0.04, 0.3);
        }
        if let Some(t) = get("shots") {
            let r = Reader("shots", t);
            own.shots.sections = r.flag("sections", own.shots.sections);
            own.shots.columns = r
                .number("columns", own.shots.columns as f32, 0., 8.)
                .round() as u32;
            own.shots.gap = r.number("gap", own.shots.gap, 0., 0.08);
            own.shots.radius = r.number("radius", own.shots.radius, 0., 0.08);
        }
        if let Some(t) = get("switcher") {
            let r = Reader("switcher", t);
            own.switcher.depth = r.pick(
                "cards",
                own.switcher.depth,
                &[("row", false), ("depth", true)],
            );
        }
        if let Some(t) = get("resume") {
            let r = Reader("resume", t);
            own.resume.title = r.pick(
                "style",
                own.resume.title,
                &[("picture", false), ("title", true)],
            );
            own.resume.progress = r.rect("progress").or(own.resume.progress);
            match t.get("backdrop") {
                None => {}
                Some(Value::Table(b)) => {
                    let at = ["resume", "backdrop"].join(".");
                    let r = Reader(&at, b);
                    let base = own.resume.backdrop.unwrap_or_default();
                    own.resume.backdrop = Some(Backdrop {
                        blur: r.number("blur", base.blur, 0., 0.2),
                        opacity: r.number("opacity", base.opacity, 0., 1.),
                    });
                    for name in b
                        .keys()
                        .filter(|n| !["blur", "opacity"].contains(&n.as_str()))
                    {
                        log::warn!("Unknown theme key resume.backdrop.{name}; ignored");
                    }
                }
                Some(_) => log::warn!("Theme resume.backdrop must be {{ blur, opacity }}; ignored"),
            }
        }
        if let Some(t) = get("search") {
            let r = Reader("search", t);
            own.search.field = r.pick(
                "style",
                own.search.field,
                &[("plain", false), ("field", true)],
            );
            own.search.row = r.number("row", own.search.row, 0.05, 0.12);
            own.search.height = r.number("height", own.search.height, 0.06, 0.12);
            own.search.field_at = r.rect("field").or(own.search.field_at);
            own.search.results = r.rect("results").or(own.search.results);
            own.search.dock_results = r.pick(
                "dock",
                own.search.dock_results,
                &[("bottom", false), ("results", true)],
            );
        }
        if let Some(t) = get("keyboard") {
            let r = Reader("keyboard", t);
            own.keyboard.sheet = r.flag("sheet", own.keyboard.sheet);
            match t.get("key") {
                None => {}
                Some(Value::Table(key)) => {
                    let r = Reader("keyboard", key);
                    let k = &mut own.keyboard;
                    k.key_h = r.number("h", k.key_h, 0.05, 0.1);
                    k.key_gap = r.number("gap", k.key_gap, 0., 0.03);
                    k.key_radius = r.number("radius", k.key_radius, 0., 0.05);
                    for name in key
                        .keys()
                        .filter(|n| !["h", "gap", "radius"].contains(&n.as_str()))
                    {
                        log::warn!("Unknown theme key keyboard.key.{name}; ignored");
                    }
                }
                Some(_) => log::warn!("Theme keyboard.key must be {{ h, gap, radius }}; ignored"),
            }
        }
        if let Some(t) = get("osd") {
            let r = Reader("osd", t);
            let o = &mut own.osd;
            o.style = r.pick(
                "style",
                o.style,
                &[("pill", OsdShape::Pill), ("capsule", OsdShape::Capsule)],
            );
            o.position = r.pick(
                "position",
                o.position,
                &[
                    ("right", OsdEdge::Right),
                    ("left", OsdEdge::Left),
                    ("top", OsdEdge::Top),
                    ("bottom", OsdEdge::Bottom),
                ],
            );
            o.fill = r.color("fill").or(o.fill);
            o.track = r.color("track").or(o.track);
            if t.contains_key("radius") {
                o.radius = Some(r.number("radius", 16. / 480., 0., 0.1));
            }
        }
        if let Some(t) = get("settings") {
            own.settings = read_settings(t, own.settings);
        }
        own
    }
}

fn read_settings(t: &Table, inherited: SettingsStyle) -> SettingsStyle {
    let r = Reader("settings", t);
    let mut own = inherited;
    own.sidebar = r.pick("layout", own.sidebar, &[("list", false), ("sidebar", true)]);
    own.row = r.number("row", own.row, 0.06, 0.12);
    own.tiles = r.pick("icon", own.tiles, &[("plain", false), ("tile", true)]);
    own.inset = r.pick("groups", own.inset, &[("flat", false), ("inset", true)]);
    own.title_below = r.pick(
        "title",
        own.title_below,
        &[("beside", false), ("below", true)],
    );
    match t.get("sidebar") {
        None => {}
        Some(Value::Table(bar)) => {
            let r = Reader("settings", bar);
            let b = &mut own.bar;
            b.x = r.number("x", b.x, 0., 0.2);
            b.w = r.number("w", b.w, 0.2, 0.6);
            b.glass = r.pick("style", b.glass, &[("glass", true), ("plain", false)]);
            b.title = r.number("title", b.title, 0.03, 0.08);
            for name in bar
                .keys()
                .filter(|n| !["x", "w", "style", "title"].contains(&n.as_str()))
            {
                log::warn!("Unknown theme key settings.sidebar.{name}; ignored");
            }
        }
        Some(_) => log::warn!("Theme settings.sidebar must be {{ x, w, style, title }}; ignored"),
    }
    match t.get("icon_colors") {
        None => {}
        Some(Value::Table(colors)) => {
            let r = Reader("settings", colors);
            for name in colors.keys() {
                match SETTINGS_PAGES.iter().position(|p| p == name) {
                    Some(n) => own.icon_colors[n] = r.color(name).or(own.icon_colors[n]),
                    None => log::warn!("Unknown theme key settings.icon_colors.{name}; ignored"),
                }
            }
        }
        Some(_) => log::warn!("Theme [settings.icon_colors] must be a table; ignored"),
    }
    own
}

struct Reader<'a>(&'a str, &'a Table);
impl Reader<'_> {
    fn pick<T: Copy>(&self, key: &str, current: T, options: &[(&str, T)]) -> T {
        let Some(value) = self.1.get(key) else {
            return current;
        };
        let found = value
            .as_str()
            .and_then(|v| options.iter().find(|(n, _)| n.eq_ignore_ascii_case(v)));
        match found {
            Some((_, choice)) => *choice,
            None => {
                let names: Vec<_> = options.iter().map(|(n, _)| *n).collect();
                log::warn!("Theme {}.{key} must be one of {names:?}; ignored", self.0);
                current
            }
        }
    }
    /// A box `{ x, y, w, h }` in screen fractions, inside the screen.
    fn rect(&self, key: &str) -> Option<Rect01> {
        let value = self.1.get(key)?;
        let found = value.as_table().and_then(|t| {
            let get = |k: &str| {
                t.get(k)
                    .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64)))
                    .map(|n| n as f32)
            };
            let rect = Rect01 {
                x: get("x")?,
                y: get("y")?,
                w: get("w")?,
                h: get("h")?,
            };
            ([rect.x, rect.y, rect.w, rect.h]
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && rect.x + rect.w <= 1.0 + 1e-4
                && rect.y + rect.h <= 1.0 + 1e-4)
                .then_some(rect)
        });
        if found.is_none() {
            log::warn!(
                "Theme {}.{key} must be {{ x, y, w, h }} inside the screen; ignored",
                self.0
            );
        }
        found
    }
    /// `#RRGGBB` or `#RRGGBBAA`.
    fn color(&self, key: &str) -> Option<Rgba> {
        let value = self.1.get(key)?;
        let found = value
            .as_str()
            .and_then(|v| crate::theme::color(v.to_owned()));
        if found.is_none() {
            log::warn!(
                "Theme {}.{key} must be #RRGGBB or #RRGGBBAA; ignored",
                self.0
            );
        }
        found
    }
    fn flag(&self, key: &str, current: bool) -> bool {
        match self.1.get(key) {
            None => current,
            Some(Value::Boolean(b)) => *b,
            Some(_) => {
                log::warn!("Theme {}.{key} must be true or false; ignored", self.0);
                current
            }
        }
    }
    fn number(&self, key: &str, current: f32, min: f32, max: f32) -> f32 {
        let Some(value) = self.1.get(key) else {
            return current;
        };
        match value
            .as_float()
            .or_else(|| value.as_integer().map(|n| n as f64))
            .map(|n| n as f32)
            .filter(|n| (min..=max).contains(n))
        {
            Some(n) => n,
            None => {
                log::warn!(
                    "Theme {}.{key} must be a number from {min} to {max}; ignored",
                    self.0
                );
                current
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(source: &str) -> Screens {
        let mut table: Table = toml::from_str(source).unwrap();
        flatten(&mut table);
        Screens::read(&table, Screens::default())
    }
    #[test]
    fn tables_read_their_keys_and_keep_the_rest() {
        let own = read(
            "[toast]\nposition = 'top'\nwidth = 0.7\nicon = 'square'\nstyle = 'glass'\n\
             [hints]\nstyle = 'capsule'\nposition = 'middle'\n\
             [status]\nbattery = false\nclock = 'text'\n\
             [dialog]\nbuttons = 'row'\nicon = true\nwidth = 9",
        );
        assert_eq!(
            own.toast,
            ToastStyle {
                position: Corner::Top,
                width: 0.7,
                icon: Badge::Square,
                glass: true
            }
        );
        assert_eq!(own.hints.style, HintsStyle::Capsule);
        assert_eq!(own.hints.position, Side::Right, "wrong value: warned, kept");
        assert!(!own.status.battery && !own.status.mono && own.status.radios);
        assert!(own.dialog.buttons_row && own.dialog.icon);
        assert_eq!(own.dialog.width, Some(1.), "out of range: the launcher's");
        assert_eq!(read(""), Screens::default());
    }
    #[test]
    fn the_settings_table_reads_its_layout_icons_groups_and_colours() {
        let plain = read("").settings;
        assert_eq!(plain, SettingsStyle::default());
        assert!(!plain.themed() && !plain.light_focus());
        let own = read(
            "[settings]\nlayout = 'sidebar'\nicon = 'tile'\ngroups = 'inset'\nrow = 0.08\n\
             title = 'below'\n\
             sidebar = { x = 0.03, w = 0.4, style = 'plain', title = 0.05 }\n\
             [settings.icon_colors]\nnetwork = '#112233'\nkitchen = '#FFFFFF'",
        )
        .settings;
        assert!(own.sidebar && own.tiles && own.inset && own.themed() && own.light_focus());
        assert!(own.title_below);
        assert_eq!(own.row, 0.08);
        assert_eq!(
            own.bar,
            SidebarStyle {
                x: 0.03,
                w: 0.4,
                glass: false,
                title: 0.05
            }
        );
        assert_eq!(own.tile(0), rgb(0x11, 0x22, 0x33));
        assert_eq!(own.tile(1), TILE_COLORS[1], "not given: the launcher's");
        let wrong = read("[settings]\nlayout = 'grid'\ngroups = 3\nsidebar = { w = 2 }").settings;
        assert_eq!(
            wrong,
            SettingsStyle::default(),
            "wrong values: warned, kept"
        );
    }
    #[test]
    fn every_screen_table_is_a_known_theme_key() {
        for table in TABLES {
            assert!(crate::theme::KNOWN_KEYS.contains(table), "{table}");
        }
    }
    #[test]
    fn the_osd_table_reads_its_shape_edge_colours_and_radius() {
        assert_eq!(read("").osd, OsdStyle::default());
        let own = read(
            "[osd]\nstyle = 'capsule'\nposition = 'left'\nfill = '#FFFFFF'\n\
             track = '#00000080'\nradius = 0.05",
        );
        assert_eq!(own.osd.style, OsdShape::Capsule);
        assert_eq!(own.osd.position, OsdEdge::Left);
        assert_eq!(
            own.osd.fill,
            Some(Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 255
            })
        );
        assert_eq!(own.osd.track.map(|c| c.a), Some(0x80));
        assert_eq!(own.osd.radius, Some(0.05));
        let wrong = read("[osd]\nstyle = 'round'\nposition = 'middle'\nfill = 'white'\nradius = 3");
        assert_eq!(wrong.osd.style, OsdShape::Pill, "wrong value: warned, kept");
        assert_eq!(wrong.osd.position, OsdEdge::Right);
        assert_eq!(wrong.osd.fill, None);
        assert_eq!(
            wrong.osd.radius,
            Some(16. / 480.),
            "out of range: the pill's"
        );
    }
    #[test]
    fn screens_take_their_places() {
        let own = read(
            "[start]\ntiles = { x = 0.05, y = 0.3, w = 0.9, h = 0.25 }\n\
             shortcuts = { x = 0.05, y = 0.6, w = 0.9, h = 0.12 }\n\
             clock = { x = 0.05, y = 0.05, w = 0.9, h = 0.15, size = 0.12 }\n\
             [menu]\nrect = { x = 0.3, y = 0.1, w = 0.4, h = 0.8 }\n\
             [search]\nfield = { x = 0.04, y = 0.1, w = 0.92, h = 0.09 }\n\
             results = { x = 0.04, y = 0.22, w = 0.92, h = 0.3 }\ndock = 'results'\n\
             [resume]\nprogress = { x = 0.3, y = 0.7, w = 0.4, h = 0.01 }\n\
             backdrop = { blur = 0, opacity = 0.8 }",
        );
        assert_eq!(own.start.tiles.map(|r| r.h), Some(0.25));
        assert_eq!(own.start.shortcuts.map(|r| r.y), Some(0.6));
        assert_eq!(own.start.clock_at.map(|r| r.h), Some(0.15));
        assert_eq!(own.start.clock, 0.12, "the hour's size inside its place");
        assert_eq!(own.menu.rect.map(|r| r.w), Some(0.4));
        assert_eq!(own.search.field_at.map(|r| r.h), Some(0.09));
        assert_eq!(own.search.results.map(|r| r.h), Some(0.3));
        assert!(own.search.dock_results);
        assert_eq!(own.resume.progress.map(|r| r.w), Some(0.4));
        assert_eq!(
            own.resume.backdrop,
            Some(Backdrop {
                blur: 0.,
                opacity: 0.8
            })
        );
        // A number is still the hour's size; nothing else moves.
        let plain = read("[start]\nclock = 0.1\n[resume]\nbackdrop = 'blur'");
        assert_eq!((plain.start.clock, plain.start.clock_at), (0.1, None));
        assert_eq!(plain.resume.backdrop, None, "wrong type: warned, left out");
        assert_eq!(read("").search, SearchStyle::default());
    }
    #[test]
    fn the_status_pill_takes_a_box_inside_the_screen() {
        let own = read("[status]\nrect = { x = 0.5, y = 0.02, w = 0.45, h = 0.08 }");
        assert_eq!(own.status.rect.map(|r| r.w), Some(0.45));
        assert_eq!(
            read("[status]\nrect = { x = 0.8, y = 0, w = 0.5, h = 0.1 }")
                .status
                .rect,
            None,
            "off the screen: the header's"
        );
    }
    #[test]
    fn the_achievements_progress_is_a_place_a_word_or_both() {
        let own = read("[achievements]\nprogress = 'ring'\nrow = 0.1");
        assert!(own.achievements.ring);
        assert_eq!(own.achievements.row, 0.1);
        let mut table: Table = toml::from_str(
            "[achievements]\nprogress = { x = 0.1, y = 0.1, w = 0.8, h = 0.2, style = 'ring' }",
        )
        .unwrap();
        flatten(&mut table);
        assert_eq!(table["achievements_progress"]["h"].as_float(), Some(0.2));
        assert!(Screens::read(&table, Screens::default()).achievements.ring);
        let mut table: Table = toml::from_str("[achievements]\nprogress = 'ring'").unwrap();
        flatten(&mut table);
        assert!(
            !table.contains_key("achievements_progress"),
            "a word is no place"
        );
    }
    #[test]
    fn screen_tables_place_what_the_loose_keys_did() {
        let mut table: Table = toml::from_str(
            "details_shot = { x = 0.1, y = 0.1, w = 0.2, h = 0.2 }\n\
             [details]\nshot = { x = 0.3, y = 0.1, w = 0.2, h = 0.2 }\n\
             [saves]\npreview = { x = 0.5, y = 0.2, w = 0.4, h = 0.5 }",
        )
        .unwrap();
        flatten(&mut table);
        assert_eq!(
            table["details_shot"]["x"].as_float(),
            Some(0.3),
            "the table wins"
        );
        assert_eq!(table["saves_card"]["w"].as_float(), Some(0.4));
    }
}
