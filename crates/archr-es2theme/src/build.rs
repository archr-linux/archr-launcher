//! The reads of a theme (one per screen shape and choice) made one: what
//! changes between them becomes variables, `[aspect.<shape>.vars]` for the
//! shapes and `[options.<id>]` choices for the subsets.
use crate::{
    es,
    map::{self, Model, OutEl},
};
use std::collections::{BTreeMap, BTreeSet};
use toml::{Table, Value};

/// The launcher's shapes, each with the ratio a theme's Ratio subset is
/// matched to.
pub const SHAPES: [(&str, f32); 3] = [("classic", 4. / 3.), ("square", 1.), ("wide", 16. / 9.)];

/// One read of the theme.
pub struct Read {
    pub model: Model,
    pub vars: BTreeMap<String, String>,
}
/// Every read: `at[c * 3 + s]` is choice `c` (0: the defaults) on shape `s`.
pub struct Reads {
    pub choices: Vec<Option<(String, String)>>,
    pub at: Vec<Read>,
}
impl Reads {
    pub fn get(&self, s: usize, c: usize) -> &Read {
        &self.at[c * 3 + s]
    }
    pub fn base(&self) -> &Model {
        &self.at[0].model
    }
}

pub struct Built {
    pub views: Vec<BuiltView>,
    pub vars: Vec<(String, Value)>,
    pub aspect_vars: BTreeMap<&'static str, Vec<(String, Value)>>,
    pub options: Vec<BuiltOption>,
    pub top: Table,
    /// Elements written once per shape: what they show changes with the
    /// shape and with a choice at once.
    pub split: usize,
    pub lost: BTreeSet<String>,
}
pub struct BuiltView {
    pub id: String,
    pub name: (String, String),
    pub systems: Vec<OutEl>,
    pub games: Vec<OutEl>,
    /// The ids of the list, carousel or grid (one per shape when split)
    /// and its box on the classic screen.
    pub collection: Vec<String>,
    pub collection_rect: Option<Table>,
    pub list_ink: Table,
    pub description: Vec<String>,
}
/// A choice: its id, its names (English, Portuguese) and the vars it sets.
pub type BuiltChoice = (String, (String, String), Vec<(String, Value)>);
pub struct BuiltOption {
    pub id: String,
    pub subset: String,
    pub name: (String, String),
    pub choices: Vec<BuiltChoice>,
}

fn leaves(v: &Value, at: &str, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Table(t) => {
            for (k, v) in t {
                let at = if at.is_empty() {
                    k.clone()
                } else {
                    format!("{at}.{k}")
                };
                leaves(v, &at, out);
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                leaves(v, &format!("{at}[{i}]"), out);
            }
        }
        other => out.push((at.to_owned(), other.clone())),
    }
}
fn leaf_map(t: &Table) -> BTreeMap<String, Value> {
    let mut l = Vec::new();
    leaves(&Value::Table(t.clone()), "", &mut l);
    l.into_iter().collect()
}
fn set_leaf(v: &mut Value, path: &str, value: Value) {
    let (head, rest) = match path.find(['.', '[']) {
        Some(0) => {
            // "[i]..."
            let end = path.find(']').unwrap_or(path.len());
            let i: usize = path[1..end].parse().unwrap_or(usize::MAX);
            let rest = path[end + 1..].trim_start_matches('.');
            if let Value::Array(a) = v {
                if let Some(item) = a.get_mut(i) {
                    if rest.is_empty() {
                        *item = value;
                    } else {
                        set_leaf(item, rest, value);
                    }
                }
            }
            return;
        }
        Some(at) => (&path[..at], &path[at..]),
        None => (path, ""),
    };
    if let Value::Table(t) = v {
        if let Some(item) = t.get_mut(head) {
            let rest = rest.strip_prefix('.').unwrap_or(rest);
            if rest.is_empty() {
                *item = value;
            } else {
                set_leaf(item, rest, value);
            }
        }
    }
}
fn signature(t: &Table) -> Vec<String> {
    leaf_map(t).into_keys().collect()
}
fn key_of(v: &Value) -> String {
    v.to_string()
}

/// The value a field has when an element leaves it out.
fn default_of(kind: &str, path: &str) -> Option<Value> {
    let s = |v: &str| Some(Value::String(v.into()));
    match (kind, path) {
        (_, "opacity") => Some(Value::Float(1.)),
        (_, "rotate") => Some(Value::Float(0.)),
        ("image" | "carousel" | "grid", "fit" | "item.fit") => s("contain"),
        ("carousel", "align") => s("center"),
        ("image", "align") | ("text", "align") | ("list", "align") => s("left"),
        ("image", "tint") | ("rating", "color") => s("#FFFFFFFF"),
        ("image", "radius") | ("video", "radius") => Some(Value::Float(0.)),
        ("text", "upper") | ("text", "wrap") => Some(Value::Boolean(false)),
        ("text", "valign") => s("middle"),
        (_, "anchor") => s("top-left"),
        _ => None,
    }
}

/// Tables of one element across reads made comparable: lists as long as
/// the longest (the last item again), left-out fields at their defaults.
fn normalize(tables: &mut [Option<Table>]) {
    let present: Vec<Table> = tables.iter().flatten().cloned().collect();
    let Some(first) = present.first() else { return };
    let kind = first
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    // Array lengths, by top-level and one-deep key.
    let mut longest: BTreeMap<String, usize> = BTreeMap::new();
    for t in &present {
        for (k, v) in t {
            if let Value::Array(a) = v {
                let e = longest.entry(k.clone()).or_default();
                *e = (*e).max(a.len());
            }
            if let Value::Table(inner) = v {
                for (k2, v2) in inner {
                    if let Value::Array(a) = v2 {
                        let e = longest.entry(format!("{k}.{k2}")).or_default();
                        *e = (*e).max(a.len());
                    }
                }
            }
        }
    }
    let pad = |v: &mut Value, n: usize| {
        if let Value::Array(a) = v {
            if !a.is_empty() && a.iter().all(|x| x.is_str()) {
                while a.len() < n {
                    let last = a.last().cloned().expect("not empty");
                    a.push(last);
                }
            }
        }
    };
    let keys: BTreeSet<String> = present.iter().flat_map(|t| t.keys().cloned()).collect();
    for t in tables.iter_mut().flatten() {
        for (path, n) in &longest {
            match path.split_once('.') {
                None => {
                    if let Some(v) = t.get_mut(path) {
                        if v.is_str() {
                            *v = Value::Array(vec![v.clone()]);
                        }
                        pad(v, *n);
                    }
                }
                Some((a, b)) => {
                    if let Some(Value::Table(inner)) = t.get_mut(a) {
                        if let Some(v) = inner.get_mut(b) {
                            if v.is_str() {
                                *v = Value::Array(vec![v.clone()]);
                            }
                            pad(v, *n);
                        }
                    }
                }
            }
        }
        for k in &keys {
            if !t.contains_key(k) {
                if let Some(d) = default_of(&kind, k) {
                    t.insert(k.clone(), d);
                }
            }
        }
        if let Some(Value::Table(item)) = t.get_mut("item") {
            let others: BTreeSet<String> = present
                .iter()
                .filter_map(|p| p.get("item").and_then(Value::as_table))
                .flat_map(|i| i.keys().cloned())
                .collect();
            for k in others {
                if !item.contains_key(&k) {
                    if let Some(d) = default_of(&kind, &format!("item.{k}")) {
                        item.insert(k, d);
                    }
                }
            }
        }
    }
    // A src written as a list everywhere when one read lists it.
    let _ = first;
}

fn var_name(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches(['_', '-']).to_owned();
    if out.is_empty() {
        "v".into()
    } else {
        out
    }
}

/// The variables: one per distinct set of values across the reads.
struct Vars<'r> {
    reads: &'r Reads,
    by_values: BTreeMap<String, String>,
    taken: BTreeSet<String>,
    list: Vec<(String, Vec<Value>)>,
}
impl Vars<'_> {
    fn name(&mut self, values: Vec<Value>, hint: &str) -> String {
        let key = values.iter().map(key_of).collect::<Vec<_>>().join("\u{1}");
        if let Some(name) = self.by_values.get(&key) {
            return name.clone();
        }
        let base = self
            .es_name(&values)
            .map_or_else(|| var_name(hint), |n| var_name(&n));
        let mut name = base.clone();
        let mut n = 2;
        while self.taken.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        self.taken.insert(name.clone());
        self.by_values.insert(key, name.clone());
        self.list.push((name.clone(), values));
        name
    }
    /// The theme's own variable that has these values in every read.
    fn es_name(&self, values: &[Value]) -> Option<String> {
        let norm = |raw: &str| -> String {
            if let Some((c, _)) = map::color(raw) {
                return c;
            }
            if let Ok(n) = raw.trim().parse::<f32>() {
                return format!("{}", map::r5(n));
            }
            raw.trim().trim_start_matches("./").to_owned()
        };
        let ours = |v: &Value| -> String {
            match v {
                Value::String(s) => s.clone(),
                Value::Float(f) => format!("{f}"),
                Value::Integer(i) => format!("{i}"),
                other => other.to_string(),
            }
        };
        let distinct: BTreeSet<String> = values.iter().map(key_of).collect();
        if distinct.len() < 2 {
            return None;
        }
        let names: Vec<&String> = self.reads.at[0].vars.keys().collect();
        names
            .into_iter()
            .find(|name| {
                self.reads.at.iter().zip(values).all(|(r, v)| {
                    r.vars.get(*name).is_some_and(|raw| {
                        let (n, o) = (norm(raw), ours(v));
                        n == o || (!n.starts_with('#') && n.len() > 2 && o.ends_with(&n))
                    })
                })
            })
            .cloned()
    }
}

pub fn build(
    reads: &Reads,
    optional: &[es::Subset],
    en: Option<&es::Es>,
    pt: Option<&es::Es>,
    notes: &mut BTreeSet<String>,
) -> Built {
    let choices = reads.choices.len();
    let mut vars = Vars {
        reads,
        by_values: BTreeMap::new(),
        taken: BTreeSet::new(),
        list: Vec::new(),
    };
    let mut built = Built {
        views: Vec::new(),
        vars: Vec::new(),
        aspect_vars: BTreeMap::new(),
        options: Vec::new(),
        top: Table::new(),
        split: 0,
        lost: BTreeSet::new(),
    };
    for view in &reads.base().views {
        let systems = screen(reads, &view.id, true, &view.systems, &mut vars, &mut built);
        let games = screen(reads, &view.id, false, &view.games, &mut vars, &mut built);
        let shaped = |id: &Option<String>| -> Vec<String> {
            let Some(id) = id else { return Vec::new() };
            games
                .iter()
                .filter(|e| e.id == *id || SHAPES.iter().any(|(s, _)| e.id == format!("{id}-{s}")))
                .map(|e| e.id.clone())
                .collect()
        };
        let collection = shaped(&view.collection);
        let description = shaped(&view.description);
        let base_list = view
            .collection
            .as_ref()
            .and_then(|c| view.games.iter().find(|e| &e.id == c));
        let collection_rect = base_list.and_then(|e| e.t.get("rect")?.as_table().cloned());
        let mut list_ink = Table::new();
        if let Some(l) = base_list {
            for k in ["font", "color", "size"] {
                if let Some(v) = l.t.get(k) {
                    list_ink.insert(k.into(), v.clone());
                }
            }
            if let Some(c) = l.t.get("selected").and_then(|s| s.get("fill")) {
                list_ink.insert("accent".into(), c.clone());
            }
        }
        let display = |es: Option<&es::Es>, pt: bool| -> String {
            let found = es.and_then(|es| {
                es.views
                    .iter()
                    .find(|v| {
                        map::id_of(&v.name) == view.id
                            || format!("{}-es", map::id_of(&v.name)) == view.id
                    })
                    .and_then(|v| v.display.clone())
            });
            found.unwrap_or_else(|| builtin_name(&view.id, pt).unwrap_or_else(|| view.name.clone()))
        };
        built.views.push(BuiltView {
            id: view.id.clone(),
            name: (display(en, false), display(pt, true)),
            systems,
            games,
            collection,
            collection_rect,
            list_ink,
            description,
        });
    }
    // The top keys (accent, text...) move with the choices too.
    let mut top = reads.base().top.clone();
    for (key, value) in reads.base().top.clone() {
        let values: Vec<Value> = reads
            .at
            .iter()
            .map(|r| {
                r.model
                    .top
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| value.clone())
            })
            .collect();
        if values.iter().map(key_of).collect::<BTreeSet<_>>().len() > 1 {
            let name = vars.name(values, &key);
            top.insert(key.clone(), Value::String(format!("${name}")));
        }
    }
    built.top = top;

    // The vars: their classic value, where each shape differs, what each
    // choice changes.
    for (name, values) in &vars.list {
        built.vars.push((name.clone(), values[0].clone()));
        for s in 1..3 {
            if key_of(&values[s]) != key_of(&values[0]) {
                built
                    .aspect_vars
                    .entry(SHAPES[s].0)
                    .or_default()
                    .push((name.clone(), values[s].clone()));
            }
        }
    }
    let mut c = 1;
    let mut setters: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for subset in optional {
        let label = |es: Option<&es::Es>, choice: Option<&str>| -> Option<String> {
            let sub = es?.subsets.iter().find(|x| x.name == subset.name)?;
            match choice {
                None => Some(sub.display.clone()),
                Some(ch) => sub
                    .choices
                    .iter()
                    .find(|x| x.name == ch)
                    .map(|x| x.display.clone()),
            }
        };
        let mut option = BuiltOption {
            id: map::id_of(&subset.name),
            subset: subset.name.clone(),
            name: (
                label(en, None).map_or_else(|| title(&subset.display), |l| title(&l)),
                label(pt, None).map_or_else(|| title(&subset.display), |l| title(&l)),
            ),
            choices: Vec::new(),
        };
        for (n, choice) in subset.choices.iter().enumerate() {
            let mut set = Vec::new();
            if n > 0 {
                for (name, values) in &vars.list {
                    // Choice vars hold for every shape (split elements
                    // have one var per shape).
                    let v = &values[c * 3];
                    if key_of(v) != key_of(&values[0]) {
                        set.push((name.clone(), v.clone()));
                        setters
                            .entry(name.clone())
                            .or_default()
                            .insert(subset.name.clone());
                    }
                }
                c += 1;
            }
            let mut id = map::id_of(&choice.name);
            while option.choices.iter().any(|(other, ..)| *other == id) {
                id.push('x');
            }
            option.choices.push((
                id,
                (
                    label(en, Some(&choice.name)).unwrap_or_else(|| choice.display.clone()),
                    label(pt, Some(&choice.name)).unwrap_or_else(|| choice.display.clone()),
                ),
                set,
            ));
        }
        if option.choices.iter().skip(1).any(|(_, _, v)| !v.is_empty()) {
            built.options.push(option);
        } else {
            notes.insert(format!(
                "opção `{}` do tema não muda nada que o launcher desenha; ficou de fora",
                subset.name
            ));
        }
    }
    debug_assert_eq!(c, choices);
    for (var, subsets) in setters {
        if subsets.len() > 1 {
            let list = subsets
                .iter()
                .map(|s| format!("`{s}`"))
                .collect::<Vec<_>>()
                .join(" e ");
            notes.insert(format!(
                "{list} mudam o mesmo valor (`{var}`): com as duas escolhidas, vale a da opção que vem por último em ordem alfabética"
            ));
        }
    }
    built.lost.retain(|l| {
        l.starts_with("a coleção")
            || built
                .options
                .iter()
                .any(|o| l.contains(&format!("`{}`", o.subset)))
    });
    if built.options.len() > 12 {
        notes.insert("o launcher aceita até 12 opções: as outras ficaram de fora".into());
        built.options.truncate(12);
    }
    for o in &mut built.options {
        o.choices.truncate(24);
    }
    built
}

/// "colorset" → "Colorset": a subset without a displayName.
fn title(name: &str) -> String {
    let mut c = name.chars();
    c.next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
}

fn builtin_name(id: &str, pt: bool) -> Option<String> {
    let (en, pt_name) = match id {
        "basic" => ("Basic", "Básica"),
        "detailed" => ("Detailed", "Detalhada"),
        "video" => ("Video", "Com vídeo"),
        "grid" => ("Grid", "Grade"),
        "gamelist" => ("Game list", "Lista de jogos"),
        _ => return None,
    };
    Some(if pt { pt_name } else { en }.into())
}

/// One screen of a view across every read.
fn screen(
    reads: &Reads,
    view: &str,
    systems: bool,
    base: &[OutEl],
    vars: &mut Vars<'_>,
    built: &mut Built,
) -> Vec<OutEl> {
    let choices = reads.choices.len();
    let list_of = |r: &Read| -> Option<Vec<OutEl>> {
        let v = r.model.views.iter().find(|v| v.id == view)?;
        Some(if systems {
            v.systems.clone()
        } else {
            v.games.clone()
        })
    };
    let lists: Vec<Option<Vec<OutEl>>> = reads.at.iter().map(list_of).collect();
    let mut ids: Vec<String> = base.iter().map(|e| e.id.clone()).collect();
    for list in lists.iter().flatten() {
        for e in list {
            if !ids.contains(&e.id) {
                ids.push(e.id.clone());
            }
        }
    }
    let name = if systems { "sistemas" } else { "jogos" };
    let mut out = Vec::new();
    for id in ids {
        let mut tables: Vec<Option<Table>> = lists
            .iter()
            .map(|l| l.as_ref()?.iter().find(|e| e.id == id).map(|e| e.t.clone()))
            .collect();
        normalize(&mut tables);
        // Per shape, the reference: the defaults' table, else the first
        // choice that has it.
        let reference =
            |s: usize| -> Option<&Table> { (0..choices).find_map(|c| tables[c * 3 + s].as_ref()) };
        let mut groups: Vec<(Vec<String>, Vec<usize>)> = Vec::new();
        for s in 0..3 {
            if let Some(t) = reference(s) {
                let sig = signature(t);
                match groups.iter_mut().find(|(g, _)| *g == sig) {
                    Some((_, shapes)) => shapes.push(s),
                    None => groups.push((sig, vec![s])),
                }
            }
        }
        // A screen has one list, carousel or grid, whatever the shape: it
        // is never written twice.
        let collection = (0..3).filter_map(reference).any(|t| {
            matches!(
                t.get("kind").and_then(Value::as_str),
                Some("list" | "grid" | "carousel")
            )
        });
        if collection && groups.len() > 1 {
            let shapes: Vec<usize> = groups.iter().flat_map(|(_, s)| s.clone()).collect();
            let sig = groups[0].0.clone();
            built.lost.insert(format!(
                "a coleção `{id}` (tela de {name}) muda de forma com o formato da tela; vale a do formato {}",
                SHAPES[groups[0].1[0]].0
            ));
            groups = vec![(sig, shapes)];
        }
        for (sig, shapes) in &groups {
            let made = element(
                reads, &id, &tables, shapes, sig, vars, built, name, collection,
            );
            out.extend(made);
        }
    }
    out
}

/// An element on the shapes `shapes`, where its fields are `sig`: one
/// table with variables, or one per shape when it must be.
#[allow(clippy::too_many_arguments)]
fn element(
    reads: &Reads,
    id: &str,
    tables: &[Option<Table>],
    shapes: &[usize],
    sig: &[String],
    vars: &mut Vars<'_>,
    built: &mut Built,
    screen: &str,
    alone: bool,
) -> Vec<OutEl> {
    let choices = reads.choices.len();
    let s0 = shapes[0];
    let rep = (0..choices)
        .find_map(|c| tables[c * 3 + s0].as_ref().filter(|t| signature(t) == sig))
        .cloned()
        .expect("a shape of the group has the element");
    let rep_leaves = leaf_map(&rep);
    // The value of every field in every read: the element's own where it
    // has these fields, its shape's defaults' where a choice drops it.
    let cell = |s: usize, c: usize| -> Option<BTreeMap<String, Value>> {
        tables[c * 3 + s]
            .as_ref()
            .filter(|t| signature(t) == sig)
            .map(leaf_map)
    };
    let mut present = vec![false; 3 * choices];
    for c in 0..choices {
        for s in 0..3 {
            present[c * 3 + s] = tables[c * 3 + s].is_some();
            if shapes.contains(&s) {
                if let Some(t) = &tables[c * 3 + s] {
                    if signature(t) != sig {
                        if let Some(Some((subset, choice))) = reads.choices.get(c) {
                            built.lost.insert(format!(
                                "`{subset}` = `{choice}` muda a forma do elemento `{id}` (tela de {screen}); fica a forma do padrão"
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut matrix: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (path, rep_value) in &rep_leaves {
        let mut values = Vec::with_capacity(3 * choices);
        for c in 0..choices {
            for s in 0..3 {
                // Shapes outside the group take the group's first.
                let s = if shapes.contains(&s) { s } else { s0 };
                let v = cell(s, c)
                    .and_then(|m| m.get(path).cloned())
                    .or_else(|| cell(s, 0).and_then(|m| m.get(path).cloned()))
                    .unwrap_or_else(|| rep_value.clone());
                values.push(v);
            }
        }
        matrix.insert(path.clone(), values);
    }
    // Shown only with some choices: opacity 0 with the others.
    let shown = |c: usize, s: usize| present[c * 3 + if shapes.contains(&s) { s } else { s0 }];
    if (0..choices).any(|c| shapes.iter().any(|&s| shown(c, s) != shown(0, s)))
        || shapes.iter().any(|&s| !shown(0, s))
    {
        let opacity = matrix
            .entry("opacity".into())
            .or_insert_with(|| vec![Value::Float(1.); 3 * choices]);
        for c in 0..choices {
            for s in 0..3 {
                if !shown(c, s) {
                    opacity[c * 3 + s] = Value::Float(0.);
                }
            }
        }
    }
    // Does a field change with the shape and with a choice at once?
    let crossed = matrix.values().any(|v| {
        let by_shape = shapes
            .iter()
            .any(|&s| (0..choices).any(|c| key_of(&v[c * 3 + s]) != key_of(&v[c * 3 + s0])));
        let by_choice = (1..choices).any(|c| {
            shapes
                .iter()
                .any(|&s| key_of(&v[c * 3 + s]) != key_of(&v[s]))
        });
        by_shape
            && by_choice
            && shapes
                .iter()
                .any(|&s| (1..choices).any(|c| key_of(&v[c * 3 + s]) != key_of(&v[s])))
    });
    if crossed && alone && shapes.len() > 1 {
        built.lost.insert(format!(
            "a coleção `{id}` (tela de {screen}) muda com o formato e com uma opção ao mesmo tempo: com a opção escolhida, vale a medida do formato 4:3"
        ));
    }
    if crossed && shapes.len() > 1 && !alone {
        built.split += 1;
        let mut out = Vec::new();
        for &s in shapes {
            let mut one = element(reads, id, tables, &[s], sig, vars, built, screen, true);
            for e in &mut one {
                e.id = format!("{id}-{}", SHAPES[s].0);
            }
            out.extend(one);
        }
        // The first keeps the plain id, for `base` and the details.
        if let Some(first) = out.first_mut() {
            first.id = id.to_owned();
        }
        return out;
    }
    let mut t = Value::Table(rep.clone());
    if let Value::Table(table) = &mut t {
        if matrix.contains_key("opacity") && !table.contains_key("opacity") {
            table.insert("opacity".into(), Value::Float(1.));
        }
    }
    for (path, values) in &matrix {
        let distinct: BTreeSet<String> = values.iter().map(key_of).collect();
        if distinct.len() <= 1 {
            set_leaf(&mut t, path, values[0].clone());
            continue;
        }
        let hint = format!("{id}_{}", path.replace(['.', '['], "_").replace(']', ""));
        let name = vars.name(values.clone(), &hint);
        set_leaf(&mut t, path, Value::String(format!("${name}")));
    }
    let Value::Table(mut t) = t else {
        unreachable!()
    };
    if shapes.len() < 3 {
        let mut when = t
            .get("when")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        when.insert(
            "aspect".into(),
            Value::Array(
                shapes
                    .iter()
                    .map(|s| Value::String(SHAPES[*s].0.into()))
                    .collect(),
            ),
        );
        t.insert("when".into(), Value::Table(when));
    }
    let id = if shapes.len() < 3 && shapes[0] != 0 && !alone {
        format!("{id}-{}", SHAPES[shapes[0]].0)
    } else {
        id.to_owned()
    };
    vec![OutEl { id, t }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_are_set_by_path() {
        let mut t: Value = toml::from_str::<Table>("a = { b = [1, { c = 2 }] }")
            .unwrap()
            .into();
        set_leaf(&mut t, "a.b[1].c", Value::Integer(5));
        set_leaf(&mut t, "a.b[0]", Value::Integer(4));
        assert_eq!(t.to_string().replace(" ", ""), "{a={b=[4,{c=5}]}}");
    }

    #[test]
    fn lists_pad_and_defaults_fill_in() {
        let a: Table =
            toml::from_str("kind = \"image\"\nsrc = [\"x\", \"y\"]\nfit = \"cover\"").unwrap();
        let b: Table = toml::from_str("kind = \"image\"\nsrc = \"x\"").unwrap();
        let mut t = vec![Some(a), Some(b)];
        normalize(&mut t);
        assert_eq!(
            signature(t[0].as_ref().unwrap()),
            signature(t[1].as_ref().unwrap())
        );
        assert_eq!(t[1].as_ref().unwrap()["fit"].as_str(), Some("contain"));
    }
}
