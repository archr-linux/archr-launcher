//! Production paths, including complete isolation for PC fixtures.
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Paths {
    pub root: Option<PathBuf>,
    pub systems: PathBuf,
    pub config: PathBuf,
    pub lock: PathBuf,
    pub cache: PathBuf,
    pub store: PathBuf,
    pub run: PathBuf,
}
impl Paths {
    pub fn new(root: Option<PathBuf>) -> Self {
        let root = root.map(|root| {
            if root.is_absolute() {
                root
            } else {
                std::env::current_dir().unwrap_or_default().join(root)
            }
        });
        let at = |path: &str| prefix(root.as_deref(), Path::new(path));
        Self {
            systems: systems_catalog(&at),
            config: at("/storage/.config/system/configs/system.cfg"),
            lock: at("/tmp/.system.cfg.lock"),
            cache: at("/storage/.cache/archr-launcher"),
            store: at("/storage/.config/archr-launcher"),
            run: at("/run/archr-launcher"),
            root,
        }
    }
    pub fn at(&self, path: &str) -> PathBuf {
        prefix(self.root.as_deref(), Path::new(path))
    }
    pub fn remap(&self, path: &Path) -> PathBuf {
        prefix(self.root.as_deref(), path)
    }
}
/// ArchR systems catalogs: the user override, then the packaged one.
pub const CATALOGS: [&str; 2] = [
    "/storage/.config/archr/systems.cfg",
    "/usr/share/archr/systems.cfg",
];
/// Which launch options each emulator and core accepts (archr package).
pub const FEATURES: &str = "/usr/share/archr/es_features.cfg";
fn systems_catalog(at: &impl Fn(&str) -> PathBuf) -> PathBuf {
    CATALOGS
        .iter()
        .map(|path| at(path))
        .find(|path| path.is_file())
        .unwrap_or_else(|| at(CATALOGS[1]))
}
fn prefix(root: Option<&Path>, path: &Path) -> PathBuf {
    match root {
        Some(root) if path.is_absolute() && !path.starts_with(root) => {
            root.join(path.strip_prefix("/").unwrap_or(path))
        }
        _ => path.to_owned(),
    }
}
