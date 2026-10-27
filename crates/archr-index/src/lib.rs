//! System definitions and game indexing for the launcher.

mod gamelist;
pub mod games;
pub mod systems;

pub use games::{clean_name, scan_system, Game};

pub use systems::{load_systems, Core, Emulator, SystemDef};

/// Errors while reading launcher index inputs.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("index I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid systems XML: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("invalid system list: {0}")]
    InvalidSystems(&'static str),
    #[error("invalid game list: {0}")]
    InvalidGamelist(&'static str),
}
