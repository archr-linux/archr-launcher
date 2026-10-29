//! Custom accent colour: hue, saturation and brightness sliders and a hex code.
use super::{Effect, Id, Page, Settings};
use crate::{
    appearance::{hex, hsv_to_rgb, parse_hex, rgb_to_hsv, ACCENTS},
    keyboard::Mode,
    state::Button,
};

/// Rows of the page: hue, saturation, brightness, hex code.
pub const ROWS: usize = 4;
const SMALL: [i16; 3] = [3, 2, 2];
const LARGE: [i16; 3] = [30, 10, 10];

/// The colour being edited, as the sliders show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Custom {
    pub hue: u16,
    pub sat: u8,
    pub val: u8,
}
impl Default for Custom {
    fn default() -> Self {
        Self::from_rgb(0xFF6B4A)
    }
}
impl Custom {
    pub fn from_rgb(rgb: u32) -> Self {
        let (hue, sat, val) = rgb_to_hsv(rgb);
        Self { hue, sat, val }
    }
    pub fn rgb(self) -> u32 {
        hsv_to_rgb(self.hue, self.sat, self.val)
    }
    /// The slider value of `row` (hue in degrees, the others in percent).
    pub fn level(self, row: usize) -> u16 {
        [self.hue, self.sat.into(), self.val.into()][row]
    }
    fn nudge(&mut self, row: usize, delta: i16) {
        match row {
            0 => self.hue = (i32::from(self.hue) + i32::from(delta)).rem_euclid(360) as u16,
            1 => self.sat = (i16::from(self.sat) + delta).clamp(0, 100) as u8,
            _ => self.val = (i16::from(self.val) + delta).clamp(0, 100) as u8,
        }
    }
}
impl Settings {
    /// Whether the accent in the draft is a custom colour.
    pub fn accent_is_custom(&self) -> bool {
        parse_hex(self.value(Id::Accent)).is_some()
    }
    /// The preset names and, last, the remembered custom colour.
    pub(super) fn accent_choices(&self) -> Vec<String> {
        ACCENTS
            .iter()
            .map(|a| (*a).into())
            .chain([hex(self.custom.rgb())])
            .collect()
    }
    /// A on the custom dot opens the editor at the colour in effect.
    pub(super) fn open_custom(&mut self) -> bool {
        let Some(rgb) = parse_hex(self.value(Id::Accent)) else {
            return false;
        };
        self.custom = Custom::from_rgb(rgb);
        self.open(Page::AccentColor);
        true
    }
    pub(super) fn custom_input(&mut self, button: Button) -> Effect {
        let row = self.cursor;
        let sliding = row < ROWS - 1;
        match button {
            Button::B => return self.back(),
            Button::Up => self.cursor = row.saturating_sub(1),
            Button::Down => self.cursor = (row + 1).min(ROWS - 1),
            Button::L | Button::R | Button::X | Button::Y if sliding => {
                let delta = match button {
                    Button::L => -SMALL[row],
                    Button::R => SMALL[row],
                    Button::X => -LARGE[row],
                    _ => LARGE[row],
                };
                self.custom.nudge(row, delta);
            }
            Button::A if sliding => {
                self.values.insert(Id::Accent, hex(self.custom.rgb()));
                return self.back();
            }
            Button::A => {
                let digits = hex(self.custom.rgb())[1..].to_owned();
                self.open_keyboard(Id::Accent, Mode::Hex, digits);
            }
            _ => {}
        }
        Effect::None
    }
}
