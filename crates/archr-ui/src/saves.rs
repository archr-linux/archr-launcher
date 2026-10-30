//! A game's save states and screenshots, as the host found them on disk.
//! The host fills `Progress` off the draw thread; this is only the model.
use std::path::PathBuf;

/// Which program wrote a state, and so how a launch can start from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A RetroArch slot (`-e N`), or the automatic state.
    RetroArch,
    /// A standalone emulator's state file (`--savestate FILE`).
    Standalone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save {
    pub path: PathBuf,
    /// The picture RetroArch saves beside the state.
    pub picture: Option<PathBuf>,
    /// None: the automatic state, written when the game closes.
    pub slot: Option<u16>,
    /// Unix seconds.
    pub saved: u64,
    pub bytes: u64,
    pub kind: Kind,
    /// "RetroArch · snes9x", "Mupen64Plus".
    pub emulator: String,
    /// The game's emulator now can start from this state.
    pub playable: bool,
    /// The cartridge saves copied to `.bak` before the game starts from
    /// this state (loading a state rewrites them when the game closes).
    pub backup: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub path: PathBuf,
    /// Unix seconds.
    pub taken: u64,
    pub bytes: u64,
    /// Pixels, from the file's header.
    pub size: Option<(u32, u32)>,
    /// The same picture the details screen shows.
    pub details: bool,
}

/// One game's progress, newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// The game this belongs to (real system, index); None before a scan.
    pub game: Option<(String, usize)>,
    pub saves: Vec<Save>,
    pub shots: Vec<Shot>,
}

/// Why a RetroArch state cannot start the game now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    Hardcore,
    Netplay,
}

/// The question over the Saves or Screenshots screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirm {
    pub action: Action,
    /// The state or the picture it is about.
    pub path: PathBuf,
    /// The action's button has the focus (else Cancel).
    pub on_action: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Play,
    DeleteSave,
    DeleteShot,
}

impl Progress {
    /// The newest state, for the game menu's "last … ago".
    pub fn latest(&self) -> Option<u64> {
        self.saves.iter().map(|s| s.saved).max()
    }
}
