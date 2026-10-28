//! Screen transitions and controller input, independent of rendering and SDL.

use archr_index::{cache::SystemEntry, Game};
mod hub;
mod order;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    A,
    B,
    X,
    Y,
    L,
    R,
    Up,
    Down,
    Start,
    Select,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Systems,
    StartHub {
        row: usize,
        tile: usize,
        shortcut: usize,
    },
    Games {
        system: String,
    },
    /// Game context is always resolved to its real system and index.
    QuickMenu {
        game: Option<(String, usize)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAction {
    Shutdown,
    Reboot,
    Suspend,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    ToggleWifi,
    Launch {
        system: String,
        game: usize,
    },
    ToggleFavorite {
        system: String,
        game: usize,
    },
    SetCore {
        system: String,
        game: usize,
        emulator: String,
        core: String,
    },
    Action(SessionAction),
    /// Replaces the saved system order; empty restores the default order.
    SetOrder(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickItem {
    Core,
    Move,
    RestoreOrder,
    Suspend,
    Reboot,
    Shutdown,
}

pub trait DataSource {
    fn systems(&self) -> &[SystemEntry];
    /// Accepts real system names and the @favorites / @recent collections.
    fn games(&self, system: &str) -> &[Game];
    /// Collection presence/count need not load or resolve any game metadata.
    fn collection_len(&self, collection: &str) -> usize {
        self.games(collection).len()
    }
    /// Resolves a displayed row to its real system and game index, including
    /// identity mappings for real systems. None makes game actions no-ops.
    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)>;
    /// Manufacturer and release year from es_systems.cfg; either may be empty.
    fn manufacturer(&self, _system: &str) -> Option<(&str, &str)> {
        None
    }
    fn favorite_count(&self, _system: &str) -> usize {
        0
    }
    /// Saved system ids in user order; empty while the default order applies.
    fn system_order(&self) -> &[String] {
        &[]
    }
    fn cores(&self, system: &str) -> Vec<(String, String)>;
    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)>;
}

pub struct App {
    pub screen: Screen,
    pub cursor: usize,
    pub page_size: usize,
    history: Vec<(Screen, usize)>,
    moving: Option<order::Moving>,
    selected_core: Option<(String, String)>,
}

impl App {
    pub fn new(page_size: usize) -> Self {
        Self {
            screen: Screen::Systems,
            cursor: 0,
            page_size,
            history: Vec::new(),
            moving: None,
            selected_core: None,
        }
    }

    /// The undimmed screen beneath a modal, including collection identity.
    pub fn backdrop(&self) -> (&Screen, usize) {
        if matches!(self.screen, Screen::QuickMenu { .. }) {
            if let Some((screen, cursor)) = self.history.last() {
                return (screen, *cursor);
            }
        }
        (&self.screen, self.cursor)
    }

    pub fn quick_menu_items(&self, data: &dyn DataSource) -> &'static [QuickItem] {
        use QuickItem::*;
        const GAME: &[QuickItem] = &[Core, Suspend, Reboot, Shutdown];
        match &self.screen {
            Screen::QuickMenu {
                game: Some((system, _)),
            } => &GAME[usize::from(data.cores(system).len() <= 1)..],
            Screen::QuickMenu { game: None } if data.system_order().is_empty() => {
                &[Move, Suspend, Reboot, Shutdown]
            }
            Screen::QuickMenu { game: None } => &[Move, RestoreOrder, Suspend, Reboot, Shutdown],
            _ => &[],
        }
    }
    pub fn selected_core(&self) -> (&str, &str) {
        self.selected_core
            .as_ref()
            .map_or(("", ""), |(emulator, core)| (emulator, core))
    }

    /// Returns effects for the caller to apply. Core selection is tracked locally
    /// while the quick menu is open, independently of persistence timing.
    pub fn handle(&mut self, b: Button, data: &dyn DataSource) -> Outcome {
        if self.moving.is_some() {
            return self.handle_move(b);
        }
        if matches!(self.screen, Screen::Systems) && b == Button::Start {
            self.screen = Screen::StartHub {
                row: 0,
                tile: usize::from(!self.hub_tile_enabled(data, 0)),
                shortcut: 0,
            };
            return Outcome::None;
        }
        if matches!(self.screen, Screen::StartHub { .. }) {
            return self.handle_hub(b, data);
        }
        let len = match &self.screen {
            Screen::StartHub { .. } => unreachable!(),
            Screen::Systems => Self::system_rows(data).len(),
            Screen::Games { system } => data.games(system).len(),
            Screen::QuickMenu { .. } => self.quick_menu_items(data).len(),
        };
        let last = len.saturating_sub(1);
        self.cursor = self.cursor.min(last);

        match b {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = self.cursor.saturating_add(1).min(last),
            Button::L | Button::R if !matches!(self.screen, Screen::QuickMenu { .. }) => {
                self.cursor = if b == Button::L {
                    self.cursor.saturating_sub(self.page_size)
                } else {
                    self.cursor.saturating_add(self.page_size).min(last)
                };
            }
            _ => return self.handle_screen(b, data),
        }
        Outcome::None
    }

    fn handle_screen(&mut self, b: Button, data: &dyn DataSource) -> Outcome {
        match self.screen.clone() {
            Screen::Systems => match b {
                Button::A => {
                    if let Some(system) = Self::system_rows(data).get(self.cursor) {
                        self.enter(Screen::Games {
                            system: system.clone(),
                        });
                    }
                }
                Button::Y => self.enter(Screen::QuickMenu { game: None }),
                _ => {}
            },
            Screen::StartHub { .. } => unreachable!(),
            Screen::Games { system } => match b {
                Button::B => self.back(),
                Button::A | Button::X | Button::Y => {
                    if self.cursor >= data.games(&system).len() {
                        return Outcome::None;
                    }
                    let Some((system, game)) = data.origin(&system, self.cursor) else {
                        return Outcome::None;
                    };
                    match b {
                        Button::A => return Outcome::Launch { system, game },
                        Button::X => return Outcome::ToggleFavorite { system, game },
                        Button::Y => {
                            self.selected_core = data.current_core(&system, game);
                            self.enter(Screen::QuickMenu {
                                game: Some((system, game)),
                            });
                        }
                        _ => unreachable!(),
                    }
                }
                _ => {}
            },
            Screen::QuickMenu { game } => {
                let items = self.quick_menu_items(data);
                match (b, items.get(self.cursor)) {
                    (Button::B, _) => self.back(),
                    (Button::A, Some(QuickItem::Move)) => {
                        self.back();
                        self.begin_move(data);
                    }
                    (Button::A, Some(QuickItem::RestoreOrder)) => {
                        self.back();
                        return self.restore_order(data);
                    }
                    (Button::A, Some(QuickItem::Suspend)) => {
                        return Outcome::Action(SessionAction::Suspend);
                    }
                    (Button::A, Some(QuickItem::Reboot)) => {
                        return Outcome::Action(SessionAction::Reboot);
                    }
                    (Button::A, Some(QuickItem::Shutdown)) => {
                        return Outcome::Action(SessionAction::Shutdown);
                    }
                    (Button::L | Button::R, Some(QuickItem::Core)) => {
                        if let Some((system, game)) = game {
                            return self.cycle_core(b, system, game, data);
                        }
                    }
                    _ => {}
                }
            }
        }
        Outcome::None
    }

    fn cycle_core(
        &mut self,
        b: Button,
        system: String,
        game: usize,
        data: &dyn DataSource,
    ) -> Outcome {
        let cores = data.cores(&system);
        if cores.is_empty() {
            return Outcome::None;
        }
        let index = cores
            .iter()
            .position(|pair| Some(pair) == self.selected_core.as_ref());
        let next = match (b, index) {
            (Button::R, Some(index)) => (index + 1) % cores.len(),
            (Button::L, Some(0)) | (Button::L, None) => cores.len() - 1,
            (Button::L, Some(index)) => index - 1,
            _ => 0,
        };
        self.selected_core = Some(cores[next].clone());
        let (emulator, core) = cores[next].clone();
        Outcome::SetCore {
            system,
            game,
            emulator,
            core,
        }
    }

    fn enter(&mut self, screen: Screen) {
        let previous = std::mem::replace(&mut self.screen, screen);
        self.history.push((previous, self.cursor));
        self.cursor = 0;
    }

    fn back(&mut self) {
        let (screen, cursor) = self.history.pop().unwrap_or((Screen::Systems, 0));
        self.screen = screen;
        self.cursor = cursor;
    }
}
