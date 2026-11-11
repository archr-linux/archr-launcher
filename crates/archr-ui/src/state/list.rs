//! A system's list by folder and its search, over the real list (`folders`).
use super::{App, Button, DataSource, Outcome, Screen, Search};
use crate::folders::{self, Level};
use crate::keyboard::{Keyboard, Mode};
use std::path::PathBuf;

/// Rows L and R jump in the search results.
const SEARCH_PAGE: usize = 5;

impl App {
    /// The folder `folder` of `system`'s list, when the user lists it by folder.
    pub fn level(data: &dyn DataSource, system: &str, folder: &str) -> Option<Level> {
        (!system.starts_with('@') && data.shows_folders(system))
            .then(|| Level::of(&data.folders(system), folder))
    }
    /// The game a list's row shows, as an index into `data.games(system)`;
    /// None on a folder.
    pub fn list_game(
        data: &dyn DataSource,
        system: &str,
        folder: &str,
        row: usize,
    ) -> Option<usize> {
        match Self::level(data, system, folder) {
            Some(level) => level.game(row),
            None => (row < data.games(system).len()).then_some(row),
        }
    }
    pub fn list_len(data: &dyn DataSource, system: &str, folder: &str) -> usize {
        Self::level(data, system, folder).map_or(data.games(system).len(), |l| l.len())
    }
    /// The covers of a folder's mosaic: its first games that have one.
    pub fn folder_covers(data: &dyn DataSource, system: &str, folder: &str) -> Vec<PathBuf> {
        let (dirs, games) = (data.folders(system), data.games(system));
        folders::within(&dirs, folder)
            .filter_map(|n| games.get(n)?.image.clone())
            .take(4)
            .collect()
    }
    /// The real system whose list a menu was opened on.
    pub fn list_system(&self) -> Option<&str> {
        match self.history.last() {
            Some((Screen::Games { system, .. }, _)) if !system.starts_with('@') => Some(system),
            _ => None,
        }
    }
    /// From the list's root (the screen now), the cursor on `game`: in its
    /// folder, entered from the root down, when `by_folder`.
    pub(super) fn select(
        &mut self,
        data: &dyn DataSource,
        system: &str,
        game: usize,
        by_folder: bool,
    ) {
        let dirs = data.folders(system);
        let Some(dir) = dirs.get(game).filter(|_| by_folder) else {
            self.cursor = game;
            return;
        };
        let mut at = String::new();
        for name in dir.split('/').filter(|n| !n.is_empty()) {
            self.cursor = Level::of(&dirs, &at).folder_row(name).unwrap_or(0);
            at = folders::join(&at, name);
            self.enter(Screen::Games {
                system: system.to_owned(),
                folder: at.clone(),
            });
        }
        self.cursor = Level::of(&dirs, &at).game_row(game).unwrap_or(0);
    }
    /// The switch of the list's menu: the list again from its root, the
    /// cursor on the game it was on.
    pub(super) fn toggle_folders(&mut self, data: &dyn DataSource) -> Outcome {
        let Some(system) = self.list_system().map(str::to_owned) else {
            return Outcome::None;
        };
        let on = !data.shows_folders(&system);
        let game = match self.history.last() {
            Some((Screen::Games { folder, .. }, row)) => {
                Self::list_game(data, &system, folder, *row).unwrap_or(0)
            }
            _ => 0,
        };
        self.back();
        while matches!(&self.screen, Screen::Games { folder, .. } if !folder.is_empty()) {
            self.back();
        }
        self.select(data, &system, game, on);
        Outcome::ShowFolders { system, on }
    }
    pub(super) fn open_search(&mut self, system: String) {
        // Its submit key goes to the results, the whole list when empty.
        let mut keyboard = Keyboard::new(Mode::Text, String::new());
        keyboard.optional = true;
        self.search = Some(Search {
            keyboard,
            results: false,
        });
        self.enter(Screen::Search { system });
    }
    /// The games found, as indices into the system's list.
    pub fn search_results(&self, data: &dyn DataSource, system: &str) -> Vec<usize> {
        let query = self.search.as_ref().map_or("", |s| s.keyboard.text());
        folders::search(data.games(system).iter().map(|g| g.name.as_str()), query)
    }
    /// The keyboard types, Start goes to the results, B closes; on the
    /// results, A plays, X favorites, Y opens the game menu, B types again.
    pub(super) fn handle_search(
        &mut self,
        b: Button,
        system: String,
        data: &dyn DataSource,
    ) -> Outcome {
        let found = self.search_results(data, &system);
        let Some(search) = self.search.as_mut() else {
            self.back();
            return Outcome::None;
        };
        if !search.results {
            match b {
                Button::B => {
                    self.search = None;
                    self.back();
                }
                Button::Start => search.results = !found.is_empty(),
                _ => {
                    let before = search.keyboard.text().len();
                    let submitted = search.keyboard.handle(b).is_some();
                    if search.keyboard.text().len() != before {
                        self.cursor = 0;
                    }
                    let found = !self.search_results(data, &system).is_empty();
                    if let Some(search) = self.search.as_mut() {
                        search.results = submitted && found;
                    }
                }
            }
            return Outcome::None;
        }
        let last = found.len().saturating_sub(1);
        self.cursor = match b {
            Button::B => {
                search.results = false;
                return Outcome::None;
            }
            Button::Up => self.cursor.saturating_sub(1),
            Button::Down => self.cursor + 1,
            Button::L => self.cursor.saturating_sub(SEARCH_PAGE),
            Button::R => self.cursor + SEARCH_PAGE,
            _ => self.cursor,
        }
        .min(last);
        let Some((system, game)) = found
            .get(self.cursor)
            .and_then(|&n| data.origin(&system, n))
        else {
            return Outcome::None;
        };
        match b {
            Button::A => Outcome::Launch { system, game },
            Button::X => Outcome::ToggleFavorite { system, game },
            Button::Y => {
                self.open_game_menu(system, game, false, data);
                Outcome::None
            }
            _ => Outcome::None,
        }
    }
}
