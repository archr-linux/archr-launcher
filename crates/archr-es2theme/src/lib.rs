//! Converts an EmulationStation theme (Batocera/ArkOS XML, with the ES-DE
//! wrappers when present) into an archr-launcher theme folder: a
//! `theme.toml` of free screens (docs/temas.md, Telas livres), the
//! pictures and fonts it uses, the original's license files and a report,
//! `CONVERSAO.md`. Runs on a PC; the launcher never reads XML.
//!
//! The theme is read once per screen shape (the Ratio subset, when it has
//! one) and once per choice of each other subset. What changes between the
//! reads becomes variables: `[aspect.<shape>.vars]` for the shapes,
//! `[options.<id>]` choices for the subsets.
pub mod build;
pub mod es;
pub mod map;
pub mod store;
pub mod xml;

use build::{Built, SHAPES};
use map::{Model, OutEl, Shape};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::{Path, PathBuf},
};
use toml::{Table, Value};

#[derive(Clone, Debug)]
pub struct Options {
    /// The language of the theme's own texts: `en`, `pt`, `es`...
    pub lang: String,
    /// The converted theme's name; the source folder's when None.
    pub name: Option<String>,
    /// The store's metadata, over what the converter writes: authors,
    /// the description in English and Portuguese, the short license.
    pub author: Option<String>,
    pub description: Option<(String, String)>,
    pub license: Option<String>,
    /// Made for the theme store (`--store`): only the files the theme
    /// names, pictures no larger than a 1024x768 screen draws them, clips
    /// re-encoded small or, past the budget, left to their stills.
    pub store: bool,
    /// The systems the console runs (`--systems <es_systems.cfg>`): with
    /// `store`, pictures of other systems are left out.
    pub systems: Option<BTreeSet<String>>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            lang: "en".into(),
            name: None,
            author: None,
            description: None,
            license: None,
            store: false,
            systems: None,
        }
    }
}

/// What a conversion made.
#[derive(Debug, Default)]
pub struct Conversion {
    pub views: Vec<String>,
    pub options: Vec<String>,
    pub files: usize,
    pub bytes: u64,
    /// The launcher's warnings reading the result: none, when all went well.
    pub warnings: Vec<String>,
    pub notes: BTreeSet<String>,
}

/// The sizes the result is checked at.
pub const SIZES: [(u32, u32); 4] = [(640, 480), (720, 720), (1024, 768), (854, 480)];

pub fn convert(src: &Path, out: &Path, opts: &Options) -> Result<Conversion, String> {
    let probe = es::load(src, &opts.lang, &BTreeMap::new())?;
    let subsets = probe.subsets.clone();
    let logos = map::logo_dir(&probe);
    let ratio = subsets
        .iter()
        .find(|s| s.is_ratio() || s.name == "aspectRatio")
        .cloned();
    let variants: Vec<Option<String>> = match subsets.iter().find(|s| s.name == "variant") {
        Some(v) => v.choices.iter().map(|c| Some(c.name.clone())).collect(),
        None => vec![None],
    };
    let optional: Vec<es::Subset> = subsets
        .iter()
        .filter(|s| {
            Some(&s.name) != ratio.as_ref().map(|r| &r.name)
                && s.name != "variant"
                && s.choices.len() >= 2
        })
        .cloned()
        .collect();
    let mut choices: Vec<Option<(String, String)>> = vec![None];
    for s in &optional {
        for c in s.choices.iter().skip(1) {
            choices.push(Some((s.name.clone(), c.name.clone())));
        }
    }
    let mut at = Vec::new();
    for choice in &choices {
        for (_, shape_ratio) in SHAPES {
            let mut model = Model::default();
            let mut vars = BTreeMap::new();
            for variant in &variants {
                let mut picked = BTreeMap::new();
                if let Some(r) = &ratio {
                    if let Some(c) = r.nearest(shape_ratio) {
                        picked.insert(r.name.clone(), c.to_owned());
                    }
                }
                if let Some(v) = variant {
                    picked.insert("variant".into(), v.clone());
                }
                if let Some((s, c)) = choice {
                    picked.insert(s.clone(), c.clone());
                }
                let es = es::load(src, &opts.lang, &picked)?;
                if vars.is_empty() {
                    vars.clone_from(&es.vars);
                }
                let mut m = map::model(&es, src, Shape { ratio: shape_ratio }, logos.as_deref());
                if let Some(v) = variant {
                    for view in &mut m.views {
                        view.id = format!("{}-{}", map::id_of(v), view.id);
                        // The variant's label in the theme's capabilities.
                        if let Some(label) = variant_label(&subsets, v) {
                            view.name = label;
                        }
                    }
                }
                model.views.extend(m.views);
                if model.top.is_empty() {
                    model.top = m.top;
                }
                model.notes.extend(m.notes);
                model.notes.extend(es.notes);
                model.mapped.extend(m.mapped);
                model.files.extend(m.files);
                model.stills.extend(m.stills);
            }
            at.push(build::Read { model, vars });
        }
    }
    let reads = build::Reads { choices, at };
    // Names in English and Portuguese, when the theme has both.
    let names = |lang: &str| es::load(src, lang, &BTreeMap::new()).ok();
    let (en, pt) = (names("en"), names("pt"));

    let mut notes: BTreeSet<String> = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut mapped = BTreeSet::new();
    let mut stills = BTreeSet::new();
    for r in &reads.at {
        stills.extend(r.model.stills.iter().cloned());
        notes.extend(r.model.notes.iter().cloned());
        files.extend(r.model.files.iter().cloned());
        mapped.extend(r.model.mapped.iter().cloned());
    }
    let mut built = build::build(&reads, &optional, en.as_ref(), pt.as_ref(), &mut notes);
    // ES-DE files read per system: the classic read with the defaults
    // again for each system, what differs carried as system variables.
    if !probe.per_system.is_empty() {
        let classic = SHAPES[0].1;
        let mut models = Vec::new();
        let mut system_es_vars = Vec::new();
        for system in systems_of(src, &probe.per_system) {
            let mut model = Model::default();
            let mut es_vars = None;
            for variant in &variants {
                let mut picked = BTreeMap::new();
                if let Some(r) = &ratio {
                    if let Some(c) = r.nearest(classic) {
                        picked.insert(r.name.clone(), c.to_owned());
                    }
                }
                if let Some(v) = variant {
                    picked.insert("variant".into(), v.clone());
                }
                let es = es::load_for(src, &opts.lang, &picked, Some(&system))?;
                if es_vars.is_none() {
                    es_vars = Some(es.vars.clone());
                }
                let mut m = map::model(&es, src, Shape { ratio: classic }, logos.as_deref());
                if let Some(v) = variant {
                    for view in &mut m.views {
                        view.id = format!("{}-{}", map::id_of(v), view.id);
                        // The variant's label in the theme's capabilities.
                        if let Some(label) = variant_label(&subsets, v) {
                            view.name = label;
                        }
                    }
                }
                model.views.extend(m.views);
                files.extend(m.files);
            }
            system_es_vars.push((system.clone(), es_vars.unwrap_or_default()));
            models.push((system, model));
        }
        build::per_system(&mut built, reads.base(), &models, &mut notes);
        build::per_system_text(&mut built, &reads.at[0].vars, &system_es_vars);
    }

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (count, bytes) = copy_files(src, out, &files, &mut notes)?;
    write_stills(src, out, &stills, &mut notes);
    let license = license_of(src);
    let name = opts.name.clone().unwrap_or_else(|| {
        src.file_name()
            .map_or("ES theme".into(), |n| n.to_string_lossy().into_owned())
    });
    let license = opts.license.clone().unwrap_or(license);
    let written = write_toml(&built, &name, src, &license, opts);
    for (file, text) in &written {
        let path = out.join(&file);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        if text.len() as u64 > 64 * 1024 {
            notes.insert(format!(
                "{file} passa de 64 KiB: o launcher não o lê; divida a visualização à mão"
            ));
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let (count, bytes) = if opts.store {
        store::prepare(out, &written, opts.systems.as_ref(), &mut notes)?
    } else {
        (count, bytes)
    };
    let warnings = check(out);
    let conversion = Conversion {
        views: built.views.iter().map(|v| v.id.clone()).collect(),
        options: built.options.iter().map(|o| o.id.clone()).collect(),
        files: count,
        bytes,
        warnings,
        notes,
    };
    let report = report(
        src,
        &name,
        &license,
        &conversion,
        &built,
        &mapped,
        &optional,
    );
    std::fs::write(out.join("CONVERSAO.md"), report).map_err(|e| e.to_string())?;
    Ok(conversion)
}

/// A variant's label (capabilities.xml), when it has one of its own.
fn variant_label(subsets: &[es::Subset], name: &str) -> Option<String> {
    subsets
        .iter()
        .find(|s| s.name == "variant")?
        .choices
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.display.clone())
        .filter(|d| !d.is_empty() && d != name)
}

/// The systems an ES-DE theme has files for: the names in the folders of
/// its per-system includes (`_default` and the like left out).
fn systems_of(src: &Path, patterns: &[String]) -> Vec<String> {
    let mut found = BTreeSet::new();
    for pattern in patterns {
        let (dir, name) = pattern.rsplit_once('/').unwrap_or(("", pattern));
        let Some((prefix, suffix)) = name.split_once("${system.theme}") else {
            continue;
        };
        let Some(Ok(entries)) = es::contained(src, dir).map(std::fs::read_dir) else {
            continue;
        };
        for e in entries.flatten() {
            let file = e.file_name().to_string_lossy().into_owned();
            let Some(system) = file
                .strip_prefix(prefix)
                .and_then(|f| f.strip_suffix(suffix))
            else {
                continue;
            };
            if !system.is_empty()
                && !system.starts_with('_')
                && system
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_+.".contains(c))
            {
                found.insert(system.to_owned());
            }
        }
    }
    found.into_iter().collect()
}

/// The oldest launcher that draws what the theme uses: 0.5.0 for free
/// screens; 0.6.0 when it has system variables, shows an element only on
/// collections or only on systems (`when.collection`) or hides one with
/// opacity 0 (a choice, shape or system that leaves it out).
fn min_launcher(b: &Built) -> &'static str {
    let collection = b.views.iter().any(|v| {
        v.systems
            .iter()
            .chain(&v.games)
            .any(|e| e.t.get("when").and_then(|w| w.get("collection")).is_some())
    });
    // An element at opacity 0 in some choice, shape or system: before
    // 0.6.0 a text drew whole there.
    let zero = |v: &Value| v.as_float() == Some(0.) || v.as_integer() == Some(0);
    let values_of = |name: &str| -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        let lists = b.aspect_vars.values().chain(b.system_vars.values()).chain(
            b.options
                .iter()
                .flat_map(|o| o.choices.iter().map(|c| &c.2)),
        );
        for list in std::iter::once(&b.vars).chain(lists) {
            out.extend(
                list.iter()
                    .filter(|(n, _)| n == name)
                    .map(|(_, v)| v.clone()),
            );
        }
        out
    };
    let hidden = b.views.iter().any(|v| {
        v.systems
            .iter()
            .chain(&v.games)
            .any(|e| match e.t.get("opacity") {
                Some(Value::String(var)) => var
                    .strip_prefix('$')
                    .is_some_and(|n| values_of(n).iter().any(zero)),
                Some(v) => zero(v),
                None => false,
            })
    });
    let count = b.views.iter().any(|v| {
        v.systems.iter().chain(&v.games).any(|e| {
            e.t.values()
                .any(|x| x.to_string().contains("${game.players.count}"))
        })
    });
    // A tinted picture with rounded corners kept its own colors before.
    let rounded_tint = b.views.iter().any(|v| {
        v.systems.iter().chain(&v.games).any(|e| {
            e.t.get("kind").and_then(Value::as_str) == Some("image")
                && e.t.contains_key("tint")
                && e.t.get("radius").is_some_and(|r| r.as_float() != Some(0.))
        })
    });
    if rounded_tint {
        return "0.6.1";
    }
    if !b.system_vars.is_empty() || collection || hidden || count {
        "0.6.0"
    } else {
        "0.5.0"
    }
}

// ---------------------------------------------------------------- TOML

fn key(k: &str) -> String {
    if !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        k.to_owned()
    } else {
        Value::String(k.to_owned()).to_string()
    }
}
fn inline(v: &Value) -> String {
    match v {
        Value::Table(t) => {
            let items: Vec<String> = t
                .iter()
                .map(|(k, v)| format!("{} = {}", key(k), inline(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(inline).collect::<Vec<_>>().join(", ")),
        Value::Float(f) if f.fract() == 0. && f.abs() < 1e15 => format!("{f:.1}"),
        other => other.to_string(),
    }
}
fn names(en: &str, pt: &str) -> String {
    if en == pt {
        format!("{{ en = {} }}", inline(&Value::String(en.into())))
    } else {
        format!(
            "{{ en = {}, pt_BR = {} }}",
            inline(&Value::String(en.into())),
            inline(&Value::String(pt.into()))
        )
    }
}
const ORDER: &[&str] = &[
    "kind", "base", "text", "src", "fallback", "of", "axis", "columns", "rows", "visible", "rect",
    "anchor",
];
fn element(out: &mut String, at: &str, e: &OutEl) {
    let _ = writeln!(out, "\n[[{at}.el]]");
    let _ = writeln!(out, "id = {}", inline(&Value::String(e.id.clone())));
    for k in ORDER {
        if let Some(v) = e.t.get(*k) {
            let _ = writeln!(out, "{} = {}", key(k), inline(v));
        }
    }
    for (k, v) in &e.t {
        if !ORDER.contains(&k.as_str()) {
            let _ = writeln!(out, "{} = {}", key(k), inline(v));
        }
    }
}

/// The theme's files: `theme.toml` with the top keys and the includes,
/// then `parts/*.toml` (each file of a theme is read up to 64 KiB).
fn write_toml(
    b: &Built,
    name: &str,
    src: &Path,
    license: &str,
    opts: &Options,
) -> Vec<(String, String)> {
    let folder = src
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut vars = String::new();
    if !b.vars.is_empty() {
        let _ = writeln!(
            vars,
            "# Variáveis: o que muda com o formato da tela e com as opções.\n[vars]"
        );
        for (k, v) in &b.vars {
            let _ = writeln!(vars, "{} = {}", key(k), inline(v));
        }
    }
    for (shape, list) in &b.aspect_vars {
        let _ = writeln!(vars, "\n[aspect.{shape}.vars]");
        for (k, v) in list {
            let _ = writeln!(vars, "{} = {}", key(k), inline(v));
        }
    }
    if !vars.is_empty() {
        parts.push(("parts/vars.toml".into(), vars));
    }
    // System variables, in files under the launcher's 64 KiB each.
    let mut chunk = String::new();
    let mut n = 1;
    for (system, list) in &b.system_vars {
        let mut table = format!("\n[system.{}.vars]\n", key(system));
        for (k, v) in list {
            let _ = writeln!(table, "{} = {}", key(k), inline(v));
        }
        if !chunk.is_empty() && chunk.len() + table.len() > 60 * 1024 {
            parts.push((
                format!("parts/systems-{n}.toml"),
                std::mem::take(&mut chunk),
            ));
            n += 1;
        }
        if chunk.is_empty() {
            chunk.push_str("# O que cada sistema mostra do seu jeito (os arquivos por sistema do tema original).\n");
        }
        chunk.push_str(&table);
    }
    if !chunk.is_empty() {
        parts.push((format!("parts/systems-{n}.toml"), chunk));
    }
    let mut options = String::new();
    for o in &b.options {
        let _ = writeln!(options, "[options.{}]", key(&o.id));
        let _ = writeln!(options, "# o subset \"{}\" do tema original", o.subset);
        let _ = writeln!(options, "name = {}", names(&o.name.0, &o.name.1));
        let _ = writeln!(
            options,
            "default = {}",
            inline(&Value::String(o.choices[0].0.clone()))
        );
        for (id, (en, pt), set) in &o.choices {
            let mut line = format!("choices.{} = {{ name = {}", key(id), names(en, pt));
            if !set.is_empty() {
                let items: Vec<String> = set
                    .iter()
                    .map(|(k, v)| format!("{} = {}", key(k), inline(v)))
                    .collect();
                let _ = write!(line, ", vars = {{ {} }}", items.join(", "));
            }
            line.push_str(" }");
            let _ = writeln!(options, "{line}\n");
        }
    }
    if !options.is_empty() {
        parts.push(("parts/options.toml".into(), options));
    }
    for v in &b.views {
        let mut out = String::new();
        let _ = writeln!(out, "[view.{}]", key(&v.id));
        let _ = writeln!(out, "name = {}", names(&v.name.0, &v.name.1));
        for e in &v.systems {
            element(&mut out, &format!("view.{}.systems", key(&v.id)), e);
        }
        for e in &v.games {
            element(&mut out, &format!("view.{}.games", key(&v.id)), e);
        }
        if !v.games.is_empty() {
            details(&mut out, v, &b.vars);
        }
        parts.push((format!("parts/view-{}.toml", v.id), out));
    }
    let mut top = String::new();
    let _ = writeln!(
        top,
        "# Convertido do tema do EmulationStation \"{folder}\" pelo archr-es2theme."
    );
    let _ = writeln!(
        top,
        "# Veja CONVERSAO.md: o que virou o quê e o que ficou de fora."
    );
    let includes: Vec<Value> = parts
        .iter()
        .map(|(p, _)| Value::String(p.clone()))
        .collect();
    if !includes.is_empty() {
        let _ = writeln!(top, "include = {}", inline(&Value::Array(includes)));
    }
    let _ = writeln!(top, "name = {}", inline(&Value::String(name.into())));
    // The view the theme opens with: EmulationStation's gamelist default,
    // "detailed", when the theme has it; else the first one converted.
    let opens = b
        .views
        .iter()
        .find(|v| v.id == "detailed")
        .or_else(|| b.views.first());
    if let Some(v) = opens {
        let _ = writeln!(
            top,
            "default_view = {}",
            inline(&Value::String(v.id.clone()))
        );
    }
    let (en, pt) = opts.description.clone().unwrap_or_else(|| {
        (
            format!("Converted from the EmulationStation theme \"{folder}\"."),
            format!("Convertido do tema \"{folder}\" do EmulationStation."),
        )
    });
    let _ = writeln!(top, "description = {}", names(&en, &pt));
    let author = opts
        .author
        .clone()
        .unwrap_or_else(|| format!("{folder} (original); converted by archr-es2theme"));
    let _ = writeln!(top, "author = {}", inline(&Value::String(author)));
    let _ = writeln!(top, "version = \"1.0.0\"");
    // The launcher it was converted for: the features it uses (frame tint,
    // default_view...) draw broken on an older one, which refuses it then.
    let _ = writeln!(top, "min_launcher = \"{}\"", min_launcher(b));
    let _ = writeln!(top, "license = {}", inline(&Value::String(license.into())));
    for (k, v) in &b.top {
        let _ = writeln!(top, "{} = {}", key(k), inline(v));
    }
    parts.insert(0, ("theme.toml".into(), top));
    parts
}

/// The details screen: the games screen, with the game's cover, facts and
/// buttons where its list was, and the description scrolling. It is drawn
/// in the theme's own style: the card in its tone (light on a theme with
/// dark text), the facts in its text's face and colors; and when the games
/// screen already shows the facts, only the cover and the buttons.
fn details(out: &mut String, v: &build::BuiltView, vars: &[(String, Value)]) {
    let at = format!("view.{}.details", key(&v.id));
    let _ = writeln!(
        out,
        "\n# Detalhes: a tela de jogos com a capa, a ficha e os botões no lugar da lista."
    );
    let _ = writeln!(out, "[{at}]");
    let _ = writeln!(out, "base = \"games\"");
    // The list goes, and its position ("7 / 24"), which would sit by the
    // details' longer button hints.
    let mut gone: Vec<Value> = v
        .collection
        .iter()
        .map(|c| Value::String(c.clone()))
        .collect();
    gone.extend(
        v.games
            .iter()
            .filter(|e| {
                e.t.get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.contains("${list.index}"))
            })
            .map(|e| Value::String(e.id.clone())),
    );
    if !gone.is_empty() {
        let _ = writeln!(out, "remove = {}", inline(&Value::Array(gone)));
    }
    for d in &v.description {
        let _ = writeln!(
            out,
            "\n[[{at}.el]]\nid = {}\nlong_text = \"scroll\"",
            inline(&Value::String(d.clone()))
        );
    }
    let Some(r) = &v.collection_rect else { return };
    // The value a field has with the defaults: its own, or its variable's.
    let value = |v: &Value| -> Value {
        match v.as_str().and_then(|s| s.strip_prefix('$')) {
            Some(name) => vars
                .iter()
                .find(|(n, _)| n == name)
                .map_or(v.clone(), |(_, v)| v.clone()),
            None => v.clone(),
        }
    };
    let texts: Vec<&map::OutEl> = v
        .games
        .iter()
        .filter(|e| {
            e.t.get("kind").and_then(Value::as_str) == Some("text")
                && e.t.get("opacity").map(&value).and_then(|o| o.as_float()) != Some(0.)
        })
        .collect();
    let says = |data: &str| {
        texts.iter().any(|e| {
            e.t.get("text")
                .and_then(Value::as_str)
                .is_some_and(|t| t.contains(data))
        })
    };
    // The theme's reading text: the description's, else the game's name's.
    let body = texts
        .iter()
        .find(|e| e.t.get("text").and_then(Value::as_str) == Some("${game.description}"))
        .or_else(|| {
            texts
                .iter()
                .find(|e| e.t.get("text").and_then(Value::as_str) == Some("${game.name}"))
        });
    let ink = body.and_then(|e| e.t.get("color")).cloned();
    let face = body.and_then(|e| e.t.get("font")).cloned();
    let light = ink
        .as_ref()
        .map(&value)
        .and_then(|c| c.as_str().and_then(map::color).map(|(c, _)| c))
        .and_then(|c| {
            let n = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).ok().map(f32::from);
            Some(0.299 * n(1)? + 0.587 * n(3)? + 0.114 * n(5)?)
        })
        .is_some_and(|l| l < 110.);
    let facts = [
        "${game.genre}",
        "${game.players}",
        "${game.developer}",
        "${game.publisher}",
        "${game.year}",
    ]
    .iter()
    .filter(|d| says(d))
    .count();
    let shown = facts >= 3;
    let get = |k: &str, d: f64| r.get(k).and_then(Value::as_float).unwrap_or(d);
    let (x, y, w, h) = (get("x", 0.), get("y", 0.15), get("w", 0.45), get("h", 0.75));
    let pad = (w * 0.06).min(0.03);
    let (x, w) = (x + pad, (w - 2. * pad).max(0.05));
    let box_of = |y0: f64, h0: f64| {
        let mut t = Table::new();
        t.insert("x".into(), Value::Float(map::r5(x as f32)));
        t.insert("y".into(), Value::Float(map::r5(y0 as f32)));
        t.insert("w".into(), Value::Float(map::r5(w as f32)));
        t.insert("h".into(), Value::Float(map::r5(h0 as f32)));
        inline(&Value::Table(t))
    };
    let ink_of = |k: &str| v.list_ink.get(k).map(inline);
    // A card behind them in the theme's tone, for any art under the list.
    let mut card = Table::new();
    card.insert("x".into(), Value::Float(map::r5((x - pad / 2.) as f32)));
    card.insert("y".into(), Value::Float(map::r5((y - pad / 2.) as f32)));
    card.insert("w".into(), Value::Float(map::r5((w + pad) as f32)));
    card.insert("h".into(), Value::Float(map::r5((h + pad) as f32)));
    let fill = if light { "#FFFFFFE6" } else { "#00000099" };
    let _ = writeln!(
        out,
        "\n[[{at}.el]]\nid = \"fundo-ficha\"\nkind = \"rect\"\nrect = {}\nfill = \"{fill}\"\nradius = 0.012\nz = 899",
        inline(&Value::Table(card))
    );
    let size = v
        .list_ink
        .get("size")
        .and_then(Value::as_float)
        .unwrap_or(0.035)
        .clamp(0.025, 0.045);
    // The facts already on the games screen: the cover takes their room.
    let cover_h = if shown { h * 0.84 } else { h * 0.5 };
    let _ = writeln!(
        out,
        "\n[[{at}.el]]\nid = \"capa\"\nkind = \"image\"\nsrc = [\"game.cover\", \"game.screenshot\"]\nrect = {}\nradius = 0.01\nz = 900",
        box_of(y, cover_h)
    );
    if !shown {
        let _ = writeln!(
            out,
            "\n[[{at}.el]]\nid = \"ficha\"\nkind = \"fields\"\nrows = [\n  {{ label = \"@genre\", value = \"${{game.genre}}\" }},\n  {{ label = \"@players\", value = \"${{game.players}}\" }},\n  {{ label = \"@developer\", value = \"${{game.developer}}\" }},\n  {{ label = \"@last_played\", value = \"${{game.last_played}}\" }},\n]\nrect = {}\nsize = {}\nupper_labels = true\nz = 900",
            box_of(y + h * 0.53, h * 0.32),
            inline(&Value::Float(map::r5((size * 0.85) as f32)))
        );
        // The theme's text, else the list's.
        let font = face.as_ref().map(inline).or_else(|| ink_of("font"));
        let color = ink.as_ref().map(inline).or_else(|| ink_of("color"));
        let label = ink_of("accent").or_else(|| light.then(|| "\"#555555FF\"".to_owned()));
        for (field, v) in [("font", font), ("color", color), ("label_color", label)] {
            if let Some(v) = v {
                let _ = writeln!(out, "{field} = {v}");
            }
        }
    }
    let _ = writeln!(
        out,
        "\n[[{at}.el]]\nid = \"botoes\"\nkind = \"actions\"\nrect = {}\nz = 900",
        box_of(y + h * 0.88, (h * 0.1).min(0.075))
    );
}

// ---------------------------------------------------------------- files

fn copy_files(
    src: &Path,
    out: &Path,
    files: &BTreeSet<(String, String)>,
    notes: &mut BTreeSet<String>,
) -> Result<(usize, u64), String> {
    // By where each copy goes: one file may go to two places (a logo folder
    // read as `logos/` and by its own path too).
    let mut pairs: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    for (from, to) in files {
        if from.contains("${") {
            let (_, name) = from.rsplit_once('/').unwrap_or(("", from));
            let start = name.find("${").unwrap_or(0);
            let end = name.rfind('}').map_or(name.len(), |e| e + 1);
            let (prefix, suffix) = (&name[..start], &name[end..]);
            let to_at = to.find("${");
            let to_end = to.rfind('}');
            for file in map::matches(src, from) {
                let fname = file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let middle = &fname[prefix.len()..fname.len() - suffix.len()];
                let dest = match (to_at, to_end) {
                    (Some(a), Some(e)) => format!("{}{}{}", &to[..a], middle, &to[e + 1..]),
                    _ => to.clone(),
                };
                if es::contained(
                    src,
                    &file.strip_prefix(src).unwrap_or(&file).to_string_lossy(),
                )
                .is_some()
                    && safe_out(&dest)
                {
                    pairs.insert(out.join(dest), file);
                } else {
                    notes.insert(format!("caminho fora do tema recusado: `{from}`"));
                }
            }
        } else if let Some(file) = es::contained(src, from).filter(|f| f.is_file()) {
            if safe_out(to) {
                pairs.insert(out.join(to), file);
            } else {
                notes.insert(format!("caminho fora do tema recusado: `{to}`"));
            }
        } else if !src.join(from).is_file() {
            // Not there: the report says so where the element is made.
        } else {
            notes.insert(format!("caminho fora do tema recusado: `{from}`"));
        }
    }
    // The original's license and credits go with it.
    if let Ok(entries) = std::fs::read_dir(src) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let keep = [
                "license", "licence", "copying", "readme", "credits", "authors", "notice",
            ]
            .iter()
            .any(|k| name.starts_with(k));
            let inside = es::contained(src, &e.file_name().to_string_lossy()).is_some();
            if keep && inside && e.path().is_file() {
                let dest = out.join("original").join(e.file_name());
                pairs.insert(dest, e.path());
            }
        }
    }
    let (mut count, mut bytes) = (0, 0);
    for (to, from) in &pairs {
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        match std::fs::copy(from, to) {
            Ok(n) => {
                count += 1;
                bytes += n;
            }
            Err(e) => {
                notes.insert(format!("não copiou {}: {e}", from.display()));
            }
        }
    }
    Ok((count, bytes))
}

/// A path the output is written at: relative, without `..`.
fn safe_out(rel: &str) -> bool {
    let rel = Path::new(rel);
    !rel.as_os_str().is_empty()
        && rel.components().all(|c| {
            matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

/// The first frame of each clip a screen shows as a picture (the launcher
/// plays one clip per screen), written with ffmpeg when the PC has it.
fn write_stills(
    src: &Path,
    out: &Path,
    stills: &BTreeSet<(String, String)>,
    notes: &mut BTreeSet<String>,
) {
    for (clip, picture) in stills {
        // The picture's folder made first, inside the output, so the
        // check below can see where it is.
        if let Some(dir) = Path::new(picture).parent() {
            if safe_out(&dir.to_string_lossy()) {
                let _ = std::fs::create_dir_all(out.join(dir));
            }
        }
        let (Some(from), Some(to)) = (es::contained(src, clip), es::contained(out, picture)) else {
            notes.insert(format!(
                "caminho fora do tema recusado: `{clip}`; o elemento fica sem imagem"
            ));
            continue;
        };
        let made = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-i"])
            .arg(from)
            .args(["-frames:v", "1", "-q:v", "3"])
            .arg(&to)
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            notes.insert(format!(
                "não deu para tirar a imagem parada de `{clip}` (precisa do ffmpeg no PC); o elemento fica sem imagem"
            ));
        }
    }
}

/// The original's license, as the theme.toml says it.
fn license_of(src: &Path) -> String {
    let mut text = String::new();
    if let Ok(entries) = std::fs::read_dir(src) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if ["license", "licence", "copying", "readme"]
                .iter()
                .any(|k| name.starts_with(k))
            {
                if let Some(file) = es::contained(src, &e.file_name().to_string_lossy()) {
                    text.push_str(&std::fs::read_to_string(file).unwrap_or_default());
                }
            }
        }
    }
    let lower = text.to_lowercase();
    if lower.contains("by-nc-sa") || lower.contains("noncommercial-sharealike") {
        "CC BY-NC-SA 4.0 (the original theme's; no commercial use)".into()
    } else if lower.contains("by-nc") || lower.contains("noncommercial") {
        "CC BY-NC (the original theme's; no commercial use)".into()
    } else if lower.contains("mit license") || lower.starts_with("mit") || lower.contains("\nmit") {
        "MIT (the original theme's; see original/)".into()
    } else if lower.contains("creative commons") {
        "Creative Commons (the original theme's; see original/)".into()
    } else {
        "see original/ (the original theme's license)".into()
    }
}
fn noncommercial(license: &str) -> bool {
    license.contains("NC")
}

// ---------------------------------------------------------------- check

thread_local! { static CAUGHT: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) }; }
struct Catch;
impl log::Log for Catch {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            CAUGHT.with(|c| {
                if let Some(list) = c.borrow_mut().as_mut() {
                    list.push(r.args().to_string());
                }
            });
        }
    }
    fn flush(&self) {}
}
/// The warnings `run` logs on this thread. The first call sets the
/// process's logger; a program with its own logger gets nothing here.
pub fn warnings_of(run: impl FnOnce()) -> Vec<String> {
    static LOGGER: Catch = Catch;
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
    });
    CAUGHT.with(|c| *c.borrow_mut() = Some(Vec::new()));
    run();
    let mut caught = CAUGHT.with(|c| c.borrow_mut().take().unwrap_or_default());
    caught.sort();
    caught.dedup();
    caught
}

/// The launcher's warnings reading the theme in `dir`: every view at
/// every size, with every choice of every option.
pub fn check(dir: &Path) -> Vec<String> {
    warnings_of(|| {
        let theme = archr_ui::Theme::load(dir);
        let mut themes = vec![theme.clone()];
        for o in theme.options() {
            for c in &o.choices {
                themes.push(theme.with_options(&[(o.id.clone(), c.id.clone())]));
            }
        }
        for t in &themes {
            for (w, h) in SIZES {
                let fitted = t.fitted(w, h);
                for (id, _) in fitted.views() {
                    let _ = fitted.view(&id);
                }
            }
        }
    })
}

// ---------------------------------------------------------------- report

fn report(
    src: &Path,
    name: &str,
    license: &str,
    c: &Conversion,
    b: &Built,
    mapped: &BTreeSet<String>,
    optional: &[es::Subset],
) -> String {
    let folder = src
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let mut r = String::new();
    let _ = writeln!(r, "# Conversão: {folder} → {name}\n");
    let _ = writeln!(
        r,
        "Feito pelo `archr-es2theme` a partir do tema do EmulationStation em `{folder}`. \
         O `theme.toml` desta pasta é um tema de telas livres do archr-launcher \
         (docs/temas.md, \"Telas livres\"); revise-o à mão antes de publicar.\n"
    );
    let _ = writeln!(r, "## Licença\n");
    let _ = writeln!(r, "- Licença do original: **{license}**. Os arquivos de licença e créditos do original estão em `original/`.");
    let _ = writeln!(r, "- A arte, as fontes e os logos são do tema original e foram só copiados; o conversor não traz arte própria.");
    if noncommercial(license) {
        let _ = writeln!(
            r,
            "- **Uso não comercial.** Pode ser distribuído de graça, inclusive na Loja de temas, \
             com os créditos dos autores, o texto da licença (`LICENSE`) e a lista do que mudou \
             em relação ao original (no README)."
        );
        if license.contains("SA") {
            let _ = writeln!(
                r,
                "- **Mesma licença (SA).** O tema convertido e qualquer versão dele ficam na licença do original."
            );
        }
        let _ = writeln!(
            r,
            "- **Fora das imagens do Arch R.** Lojas vendem consoles com o sistema já gravado, \
             e um tema não comercial dentro da imagem iria junto numa venda."
        );
    }
    let _ = writeln!(r, "\n## Resultado\n");
    let _ = writeln!(
        r,
        "- Visualizações: {} ({}).",
        c.views.len(),
        b.views
            .iter()
            .map(|v| format!("`{}` \"{}\"", v.id, v.name.1))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(r, "- Cada visualização tem as telas de sistemas, de jogos e de detalhes (a de jogos, com os botões no lugar da lista).");
    let _ = writeln!(
        r,
        "- Arquivos copiados: {} ({:.1} MB).",
        c.files,
        c.bytes as f64 / 1_048_576.
    );
    let _ = writeln!(r, "- Variáveis: {}.", b.vars.len());
    if b.options.is_empty() {
        let _ = writeln!(r, "- Opções do tema: nenhuma.");
    } else {
        let _ = writeln!(r, "- Opções do tema (Aparência › Opções do tema):");
        for o in &b.options {
            let _ = writeln!(
                r,
                "  - `{}` \"{}\", do subset `{}`: {}.",
                o.id,
                o.name.1,
                o.subset,
                o.choices
                    .iter()
                    .map(|(id, n, _)| format!("`{id}` \"{}\"", n.1))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    let shapes: Vec<&str> = SHAPES.iter().map(|(s, _)| *s).collect();
    let _ = writeln!(
        r,
        "- Formatos de tela lidos: {} (`[aspect.<formato>.vars]` guarda o que muda).",
        shapes.join(", ")
    );
    let skipped: Vec<&str> = optional
        .iter()
        .filter(|s| !b.options.iter().any(|o| o.subset == s.name))
        .map(|s| s.name.as_str())
        .collect();
    if !skipped.is_empty() {
        let _ = writeln!(
            r,
            "- Subsets sem efeito no launcher: {}.",
            skipped
                .iter()
                .map(|s| format!("`{s}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let _ = writeln!(r, "\n## O que virou o quê\n");
    for m in mapped {
        let _ = writeln!(r, "- {m}");
    }
    let _ = writeln!(r, "\n## O que ficou de fora ou mudou\n");
    if b.split > 0 {
        let _ = writeln!(
            r,
            "- {} elementos mudam com o formato de tela e com uma opção ao mesmo tempo: estão escritos uma vez por formato (`when.aspect`), cada um com as suas variáveis.",
            b.split
        );
    }
    for l in &b.lost {
        let _ = writeln!(r, "- {l}");
    }
    for n in &c.notes {
        let _ = writeln!(r, "- {n}");
    }
    let _ = writeln!(r, "- Imagens com `${{system}}` no nome usam os nomes de sistema do tema original; um sistema do launcher com outro nome fica sem a imagem (os logos, copiados para `logos/`, também acham o nome que o sistema divide).");
    let _ = writeln!(
        r,
        "- Elementos que o ES desenha por padrão sem o tema citá-los não são criados."
    );
    let _ = writeln!(r, "\n## Avisos do launcher ao ler o tema\n");
    if c.warnings.is_empty() {
        let _ = writeln!(r, "Nenhum: o tema carrega sem avisos em todas as visualizações, em 640×480, 720×720, 1024×768 e 854×480, com todas as escolhas das opções.");
    } else {
        for w in &c.warnings {
            let _ = writeln!(r, "- {w}");
        }
    }
    r
}
