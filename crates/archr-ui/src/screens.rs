//! One table per screen (docs/temas.md, Uma tabela por tela): `[toast]`,
//! `[hints]`, `[status]`, `[dialog]` and the tables that gather the older
//! loose keys (`[details]`, `[achievements]`, `[saves]`, `[shots]`,
//! `[switcher]`, `[resume]`, `[start]`). Every key is optional; what a
//! theme leaves out is inherited, and the defaults draw as the launcher
//! always has.
use crate::theme::Rect01;
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
}
impl Default for Hints {
    fn default() -> Self {
        Self {
            style: HintsStyle::Bar,
            position: Side::Right,
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
}
impl Default for StartStyle {
    fn default() -> Self {
        Self {
            control: false,
            radius: 26. / 480.,
            shortcut: 52. / 480.,
            clock: 44. / 480.,
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
}
impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            context: false,
            anchor_focus: true,
            width: 300. / 480.,
            row: 38. / 480.,
            radius: 26. / 480.,
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
        "hints" => &["style", "position"],
        "status" => &["style", "battery", "radios", "clock", "rect"],
        "dialog" => &["width", "radius", "icon", "buttons", "style"],
        "start" => &["style", "radius", "shortcut", "clock"],
        "menu" => &["style", "anchor", "width", "row", "radius"],
        "details" => &["style", "shelf"],
        "achievements" => &["style", "row", "gap", "badge", "badge_radius"],
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
            own.start.clock = r.number("clock", own.start.clock, 0.04, 0.16);
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
        own
    }
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
