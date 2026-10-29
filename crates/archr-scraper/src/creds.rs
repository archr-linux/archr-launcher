//! The user's own ScreenScraper login, kept in the launcher store directory.
use archr_io::{publish_private, read_utf8, remove_durable, Symlinks::Reject};
use serde::{Deserialize, Serialize};
use std::{io, path::Path};

const FILE: &str = "scraper.toml";

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Credentials {
    pub user: String,
    pub password: String,
}

// Never derive Debug: the password must not reach a log by accident.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Credentials({}, <hidden>)", self.user)
    }
}

impl Credentials {
    /// `None` when nothing is stored or the file is unreadable. A symbolic
    /// link in its place is not followed.
    pub fn load(dir: &Path) -> Option<Self> {
        let source = read_utf8(&dir.join(FILE), 4096, Reject).ok()?;
        toml::from_str::<Self>(&source)
            .ok()
            .filter(|c| !c.user.is_empty() && !c.password.is_empty())
    }

    /// Atomic and durable (file and folder synced), mode 0600 before the
    /// file becomes visible.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let source = toml::to_string(self).map_err(io::Error::other)?;
        publish_private(&dir.join(FILE), source.as_bytes())
    }

    pub fn clear(dir: &Path) -> io::Result<()> {
        remove_durable(&dir.join(FILE))
    }
}
