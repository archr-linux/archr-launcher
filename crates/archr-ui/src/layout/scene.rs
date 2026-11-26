//! Free screens (docs/temas.md, Telas livres): a systems or games screen
//! described as a list of elements (`[[view.<id>.<screen>.el]]`) drawn in
//! the order of their `z`, instead of the view's fixed places.
//!
//! The elements are few on purpose: pictures (`image`, `frame`), text
//! (`text`, `fields`), boxes (`rect`, `panel`), the game's stars (`rating`)
//! and the launcher's own pieces (`list`, `hints`, `status`). Every value is read once, when the view is laid
//! out; data (`${system}`, `${game.name}`...) is filled in as each frame is
//! drawn. A bad value is logged and left out, never fatal.
use super::{known, Align, List, Panel, PanelStyle, Reader};
use crate::theme::{Aspect, Gradient, Rect01, Rgba};
use toml::{Table, Value};

/// A free screen: its elements, in drawing order (`z`, then file order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub elements: Vec<Element>,
    /// Font files the elements name (`font = "fonts/x.ttf"`), inside the
    /// theme folder; `Font::File(n)` is the n-th.
    pub fonts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: String,
    pub z: i32,
    /// Its box, the anchor already applied: the top left corner. It may
    /// reach past the screen; what is outside is not seen.
    pub rect: Rect01,
    pub opacity: f32,
    /// Degrees, clockwise, about `pivot` (fractions of the box).
    pub rotate: f32,
    pub pivot: (f32, f32),
    /// Drawn only on these systems (`when.system`); empty: on all.
    pub systems: Vec<String>,
    /// Drawn only when every one of these has a value (`when.has`).
    pub has: Vec<Data>,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Image(Image),
    Text(Text),
    /// A plain, rounded or shaded box (`kind = "rect"`).
    Fill(Fill),
    /// A theme picture as a nine-slice frame over the box.
    Frame(Slices),
    /// The screen's list (the view's `list`, placed here): it takes the
    /// d-pad as always. Its geometry lives in the layout's `list`.
    List(ListInk),
    Hints(Hints),
    /// Clock, battery and radios: the launcher's pill, or plain in `color`.
    Status(Option<Rgba>),
    Panel(Panel),
    /// The game's rating as five stars, in halves.
    Rating(Stars),
    /// A label and value per line; a line without a value is left out.
    Fields(Fields),
}

/// Five stars: the theme's pictures (`star`, and `star_half`,
/// `star_empty` if it has them) or the launcher's star.
#[derive(Clone, Debug, PartialEq)]
pub struct Stars {
    pub full: Option<Template>,
    pub half: Option<Template>,
    pub empty: Option<Template>,
    /// Tint of the full stars; the theme's text for the launcher's star.
    pub color: Option<Rgba>,
    /// Tint of an empty star drawn from the full one; `color` when None.
    pub empty_color: Option<Rgba>,
    /// Opacity of an empty star drawn from the full one.
    pub empty_opacity: f32,
    /// A star's side and the room between two, fractions of the screen
    /// height; None: the box's height, a fifth of the star.
    pub size: Option<f32>,
    pub spacing: Option<f32>,
    pub align: Align,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fields {
    pub rows: Vec<Field>,
    /// Lines without a value are left out and the next ones move up.
    pub hide_empty: bool,
    /// A line's height, a fraction of the screen height; None: the box
    /// shared by the lines.
    pub row: Option<f32>,
    /// How labels and values are written (their `text` unused). A label
    /// without `label_color` is the theme's `muted`.
    pub label: Text,
    pub value: Text,
    /// A thin line under each line.
    pub divider: Option<Rgba>,
    /// For the lines that show the rating.
    pub stars: Stars,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub label: Template,
    /// None: the game's rating, as stars.
    pub value: Option<Template>,
}

/// What a picture shows: the first source that exists.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub src: Vec<Template>,
    pub fit: ImageFit,
    /// Horizontal place of a contained picture narrower than its box.
    pub align: Align,
    pub tint: Option<Rgba>,
    /// Corner radius, a fraction of the screen height.
    pub radius: f32,
    /// Drawn instead when no source exists.
    pub fallback: Option<Box<Text>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFit {
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub text: Template,
    pub font: Font,
    /// A fraction of the screen height.
    pub size: f32,
    /// The theme's `text` when None.
    pub color: Option<Rgba>,
    pub align: Align,
    pub valign: VAlign,
    pub upper: bool,
    pub shadow: Option<Shadow>,
    /// Several lines; the last one that fits ends in "…".
    pub wrap: bool,
    /// One line too long for its box gets smaller, down to 60 % of `size`,
    /// before it ends in "…" (`fit_text = "shrink"`).
    pub shrink: bool,
    /// Line height, times the font size.
    pub line: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}
/// The text again under it, moved by `x` and `y` (fractions of the height).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Rgba,
    pub x: f32,
    pub y: f32,
}
/// One of the theme's faces, or a font file of the theme (`Scene::fonts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Regular,
    Semibold,
    Bold,
    Mono,
    File(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub fill: Rgba,
    pub border: Option<Rgba>,
    /// A fraction of the screen height.
    pub radius: f32,
    pub gradient: Option<Gradient>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Slices {
    pub image: Template,
    /// Image pixels kept whole: left, top, right, bottom.
    pub slice: [u32; 4],
    /// Reference pixels (480 high) per image pixel.
    pub scale: f32,
    pub tile: bool,
}

/// The list's own colors, over the theme's: rows, the selected row's fill
/// and its text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ListInk {
    pub color: Option<Rgba>,
    pub fill: Option<Rgba>,
    pub selected: Option<Rgba>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hints {
    pub align: Align,
    pub style: HintStyle,
    /// The words; the letters' ring or disc too, unless `badge` says.
    pub color: Option<Rgba>,
    pub badge: Option<Rgba>,
    /// The letter inside a filled disc.
    pub letter: Option<Rgba>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintStyle {
    /// The letter in a filled disc (the launcher's).
    Filled,
    /// The letter in a ring.
    Outline,
    /// The letter alone.
    Text,
}

/// Something the screen knows, filled in as it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    /// The system's id (`snes`); a collection's without its `@`.
    System,
    SystemName,
    SystemMaker,
    SystemYear,
    SystemCount,
    /// "213 games", in the user's language.
    SystemGames,
    SystemFavorites,
    GameName,
    GameDescription,
    GameGenre,
    GameDeveloper,
    GamePublisher,
    GameYear,
    GamePlayers,
    GameRating,
    GameLastPlayed,
    /// The game's pictures: in `src`, the files; in text, nothing.
    GameCover,
    GameScreenshot,
    /// The scraper's wheel (marquee) picture.
    GameLogo,
    /// The system's logo in the theme's `logos/` folder, any picture type,
    /// its own name or the one it shares (`megadrive` → `genesis`).
    SystemLogo,
    /// The system's picture the launcher shows: the user's, the theme's
    /// `systems/`, then the one shipped with the launcher.
    SystemImage,
    /// Position of the focused item, from 1, and how many there are.
    ListIndex,
    ListTotal,
    /// The focused item's id: the system, or the game's file name.
    Item,
    /// `classic`, `square` or `wide`.
    Aspect,
}
impl Data {
    /// `system`, `system.name`, `game.description`...
    pub fn parse(name: &str) -> Option<Self> {
        let (root, field) = name.split_once('.').unwrap_or((name, ""));
        Some(match (root, field) {
            ("system", "") => Self::System,
            ("system", "name" | "fullname") => Self::SystemName,
            ("system", "maker") => Self::SystemMaker,
            ("system", "year") => Self::SystemYear,
            ("system", "count") => Self::SystemCount,
            ("system", "games") => Self::SystemGames,
            ("system", "favorites") => Self::SystemFavorites,
            ("system", "logo") => Self::SystemLogo,
            ("system", "image") => Self::SystemImage,
            ("game", "name") => Self::GameName,
            ("game", "description") => Self::GameDescription,
            ("game", "genre") => Self::GameGenre,
            ("game", "developer") => Self::GameDeveloper,
            ("game", "publisher") => Self::GamePublisher,
            ("game", "year") => Self::GameYear,
            ("game", "players") => Self::GamePlayers,
            ("game", "rating") => Self::GameRating,
            ("game", "last_played") => Self::GameLastPlayed,
            ("game", "cover") => Self::GameCover,
            ("game", "screenshot") => Self::GameScreenshot,
            ("game", "logo" | "wheel" | "marquee") => Self::GameLogo,
            ("list", "index") => Self::ListIndex,
            ("list", "total") => Self::ListTotal,
            ("item", "") => Self::Item,
            ("aspect", "") => Self::Aspect,
            _ => return None,
        })
    }
    /// A picture rather than text.
    pub fn picture(self) -> bool {
        matches!(
            self,
            Self::GameCover
                | Self::GameScreenshot
                | Self::GameLogo
                | Self::SystemLogo
                | Self::SystemImage
        )
    }
}

/// Text with data in it, read once: `"${system.count} jogos"`. A text that
/// is only `@key` is a phrase of the launcher's catalog instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Template(pub Vec<Piece>);
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Data(Data),
    /// A catalog phrase (`@description`), in the user's language.
    Phrase(String),
}
impl Template {
    pub fn parse(source: &str, at: &str) -> Self {
        if let Some(key) = source.strip_prefix('@').filter(|k| phrase_shaped(k)) {
            return Self(vec![Piece::Phrase(key.to_owned())]);
        }
        let mut pieces = Vec::new();
        let mut rest = source;
        while let Some(start) = rest.find("${") {
            let Some(end) = rest[start..].find('}') else {
                break;
            };
            let name = &rest[start + 2..start + end];
            match Data::parse(name) {
                Some(data) => {
                    push_text(&mut pieces, &rest[..start]);
                    pieces.push(Piece::Data(data));
                }
                None => {
                    log::warn!("Theme {at}: unknown data ${{{name}}}; written as is");
                    push_text(&mut pieces, &rest[..start + end + 1]);
                }
            }
            rest = &rest[start + end + 1..];
        }
        push_text(&mut pieces, rest);
        Self(pieces)
    }
    /// The data it holds.
    pub fn data(&self) -> impl Iterator<Item = Data> + '_ {
        self.0.iter().filter_map(|piece| match piece {
            Piece::Data(data) => Some(*data),
            _ => None,
        })
    }
    /// The text, with `value` filling in data and `phrase` the catalog.
    pub fn fill(&self, value: impl Fn(Data) -> String, phrase: impl Fn(&str) -> String) -> String {
        let mut out = String::new();
        for piece in &self.0 {
            match piece {
                Piece::Text(text) => out.push_str(text),
                Piece::Data(data) => out.push_str(&value(*data)),
                Piece::Phrase(key) => out.push_str(&phrase(key)),
            }
        }
        out
    }
}
fn push_text(pieces: &mut Vec<Piece>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(Piece::Text(last)) = pieces.last_mut() {
        last.push_str(text);
    } else {
        pieces.push(Piece::Text(text.to_owned()));
    }
}
/// `description`, `details.genre`: letters, digits, `_` and dots.
fn phrase_shaped(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Where a screen element is read: the view, the screen and the shape the
/// view is laid out for.
pub(super) struct Place<'a> {
    pub view: &'a Table,
    pub screen: &'a str,
    pub at: &'a str,
    pub aspect: Aspect,
    /// Reference pixels of the screen's height (480; 640 when square):
    /// a size given as a fraction of the height becomes the list's
    /// fraction of 480.
    pub tall: f32,
}

const KINDS: &[&str] = &[
    "image", "text", "rect", "frame", "list", "hints", "status", "panel", "rating", "fields",
];
/// A rating's own keys, also read by `fields` for its stars.
const STARS: &[&str] = &[
    "star",
    "star_half",
    "star_empty",
    "color",
    "empty",
    "empty_opacity",
    "spacing",
    "align",
];
const COMMON: &[&str] = &[
    "id", "kind", "base", "rect", "anchor", "z", "opacity", "rotate", "pivot", "when",
];

impl Scene {
    /// The screen's `el` list; None (with a warning) when it has no
    /// element left, so the screen keeps its fixed places. The `list`
    /// element's geometry goes into `list`.
    pub(super) fn parse(items: &Value, place: &Place<'_>, list: &mut List) -> Option<Self> {
        let at = format!("{}.el", place.at);
        let Some(items) = items.as_array() else {
            log::warn!("Theme {at} must be a list of [[...el]] tables; ignored");
            return None;
        };
        let mut scene = Self::default();
        let mut lists = 0;
        for (n, item) in items.iter().enumerate() {
            let at = format!("{at}[{n}]");
            let Some(table) = resolve(item, place, &at, 0) else {
                continue;
            };
            // One list: a second one is turned away before it moves the first.
            if table.get("kind").and_then(Value::as_str) == Some("list") && lists > 0 {
                log::warn!("Theme {at}: a screen has one list; ignored");
                continue;
            }
            if let Some(element) = scene.element(&table, place, &at, list) {
                if matches!(element.kind, Kind::List(_)) {
                    lists += 1;
                }
                scene.elements.push(element);
            }
        }
        if scene.elements.is_empty() {
            log::warn!(
                "Theme {}: no element to draw; the screen keeps its view",
                place.at
            );
            return None;
        }
        if lists == 0 {
            log::info!(
                "Theme {}: no list element; the d-pad still moves through an unseen list",
                place.at
            );
        }
        // Stable: equal z keep the file's order.
        scene.elements.sort_by_key(|e| e.z);
        Some(scene)
    }

    fn element(
        &mut self,
        t: &Table,
        place: &Place<'_>,
        at: &str,
        list: &mut List,
    ) -> Option<Element> {
        let r = Reader { table: t, at };
        let kind = t.get("kind").and_then(Value::as_str).unwrap_or("");
        if !KINDS.contains(&kind) {
            log::warn!("Theme {at}.kind must be one of {KINDS:?}; element ignored");
            return None;
        }
        let when = Table::new();
        let when = match t.get("when") {
            None => &when,
            Some(Value::Table(when)) => when,
            Some(_) => {
                log::warn!("Theme {at}.when must be a table; ignored");
                &when
            }
        };
        known(when, &format!("{at}.when"), &["system", "aspect", "has"]);
        let aspects = names(when.get("aspect"), &format!("{at}.when.aspect"));
        for aspect in &aspects {
            if !["classic", "square", "wide"].contains(&aspect.as_str()) {
                log::warn!("Theme {at}.when.aspect: unknown shape {aspect}");
            }
        }
        if !aspects.is_empty() && !aspects.iter().any(|a| a == place.aspect.name()) {
            return None;
        }
        let has = names(when.get("has"), &format!("{at}.when.has"))
            .iter()
            .filter_map(|name| {
                let data = Data::parse(name);
                if data.is_none() {
                    log::warn!("Theme {at}.when.has: unknown data {name}; ignored");
                }
                data
            })
            .collect();
        let rect = self.rect(&r);
        let (x, y) = r.anchor();
        let rect = Rect01 {
            x: rect.x - x * rect.w,
            y: rect.y - y * rect.h,
            ..rect
        };
        let pivot = match t.get("pivot") {
            None => (0.5, 0.5),
            Some(value) => pair(value, -10., 10.).unwrap_or_else(|| {
                log::warn!("Theme {at}.pivot must be [x, y]; using the middle");
                (0.5, 0.5)
            }),
        };
        let kind = match kind {
            "image" => Kind::Image(self.image(&r)?),
            "text" => Kind::Text(self.text(&r, &[])?),
            "rect" => {
                r.known_with(&["fill", "border", "radius", "gradient"]);
                Kind::Fill(Fill {
                    fill: r.color("fill").unwrap_or(Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    }),
                    border: r.color("border"),
                    radius: r.number("radius", 0., 0., 0.5),
                    gradient: t
                        .get("gradient")
                        .and_then(|g| crate::theme::gradient(g, &format!("{at}.gradient"))),
                })
            }
            "frame" => {
                r.known_with(&["image", "slice", "scale", "tile"]);
                let image = r.path("image")?;
                let slice = match crate::theme::sides(t.get("slice"), 256.) {
                    Some(slice) => slice.map(|n| n as u32),
                    None => {
                        log::warn!(
                            "Theme {at}.slice must be a number or [l, t, r, b]; element ignored"
                        );
                        return None;
                    }
                };
                Kind::Frame(Slices {
                    image,
                    slice,
                    scale: r.number("scale", 1., 0.25, 8.),
                    tile: r.flag("tile", false),
                })
            }
            "list" => Kind::List(list_element(&r, rect, place, list)),
            "hints" => {
                r.known_with(&["align", "style", "color", "badge", "letter_color"]);
                Kind::Hints(Hints {
                    align: r.align("align", Align::Right),
                    style: r.pick(
                        "style",
                        HintStyle::Filled,
                        &[
                            ("filled", HintStyle::Filled),
                            ("outline", HintStyle::Outline),
                            ("text", HintStyle::Text),
                        ],
                    ),
                    color: r.color("color"),
                    badge: r.color("badge"),
                    letter: r.color("letter_color"),
                })
            }
            "status" => {
                r.known_with(&["color"]);
                Kind::Status(r.color("color"))
            }
            "rating" => {
                r.known_with(&[STARS, &["value", "size"]].concat());
                if let Some(value) = r
                    .text("value")
                    .filter(|v| Data::parse(v) != Some(Data::GameRating))
                {
                    log::warn!("Theme {at}.value {value:?}: only game.rating has stars; using it");
                }
                let mut stars = stars(&r, Align::Left);
                stars.size = r.number_opt_in("size", 0.005, 0.3);
                Kind::Rating(stars)
            }
            "fields" => Kind::Fields(self.fields(&r)?),
            _ => {
                r.known_with(&["radius", "style", "gradient"]);
                Kind::Panel(Panel {
                    rect,
                    radius: r.number("radius", super::h(14.), 0., 0.5),
                    style: r.pick(
                        "style",
                        PanelStyle::Plain,
                        &[("plain", PanelStyle::Plain), ("glass", PanelStyle::Glass)],
                    ),
                    gradient: t
                        .get("gradient")
                        .and_then(|g| crate::theme::gradient(g, &format!("{at}.gradient"))),
                    opacity: 1.,
                })
            }
        };
        Some(Element {
            id: r.text("id").unwrap_or_default(),
            z: r.number("z", 0., -1000., 1000.).round() as i32,
            rect,
            opacity: r.number("opacity", 1., 0., 1.),
            rotate: r.number("rotate", 0., -360., 360.),
            pivot,
            systems: names(when.get("system"), &format!("{at}.when.system")),
            has,
            kind,
        })
    }

    /// `rect = { x, y, w, h }`: fractions of the screen, the whole screen
    /// when left out.
    fn rect(&self, r: &Reader) -> Rect01 {
        let whole = Rect01 {
            x: 0.,
            y: 0.,
            w: 1.,
            h: 1.,
        };
        let Some(value) = r.table.get("rect") else {
            return whole;
        };
        let Some(t) = value.as_table() else {
            log::warn!(
                "Theme {}.rect must be {{ x, y, w, h }}; the whole screen",
                r.at
            );
            return whole;
        };
        let at = format!("{}.rect", r.at);
        let r = Reader { table: t, at: &at };
        r.known(&["x", "y", "w", "h"]);
        Rect01 {
            x: r.number("x", 0., -1., 2.),
            y: r.number("y", 0., -1., 2.),
            w: r.number("w", 1., 0.001, 4.),
            h: r.number("h", 1., 0.001, 4.),
        }
    }

    fn image(&mut self, r: &Reader) -> Option<Image> {
        r.known_with(&[
            "src",
            "fit",
            "align",
            "tint",
            "radius",
            "fallback",
            "fallback_text",
        ]);
        let mut src: Vec<Template> = match r.table.get("src") {
            Some(Value::String(one)) => vec![source(one, r.at)],
            Some(Value::Array(many)) => many
                .iter()
                .filter_map(|v| match v.as_str() {
                    Some(one) => Some(source(one, r.at)),
                    None => {
                        log::warn!("Theme {}.src: each source must be text; ignored", r.at);
                        None
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        src.extend(r.path("fallback"));
        src.retain(|t| !t.0.is_empty());
        let fallback = match r.table.get("fallback_text") {
            None => None,
            Some(Value::String(text)) => {
                let mut table = Table::new();
                table.insert("text".into(), Value::String(text.clone()));
                let at = format!("{}.fallback_text", r.at);
                self.text(
                    &Reader {
                        table: &table,
                        at: &at,
                    },
                    &[],
                )
                .map(Box::new)
            }
            Some(Value::Table(table)) => {
                let at = format!("{}.fallback_text", r.at);
                self.text(&Reader { table, at: &at }, &["text"])
                    .map(Box::new)
            }
            Some(_) => {
                log::warn!(
                    "Theme {}.fallback_text must be text or a table; ignored",
                    r.at
                );
                None
            }
        };
        if src.is_empty() && fallback.is_none() {
            log::warn!("Theme {}.src names no picture; element ignored", r.at);
            return None;
        }
        Some(Image {
            src,
            fit: r.pick(
                "fit",
                ImageFit::Contain,
                &[
                    ("contain", ImageFit::Contain),
                    ("cover", ImageFit::Cover),
                    ("stretch", ImageFit::Stretch),
                ],
            ),
            align: r.align("align", Align::Center),
            tint: r.color("tint"),
            radius: r.number("radius", 0., 0., 0.5),
            fallback,
        })
    }

    /// A text element's fields; `only` limits them (a fallback table).
    fn text(&mut self, r: &Reader, only: &[&str]) -> Option<Text> {
        let keys = [
            "text", "font", "size", "color", "align", "valign", "upper", "shadow", "wrap", "line",
            "fit_text",
        ];
        if only.is_empty() {
            r.known_with(&keys);
        } else {
            known(r.table, r.at, &keys);
        }
        let Some(text) = r.text("text") else {
            log::warn!("Theme {}.text must be text; element ignored", r.at);
            return None;
        };
        let wrap = r.flag("wrap", false);
        let shadow = match r.table.get("shadow") {
            None => None,
            Some(Value::Table(t)) => {
                let at = format!("{}.shadow", r.at);
                let s = Reader { table: t, at: &at };
                s.known(&["color", "x", "y"]);
                Some(Shadow {
                    color: s.color("color").unwrap_or(Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 160,
                    }),
                    x: s.number("x", 0.0021, -0.05, 0.05),
                    y: s.number("y", 0.0021, -0.05, 0.05),
                })
            }
            Some(_) => {
                log::warn!("Theme {}.shadow must be {{ color, x, y }}; ignored", r.at);
                None
            }
        };
        Some(Text {
            text: Template::parse(&text, r.at),
            font: self.font(r, "font"),
            size: r.number("size", 0.035, 0.01, 0.3),
            color: r.color("color"),
            align: r.align("align", Align::Left),
            valign: r.pick(
                "valign",
                if wrap { VAlign::Top } else { VAlign::Middle },
                &[
                    ("top", VAlign::Top),
                    ("middle", VAlign::Middle),
                    ("bottom", VAlign::Bottom),
                ],
            ),
            upper: r.flag("upper", false),
            shadow,
            wrap,
            shrink: r.pick("fit_text", false, &[("ellipsis", false), ("shrink", true)]),
            line: r.number("line", 1.25, 0.8, 3.),
        })
    }

    /// `key`: a face of the theme by name, or a file in its folder.
    fn font(&mut self, r: &Reader, key: &str) -> Font {
        let Some(name) = r.text(key) else {
            return Font::Regular;
        };
        match name.as_str() {
            "regular" => Font::Regular,
            "semibold" => Font::Semibold,
            "bold" => Font::Bold,
            "mono" => Font::Mono,
            file if safe_path(file) => {
                let index = self
                    .fonts
                    .iter()
                    .position(|f| f == file)
                    .unwrap_or_else(|| {
                        self.fonts.push(file.to_owned());
                        self.fonts.len() - 1
                    });
                match u8::try_from(index) {
                    Ok(n) if n < 16 => Font::File(n),
                    _ => {
                        log::warn!("Theme {}.{key}: at most 16 font files; regular face", r.at);
                        Font::Regular
                    }
                }
            }
            _ => {
                log::warn!(
                    "Theme {}.{key} must be regular, semibold, bold, mono or a file inside the theme; regular face",
                    r.at
                );
                Font::Regular
            }
        }
    }
}

impl Scene {
    /// `rows = [{ label, value }, { label, rating = "game.rating" }]` and
    /// how labels and values are written.
    fn fields(&mut self, r: &Reader) -> Option<Fields> {
        r.known_with(
            &[
                STARS,
                &[
                    "rows",
                    "hide_empty",
                    "upper_labels",
                    "row",
                    "font",
                    "size",
                    "label_font",
                    "label_size",
                    "label_color",
                    "divider",
                    "star_size",
                ],
            ]
            .concat(),
        );
        let Some(items) = r.table.get("rows").and_then(Value::as_array) else {
            log::warn!(
                "Theme {}.rows must be a list of {{ label, value }}; element ignored",
                r.at
            );
            return None;
        };
        let mut rows = Vec::new();
        for (n, item) in items.iter().enumerate() {
            let at = format!("{}.rows[{n}]", r.at);
            let Some(t) = item.as_table() else {
                log::warn!("Theme {at} must be {{ label, value }}; ignored");
                continue;
            };
            let row = Reader { table: t, at: &at };
            row.known(&["label", "value", "rating"]);
            let label = Template::parse(&row.text("label").unwrap_or_default(), &at);
            let value = match (row.text("value"), row.text("rating")) {
                (Some(value), _) => Some(Template::parse(&value, &at)),
                (None, Some(rating)) => {
                    if Data::parse(&rating) != Some(Data::GameRating) {
                        log::warn!(
                            "Theme {at}.rating {rating:?}: only game.rating has stars; using it"
                        );
                    }
                    None
                }
                (None, None) => {
                    log::warn!("Theme {at} needs a value or a rating; ignored");
                    continue;
                }
            };
            rows.push(Field { label, value });
        }
        if rows.is_empty() {
            log::warn!("Theme {}.rows has no line; element ignored", r.at);
            return None;
        }
        let size = r.number("size", 0.03, 0.01, 0.3);
        let value = Text {
            text: Template::default(),
            font: self.font(r, "font"),
            size,
            color: r.color("color"),
            align: Align::Right,
            valign: VAlign::Middle,
            upper: false,
            shadow: None,
            wrap: false,
            shrink: true,
            line: 1.25,
        };
        let label = Text {
            font: self.font(r, "label_font"),
            size: r.number("label_size", size, 0.01, 0.3),
            color: r.color("label_color"),
            align: Align::Left,
            upper: r.flag("upper_labels", false),
            shrink: false,
            ..value.clone()
        };
        let mut stars = stars(r, Align::Right);
        stars.size = r.number_opt_in("star_size", 0.005, 0.3).or(Some(size));
        Some(Fields {
            rows,
            hide_empty: r.flag("hide_empty", true),
            row: r.number_opt_in("row", 0.01, 1.),
            label,
            value,
            divider: r.color("divider"),
            stars,
        })
    }
}

/// The stars' pictures, colors and room, as `rating` and `fields` give them.
fn stars(r: &Reader, align: Align) -> Stars {
    let full = r.path("star");
    let empty_color = r.color("empty");
    let empty = r.path("star_empty");
    // A plain empty star is a faded full one, unless the theme colors it.
    let faded = if empty_color.is_some() || empty.is_some() {
        1.
    } else {
        0.35
    };
    Stars {
        half: r.path("star_half"),
        color: r.color("color"),
        empty_opacity: r.number("empty_opacity", faded, 0., 1.),
        size: None,
        spacing: r.number_opt_in("spacing", 0., 0.2),
        align: r.align("align", align),
        full,
        empty,
        empty_color,
    }
}

/// The list element: its box and sizes into the layout's list (sizes as
/// fractions of the screen height become the list's fractions of 480),
/// its colors kept for drawing.
fn list_element(r: &Reader, rect: Rect01, place: &Place<'_>, list: &mut List) -> ListInk {
    r.known_with(&[
        "row",
        "selected",
        "gap",
        "size",
        "selected_size",
        "radius",
        "axis",
        "mark",
        "scroll",
        "cursor",
        "columns",
        "color",
    ]);
    let k = place.tall / 480.;
    let mut t = Table::new();
    let x = rect.x.clamp(0., 0.99);
    let y = rect.y.clamp(0., 0.99);
    let fraction = |n: f32| Value::Float(f64::from(n));
    t.insert("x".into(), fraction(x));
    t.insert("y".into(), fraction(y));
    t.insert("w".into(), fraction((rect.x + rect.w).min(1.) - x));
    t.insert("h".into(), fraction((rect.y + rect.h).min(1.) - y));
    let scaled = |key: &str| {
        r.table
            .get(key)
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64)))
            .map(|n| Value::Float(n * f64::from(k)))
    };
    for (from, to) in [
        ("row", "row"),
        ("gap", "gap"),
        ("radius", "radius"),
        ("size", "font"),
        ("selected_size", "selected_font"),
    ] {
        if let Some(value) = scaled(from) {
            t.insert(to.into(), value);
        } else if r.table.contains_key(from) {
            log::warn!("Theme {}.{from} must be a number; ignored", r.at);
        }
    }
    let mut ink = ListInk {
        color: r.color("color"),
        ..ListInk::default()
    };
    match r.table.get("selected") {
        None => {}
        Some(Value::Table(s)) => {
            let at = format!("{}.selected", r.at);
            let s = Reader { table: s, at: &at };
            s.known(&["height", "fill", "color"]);
            ink.fill = s.color("fill");
            ink.selected = s.color("color");
            if let Some(height) = s.number_opt("height") {
                t.insert("selected".into(), Value::Float(f64::from(height * k)));
            }
        }
        Some(_) => {
            if let Some(value) = scaled("selected") {
                t.insert("selected".into(), value);
            } else {
                log::warn!(
                    "Theme {}.selected must be a number or a table; ignored",
                    r.at
                );
            }
        }
    }
    // A new row height brings a selected row as tall, a new size the
    // selected text one pixel larger, unless they are given.
    if t.contains_key("row") && !t.contains_key("selected") {
        t.insert("selected".into(), t["row"].clone());
    }
    if let (Some(Value::Float(font)), false) = (t.get("font"), t.contains_key("selected_font")) {
        let font = *font + 1. / 480.;
        t.insert("selected_font".into(), Value::Float(font));
    }
    for key in ["axis", "mark", "scroll", "cursor", "columns"] {
        if let Some(value) = r.table.get(key) {
            t.insert(key.into(), value.clone());
        }
    }
    // The row before the selected height: List::overlay limits one by the other.
    if let Some(row) = t.get("row").and_then(Value::as_float) {
        list.row = (row as f32).clamp(0.02, 1.);
        list.selected = list.selected.max(list.row);
    }
    list.overlay(&t, r.at);
    list.opacity = 1.;
    ink
}

/// The element's table, with the element it names in `base` under it.
fn resolve(item: &Value, place: &Place<'_>, at: &str, depth: u8) -> Option<Table> {
    let Some(table) = item.as_table() else {
        log::warn!("Theme {at} must be a table; ignored");
        return None;
    };
    let Some(base) = table.get("base") else {
        return Some(table.clone());
    };
    let found = base.as_str().and_then(|name| {
        let (screen, id) = name.split_once('.').unwrap_or((place.screen, name));
        place
            .view
            .get(screen)?
            .get("el")?
            .as_array()?
            .iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some(id))
    });
    let Some(found) = found.filter(|_| depth < 4) else {
        log::warn!("Theme {at}.base names no element of this view; ignored");
        return None;
    };
    let mut merged = resolve(found, place, at, depth + 1)?;
    for (key, value) in table {
        if key != "base" {
            merged.insert(key.clone(), value.clone());
        }
    }
    Some(merged)
}

/// A picture source: a game's picture (`game.cover`) or a path inside the
/// theme folder, which may hold data (`arts/${system}.jpg`).
fn source(text: &str, at: &str) -> Template {
    if let Some(data) = Data::parse(text).filter(|d| d.picture()) {
        return Template(vec![Piece::Data(data)]);
    }
    if !safe_path(text) {
        log::warn!("Theme {at}.src {text:?} must be a path inside the theme; ignored");
        return Template::default();
    }
    Template::parse(text, at)
}
/// Relative, inside the folder: no root, no `..`.
pub(crate) fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && std::path::Path::new(path)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
/// One name or a list of them.
fn names(value: Option<&Value>, at: &str) -> Vec<String> {
    match value {
        None => Vec::new(),
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(many)) => many
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Some(_) => {
            log::warn!("Theme {at} must be text or a list of text; ignored");
            Vec::new()
        }
    }
}
/// `[a, b]`, each within `min..=max`.
fn pair(value: &Value, min: f32, max: f32) -> Option<(f32, f32)> {
    let items = value.as_array()?;
    let number = |v: &Value| {
        v.as_float()
            .or_else(|| v.as_integer().map(|n| n as f64))
            .map(|n| n as f32)
            .filter(|n| n.is_finite() && (min..=max).contains(n))
    };
    match items.as_slice() {
        [a, b] => Some((number(a)?, number(b)?)),
        _ => None,
    }
}

impl Reader<'_> {
    fn known_with(&self, own: &[&str]) {
        let keys: Vec<&str> = COMMON.iter().chain(own).copied().collect();
        known(self.table, self.at, &keys);
    }
    fn text(&self, key: &str) -> Option<String> {
        match self.table.get(key)? {
            Value::String(text) => Some(text.clone()),
            _ => {
                log::warn!("Theme {}.{key} must be text; ignored", self.at);
                None
            }
        }
    }
    fn number_opt(&self, key: &str) -> Option<f32> {
        self.number_opt_in(key, 0., 1.)
    }
    /// A number within `min..=max` when given; a bad one is left out.
    fn number_opt_in(&self, key: &str, min: f32, max: f32) -> Option<f32> {
        self.table.get(key)?;
        let n = self.number(key, f32::NAN, min, max);
        (!n.is_nan()).then_some(n)
    }
    fn color(&self, key: &str) -> Option<Rgba> {
        let text = self.text(key)?;
        let color = crate::theme::color(text.clone());
        if color.is_none() {
            log::warn!("Theme {}.{key} = {text:?} is not a color; ignored", self.at);
        }
        color
    }
    fn align(&self, key: &str, current: Align) -> Align {
        self.pick(
            key,
            current,
            &[
                ("left", Align::Left),
                ("center", Align::Center),
                ("right", Align::Right),
            ],
        )
    }
    /// A path inside the theme folder, which may hold data.
    fn path(&self, key: &str) -> Option<Template> {
        let text = self.text(key)?;
        if safe_path(&text) {
            Some(Template::parse(&text, &format!("{}.{key}", self.at)))
        } else {
            log::warn!(
                "Theme {}.{key} {text:?} must be a path inside the theme; ignored",
                self.at
            );
            None
        }
    }
    /// `anchor`: the point of the box at `x`, `y`, as a fraction of it.
    fn anchor(&self) -> (f32, f32) {
        let Some(value) = self.table.get("anchor") else {
            return (0., 0.);
        };
        if let Some(point) = pair(value, 0., 1.) {
            return point;
        }
        let named = value.as_str().and_then(|name| {
            Some(match name {
                "top-left" => (0., 0.),
                "top" => (0.5, 0.),
                "top-right" => (1., 0.),
                "left" => (0., 0.5),
                "center" => (0.5, 0.5),
                "right" => (1., 0.5),
                "bottom-left" => (0., 1.),
                "bottom" => (0.5, 1.),
                "bottom-right" => (1., 1.),
                _ => return None,
            })
        });
        named.unwrap_or_else(|| {
            log::warn!(
                "Theme {}.anchor must be top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right or [x, y]; top-left",
                self.at
            );
            (0., 0.)
        })
    }
}

/// `[vars]` written into every value of a theme table: a value that is
/// `"$name"` becomes the variable itself (a number stays a number), and
/// `${name}` inside text its text. Data names (`${system}`) are left for
/// drawing. The `vars` table itself is left alone.
pub(crate) fn substitute(table: &mut Table, vars: &Table) {
    if vars.is_empty() {
        return;
    }
    for (key, value) in table.iter_mut() {
        if key != "vars" {
            substitute_value(value, vars, key);
        }
    }
}
fn substitute_value(value: &mut Value, vars: &Table, at: &str) {
    match value {
        Value::String(text) => {
            if let Some(name) = text.strip_prefix('$').filter(|n| var_shaped(n)) {
                match vars.get(name) {
                    Some(var) => *value = var.clone(),
                    None => log::warn!("Theme {at}: unknown variable ${name}; written as is"),
                }
                return;
            }
            if text.contains("${") {
                *text = fill_vars(text, vars);
            }
        }
        Value::Array(items) => {
            for item in items {
                substitute_value(item, vars, at);
            }
        }
        Value::Table(table) => {
            for (key, item) in table.iter_mut() {
                substitute_value(item, vars, &format!("{at}.{key}"));
            }
        }
        _ => {}
    }
}
fn fill_vars(text: &str, vars: &Table) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        let name = &rest[start + 2..start + end];
        out.push_str(&rest[..start]);
        // Data names are filled in as the screen is drawn, never by a variable.
        let var = if Data::parse(name).is_some() {
            None
        } else {
            vars.get(name)
        };
        match var {
            Some(Value::String(var)) => out.push_str(var),
            Some(Value::Float(n)) => out.push_str(&n.to_string()),
            Some(Value::Integer(n)) => out.push_str(&n.to_string()),
            _ => out.push_str(&rest[start..start + end + 1]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}
fn var_shaped(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_split_data_from_text_and_keep_unknown_names() {
        let t = Template::parse("${system.count} jogos · ${odd} ${game.name}", "t");
        assert_eq!(
            t.0,
            vec![
                Piece::Data(Data::SystemCount),
                Piece::Text(" jogos · ${odd} ".into()),
                Piece::Data(Data::GameName),
            ]
        );
        let filled = t.fill(
            |d| match d {
                Data::SystemCount => "213".into(),
                _ => "Sonic".into(),
            },
            |_| String::new(),
        );
        assert_eq!(filled, "213 jogos · ${odd} Sonic");
        assert_eq!(
            Template::parse("@description", "t").0,
            vec![Piece::Phrase("description".into())]
        );
        assert_eq!(
            Template::parse("@ home", "t").0,
            vec![Piece::Text("@ home".into())]
        );
    }

    #[test]
    fn variables_fill_whole_values_and_text() {
        let vars: Table = toml::from_str("a = '#112233'\nn = 0.4\np = 'arts'").unwrap();
        let mut t: Table = toml::from_str(
            "accent = '$a'\n[x]\nopacity = '$n'\nsrc = ['${p}/${system}.jpg', '$missing']",
        )
        .unwrap();
        substitute(&mut t, &vars);
        assert_eq!(t["accent"].as_str(), Some("#112233"));
        assert_eq!(t["x"]["opacity"].as_float(), Some(0.4));
        let src = t["x"]["src"].as_array().unwrap();
        assert_eq!(src[0].as_str(), Some("arts/${system}.jpg"));
        assert_eq!(src[1].as_str(), Some("$missing"));
    }
}
