use std::{collections::HashSet, io, path::Path};

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
        self.recent.retain(|recent| recent != key);
        self.recent.insert(0, key.to_owned());
        self.recent.truncate(MAX_RECENT);
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
