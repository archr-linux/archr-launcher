//! Theme sounds: which file plays for each event. Pure data, no audio; the
//! launcher decodes and plays. A theme needs no declaration: files named after
//! the events in `sounds/` (and `sounds/systems/<system>`) are found by name,
//! and an optional `[sounds]` table renames or silences them.
use archr_io::{read_utf8, Symlinks::Follow};
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_THEME_BYTES: u64 = 64 * 1024;
/// Longer files are music, not interface feedback.
const MAX_SOUND_BYTES: u64 = 1024 * 1024;
const EXTENSIONS: [&str; 2] = ["ogg", "wav"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    Boot,
    Move,
    Page,
    Select,
    Back,
    Launch,
    MenuOpen,
    MenuClose,
    Toggle,
    Error,
    Notice,
    Favorite,
}

impl Event {
    pub const ALL: [Event; 12] = [
        Self::Boot,
        Self::Move,
        Self::Page,
        Self::Select,
        Self::Back,
        Self::Launch,
        Self::MenuOpen,
        Self::MenuClose,
        Self::Toggle,
        Self::Error,
        Self::Notice,
        Self::Favorite,
    ];
    /// The theme key and default file stem.
    pub fn name(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Move => "move",
            Self::Page => "page",
            Self::Select => "select",
            Self::Back => "back",
            Self::Launch => "launch",
            Self::MenuOpen => "menu_open",
            Self::MenuClose => "menu_close",
            Self::Toggle => "toggle",
            Self::Error => "error",
            Self::Notice => "notice",
            Self::Favorite => "favorite",
        }
    }
}

/// Resolved files of one theme. An event without a file is silent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sounds {
    events: HashMap<Event, PathBuf>,
    systems: HashMap<String, PathBuf>,
}

impl Sounds {
    pub fn load(dir: &Path) -> Self {
        let table = read_utf8(&dir.join("theme.toml"), MAX_THEME_BYTES, Follow)
            .ok()
            .and_then(|source| toml::from_str::<toml::Table>(&source).ok())
            .and_then(|table| match table.get("sounds") {
                Some(toml::Value::Table(sounds)) => Some(sounds.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let declared = |table: &toml::Table, name: &str| match table.get(name) {
            Some(toml::Value::String(path)) => Some(path.clone()),
            _ => None,
        };
        let mut sounds = Self::default();
        for event in Event::ALL {
            let file = match declared(&table, event.name()) {
                Some(path) if path.is_empty() => continue,
                Some(path) => confine(dir, &path),
                None => by_name(&dir.join("sounds"), event.name()),
            };
            sounds.events.extend(file.map(|file| (event, file)));
        }
        let systems = dir.join("sounds/systems");
        for entry in fs::read_dir(&systems).into_iter().flatten().flatten() {
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                sounds.systems.extend(
                    by_name(&systems, stem)
                        .filter(|file| *file == path)
                        .map(|file| (stem.to_owned(), file)),
                );
            }
        }
        if let Some(toml::Value::Table(declared)) = table.get("systems") {
            for (system, path) in declared {
                match path.as_str() {
                    Some("") => sounds.systems.remove(system),
                    Some(path) => sounds
                        .systems
                        .insert(system.clone(), confine(dir, path).unwrap_or_default()),
                    None => None,
                };
            }
        }
        sounds
            .systems
            .retain(|_, file| !file.as_os_str().is_empty());
        sounds
    }
    pub fn event(&self, event: Event) -> Option<&Path> {
        self.events.get(&event).map(PathBuf::as_path)
    }
    pub fn system(&self, system: &str) -> Option<&Path> {
        self.systems.get(system).map(PathBuf::as_path)
    }
    /// Every distinct file to preload.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<_> = self.events.values().chain(self.systems.values()).collect();
        files.sort();
        files.dedup();
        files.into_iter().cloned().collect()
    }
}

/// `<folder>/<stem>.ogg` or `.wav`, when it exists and is small.
fn by_name(folder: &Path, stem: &str) -> Option<PathBuf> {
    EXTENSIONS
        .iter()
        .map(|extension| folder.join(format!("{stem}.{extension}")))
        .find(|path| usable(path))
}

/// A declared path stays inside the theme directory.
fn confine(dir: &Path, value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    let relative = path
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    let found = dir.join(path).canonicalize().ok();
    let inside = found
        .as_ref()
        .zip(dir.canonicalize().ok())
        .is_some_and(|(found, dir)| found.starts_with(dir));
    let known = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
    if relative && inside && known && found.as_deref().is_some_and(usable) {
        found
    } else {
        log::warn!("Theme sound {value:?} is not a small .ogg/.wav file inside the theme; silent");
        None
    }
}

fn usable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_SOUND_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn theme(files: &[&str], toml: &str) -> (TempDir, Sounds) {
        let dir = TempDir::new().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"RIFF").unwrap();
        }
        fs::write(dir.path().join("theme.toml"), toml).unwrap();
        let sounds = Sounds::load(dir.path());
        (dir, sounds)
    }

    #[test]
    fn theme_without_sounds_is_silent() {
        let (_dir, sounds) = theme(&[], "margin = 0.1");
        assert!(Event::ALL.iter().all(|e| sounds.event(*e).is_none()));
        assert!(sounds.files().is_empty());
    }

    #[test]
    fn default_names_need_no_declaration() {
        let (dir, sounds) = theme(&["sounds/move.ogg", "sounds/menu_open.wav"], "");
        assert_eq!(
            sounds.event(Event::Move),
            Some(dir.path().join("sounds/move.ogg").as_path())
        );
        assert!(sounds.event(Event::MenuOpen).is_some());
        assert!(sounds.event(Event::Back).is_none());
    }

    #[test]
    fn table_renames_and_silences() {
        let (dir, sounds) = theme(
            &["sounds/move.ogg", "sounds/tick.wav", "sounds/select.ogg"],
            "[sounds]\nmove = \"sounds/tick.wav\"\nselect = \"\"\nback = \"sounds/missing.ogg\"",
        );
        assert_eq!(
            sounds.event(Event::Move),
            Some(dir.path().join("sounds/tick.wav").as_path())
        );
        assert!(sounds.event(Event::Select).is_none());
        assert!(sounds.event(Event::Back).is_none());
    }

    #[test]
    fn declared_paths_stay_inside_the_theme() {
        let (_dir, sounds) = theme(
            &["sounds/a.ogg"],
            "[sounds]\nmove = \"../outside.ogg\"\npage = \"/etc/passwd\"\nback = \"sounds/a.mp3\"",
        );
        assert!(sounds.files().iter().all(|f| f.ends_with("a.ogg")));
        assert!(sounds.event(Event::Move).is_none());
        assert!(sounds.event(Event::Page).is_none());
        assert!(sounds.event(Event::Back).is_none());
    }

    #[test]
    fn systems_by_name_or_table() {
        let (dir, sounds) = theme(
            &[
                "sounds/systems/snes.ogg",
                "sounds/systems/nes.wav",
                "sounds/other.ogg",
            ],
            "[sounds.systems]\ngba = \"sounds/other.ogg\"\nnes = \"\"",
        );
        assert_eq!(
            sounds.system("snes"),
            Some(dir.path().join("sounds/systems/snes.ogg").as_path())
        );
        assert!(sounds.system("nes").is_none());
        assert!(sounds.system("gba").is_some());
        assert!(sounds.system("psx").is_none());
        assert_eq!(sounds.files().len(), 2);
    }

    #[test]
    fn oversized_files_are_ignored() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("sounds")).unwrap();
        fs::write(
            dir.path().join("sounds/move.ogg"),
            vec![0; MAX_SOUND_BYTES as usize + 1],
        )
        .unwrap();
        assert!(Sounds::load(dir.path()).event(Event::Move).is_none());
    }
}
