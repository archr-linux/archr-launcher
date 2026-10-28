//! Pure network-list parsing and controller-driven text entry.
use crate::state::Button;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub security: String,
    pub signal: String,
    pub connected: bool,
    pub known: bool,
}

/// iwd's detail format escapes control characters inside SSIDs, not separators.
pub fn parse_scanlist(output: &str) -> Vec<Network> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let ssid = unescape(fields.next()?);
            let security = fields.next()?;
            let signal = fields.next()?.parse::<u8>().ok()?;
            let connected = fields.next()?;
            let known = fields.next()?;
            if fields.next().is_some()
                || ssid.is_empty()
                || signal > 100
                || !matches!(connected, "0" | "1")
                || !matches!(known, "0" | "1")
            {
                return None;
            }
            Some(Network {
                ssid,
                security: security.into(),
                signal: signal.to_string(),
                connected: connected == "1",
                known: known == "1",
            })
        })
        .collect()
}
fn unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Password and text fields never log or persist partial input.
#[derive(Clone, Debug)]
pub struct Keyboard {
    pub title: String,
    pub secret: bool,
    pub value: String,
    pub selected: usize,
    pub shifted: bool,
    pub numeric: bool,
}
impl Keyboard {
    pub fn new(title: String, secret: bool, value: String) -> Self {
        Self {
            title,
            secret,
            value,
            selected: 0,
            shifted: false,
            numeric: false,
        }
    }
    pub fn display(&self) -> String {
        if self.secret {
            "•".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        }
    }
    pub fn push(&mut self, ch: char) {
        if self.value.len() + ch.len_utf8() <= 128 && !ch.is_control() {
            self.value.push(ch);
        }
    }
    fn valid(&self) -> bool {
        !self.value.is_empty()
            && (!self.secret
                || (8..=63).contains(&self.value.len())
                || (self.value.len() == 64 && self.value.bytes().all(|b| b.is_ascii_hexdigit())))
    }
    pub fn handle(&mut self, button: Button) -> Option<String> {
        const KEYS: &[u8; 30] = b"qwertyuiopasdfghjkl  zxcvbnm  ";
        const NUMBERS: &[u8; 30] = b"1234567890!@#$%^&*(  .:/_-+?  ";
        match button {
            Button::L => self.selected = (self.selected + 30) % 31,
            Button::R => self.selected = (self.selected + 1) % 31,
            Button::Up => {
                self.selected = if self.selected == 30 {
                    20
                } else {
                    (self.selected + 20) % 30
                }
            }
            Button::Down => {
                self.selected = if self.selected == 30 {
                    0
                } else if self.selected >= 20 {
                    30
                } else {
                    self.selected + 10
                }
            }
            Button::X => {
                self.value.pop();
            }
            Button::Y => self.shifted = !self.shifted,
            Button::A => match self.selected {
                30 if self.valid() => return Some(self.value.clone()),
                30 => {}
                19 => self.numeric = !self.numeric,
                20 => self.shifted = !self.shifted,
                29 => {
                    self.value.pop();
                }
                28 => self.push(' '),
                n => {
                    let key = if self.numeric { NUMBERS[n] } else { KEYS[n] };
                    if self.shifted {
                        self.push((key as char).to_ascii_uppercase());
                        self.shifted = false;
                    } else {
                        self.push(key as char);
                    }
                }
            },
            Button::Start if self.valid() => return Some(self.value.clone()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanlist_detail_keeps_ssid_and_ignores_malformed_records() {
        let output = "Café 5G\twpa2\t78\t1\t0\ninvalid\nGuest\topen\t52\t0\t1\n";
        let networks = parse_scanlist(output);
        assert_eq!(networks.len(), 2);
        assert_eq!(networks[0].ssid, "Café 5G");
        assert!(networks[0].connected);
        assert!(!networks[0].known);
        assert!(networks[1].known);
    }

    #[test]
    fn numeric_keyboard_can_enter_ip_address_and_port() {
        let mut keyboard = Keyboard::new("Host".into(), false, "".into());
        keyboard.selected = 19;
        keyboard.handle(Button::A);
        keyboard.selected = 0;
        keyboard.handle(Button::A);
        keyboard.selected = 21;
        keyboard.handle(Button::A);
        assert_eq!(keyboard.value, "1.");
    }

    #[test]
    fn keyboard_never_submits_short_wpa_key() {
        let mut keyboard = Keyboard::new("Casa".into(), true, "".into());
        for _ in 0..7 {
            keyboard.push('x');
        }
        assert_eq!(keyboard.handle(Button::Start), None);
        keyboard.push('x');
        assert_eq!(keyboard.handle(Button::Start), Some("xxxxxxxx".into()));
    }

    #[test]
    fn keyboard_connect_button_is_reachable_and_submits_only_valid_passwords() {
        let mut keyboard = Keyboard::new("Casa".into(), true, "short".into());
        keyboard.selected = 28;
        keyboard.handle(Button::Down);
        assert_eq!(keyboard.selected, 30);
        assert_eq!(keyboard.handle(Button::A), None);
        keyboard.push('x');
        keyboard.push('x');
        keyboard.push('x');
        assert_eq!(keyboard.handle(Button::A), Some("shortxxx".into()));
        keyboard.handle(Button::Up);
        assert_eq!(keyboard.selected, 20);
    }

    #[test]
    fn keyboard_does_not_expose_password_and_deletes_unicode_character() {
        let mut keyboard = Keyboard::new("Casa".into(), true, "é".into());
        assert_eq!(keyboard.display(), "•");
        keyboard.handle(Button::X);
        assert_eq!(keyboard.value, "");
    }
}
