use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::{self, Write},
    path::Path,
};

use archr_index::{Game, SystemDef};
use serde::{Deserialize, Serialize};

use archr_io::{atomic_publish, read_utf8, Symlinks::Follow};

const MAX_STORE_BYTES: u64 = 256 * 1024;
const MAX_RECENT: usize = 30;

/// Launcher-owned favorites and recents, keyed by `system/relative-rom-path`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Store {
    #[serde(skip)]
    pub(crate) load_error: Option<String>,
    favorites: Vec<String>,
    #[serde(skip)]
    members: HashSet<String>,
    recent: Vec<String>,
    /// System ids in user order; empty means the default order.
    order: Vec<String>,
    imported_systems: BTreeSet<String>,
    imported_times: HashMap<String, u64>,
}

impl Store {
    /// Invalid data is nonfatal to browsing, but disables saving this snapshot.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("store.toml");
        let result = read_utf8(&path, MAX_STORE_BYTES, Follow).and_then(|source| {
            toml::from_str::<Self>(&source)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        });
        match result {
            Ok(mut store) => {
                store.migrate(&[]);
                store
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!(
                    "Cannot load store {}: {error}; saving disabled",
                    path.display()
                );
                Self {
                    load_error: Some(error.to_string()),
                    ..Self::default()
                }
            }
        }
    }

    pub fn save(&self, dir: &Path) -> io::Result<()> {
        if let Some(error) = &self.load_error {
            return Err(archr_io::invalid_data(error));
        }
        let source = toml::to_string(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if source.len() as u64 > MAX_STORE_BYTES {
            return Err(archr_io::invalid_data("store exceeds 256 KiB"));
        }
        atomic_publish(&dir.join("store.toml"), true, |file| {
            file.write_all(source.as_bytes())
        })
    }

    pub fn toggle_favorite(&mut self, key: &str) {
        if self.members.remove(key) {
            self.favorites.retain(|favorite| favorite != key);
        } else {
            self.members.insert(key.to_owned());
            self.favorites.push(key.to_owned());
        }
    }

    pub fn push_recent(&mut self, key: &str) {
        self.imported_times.remove(key);
        self.recent.retain(|recent| recent != key);
        self.recent.insert(0, key.to_owned());
        self.recent.truncate(MAX_RECENT);
    }

    /// Convert only absolute paths below their configured system directory.
    /// Unknown systems/paths stay intact; no filesystem or symlink resolution.
    pub fn migrate(&mut self, systems: &[SystemDef]) -> bool {
        let mut changed = false;
        let mut normalize = |key: &mut String| {
            let relative = key.split_once('/').and_then(|(name, path)| {
                systems
                    .iter()
                    .find(|sys| sys.name == name)
                    .filter(|_| Path::new(path).is_absolute())
                    .and_then(|sys| game_key(sys, Path::new(path)))
            });
            if let Some(relative) = relative {
                changed |= *key != relative;
                *key = relative;
            }
        };
        for keys in [&mut self.favorites, &mut self.recent] {
            let mut seen = HashSet::new();
            keys.retain_mut(|key| {
                normalize(key);
                seen.insert(key.clone())
            });
        }
        let mut times = HashMap::<String, u64>::new();
        for (mut key, time) in self.imported_times.drain() {
            normalize(&mut key);
            let entry = times.entry(key).or_default();
            *entry = (*entry).max(time);
        }
        self.imported_times = times;
        self.recent.truncate(MAX_RECENT);
        self.members = self.favorites.iter().cloned().collect();
        changed
    }

    /// Import ES metadata once per system, only after its games are loaded.
    /// Launcher history stays ahead of imported entries; ES entries retain UTC order.
    pub fn import_games(&mut self, system: &SystemDef, games: &[Game]) -> bool {
        if !self.imported_systems.insert(system.name.clone()) {
            return false;
        }
        for game in games {
            let Some(key) = game_key(system, &game.path) else {
                continue;
            };
            if game.favorite && self.members.insert(key.clone()) {
                self.favorites.push(key.clone());
            }
            if let Some(time) = game.last_played {
                if !self.recent.contains(&key) {
                    self.recent.push(key.clone());
                    self.imported_times.insert(key, time);
                }
            }
        }
        self.recent.sort_by_key(|key| {
            std::cmp::Reverse(self.imported_times.get(key).copied().unwrap_or(u64::MAX))
        });
        self.recent.truncate(MAX_RECENT);
        self.imported_times
            .retain(|key, _| self.recent.contains(key));
        true
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }

    pub fn set_order(&mut self, order: Vec<String>) {
        self.order = order;
    }

    pub fn is_favorite(&self, key: &str) -> bool {
        self.members.contains(key)
    }

    pub fn favorites(&self) -> &[String] {
        &self.favorites
    }

    /// Most recent first, without duplicate keys.
    pub fn recent(&self) -> &[String] {
        &self.recent
    }
}

pub fn game_key(system: &SystemDef, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(&system.path).ok()?.to_str()?;
    Some(format!("{}/{relative}", system.name))
}
