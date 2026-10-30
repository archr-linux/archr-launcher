//! Screen transitions and controller input, independent of rendering and SDL.

use crate::layout::{Axis, Layout, View};
use crate::modules::{Call, Context};
use crate::saves::{Action, Blocked, Confirm, Kind, Progress, Save};
use archr_index::{cache::SystemEntry, Game};
use std::path::PathBuf;
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
    /// The game's save states (`App::progress`); the cursor is the state.
    Saves {
        system: String,
        game: usize,
    },
    /// The game's screenshots in a grid; the cursor is the picture.
    Shots {
        system: String,
        game: usize,
    },
    /// One screenshot over the whole screen.
    Shot {
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
    /// Launch the game from this save state.
    PlayFrom {
        system: String,
        game: usize,
        state: PathBuf,
    },
    /// Delete these files (a state and its picture, or a screenshot), then
    /// look at the game's progress again.
    Delete {
        system: String,
        game: usize,
        paths: Vec<PathBuf>,
    },
    /// Make this screenshot the one the details screen shows.
    UseShot {
        system: String,
        game: usize,
        path: PathBuf,
    },
    SetCore {
        system: String,
        game: usize,
        emulator: String,
        core: String,
    },
    /// This game's MangoHud: "1", "0", or "" to follow the system.
    SetMangohud {
        system: String,
        game: usize,
        value: String,
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
    /// The game's save states, when it has any.
    Saves,
    /// The game's screenshots, when it has any.
    Shots,
    Achievements,
    Settings,
    Netplay,
    Core,
    /// This game's MangoHud, over the system's choice.
    Mangohud,
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
    /// This game's own MangoHud value ("" when it follows the system), and
    /// the system's, as a launch would resolve it without the game's.
    fn mangohud(&self, _system: &str, _game: usize) -> (String, String) {
        (String::new(), String::new())
    }
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
    /// The device runs MangoHud: the game menu offers it per game.
    pub mangohud: bool,
    /// The achievements screen's list, filled by the host.
    pub sheet: crate::cheevos::Sheet,
    /// Achievements per page, reported by the renderer (theme layout).
    pub sheet_page: usize,
    /// Packages a system update would bring, when the START menu tells.
    pub updates: usize,
    /// Installed modules and their live values, from the host.
    pub modules: std::sync::Arc<crate::modules::View>,
    /// The focused module widget of the Start menu (its row is 2).
    pub hub_widget: usize,
    /// The game menu's game's states and screenshots, filled by the host.
    pub progress: Progress,
    /// A game whose progress the host should look at now (it takes it).
    pub scan: Option<(String, usize)>,
    /// Why RetroArch cannot start from a state now (hardcore, netplay).
    pub blocked: Option<Blocked>,
    /// The open question on the Saves or Screenshots screens.
    pub confirm: Option<Confirm>,
    /// States per page and the screenshot grid, reported by the renderer.
    pub saves_page: usize,
    pub shots_columns: usize,
    pub shots_page: usize,
    history: Vec<(Screen, usize)>,
    moving: Option<order::Moving>,
    selected_core: Option<(String, String)>,
    /// The open game menu's MangoHud: the game's value, the system's.
    selected_mangohud: (String, String),
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
            mangohud: false,
            sheet: crate::cheevos::Sheet::default(),
            sheet_page: 1,
            updates: 0,
            history: Vec::new(),
            moving: None,
            selected_core: None,
            selected_mangohud: Default::default(),
            modules: std::sync::Arc::default(),
            hub_widget: 0,
            progress: Progress::default(),
            scan: None,
            blocked: None,
            confirm: None,
            saves_page: 1,
            shots_columns: 1,
            shots_page: 1,
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

    /// The game menu only holds what concerns the game; power actions live
    /// in the system menu.
    fn game_menu_items(
        &self,
        data: &dyn DataSource,
        (system, game): (&str, usize),
        from_details: bool,
    ) -> Vec<QuickItem> {
        use QuickItem::*;
        let mut items = Vec::new();
        if data.cores(system).len() > 1 {
            items.push(Core);
        }
        if !from_details {
            items.push(Details);
        }
        // Only what the host found for this game, once it looked.
        if self.progress.game.as_ref().map(|(s, g)| (s.as_str(), *g)) == Some((system, game)) {
            if !self.progress.saves.is_empty() {
                items.push(Saves);
            }
            if !self.progress.shots.is_empty() {
                items.push(Shots);
            }
        }
        if self.cheevos {
            items.push(Achievements);
        }
        if self.mangohud {
            items.push(Mangohud);
        }
        items.extend(self.module_items(Context::Game, system));
        items
    }

    pub fn quick_menu_items(&self, data: &dyn DataSource) -> Vec<QuickItem> {
        use QuickItem::*;
        let from_details = matches!(self.history.last(), Some((Screen::Details { .. }, _)));
        match &self.screen {
            Screen::QuickMenu {
                game: Some((system, game)),
            } => self.game_menu_items(data, (system, *game), from_details),
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
    /// The game menu's MangoHud row: the game's value and the system's.
    pub fn selected_mangohud(&self) -> (&str, &str) {
        (&self.selected_mangohud.0, &self.selected_mangohud.1)
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
        if let Screen::Saves { system, game }
        | Screen::Shots { system, game }
        | Screen::Shot { system, game } = self.screen.clone()
        {
            return self.handle_progress(b, system, game);
        }
        let len = match &self.screen {
            Screen::StartHub { .. }
            | Screen::Details { .. }
            | Screen::Achievements { .. }
            | Screen::Saves { .. }
            | Screen::Shots { .. }
            | Screen::Shot { .. } => {
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
            Screen::StartHub { .. }
            | Screen::Details { .. }
            | Screen::Achievements { .. }
            | Screen::Saves { .. }
            | Screen::Shots { .. }
            | Screen::Shot { .. } => {
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
                        Button::Y => self.open_game_menu(system, game, false, data),
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
                    (Button::A, Some(item @ (QuickItem::Saves | QuickItem::Shots))) => {
                        let item = *item;
                        self.back();
                        if let Some((system, game)) = game {
                            self.enter(if item == QuickItem::Saves {
                                Screen::Saves { system, game }
                            } else {
                                Screen::Shots { system, game }
                            });
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
                                item: None,
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
                    (Button::L | Button::R, Some(QuickItem::Mangohud)) => {
                        if let Some((system, game)) = game {
                            return self.cycle_mangohud(b, system, game);
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
            Button::Y => self.open_game_menu(system, game, true, data),
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

    /// The game menu, after the host is asked to look at the game's states
    /// and screenshots again; its rows follow when the answer comes.
    fn open_game_menu(
        &mut self,
        system: String,
        game: usize,
        from_details: bool,
        data: &dyn DataSource,
    ) {
        if self.progress.game != Some((system.clone(), game)) {
            self.progress = Progress::default();
        }
        self.scan = Some((system.clone(), game));
        if self
            .game_menu_items(data, (&system, game), from_details)
            .is_empty()
        {
            return;
        }
        self.selected_core = data.current_core(&system, game);
        self.selected_mangohud = data.mangohud(&system, game);
        self.enter(Screen::QuickMenu {
            game: Some((system, game)),
        });
    }

    /// The host's answer: the open menu keeps its selected row, a list its
    /// place, and a screen left with nothing to show closes.
    pub fn set_progress(&mut self, progress: Progress, data: &dyn DataSource) {
        let selected = self.quick_menu_items(data).get(self.cursor).copied();
        self.progress = progress;
        match &self.screen {
            Screen::QuickMenu { .. } => {
                let items = self.quick_menu_items(data);
                if let Some(at) = selected.and_then(|s| items.iter().position(|i| *i == s)) {
                    self.cursor = at;
                }
                self.cursor = self.cursor.min(items.len().saturating_sub(1));
            }
            Screen::Saves { .. } | Screen::Shots { .. } | Screen::Shot { .. } => {
                let len = if matches!(self.screen, Screen::Saves { .. }) {
                    self.progress.saves.len()
                } else {
                    self.progress.shots.len()
                };
                if len == 0 {
                    self.confirm = None;
                    self.back();
                    if matches!(self.screen, Screen::Shots { .. }) {
                        self.back();
                    }
                } else {
                    self.cursor = self.cursor.min(len - 1);
                }
            }
            _ => {}
        }
    }

    /// A RetroArch state cannot start the game in hardcore mode or netplay.
    pub fn can_play(&self, save: &Save) -> bool {
        save.playable && (save.kind != Kind::RetroArch || self.blocked.is_none())
    }

    /// Saves, Screenshots and one screenshot; an open question first.
    fn handle_progress(&mut self, b: Button, system: String, game: usize) -> Outcome {
        if let Some(confirm) = &mut self.confirm {
            match b {
                Button::L | Button::R | Button::Up | Button::Down => {
                    confirm.on_action = !confirm.on_action
                }
                Button::B => self.confirm = None,
                Button::A => {
                    let confirm = self.confirm.take().expect("open question");
                    if confirm.on_action {
                        return self.answer(confirm, system, game);
                    }
                }
                _ => {}
            }
            return Outcome::None;
        }
        let saves = matches!(self.screen, Screen::Saves { .. });
        let len = if saves {
            self.progress.saves.len()
        } else {
            self.progress.shots.len()
        };
        let last = len.saturating_sub(1);
        let ask = |action, index, on_action| {
            Some(Confirm {
                action,
                index,
                on_action,
            })
        };
        match (&self.screen, b) {
            (_, Button::B) if matches!(self.screen, Screen::Shot { .. }) => {
                // Back in the grid on the picture last shown.
                let at = self.cursor;
                self.back();
                self.cursor = at;
            }
            (_, Button::B) => self.back(),
            (Screen::Saves { .. }, Button::Up) => self.cursor = self.cursor.saturating_sub(1),
            (Screen::Saves { .. }, Button::Down) => self.cursor = (self.cursor + 1).min(last),
            (Screen::Saves { .. }, Button::L) => {
                self.cursor = self.cursor.saturating_sub(self.saves_page.max(1))
            }
            (Screen::Saves { .. }, Button::R) => {
                self.cursor = (self.cursor + self.saves_page.max(1)).min(last)
            }
            (Screen::Saves { .. }, Button::A) => {
                if self
                    .progress
                    .saves
                    .get(self.cursor)
                    .is_some_and(|save| self.can_play(save))
                {
                    self.confirm = ask(Action::Play, self.cursor, true);
                }
            }
            (Screen::Saves { .. }, Button::X) if len > 0 => {
                self.confirm = ask(Action::DeleteSave, self.cursor, false)
            }
            (Screen::Shots { .. }, Button::Up) => {
                let columns = self.shots_columns.max(1);
                if self.cursor >= columns {
                    self.cursor -= columns;
                }
            }
            (Screen::Shots { .. }, Button::Down) => {
                let columns = self.shots_columns.max(1);
                if self.cursor / columns < last / columns {
                    self.cursor = (self.cursor + columns).min(last);
                }
            }
            (Screen::Shots { .. } | Screen::Shot { .. }, Button::L) => {
                self.cursor = self.cursor.saturating_sub(1)
            }
            (Screen::Shots { .. } | Screen::Shot { .. }, Button::R) => {
                self.cursor = (self.cursor + 1).min(last)
            }
            (Screen::Shots { .. }, Button::A) if len > 0 => {
                let at = self.cursor;
                self.enter(Screen::Shot { system, game });
                self.cursor = at;
            }
            (Screen::Shots { .. } | Screen::Shot { .. }, Button::X) if len > 0 => {
                self.confirm = ask(Action::DeleteShot, self.cursor, false)
            }
            (Screen::Shot { .. }, Button::Y) => {
                if let Some(shot) = self.progress.shots.get(self.cursor).filter(|s| !s.details) {
                    return Outcome::UseShot {
                        system,
                        game,
                        path: shot.path.clone(),
                    };
                }
            }
            _ => {}
        }
        Outcome::None
    }

    fn answer(&mut self, confirm: Confirm, system: String, game: usize) -> Outcome {
        match confirm.action {
            Action::Play => match self.progress.saves.get(confirm.index) {
                Some(save) => Outcome::PlayFrom {
                    system,
                    game,
                    state: save.path.clone(),
                },
                None => Outcome::None,
            },
            Action::DeleteSave => match self.progress.saves.get(confirm.index) {
                Some(save) => Outcome::Delete {
                    system,
                    game,
                    paths: std::iter::once(save.path.clone())
                        .chain(save.picture.clone())
                        .collect(),
                },
                None => Outcome::None,
            },
            Action::DeleteShot => match self.progress.shots.get(confirm.index) {
                Some(shot) => Outcome::Delete {
                    system,
                    game,
                    paths: vec![shot.path.clone()],
                },
                None => Outcome::None,
            },
        }
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

    /// "" (as the system), on, off: the system's own choices.
    fn cycle_mangohud(&mut self, b: Button, system: String, game: usize) -> Outcome {
        let choices = crate::settings::Id::MangohudGame.choices();
        let at = choices
            .iter()
            .position(|v| *v == self.selected_mangohud.0)
            .unwrap_or(0);
        let step = if b == Button::L { choices.len() - 1 } else { 1 };
        let value = choices[(at + step) % choices.len()].to_owned();
        self.selected_mangohud.0.clone_from(&value);
        Outcome::SetMangohud {
            system,
            game,
            value,
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
