//! RetroAchievements without a personal web API key: sign-in, the user's
//! points and a game's achievements through the same `dorequest.php` calls
//! RetroArch makes with the user's token, and the rcheevos game hash.
//! Everything here blocks: call it from a worker.
mod api;
mod hash;

pub use api::{Account, Achievement, Client, Error, Game, HOSTS};
pub use hash::{rom_hash, supported};
