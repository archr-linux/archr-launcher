//! Netplay lobby: pure state and navigation. Fetching and launching belong to the host.
use crate::state::Button;
use std::path::PathBuf;

/// Display order of the lobby sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Lan,
    Online,
    Unavailable,
}
impl Group {
    pub fn title(self) -> &'static str {
        ["netplay.lan", "netplay.online", "netplay.unavailable"][self as usize]
    }
}

/// How the room's game relates to the local library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rom {
    Same,
    Different,
    Unknown,
    NoRom,
    NoCore,
}
impl Rom {
    pub fn label(self) -> &'static str {
        [
            "netplay.rom_same",
            "netplay.rom_different",
            "netplay.rom_unknown",
            "netplay.rom_missing",
            "netplay.rom_nocore",
        ][self as usize]
    }
    pub fn note(self) -> &'static str {
        [
            "netplay.note_same",
            "netplay.note_different",
            "netplay.note_unknown",
            "netplay.note_missing",
            "netplay.note_nocore",
        ][self as usize]
    }
}

/// Everything the host needs to start the game as a netplay client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Join {
    pub game: String,
    pub system: String,
    pub rom: PathBuf,
    pub emulator: String,
    pub core: String,
    pub ip: String,
    pub port: u16,
    pub spectate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    pub group: Group,
    pub game: String,
    pub host: String,
    pub place: String,
    pub core: String,
    /// Relay (MITM) name; empty when the room is reached directly.
    pub relay: String,
    pub latency_ms: Option<u32>,
    pub rom: Rom,
    /// Present only when the game and core are installed and the address is valid.
    pub join: Option<Join>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Close,
    Refresh,
    Join(Box<Join>),
}

/// Rooms are kept sorted by group, so the cursor follows the display order.
pub struct Lobby {
    pub rooms: Vec<Room>,
    pub cursor: usize,
    pub loading: bool,
    pub failed: bool,
    /// Selected action of the open Join / Watch menu.
    pub menu: Option<usize>,
}
impl Default for Lobby {
    fn default() -> Self {
        Self {
            rooms: Vec::new(),
            cursor: 0,
            loading: true,
            failed: false,
            menu: None,
        }
    }
}
impl Lobby {
    pub fn set(&mut self, rooms: Vec<Room>, failed: bool) {
        self.cursor = self.cursor.min(rooms.len().saturating_sub(1));
        self.rooms = rooms;
        self.failed = failed;
        self.loading = false;
    }
    pub fn handle(&mut self, button: Button) -> Effect {
        if let Some(item) = self.menu {
            match button {
                Button::Up | Button::Down => self.menu = Some(1 - item),
                Button::B => self.menu = None,
                Button::A => {
                    self.menu = None;
                    let join = self.rooms.get(self.cursor).and_then(|r| r.join.clone());
                    if let Some(join) = join {
                        return Effect::Join(Box::new(Join {
                            spectate: item == 1,
                            ..join
                        }));
                    }
                }
                _ => {}
            }
            return Effect::None;
        }
        let last = self.rooms.len().saturating_sub(1);
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(last),
            Button::B => return Effect::Close,
            Button::Y if !self.loading => {
                self.loading = true;
                return Effect::Refresh;
            }
            Button::A
                if self
                    .rooms
                    .get(self.cursor)
                    .is_some_and(|r| r.join.is_some()) =>
            {
                self.menu = Some(0);
            }
            _ => {}
        }
        Effect::None
    }
}
