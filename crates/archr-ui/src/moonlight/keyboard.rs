//! Temporary IPv4 input adapter; replace this module/render adapter with the
//! shared settings keyboard at merge without changing Moonlight's runtime.
use crate::state::Button;

#[derive(Clone, Debug, Default)]
pub struct Keyboard {
    text: String,
    cursor: usize,
}
impl Keyboard {
    pub const KEYS: [&'static str; 13] = [
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        "0",
        ".",
        "moonlight.delete",
        "moonlight.connect",
    ];
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn handle(&mut self, button: Button) -> Option<String> {
        let count = Self::KEYS.len();
        match button {
            Button::L => self.cursor = (self.cursor + count - 1) % count,
            Button::R => self.cursor = (self.cursor + 1) % count,
            Button::Up => self.cursor = (self.cursor + count - 5) % count,
            Button::Down => self.cursor = (self.cursor + 5) % count,
            Button::X => {
                self.text.pop();
            }
            Button::A if self.cursor == 11 => {
                self.text.pop();
            }
            Button::A if self.cursor == 12 => return Some(self.text.clone()),
            Button::A if self.text.len() < 15 => self.text.push_str(Self::KEYS[self.cursor]),
            _ => {}
        }
        None
    }
}
