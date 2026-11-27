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
}
impl Default for Options {
    fn default() -> Self {
        Self {
            lang: "en".into(),
            name: None,
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
                let mut m = map::model(&es, src, Shape { ratio: shape_ratio });
                if let Some(v) = variant {
                    for view in &mut m.views {
                        view.id = format!("{}-{}", map::id_of(v), view.id);
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
    for r in &reads.at {
        notes.extend(r.model.notes.iter().cloned());
        files.extend(r.model.files.iter().cloned());
        mapped.extend(r.model.mapped.iter().cloned());
    }
    let built = build::build(&reads, &optional, en.as_ref(), pt.as_ref(), &mut notes);

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (count, bytes) = copy_files(src, out, &files, &mut notes)?;
    let license = license_of(src);
    let name = opts.name.clone().unwrap_or_else(|| {
        src.file_name()
            .map_or("ES theme".into(), |n| n.to_string_lossy().into_owned())
    });
    for (file, text) in write_toml(&built, &name, src, &license) {
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
fn write_toml(b: &Built, name: &str, src: &Path, license: &str) -> Vec<(String, String)> {
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
            details(&mut out, v);
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
    let _ = writeln!(
        top,
        "description = {}",
        names(
            &format!("Converted from the EmulationStation theme \"{folder}\"."),
            &format!("Convertido do tema \"{folder}\" do EmulationStation.")
        )
    );
    let _ = writeln!(
        top,
        "author = {}",
        inline(&Value::String(format!(
            "{folder} (original); converted by archr-es2theme"
        )))
    );
    let _ = writeln!(top, "version = \"1.0.0\"");
    // The launcher it was converted for: the features it uses (frame tint,
    // default_view...) draw broken on an older one, which refuses it then.
    let _ = writeln!(top, "min_launcher = \"{}\"", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(top, "license = {}", inline(&Value::String(license.into())));
    for (k, v) in &b.top {
        let _ = writeln!(top, "{} = {}", key(k), inline(v));
    }
    parts.insert(0, ("theme.toml".into(), top));
    parts
}

/// The details screen: the games screen, with the game's cover, facts and
/// buttons where its list was, and the description scrolling.
fn details(out: &mut String, v: &build::BuiltView) {
    let at = format!("view.{}.details", key(&v.id));
    let _ = writeln!(
        out,
        "\n# Detalhes: a tela de jogos com a capa, a ficha e os botões no lugar da lista."
    );
    let _ = writeln!(out, "[{at}]");
    let _ = writeln!(out, "base = \"games\"");
    if !v.collection.is_empty() {
        let ids: Vec<Value> = v
            .collection
            .iter()
            .map(|c| Value::String(c.clone()))
            .collect();
        let _ = writeln!(out, "remove = {}", inline(&Value::Array(ids)));
    }
    for d in &v.description {
        let _ = writeln!(
            out,
            "\n[[{at}.el]]\nid = {}\nlong_text = \"scroll\"",
            inline(&Value::String(d.clone()))
        );
    }
    let Some(r) = &v.collection_rect else { return };
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
    let ink = |k: &str| v.list_ink.get(k).map(inline);
    // A dark card behind them, for any art under the list's place.
    let mut card = Table::new();
    card.insert("x".into(), Value::Float(map::r5((x - pad / 2.) as f32)));
    card.insert("y".into(), Value::Float(map::r5((y - pad / 2.) as f32)));
    card.insert("w".into(), Value::Float(map::r5((w + pad) as f32)));
    card.insert("h".into(), Value::Float(map::r5((h + pad) as f32)));
    let _ = writeln!(
        out,
        "\n[[{at}.el]]\nid = \"fundo-ficha\"\nkind = \"rect\"\nrect = {}\nfill = \"#00000099\"\nradius = 0.012\nz = 899",
        inline(&Value::Table(card))
    );
    let size = v
        .list_ink
        .get("size")
        .and_then(Value::as_float)
        .unwrap_or(0.035)
        .clamp(0.025, 0.045);
    let _ = writeln!(
        out,
        "\n[[{at}.el]]\nid = \"capa\"\nkind = \"image\"\nsrc = [\"game.cover\", \"game.screenshot\"]\nrect = {}\nradius = 0.01\nz = 900",
        box_of(y, h * 0.5)
    );
    let _ = writeln!(
        out,
        "\n[[{at}.el]]\nid = \"ficha\"\nkind = \"fields\"\nrows = [\n  {{ label = \"@genre\", value = \"${{game.genre}}\" }},\n  {{ label = \"@players\", value = \"${{game.players}}\" }},\n  {{ label = \"@developer\", value = \"${{game.developer}}\" }},\n  {{ label = \"@last_played\", value = \"${{game.last_played}}\" }},\n]\nrect = {}\nsize = {}\nupper_labels = true\nz = 900",
        box_of(y + h * 0.53, h * 0.32),
        inline(&Value::Float(map::r5((size * 0.85) as f32)))
    );
    for (k, field) in [
        ("font", "font"),
        ("color", "color"),
        ("accent", "label_color"),
    ] {
        if let Some(value) = ink(k) {
            let _ = writeln!(out, "{field} = {value}");
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
                pairs.insert(file, out.join(dest));
            }
        } else if src.join(from).is_file() {
            pairs.insert(src.join(from), out.join(to));
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
            if keep && e.path().is_file() {
                let dest = out.join("original").join(e.file_name());
                pairs.insert(e.path(), dest);
            }
        }
    }
    let (mut count, mut bytes) = (0, 0);
    for (from, to) in &pairs {
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
                text.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
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
            "- **Uso não comercial.** Não pode ir nas imagens do Arch R nem na Loja de temas sem autorização por escrito do autor."
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
