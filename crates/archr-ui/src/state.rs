//! Screen transitions and controller input, independent of rendering and SDL.

use crate::layout::{Axis, Layout, View};
use crate::modules::{Call, Context};
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
    /// Scraped text and art of one game, by real system and index.
    Details {
        system: String,
        game: usize,
    },
    /// The game's RetroAchievements (`App::sheet`), by real system and index.
    Achievements {
        system: String,
        game: usize,
    },
}

/// Buttons of the details screen, left to right.
pub const DETAILS_ACTIONS: [&str; 3] = ["hint.play", "hint.favorite", "details.rescrape"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAction {
    Shutdown,
    Reboot,
    Suspend,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Settings,
    /// Opens Settings, System, Updates.
    Updates,
    /// Opens the Netplay lobby.
    OpenNetplay,
    /// Starts a lobby room as a netplay client.
    Netplay(Box<crate::netplay::Join>),
    ToggleWifi,
    Moonlight,
    MoonlightStream(crate::moonlight::Stream),
    Launch {
        system: String,
        game: usize,
    },
    ToggleFavorite {
        system: String,
        game: usize,
    },
    /// Scrape this one game again, replacing what an earlier scrape wrote.
    Rescrape {
        system: String,
        game: usize,
    },
    /// Fetch this game's achievements into `App::sheet`.
    Achievements {
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
    /// Something for the module host, or a module page to open.
    Module(crate::modules::Request),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickItem {
    Details,
    Achievements,
    Settings,
    Netplay,
    Core,
    Move,
    RestoreOrder,
    Suspend,
    Reboot,
    Shutdown,
    /// A module's item: an index into `View::quick` for this menu.
    Module(u8),
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
    /// Manufacturer and release year from the systems catalog; either may be empty.
    fn manufacturer(&self, _system: &str) -> Option<(&str, &str)> {
        None
    }
    fn favorite_count(&self, _system: &str) -> usize {
        0
    }
    /// A system launched by its catalog command, without emulators: its
    /// entries are tools, not games.
    fn is_tools(&self, _system: &str) -> bool {
        false
    }
    /// Saved system ids in user order; empty while the default order applies.
    fn system_order(&self) -> &[String] {
        &[]
    }
    fn cores(&self, system: &str) -> Vec<(String, String)>;
    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)>;
    /// `system/relative/path.rom`, the key favorites and modules use.
    fn game_key(&self, _system: &str, _game: usize) -> Option<String> {
        None
    }
}

/// How a screen's list answers the shoulders and the d-pad, which send the
/// same L/R: a vertical list jumps a page; a horizontal one (the carousel)
/// steps one entry, so ← and → reach the neighbours. ↑/↓ step one entry in
/// both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nav {
    pub axis: Axis,
    /// Entries the list shows at once.
    pub page: usize,
}
impl Nav {
    pub fn vertical(page: usize) -> Self {
        Self {
            axis: Axis::Vertical,
            page,
        }
    }
    /// From the screen's layout in the active view.
    pub fn of(layout: &Layout) -> Self {
        let list = &layout.list;
        let page = (list.rect.h / (list.row + list.gap)).floor().max(1.) as usize;
        Self {
            axis: list.axis,
            page,
        }
    }
    /// How far L or R moves the cursor.
    fn step(self) -> usize {
        match self.axis {
            Axis::Vertical => self.page,
            Axis::Horizontal => 1,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub cursor: usize,
    /// List navigation of the systems and the games screens.
    pub systems_nav: Nav,
    pub games_nav: Nav,
    /// `global.netplay` is on: the quick menu offers the lobby.
    pub netplay: bool,
    /// Details screen: focused button and description scroll, in lines.
    pub details_focus: usize,
    pub details_scroll: usize,
    /// Lines the description can scroll, reported by the renderer.
    pub details_overflow: usize,
    /// The host can scrape a single game now (credential present, no run going).
    pub can_rescrape: bool,
    /// A single-game scrape is running.
    pub rescraping: bool,
    /// RetroAchievements is on and signed in: the game menu offers it.
    pub cheevos: bool,
    /// The achievements screen's list, filled by the host.
    pub sheet: crate::cheevos::Sheet,
    /// Achievements per page, reported by the renderer (theme layout).
    pub sheet_page: usize,
    /// Packages a system update would bring, when the START menu tells.
    pub updates: usize,
    /// Installed modules and their live values, from the host.
    pub modules: std::sync::Arc<crate::modules::View>,
    history: Vec<(Screen, usize)>,
    moving: Option<order::Moving>,
    selected_core: Option<(String, String)>,
}

impl App {
    pub fn new(page_size: usize) -> Self {
        Self {
            screen: Screen::Systems,
            cursor: 0,
            systems_nav: Nav::vertical(page_size),
            games_nav: Nav::vertical(page_size),
            netplay: false,
            details_focus: 0,
            details_scroll: 0,
            details_overflow: 0,
            can_rescrape: false,
            rescraping: false,
            cheevos: false,
            sheet: crate::cheevos::Sheet::default(),
            sheet_page: 1,
            updates: 0,
            history: Vec::new(),
            moving: None,
            selected_core: None,
            modules: std::sync::Arc::default(),
        }
    }

    /// Follows the active view; called again whenever the theme or the view changes.
    pub fn set_view(&mut self, view: &View) {
        self.systems_nav = Nav::of(&view.systems);
        self.games_nav = Nav::of(&view.games);
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

    pub fn quick_menu_items(&self, data: &dyn DataSource) -> Vec<QuickItem> {
        use QuickItem::*;
        let from_details = matches!(self.history.last(), Some((Screen::Details { .. }, _)));
        match &self.screen {
            Screen::QuickMenu {
                game: Some((system, _)),
            } => {
                let mut items = Vec::new();
                if data.cores(system).len() > 1 {
                    items.push(Core);
                }
                if !from_details {
                    items.push(Details);
                }
                if self.cheevos {
                    items.push(Achievements);
                }
                items.extend(self.module_items(Context::Game, system));
                items.extend([Suspend, Reboot, Shutdown]);
                items
            }
            Screen::QuickMenu { game: None } => {
                let items: &[QuickItem] = match (self.netplay, data.system_order().is_empty()) {
                    (false, true) => &[Settings, Move, Suspend, Reboot, Shutdown],
                    (false, false) => &[Settings, Move, RestoreOrder, Suspend, Reboot, Shutdown],
                    (true, true) => &[Settings, Netplay, Move, Suspend, Reboot, Shutdown],
                    (true, false) => &[
                        Settings,
                        Netplay,
                        Move,
                        RestoreOrder,
                        Suspend,
                        Reboot,
                        Shutdown,
                    ],
                };
                let mut items = items.to_vec();
                let system = self.quick_system(data).unwrap_or_default();
                let at = items.len() - 3;
                items.splice(at..at, self.module_items(Context::System, &system));
                items
            }
            _ => Vec::new(),
        }
    }
    fn module_items(&self, context: Context, system: &str) -> Vec<QuickItem> {
        (0..self.modules.quick(context, system).len())
            .map(|n| QuickItem::Module(n as u8))
            .collect()
    }
    /// The system a systems-screen quick menu was opened on.
    fn quick_system(&self, data: &dyn DataSource) -> Option<String> {
        let (_, cursor) = self.history.last()?;
        Self::system_rows(data).get(*cursor).cloned()
    }
    /// A module item of the open quick menu: its module id and declaration.
    pub fn quick_module(
        &self,
        n: u8,
        data: &dyn DataSource,
    ) -> Option<(String, &crate::modules::Quick)> {
        let (context, system) = match &self.screen {
            Screen::QuickMenu {
                game: Some((system, _)),
            } => (Context::Game, system.clone()),
            Screen::QuickMenu { game: None } => (Context::System, self.quick_system(data)?),
            _ => return None,
        };
        let items = self.modules.quick(context, &system);
        let (module, quick) = items.get(usize::from(n))?;
        Some((self.modules.modules[*module].id.clone(), *quick))
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
        if let Screen::Details { system, game } = self.screen.clone() {
            return self.handle_details(b, system, game, data);
        }
        if matches!(self.screen, Screen::Achievements { .. }) {
            self.handle_achievements(b);
            return Outcome::None;
        }
        let len = match &self.screen {
            Screen::StartHub { .. } | Screen::Details { .. } | Screen::Achievements { .. } => {
                unreachable!()
            }
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
                let step = if matches!(self.screen, Screen::Games { .. }) {
                    self.games_nav
                } else {
                    self.systems_nav
                }
                .step();
                self.cursor = if b == Button::L {
                    self.cursor.saturating_sub(step)
                } else {
                    self.cursor.saturating_add(step).min(last)
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
                        if system == crate::moonlight::SYSTEM {
                            return Outcome::Moonlight;
                        }
                        self.enter(Screen::Games {
                            system: system.clone(),
                        });
                    }
                }
                Button::Y => self.enter(Screen::QuickMenu { game: None }),
                _ => {}
            },
            Screen::StartHub { .. } | Screen::Details { .. } | Screen::Achievements { .. } => {
                unreachable!()
            }
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
                    (Button::A, Some(QuickItem::Details)) => {
                        self.back();
                        if let Some((system, game)) = game {
                            self.enter(Screen::Details { system, game });
                            self.details_focus = 0;
                            self.details_scroll = 0;
                        }
                    }
                    (Button::A, Some(QuickItem::Achievements)) => {
                        self.back();
                        if let Some((system, game)) = game {
                            self.enter(Screen::Achievements {
                                system: system.clone(),
                                game,
                            });
                            self.sheet = crate::cheevos::Sheet::default();
                            return Outcome::Achievements { system, game };
                        }
                    }
                    (Button::A, Some(QuickItem::Settings)) => {
                        self.back();
                        return Outcome::Settings;
                    }
                    (Button::A, Some(QuickItem::Netplay)) => {
                        self.back();
                        return Outcome::OpenNetplay;
                    }
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
                    (Button::A, Some(QuickItem::Module(n))) => {
                        let Some((module, quick)) = self.quick_module(*n, data) else {
                            return Outcome::None;
                        };
                        let action = quick.action.clone();
                        let call = match &game {
                            Some((system, index)) => Call {
                                system: system.clone(),
                                game_key: data.game_key(system, *index).unwrap_or_default(),
                                rom: data
                                    .games(system)
                                    .get(*index)
                                    .map(|g| g.path.display().to_string())
                                    .unwrap_or_default(),
                            },
                            None => Call {
                                system: self.quick_system(data).unwrap_or_default(),
                                ..Call::default()
                            },
                        };
                        self.back();
                        return Outcome::Module(crate::modules::Request::Invoke {
                            module,
                            action,
                            call: Some(call),
                        });
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

    /// The details game moved (its system was re-read and re-sorted): follow
    /// it, and the list cursor below it when that list is the same system.
    pub fn retarget_details(&mut self, index: usize) {
        let mut system = None;
        for screen in
            std::iter::once(&mut self.screen).chain(self.history.iter_mut().map(|(s, _)| s))
        {
            if let Screen::Details { system: name, game } = screen {
                *game = index;
                system = Some(name.clone());
            }
        }
        for (screen, cursor) in &mut self.history {
            if matches!(screen, Screen::Games { system: name } if Some(&*name) == system.as_ref()) {
                *cursor = index;
            }
        }
    }

    fn handle_details(
        &mut self,
        b: Button,
        system: String,
        game: usize,
        data: &dyn DataSource,
    ) -> Outcome {
        let last = DETAILS_ACTIONS.len() - 1;
        match b {
            Button::B => self.back(),
            Button::Up => self.details_scroll = self.details_scroll.saturating_sub(1),
            Button::Down => {
                self.details_scroll = (self.details_scroll + 1).min(self.details_overflow)
            }
            Button::L => self.details_focus = self.details_focus.saturating_sub(1),
            Button::R => self.details_focus = (self.details_focus + 1).min(last),
            Button::Y => {
                self.selected_core = data.current_core(&system, game);
                self.enter(Screen::QuickMenu {
                    game: Some((system, game)),
                });
            }
            Button::A => {
                return match self.details_focus {
                    0 => Outcome::Launch { system, game },
                    1 => Outcome::ToggleFavorite { system, game },
                    _ if self.can_rescrape && !self.rescraping => {
                        Outcome::Rescrape { system, game }
                    }
                    _ => Outcome::None,
                }
            }
            _ => {}
        }
        Outcome::None
    }

    /// The list moves by one with ↑ ↓ and by a page with L R; B goes back.
    fn handle_achievements(&mut self, b: Button) {
        let last = self.sheet.items.len().saturating_sub(1);
        let page = self.sheet_page.max(1);
        self.cursor = match b {
            Button::B => return self.back(),
            Button::Up => self.cursor.saturating_sub(1),
            Button::Down => self.cursor + 1,
            Button::L => self.cursor.saturating_sub(page),
            Button::R => self.cursor + page,
            _ => self.cursor,
        }
        .min(last);
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
