//! Resolution-independent launcher presentation types.

#[cfg(test)]
static SDL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub mod i18n;
pub mod theme;

pub use theme::{resolve_theme, theme_dir, Rect01, Rgba, SystemScreen, Theme};

pub mod covers;

pub mod input;
pub mod state;

pub mod controllers;

pub mod render;

pub mod joystick_mapping;

mod badges;
pub mod motion;
mod primitives;
pub mod startup;
pub mod status;
mod text;
