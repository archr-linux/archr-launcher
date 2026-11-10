//! Resolution-independent launcher presentation types.

#[cfg(test)]
static SDL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub mod appearance;
pub mod cheevos;
pub mod i18n;
pub mod layout;
pub mod sounds;
pub mod theme;
pub mod usb;

pub use theme::{
    resolve_theme, theme_dir, Aspect, Frame9, Outline, Rect01, Rgba, ShadowStyle, Shape,
    SystemScreen, Theme, DEFAULT_THEME_DIR,
};

pub mod convert;
pub mod covers;

pub mod input;
pub mod pad;
pub mod state;

pub mod controllers;
pub mod netplay;

pub mod render;
pub mod resume;
pub mod saves;

mod badges;
mod icons;
pub use icons::DEFAULT_ICONS_DIR;
pub mod motion;
mod primitives;
pub mod startup;
pub mod status;
mod text;

pub mod apps;
pub mod keyboard;
pub mod modules;
pub mod moonlight;
pub mod settings;
pub mod shortcuts;

/// A footer action may be bound to one button or a grouped pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintButton {
    Key(char),
    Pair(char, char),
    Start,
    Select,
}
impl From<char> for HintButton {
    fn from(key: char) -> Self {
        Self::Key(key)
    }
}
