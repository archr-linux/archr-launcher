//! ScreenScraper (API v2) game-art scraper. Pure engine, no UI.
//!
//! Usage from the settings page (nothing here blocks; all work is on a worker):
//! ```ignore
//! let Some(engine) = Engine::production(paths.cache.clone(), Credentials::load(&paths.store))
//!     else { /* feature unavailable: keep the page disabled */ };
//! let job = engine.start(systems, Options::new(&language));
//! // every frame:  while let Some(event) = job.poll() { ... }
//! // B button:     job.cancel();
//! ```
//! `available()` says whether this build carries the developer login.
//! `Engine::verify()` (blocking, call it from a worker) tests the user's login.
mod api;
mod creds;
mod engine;
mod hash;
mod http;
mod systems;
mod validate;

pub use creds::Credentials;
pub use engine::{
    available, Engine, Event, Job, Media, Options, Progress, Report, Stop, Timing, UserInfo,
};
pub use http::{trusted, Curl, Http, NetError, Response};
pub use systems::system_id;
