//! Screen transitions and controller input, independent of rendering and SDL.

use archr_index::{cache::SystemEntry, Game};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickItem {
    Core,
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
    fn cores(&self, system: &str) -> Vec<(String, String)>;
    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)>;
}

pub struct App {
    pub screen: Screen,
    pub cursor: usize,
    pub page_size: usize,
    history: Vec<(Screen, usize)>,
    selected_core: Option<(String, String)>,
}

impl App {
    pub fn new(page_size: usize) -> Self {
        Self {
            screen: Screen::Systems,
            cursor: 0,
            page_size,
            history: Vec::new(),
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

    /// Shared row ordering for navigation and rendering.
    pub fn system_rows(data: &dyn DataSource) -> Vec<String> {
        ["@favorites", "@recent"]
            .into_iter()
            .filter(|name| data.collection_len(name) > 0)
            .map(str::to_owned)
            .chain(
                data.systems()
                    .iter()
                    .filter(|system| system.game_count > 0)
                    .map(|system| system.name.clone()),
            )
            .collect()
    }

    pub fn quick_menu_items(&self, data: &dyn DataSource) -> &'static [QuickItem] {
        const ITEMS: &[QuickItem] = &[
            QuickItem::Core,
            QuickItem::Suspend,
            QuickItem::Reboot,
            QuickItem::Shutdown,
        ];
        match &self.screen {
            Screen::QuickMenu { game } => {
                let core = game
                    .as_ref()
                    .is_some_and(|(system, _)| data.cores(system).len() > 1);
                &ITEMS[usize::from(!core)..]
            }
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
        let len = match &self.screen {
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
