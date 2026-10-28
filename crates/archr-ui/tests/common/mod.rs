use archr_index::{cache::SystemEntry, Game};
use archr_ui::state::DataSource;
use std::collections::HashMap;

#[derive(Default)]
pub struct FakeData {
    pub systems: Vec<SystemEntry>,
    pub games: HashMap<String, Vec<Game>>,
    pub origins: HashMap<(String, usize), (String, usize)>,
    pub cores: HashMap<String, Vec<(String, String)>>,
    pub counts: HashMap<String, usize>,
    pub current: HashMap<(String, usize), (String, String)>,
    pub order: Vec<String>,
}

impl DataSource for FakeData {
    fn system_order(&self) -> &[String] {
        &self.order
    }

    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }

    fn games(&self, system: &str) -> &[Game] {
        self.games.get(system).map(Vec::as_slice).unwrap_or(&[])
    }

    fn collection_len(&self, name: &str) -> usize {
        self.counts
            .get(name)
            .copied()
            .unwrap_or_else(|| self.games(name).len())
    }

    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        if system.starts_with('@') {
            self.origins.get(&(system.into(), game)).cloned()
        } else {
            self.games(system).get(game).map(|_| (system.into(), game))
        }
    }

    fn cores(&self, system: &str) -> Vec<(String, String)> {
        self.cores.get(system).cloned().unwrap_or_default()
    }

    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)> {
        self.current.get(&(system.into(), game)).cloned()
    }
}
