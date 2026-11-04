//! The question asked after a system or a game changed emulator: take the
//! cartridge saves along (Converter-save). The host plans and converts.
use crate::state::Button;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub system: String,
    /// One game's ROM, or the whole system.
    pub rom: Option<PathBuf>,
    /// Emulator and core before and after the change.
    pub from: (String, String),
    pub to: (String, String),
    pub from_name: String,
    pub to_name: String,
    /// Save extensions, ".dsv" and ".sav".
    pub from_ext: String,
    pub to_ext: String,
    pub count: usize,
    /// The count is an upper bound (N64 ROMs are read only to convert).
    pub up_to: bool,
    /// Of them, older saves at the destination that are replaced (after
    /// their own copy).
    pub replace: usize,
    /// Where the originals are copied, as the dialog shows it.
    pub keep: String,
    /// The Convert button has the focus (else "Not now").
    pub on_convert: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Convert,
    Later,
}
impl Offer {
    /// ‹ › move between the buttons, A answers the focused one, B declines.
    pub fn handle(&mut self, button: Button) -> Option<Answer> {
        match button {
            Button::L | Button::R => self.on_convert = !self.on_convert,
            Button::A if self.on_convert => return Some(Answer::Convert),
            Button::A | Button::B => return Some(Answer::Later),
            _ => {}
        }
        None
    }
}

impl Default for Offer {
    fn default() -> Self {
        Self {
            system: String::new(),
            rom: None,
            from: Default::default(),
            to: Default::default(),
            from_name: String::new(),
            to_name: String::new(),
            from_ext: String::new(),
            to_ext: String::new(),
            count: 0,
            up_to: false,
            replace: 0,
            keep: String::new(),
            on_convert: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn convert_has_the_focus_and_b_always_declines() {
        let mut offer = Offer {
            on_convert: true,
            ..Offer::default()
        };
        assert_eq!(offer.handle(Button::Down), None);
        assert_eq!(offer.handle(Button::A), Some(Answer::Convert));
        assert_eq!(offer.handle(Button::R), None);
        assert_eq!(offer.handle(Button::A), Some(Answer::Later));
        assert_eq!(offer.handle(Button::L), None);
        assert_eq!(offer.handle(Button::B), Some(Answer::Later));
    }
}
