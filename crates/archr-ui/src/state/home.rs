//! The shelves (`Screen::Home`, docs/temas.md, Prateleiras): tabs of
//! shelves the launcher opens on when the view declares `home`. ↑/↓ change
//! shelf, ←/→ walk along one, L1/R1 change tab; A opens or plays, Y opens
//! the game menu, B does nothing here (screens opened from here come back).
//! ↑ from the first shelf reaches the showcase's buttons (Continue,
//! Favorite, More): ←/→ along them, A presses one, ↓ goes back.
use super::{App, Button, DataSource, Outcome, Screen};
use crate::layout::{ShelfKind, Tab};
use std::path::PathBuf;

/// Something on a shelf.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// A recent game: its index in `App::recent`.
    Recent(usize),
    /// A systems row (a real system or a collection).
    System(String),
    /// A game by its real system and index.
    Game { system: String, game: usize },
    /// A module's app row (`@app:<id>`).
    App(String),
}

/// A shelf as the tab shows it: what kind it is, the system it gathers
/// (the Favorites tab's), and its items, never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shelf {
    pub kind: ShelfKind,
    pub system: Option<String>,
    pub items: Vec<Item>,
}

impl App {
    /// The screen the launcher rests on: the shelves when the view has them.
    pub(crate) fn root(&self) -> Screen {
        if self.home.is_some() {
            Screen::Home {
                tab: Tab::Home,
                shelf: 0,
                item: 0,
            }
        } else {
            Screen::Systems
        }
    }
    /// On the systems screen, or on the shelves.
    pub fn at_root(&self) -> bool {
        matches!(self.screen, Screen::Systems | Screen::Home { .. })
    }
    /// Back to the screen the launcher rests on, forgetting the way here.
    pub fn go_root(&mut self) {
        self.screen = self.root();
        self.cursor = 0;
        self.home_hero = None;
        self.history.clear();
    }
    /// The showcase's buttons for `focused`, as the theme lists them
    /// (`play`, `favorite`, `more`): a game has them all, a system or an
    /// app only the first, which opens it.
    pub fn hero_actions(&self, focused: &Item) -> Vec<String> {
        let Some(home) = &self.home else {
            return Vec::new();
        };
        let game = matches!(focused, Item::Recent(_) | Item::Game { .. });
        home.hero
            .actions
            .iter()
            .filter(|a| game || *a == "play")
            .cloned()
            .collect()
    }

    /// The real systems and the Modules row, in the user's order.
    fn home_systems(data: &dyn DataSource) -> Vec<String> {
        Self::system_rows(data)
            .into_iter()
            .filter(|row| crate::modules::among_systems(row))
            .collect()
    }
    /// The favorite games by their real system and index.
    fn home_favorites(data: &dyn DataSource) -> Vec<(String, usize)> {
        (0..data.games("@favorites").len())
            .filter_map(|n| data.origin("@favorites", n))
            .collect()
    }

    /// The shelves of `tab`, empty ones left out. The Library tab is one
    /// shelf: every system, laid out as a grid.
    pub fn shelves(&self, tab: Tab, data: &dyn DataSource) -> Vec<Shelf> {
        let Some(home) = &self.home else {
            return Vec::new();
        };
        let shelf = |kind, system, items: Vec<Item>| Shelf {
            kind,
            system,
            items,
        };
        let shelves = match tab {
            Tab::Home => home
                .order
                .iter()
                .map(|kind| {
                    let items = match kind {
                        ShelfKind::Continue => (0..self.recent.len()).map(Item::Recent).collect(),
                        ShelfKind::Systems => Self::home_systems(data)
                            .into_iter()
                            .map(Item::System)
                            .collect(),
                        ShelfKind::Favorites => Self::home_favorites(data)
                            .into_iter()
                            .map(|(system, game)| Item::Game { system, game })
                            .collect(),
                        ShelfKind::Apps => data
                            .apps()
                            .iter()
                            .map(|app| Item::App(app.key.clone()))
                            .collect(),
                        ShelfKind::Collections => Self::system_rows(data)
                            .into_iter()
                            .filter(|row| {
                                !crate::modules::among_systems(row)
                                    && crate::apps::module_of(row).is_none()
                            })
                            .map(Item::System)
                            .collect(),
                    };
                    shelf(*kind, None, items)
                })
                .collect(),
            Tab::Library => vec![shelf(
                ShelfKind::Systems,
                None,
                Self::home_systems(data)
                    .into_iter()
                    .map(Item::System)
                    .collect(),
            )],
            // One shelf per system, in the systems' order.
            Tab::Favorites => {
                let favorites = Self::home_favorites(data);
                Self::home_systems(data)
                    .into_iter()
                    .map(|system| {
                        let items = favorites
                            .iter()
                            .filter(|(s, _)| *s == system)
                            .map(|(s, game)| Item::Game {
                                system: s.clone(),
                                game: *game,
                            })
                            .collect();
                        shelf(ShelfKind::Favorites, Some(system), items)
                    })
                    .collect()
            }
        };
        shelves
            .into_iter()
            .filter(|s: &Shelf| !s.items.is_empty())
            .collect()
    }

    /// The focused item of the shelves, if any.
    pub fn home_focus(&self, data: &dyn DataSource) -> Option<Item> {
        let Screen::Home { tab, shelf, item } = self.backdrop().0 else {
            return None;
        };
        let shelves = self.shelves(*tab, data);
        let shelf = shelves.get(*shelf)?;
        shelf.items.get(*item).cloned()
    }

    /// Columns of the games screen when the view lays it out as a grid.
    pub fn games_grid(&self) -> Option<usize> {
        self.games_grid
    }
    /// Whether the games screen is a free screen, which loads its own
    /// pictures.
    pub fn games_free(&self) -> bool {
        self.games_free
    }
    /// Columns of the Library tab's grid.
    fn library_columns(&self) -> usize {
        self.home.as_ref().map_or(3, |h| h.library.columns.max(1))
    }

    /// `raw` is the button as pressed: ← and → walk a shelf, L1 and R1
    /// change tab.
    pub(super) fn handle_home(&mut self, raw: Button, data: &dyn DataSource) -> Outcome {
        let Screen::Home {
            mut tab,
            mut shelf,
            mut item,
        } = self.screen.clone()
        else {
            return Outcome::None;
        };
        let tabs = self.home.as_ref().map_or_else(Vec::new, |h| h.tabs.clone());
        let shelves = self.shelves(tab, data);
        shelf = shelf.min(shelves.len().saturating_sub(1));
        let len = |shelf: usize| shelves.get(shelf).map_or(0, |s| s.items.len());
        item = item.min(len(shelf).saturating_sub(1));
        // The Library tab is a grid: ↑/↓ go a row.
        let grid = tab == Tab::Library;
        let columns = self.library_columns();
        let mut outcome = Outcome::None;
        let focused = shelves.get(shelf).and_then(|s| s.items.get(item)).cloned();
        // On the showcase's buttons: ← and → go along them, ↓ (or B) goes
        // back to the shelf, A does what the button says; X and Y as on
        // the item.
        if let Some(button) = self.home_hero.filter(|_| tab == Tab::Home) {
            let actions = focused
                .as_ref()
                .map_or_else(Vec::new, |f| self.hero_actions(f));
            let last = actions.len().saturating_sub(1);
            let button = button.min(last);
            match raw {
                Button::Left => self.home_hero = Some(button.saturating_sub(1)),
                Button::Right => self.home_hero = Some((button + 1).min(last)),
                Button::Down | Button::B => self.home_hero = None,
                Button::A | Button::X | Button::Y => {
                    let press = match (raw, actions.get(button).map(String::as_str)) {
                        (Button::A, Some("favorite")) => Button::X,
                        (Button::A, Some("more")) => Button::Y,
                        (pressed, _) => pressed,
                    };
                    if let Some(focused) = focused.clone() {
                        outcome = self.home_action(press, focused, data);
                    }
                    if !matches!(self.screen, Screen::Home { .. }) {
                        if let Some((Screen::Home { .. }, _)) = self.history.last_mut() {
                            let last = self.history.len() - 1;
                            self.history[last].0 = Screen::Home { tab, shelf, item };
                        }
                        return outcome;
                    }
                }
                // Another tab starts on its shelves.
                Button::L | Button::R => self.home_hero = None,
                _ => {}
            }
            if !matches!(raw, Button::L | Button::R) {
                self.screen = Screen::Home { tab, shelf, item };
                return outcome;
            }
        }
        match raw {
            Button::Up if grid => item = item.saturating_sub(columns).min(item),
            Button::Down if grid => {
                if item + columns < len(shelf) {
                    item += columns;
                }
            }
            Button::Up => {
                if shelf > 0 {
                    shelf -= 1;
                    item = item.min(len(shelf).saturating_sub(1));
                } else if tab == Tab::Home
                    && focused
                        .as_ref()
                        .is_some_and(|f| !self.hero_actions(f).is_empty())
                {
                    // Up from the first shelf: the showcase's buttons.
                    self.home_hero = Some(0);
                }
            }
            Button::Down => {
                if shelf + 1 < shelves.len() {
                    shelf += 1;
                    item = item.min(len(shelf).saturating_sub(1));
                }
            }
            Button::Left => item = item.saturating_sub(1),
            Button::Right => item = (item + 1).min(len(shelf).saturating_sub(1)),
            Button::L | Button::R => {
                let at = tabs.iter().position(|t| *t == tab).unwrap_or(0);
                let next = if raw == Button::L {
                    at.checked_sub(1)
                } else {
                    Some(at + 1).filter(|n| *n < tabs.len())
                };
                if let Some(next) = next.and_then(|n| tabs.get(n)) {
                    (tab, shelf, item) = (*next, 0, 0);
                }
            }
            Button::A | Button::X | Button::Y => {
                if let Some(focused) = shelves.get(shelf).and_then(|s| s.items.get(item)) {
                    outcome = self.home_action(raw, focused.clone(), data);
                }
                // An action may have opened another screen: it keeps this one.
                if !matches!(self.screen, Screen::Home { .. }) {
                    if let Some((Screen::Home { .. }, _)) = self.history.last_mut() {
                        let last = self.history.len() - 1;
                        self.history[last].0 = Screen::Home { tab, shelf, item };
                    }
                    return outcome;
                }
            }
            _ => {}
        }
        self.screen = Screen::Home { tab, shelf, item };
        outcome
    }

    fn home_action(&mut self, b: Button, focused: Item, data: &dyn DataSource) -> Outcome {
        match (b, focused) {
            (Button::A, Item::Recent(n)) => match self.recent.get(n) {
                Some(game) => Self::resume(game),
                None => Outcome::None,
            },
            (Button::A, Item::System(system)) => {
                if system == crate::moonlight::SYSTEM {
                    return Outcome::Moonlight;
                }
                self.enter(Screen::games(system));
                Outcome::None
            }
            (Button::A, Item::Game { system, game }) => Outcome::Launch { system, game },
            (Button::A, Item::App(row)) => match crate::apps::module_of(&row) {
                Some(module) => {
                    Outcome::Module(crate::modules::Request::OpenApp(module.to_owned()))
                }
                None => Outcome::None,
            },
            (Button::X, Item::Game { system, game }) => Outcome::ToggleFavorite { system, game },
            (Button::X, Item::Recent(n)) => match self.recent.get(n) {
                Some(recent) => Outcome::ToggleFavorite {
                    system: recent.system.clone(),
                    game: recent.game,
                },
                None => Outcome::None,
            },
            (Button::Y, Item::Game { system, game }) => {
                self.open_game_menu(system, game, false, data);
                Outcome::None
            }
            (Button::Y, Item::Recent(n)) => {
                if let Some(recent) = self.recent.get(n).cloned() {
                    self.open_game_menu(recent.system, recent.game, false, data);
                    self.recent_menu = matches!(self.screen, Screen::QuickMenu { .. });
                }
                Outcome::None
            }
            _ => Outcome::None,
        }
    }

    /// Pictures the shelves show now: the covers of the games on screen and
    /// the pictures of the recent games.
    pub fn home_paths(&self, data: &dyn DataSource) -> (Vec<PathBuf>, Vec<PathBuf>) {
        let Screen::Home { tab, .. } = self.backdrop().0 else {
            return Default::default();
        };
        let mut covers = Vec::new();
        let mut pictures = Vec::new();
        for shelf in self.shelves(*tab, data) {
            for item in shelf.items.iter().take(12) {
                match item {
                    Item::Recent(n) => {
                        if let Some(path) = self.recent.get(*n).and_then(|r| r.picture.clone()) {
                            pictures.push(path);
                        }
                    }
                    Item::Game { system, game } => {
                        if let Some(path) =
                            data.games(system).get(*game).and_then(|g| g.image.clone())
                        {
                            covers.push(path);
                        }
                    }
                    _ => {}
                }
            }
        }
        (covers, pictures)
    }
}
