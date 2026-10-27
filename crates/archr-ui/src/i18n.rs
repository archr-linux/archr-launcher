//! Small, bounded TOML catalogs. No locale framework or process-global language.
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

const ENGLISH: &str = include_str!("../../../locales/en.toml");
const MAX_BYTES: u64 = 64 * 1024;
type Catalog = BTreeMap<String, Entry>;

#[derive(Debug)]
enum Entry {
    Text(String),
    Plural(BTreeMap<String, String>),
}

pub struct I18n {
    language: String,
    catalog: Catalog,
    english: Catalog,
    warned: RefCell<HashSet<String>>,
}
impl Default for I18n {
    fn default() -> Self {
        Self {
            language: "en".into(),
            catalog: BTreeMap::new(),
            english: parse(ENGLISH).expect("valid embedded English catalog"),
            warned: RefCell::new(HashSet::new()),
        }
    }
}
impl I18n {
    /// Call once with the already loaded system.language; LANG is used only
    /// when that key is absent. New language files need no registry entry.
    pub fn load(dir: &Path, configured: Option<&str>, lang_env: Option<&str>) -> Self {
        let mut result = Self::default();
        let code = normalize(configured.or(lang_env).unwrap_or("en"));
        let base = code.split('_').next().unwrap_or("en");
        // Portuguese is shipped as pt_BR, also serving the bare pt locale.
        let candidates = [
            code.as_str(),
            base,
            if base == "pt" { "pt_BR" } else { "en" },
            "en",
        ];
        let mut tried = HashSet::new();
        for candidate in candidates {
            if !tried.insert(candidate) {
                continue;
            }
            let path = dir.join(format!("{candidate}.toml"));
            match read_catalog(&path).and_then(|source| {
                parse(&source).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }) {
                Ok(catalog) => {
                    result.language = candidate.to_owned();
                    result.catalog = catalog;
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => log::warn!(
                    "Cannot read catalog {}: {error}; trying fallback",
                    path.display()
                ),
            }
        }
        result
    }
    pub fn language(&self) -> &str {
        &self.language
    }
    pub fn text(&self, key: &str) -> String {
        self.lookup(key, None).to_owned()
    }
    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        interpolate(self.lookup(key, None), args)
    }
    pub fn plural(&self, key: &str, n: f64, args: &[(&str, &str)]) -> String {
        interpolate(self.lookup(key, Some(n)), args)
    }
    fn lookup<'a>(&'a self, key: &'a str, n: Option<f64>) -> &'a str {
        if let Some(text) = self
            .catalog
            .get(key)
            .and_then(|e| select(e, &self.language, n))
        {
            return text;
        }
        // Warn only once per key, including a key missing from the selected
        // catalog but present in English. The embedded-English default is not
        // considered a missing translation.
        if (self.language != "en" || !self.english.contains_key(key))
            && self.warned.borrow_mut().insert(key.to_owned())
        {
            log::warn!(
                "Missing translation key {key:?} in {}; using English or key",
                self.language
            );
        }
        self.english
            .get(key)
            .and_then(|e| select(e, "en", n))
            .unwrap_or(key)
    }
    pub fn players(&self, players: &str) -> String {
        let n = players
            .rsplit(['-', '–'])
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(2.);
        self.plural("metadata.players", n, &[("count", players)])
    }
    /// Calendar-free relative age: months are 30 days, years 365 days.
    /// Future timestamps are clamped to now (RTC/network clock correction).
    pub fn relative_time(&self, timestamp: u64, now: u64) -> String {
        let age = now.saturating_sub(timestamp);
        let (key, count) = match age {
            0..60 => return self.text("time.now"),
            60..3600 => ("time.minutes", age / 60),
            3600..86400 => ("time.hours", age / 3600),
            86400..172800 => return self.text("time.yesterday"),
            172800..2592000 => ("time.days", age / 86400),
            2592000..31536000 => ("time.months", age / 2592000),
            _ => ("time.years", age / 31536000),
        };
        self.plural(key, count as f64, &[("count", &count.to_string())])
    }
}
fn normalize(source: &str) -> String {
    let source = source
        .split(['.', '@'])
        .next()
        .unwrap_or("")
        .replace('-', "_");
    if source.is_empty()
        || source.len() > 64
        || !source
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return "en".into();
    }
    let mut parts = source.split('_');
    let mut code = parts.next().unwrap_or("en").to_ascii_lowercase();
    for part in parts {
        code.push('_');
        code.push_str(&if part.len() == 2 {
            part.to_ascii_uppercase()
        } else {
            part.to_owned()
        });
    }
    code
}
fn parse(source: &str) -> Result<Catalog, String> {
    let values: toml::Table = toml::from_str(source).map_err(|e| format!("{e}"))?;
    values
        .into_iter()
        .map(|(key, value)| {
            let entry = match value {
                toml::Value::String(s) => Entry::Text(s),
                toml::Value::Table(table) => {
                    if !table.contains_key("other") {
                        return Err(format!("Plural {key} lacks other"));
                    }
                    let forms = table
                        .into_iter()
                        .map(|(category, v)| {
                            if !matches!(category.as_str(), "one" | "few" | "many" | "other") {
                                return Err(format!("Invalid plural category {category}"));
                            }
                            v.as_str()
                                .map(|s| (category, s.to_owned()))
                                .ok_or_else(|| format!("Invalid plural {key}"))
                        })
                        .collect::<Result<_, _>>()?;
                    Entry::Plural(forms)
                }
                _ => return Err(format!("Invalid catalog entry {key}")),
            };
            Ok((key, entry))
        })
        .collect()
}
fn select<'a>(entry: &'a Entry, lang: &str, n: Option<f64>) -> Option<&'a str> {
    match entry {
        Entry::Text(s) => Some(s),
        Entry::Plural(forms) => forms
            .get(plural_category(lang, n?))
            .or_else(|| forms.get("other"))
            .map(String::as_str),
    }
}
/// Small CLDR cardinal rules; add a language branch here for future catalogs.
/// Counts in the launcher are integers; f64 also supports fractional `other`.
pub fn plural_category(language: &str, n: f64) -> &'static str {
    if !n.is_finite() {
        return "other";
    }
    let n = n.abs();
    let lang = language.split('_').next().unwrap_or(language);
    match lang {
        "pt" if n.floor() <= 1. => "one",
        "ru" if n.fract() != 0. => "other",
        "ru" => {
            let last = n % 10.;
            let last_two = n % 100.;
            if last == 1. && last_two != 11. {
                "one"
            } else if (2. ..=4.).contains(&last) && !(12. ..=14.).contains(&last_two) {
                "few"
            } else {
                "many"
            }
        }
        _ if n == 1. => "one",
        _ => "other",
    }
}
fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start + 1..].find('}') else {
            result.push_str(&rest[start..]);
            return result;
        };
        let end = start + 1 + end;
        let name = &rest[start + 1..end];
        result.push_str(
            args.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, v)| *v)
                .unwrap_or(&rest[start..=end]),
        );
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}
fn read_catalog(path: &Path) -> io::Result<String> {
    // O_NONBLOCK prevents a FIFO from hanging startup; O_NOFOLLOW rejects a
    // catalog symlink, then fstat validates the actual opened file (no race).
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "catalog must be a regular file of at most 64 KiB",
        ));
    }
    let mut source = String::new();
    file.take(MAX_BYTES + 1).read_to_string(&mut source)?;
    if source.len() as u64 > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "catalog exceeds 64 KiB",
        ));
    }
    Ok(source)
}

/// Unrendered worker feedback: keep presentation language out of workers.
#[derive(Debug)]
pub struct Message {
    pub key: &'static str,
    pub args: Vec<(String, String)>,
}
impl Message {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            args: Vec::new(),
        }
    }
    pub fn with(mut self, name: &str, value: impl ToString) -> Self {
        self.args.push((name.to_owned(), value.to_string()));
        self
    }
    pub fn render(&self, catalog: &I18n) -> String {
        let args: Vec<_> = self
            .args
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        catalog.format(self.key, &args)
    }
}
