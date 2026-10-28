//! Controller-driven on-screen keyboard shared by settings and Moonlight.
//! One key table per layer drives navigation, input and rendering.
use crate::state::Button;
use std::net::Ipv4Addr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Text,
    Secret,
    Ipv4,
    Digits,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(&'static str),
    Shift,
    Back,
    Space,
    /// Switches to the given layer; the label is the layer's name.
    Layer(usize, &'static str),
    Submit,
}
impl Key {
    /// Width in mockup pixels (Wifi-senha: 40 px letters, 56 px modifiers).
    pub fn width(self) -> f32 {
        match self {
            Self::Char(text) if text.len() > 1 => 56.,
            Self::Char(_) => 40.,
            Self::Space => 260.,
            Self::Submit => 120.,
            _ => 56.,
        }
    }
}
type Layer = &'static [&'static [Key]];
macro_rules! chars {
    ($($c:literal)*) => { &[$(Key::Char($c)),*] };
}
const LETTERS: Layer = &[
    chars!("q" "w" "e" "r" "t" "y" "u" "i" "o" "p"),
    chars!("a" "s" "d" "f" "g" "h" "j" "k" "l"),
    &[
        Key::Shift,
        Key::Char("z"),
        Key::Char("x"),
        Key::Char("c"),
        Key::Char("v"),
        Key::Char("b"),
        Key::Char("n"),
        Key::Char("m"),
        Key::Back,
    ],
    &[
        Key::Layer(1, "123"),
        Key::Space,
        Key::Char(".com"),
        Key::Submit,
    ],
];
const SYMBOLS: Layer = &[
    chars!("1" "2" "3" "4" "5" "6" "7" "8" "9" "0"),
    chars!("-" "/" ":" ";" "(" ")" "$" "&" "@" "\""),
    &[
        Key::Layer(2, "#+="),
        Key::Char("."),
        Key::Char(","),
        Key::Char("?"),
        Key::Char("!"),
        Key::Char("'"),
        Key::Back,
    ],
    &[
        Key::Layer(0, "abc"),
        Key::Space,
        Key::Char(".com"),
        Key::Submit,
    ],
];
const MORE: Layer = &[
    chars!("[" "]" "{" "}" "#" "%" "^" "*" "+" "="),
    chars!("_" "\\" "|" "~" "<" ">" "`"),
    &[
        Key::Layer(1, "123"),
        Key::Char("."),
        Key::Char(","),
        Key::Char("?"),
        Key::Char("!"),
        Key::Char("'"),
        Key::Back,
    ],
    &[
        Key::Layer(0, "abc"),
        Key::Space,
        Key::Char(".com"),
        Key::Submit,
    ],
];
const IPV4: Layer = &[
    chars!("1" "2" "3" "4" "5" "6" "7" "8" "9" "0"),
    &[Key::Char("."), Key::Back, Key::Submit],
];
const DIGITS: Layer = &[
    chars!("1" "2" "3" "4" "5" "6" "7" "8" "9" "0"),
    &[Key::Back, Key::Submit],
];

/// Partial input is never logged: `Debug` hides the value.
#[derive(Clone)]
pub struct Keyboard {
    pub mode: Mode,
    value: String,
    /// Byte limit (an SSID holds 32 bytes, a WPA passphrase 63).
    pub max: usize,
    /// Empty input submits (an open hidden network has no password).
    pub optional: bool,
    pub shifted: bool,
    pub reveal: bool,
    layer: usize,
    row: usize,
    column: usize,
}
impl std::fmt::Debug for Keyboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keyboard({:?}, {} bytes)", self.mode, self.value.len())
    }
}
impl Keyboard {
    pub fn new(mode: Mode, value: String) -> Self {
        Self {
            mode,
            value,
            max: match mode {
                Mode::Ipv4 => 15,
                Mode::Digits => 5,
                _ => 128,
            },
            optional: false,
            shifted: false,
            reveal: false,
            layer: 0,
            row: 0,
            column: 0,
        }
    }
    pub fn text(&self) -> &str {
        &self.value
    }
    pub fn rows(&self) -> Layer {
        match (self.mode, self.layer) {
            (Mode::Ipv4, _) => IPV4,
            (Mode::Digits, _) => DIGITS,
            (_, 1) => SYMBOLS,
            (_, 2) => MORE,
            _ => LETTERS,
        }
    }
    pub fn selected(&self) -> (usize, usize) {
        (self.row, self.column)
    }
    pub fn display(&self) -> String {
        if self.mode == Mode::Secret && !self.reveal {
            "•".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        }
    }
    pub fn valid(&self) -> bool {
        if self.value.is_empty() {
            return self.optional;
        }
        match self.mode {
            Mode::Ipv4 => self.value.parse::<Ipv4Addr>().is_ok(),
            Mode::Digits => self.value.parse::<u16>().is_ok_and(|port| port > 0),
            _ => true,
        }
    }
    fn push(&mut self, text: &str) {
        let text = if self.shifted {
            self.shifted = false;
            text.to_uppercase()
        } else {
            text.to_owned()
        };
        if self.value.len() + text.len() <= self.max {
            self.value.push_str(&text);
        }
    }
    fn center(&self, row: usize, column: usize) -> f32 {
        let keys = self.rows()[row];
        keys[..column].iter().map(|k| k.width() + 6.).sum::<f32>() + keys[column].width() / 2.
            - (keys.iter().map(|k| k.width() + 6.).sum::<f32>() - 6.) / 2.
    }
    /// Up/Down land on the key nearest the same horizontal position.
    fn vertical(&mut self, row: usize) {
        let x = self.center(self.row, self.column);
        self.column = (0..self.rows()[row].len())
            .min_by(|&a, &b| {
                (self.center(row, a) - x)
                    .abs()
                    .total_cmp(&(self.center(row, b) - x).abs())
            })
            .unwrap_or(0);
        self.row = row;
    }
    /// Returns the submitted value. Invalid input is never submitted.
    pub fn handle(&mut self, button: Button) -> Option<String> {
        let rows = self.rows().len();
        let width = self.rows()[self.row].len();
        match button {
            Button::L => self.column = (self.column + width - 1) % width,
            Button::R => self.column = (self.column + 1) % width,
            Button::Up => self.vertical((self.row + rows - 1) % rows),
            Button::Down => self.vertical((self.row + 1) % rows),
            Button::Y => {
                self.value.pop();
            }
            Button::X if self.mode == Mode::Secret => self.reveal = !self.reveal,
            Button::Start if self.valid() => return Some(self.value.clone()),
            Button::A => match self.rows()[self.row][self.column] {
                Key::Char(text) => self.push(text),
                Key::Space => self.push(" "),
                Key::Shift => self.shifted = !self.shifted,
                Key::Back => {
                    self.value.pop();
                }
                Key::Layer(layer, _) => {
                    self.layer = layer;
                    self.shifted = false;
                    self.row = self.row.min(self.rows().len() - 1);
                    self.column = self.column.min(self.rows()[self.row].len() - 1);
                }
                Key::Submit if self.valid() => return Some(self.value.clone()),
                Key::Submit => {}
            },
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(keyboard: &mut Keyboard, target: Key) -> Option<String> {
        let (row, column) = (0..4)
            .flat_map(|r| (0..12).map(move |c| (r, c)))
            .find(|&(r, c)| keyboard.rows().get(r).and_then(|k| k.get(c)) == Some(&target))
            .expect("key on layer");
        keyboard.row = row;
        keyboard.column = column;
        keyboard.handle(Button::A)
    }
    #[test]
    fn every_printable_ascii_character_is_reachable() {
        let mut keyboard = Keyboard::new(Mode::Secret, String::new());
        for layer in 0..3 {
            keyboard.layer = layer;
            for row in keyboard.rows() {
                for key in *row {
                    if let Key::Char(text) = key {
                        keyboard.push(text);
                    }
                }
            }
        }
        let mut typed: Vec<char> = keyboard.text().chars().collect();
        typed.extend('A'..='Z');
        typed.push(' ');
        for ch in ' '..='~' {
            assert!(typed.contains(&ch), "missing {ch:?}");
        }
    }
    #[test]
    fn shift_is_one_shot_and_layers_switch() {
        let mut keyboard = Keyboard::new(Mode::Text, String::new());
        press(&mut keyboard, Key::Shift);
        press(&mut keyboard, Key::Char("q"));
        press(&mut keyboard, Key::Char("q"));
        press(&mut keyboard, Key::Layer(1, "123"));
        press(&mut keyboard, Key::Char("@"));
        press(&mut keyboard, Key::Layer(2, "#+="));
        press(&mut keyboard, Key::Char("~"));
        assert_eq!(keyboard.text(), "Qq@~");
    }
    #[test]
    fn vertical_moves_keep_the_column_and_reach_the_bottom_row() {
        let mut keyboard = Keyboard::new(Mode::Secret, String::new());
        keyboard.column = 9;
        keyboard.handle(Button::Down);
        assert_eq!(keyboard.selected(), (1, 8));
        keyboard.handle(Button::Down);
        keyboard.handle(Button::Down);
        assert_eq!(keyboard.rows()[3][keyboard.column], Key::Submit);
        keyboard.handle(Button::Down);
        assert_eq!(keyboard.selected().0, 0);
    }
    #[test]
    fn modes_validate_before_submitting() {
        let mut ip = Keyboard::new(Mode::Ipv4, "192.168.0".into());
        assert_eq!(press(&mut ip, Key::Submit), None);
        press(&mut ip, Key::Char("."));
        press(&mut ip, Key::Char("7"));
        assert_eq!(ip.handle(Button::Start), Some("192.168.0.7".into()));
        let mut port = Keyboard::new(Mode::Digits, "0".into());
        assert_eq!(port.handle(Button::Start), None);
        let mut hidden = Keyboard::new(Mode::Secret, String::new());
        assert_eq!(hidden.handle(Button::Start), None);
        hidden.optional = true;
        assert_eq!(hidden.handle(Button::Start), Some(String::new()));
    }
    #[test]
    fn secret_is_masked_until_revealed_and_never_debug_printed() {
        let mut keyboard = Keyboard::new(Mode::Secret, "pé".into());
        assert_eq!(keyboard.display(), "••");
        keyboard.handle(Button::X);
        assert_eq!(keyboard.display(), "pé");
        assert!(!format!("{keyboard:?}").contains("pé"));
        keyboard.handle(Button::Y);
        assert_eq!(keyboard.text(), "p");
    }
    #[test]
    fn byte_limit_is_enforced() {
        let mut keyboard = Keyboard::new(Mode::Text, "x".repeat(31));
        keyboard.max = 32;
        press(&mut keyboard, Key::Char(".com"));
        assert_eq!(keyboard.text().len(), 31);
        press(&mut keyboard, Key::Char("a"));
        assert_eq!(keyboard.text().len(), 32);
    }
}
