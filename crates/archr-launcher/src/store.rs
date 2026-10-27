use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::file_io::{atomic_write, read_bounded};

const MAX_STORE_BYTES: u64 = 256 * 1024;
const MAX_RECENT: usize = 30;

/// Launcher-owned favorites and recents, keyed by `system/rom-path` verbatim.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Store {
    favorites: Vec<String>,
    recent: Vec<String>,
    imported_systems: Vec<String>,
    imported_times: HashMap<String, u64>,
}

impl Store {
    /// Unreadable or invalid data is nonfatal: start empty and report a warning.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("store.toml");
        let result = read_bounded(&path, MAX_STORE_BYTES).and_then(|source| {
            toml::from_str::<Self>(&source)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        });
        match result {
            Ok(mut store) => {
                for keys in [&mut store.favorites, &mut store.recent] {
                    let mut seen = HashSet::new();
                    keys.retain(|key| seen.insert(key.clone()));
                }
                store.recent.truncate(MAX_RECENT);
                store
            }
            Err(error) => {
                log::warn!(
                    "Cannot load store {}: {error}; using empty store",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let source = toml::to_string(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        atomic_write(dir, "store.toml", source.as_bytes())
    }

    pub fn toggle_favorite(&mut self, key: &str) {
        if self.is_favorite(key) {
            self.favorites.retain(|favorite| favorite != key);
        } else {
            self.favorites.push(key.to_owned());
        }
    }

    pub fn push_recent(&mut self, key: &str) {
        self.imported_times.remove(key);
        self.recent.retain(|recent| recent != key);
        self.recent.insert(0, key.to_owned());
        self.recent.truncate(MAX_RECENT);
    }

    /// Import ES metadata once per system, only after its games are loaded.
    /// Launcher history stays ahead of imported entries; ES entries retain UTC order.
    pub fn import_games(&mut self, system: &str, games: &[archr_index::Game]) -> bool {
        if self.imported_systems.iter().any(|name| name == system) {
            return false;
        }
        self.imported_systems.push(system.to_owned());
        for game in games {
            let Some(path) = game.path.to_str() else {
                continue;
            };
            let key = format!("{system}/{path}");
            if game.favorite && !self.is_favorite(&key) {
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

    pub fn is_favorite(&self, key: &str) -> bool {
        self.favorites.iter().any(|favorite| favorite == key)
    }

    pub fn favorites(&self) -> &[String] {
        &self.favorites
    }

    /// Most recent first, without duplicate keys.
    pub fn recent(&self) -> &[String] {
        &self.recent
    }
}
