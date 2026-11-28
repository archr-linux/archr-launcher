//! One read of an ES theme (see `es`) made into the launcher's views:
//! each element becomes a free-screen element (docs/temas.md, Telas
//! livres), measured for one screen shape.
use crate::es::{El, Es, View};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

/// A free-screen element: its id and its fields.
#[derive(Clone, Debug, PartialEq)]
pub struct OutEl {
    pub id: String,
    pub t: Table,
}
#[derive(Clone, Debug, PartialEq)]
pub struct OutView {
    pub id: String,
    pub name: String,
    pub systems: Vec<OutEl>,
    pub games: Vec<OutEl>,
    /// The ids the details screen leaves out (the list, carousel or grid)
    /// and the description that scrolls there.
    pub collection: Option<String>,
    pub description: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub views: Vec<OutView>,
    pub top: Table,
    /// What could not be converted, and why.
    pub notes: BTreeSet<String>,
    /// "what it was → what it became", for the report.
    pub mapped: BTreeSet<String>,
    /// Picture patterns to copy: from the source's root, to the output's.
    pub files: BTreeSet<(String, String)>,
    /// Clips whose first frame is written as a picture: the clip from the
    /// source's root, the picture in the output's.
    pub stills: BTreeSet<(String, String)>,
}

/// The screen a read is measured for: width over height.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub ratio: f32,
}

const HINTS_Z: i64 = 995;
/// A mark on a video table while a screen is made: ES plays its sound.
const AUDIO: &str = "\u{0}audio";
/// A mark on a text as wide as its words in ES (no width): `fit_widths`
/// stops it at the next element on its line and takes it away.
const AUTO_W: &str = "\u{0}auto_w";
/// A mark on the status made from ES-DE's clock (`one_status`).
const CLOCK: &str = "\u{0}clock";
const STATUS_Z: i64 = 996;

pub fn model(es: &Es, root: &Path, shape: Shape, logo_dir: Option<&str>) -> Model {
    let mut m = Mapper {
        root,
        shape,
        out: Model::default(),
        esde: es.esde,
        logo: logo_dir.map(str::to_owned),
    };
    m.run(es);
    m.out
}

struct Mapper<'a> {
    root: &'a Path,
    shape: Shape,
    out: Model,
    esde: bool,
    /// The system logo's folder pattern, copied into `logos/`.
    logo: Option<String>,
}

// ---------------------------------------------------------------- values

pub fn r5(v: f32) -> f64 {
    (f64::from(v) * 100_000.).round() / 100_000.
}
fn num(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok().filter(|n| n.is_finite())
}
/// "x y"; one number is both, and a word ES cannot read (an unknown
/// variable) is 0, as ES reads it.
fn pair(s: &str) -> Option<(f32, f32)> {
    let mut it = s.split_whitespace();
    let a = num(it.next()?).unwrap_or(0.);
    Some(match it.next() {
        Some(b) => (a, num(b).unwrap_or(0.)),
        None => (a, a),
    })
}
fn quad(s: &str) -> Option<[f32; 4]> {
    let v: Vec<f32> = s.split_whitespace().filter_map(num).collect();
    match v.len() {
        4 => Some([v[0], v[1], v[2], v[3]]),
        2 => Some([v[0], v[1], v[0], v[1]]),
        1 => Some([v[0]; 4]),
        _ => None,
    }
}
fn flag(s: Option<&str>) -> Option<bool> {
    s.map(|s| matches!(s.trim(), "true" | "1"))
}
/// An ES color (RRGGBB or RRGGBBAA, with or without #): "#RRGGBBAA" and
/// its alpha.
pub fn color(s: &str) -> Option<(String, u8)> {
    let s = s.trim().trim_start_matches('#').trim_start_matches("0x");
    let full = match s.len() {
        6 => format!("{s}FF"),
        8 => s.to_owned(),
        3 => s.chars().flat_map(|c| [c, c]).chain("FF".chars()).collect(),
        _ => return None,
    };
    if !full.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let alpha = u8::from_str_radix(&full[6..8], 16).ok()?;
    Some((format!("#{}", full.to_uppercase()), alpha))
}
fn f(v: f32) -> Value {
    Value::Float(r5(v))
}
fn s(v: impl Into<String>) -> Value {
    Value::String(v.into())
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> Value {
    let mut t = Table::new();
    t.insert("x".into(), f(x.clamp(-1., 2.)));
    t.insert("y".into(), f(y.clamp(-1., 2.)));
    t.insert("w".into(), f(w.clamp(0.001, 4.)));
    t.insert("h".into(), f(h.clamp(0.001, 4.)));
    Value::Table(t)
}
fn anchor(ox: f32, oy: f32) -> Option<Value> {
    let named = [
        ((0., 0.), "top-left"),
        ((0.5, 0.), "top"),
        ((1., 0.), "top-right"),
        ((0., 0.5), "left"),
        ((0.5, 0.5), "center"),
        ((1., 0.5), "right"),
        ((0., 1.), "bottom-left"),
        ((0.5, 1.), "bottom"),
        ((1., 1.), "bottom-right"),
    ];
    if (ox, oy) == (0., 0.) {
        return None;
    }
    match named
        .iter()
        .find(|((x, y), _)| (x - ox).abs() < 1e-4 && (y - oy).abs() < 1e-4)
    {
        Some((_, name)) => Some(s(*name)),
        None => Some(Value::Array(vec![f(ox), f(oy)])),
    }
}
/// An element's box on the screen (its anchor applied), when its rect is
/// numbers.
fn bounds(t: &Table) -> Option<(f32, f32, f32, f32)> {
    let r = t.get("rect")?.as_table()?;
    let n = |k: &str| r.get(k).and_then(Value::as_float).map(|v| v as f32);
    let (x, y, w, h) = (n("x")?, n("y")?, n("w")?, n("h")?);
    let (ax, ay) = match t.get("anchor") {
        Some(Value::String(a)) => match a.as_str() {
            "top" => (0.5, 0.),
            "top-right" => (1., 0.),
            "left" => (0., 0.5),
            "center" => (0.5, 0.5),
            "right" => (1., 0.5),
            "bottom-left" => (0., 1.),
            "bottom" => (0.5, 1.),
            "bottom-right" => (1., 1.),
            _ => (0., 0.),
        },
        Some(Value::Array(a)) => (
            a.first().and_then(Value::as_float).unwrap_or(0.) as f32,
            a.get(1).and_then(Value::as_float).unwrap_or(0.) as f32,
        ),
        _ => (0., 0.),
    };
    let (x0, y0) = (x - ax * w, y - ay * h);
    Some((x0, y0, x0 + w, y0 + h))
}

/// A row of items (a horizontal carousel) whose items reach below the top
/// of the button hints is raised to end there: ES-DE lets such a row run
/// under its help or off the screen, the launcher's hints would cover it.
fn lift_over_hints(out: &mut [OutEl]) {
    let Some(top) = out
        .iter()
        .filter(|e| e.t.get("kind").and_then(Value::as_str) == Some("hints"))
        .filter_map(|e| bounds(&e.t))
        .map(|(_, y0, _, _)| y0)
        .reduce(f32::min)
    else {
        return;
    };
    for e in out.iter_mut() {
        let t = &mut e.t;
        if t.get("kind").and_then(Value::as_str) != Some("carousel")
            || t.get("axis").and_then(Value::as_str) != Some("horizontal")
        {
            continue;
        }
        let Some((_, y0, _, y1)) = bounds(t) else {
            continue;
        };
        let item = t
            .get("item")
            .and_then(|i| i.get("h"))
            .and_then(Value::as_float)
            .map_or(y1 - y0, |h| h as f32);
        let scale = t
            .get("selected")
            .and_then(|s| s.get("scale"))
            .and_then(Value::as_float)
            .unwrap_or(1.) as f32;
        // The focused item grows from the row's middle; a row whose box
        // runs off the screen draws its items down to the box's bottom.
        let grown = (y0 + y1) / 2. + item * scale.max(1.) / 2.;
        let bottom = if y1 > 1. {
            grown.max(y1.min(1.2))
        } else {
            grown
        };
        if bottom <= top {
            continue;
        }
        if let Some(Value::Table(r)) = t.get_mut("rect") {
            if let Some(y) = r.get("y").and_then(Value::as_float) {
                r.insert("y".into(), Value::Float(r5(y as f32 - (bottom - top))));
            }
        }
    }
}

/// A text ES sizes to its words (no width) reaches to the screen's edge
/// here; the launcher's words are longer ("1 a 2 jogadores" for "1-2"), so
/// it stops short of the next element on its line that shows with it (the
/// same opacity) and shrinks to fit there.
fn fit_widths(out: &mut [OutEl]) {
    let boxes: Vec<Option<(f32, f32, f32, f32)>> = out.iter().map(|e| bounds(&e.t)).collect();
    let shown = |t: &Table| t.get("opacity").map(Value::to_string).unwrap_or_default();
    let mut limits = Vec::new();
    for (i, e) in out.iter().enumerate() {
        if e.t.get(AUTO_W).is_none() {
            continue;
        }
        let Some((x0, y0, x1, y1)) = boxes[i] else {
            continue;
        };
        // Only a text that grows from one side: its box starts at its
        // anchor (left) or ends there (right).
        let align = e.t.get("align").and_then(Value::as_str).unwrap_or("left");
        let anchor_x = match e.t.get("anchor") {
            None => 0.,
            Some(Value::String(a)) if a.ends_with("left") => 0.,
            Some(Value::String(a)) if a.ends_with("right") => 1.,
            Some(Value::String(a)) if a == "top" || a == "bottom" || a == "center" => 0.5,
            Some(Value::Array(a)) => a.first().and_then(Value::as_float).unwrap_or(0.) as f32,
            Some(_) => 0.,
        };
        let right = match (anchor_x, align) {
            (x, "left") if x < 0.01 => false,
            (x, "right") if x > 0.99 => true,
            _ => continue,
        };
        let mut edge: Option<f32> = None;
        for (j, other) in out.iter().enumerate() {
            let Some((ox0, oy0, ox1, oy1)) = boxes[j].filter(|_| j != i) else {
                continue;
            };
            let overlap = (y1.min(oy1) - y0.max(oy0)) / (y1 - y0).max(1e-3);
            if overlap < 0.3 || shown(&other.t) != shown(&e.t) || ox1 - ox0 > 0.9 {
                continue;
            }
            if !right && ox0 > x0 + 0.01 && ox0 < x1 {
                edge = Some(edge.map_or(ox0, |v| v.min(ox0)));
            }
            if right && ox1 < x1 - 0.01 && ox1 > x0 {
                edge = Some(edge.map_or(ox1, |v| v.max(ox1)));
            }
        }
        if let Some(edge) = edge {
            limits.push((i, edge, right));
        }
    }
    for (i, edge, right) in limits {
        let Some((x0, _, x1, _)) = boxes[i] else {
            continue;
        };
        let t = &mut out[i].t;
        let w = if right {
            x1 - edge - 0.008
        } else {
            edge - x0 - 0.008
        }
        .max(0.02);
        let shift = if right { x1 - w } else { x0 };
        if let Some(Value::Table(r)) = t.get_mut("rect") {
            let ax = r.get("w").and_then(Value::as_float).map_or(0., |old| {
                let x = r.get("x").and_then(Value::as_float).unwrap_or(0.) as f32;
                (x - x0) / (old as f32).max(1e-3)
            });
            r.insert("w".into(), Value::Float(r5(w)));
            r.insert("x".into(), Value::Float(r5(shift + ax * w)));
        }
        t.insert("fit_text".into(), s("shrink"));
    }
    for e in out.iter_mut() {
        e.t.remove(AUTO_W);
    }
}

/// A theme's own label of a game fact the launcher does not keep (play
/// time, play count, badges, kid-friendly, completed...): by its element's
/// name or its words. Its value is left out, so the label would stand
/// alone over nothing.
fn missing_label(name: &str, text: &str) -> bool {
    let squash = |t: &str| -> String {
        t.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    };
    let (name, text) = (squash(name), squash(text));
    [
        "playtime",
        "playcount",
        "timesplayed",
        "kidgame",
        "kidfriendly",
        "badge",
        "completed",
        "broken",
        "altemulator",
    ]
    .iter()
    .any(|k| name.contains(k) || text.contains(k))
}

/// The color of a picture that is one opaque color (a small PNG), else
/// None.
fn solid(root: &Path, rel: &str) -> Option<[u8; 3]> {
    if rel.contains("${") || !rel.to_lowercase().ends_with(".png") {
        return None;
    }
    let path = crate::es::contained(root, rel)?;
    if std::fs::metadata(&path).ok()?.len() > 4096 {
        return None;
    }
    let img = image::open(path).ok()?.to_rgba8();
    let first = *img.pixels().next()?;
    (first[3] == 255 && img.pixels().all(|p| *p == first)).then_some([first[0], first[1], first[2]])
}

/// The folder of the systems' logos: the first `${system.theme}` picture
/// of the systems screen's carousel or grid (`staticImage`, `logo`).
pub fn logo_dir(es: &Es) -> Option<String> {
    let view = es.view("system")?;
    let paths = view.els.iter().flat_map(|e| {
        let own: Vec<String> = match e.kind.as_str() {
            "carousel" | "grid" => ["staticImage", "defaultImage"]
                .iter()
                .filter_map(|k| e.get(k).map(str::to_owned))
                .collect(),
            "image" if e.name == "logo" => e.paths.clone(),
            _ => Vec::new(),
        };
        own
    });
    paths
        .filter_map(|p| {
            let (dir, name) = p.rsplit_once('/')?;
            (name.starts_with("${system.theme}.") || name.starts_with("${system.name}."))
                .then(|| dir.to_owned())
        })
        .next()
}

/// `${system.theme}` and friends as the launcher's data; in an item of a
/// collection, the item's.
pub(crate) fn data(text: &str, item: bool) -> String {
    let id = if item { "${item}" } else { "${system}" };
    let name = if item {
        "${item.name}"
    } else {
        "${system.name}"
    };
    text.replace("${system.theme}", id)
        .replace("${system.name}", id)
        .replace("${system.fullName}", name)
        .replace("${system.fullname}", name)
}
/// ES-DE's names shown only for some systems: `.noCollections` on the
/// systems, `.autoCollections` on the automatic collections (the
/// launcher's favorites and recent), `.customCollections` on the user's
/// own (the launcher has none: never). The text with plain names, and
/// `when.collection`; None when it never shows.
fn collection_name(text: &str) -> Option<(String, Option<bool>)> {
    let mut only = None;
    let mut out = text.to_owned();
    for field in ["fullName", "name"] {
        for (suffix, shows) in [("noCollections", false), ("autoCollections", true)] {
            let from = format!("${{system.{field}.{suffix}}}");
            if out.contains(&from) {
                out = out.replace(&from, &format!("${{system.{field}}}"));
                only = Some(shows);
            }
        }
        if out.contains(&format!("${{system.{field}.customCollections}}")) {
            return None;
        }
    }
    Some((out, only))
}
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ô' | 'õ' | 'ò' => 'o',
            'ú' | 'ü' | 'ù' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "x".into()
    } else {
        out
    }
}
pub fn id_of(name: &str) -> String {
    slug(name)
}

/// Width over height of the picture at `rel` (from the source's root),
/// a `${...}` pattern measured on its first file.
fn picture_ratio(root: &Path, rel: &str) -> Option<f32> {
    let file = first_file(root, rel)?;
    let (w, h) = if file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        svg_size(&std::fs::read_to_string(&file).ok()?)?
    } else {
        let (w, h) = image::image_dimensions(&file).ok()?;
        (w as f32, h as f32)
    };
    (w > 0. && h > 0.).then_some(w / h)
}
fn picture_pixels(root: &Path, rel: &str) -> Option<(f32, f32)> {
    let file = first_file(root, rel)?;
    if file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        return svg_size(&std::fs::read_to_string(&file).ok()?);
    }
    let (w, h) = image::image_dimensions(&file).ok()?;
    Some((w as f32, h as f32))
}
fn svg_size(text: &str) -> Option<(f32, f32)> {
    let view = text.find("viewBox=").map(|at| &text[at + 9..])?;
    let view = &view[..view.find(['"', '\''])?];
    let v: Vec<f32> = view.split([' ', ',']).filter_map(num).collect();
    (v.len() == 4).then(|| (v[2], v[3]))
}
/// The file at `rel`, or the first one a `${...}` pattern matches.
pub fn first_file(root: &Path, rel: &str) -> Option<PathBuf> {
    matches(root, rel).into_iter().next()
}
/// Every file a path with `${...}` in its file name matches, sorted.
pub fn matches(root: &Path, rel: &str) -> Vec<PathBuf> {
    let Some(at) = rel.find("${") else {
        return crate::es::contained(root, rel)
            .filter(|p| p.is_file())
            .into_iter()
            .collect();
    };
    let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel));
    if dir.contains("${") || at < dir.len() {
        return Vec::new();
    }
    let start = name.find("${").unwrap_or(0);
    let end = name.rfind('}').map_or(name.len(), |e| e + 1);
    let (prefix, suffix) = (&name[..start], &name[end..]);
    let folder = if dir.is_empty() {
        Some(root.to_path_buf())
    } else {
        crate::es::contained(root, dir)
    };
    let Some(Ok(entries)) = folder.map(std::fs::read_dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.strip_prefix(root)
                .ok()
                .and_then(|r| crate::es::contained(root, &r.to_string_lossy()))
                .is_some()
                && p.is_file()
                && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n.len() > prefix.len() + suffix.len()
                        && n.starts_with(prefix)
                        && n.ends_with(suffix)
                })
        })
        .collect();
    found.sort();
    found
}

// ---------------------------------------------------------------- geometry

struct Geo {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    ox: f32,
    oy: f32,
    fit: &'static str,
}

impl Mapper<'_> {
    fn note(&mut self, text: impl Into<String>) {
        self.out.notes.insert(text.into());
    }
    /// A file of the theme's, inside its folder (`es::contained`).
    fn has(&self, rel: &str) -> bool {
        crate::es::contained(self.root, rel).is_some_and(|f| f.is_file())
    }
    fn mapped(&mut self, what: &str, became: &str) {
        self.out.mapped.insert(format!("`{what}` → {became}"));
    }

    /// Position, size and fit of an element; `picture` is the file whose
    /// shape fills in a size of 0.
    fn geo(
        &self,
        el: &El,
        pos: (f32, f32),
        size: (f32, f32),
        origin: (f32, f32),
        picture: Option<&str>,
    ) -> Geo {
        let (x, y) = el.get("pos").and_then(pair).unwrap_or(pos);
        let (ox, oy) = el.get("origin").and_then(pair).unwrap_or(origin);
        let ar = picture
            .and_then(|p| picture_ratio(self.root, p))
            .or_else(|| media_ratio(el));
        let sr = self.shape.ratio;
        let (mut w, mut h, mut fit) = if let Some((w, h)) = el.get("maxSize").and_then(pair) {
            (w, h, "contain")
        } else if let Some((w, h)) = el
            .get("minSize")
            .or_else(|| el.get("cropSize"))
            .and_then(pair)
        {
            // ES-DE's `cropSize`: the box filled, the picture cut.
            (w, h, "cover")
        } else if let Some((w, h)) = el.get("size").and_then(pair) {
            (w, h, "stretch")
        } else {
            match picture.and_then(|p| picture_pixels(self.root, p)) {
                // No size: the picture's own pixels on a 480-line screen.
                Some((pw, ph)) if el.get("size").is_none() && size == (0., 0.) => {
                    (pw / (480. * sr), ph / 480., "stretch")
                }
                _ => (size.0, size.1, "stretch"),
            }
        };
        if w <= 0. && h <= 0. {
            if let Some((pw, ph)) = picture.and_then(|p| picture_pixels(self.root, p)) {
                (w, h) = (pw / (480. * sr), ph / 480.);
            }
        }
        if w <= 0. || h <= 0. {
            match ar {
                Some(ar) if h > 0. => w = h * ar / sr,
                Some(ar) if w > 0. => h = w * sr / ar,
                _ => {
                    // The picture's shape is not known: a wide box it fits in.
                    fit = "contain";
                    if w <= 0. {
                        w = 4.;
                    }
                    if h <= 0. {
                        h = 4.;
                    }
                }
            }
        }
        Geo {
            x,
            y,
            w,
            h,
            ox,
            oy,
            fit,
        }
    }

    fn place(&self, t: &mut Table, g: &Geo) {
        t.insert("rect".into(), rect(g.x, g.y, g.w, g.h));
        if let Some(a) = anchor(g.ox, g.oy) {
            t.insert("anchor".into(), a);
        }
    }

    /// z, opacity, rotation and entrance, common to every element.
    fn common(&mut self, t: &mut Table, el: &El, z: f32) {
        let z = el.get("zIndex").and_then(num).unwrap_or(z);
        t.insert(
            "z".into(),
            Value::Integer(((z * 4.).round() as i64).clamp(-990, 990)),
        );
        if let Some(r) = el.get("rotation").and_then(num).filter(|r| *r != 0.) {
            t.insert("rotate".into(), f(r.clamp(-360., 360.)));
            if let Some((px, py)) = el.get("rotationOrigin").and_then(pair) {
                t.insert("pivot".into(), Value::Array(vec![f(px), f(py)]));
            }
        }
        if let Some(o) = el.get("opacity").and_then(num) {
            let o = if o > 1. { o / 255. } else { o };
            if o < 1. {
                t.insert("opacity".into(), f(o.max(0.)));
            }
        }
        if let Some(enter) = self.enter(el) {
            t.insert("enter".into(), Value::Table(enter));
        }
    }

    /// A storyboard that slides, fades or grows, as `enter`.
    fn enter(&mut self, el: &El) -> Option<Table> {
        if el.story.is_empty() {
            return None;
        }
        let mut t = Table::new();
        let (mut duration, mut delay, mut fade, mut moved) = (0f32, f32::MAX, false, false);
        let ease = "out";
        for a in &el.story {
            let (from, to) = (num(&a.from), num(&a.to));
            duration = duration.max(a.duration / 1000.);
            delay = delay.min(a.begin / 1000.);
            match (a.property.as_str(), from, to) {
                ("x" | "y", Some(from), Some(to)) if (from - to).abs() > 1e-4 => {
                    let side = match (a.property.as_str(), from < to) {
                        ("x", true) => "left",
                        ("x", false) => "right",
                        (_, true) => "top",
                        _ => "bottom",
                    };
                    t.insert("from".into(), s(side));
                    t.insert("distance".into(), f((from - to).abs().min(1.)));
                    moved = true;
                }
                ("opacity", Some(from), Some(to)) if from < to => fade = true,
                ("scale" | "zoom", Some(from), Some(_)) if (from - 1.).abs() > 1e-3 => {
                    t.insert("scale".into(), f(from.clamp(0.1, 4.)));
                    moved = true;
                }
                (p, ..) => {
                    let p = p.to_owned();
                    self.note(format!(
                        "animação da propriedade `{p}` (storyboard) não existe; ignorada"
                    ));
                }
            }
        }
        if !fade && !moved {
            return None;
        }
        if !fade {
            t.insert("fade".into(), Value::Boolean(false));
        }
        if duration > 0.6 {
            self.note(
                "animações de entrada mais longas que 0,6 s ficam com 0,6 s (o máximo do launcher)",
            );
        }
        let duration = duration.clamp(0.05, 0.6);
        t.insert("duration".into(), f(duration));
        let delay = if delay == f32::MAX {
            0.
        } else {
            delay.min(0.6 - duration).max(0.)
        };
        if delay > 0. {
            t.insert("delay".into(), f(delay));
        }
        if ease != "out" {
            t.insert("ease".into(), s(ease));
        }
        Some(t)
    }

    fn hidden(&self, el: &El) -> bool {
        if flag(el.get("visible")) == Some(false) {
            return true;
        }
        // ES-DE's help, clock and status for its menus (`menu`) or for no
        // screen (`none`): the launcher's screens show the `view` ones.
        if matches!(el.get("scope").map(str::trim), Some("menu" | "none")) {
            return true;
        }
        if let Some((x, y)) = el.get("pos").and_then(pair) {
            if !(-1.0..1.5).contains(&x) || !(-1.0..1.5).contains(&y) {
                return true;
            }
        }
        false
    }

    // ------------------------------------------------------------ text

    /// A text element, or None when ES would not show it (size 0, no
    /// color, hidden).
    fn text(
        &mut self,
        el: &El,
        text: &str,
        pos: (f32, f32),
        size: (f32, f32),
        z: f32,
        wrap: Option<bool>,
    ) -> Option<Table> {
        if self.hidden(el) {
            return None;
        }
        let font_size = el.get("fontSize").and_then(num).unwrap_or(0.035);
        if font_size <= 0.001 {
            return None;
        }
        let ink = match el.prop("color") {
            Some(p) if p.value.trim().is_empty() => return None,
            Some(p) => match color(&p.value) {
                Some((_, 0)) => return None,
                c => c,
            },
            None => None,
        };
        let (text, collection) = collection_name(text)?;
        let mut t = Table::new();
        t.insert("kind".into(), s("text"));
        t.insert("text".into(), s(data(&text, false)));
        if let Some(collection) = collection {
            let mut when = Table::new();
            when.insert("collection".into(), Value::Boolean(collection));
            t.insert("when".into(), Value::Table(when));
            self.mapped(
                "${system.fullName.noCollections} / .autoCollections",
                "`${system.name}` com `when.collection`",
            );
        }
        let mut g = self.geo(el, pos, size, (0., 0.), None);
        // A size 0 is as wide as the text: from the origin to the edge.
        let auto_w = el.get("size").and_then(pair).is_none_or(|(w, _)| w <= 0.)
            && el.get("maxSize").is_none();
        let line = font_size * 1.5;
        if el.get("size").and_then(pair).is_none_or(|(_, h)| h <= 0.) {
            g.h = line;
        }
        if auto_w {
            g.w = if g.ox >= 0.75 {
                g.x
            } else if g.ox <= 0.25 {
                1. - g.x
            } else {
                2. * g.x.min(1. - g.x)
            }
            .max(0.05);
        }
        self.place(&mut t, &g);
        if auto_w {
            t.insert(AUTO_W.into(), Value::Boolean(true));
        }
        if let Some(font) = el.get("fontPath").filter(|f| !f.is_empty()) {
            t.insert("font".into(), s(font));
            self.file(font, font);
        }
        t.insert("size".into(), f(font_size.clamp(0.01, 0.3)));
        if let Some((c, _)) = ink {
            t.insert("color".into(), s(c));
        }
        // `horizontalAlignment` is ES-DE's spelling.
        let align = match el
            .get("alignment")
            .or_else(|| el.get("horizontalAlignment"))
            .map(str::trim)
        {
            Some("center") => Some("center"),
            Some("right") => Some("right"),
            Some("left") | Some("justify") => None,
            _ if auto_w && g.ox >= 0.75 => Some("right"),
            _ if auto_w && (g.ox - 0.5).abs() < 0.25 => Some("center"),
            _ => None,
        };
        if let Some(a) = align {
            t.insert("align".into(), s(a));
        }
        let wrap = wrap.unwrap_or(g.h >= font_size * 2.4);
        // An ES-DE container scrolls its text from the top: its
        // verticalAlignment does not apply.
        let container = self.esde && flag(el.get("container")) == Some(true);
        let valign = if container {
            Some("top")
        } else {
            el.get("verticalAlignment").map(str::trim)
        };
        match valign {
            Some("top") if !wrap => {
                t.insert("valign".into(), s("top"));
            }
            Some("bottom") => {
                t.insert("valign".into(), s("bottom"));
            }
            Some("center") if wrap => {
                t.insert("valign".into(), s("middle"));
            }
            _ => {}
        }
        if flag(el.get("forceUppercase")) == Some(true)
            || el.get("letterCase").map(str::trim) == Some("uppercase")
        {
            t.insert("upper".into(), Value::Boolean(true));
        }
        if wrap {
            t.insert("wrap".into(), Value::Boolean(true));
            if let Some(ls) = el.get("lineSpacing").and_then(num) {
                // ES-DE's lineSpacing is the line's height in font sizes,
                // as the launcher's `line`; the older ES's runs a little
                // tighter.
                let k = if self.esde { 1. } else { 1.1 };
                t.insert("line".into(), f((ls * k).clamp(0.8, 3.)));
            }
        }
        if let Some((glow, a)) = el.get("glowColor").and_then(color) {
            if a > 0 {
                let mut sh = Table::new();
                sh.insert("color".into(), s(glow));
                sh.insert("x".into(), f(0.0021));
                sh.insert("y".into(), f(0.0021));
                t.insert("shadow".into(), Value::Table(sh));
            }
        }
        self.common(&mut t, el, z);
        Some(t)
    }

    /// A backgroundColor behind a text: a caixa of its own, just under it.
    fn backdrop(&self, el: &El, text: &Table) -> Option<Table> {
        let (c, a) = el.get("backgroundColor").and_then(color)?;
        if a == 0 {
            return None;
        }
        let mut t = Table::new();
        t.insert("kind".into(), s("rect"));
        for key in ["rect", "anchor", "rotate", "pivot", "opacity", "enter"] {
            if let Some(v) = text.get(key) {
                t.insert(key.into(), v.clone());
            }
        }
        t.insert("fill".into(), s(c));
        let z = text.get("z").and_then(Value::as_integer).unwrap_or(0);
        t.insert("z".into(), Value::Integer(z));
        Some(t)
    }

    // ------------------------------------------------------------ files

    /// Remember a theme file (or pattern) to copy, at `to` in the output.
    fn file(&mut self, from: &str, to: &str) {
        self.out.files.insert((from.to_owned(), to.to_owned()));
    }
    /// The element's `<path>`s as alternatives, the one ES would pick
    /// first (the last that exists); files that are not there left out.
    fn sources(&mut self, el: &El) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in el.paths.iter().rev() {
            if out.contains(p) {
                continue;
            }
            if p.contains("${") || self.has(p) {
                out.push(p.clone());
            } else {
                let p = p.clone();
                self.note(format!("arquivo que não existe no tema: `{p}`"));
            }
        }
        out
    }
    fn picture_sources(&mut self, el: &El) -> Vec<String> {
        let found = self.sources(el);
        for p in &found {
            self.file(p, p);
        }
        found.iter().map(|p| data(p, false)).collect()
    }

    // ------------------------------------------------------------ images

    #[allow(clippy::too_many_arguments)]
    fn image(
        &mut self,
        el: &El,
        src: Vec<String>,
        pos: (f32, f32),
        size: (f32, f32),
        origin: (f32, f32),
        z: f32,
        picture: Option<&str>,
    ) -> Option<Table> {
        if self.hidden(el) || src.is_empty() {
            return None;
        }
        let mut t = Table::new();
        let g = self.geo(el, pos, size, origin, picture);
        let tile = flag(el.get("tile")) == Some(true);
        let (tint, alpha) = el
            .get("color")
            .and_then(color)
            .unwrap_or(("#FFFFFFFF".into(), 255));
        if alpha == 0 {
            return None;
        }
        // A picture of one solid color (a theme's 1-pixel "pixel.png"
        // stretched over a box, tinted): a box of that color, drawn the
        // same on every launcher, rounded corners and all.
        if let Some(rgb) = (src.len() == 1)
            .then(|| solid(self.root, &src[0]))
            .flatten()
        {
            let k = |i: usize, c: u8| {
                let t = u8::from_str_radix(&tint[1 + 2 * i..3 + 2 * i], 16).unwrap_or(255);
                (u16::from(c) * u16::from(t) / 255) as u8
            };
            let fill = format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                k(0, rgb[0]),
                k(1, rgb[1]),
                k(2, rgb[2]),
                alpha
            );
            t.insert("kind".into(), s("rect"));
            t.insert("fill".into(), s(fill));
            if let Some(r) = el
                .get("roundCorners")
                .or_else(|| el.get("cornerRadius"))
                .and_then(num)
                .filter(|r| *r > 0.)
            {
                t.insert("radius".into(), f(r.min(0.5)));
            }
            self.place(&mut t, &g);
            self.common(&mut t, el, z);
            self.mapped("image de uma cor só", "rect (fill)");
            return Some(t);
        }
        if tile {
            // A tiled picture: a frame with no edges, repeated.
            t.insert("kind".into(), s("frame"));
            t.insert("image".into(), s(src[0].clone()));
            t.insert("slice".into(), Value::Integer(0));
            t.insert("tile".into(), Value::Boolean(true));
            if !tint.starts_with("#FFFFFF") {
                t.insert("tint".into(), s(tint.clone()));
                self.mapped("image com tile e color", "frame (slice 0, tile, tint)");
            }
            self.mapped("image com tile", "frame (slice 0, tile)");
        } else {
            t.insert("kind".into(), s("image"));
            // A photo over the whole screen is cut, never squashed, on a
            // screen of another shape; a PNG cut to the screen stretches.
            let photo = src.iter().all(|p| {
                let p = p.to_lowercase();
                p.ends_with(".jpg") || p.ends_with(".jpeg")
            });
            if src.len() == 1 {
                t.insert("src".into(), s(src[0].clone()));
            } else {
                t.insert("src".into(), Value::Array(src.into_iter().map(s).collect()));
            }
            let fit = if g.fit == "stretch" && photo && g.w >= 0.99 && g.h >= 0.99 {
                "cover"
            } else {
                g.fit
            };
            if fit != "contain" {
                t.insert("fit".into(), s(fit));
            }
            if g.fit == "contain" && g.w >= 3.9 {
                // Kept in the middle of a box wider than the screen.
            } else if g.fit == "contain" && g.ox <= 0.01 {
                t.insert("align".into(), s("left"));
            } else if g.fit == "contain" && g.ox >= 0.99 {
                t.insert("align".into(), s("right"));
            }
            if !tint.starts_with("#FFFFFF") {
                t.insert("tint".into(), s(format!("{}FF", &tint[..7])));
            }
            if let Some(r) = el
                .get("roundCorners")
                .or_else(|| el.get("cornerRadius"))
                .or_else(|| el.get("imageCornerRadius"))
                .or_else(|| el.get("videoCornerRadius"))
                .and_then(num)
                .filter(|r| *r > 0.)
            {
                t.insert("radius".into(), f(r.min(0.5)));
            }
        }
        self.place(&mut t, &g);
        self.common(&mut t, el, z);
        if alpha < 255 {
            let o = t.get("opacity").and_then(Value::as_float).unwrap_or(1.) as f32;
            t.insert("opacity".into(), f(o * f32::from(alpha) / 255.));
        }
        Some(t)
    }

    // ------------------------------------------------------------ run

    fn run(&mut self, es: &Es) {
        // The gamelist views that become the launcher's views.
        let builtin = ["basic", "detailed", "video", "grid"];
        let mut names: Vec<String> = Vec::new();
        if self.esde {
            if es.view("gamelist").is_some() {
                names.push("gamelist".into());
            }
        } else {
            if es.view("detailed").is_some() {
                names.push("detailed".into());
            } else if es.view("basic").is_some() {
                names.push("basic".into());
            }
            for v in ["video", "grid"] {
                if es.view(v).is_some_and(|view| !view.els.is_empty()) {
                    names.push(v.into());
                }
            }
            for v in &es.views {
                if v.custom && !builtin.contains(&v.name.as_str()) && !names.contains(&v.name) {
                    names.push(v.name.clone());
                }
            }
        }
        if names.is_empty() {
            self.note("o tema não tem lista de jogos (basic, detailed, grid ou gamelist): a tela de jogos fica a de sempre");
        }
        // The default view first.
        if let Some(d) = &es.default_view {
            if let Some(at) = names.iter().position(|n| n == d) {
                let first = names.remove(at);
                names.insert(0, first);
            }
        }
        let system = es.view("system").cloned().unwrap_or_default();
        let screen = es.view("screen").cloned().unwrap_or_default();
        for v in ["menu"] {
            if es.view(v).is_some() {
                self.note("a tela `menu` do ES não tem equivalente: os menus do launcher seguem as cores do tema");
            }
        }
        let systems = self.systems(&system, &screen);
        let mut used = BTreeSet::new();
        for name in names {
            let Some(view) = es.flattened(&name) else {
                continue;
            };
            let mut id = slug(&name);
            if id == "lista" || used.contains(&id) {
                id = format!("{id}-es");
            }
            used.insert(id.clone());
            let display = view.display.clone().unwrap_or_else(|| title(&name));
            let (games, collection, description) = self.games(&view, &screen);
            self.out.views.push(OutView {
                id,
                name: display,
                systems: systems.clone(),
                games,
                collection,
                description,
            });
        }
        if self.out.views.is_empty() && !systems.is_empty() {
            // Only a systems screen: one view with it.
            self.out.views.push(OutView {
                id: "es".into(),
                name: "ES".into(),
                systems,
                games: Vec::new(),
                collection: None,
                description: None,
            });
        }
        self.top(es);
    }

    /// Colors and font of the launcher's other screens, from the list.
    fn top(&mut self, es: &Es) {
        let list = ["detailed", "basic", "gamelist"]
            .iter()
            .filter_map(|v| es.view(v))
            .find_map(|v| v.els.iter().find(|e| e.kind == "textlist").cloned());
        let mut top = Table::new();
        top.insert("background".into(), s("#000000"));
        if let Some(l) = list {
            if let Some((c, _)) = l.get("selectorColor").and_then(color) {
                top.insert("accent".into(), s(c));
            }
            if let Some((c, _)) = l.get("selectedColor").and_then(color) {
                top.insert("accent_text".into(), s(c));
            }
            if let Some((c, _)) = l.get("primaryColor").and_then(color) {
                top.insert("text".into(), s(c));
            }
            if let Some(font) = l
                .get("fontPath")
                .filter(|f| f.ends_with(".ttf") || f.ends_with(".otf"))
            {
                top.insert("font".into(), s(font));
                self.file(font, font);
            }
        }
        self.out.top = top;
    }

    // ------------------------------------------------------------ systems

    fn systems(&mut self, view: &View, screen: &View) -> Vec<OutEl> {
        let mut out = Vec::new();
        let logo = view.el("logo").cloned();
        let logo_text = view.el("logoText").cloned();
        let mut carousel = false;
        for el in &view.els {
            let made: Vec<Table> = match (el.kind.as_str(), el.name.as_str()) {
                ("carousel", _) if !carousel => {
                    carousel = true;
                    self.carousel(el, logo.as_ref(), logo_text.as_ref())
                        .into_iter()
                        .collect()
                }
                // ES-DE's systems as a grid of logos, or as a list.
                ("grid", _) if self.esde && !carousel => {
                    carousel = true;
                    self.esde_grid(el, true).into_iter().collect()
                }
                ("textlist", _) if self.esde && !carousel => {
                    carousel = true;
                    self.list(el, false).into_iter().collect()
                }
                ("text", "systemInfo") => {
                    self.mapped("systemInfo", "text `${system.games}`");
                    let mut v = Vec::new();
                    if let Some(t) = self.text(
                        el,
                        "${system.games}",
                        (0., 0.65),
                        (1., 0.),
                        50.,
                        Some(false),
                    ) {
                        v.extend(self.backdrop(el, &t));
                        v.push(t);
                    }
                    v
                }
                ("image", "logo") | ("text", "logoText") => Vec::new(),
                ("helpsystem", _) => self.hints(el).into_iter().collect(),
                _ if el.extra || self.esde => self.extra(el, esde_z(&el.kind, self.esde)),
                // A gamelist's element written for every view: ES skips it here.
                (_, name) if crate::es::builtin_kind(name).is_some() => Vec::new(),
                (kind, name) => {
                    let (kind, name) = (kind.to_owned(), name.to_owned());
                    self.note(format!("elemento `{kind} {name}` da tela de sistemas não tem equivalente; ignorado"));
                    Vec::new()
                }
            };
            for t in made {
                let id = unique(&out, &el.name);
                out.push(OutEl { id, t });
            }
        }
        if !carousel {
            // ES draws its carousel of logos when the theme says nothing.
            let default = El {
                kind: "carousel".into(),
                name: "systemcarousel".into(),
                extra: false,
                props: Vec::new(),
                paths: Vec::new(),
                story: Vec::new(),
            };
            if let Some(t) = self.carousel(&default, logo.as_ref(), logo_text.as_ref()) {
                out.push(OutEl {
                    id: "systemcarousel".into(),
                    t,
                });
            }
        }
        self.screen_view(screen, &mut out);
        fit_widths(&mut out);
        lift_over_hints(&mut out);
        self.one_video(&mut out, "sistemas");
        self.gif_stills(&mut out);
        self.one_status(&mut out);
        out
    }

    /// The system carousel: a row, a column or a wheel of logos.
    fn carousel(&mut self, el: &El, logo: Option<&El>, logo_text: Option<&El>) -> Option<Table> {
        let get = |a: &str, b: &str| el.get(a).or_else(|| el.get(b));
        let kind = get("type", "type")
            .unwrap_or("horizontal")
            .replace("Wheel", "_wheel")
            .to_lowercase();
        let vertical = kind.starts_with("vertical");
        let mut t = Table::new();
        t.insert("kind".into(), s("carousel"));
        t.insert("of".into(), s("systems"));
        t.insert(
            "axis".into(),
            s(if vertical { "vertical" } else { "horizontal" }),
        );
        let g = self.geo(el, (0., 0.38375), (1., 0.2325), (0., 0.), None);
        // ES-DE moves the items off the middle by a fraction of the box
        // (`horizontalOffset`, `verticalOffset`, along and across): the
        // launcher keeps them in the middle, so the box moves instead.
        let shift = |key: &str| el.get(key).and_then(num).unwrap_or(0.).clamp(-1., 1.);
        let g = Geo {
            fit: "contain",
            x: g.x + shift("horizontalOffset") * g.w,
            y: g.y + shift("verticalOffset") * g.h,
            ..g
        };
        self.place(&mut t, &g);
        let visible = get("maxLogoCount", "maxItemCount")
            .and_then(num)
            .unwrap_or(3.)
            .round()
            .clamp(1., 15.);
        t.insert("visible".into(), Value::Integer(visible as i64));
        if kind.ends_with("_wheel") {
            let mut w = Table::new();
            w.insert(
                "angle".into(),
                f(get("logoRotation", "itemRotation")
                    .and_then(num)
                    .unwrap_or(7.5)
                    .clamp(-90., 90.)),
            );
            let (px, py) = get("logoRotationOrigin", "itemRotationOrigin")
                .and_then(pair)
                .unwrap_or((-5., 0.5));
            w.insert(
                "pivot".into(),
                Value::Array(vec![f(px.clamp(-10., 10.)), f(py.clamp(-10., 10.))]),
            );
            t.insert("wheel".into(), Value::Table(w));
        }
        let mut item = Table::new();
        let mut images = Vec::new();
        let logo_paths = logo.map(|l| self.sources(l)).unwrap_or_default();
        // ES-DE shows `defaultImage` where `staticImage` has no file.
        let static_image: Vec<String> = ["staticImage", "defaultImage"]
            .iter()
            .filter_map(|k| el.get(k).filter(|p| !p.is_empty()).map(str::to_owned))
            .collect();
        for p in logo_paths.iter().chain(static_image.iter()) {
            let src = self.logo_source(p);
            if !images.contains(&src) {
                images.push(src);
            }
        }
        if images.is_empty() {
            images.push("system.logo".into());
        }
        item.insert(
            "image".into(),
            if images.len() == 1 {
                s(images[0].clone())
            } else {
                Value::Array(images.into_iter().map(s).collect())
            },
        );
        item.insert("text".into(), s("${item.name}"));
        if let Some(lt) = logo_text {
            if let Some(font) = lt.get("fontPath").filter(|f| !f.is_empty()) {
                item.insert("font".into(), s(font));
                self.file(font, font);
            }
            if let Some(size) = lt.get("fontSize").and_then(num).filter(|n| *n > 0.) {
                item.insert("size".into(), f(size.clamp(0.01, 0.3)));
            }
            if let Some((c, _)) = lt.get("color").and_then(color) {
                item.insert("color".into(), s(c));
            }
        }
        let (lw, lh) = get("logoSize", "itemSize")
            .and_then(pair)
            .unwrap_or((0.25, 0.155));
        item.insert("w".into(), f(lw.clamp(0.01, 1.)));
        item.insert("h".into(), f(lh.clamp(0.01, 1.)));
        t.insert("item".into(), Value::Table(item));
        let scale = get("logoScale", "itemScale")
            .and_then(num)
            .unwrap_or(1.2)
            .clamp(0.25, 4.);
        let mut sel = Table::new();
        sel.insert("scale".into(), f(scale));
        // ES-DE's color for the focused item's picture (a white logo on a
        // white selector turns black).
        if let Some((c, _)) = el.get("imageSelectedColor").and_then(color) {
            sel.insert("tint".into(), s(c));
        }
        sel.insert("opacity".into(), f(1.));
        t.insert("selected".into(), Value::Table(sel));
        let dim = get("minLogoOpacity", "unfocusedItemOpacity")
            .and_then(num)
            .unwrap_or(0.5);
        t.insert("opacity".into(), f(dim.clamp(0., 1.)));
        if vertical {
            match get("logoAlignment", "itemHorizontalAlignment").map(str::trim) {
                Some("left") => {
                    t.insert("align".into(), s("left"));
                }
                Some("right") => {
                    t.insert("align".into(), s("right"));
                }
                _ => {}
            }
        }
        t.insert("loop".into(), Value::Boolean(true));
        self.common(&mut t, el, if self.esde { 50. } else { 40. });
        for key in ["logoPos", "colorEnd", "gradientType"] {
            if el.get(key).is_some() {
                self.note(format!(
                    "carrossel: `{key}` não existe no launcher; ignorado"
                ));
            }
        }
        self.mapped(
            &format!("carousel {} ({kind})", el.name),
            "carousel de sistemas",
        );
        Some(t)
    }

    /// A system logo pattern: copied into `logos/` and read as
    /// `system.logo`, which also finds the name a system shares.
    fn logo_source(&mut self, path: &str) -> String {
        let Some((dir, name)) = path.rsplit_once('/') else {
            self.file(path, path);
            return data(path, true);
        };
        // One folder of logos becomes `logos/` (`logo_dir`, the systems
        // carousel's of the theme's first read); another folder (a variant's
        // own drawings) keeps its path, or the two would mix in `logos/`.
        let ours = self.logo.as_deref().is_none_or(|d| d == dir);
        if ours && (name.starts_with("${system.theme}.") || name.starts_with("${system.name}.")) {
            let ext = name.rsplit_once('.').map_or("png", |(_, e)| e);
            self.file(path, &format!("logos/${{system.theme}}.{ext}"));
            if self.logo.is_none() {
                self.logo = Some(dir.to_owned());
                self.mapped(
                    &format!("{dir}/${{system.theme}}"),
                    "logos/ (o `system.logo` do launcher)",
                );
            }
            "system.logo".into()
        } else {
            self.file(path, path);
            data(path, true)
        }
    }

    fn hints(&mut self, el: &El) -> Option<Table> {
        if self.hidden(el) {
            return None;
        }
        let (x, y) = el.get("pos").and_then(pair).unwrap_or((0.012, 0.9515));
        let (ox, oy) = el.get("origin").and_then(pair).unwrap_or((0., 0.));
        let size = el.get("fontSize").and_then(num).unwrap_or(0.035).max(0.02);
        let h = (size * 2.).max(0.045);
        let (rx, w, align) = if ox >= 0.75 {
            (x - 0.96, 0.96, "right")
        } else if ox > 0.25 {
            let w = 2. * x.min(1. - x);
            (x - w / 2., w, "center")
        } else {
            (x, (0.985 - x).max(0.1), "left")
        };
        let mut t = Table::new();
        t.insert("kind".into(), s("hints"));
        // Inside the screen: ES-DE's help may hang below its edge, where the
        // launcher's would be cut.
        let top = (y - oy * h).clamp(0., 1. - h);
        t.insert("rect".into(), rect(rx.max(0.), top, w, h));
        t.insert("align".into(), s(align));
        // ES-DE's help is gray (777777) where the theme says no color.
        let gray = self.esde.then_some("777777");
        if let Some((c, _)) = el.get("textColor").or(gray).and_then(color) {
            t.insert("color".into(), s(c));
        }
        if let Some((c, _)) = el.get("iconColor").or(gray).and_then(color) {
            t.insert("badge".into(), s(c));
        }
        t.insert("z".into(), Value::Integer(HINTS_Z));
        if el.get("fontPath").is_some() || el.get("fontSize").is_some() {
            self.note("dicas de botões (`helpsystem`): fonte e tamanho são os do launcher");
        }
        self.mapped("helpsystem", "hints");
        Some(t)
    }

    /// The `screen` view's clock and battery, on every screen.
    fn screen_view(&mut self, screen: &View, out: &mut Vec<OutEl>) {
        let battery = screen.el("batteryIndicator").filter(|e| !self.hidden(e));
        let clock = screen.el("clock").filter(|e| !self.hidden(e));
        let Some(at) = battery.or(clock) else {
            if screen.el("controllerActivity").is_some() {
                self.note("`controllerActivity` (atividade dos controles) não existe no launcher; ignorado");
            }
            return;
        };
        let (x, y) = at.get("pos").and_then(pair).unwrap_or((0.9, 0.02));
        let (sw, sh) = at.get("size").and_then(pair).unwrap_or((0.05, 0.05));
        let right = (x + sw).min(0.995);
        let mut t = Table::new();
        t.insert("kind".into(), s("status"));
        t.insert("rect".into(), rect(right - 0.3, y, 0.3, sh.max(0.04)));
        if let Some((c, _)) = at.get("color").and_then(color) {
            t.insert("color".into(), s(c));
        }
        t.insert("z".into(), Value::Integer(STATUS_Z));
        self.mapped(
            "clock / batteryIndicator",
            "status (relógio, bateria e rádios do launcher)",
        );
        out.push(OutEl {
            id: "status".into(),
            t,
        });
    }

    // ------------------------------------------------------------ games

    fn games(
        &mut self,
        view: &View,
        screen: &View,
    ) -> (Vec<OutEl>, Option<String>, Option<String>) {
        let mut out: Vec<OutEl> = Vec::new();
        let mut collection = None;
        let mut description = None;
        let detailed = view.name != "basic";
        let logo_text = view.el("logoText").cloned();
        let has_logo = view
            .el("logo")
            .is_some_and(|l| !l.paths.is_empty() && !self.hidden(l));
        let tiles: Vec<El> = view
            .els
            .iter()
            .filter(|e| e.kind == "gridtile" || e.name.starts_with("gridtile"))
            .cloned()
            .collect();
        // Built-in elements first, then the extras: ES's order for equal z.
        let mut order: Vec<&El> = view.els.iter().filter(|e| !e.extra).collect();
        order.extend(view.els.iter().filter(|e| e.extra));
        for el in order {
            if el.name.starts_with("gridtile") || el.kind == "gridtile" {
                continue;
            }
            let made: Vec<Table> = match (el.kind.as_str(), el.name.as_str()) {
                ("textlist", _) | ("imagegrid", _) | ("carousel", _) | ("grid", _)
                    if collection.is_none() =>
                {
                    let made = if el.kind == "textlist" {
                        self.list(el, detailed)
                    } else if self.esde && el.kind == "grid" {
                        self.esde_grid(el, false)
                    } else if self.esde && el.kind == "carousel" {
                        self.esde_carousel(el)
                    } else {
                        self.grid(el, &tiles)
                    };
                    if made.is_some() {
                        collection = Some(el.name.clone());
                    }
                    made.into_iter().collect()
                }
                ("helpsystem", _) => self.hints(el).into_iter().collect(),
                ("image", "logo") => {
                    let src: Vec<String> = self
                        .sources(el)
                        .iter()
                        .map(|p| self.logo_source(p))
                        .collect();
                    let mut t = self.image(el, src, (0.5, 0.), (0.4, 0.13), (0.5, 0.), 50., None);
                    if let (Some(t), Some(lt)) = (t.as_mut(), logo_text.as_ref()) {
                        if let Some(fb) =
                            self.text(lt, "${system.name}", (0., 0.), (1., 0.1), 50., Some(false))
                        {
                            let mut fbt = Table::new();
                            for key in ["text", "font", "size", "color", "align", "upper", "shadow"]
                            {
                                if let Some(v) = fb.get(key) {
                                    fbt.insert(key.into(), v.clone());
                                }
                            }
                            fbt.entry("align").or_insert(s("center"));
                            t.insert("fallback_text".into(), Value::Table(fbt));
                        }
                    }
                    t.into_iter().collect()
                }
                // No logo picture: ES shows the system's name instead.
                ("text", "logoText") if !has_logo => {
                    let made = self.text(
                        el,
                        "${system.fullName}",
                        (0.5, 0.),
                        (1., 0.13),
                        50.,
                        Some(false),
                    );
                    if made.is_some() {
                        self.mapped("logoText", "text `${system.name}`");
                    }
                    made.into_iter().collect()
                }
                ("text", "logoText") => Vec::new(),
                ("image", "background") if !el.extra => {
                    let src = self.picture_sources(el);
                    let first = src.first().cloned();
                    self.image(el, src, (0., 0.), (1., 1.), (0., 0.), 0., first.as_deref())
                        .into_iter()
                        .collect()
                }
                (_, name) if name.starts_with("md_") && !el.extra => {
                    let made = self.metadata(el);
                    if name == "md_description" && !made.is_empty() {
                        description = Some(name.to_owned());
                    }
                    made
                }
                _ if el.extra || self.esde => self.extra(el, esde_z(&el.kind, self.esde)),
                (_, "systemcarousel" | "systemInfo") => Vec::new(),
                (_, "gamelist" | "gamegrid") => Vec::new(),
                (kind, name) => {
                    let (kind, name) = (kind.to_owned(), name.to_owned());
                    self.note(format!(
                        "elemento `{kind} {name}` da lista de jogos não tem equivalente; ignorado"
                    ));
                    Vec::new()
                }
            };
            for t in made {
                let id = unique(&out, &el.name);
                if t.get("kind").and_then(Value::as_str) == Some("text")
                    && t.get("text").and_then(Value::as_str) == Some("${game.description}")
                {
                    description = Some(id.clone());
                }
                out.push(OutEl { id, t });
            }
        }
        // The id the details screen removes.
        collection = out
            .iter()
            .find(|e| {
                matches!(
                    e.t.get("kind").and_then(Value::as_str),
                    Some("list" | "grid" | "carousel")
                )
            })
            .map(|e| e.id.clone());
        self.screen_view(screen, &mut out);
        fit_widths(&mut out);
        lift_over_hints(&mut out);
        self.one_video(&mut out, "jogos");
        self.gif_stills(&mut out);
        self.one_status(&mut out);
        (out, collection, description)
    }

    fn list(&mut self, el: &El, detailed: bool) -> Option<Table> {
        let mut t = Table::new();
        t.insert("kind".into(), s("list"));
        let (pos, size) = if detailed {
            ((0.51, 0.2), (0.49, 0.8))
        } else {
            ((0., 0.2), (1., 0.8))
        };
        if el.get("pos").is_none() {
            self.note("lista sem `pos`: posição padrão do ES (aproximada)");
        }
        let g = self.geo(el, pos, size, (0., 0.), None);
        self.place(&mut t, &g);
        let size = el
            .get("fontSize")
            .and_then(num)
            .unwrap_or(0.045)
            .clamp(0.01, 0.3);
        let ls = el.get("lineSpacing").and_then(num).unwrap_or(1.5);
        t.insert("size".into(), f(size));
        // ES's line is the font's tallest glyph times lineSpacing: about
        // 1.75 times the size (measured on Albedo's captures).
        let row = (size * ls * 1.75).clamp(0.02, 0.3);
        t.insert("row".into(), f(row));
        if let Some(font) = el.get("fontPath").filter(|f| !f.is_empty()) {
            t.insert("font".into(), s(font));
            self.file(font, font);
        }
        if let Some((c, _)) = el.get("primaryColor").and_then(color) {
            t.insert("color".into(), s(c));
        }
        match el
            .get("alignment")
            .or_else(|| el.get("horizontalAlignment"))
            .map(str::trim)
        {
            Some("center") => {
                t.insert("align".into(), s("center"));
            }
            Some("right") => {
                t.insert("align".into(), s("right"));
            }
            _ => {}
        }
        let mut sel = Table::new();
        // The launcher's highlight is at least a row tall (the row's when
        // left out).
        if let Some(h) = el
            .get("selectorHeight")
            .and_then(num)
            .filter(|h| *h > row + 0.001)
        {
            sel.insert("height".into(), f(h.min(0.3)));
        }
        if let Some((c, _)) = el.get("selectorColor").and_then(color) {
            sel.insert("fill".into(), s(c));
        }
        if let Some((c, _)) = el.get("selectedColor").and_then(color) {
            sel.insert("color".into(), s(c));
        }
        // ES-DE's picture for the selected row, in place of the bar: a
        // nine-slice frame whose corners are half the bar tall.
        let bar = sel
            .get("height")
            .and_then(Value::as_float)
            .unwrap_or(f64::from(row)) as f32;
        if let Some(frame) = self.selector_frame(el, "selectorImagePath", bar) {
            sel.insert("frame".into(), Value::Table(frame));
            sel.insert("fill".into(), s("#00000000"));
        }
        if !sel.is_empty() {
            t.insert("selected".into(), Value::Table(sel));
        }
        t.insert("scroll".into(), s("center"));
        // ES's selector bar has square corners.
        t.insert("radius".into(), f(0.));
        self.common(&mut t, el, if self.esde { 50. } else { 20. });
        for key in [
            "horizontalMargin",
            "selectorColorEnd",
            "scrollSound",
            "selectorOffsetY",
        ] {
            if el.get(key).is_some() {
                self.note(format!("lista: `{key}` não existe no launcher; ignorado"));
            }
        }
        self.mapped(&format!("textlist {}", el.name), "list");
        Some(t)
    }

    /// ES-DE's picture for the selection (`key`), tinted by
    /// `selectorColor`: a nine-slice frame whose corners are half of
    /// `tall` (a fraction of the screen height).
    fn selector_frame(&mut self, el: &El, key: &str, tall: f32) -> Option<Table> {
        let p = el
            .get(key)
            .filter(|p| !p.contains("${") && self.has(p))?
            .to_owned();
        self.file(&p, &p);
        let mut frame = Table::new();
        frame.insert("image".into(), s(p.clone()));
        match picture_pixels(self.root, &p) {
            Some((pw, ph)) if pw.min(ph) >= 3. => {
                // Drawn at least a quarter of their pixels (the launcher's
                // smallest scale).
                let half = tall * 480. / 2.;
                let slice = (pw.min(ph) / 3.).min(half / 0.25).round().max(1.);
                frame.insert("slice".into(), Value::Integer(slice as i64));
                frame.insert("scale".into(), f((half / slice).clamp(0.25, 4.)));
            }
            _ => {
                frame.insert("slice".into(), Value::Integer(0));
            }
        }
        if let Some((c, _)) = el.get("selectorColor").and_then(color) {
            if !c.starts_with("#FFFFFF") {
                frame.insert("tint".into(), s(c));
            }
        }
        Some(frame)
    }

    /// The pictures of an ES-DE collection's items: on the systems screen
    /// the theme's (`staticImage`, `defaultImage`) and the system's logo;
    /// on the games screen the game's (`imageType`) and the theme's
    /// `defaultImage`.
    fn esde_items(&mut self, el: &El, systems: bool) -> Vec<String> {
        let mut images = Vec::new();
        if systems {
            for key in ["staticImage", "defaultImage"] {
                if let Some(p) = el.get(key).filter(|p| !p.is_empty()).map(str::to_owned) {
                    images.push(self.logo_source(&p));
                }
            }
            if !images.iter().any(|i| i == "system.logo") {
                images.insert(usize::from(!images.is_empty()), "system.logo".into());
            }
        } else {
            let kinds = el.get("imageType").unwrap_or("marquee").to_owned();
            for kind in kinds.split(',').map(str::trim) {
                let src = match kind {
                    "cover" | "backcover" | "3dbox" | "physicalmedia" => "game.cover",
                    "marquee" => "game.logo",
                    "none" => continue,
                    _ => "game.screenshot",
                };
                if !images.iter().any(|i| i == src) {
                    images.push(src.into());
                }
            }
            if let Some(p) = el
                .get("defaultImage")
                .filter(|p| !p.contains("${") && self.has(p))
                .map(str::to_owned)
            {
                self.file(&p, &p);
                images.push(p);
            }
        }
        images
    }

    /// An ES-DE `grid` (systems or games): its columns and rows from its
    /// size, the items' size and the room between them, as ES-DE lays it.
    fn esde_grid(&mut self, el: &El, systems: bool) -> Option<Table> {
        if self.hidden(el) {
            return None;
        }
        let mut t = Table::new();
        t.insert("kind".into(), s("grid"));
        t.insert("of".into(), s(if systems { "systems" } else { "games" }));
        let g = self.geo(el, (0., 0.1), (1., 0.8), (0., 0.), None);
        // ES-DE: -1 on one side makes it as long as the other, in pixels.
        let sr = self.shape.ratio;
        let square = |(w, h): (f32, f32)| match (w < 0., h < 0.) {
            (true, false) => (h / sr, h),
            (false, true) => (w, w * sr),
            _ => (w.max(0.), h.max(0.)),
        };
        let (iw, ih) = square(el.get("itemSize").and_then(pair).unwrap_or((0.15, 0.25)));
        let (gx, gy) = square(el.get("itemSpacing").and_then(pair).unwrap_or((0.02, 0.03)));
        let fit = |room: f32, item: f32, gap: f32| {
            // A hair over: 0.85 / 0.28333 is three rows, not two.
            (((room + gap) / (item + gap).max(0.01) + 1e-3).floor() as i64).clamp(1, 8)
        };
        let (columns, rows) = (fit(g.w, iw, gx), fit(g.h, ih, gy));
        t.insert("columns".into(), Value::Integer(columns));
        t.insert("rows".into(), Value::Integer(rows));
        // The launcher shares its box among the cells: the box is made as
        // large as ES-DE's items and gaps, from its top left corner.
        let used = |n: i64, item: f32, gap: f32| n as f32 * item + (n - 1) as f32 * gap;
        let (w, h) = (used(columns, iw, gx).min(g.w), used(rows, ih, gy).min(g.h));
        let g = Geo {
            x: g.x - g.ox * g.w + g.ox * w,
            y: g.y - g.oy * g.h + g.oy * h,
            w,
            h,
            ..g
        };
        self.place(&mut t, &g);
        t.insert("spacing".into(), f(gy.clamp(0., 0.5)));
        let mut item = Table::new();
        let images = self.esde_items(el, systems);
        item.insert(
            "image".into(),
            Value::Array(images.into_iter().map(s).collect()),
        );
        item.insert("text".into(), s("${item.name}"));
        if let Some(fit) = match el.get("imageFit").map(str::trim) {
            Some("cover") => Some("cover"),
            Some("fill") => Some("stretch"),
            _ => None,
        } {
            item.insert("fit".into(), s(fit));
        }
        if let Some(r) = el
            .get("imageCornerRadius")
            .and_then(num)
            .filter(|r| *r > 0.)
        {
            item.insert("radius".into(), f(r.clamp(0., 0.5)));
        }
        if let Some((c, a)) = el.get("backgroundColor").and_then(color) {
            if a > 0 {
                item.insert("fill".into(), s(c));
            }
        }
        if let Some(font) = el.get("fontPath").filter(|f| !f.is_empty()) {
            item.insert("font".into(), s(font));
            self.file(font, font);
        }
        if let Some((c, _)) = el.get("textColor").and_then(color) {
            item.insert("color".into(), s(c));
        }
        t.insert("item".into(), Value::Table(item));
        let mut sel = Table::new();
        let scale = el
            .get("itemScale")
            .and_then(num)
            .unwrap_or(1.05)
            .clamp(0.25, 4.);
        sel.insert("scale".into(), f(scale));
        // ES-DE's color for the focused item's picture (a white logo on a
        // white selector turns black).
        if let Some((c, _)) = el.get("imageSelectedColor").and_then(color) {
            sel.insert("tint".into(), s(c));
        }
        sel.insert("opacity".into(), f(1.));
        // ES-DE draws its selector picture behind the item, larger: the
        // launcher's frame goes around it, so the color's ring instead.
        if let Some((c, a)) = el.get("selectorColor").and_then(color) {
            if a > 0 {
                let mut ring = Table::new();
                ring.insert("color".into(), s(c));
                ring.insert("width".into(), f(0.006));
                ring.insert("radius".into(), f(0.012));
                sel.insert("frame".into(), Value::Table(ring));
            }
        }
        t.insert("selected".into(), Value::Table(sel));
        if let Some(o) = el.get("unfocusedItemOpacity").and_then(num) {
            t.insert("opacity".into(), f(o.clamp(0., 1.)));
        }
        t.insert("loop".into(), Value::Boolean(false));
        self.common(&mut t, el, 50.);
        self.mapped(
            &format!("grid {} (ES-DE)", el.name),
            if systems {
                "grid de sistemas"
            } else {
                "grid de jogos"
            },
        );
        Some(t)
    }

    /// An ES-DE `carousel` of games: the systems' carousel with the
    /// game's pictures (`imageType`) for items.
    fn esde_carousel(&mut self, el: &El) -> Option<Table> {
        if self.hidden(el) {
            return None;
        }
        let mut t = self.carousel(el, None, None)?;
        t.insert("of".into(), s("games"));
        let images = self.esde_items(el, false);
        if let Some(Value::Table(item)) = t.get_mut("item") {
            item.insert(
                "image".into(),
                Value::Array(images.into_iter().map(s).collect()),
            );
        }
        if flag(el.get("reflections")) == Some(true) {
            let o = el.get("reflectionsOpacity").and_then(num).unwrap_or(0.5);
            t.insert("reflection".into(), f(o.clamp(0., 1.)));
        }
        if let Some(l) = el.get("wrapAround").and_then(|w| flag(Some(w))) {
            t.insert("loop".into(), Value::Boolean(l));
        }
        self.mapped(
            &format!("carousel {} (ES-DE)", el.name),
            "carousel de jogos",
        );
        Some(t)
    }

    /// An image grid: a grid, or a carousel when it is one row or column.
    fn grid(&mut self, el: &El, tiles: &[El]) -> Option<Table> {
        let tile = |name: &str| tiles.iter().find(|t| t.name == name);
        let (c, r) = el
            .get("autoLayout")
            .and_then(pair)
            .map_or((4, 2), |(c, r)| {
                (c.round().max(1.) as i64, r.round().max(1.) as i64)
            });
        let horizontal = el.get("scrollDirection").map(str::trim) == Some("horizontal");
        let mut t = Table::new();
        let carousel = (r == 1 && horizontal) || (c == 1 && !horizontal);
        if carousel {
            t.insert("kind".into(), s("carousel"));
            t.insert("of".into(), s("games"));
            t.insert(
                "axis".into(),
                s(if r == 1 { "horizontal" } else { "vertical" }),
            );
            t.insert("visible".into(), Value::Integer(c.max(r).clamp(1, 15)));
        } else {
            t.insert("kind".into(), s("grid"));
            t.insert("of".into(), s("games"));
            t.insert("columns".into(), Value::Integer(c.clamp(1, 8)));
            t.insert("rows".into(), Value::Integer(r.clamp(1, 8)));
            if horizontal {
                self.note(
                    "grade que rola na horizontal: no launcher a grade rola as linhas, na vertical",
                );
            }
        }
        let mut g = self.geo(el, (0., 0.2), (1., 0.8), (0., 0.), None);
        if let Some([l, tp, rr, b]) = el.get("padding").and_then(quad) {
            g.x += l;
            g.y += tp;
            g.w = (g.w - l - rr).max(0.05);
            g.h = (g.h - tp - b).max(0.05);
        }
        self.place(&mut t, &g);
        let mut item = Table::new();
        let images: Vec<&str> = match el
            .get("imageSource")
            .or_else(|| el.get("imageType"))
            .map(str::trim)
        {
            Some("image" | "screenshot") => vec!["game.screenshot", "game.cover"],
            Some("marquee" | "wheel") => vec!["game.logo", "game.cover"],
            _ => vec!["game.cover", "game.screenshot"],
        };
        let mut images: Vec<String> = images.into_iter().map(str::to_owned).collect();
        if let Some(icon) = el
            .get("gameImage")
            .filter(|p| self.has(p))
            .map(str::to_owned)
        {
            self.file(&icon, &icon);
            images.push(icon);
        }
        item.insert(
            "image".into(),
            Value::Array(images.into_iter().map(s).collect()),
        );
        let default = tile("default");
        let selected = tile("selected");
        if let Some(d) = default {
            match d.get("imageSizeMode").map(str::trim) {
                Some("minSize") => {
                    item.insert("fit".into(), s("cover"));
                }
                Some("size") => {
                    item.insert("fit".into(), s("stretch"));
                }
                _ => {}
            }
            if let Some((fill, a)) = d
                .get("backgroundCenterColor")
                .or_else(|| d.get("backgroundColor"))
                .and_then(color)
            {
                if a > 0 {
                    item.insert("fill".into(), s(fill));
                }
            }
            if let Some((_, a)) = d.get("imageColor").and_then(color) {
                t.insert("opacity".into(), f(f32::from(a) / 255.));
            }
            if carousel {
                if let Some(rf) = d.get("reflexion").and_then(pair).filter(|(a, _)| *a > 0.) {
                    t.insert("reflection".into(), f(rf.0.clamp(0., 1.)));
                }
            }
        }
        if let Some(r) = tile("gridtile.image")
            .and_then(|i| i.get("roundCorners"))
            .and_then(num)
        {
            item.insert("radius".into(), f(r.clamp(0., 0.5) * 0.25));
        }
        if tile("gridtile.marquee").is_some_and(|m| !self.hidden(m)) {
            item.insert("logo".into(), s("game.logo"));
        }
        if let Some(text) = tile("gridtile") {
            if let Some(size) = text.get("fontSize").and_then(num).filter(|n| *n > 0.) {
                item.insert("size".into(), f(size.clamp(0.01, 0.3)));
            }
            if let Some(font) = text.get("fontPath").filter(|f| !f.is_empty()) {
                item.insert("font".into(), s(font));
                self.file(font, font);
            }
            if flag(text.get("visible")) != Some(false) {
                self.note("grade: o nome embaixo de cada cartão (`text gridtile`) só aparece no launcher quando falta a imagem");
            }
        }
        t.insert("item".into(), Value::Table(item));
        let mut sel = Table::new();
        let zoom = el
            .get("autoLayoutSelectedZoom")
            .and_then(num)
            .unwrap_or(1.)
            .clamp(0.25, 4.);
        sel.insert("scale".into(), f(zoom));
        let sel_alpha = selected
            .and_then(|s| s.get("imageColor"))
            .and_then(color)
            .map_or(1., |(_, a)| f32::from(a) / 255.);
        sel.insert("opacity".into(), f(sel_alpha));
        // The chosen card's background (a color, or a nine-slice ES tints)
        // as a ring around it.
        let ring = selected
            .and_then(|s| {
                s.get("backgroundColor")
                    .or_else(|| s.get("backgroundCenterColor"))
            })
            .and_then(color)
            .filter(|(_, a)| *a > 0);
        if let Some((c, _)) = ring {
            let pad = selected
                .and_then(|s| s.get("padding"))
                .and_then(pair)
                .map_or(4., |p| p.0.max(1.));
            let mut frame = Table::new();
            frame.insert("color".into(), s(c));
            frame.insert("width".into(), f((pad / 480.).clamp(0.002, 0.03)));
            frame.insert("radius".into(), f(0.012));
            sel.insert("frame".into(), Value::Table(frame));
            self.mapped("gridtile selected (fundo)", "selected.frame (anel)");
        }
        t.insert("selected".into(), Value::Table(sel));
        if let Some(m) = el.get("margin").and_then(pair) {
            if !carousel {
                t.insert("spacing".into(), f(m.1.clamp(0., 1.)));
            }
        }
        if let Some(fav) = tile("gridtile.favorite") {
            let mut fv = Table::new();
            if let Some(p) = fav.paths.last().filter(|p| self.has(p)).cloned() {
                self.file(&p, &p);
                fv.insert("image".into(), s(p));
            }
            let (x, y) = fav.get("pos").and_then(pair).unwrap_or((0.75, 0.));
            let corner = match (y > 0.5, x > 0.4) {
                (false, true) => "top-right",
                (false, false) => "top-left",
                (true, true) => "bottom-right",
                (true, false) => "bottom-left",
            };
            fv.insert("corner".into(), s(corner));
            if let Some((_, h)) = fav
                .get("maxSize")
                .or_else(|| fav.get("size"))
                .and_then(pair)
            {
                fv.insert("size".into(), f(h.clamp(0.05, 1.)));
            }
            if let Some((c, _)) = fav.get("color").and_then(color) {
                fv.insert("tint".into(), s(format!("{}FF", &c[..7])));
            }
            t.insert("favorite".into(), Value::Table(fv));
        }
        t.insert(
            "loop".into(),
            Value::Boolean(flag(el.get("scrollLoop")) == Some(true) || carousel),
        );
        self.common(&mut t, el, 20.);
        if el.get("showVideoAtDelay").is_some() {
            self.note("grade com vídeo no cartão (`showVideoAtDelay`): o launcher não toca vídeo dentro da grade");
        }
        if tiles
            .iter()
            .any(|t| t.name.contains(':') || t.kind == "ninepatch")
        {
            self.note("grade: estados `:selected`/`:videoplaying` e `animateColor` do cartão não existem; vale o estado normal");
        }
        let what = if carousel {
            "carousel de jogos"
        } else {
            "grid de jogos"
        };
        self.mapped(&format!("imagegrid {} ({c}x{r})", el.name), what);
        Some(t)
    }

    /// The `md_*` elements of a gamelist view.
    fn metadata(&mut self, el: &El) -> Vec<Table> {
        let name = el.name.as_str();
        let label = |key: &str| match key {
            "md_lbl_rating" => Some("@rating"),
            "md_lbl_releasedate" => Some("@released"),
            "md_lbl_developer" => Some("@developer"),
            "md_lbl_publisher" => Some("@publisher"),
            "md_lbl_genre" => Some("@genre"),
            "md_lbl_players" => Some("@players"),
            "md_lbl_lastplayed" => Some("@last_played"),
            _ => None,
        };
        let value = |key: &str| match key {
            "md_name" => Some("${game.name}"),
            "md_description" => Some("${game.description}"),
            "md_developer" => Some("${game.developer}"),
            "md_publisher" => Some("${game.publisher}"),
            "md_genre" => Some("${game.genre}"),
            "md_players" => Some("${game.players}"),
            "md_releasedate" => Some("${game.year}"),
            "md_lastplayed" => Some("${game.last_played}"),
            _ => None,
        };
        let default_pos = (0.01, 0.65);
        let mut out = Vec::new();
        match el.kind.as_str() {
            "text" | "datetime" => {
                let text = el
                    .get("text")
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
                    .or_else(|| label(name).or_else(|| value(name)).map(str::to_owned));
                let Some(text) = text else {
                    if !self.hidden(el) && crate::es::builtin_kind(name).is_some() {
                        let name = name.to_owned();
                        self.note(format!("`{name}` não tem dado no launcher (contagem de partidas, favorito); ignorado"));
                    }
                    return out;
                };
                if el.kind == "datetime" && name == "md_releasedate" {
                    self.note("data de lançamento (`md_releasedate`): o launcher mostra só o ano");
                }
                if el.get("pos").is_none() {
                    self.note("elementos `md_*` sem `pos`: posição padrão do ES (aproximada)");
                }
                let wrap = (name == "md_description").then_some(true);
                let size = if name == "md_description" {
                    (0.48, 0.15)
                } else {
                    (0.24, 0.)
                };
                if let Some(t) = self.text(el, &text, default_pos, size, 40., wrap) {
                    out.extend(self.backdrop(el, &t));
                    out.push(t);
                    self.mapped(&format!("{} {name}", el.kind), &format!("text `{text}`"));
                }
            }
            "image" => {
                let (data_src, z, pos): (&[&str], f32, (f32, f32)) = match name {
                    "md_image" => (&["game.screenshot", "game.cover"], 30., (0.25, 0.4125)),
                    "md_thumbnail" => (&["game.cover", "game.screenshot"], 30., (0.25, 0.4125)),
                    "md_marquee" | "md_wheel" => (&["game.logo"], 35., (0.25, 0.25)),
                    "md_fanart" => (&["game.screenshot"], 10., (0.5, 0.5)),
                    _ => {
                        let name = name.to_owned();
                        self.note(format!(
                            "imagem `{name}` não tem dado no launcher; ignorada"
                        ));
                        return out;
                    }
                };
                let src: Vec<String> = data_src.iter().map(|d| (*d).to_owned()).collect();
                let fallback = self.picture_sources(el);
                // ES centers the game's pictures and clip on their pos.
                let origin = (0.5, 0.5);
                let mut el2 = el.clone();
                if el.get("maxSize").is_none()
                    && el.get("size").is_none()
                    && el.get("minSize").is_none()
                    && el.get("cropSize").is_none()
                {
                    el2.props.push((
                        "maxSize".into(),
                        crate::es::Prop {
                            value: "0.48 0.4".into(),
                            var: None,
                        },
                    ));
                }
                if let Some(mut t) = self.image(&el2, src, pos, (0.48, 0.4), origin, z, None) {
                    if let Some(fb) = fallback.first() {
                        t.insert("fallback".into(), s(fb.clone()));
                    }
                    if name == "md_marquee" {
                        // No game logo: the game's name, as ES leaves it blank.
                        t.insert("when".into(), {
                            let mut w = Table::new();
                            w.insert("has".into(), s("game.logo"));
                            Value::Table(w)
                        });
                    }
                    out.push(t);
                    self.mapped(
                        &format!("image {name}"),
                        &format!("image `{}`", data_src.join(", ")),
                    );
                }
            }
            "video" => {
                if let Some(t) = self.video(el, &["game.video"], 30.) {
                    out.push(t);
                }
            }
            "rating" => {
                if let Some(t) = self.rating(el) {
                    out.push(t);
                }
            }
            kind => {
                let (kind, name) = (kind.to_owned(), name.to_owned());
                self.note(format!(
                    "elemento `{kind} {name}` não tem equivalente; ignorado"
                ));
            }
        }
        out
    }

    fn video(&mut self, el: &El, src: &[&str], z: f32) -> Option<Table> {
        if self.hidden(el) {
            return None;
        }
        let mut t = Table::new();
        t.insert("kind".into(), s("video"));
        let own = self.sources(el);
        let mut clips: Vec<String> = src.iter().map(|c| (*c).to_owned()).collect();
        let is_clip = |p: &str| p.ends_with(".mp4") || p.ends_with(".mkv") || p.ends_with(".avi");
        // ES-DE's `default`: the clip of a system without its own.
        let default = el
            .get("default")
            .filter(|p| is_clip(p) && !p.contains("${") && self.has(p))
            .map(str::to_owned);
        for p in own.iter().filter(|p| is_clip(p)).chain(default.iter()) {
            self.file(p, p);
            clips.push(data(p, false));
        }
        if clips.is_empty() {
            return None;
        }
        // A theme's clip still has a picture until it plays and with the
        // clips off: the theme's own picture of each system (a folder of
        // pictures beside the clips', by the same names), then ES-DE's
        // `defaultImage`.
        let mut stills: Vec<String> = Vec::new();
        for p in own.iter().filter(|p| is_clip(p) && p.contains("${")) {
            stills.extend(self.sibling_pictures(p));
        }
        if let Some(p) = el
            .get("defaultImage")
            .filter(|p| !p.contains("${") && self.has(p))
            .map(str::to_owned)
        {
            self.file(&p, &p);
            stills.push(p);
        }
        t.insert(
            "src".into(),
            if clips.len() == 1 {
                s(clips[0].clone())
            } else {
                Value::Array(clips.into_iter().map(s).collect())
            },
        );
        let snapshot = match el.get("snapshotSource").map(str::trim) {
            Some("thumbnail") => vec!["game.cover", "game.screenshot"],
            Some("marquee") => vec!["game.logo"],
            _ => vec!["game.screenshot", "game.cover"],
        };
        let mut fallback: Vec<String> = Vec::new();
        if flag(el.get("showSnapshotNoVideo")) != Some(false) && src.contains(&"game.video") {
            fallback.extend(snapshot.into_iter().map(str::to_owned));
        }
        fallback.extend(stills);
        if !fallback.is_empty() {
            t.insert(
                "fallback".into(),
                Value::Array(fallback.into_iter().map(s).collect()),
            );
        }
        // ES centers the game's clip on its pos.
        let origin = if el.name.starts_with("md_") {
            (0.5, 0.5)
        } else {
            (0., 0.)
        };
        let mut el2 = el.clone();
        if el.get("maxSize").is_none()
            && el.get("size").is_none()
            && el.get("minSize").is_none()
            && el.get("cropSize").is_none()
        {
            el2.props.push((
                "maxSize".into(),
                crate::es::Prop {
                    value: "0.48 0.4".into(),
                    var: None,
                },
            ));
        }
        let g = self.geo(&el2, (0.25, 0.4125), (0.48, 0.4), origin, None);
        self.place(&mut t, &g);
        t.insert(
            "fit".into(),
            s(match g.fit {
                "stretch" => "stretch",
                "cover" => "cover",
                _ => "contain",
            }),
        );
        if let Some(r) = el
            .get("roundCorners")
            .or_else(|| el.get("cornerRadius"))
            .or_else(|| el.get("imageCornerRadius"))
            .or_else(|| el.get("videoCornerRadius"))
            .and_then(num)
            .filter(|r| *r > 0.)
        {
            t.insert("radius".into(), f(r.min(0.5)));
        }
        if let Some(d) = el.get("delay").and_then(num) {
            t.insert("delay".into(), f(d.clamp(0., 10.)));
        }
        self.common(&mut t, el, z);
        // ES-DE plays a clip's sound unless it says no; `one_video` reads
        // this mark and takes it away.
        if flag(el.get("audio")) != Some(false) {
            t.insert(AUDIO.into(), Value::Boolean(true));
        }
        self.mapped(&format!("video {}", el.name), "video");
        Some(t)
    }

    /// Pictures named as the clips of `clip` (`a/1080p/${system.theme}.mp4`)
    /// in a folder beside theirs (`a/images/<system>.webp`), as the
    /// launcher's sources.
    fn sibling_pictures(&mut self, clip: &str) -> Vec<String> {
        let Some((dir, name)) = clip.rsplit_once('/') else {
            return Vec::new();
        };
        let parent = dir.rsplit_once('/').map_or("", |(p, _)| p);
        let Some(stem) = name.rsplit_once('.').map(|(s, _)| s) else {
            return Vec::new();
        };
        let folder = if parent.is_empty() {
            Some(self.root.to_path_buf())
        } else {
            crate::es::contained(self.root, parent)
        };
        let Some(Ok(entries)) = folder.map(std::fs::read_dir) else {
            return Vec::new();
        };
        let mut folders: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if parent.is_empty() {
                    name
                } else {
                    format!("{parent}/{name}")
                }
            })
            .filter(|d| d != dir)
            .collect();
        folders.sort();
        let mut out = Vec::new();
        for folder in folders {
            for ext in ["webp", "png", "jpg"] {
                let pattern = format!("{folder}/{stem}.{ext}");
                if !matches(self.root, &pattern).is_empty() {
                    self.file(&pattern, &pattern);
                    out.push(data(&pattern, false));
                }
            }
        }
        out
    }

    /// A launcher screen plays one clip (one decoder on a 1.4 GHz
    /// handheld). With more, the game's or system's clip stays, else the
    /// first silent one; the others become their still picture (their
    /// `fallback`, or the clip's first frame), and a clip with sound
    /// under another, ES's background music, goes: the launcher is mute.
    fn one_video(&mut self, out: &mut Vec<OutEl>, screen: &str) {
        let is_video = |e: &OutEl| e.t.get("kind").and_then(Value::as_str) == Some("video");
        let item = |t: &Table| {
            let src = t.get("src").map(Value::to_string).unwrap_or_default();
            src.contains("game.video") || src.contains("system.video")
        };
        let rank = |t: &Table| {
            if item(t) {
                0
            } else if t.contains_key(AUDIO) {
                2
            } else {
                1
            }
        };
        let videos: Vec<usize> = (0..out.len()).filter(|&i| is_video(&out[i])).collect();
        let keep = videos.iter().copied().min_by_key(|&i| rank(&out[i].t));
        for &i in videos.iter().rev() {
            if Some(i) == keep {
                continue;
            }
            let id = out[i].id.clone();
            let t = &mut out[i].t;
            let still = match t.get("fallback").cloned() {
                Some(fallback) => Some(fallback),
                None if rank(t) == 2 => None,
                None => t
                    .get("src")
                    .and_then(Value::as_str)
                    .filter(|p| !p.contains("${"))
                    .map(|clip| {
                        let stem = clip.rsplit_once('.').map_or(clip, |(s, _)| s);
                        let picture = format!("{stem}-still.jpg");
                        self.out.stills.insert((clip.to_owned(), picture.clone()));
                        s(picture)
                    }),
            };
            match still {
                Some(src) => {
                    for k in ["delay", "loop", "mute", "fallback", AUDIO] {
                        t.remove(k);
                    }
                    t.insert("kind".into(), s("image"));
                    t.insert("src".into(), src);
                    self.note(format!(
                        "um segundo vídeo na tela de {screen} (`{id}`) vira a imagem parada dele: o launcher toca um vídeo por tela"
                    ));
                }
                None => {
                    out.remove(i);
                    self.note(format!(
                        "vídeo com som por baixo de outro na tela de {screen} (`{id}`, música de fundo) fica de fora: o launcher é mudo"
                    ));
                }
            }
        }
        for e in out.iter_mut() {
            e.t.remove(AUDIO);
        }
    }

    /// GIF pictures (ES-DE draws them, and plays an `animation`): the
    /// launcher reads PNG, JPG, WebP and SVG, so the first frame as a PNG.
    fn gif_stills(&mut self, out: &mut [OutEl]) {
        let swap = |v: &mut Value, stills: &mut BTreeSet<(String, String)>| {
            if let Value::String(p) = v {
                if p.to_lowercase().ends_with(".gif") && !p.contains("${") {
                    let picture = format!("{}-still.png", &p[..p.len() - 4]);
                    stills.insert((p.clone(), picture.clone()));
                    *p = picture;
                }
            }
        };
        for e in out.iter_mut() {
            for key in ["src", "image"] {
                match e.t.get_mut(key) {
                    Some(Value::Array(items)) => {
                        items.iter_mut().for_each(|v| swap(v, &mut self.out.stills))
                    }
                    Some(v) => swap(v, &mut self.out.stills),
                    None => {}
                }
            }
        }
    }

    /// One status pill per screen: ES-DE's clock and system status (battery,
    /// radios) are the launcher's one `status`. It goes where the theme's
    /// clock is, when it shows one: there it has room (a system status is
    /// a few icons, the pill is longer), and the clock's own art (an LCD
    /// "88:88" under the digits) goes, since the pill draws the time its
    /// own way.
    fn one_status(&mut self, out: &mut Vec<OutEl>) {
        let status: Vec<usize> = (0..out.len())
            .filter(|&i| out[i].t.get("kind").and_then(Value::as_str) == Some("status"))
            .collect();
        let off = |t: &Table| t.get("opacity").and_then(Value::as_float) == Some(0.);
        let clock = status
            .iter()
            .copied()
            .find(|&i| out[i].t.contains_key(CLOCK) && !off(&out[i].t));
        let keep = clock
            .or_else(|| {
                status
                    .iter()
                    .copied()
                    .find(|&i| out[i].id.contains("status"))
            })
            .or_else(|| status.first().copied());
        let Some(keep) = keep else { return };
        for e in out.iter_mut() {
            e.t.remove(CLOCK);
        }
        let keep_id = out[keep].id.clone();
        out.retain(|e| {
            let kind = e.t.get("kind").and_then(Value::as_str);
            let art = clock.is_some()
                && matches!(kind, Some("image" | "frame"))
                && e.id.to_lowercase().contains("clock");
            !(kind == Some("status") && e.id != keep_id) && !art
        });
        if clock.is_some() {
            self.note("o relógio do tema vira a pílula de relógio e bateria do launcher; o desenho de fundo dele (`clock...`) fica de fora");
        }
    }

    fn rating(&mut self, el: &El) -> Option<Table> {
        if self.hidden(el) {
            return None;
        }
        let mut t = Table::new();
        t.insert("kind".into(), s("rating"));
        let (w, h) = el.get("size").and_then(pair).unwrap_or((0., 0.06));
        let h = if h > 0. { h } else { w / 5. * self.shape.ratio };
        let (x, y) = el.get("pos").and_then(pair).unwrap_or((0.01, 0.6));
        let (ox, oy) = el.get("origin").and_then(pair).unwrap_or((0., 0.));
        // Five square stars side by side, as ES draws them.
        let star_w = h / self.shape.ratio;
        let width = (star_w * 5.).max(w.min(1.));
        t.insert("spacing".into(), f(0.));
        let g = Geo {
            x,
            y,
            w: width,
            h: h.max(0.005),
            ox,
            oy,
            fit: "contain",
        };
        self.place(&mut t, &g);
        let filled = el
            .get("filledPath")
            .filter(|p| !p.is_empty())
            .map(str::to_owned);
        let unfilled = el
            .get("unfilledPath")
            .filter(|p| !p.is_empty())
            .map(str::to_owned);
        if let Some(p) = &filled {
            if self.has(p) {
                self.file(p, p);
                t.insert("star".into(), s(p.clone()));
            }
        }
        if let (Some(u), Some(fl)) = (&unfilled, &filled) {
            if u != fl && self.has(u) {
                self.file(u, u);
                t.insert("star_empty".into(), s(u.clone()));
            }
        }
        if let Some((c, _)) = el.get("color").and_then(color) {
            t.insert("color".into(), s(format!("{}FF", &c[..7])));
        }
        if let Some((c, a)) = el.get("unfilledColor").and_then(color) {
            t.insert("empty".into(), s(format!("{}FF", &c[..7])));
            if a < 255 {
                t.insert("empty_opacity".into(), f(f32::from(a) / 255.));
            }
        }
        t.insert("size".into(), f(h.clamp(0.005, 0.3)));
        self.common(&mut t, el, 40.);
        self.mapped(&format!("rating {}", el.name), "rating");
        Some(t)
    }

    /// An `extra="true"` element (or any element in ES-DE).
    fn extra(&mut self, el: &El, z: f32) -> Vec<Table> {
        let mut out = Vec::new();
        match el.kind.as_str() {
            "image" => {
                if let Some(kind) = el.get("imageType") {
                    // ES-DE: an image of the game's.
                    let src = match kind.split(',').next().unwrap_or("").trim() {
                        "cover" | "backcover" | "3dbox" | "physicalmedia" => vec!["game.cover"],
                        "marquee" => vec!["game.logo"],
                        "fanart" | "titlescreen" | "screenshot" | "miximage" => vec!["game.screenshot"],
                        _ => vec!["game.screenshot", "game.cover"],
                    };
                    let src: Vec<String> = src.into_iter().map(str::to_owned).collect();
                    out.extend(self.image(el, src, (0.5, 0.5), (0.4, 0.4), (0., 0.), z, None));
                    return out;
                }
                let src = self.picture_sources(el);
                if src.is_empty() {
                    if el.paths.is_empty() && !self.hidden(el) {
                        let name = el.name.clone();
                        self.note(format!("imagem `{name}` sem arquivo (ou só com fonte aleatória/externa); ignorada"));
                    }
                    return out;
                }
                let first = src.first().map(|p| p.replace("${system}", "${system.theme}"));
                out.extend(self.image(el, src, (0., 0.), (0., 0.), (0., 0.), z, first.as_deref()));
                self.mapped("image extra", "image");
            }
            "text" | "datetime" => {
                let text = if let Some(m) = el.get("metadata") {
                    match m.trim() {
                        "name" => "${game.name}".to_owned(),
                        "description" => "${game.description}".into(),
                        "developer" => "${game.developer}".into(),
                        "publisher" => "${game.publisher}".into(),
                        "genre" => "${game.genre}".into(),
                        // ES-DE writes the list's own "1-2", no words.
                        "players" if self.esde => "${game.players.count}".into(),
                        "players" => "${game.players}".into(),
                        "releasedate" => "${game.year}".into(),
                        "lastplayed" => "${game.last_played}".into(),
                        _ => return out,
                    }
                } else if let Some(sd) = el.get("systemdata") {
                    // ES's own names: `data` makes them the launcher's.
                    match sd.trim() {
                        "name" => "${system.name}".to_owned(),
                        "fullname" => "${system.fullName}".into(),
                        "gamecount" | "gamecountGames" => "${system.games}".into(),
                        "gamecountGamesNoText" => "${system.count}".into(),
                        "gamecountFavorites" | "gamecountFavoritesNoText" => {
                            "${system.favorites}".into()
                        }
                        _ => "${system.games}".into(),
                    }
                } else {
                    match el.get("text") {
                        Some(t) if !t.trim().is_empty() => {
                            if missing_label(&el.name, t) {
                                let name = el.name.clone();
                                self.note(format!("rótulo `{name}` de um dado que o launcher não tem (tempo de jogo, partidas, selos...); fica de fora"));
                                return out;
                            }
                            t.to_owned()
                        }
                        _ => return out,
                    }
                };
                let wrap = (text == "${game.description}").then_some(true);
                if let Some(t) = self.text(el, &text, (0., 0.), (0., 0.), z, wrap) {
                    out.extend(self.backdrop(el, &t));
                    out.push(t);
                }
                self.mapped("text extra", "text");
            }
            "video" => {
                let src: &[&str] = if el.get("imageType").is_some() || self.esde && el.paths.is_empty() {
                    &["game.video"]
                } else {
                    &[]
                };
                if let Some(t) = self.video(el, src, z) {
                    out.push(t);
                } else if !self.hidden(el) {
                    self.note("vídeo sem arquivo próprio (ou aleatório, `{random}`): não existe no launcher; ignorado");
                }
            }
            "rating" => out.extend(self.rating(el)),
            // A GIF plays in ES-DE; here it stands still (`gif_stills`).
            "animation" => {
                let src: Vec<String> = self
                    .picture_sources(el)
                    .into_iter()
                    .filter(|p| p.to_lowercase().ends_with(".gif"))
                    .collect();
                if src.is_empty() {
                    if !self.hidden(el) && !el.paths.is_empty() {
                        self.note("animação Lottie (`animation` .json) não existe no launcher; ignorada");
                    }
                    return out;
                }
                let first = src.first().cloned();
                out.extend(self.image(el, src, (0., 0.), (0., 0.), (0., 0.), z, first.as_deref()));
                self.mapped("animation (GIF)", "image parada (o primeiro quadro)");
            }
            // ES-DE's clock and battery/radio icons: the launcher's pill.
            "clock" | "systemstatus" => {
                if self.hidden(el) {
                    return out;
                }
                let g = self.geo(el, (0.98, 0.02), (0.2, 0.04), (1., 0.), None);
                // A clock is as tall as its digits; the pill, a quarter of
                // the screen wide at most.
                let tall = el.get("fontSize").and_then(num).map(|f| f * 1.3);
                let (w, h) = (
                    g.w.clamp(0.12, 0.25),
                    tall.unwrap_or(g.h).clamp(0.03, 0.08),
                );
                let (x, y) = (g.x - g.ox * w, g.y - g.oy * h);
                let mut t = Table::new();
                t.insert("kind".into(), s("status"));
                t.insert("rect".into(), rect(x, y, w, h));
                let ink = el.get("color").or_else(|| el.get("textColor"));
                if let Some((c, _)) = ink.and_then(color) {
                    t.insert("color".into(), s(c));
                }
                t.insert("z".into(), Value::Integer(STATUS_Z));
                if el.kind == "clock" {
                    t.insert(CLOCK.into(), Value::Boolean(true));
                }
                self.mapped(
                    "clock / systemstatus",
                    "status (relógio, bateria e rádios do launcher)",
                );
                out.push(t);
            }
            // "12 / 345": the focused game's place in the list.
            "gamelistinfo" => {
                if let Some(t) = self.text(
                    el,
                    "${list.index} / ${list.total}",
                    (0., 0.),
                    (0., 0.),
                    z,
                    Some(false),
                ) {
                    out.push(t);
                    self.mapped("gamelistinfo", "text `${list.index} / ${list.total}`");
                }
            }
            "ninepatch" => {
                let src = self.sources(el);
                if let Some(p) = src.first().cloned() {
                    self.file(&p, &p);
                    let mut t = Table::new();
                    t.insert("kind".into(), s("frame"));
                    t.insert("image".into(), s(data(&p, false)));
                    let corner = el.get("cornerSize").and_then(pair).map_or(16., |c| c.0.max(1.));
                    t.insert("slice".into(), Value::Integer(corner.round() as i64));
                    let g = self.geo(el, (0., 0.), (1., 1.), (0., 0.), None);
                    self.place(&mut t, &g);
                    self.common(&mut t, el, z);
                    out.push(t);
                    self.mapped("ninepatch", "frame");
                }
            }
            "sound" => self.note("sons (`sound`, música de fundo por sistema) não são convertidos; use `sounds/` do tema do launcher"),
            "helpsystem" => out.extend(self.hints(el)),
            "carousel" | "textlist" | "imagegrid" | "grid" => {}
            kind => {
                let kind = kind.to_owned();
                self.note(format!("elemento `{kind}` não tem equivalente; ignorado"));
            }
        }
        out
    }
}

/// The usual shape of a game's picture or clip (ES-DE `imageType`), for
/// a size given on one side only (`<size>0 1</size>`: as tall as the
/// screen, as wide as the picture).
fn media_ratio(el: &El) -> Option<f32> {
    let kind = el
        .get("imageType")
        .map(|k| k.split(',').next().unwrap_or("").trim());
    Some(match (el.kind.as_str(), kind) {
        (_, Some("fanart")) => 16. / 9.,
        (_, Some("screenshot" | "titlescreen" | "miximage")) => 4. / 3.,
        (_, Some("cover" | "backcover")) => 0.7,
        (_, Some("3dbox")) => 0.8,
        (_, Some("physicalmedia")) => 1.,
        (_, Some("marquee")) => 2.5,
        ("video", _) => 4. / 3.,
        _ => return None,
    })
}

/// The z an element has when the theme gives none: ES-DE's defaults per
/// type (text 40, image 30...); 10 for the other dialects' extras.
fn esde_z(kind: &str, esde: bool) -> f32 {
    if !esde {
        return 10.;
    }
    match kind {
        "image" | "video" => 30.,
        "animation" | "badges" => 35.,
        "text" | "datetime" => 40.,
        "rating" | "gamelistinfo" => 45.,
        "carousel" | "grid" | "textlist" | "helpsystem" => 50.,
        "clock" => 80.,
        "systemstatus" => 85.,
        _ => 10.,
    }
}

/// A view name as a title: "detailed" → "Detailed".
fn title(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// `name` made an id no other element of `out` has.
fn unique(out: &[OutEl], name: &str) -> String {
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while out.iter().any(|e| e.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_names_read_as_the_launcher_writes_them() {
        assert_eq!(color("f9c440"), Some(("#F9C440FF".into(), 255)));
        assert_eq!(color("8b000050"), Some(("#8B000050".into(), 0x50)));
        assert_eq!(color(""), None);
        assert_eq!(slug("Boxes + details"), "boxes-details");
        assert_eq!(data("a/${system.theme}.png", false), "a/${system}.png");
        assert_eq!(data("${system.fullName}", true), "${item.name}");
    }
}
