//! A game's RetroAchievements as the host fetched them (RA-jogo).
use std::path::PathBuf;

/// Where the shown list came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fetch {
    #[default]
    Loading,
    /// Just fetched from the service.
    Fresh,
    /// The service did not answer; this is the copy saved last time.
    Saved,
    /// Nothing to show; a locale key says why.
    Failed(&'static str),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub title: String,
    pub description: String,
    pub points: u64,
    pub unlocked: bool,
    /// Badge image on disk, once downloaded.
    pub badge: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sheet {
    pub fetch: Fetch,
    /// Unlocks counted are the hardcore ones.
    pub hardcore: bool,
    /// Unlocked first, then locked, each in the set's own order.
    pub items: Vec<Item>,
}

impl Sheet {
    /// Unlocked count, points earned and points available.
    pub fn progress(&self) -> (usize, u64, u64) {
        self.items.iter().fold((0, 0, 0), |(n, got, all), item| {
            if item.unlocked {
                (n + 1, got + item.points, all + item.points)
            } else {
                (n, got, all + item.points)
            }
        })
    }
}
