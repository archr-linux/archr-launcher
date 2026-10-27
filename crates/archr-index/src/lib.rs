//! System definitions and game indexing for the launcher.

pub mod systems;

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
}
