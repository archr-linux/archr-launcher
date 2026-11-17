//! One table per screen (docs/temas.md, Uma tabela por tela): `[toast]`,
//! `[hints]`, `[status]`, `[dialog]` and the tables that gather the older
//! loose keys (`[details]`, `[achievements]`, `[saves]`, `[shots]`,
//! `[switcher]`, `[resume]`, `[start]`). Every key is optional; what a
//! theme leaves out is inherited, and the defaults draw as the launcher
//! always has.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusStyle {
    pub glass: bool,
    pub battery: bool,
    pub radios: bool,
    /// The clock in the monospaced face (the default) or the semibold one.
    pub mono: bool,
}
impl Default for StatusStyle {
    fn default() -> Self {
        Self {
            glass: false,
            battery: true,
            radios: true,
            mono: true,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Screens {
    pub toast: ToastStyle,
    pub hints: Hints,
    pub status: StatusStyle,
    pub dialog: DialogStyle,
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
];

/// `table` with each screen table's rectangle under its old loose key, so
/// both spellings are read alike; the table's wins when both are given.
pub(crate) fn flatten(table: &mut Table) {
    for (screen, key, old) in RECTS {
        let rect = table
            .get(*screen)
            .and_then(Value::as_table)
            .and_then(|t| t.get(*key))
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
        "status" => &["style", "battery", "radios", "clock"],
        "dialog" => &["width", "radius", "icon", "buttons", "style"],
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
