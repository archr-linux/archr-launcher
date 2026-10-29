//! Resolution-independent launcher presentation types.

#[cfg(test)]
static SDL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub mod appearance;
pub mod cheevos;
pub mod i18n;
pub mod layout;
pub mod sounds;
pub mod theme;

pub use theme::{resolve_theme, theme_dir, Rect01, Rgba, SystemScreen, Theme, DEFAULT_THEME_DIR};

pub mod covers;

pub mod input;
pub mod pad;
pub mod state;

pub mod controllers;
pub mod netplay;

pub mod render;

mod badges;
mod icons;
pub use icons::DEFAULT_ICONS_DIR;
pub mod motion;
mod primitives;
pub mod startup;
pub mod status;
mod text;

pub mod keyboard;
pub mod moonlight;
pub mod settings;

/// A footer action may be bound to one button or a grouped pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintButton {
    Key(char),
    Pair(char, char),
    Start,
}
impl From<char> for HintButton {
    fn from(key: char) -> Self {
        Self::Key(key)
    }
}
