//! Resolution-independent launcher presentation types.

pub mod i18n;
pub mod theme;

pub use theme::{resolve_theme, Rect01, Rgba, SystemScreen, Theme};

pub mod covers;

pub mod input;
pub mod state;

pub mod controllers;

pub mod render;

pub mod joystick_mapping;

mod badges;
mod icons;
pub mod motion;
mod primitives;
mod raster_cache;
pub mod startup;
pub mod status;
mod text;

pub mod settings;

/// A footer action may be bound to one button or a grouped pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintButton {
    Key(char),
    Pair(char, char),
}
impl From<char> for HintButton {
    fn from(key: char) -> Self {
        Self::Key(key)
    }
}
