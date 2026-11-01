//! Games › BIOS (Bios, Bios-livre): per system, the BIOS its emulators use,
//! each file against its MD5, and the free BIOS the system can install.
//! `archr-systems --json` decides everything shown; `archr-bios-livres`
//! installs. The host runs both off SDL's thread. Only free BIOS are ever
//! installed: the page says where to copy the console's own.
use super::{Effect, Page, Settings};
use crate::state::Button;
use crate::HintButton;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    Ok,
    Missing,
    /// Present with another MD5.
    Bad,
    /// A known free BIOS under the console's name (C-BIOS as MSX.ROM).
    Free,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Bad,
    /// No console BIOS, and a core carries a free one.
    Builtin,
    /// The free BIOS of the package is installed.
    Free,
    Optional,
    Missing,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct File {
    pub file: String,
    pub md5: String,
    pub state: FileState,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct System {
    pub id: String,
    pub name: String,
    pub cores: String,
    pub verdict: Verdict,
    /// The free BIOS `archr-bios-livres install` offers: "psx" or "msx".
    pub free: Option<String>,
    pub free_installed: bool,
    pub files: Vec<File>,
}
impl System {
    /// The page offers to install its free BIOS.
    pub fn installable(&self) -> bool {
        self.free.is_some() && !self.free_installed
    }
}
#[derive(Debug, Deserialize)]
struct Report {
    systems: Vec<System>,
}
/// The systems of `archr-systems --json`.
pub fn parse(json: &str) -> Result<Vec<System>, String> {
    serde_json::from_str::<Report>(json)
        .map(|r| r.systems)
        .map_err(|e| e.to_string())
}
#[derive(Clone, Debug, Default)]
pub struct Bios {
    pub systems: Option<Vec<System>>,
    pub scanning: bool,
    /// Look again, keeping the list on screen meanwhile.
    pub stale: bool,
    /// The install question is open; `choice` 0 installs, 1 cancels.
    pub confirm: bool,
    pub choice: u8,
    pub installing: bool,
}

impl Settings {
    /// Nothing known yet about an open page: the host looks.
    pub fn bios_wants_scan(&self) -> bool {
        self.page() == Page::Bios
            && (self.bios.systems.is_none() || self.bios.stale)
            && !self.bios.scanning
    }
    pub fn bios_system(&self) -> Option<&System> {
        self.bios.systems.as_ref()?.get(self.cursor)
    }
    pub(super) fn bios_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::{Key, Pair};
        if self.bios.installing {
            &[]
        } else if self.bios.confirm {
            &[
                (Pair('‹', '›'), "bios.choose"),
                (Key('A'), "hint.confirm"),
                (Key('B'), "hint.cancel"),
            ]
        } else if self.bios_system().is_some_and(System::installable) {
            &[
                (Key('A'), "bios.choose"),
                (Key('Y'), "bios.check_again"),
                (Key('B'), "hint.back"),
            ]
        } else {
            &[(Key('Y'), "bios.check_again"), (Key('B'), "hint.back")]
        }
    }
    pub(super) fn bios_input(&mut self, button: Button) -> Option<Effect> {
        if self.bios.installing {
            return Some(Effect::None);
        }
        if self.bios.confirm {
            match button {
                Button::L | Button::R => self.bios.choice = 1 - self.bios.choice.min(1),
                Button::A if self.bios.choice == 0 => {
                    self.bios.confirm = false;
                    let free = self.bios_system().and_then(|s| s.free.clone())?;
                    self.bios.installing = true;
                    return Some(Effect::BiosInstall(free));
                }
                Button::A | Button::B => self.bios.confirm = false,
                _ => {}
            }
            return Some(Effect::None);
        }
        let last = self
            .bios
            .systems
            .as_ref()
            .map_or(0, Vec::len)
            .saturating_sub(1);
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(last),
            Button::A if self.bios_system().is_some_and(System::installable) => {
                self.bios.confirm = true;
                self.bios.choice = 0;
            }
            Button::Y => self.bios.stale = true,
            Button::B => return None,
            _ => {}
        }
        Some(Effect::None)
    }
    /// The install's outcome: the page looks again either way.
    pub fn bios_installed(&mut self, result: Result<(), String>) {
        self.bios.installing = false;
        self.bios.stale = true;
        if let Err(error) = result {
            log::warn!("archr-bios-livres: {error}");
            self.fail(None, "bios.install_failed".into());
        }
    }
    /// A fresh list keeps the cursor on the same system.
    pub fn bios_scanned(&mut self, systems: Vec<System>) {
        let focus = self.bios_system().map(|s| s.id.clone());
        self.bios.scanning = false;
        self.bios.stale = false;
        if self.page() == Page::Bios {
            self.cursor = focus
                .and_then(|id| systems.iter().position(|s| s.id == id))
                .unwrap_or(0)
                .min(systems.len().saturating_sub(1));
        }
        self.bios.systems = Some(systems);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Id;

    const JSON: &str = r#"{"bios_dir":"/storage/roms/bios","systems":[
      {"id":"psx","name":"PlayStation","cores":"pcsx_rearmed · DuckStation","verdict":"builtin",
       "builtin":true,"free":"psx","free_installed":false,
       "files":[{"file":"scph5501.bin","md5":"490f666e1afb15b7362b406ed1cea246","state":"missing"}]},
      {"id":"gba","name":"Game Boy Advance","cores":"gpSP · mGBA","verdict":"ok","builtin":true,
       "free":null,"free_installed":false,
       "files":[{"file":"gba_bios.bin","md5":"a860e8c0b6d573d191e4ec7db1b1e4f6","state":"ok"}]},
      {"id":"msx","name":"MSX","cores":"blueMSX · fMSX","verdict":"free","builtin":false,
       "free":"msx","free_installed":true,
       "files":[{"file":"MSX.ROM","md5":"364a1a579fe5cb8dba54519bcfcdac0d","state":"free"}]}]}"#;

    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Games);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::BiosLink)
            .expect("Games links to BIOS");
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Bios);
        assert!(state.bios_wants_scan());
        state.bios.scanning = true;
        state.bios_scanned(parse(JSON).unwrap());
        state
    }

    #[test]
    fn the_scripts_json_parses() {
        let systems = parse(JSON).unwrap();
        assert_eq!(systems.len(), 3);
        assert_eq!(systems[0].verdict, Verdict::Builtin);
        assert_eq!(systems[2].files[0].state, FileState::Free);
        assert!(systems[0].installable() && !systems[1].installable() && !systems[2].installable());
        assert!(parse("No missing bios").is_err());
    }
    #[test]
    fn a_installs_the_free_bios_after_a_question() {
        let mut state = page();
        assert!(!state.bios_wants_scan());
        assert_eq!(state.bios_hints().len(), 3);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.bios.confirm);
        // Cancel, chosen with the D-pad, closes it.
        state.handle(Button::R);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(!state.bios.confirm);
        state.handle(Button::A);
        assert_eq!(state.handle(Button::A), Effect::BiosInstall("psx".into()));
        assert!(state.bios.installing);
        assert_eq!(
            state.handle(Button::B),
            Effect::None,
            "waits for the install"
        );
        state.bios_installed(Ok(()));
        assert!(state.bios_wants_scan(), "looks again");
        assert_eq!(state.page(), Page::Bios);
    }
    #[test]
    fn systems_without_a_free_file_only_scroll_and_check_again() {
        let mut state = page();
        state.handle(Button::Down);
        assert_eq!(state.bios_system().unwrap().id, "gba");
        assert_eq!(state.bios_hints().len(), 2);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(!state.bios.confirm);
        for _ in 0..5 {
            state.handle(Button::Down);
        }
        assert_eq!(state.cursor, 2);
        state.handle(Button::Y);
        assert!(state.bios_wants_scan());
        // The new list keeps the system in focus.
        state.bios.scanning = true;
        state.bios_scanned(parse(JSON).unwrap());
        assert_eq!(state.cursor, 2);
        state.bios_installed(Err("pacman".into()));
        assert_eq!(state.error, Some((None, "bios.install_failed".into())));
        assert_eq!(state.handle(Button::B), Effect::None);
        assert_eq!(state.page(), Page::Games);
    }
}
