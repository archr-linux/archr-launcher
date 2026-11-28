//! An EmulationStation theme (the Batocera/ArkOS dialect, and the ES-DE
//! wrappers `variant`, `colorScheme` and `aspectRatio`) read for one set of
//! choices: includes, variables, subsets and conditions applied, views
//! with their elements merged by name in the order the files give them.
use crate::xml::{self, Node};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub name: String,
    /// The choice's displayName, its variables filled in.
    pub display: String,
}
/// A choice the theme offers its user: a Batocera subset, or the ES-DE
/// variants, color schemes and aspect ratios.
#[derive(Clone, Debug, PartialEq)]
pub struct Subset {
    pub name: String,
    pub display: String,
    pub choices: Vec<Choice>,
}
impl Subset {
    /// "4-3", "16:9", "5_3": width over height.
    pub fn ratio_of(name: &str) -> Option<f32> {
        let (w, h) = name.trim().split_once(['-', ':', '_', 'x'])?;
        let (w, h) = (w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?);
        (w > 0. && h > 0.).then_some(w / h)
    }
    /// A subset that picks the screen's shape.
    pub fn is_ratio(&self) -> bool {
        let name = self.name.to_lowercase();
        (name.contains("ratio") || name.contains("aspect"))
            && self
                .choices
                .iter()
                .all(|c| Self::ratio_of(&c.name).is_some())
    }
    /// The choice nearest a screen of `ratio` (width over height).
    pub fn nearest(&self, ratio: f32) -> Option<&str> {
        self.choices
            .iter()
            .filter_map(|c| Some((c, Self::ratio_of(&c.name)?)))
            .min_by(|a, b| (a.1 - ratio).abs().total_cmp(&(b.1 - ratio).abs()))
            .map(|(c, _)| c.name.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prop {
    pub value: String,
    /// The theme variable the whole value came from, as in `${primaryColor}`.
    pub var: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Anim {
    pub property: String,
    pub from: String,
    pub to: String,
    pub duration: f32,
    pub begin: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct El {
    pub kind: String,
    pub name: String,
    pub extra: bool,
    pub props: Vec<(String, Prop)>,
    /// Every `<path>` met, in order: ES keeps the last one whose file exists.
    pub paths: Vec<String>,
    pub story: Vec<Anim>,
}
impl El {
    pub fn prop(&self, key: &str) -> Option<&Prop> {
        self.props.iter().find(|(k, _)| k == key).map(|(_, p)| p)
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.prop(key).map(|p| p.value.as_str())
    }
    fn set(&mut self, key: &str, prop: Prop) {
        match self.props.iter_mut().find(|(k, _)| k == key) {
            Some((_, old)) => *old = prop,
            None => self.props.push((key.into(), prop)),
        }
    }
    fn merge(&mut self, over: &El) {
        self.extra |= over.extra;
        for (k, p) in &over.props {
            self.set(k, p.clone());
        }
        self.paths.extend(over.paths.iter().cloned());
        if !over.story.is_empty() {
            self.story.clone_from(&over.story);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub name: String,
    pub inherits: Option<String>,
    pub display: Option<String>,
    pub custom: bool,
    pub els: Vec<El>,
}
impl View {
    pub fn el(&self, name: &str) -> Option<&El> {
        self.els.iter().find(|e| e.name == name)
    }
}

/// The theme as one set of choices sees it.
#[derive(Clone, Debug, Default)]
pub struct Es {
    pub views: Vec<View>,
    pub default_view: Option<String>,
    pub subsets: Vec<Subset>,
    pub notes: BTreeSet<String>,
    /// Is it the ES-DE dialect (capabilities.xml, variants)?
    pub esde: bool,
    /// The theme's variables as the files left them.
    pub vars: BTreeMap<String, String>,
    /// ES-DE files included per system (`metadata/${system.theme}.xml`),
    /// from the theme's root: read again for each system (`load_for`).
    pub per_system: Vec<String>,
}
impl Es {
    pub fn view(&self, name: &str) -> Option<&View> {
        self.views.iter().find(|v| v.name == name)
    }
    /// A custom view with what it inherits laid under it.
    pub fn flattened(&self, name: &str) -> Option<View> {
        let mut chain = Vec::new();
        let mut at = self.view(name)?;
        loop {
            chain.push(at);
            match at.inherits.as_deref().and_then(|p| self.view(p)) {
                Some(parent) if chain.len() < 8 && !chain.iter().any(|v| v.name == parent.name) => {
                    at = parent
                }
                _ => break,
            }
        }
        let mut view = View {
            name: name.into(),
            ..chain[0].clone()
        };
        view.els.clear();
        for v in chain.iter().rev() {
            for el in &v.els {
                match view.els.iter_mut().find(|e| e.name == el.name) {
                    Some(old) => old.merge(el),
                    None => view.els.push(el.clone()),
                }
            }
        }
        Some(view)
    }
}

/// Read the theme in `root` with `picked` choices (subset → choice) in the
/// language `lang` (`en`, `pt`...). Choices left out are the first one.
pub fn load(root: &Path, lang: &str, picked: &BTreeMap<String, String>) -> Result<Es, String> {
    load_for(root, lang, picked, None)
}
/// `load` as one system sees it: `${system.theme}` in an include is that
/// system's id. Without a system, those includes are only listed in
/// `per_system`.
pub fn load_for(
    root: &Path,
    lang: &str,
    picked: &BTreeMap<String, String>,
    system: Option<&str>,
) -> Result<Es, String> {
    let entry = root.join("theme.xml");
    if !entry.is_file() {
        return Err(format!("{} has no theme.xml", root.display()));
    }
    let mut loader = Loader {
        root: root.to_path_buf(),
        lang: lang.to_lowercase(),
        picked: picked.clone(),
        system: system.map(str::to_owned),
        vars: BTreeMap::new(),
        es: Es::default(),
        open: Vec::new(),
    };
    loader.capabilities();
    loader.file(&entry)?;
    loader.es.vars = loader.vars;
    Ok(loader.es)
}

struct Loader {
    root: PathBuf,
    lang: String,
    picked: BTreeMap<String, String>,
    system: Option<String>,
    vars: BTreeMap<String, String>,
    es: Es,
    open: Vec<PathBuf>,
}

fn split_names(names: &str) -> Vec<String> {
    names
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Loader {
    fn note(&mut self, text: impl Into<String>) {
        self.es.notes.insert(text.into());
    }

    /// ES-DE lists its variants, color schemes and aspect ratios in
    /// capabilities.xml: they become subsets of the same names.
    fn capabilities(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.root.join("capabilities.xml")) else {
            return;
        };
        let Ok(node) = xml::parse(&text) else {
            self.note("capabilities.xml não pôde ser lido");
            return;
        };
        self.es.esde = true;
        for (tag, subset) in [
            ("variant", "variant"),
            ("colorScheme", "colorScheme"),
            ("fontSize", "fontSize"),
            ("aspectRatio", "aspectRatio"),
        ] {
            let mut choices: Vec<Choice> = node
                .children
                .iter()
                .filter(|c| c.name == tag)
                .filter_map(|c| {
                    let name = c
                        .attr("name")
                        .map_or_else(|| c.text.trim().to_owned(), str::to_owned);
                    // The label in the conversion's language, else en_US's,
                    // else the first.
                    let labels: Vec<&Node> =
                        c.children.iter().filter(|l| l.name == "label").collect();
                    let wanted = locale(&self.lang);
                    let label = labels
                        .iter()
                        .find(|l| {
                            l.attr("language")
                                .is_some_and(|x| x.eq_ignore_ascii_case(&wanted))
                        })
                        .or_else(|| labels.iter().find(|l| l.attr("language") == Some("en_US")))
                        .or_else(|| labels.first())
                        .map_or_else(|| name.clone(), |l| l.text.trim().to_owned());
                    (!name.is_empty()).then_some(Choice {
                        name,
                        display: label,
                    })
                })
                .collect();
            // ES-DE's font size is "medium" until the user picks another.
            if tag == "fontSize" {
                if let Some(at) = choices.iter().position(|c| c.name == "medium") {
                    let medium = choices.remove(at);
                    choices.insert(0, medium);
                }
            }
            if !choices.is_empty() {
                self.es.subsets.push(Subset {
                    name: subset.into(),
                    display: subset.into(),
                    choices,
                });
            }
        }
    }

    fn chosen(&self, subset: &str) -> Option<&str> {
        if let Some(c) = self.picked.get(subset) {
            return Some(c);
        }
        self.es
            .subsets
            .iter()
            .find(|s| s.name == subset)
            .and_then(|s| s.choices.first())
            .map(|c| c.name.as_str())
    }

    /// Does the node's condition hold (ifSubset, ifHelpPrompts, ifArch...)?
    fn active(&mut self, node: &Node) -> bool {
        for (key, value) in &node.attrs {
            let holds = match key.as_str() {
                "ifSubset" => self.if_subset(value),
                // The launcher shows its button hints.
                "ifHelpPrompts" => value.trim() == "true",
                "ifArch" => false,
                "ifNotArch" => true,
                "tinyScreen" => value.trim() != "true",
                other if other.starts_with("if") => {
                    let other = other.to_owned();
                    self.note(format!(
                        "condição `{other}` não existe no launcher; tratada como verdadeira"
                    ));
                    true
                }
                _ => true,
            };
            if !holds {
                return false;
            }
        }
        true
    }
    fn if_subset(&mut self, clauses: &str) -> bool {
        let mut parsed: Vec<(String, Vec<String>)> = Vec::new();
        for clause in clauses.split(',') {
            match clause.split_once(':') {
                Some((name, values)) => parsed.push((
                    name.trim().into(),
                    values.split('|').map(|v| v.trim().to_owned()).collect(),
                )),
                // "Ratio:5-3 | 16-9" written with a comma: more values.
                None => {
                    if let Some(last) = parsed.last_mut() {
                        last.1
                            .extend(clause.split('|').map(|v| v.trim().to_owned()));
                    }
                }
            }
        }
        parsed.iter().all(|(name, values)| {
            self.chosen(name)
                .is_some_and(|c| values.iter().any(|v| v == c))
        })
    }

    fn file(&mut self, path: &Path) -> Result<(), String> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if self.open.contains(&canonical) || self.open.len() > 12 {
            self.note(format!(
                "include repetido ou fundo demais: {}",
                self.rel(path)
            ));
            return Ok(());
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let node = xml::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if self.open.is_empty() {
            self.es.default_view = node.attr("defaultView").map(str::to_owned);
        }
        self.open.push(canonical);
        let dir = path.parent().unwrap_or(&self.root).to_path_buf();
        self.theme(&node, &dir);
        self.open.pop();
        Ok(())
    }

    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// The children of `<theme>` (and of wrappers inside it). ES-DE reads
    /// a file in passes: its variables (and the wrappers that only hold
    /// variables) first, then the includes, the views, the variants and
    /// the aspect ratios; the other dialects, in the file's order.
    fn theme(&mut self, node: &Node, dir: &Path) {
        if !self.es.esde {
            return self.children(node, dir, None);
        }
        for pass in 0..5 {
            self.children(node, dir, Some(pass));
        }
    }
    fn children(&mut self, node: &Node, dir: &Path, pass: Option<u8>) {
        for child in &node.children {
            if let Some(pass) = pass {
                let at = match child.name.as_str() {
                    "variables" | "colorScheme" | "fontSize" | "language" => 0,
                    "include" | "subset" => 1,
                    "view" | "customView" | "feature" => 2,
                    "variant" => 3,
                    "aspectRatio" => 4,
                    _ => 0,
                };
                if at != pass {
                    continue;
                }
            }
            match child.name.as_str() {
                "include" => self.include(child, dir),
                "subset" => self.subset(child, dir),
                "variables" => {
                    if self.active(child) {
                        for var in &child.children {
                            if self.active(var) {
                                let value = self.fill(var.text.trim()).0;
                                self.vars.insert(var.name.clone(), value);
                            }
                        }
                    }
                }
                "view" => self.view(child, dir, false),
                "customView" => self.view(child, dir, true),
                "feature" => self.theme(child, dir),
                "variant" | "colorScheme" | "fontSize" | "aspectRatio" | "language" => {
                    if self.wrapper(child) {
                        self.theme(child, dir);
                    }
                }
                "formatVersion" | "splash" | "sound" => {}
                other => {
                    let other = other.to_owned();
                    self.note(format!(
                        "bloco `<{other}>` do tema não tem equivalente; ignorado"
                    ));
                }
            }
        }
    }
    /// An ES-DE wrapper: does its `name` list the choice in effect (for
    /// `<language>`, the conversion's language)?
    fn wrapper(&mut self, node: &Node) -> bool {
        let subset = node.name.as_str();
        if subset == "language" {
            let locale = locale(&self.lang);
            return split_names(node.attr("name").unwrap_or(""))
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&locale));
        }
        let names = split_names(node.attr("name").unwrap_or(""));
        let chosen = self.chosen(subset).map(str::to_owned);
        names
            .iter()
            .any(|n| n == "all" || Some(n.as_str()) == chosen.as_deref())
    }

    fn include(&mut self, node: &Node, dir: &Path) {
        if let Some(lang) = node.attr("lang") {
            if !self.lang.starts_with(&lang.to_lowercase()) {
                return;
            }
        }
        // The older form: <include subset="colorset" name="Dark">file</include>.
        if let Some(subset) = node.attr("subset") {
            let name = node.attr("name").unwrap_or("").to_owned();
            let display = node.attr("displayName").unwrap_or(&name).to_owned();
            self.register(subset, &name, &display, "");
            if self.chosen(subset) != Some(name.as_str()) {
                return;
            }
        }
        if !self.active(node) {
            return;
        }
        self.include_file(node.text.trim(), dir);
    }
    fn include_file(&mut self, file: &str, dir: &Path) {
        if file.is_empty() {
            return;
        }
        let mut file = self.fill(file).0;
        if file.contains("${system.theme}") {
            match &self.system {
                Some(system) => file = file.replace("${system.theme}", system),
                None => {
                    let rel = self.rel(&normalize(&dir.join(&file)));
                    if !self.es.per_system.contains(&rel) {
                        self.es.per_system.push(rel);
                    }
                    return;
                }
            }
            // A system without a file of its own keeps the defaults, as
            // ES-DE does for an include written with a variable.
            if !normalize(&dir.join(&file)).is_file() {
                return;
            }
        }
        let path = normalize(&dir.join(&file));
        let inside = path
            .strip_prefix(&self.root)
            .ok()
            .and_then(|rel| contained(&self.root, &rel.to_string_lossy()))
            .is_some();
        if !inside || !path.is_file() {
            self.note(format!("include que não existe: {file}"));
            return;
        }
        if let Err(e) = self.file(&path) {
            self.note(format!("include ilegível: {e}"));
        }
    }
    fn register(&mut self, subset: &str, name: &str, display: &str, subset_display: &str) {
        let display = self.fill(display).0;
        let subset_display = self.fill(subset_display).0;
        let at = match self.es.subsets.iter().position(|s| s.name == subset) {
            Some(at) => at,
            None => {
                self.es.subsets.push(Subset {
                    name: subset.into(),
                    display: if subset_display.is_empty() {
                        subset.into()
                    } else {
                        subset_display
                    },
                    choices: Vec::new(),
                });
                self.es.subsets.len() - 1
            }
        };
        let s = &mut self.es.subsets[at];
        if !name.is_empty() && !s.choices.iter().any(|c| c.name == name) {
            s.choices.push(Choice {
                name: name.into(),
                display,
            });
        }
    }
    fn subset(&mut self, node: &Node, dir: &Path) {
        let Some(subset) = node.attr("name").map(str::to_owned) else {
            return;
        };
        let subset_display = node.attr("displayName").unwrap_or(&subset).to_owned();
        let mut chosen_file = None;
        for choice in node.children.iter().filter(|c| c.name == "include") {
            if !self.active(choice) {
                continue;
            }
            let name = choice.attr("name").unwrap_or("").to_owned();
            let display = choice.attr("displayName").unwrap_or(&name).to_owned();
            let first = !self
                .es
                .subsets
                .iter()
                .any(|s| s.name == subset && s.choices.iter().any(|c| c.name == name));
            self.register(&subset, &name, &display, &subset_display);
            if first && self.chosen(&subset) == Some(name.as_str()) && chosen_file.is_none() {
                chosen_file = Some(choice.text.trim().to_owned());
            }
        }
        if let Some(file) = chosen_file {
            self.include_file(&file, dir);
        }
    }

    fn view(&mut self, node: &Node, dir: &Path, custom: bool) {
        if !self.active(node) {
            return;
        }
        let raw = node.attr("name").unwrap_or("");
        let names = if custom {
            vec![raw.trim().to_owned()]
        } else {
            split_names(raw)
        };
        for name in names {
            let at = match self.es.views.iter().position(|v| v.name == name) {
                Some(at) => at,
                None => {
                    self.es.views.push(View {
                        name: name.clone(),
                        custom,
                        ..View::default()
                    });
                    self.es.views.len() - 1
                }
            };
            if custom {
                let inherits = node.attr("inherits").map(str::to_owned);
                let display = node.attr("displayName").map(|d| self.fill(d).0);
                let view = &mut self.es.views[at];
                view.inherits = inherits.or(view.inherits.take());
                view.display = display.or(view.display.take());
            }
            self.elements(at, node, dir);
        }
    }
    fn elements(&mut self, view: usize, node: &Node, dir: &Path) {
        for child in &node.children {
            if matches!(child.name.as_str(), "feature") {
                self.elements(view, child, dir);
                continue;
            }
            if matches!(
                child.name.as_str(),
                "variant" | "colorScheme" | "fontSize" | "aspectRatio" | "language"
            ) {
                if self.wrapper(child) {
                    self.elements(view, child, dir);
                }
                continue;
            }
            if !self.active(child) {
                continue;
            }
            let Some(element) = self.element(child, dir) else {
                continue;
            };
            for name in split_names(child.attr("name").unwrap_or("")) {
                // ES ignores a built-in element written with another type.
                if builtin_kind(&name).is_some_and(|k| k != element.kind) && !element.extra {
                    continue;
                }
                let mut el = element.clone();
                el.name.clone_from(&name);
                let els = &mut self.es.views[view].els;
                match els.iter_mut().find(|e| e.name == name) {
                    Some(old) if old.kind == el.kind => old.merge(&el),
                    Some(_) => {
                        let text =
                            format!("elemento `{name}` aparece com dois tipos; vale o primeiro");
                        self.note(text);
                    }
                    None => els.push(el),
                }
            }
        }
    }
    fn element(&mut self, node: &Node, dir: &Path) -> Option<El> {
        let mut el = El {
            kind: node.name.clone(),
            name: String::new(),
            extra: node.attr("extra") == Some("true"),
            props: Vec::new(),
            paths: Vec::new(),
            story: Vec::new(),
        };
        for prop in &node.children {
            if !self.active(prop) {
                continue;
            }
            match prop.name.as_str() {
                "storyboard" => {
                    if prop.attr("event").is_some_and(|e| e != "activate") {
                        continue;
                    }
                    el.story = prop
                        .children
                        .iter()
                        .filter(|a| a.name == "animation")
                        .map(|a| Anim {
                            property: a.attr("property").unwrap_or("").into(),
                            from: a.attr("from").unwrap_or("").into(),
                            to: a.attr("to").unwrap_or("").into(),
                            duration: a
                                .attr("duration")
                                .and_then(|d| d.parse().ok())
                                .unwrap_or(0.),
                            begin: a.attr("begin").and_then(|d| d.parse().ok()).unwrap_or(0.),
                        })
                        .collect();
                }
                key => {
                    let (value, var) = self.fill(prop.text.trim());
                    if is_path(key) {
                        if let Some(path) = self.path(&value, dir) {
                            if key == "path" {
                                el.paths.push(path);
                            } else {
                                el.set(key, Prop { value: path, var });
                            }
                        } else if key != "path" {
                            el.set(key, Prop::default());
                        }
                    } else {
                        el.set(key, Prop { value, var });
                    }
                }
            }
        }
        Some(el)
    }

    /// `${name}` replaced by the theme's variables; `${system.*}` stays.
    /// The variable's name when it is the whole value.
    fn fill(&self, text: &str) -> (String, Option<String>) {
        let whole = text
            .strip_prefix("${")
            .and_then(|t| t.strip_suffix('}'))
            .filter(|n| self.vars.contains_key(*n) && !n.contains('}'))
            .map(str::to_owned);
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("${") {
            let Some(end) = rest[start..].find('}') else {
                break;
            };
            let name = &rest[start + 2..start + end];
            out.push_str(&rest[..start]);
            match self.vars.get(name) {
                Some(v) => out.push_str(v),
                None => out.push_str(&rest[start..start + end + 1]),
            }
            rest = &rest[start + end + 1..];
        }
        out.push_str(rest);
        (out, whole)
    }

    /// A file of the theme, as a path from its root with `${system.theme}`
    /// kept; None for what lives outside it.
    fn path(&mut self, raw: &str, dir: &Path) -> Option<String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if raw.starts_with(":/")
            || raw.starts_with('{')
            || raw.starts_with('~')
            || raw.starts_with('/')
        {
            self.note(format!(
                "arquivo fora do tema ou do ES (`{raw}`) não é copiado; ignorado"
            ));
            return None;
        }
        let raw = fix_bare_system(raw);
        // Relative to the file that says it, or (a variable written in
        // another folder, a theme laid out per system) to the theme's root.
        let candidates = [normalize(&dir.join(&raw)), clamp(&self.root, &raw)];
        let found = candidates
            .iter()
            .find(|p| p.starts_with(&self.root) && exists(p))
            .or_else(|| candidates.iter().find(|p| p.starts_with(&self.root)))?;
        let rel = found
            .strip_prefix(&self.root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        Some(rel)
    }
}

/// The ES-DE locale a conversion language reads: `pt` is `pt_BR`, `en`
/// `en_US`, `de` `de_DE`; one with its region stays as it is.
fn locale(lang: &str) -> String {
    let lang = lang.replace('-', "_");
    if let Some((l, r)) = lang.split_once('_') {
        return format!("{}_{}", l.to_lowercase(), r.to_uppercase());
    }
    let l = lang.to_lowercase();
    match l.as_str() {
        "pt" => "pt_BR".into(),
        "en" => "en_US".into(),
        "zh" => "zh_CN".into(),
        "ja" => "ja_JP".into(),
        "ko" => "ko_KR".into(),
        "sv" => "sv_SE".into(),
        "ar" => "ar_SA".into(),
        "sr" => "sr_RS".into(),
        "bs" => "bs_BA".into(),
        "ca" => "ca_ES".into(),
        "hr" => "hr_HR".into(),
        "vi" => "vi_VN".into(),
        _ => format!("{l}_{}", l.to_uppercase()),
    }
}

/// `$system` and `$system.name` (the old spelling) as `${system.theme}`
/// and `${system.name}`.
fn fix_bare_system(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(at) = rest.find("$system") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 7..];
        if after.starts_with('}') || after.starts_with('.') && rest[..at].ends_with('{') {
            out.push_str("$system");
            rest = after;
            continue;
        }
        let field = ["theme", "name", "fullName"]
            .into_iter()
            .find(|f| after.strip_prefix('.').is_some_and(|a| a.starts_with(f)));
        match field {
            Some(f) => {
                out.push_str(&format!("${{system.{f}}}"));
                rest = &after[1 + f.len()..];
            }
            None => {
                out.push_str("${system.theme}");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The type of ES's built-in elements, by name.
pub fn builtin_kind(name: &str) -> Option<&'static str> {
    Some(match name {
        "md_releasedate" | "md_lastplayed" => "datetime",
        "md_rating" => "rating",
        "md_image" | "md_thumbnail" | "md_marquee" | "md_fanart" | "md_wheel" | "logo"
        | "background" => "image",
        "md_video" => "video",
        "gamelist" => "textlist",
        "gamegrid" => "imagegrid",
        "systemcarousel" => "carousel",
        "help" => "helpsystem",
        "systemInfo" | "logoText" | "md_name" | "md_description" | "md_developer"
        | "md_publisher" | "md_genre" | "md_players" | "md_playcount" | "md_favorite" => "text",
        n if n.starts_with("md_lbl_") => "text",
        _ => return None,
    })
}

fn is_path(key: &str) -> bool {
    matches!(
        key,
        "path"
            | "default"
            | "staticImage"
            | "defaultImage"
            | "defaultFolderImage"
            | "gameImage"
            | "folderImage"
            | "selectorImage"
            | "backgroundImage"
    ) || key.ends_with("Path")
}

/// `rel` (a path a theme names) inside `root`, or None: an absolute path,
/// a `..` or a link that leads out of `root` is refused. A file that does
/// not exist yet (an output) is checked by the folder it goes in, which
/// must exist. Every path a theme names is read or written through this.
pub fn contained(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    if rel.as_os_str().is_empty()
        || !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let path = root.join(rel);
    let root = canonical(root)?;
    let real = match path.canonicalize() {
        Ok(real) => real,
        Err(_) => {
            let parent = path.parent()?.canonicalize().ok()?;
            parent.join(path.file_name()?)
        }
    };
    real.starts_with(&root).then_some(path)
}

/// A root folder's real path, found once: `contained` asks it for every file.
fn canonical(root: &Path) -> Option<PathBuf> {
    thread_local! {
        static SEEN: std::cell::RefCell<Vec<(PathBuf, PathBuf)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    if let Some(real) = SEEN.with(|s| {
        s.borrow()
            .iter()
            .find(|(p, _)| p == root)
            .map(|(_, r)| r.clone())
    }) {
        return Some(real);
    }
    let real = root.canonicalize().ok()?;
    SEEN.with(|s| s.borrow_mut().push((root.to_path_buf(), real.clone())));
    Some(real)
}

/// The path without `.` and `..`.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}
/// `raw` from the root, its leading `..` dropped.
fn clamp(root: &Path, raw: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for c in Path::new(raw).components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::ParentDir if out != root => {
                out.pop();
            }
            _ => {}
        }
    }
    out
}
/// The file exists, or for a path with `${...}`, the folder it is in.
fn exists(path: &Path) -> bool {
    let text = path.to_string_lossy();
    match text.find("${") {
        Some(at) => Path::new(&text[..at]).parent().is_some_and(|p| p.is_dir()),
        None => path.exists(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_names_read_and_the_nearest_wins() {
        assert_eq!(Subset::ratio_of("16-9"), Some(16. / 9.));
        assert_eq!(Subset::ratio_of("4:3"), Some(4. / 3.));
        assert_eq!(Subset::ratio_of("medium"), None);
        let s = Subset {
            name: "Ratio".into(),
            display: String::new(),
            choices: ["3-2", "4-3", "5-3"]
                .iter()
                .map(|n| Choice {
                    name: (*n).into(),
                    display: String::new(),
                })
                .collect(),
        };
        assert!(s.is_ratio());
        assert_eq!(s.nearest(1.0), Some("4-3"));
        assert_eq!(s.nearest(16. / 9.), Some("5-3"));
    }

    #[test]
    fn the_old_system_spelling_is_read() {
        assert_eq!(fix_bare_system("a/$system.png"), "a/${system.theme}.png");
        assert_eq!(
            fix_bare_system("a/${system.theme}.png"),
            "a/${system.theme}.png"
        );
        assert_eq!(
            fix_bare_system("a/$system.theme.jpg"),
            "a/${system.theme}.jpg"
        );
    }
}
