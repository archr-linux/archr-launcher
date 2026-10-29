//! Where a theme puts things: the views (`[view.<id>]` in theme.toml) that
//! place the list, art and text of the systems and games screens.
//!
//! Every key is optional. The "lista" view is the classic layout, built from
//! the theme's top-level geometry; any other view starts from it and takes
//! the builtin theme's table for that id, then the theme's own. Values out of
//! range are limited, unknown keys are logged and ignored.
pub use crate::text::Align;
use crate::theme::{Rect01, Theme};
use toml::{Table, Value};

/// Id of the classic list layout, which every theme offers.
pub const LISTA: &str = "lista";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}
/// How the selected entry is marked: a filled pill, a bar beside the text,
/// or (horizontal lists) a tile with the system art that grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Pill,
    Bar,
    Tile,
}
/// The list scrolls only when the selection reaches an edge, or keeps it centered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scroll {
    Edge,
    Center,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    Contain,
    Cover,
}
/// The system name with its counter, or a single muted line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Header {
    Full,
    Compact,
}
/// Rectangles use screen fractions (x/w of the width, y/h of the height);
/// heights, gaps, radii and font sizes are fractions of the screen height.
#[derive(Clone, Debug, PartialEq)]
pub struct List {
    pub rect: Rect01,
    pub row: f32,
    pub selected: f32,
    pub gap: f32,
    pub font: f32,
    pub selected_font: f32,
    pub radius: f32,
    pub axis: Axis,
    pub mark: Mark,
    pub scroll: Scroll,
    /// Text drawn before the selected entry, blinking; empty for none.
    pub cursor: String,
}
/// A raised card (the systems panel, the frame behind a cover).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub rect: Rect01,
    pub radius: f32,
}
/// The console picture or the game cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Art {
    pub rect: Rect01,
    pub fit: Fit,
    pub radius: f32,
}
/// The cover again, filling the screen behind everything, faded into the
/// background from the left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub opacity: f32,
    /// Fraction of the width covered by plain background.
    pub solid: f32,
    /// Background opacity at the right edge.
    pub edge: f32,
}
/// The system name and its details.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    pub rect: Rect01,
    pub title: f32,
    pub meta: f32,
    pub align: Align,
    /// Stacked up from the bottom of the box instead of down from its top.
    pub bottom: bool,
    /// Details on one line under the name instead of maker above, counts below.
    pub inline: bool,
}
/// Text box: year and players on the left, last played on the right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meta {
    pub rect: Rect01,
    pub size: f32,
}
/// A theme picture placed on a screen: still, or a strip of frames side by
/// side that plays `fps` times a second.
#[derive(Clone, Debug, PartialEq)]
pub struct Decor {
    /// Path inside the theme folder.
    pub image: String,
    pub x: f32,
    pub y: f32,
    /// Reference pixels (480 high) per image pixel.
    pub scale: f32,
    pub frames: u32,
    pub fps: f32,
    /// Behind the list and panels instead of over them.
    pub back: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub header: Header,
    /// Systems screen title: the spaced brand ("ARCHR") or the screen's
    /// name ("Systems", per language).
    pub brand: bool,
    pub status: bool,
    pub divider: bool,
    pub summary: bool,
    pub list: List,
    pub panel: Option<Panel>,
    pub art: Option<Art>,
    pub backdrop: Option<Backdrop>,
    pub info: Option<Info>,
    pub meta: Option<Meta>,
    pub dots: Option<Rect01>,
    pub decor: Vec<Decor>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: String,
    pub systems: Layout,
    pub games: Layout,
}

/// A display name, the same everywhere or per language (`en`, `pt_BR`, ...).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names(Vec<(String, String)>);
impl Names {
    pub fn plain(name: &str) -> Self {
        Self(vec![(String::new(), name.into())])
    }
    pub fn parse(value: Option<&Value>, at: &str) -> Self {
        match value {
            None => Self::default(),
            Some(Value::String(name)) => Self::plain(name),
            Some(Value::Table(names)) => Self(
                names
                    .iter()
                    .filter_map(|(language, name)| match name.as_str() {
                        Some(name) => Some((language.clone(), name.to_owned())),
                        None => {
                            log::warn!("Theme {at}.name.{language} is not text; ignored");
                            None
                        }
                    })
                    .collect(),
            ),
            Some(_) => {
                log::warn!("Theme {at}.name is neither text nor a table; ignored");
                Self::default()
            }
        }
    }
    /// Each text at most `limit` characters.
    pub(crate) fn truncated(self, limit: usize) -> Self {
        Self(
            self.0
                .into_iter()
                .map(|(language, text)| (language, text.chars().take(limit).collect()))
                .collect(),
        )
    }
    /// The name for `language` (`pt_BR` also matches `pt`), then English,
    /// then any; `None` when the theme gave none.
    pub fn get(&self, language: &str) -> Option<&str> {
        let base = language.split('_').next().unwrap_or(language);
        [language, base, "", "en"]
            .iter()
            .find_map(|wanted| self.0.iter().find(|(l, _)| l == wanted))
            .or(self.0.first())
            .map(|(_, name)| name.as_str())
    }
}

/// Screen fractions from pixels of the 640×480 reference screen.
const fn w(px: f32) -> f32 {
    px / 640.
}
const fn h(px: f32) -> f32 {
    px / 480.
}

impl View {
    /// The classic layout, from the theme's top-level geometry.
    pub fn lista(t: &Theme) -> Self {
        let top = t.header_height + h(6.);
        let bottom = 1. - t.footer_height - h(6.);
        let margin = t.margin * 0.75;
        let panel = Rect01 {
            x: 1. - margin - w(250.),
            y: top,
            w: w(250.),
            h: 1. - t.footer_height - h(8.) - top,
        };
        let list = |rect, row, selected, font| List {
            rect,
            row,
            selected,
            gap: h(2.),
            font,
            selected_font: font + h(1.),
            radius: t.radius_item,
            axis: Axis::Vertical,
            mark: Mark::Pill,
            scroll: Scroll::Edge,
            cursor: String::new(),
        };
        let column = |x, width| Rect01 {
            x,
            y: top,
            w: width,
            h: bottom - top,
        };
        let base = |list| Layout {
            header: Header::Full,
            brand: true,
            status: true,
            divider: true,
            summary: true,
            list,
            panel: None,
            art: None,
            backdrop: None,
            info: None,
            meta: None,
            dots: None,
            decor: Vec::new(),
        };
        let cover = t.cover;
        Self {
            id: LISTA.into(),
            systems: Layout {
                panel: Some(Panel {
                    rect: panel,
                    radius: h(14.),
                }),
                art: Some(Art {
                    rect: Rect01 {
                        x: panel.x + w(20.),
                        y: panel.y + h(46.),
                        w: w(210.),
                        h: h(160.),
                    },
                    fit: Fit::Contain,
                    radius: 0.,
                }),
                info: Some(Info {
                    rect: Rect01 {
                        x: panel.x + w(16.),
                        y: panel.y,
                        w: panel.w - w(32.),
                        h: panel.h - h(14.),
                    },
                    title: h(22.),
                    meta: h(12.),
                    align: Align::Left,
                    bottom: true,
                    inline: false,
                }),
                ..base(list(
                    column(margin, panel.x - w(14.) - margin),
                    t.row_height,
                    t.selected_height,
                    t.font_size,
                ))
            },
            games: Layout {
                panel: Some(Panel {
                    rect: cover,
                    radius: t.radius_card * 0.75,
                }),
                art: Some(Art {
                    rect: cover,
                    fit: Fit::Contain,
                    radius: (t.radius_card * 0.75 - h(1.)).max(0.),
                }),
                meta: Some(Meta {
                    rect: Rect01 {
                        x: cover.x,
                        y: (cover.y + cover.h + h(9.)).min(1. - h(20.)),
                        w: cover.w,
                        h: h(20.),
                    },
                    size: h(12.),
                }),
                ..base(list(
                    column(margin, t.list_width),
                    t.game_row_height,
                    t.game_selected_height,
                    t.font_size - h(1.),
                ))
            },
        }
    }
    pub fn screen(&self, games: bool) -> &Layout {
        if games {
            &self.games
        } else {
            &self.systems
        }
    }
    /// Lays the `[view.<id>]` table over this view.
    pub(crate) fn overlay(&mut self, table: &Table, at: &str) {
        known(table, at, &["name", "systems", "games"]);
        for (key, layout) in [("systems", &mut self.systems), ("games", &mut self.games)] {
            if let Some(table) = sub(table, at, key) {
                layout.overlay(table, &format!("{at}.{key}"));
            }
        }
    }
}

impl Layout {
    fn overlay(&mut self, t: &Table, at: &str) {
        known(
            t,
            at,
            &[
                "header", "title", "status", "divider", "summary", "list", "panel", "art",
                "backdrop", "info", "meta", "dots", "decor",
            ],
        );
        let r = Reader { table: t, at };
        self.header = r.pick(
            "header",
            self.header,
            &[("full", Header::Full), ("compact", Header::Compact)],
        );
        self.brand = r.pick("title", self.brand, &[("brand", true), ("screen", false)]);
        self.status = r.flag("status", self.status);
        self.divider = r.flag("divider", self.divider);
        self.summary = r.flag("summary", self.summary);
        if let Some(t) = sub(t, at, "list") {
            self.list.overlay(t, &format!("{at}.list"));
        }
        element(t, at, "panel", &mut self.panel, |p, r| {
            p.rect = r.rect(p.rect);
            p.radius = r.number("radius", p.radius, 0., 0.5);
        });
        element(t, at, "art", &mut self.art, |a, r| {
            a.rect = r.rect(a.rect);
            a.fit = r.pick(
                "fit",
                a.fit,
                &[("contain", Fit::Contain), ("cover", Fit::Cover)],
            );
            a.radius = r.number("radius", a.radius, 0., 0.5);
        });
        element(t, at, "backdrop", &mut self.backdrop, |b, r| {
            b.opacity = r.number("opacity", b.opacity, 0., 1.);
            b.solid = r.number("solid", b.solid, 0., 1.);
            b.edge = r.number("edge", b.edge, 0., 1.);
        });
        element(t, at, "info", &mut self.info, |i, r| {
            i.rect = r.rect(i.rect);
            i.title = r.number("title", i.title, 0.01, 0.3);
            i.meta = r.number("meta", i.meta, 0.01, 0.3);
            i.align = r.pick(
                "align",
                i.align,
                &[
                    ("left", Align::Left),
                    ("center", Align::Center),
                    ("right", Align::Right),
                ],
            );
            i.bottom = r.pick("valign", i.bottom, &[("top", false), ("bottom", true)]);
            i.inline = r.pick("details", i.inline, &[("stacked", false), ("inline", true)]);
        });
        element(t, at, "meta", &mut self.meta, |m, r| {
            m.rect = r.rect(m.rect);
            m.size = r.number("size", m.size, 0.01, 0.3);
        });
        element(t, at, "dots", &mut self.dots, |d, r| *d = r.rect(*d));
        match t.get("decor") {
            None => {}
            Some(Value::Array(items)) => {
                self.decor = items
                    .iter()
                    .enumerate()
                    .filter_map(|(n, item)| decor(item, &format!("{at}.decor[{n}]")))
                    .collect();
            }
            Some(_) => log::warn!("Theme {at}.decor must be a list of tables; ignored"),
        }
    }
}

/// One `decor` entry; None (with a warning) when its image path is unsafe.
fn decor(item: &Value, at: &str) -> Option<Decor> {
    let Some(t) = item.as_table() else {
        log::warn!("Theme {at} must be a table; ignored");
        return None;
    };
    known(
        t,
        at,
        &["image", "x", "y", "scale", "frames", "fps", "layer"],
    );
    let image = t.get("image").and_then(Value::as_str).filter(|path| {
        !path.is_empty()
            && std::path::Path::new(path)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
    });
    let Some(image) = image else {
        log::warn!("Theme {at}.image must be a path inside the theme; ignored");
        return None;
    };
    let r = Reader { table: t, at };
    Some(Decor {
        image: image.to_owned(),
        x: r.number("x", 0., 0., 1.),
        y: r.number("y", 0., 0., 1.),
        scale: r.number("scale", 1., 0.25, 8.),
        frames: r.number("frames", 1., 1., 64.) as u32,
        fps: r.number("fps", 0., 0., 30.),
        back: r.pick("layer", false, &[("front", false), ("back", true)]),
    })
}

impl List {
    fn overlay(&mut self, t: &Table, at: &str) {
        let r = Reader { table: t, at };
        known(
            t,
            at,
            &[
                "x",
                "y",
                "w",
                "h",
                "row",
                "selected",
                "gap",
                "font",
                "selected_font",
                "radius",
                "axis",
                "mark",
                "scroll",
                "cursor",
            ],
        );
        self.rect = r.rect(self.rect);
        self.row = r.number("row", self.row, 0.02, 1.);
        self.selected = r.number("selected", self.selected, self.row, 1.);
        self.gap = r.number("gap", self.gap, 0., 0.5);
        self.font = r.number("font", self.font, 0.01, 0.3);
        self.selected_font = r.number("selected_font", self.selected_font, 0.01, 0.3);
        self.radius = r.number("radius", self.radius, 0., 0.5);
        self.axis = r.pick(
            "axis",
            self.axis,
            &[
                ("vertical", Axis::Vertical),
                ("horizontal", Axis::Horizontal),
            ],
        );
        self.mark = r.pick(
            "mark",
            self.mark,
            &[
                ("pill", Mark::Pill),
                ("bar", Mark::Bar),
                ("tile", Mark::Tile),
            ],
        );
        self.scroll = r.pick(
            "scroll",
            self.scroll,
            &[("edge", Scroll::Edge), ("center", Scroll::Center)],
        );
        match t.get("cursor") {
            None => {}
            Some(Value::String(cursor)) => self.cursor = cursor.chars().take(4).collect(),
            Some(_) => log::warn!("Theme {at}.cursor must be text; ignored"),
        }
        // A tile list is always a centered horizontal row.
        if self.mark == Mark::Tile {
            self.axis = Axis::Horizontal;
        }
    }
}

fn known(table: &Table, at: &str, keys: &[&str]) {
    for key in table.keys().filter(|k| !keys.contains(&k.as_str())) {
        log::warn!("Unknown theme key {at}.{key}; ignored");
    }
}
fn sub<'t>(table: &'t Table, at: &str, key: &str) -> Option<&'t Table> {
    let value = table.get(key)?;
    let table = value.as_table();
    if table.is_none() {
        log::warn!("Theme {at}.{key} must be a table; ignored");
    }
    table
}
/// An optional element: `false` removes it, a table adjusts it (starting
/// from a neutral default when the view had none).
fn element<T: Default>(
    table: &Table,
    at: &str,
    key: &str,
    slot: &mut Option<T>,
    overlay: impl FnOnce(&mut T, &Reader),
) {
    match table.get(key) {
        None => {}
        Some(Value::Boolean(false)) => *slot = None,
        Some(Value::Table(t)) => {
            let at = format!("{at}.{key}");
            let element = slot.get_or_insert_with(T::default);
            overlay(element, &Reader { table: t, at: &at });
        }
        Some(_) => log::warn!("Theme {at}.{key} must be a table or false; ignored"),
    }
}
struct Reader<'a> {
    table: &'a Table,
    at: &'a str,
}
impl Reader<'_> {
    fn number(&self, key: &str, current: f32, min: f32, max: f32) -> f32 {
        let Some(value) = self.table.get(key) else {
            return current;
        };
        match value
            .as_float()
            .or_else(|| value.as_integer().map(|n| n as f64))
            .filter(|n| n.is_finite())
        {
            Some(n) => {
                let limited = (n as f32).clamp(min, max.max(min));
                if limited != n as f32 {
                    log::warn!(
                        "Theme {}.{key} = {n} is out of range; using {limited}",
                        self.at
                    );
                }
                limited
            }
            None => {
                log::warn!("Theme {}.{key} must be a number; ignored", self.at);
                current
            }
        }
    }
    fn flag(&self, key: &str, current: bool) -> bool {
        match self.table.get(key) {
            None => current,
            Some(Value::Boolean(b)) => *b,
            Some(_) => {
                log::warn!("Theme {}.{key} must be true or false; ignored", self.at);
                current
            }
        }
    }
    fn pick<T: Copy>(&self, key: &str, current: T, options: &[(&str, T)]) -> T {
        let Some(value) = self.table.get(key) else {
            return current;
        };
        let found = value.as_str().and_then(|v| {
            options
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(v))
        });
        match found {
            Some((_, choice)) => *choice,
            None => {
                let names: Vec<_> = options.iter().map(|(n, _)| *n).collect();
                log::warn!("Theme {}.{key} must be one of {names:?}; ignored", self.at);
                current
            }
        }
    }
    /// `x`, `y`, `w` and `h`, kept on screen.
    fn rect(&self, current: Rect01) -> Rect01 {
        let x = self.number("x", current.x, 0., 1.);
        let y = self.number("y", current.y, 0., 1.);
        Rect01 {
            x,
            y,
            w: self.number("w", current.w, 0.001, 1. - x),
            h: self.number("h", current.h, 0.001, 1. - y),
        }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            rect: Rect01 {
                x: 0.25,
                y: 0.25,
                w: 0.5,
                h: 0.5,
            },
            radius: h(14.),
        }
    }
}
impl Default for Art {
    fn default() -> Self {
        Self {
            rect: Panel::default().rect,
            fit: Fit::Contain,
            radius: h(12.),
        }
    }
}
impl Default for Backdrop {
    fn default() -> Self {
        Self {
            opacity: 0.55,
            solid: 0.42,
            edge: 0.6,
        }
    }
}
impl Default for Info {
    fn default() -> Self {
        Self {
            rect: Panel::default().rect,
            title: h(22.),
            meta: h(12.),
            align: Align::Left,
            bottom: false,
            inline: false,
        }
    }
}
impl Default for Meta {
    fn default() -> Self {
        Self {
            rect: Panel::default().rect,
            size: h(12.),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(source: &str) -> View {
        let mut view = View::lista(&Theme::builtin());
        view.overlay(&toml::from_str(source).unwrap(), "test");
        view
    }
    #[test]
    fn the_classic_layout_matches_the_grafite_pixels() {
        let lista = View::lista(&Theme::builtin());
        let px = |r: Rect01| crate::render::pixels(r, 640, 480);
        let list = px(lista.systems.list.rect);
        assert_eq!(
            (list.x(), list.y(), list.width(), list.height()),
            (16, 50, 344, 376)
        );
        let panel = px(lista.systems.panel.unwrap().rect);
        assert_eq!(
            (panel.x(), panel.y(), panel.width(), panel.height()),
            (374, 50, 250, 374)
        );
        let art = px(lista.systems.art.unwrap().rect);
        assert_eq!(
            (art.x(), art.y(), art.width(), art.height()),
            (394, 96, 210, 160)
        );
        assert_eq!(px(lista.games.list.rect).width(), 378);
        assert_eq!(lista.games.art.unwrap().rect, Theme::builtin().cover);
    }
    #[test]
    fn keys_inherit_one_by_one_and_bad_values_are_limited_or_ignored() {
        let base = View::lista(&Theme::builtin());
        let changed = view(
            "[games.list]\nx = 0.5\nw = 0.9\nrow = 'big'\nmark = 'bar'\nfuture = 1\n\
             [games]\npanel = false\nheader = 'odd'\n[systems.info]\nalign = 'center'",
        );
        let list = &changed.games.list;
        assert_eq!((list.rect.x, list.rect.w), (0.5, 0.5), "kept on screen");
        assert_eq!(
            list.row, base.games.list.row,
            "wrong type keeps the inherited value"
        );
        assert_eq!(list.mark, Mark::Bar);
        assert_eq!(list.rect.y, base.games.list.rect.y);
        assert_eq!(changed.games.panel, None);
        assert_eq!(changed.games.header, Header::Full);
        let info = changed.systems.info.unwrap();
        assert_eq!(info.align, Align::Center);
        assert_eq!(info.title, base.systems.info.unwrap().title);
        assert_eq!(changed.systems.list, base.systems.list);
    }
    #[test]
    fn the_cursor_is_short_text_and_off_by_default() {
        assert_eq!(view("").games.list.cursor, "");
        assert_eq!(
            view("[games.list]\ncursor = '>>>>>>'").games.list.cursor,
            ">>>>"
        );
        assert_eq!(view("[games.list]\ncursor = 1").games.list.cursor, "");
    }
    #[test]
    fn decor_lists_replace_the_inherited_one_and_skip_bad_entries() {
        let v = view(
            "[systems]\ndecor = [{ image = 'decor/palm.png', x = 0.9, y = 0.1, scale = 2, frames = 4, fps = 6, layer = 'back' },\
             { image = '../x.png' }, { x = 1 }, { image = 'shell.png', x = 7 }]",
        );
        let d = &v.systems.decor;
        assert_eq!(d.len(), 2);
        assert_eq!(
            (
                d[0].image.as_str(),
                d[0].x,
                d[0].scale,
                d[0].frames,
                d[0].fps,
                d[0].back
            ),
            ("decor/palm.png", 0.9, 2., 4, 6., true)
        );
        assert_eq!(
            (d[1].x, d[1].frames, d[1].back),
            (1., 1, false),
            "kept on screen"
        );
        assert!(v.games.decor.is_empty());
        let mut cleared = v.clone();
        cleared.overlay(&toml::from_str("[systems]\ndecor = []").unwrap(), "t");
        assert!(cleared.systems.decor.is_empty());
    }
    #[test]
    fn the_systems_title_is_the_brand_unless_the_view_names_the_screen() {
        assert!(view("").systems.brand);
        assert!(!view("[systems]\ntitle = 'screen'").systems.brand);
        assert!(view("[systems]\ntitle = 'odd'").systems.brand);
    }
    #[test]
    fn tiles_are_horizontal() {
        let tiles = view("[systems.list]\nmark = 'tile'");
        assert_eq!(tiles.systems.list.axis, Axis::Horizontal);
    }
    #[test]
    fn names_follow_the_language_then_english() {
        let names = Names::parse(
            Some(
                &toml::from_str::<Table>("n = { en = 'List', pt_BR = 'Lista', ru = 'Список' }")
                    .unwrap()["n"],
            ),
            "t",
        );
        assert_eq!(names.get("pt_BR"), Some("Lista"));
        assert_eq!(names.get("ru"), Some("Список"));
        assert_eq!(names.get("es"), Some("List"));
        assert_eq!(Names::plain("Grafite").get("ru"), Some("Grafite"));
        assert_eq!(Names::default().get("en"), None);
    }
}
