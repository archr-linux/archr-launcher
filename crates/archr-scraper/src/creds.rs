//! The user's own ScreenScraper login, kept in the launcher store directory.
use archr_io::{atomic_publish, read_utf8, Symlinks::Follow};
use serde::{Deserialize, Serialize};
use std::{fs, io, io::Write, os::unix::fs::PermissionsExt, path::Path};

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
    /// `None` when nothing is stored or the file is unreadable.
    pub fn load(dir: &Path) -> Option<Self> {
        let source = read_utf8(&dir.join(FILE), 4096, Follow).ok()?;
        toml::from_str::<Self>(&source)
            .ok()
            .filter(|c| !c.user.is_empty() && !c.password.is_empty())
    }

    /// Atomic, mode 0600 before the file becomes visible.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let source = toml::to_string(self).map_err(io::Error::other)?;
        atomic_publish(&dir.join(FILE), true, |file| {
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file.write_all(source.as_bytes())
        })
    }

    pub fn clear(dir: &Path) -> io::Result<()> {
        match fs::remove_file(dir.join(FILE)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}
