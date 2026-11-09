//! Custom colours (the accent, the palette and the background): hue,
//! saturation and brightness sliders and a hex code.
use super::{Effect, Id, Page, Settings};
use crate::{
    appearance::{hex, hsv_to_rgb, parse_hex, rgb, rgb_to_hsv, ACCENTS, ACCENT_RGB, BACKGROUNDS},
    keyboard::Mode,
    state::Button,
    theme::Rgba,
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
    /// The theme's own accent (an empty value), the presets that differ
    /// from it and, last, the remembered custom colour.
    pub fn accent_choices(&self) -> Vec<String> {
        let own = self.base_theme.highlight_bg;
        std::iter::once(String::new())
            .chain(
                ACCENTS
                    .iter()
                    .zip(ACCENT_RGB)
                    .filter(|(_, color)| rgb(*color) != own)
                    .map(|(name, _)| (*name).into()),
            )
            .chain([hex(self.custom.rgb())])
            .collect()
    }
    /// Which dot is chosen: a preset hidden for matching the theme's accent
    /// is the theme's dot.
    pub fn accent_index(&self) -> usize {
        let value = self.value(Id::Accent);
        self.accent_choices()
            .iter()
            .position(|choice| choice == value)
            .unwrap_or(0)
    }
    /// Rows whose value is a colour the editor can set.
    pub fn is_colour(id: Id) -> bool {
        matches!(
            id,
            Id::Accent
                | Id::Background
                | Id::TextColor
                | Id::SelectedTextColor
                | Id::SecondaryColor
        )
    }
    /// The colour `id` has on screen now, with the draft applied.
    fn colour_in_effect(&self, id: Id) -> u32 {
        let theme = self.appearance().apply(&self.base_theme);
        let c: Rgba = match id {
            Id::Background => theme.background,
            Id::TextColor => theme.text,
            Id::SelectedTextColor => theme.highlight_text,
            Id::SecondaryColor => theme.muted,
            _ => theme.highlight_bg,
        };
        u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b)
    }
    /// The theme's own colour (empty), the presets (background only) and the
    /// row's own colour, once the editor made one.
    pub fn colour_choices(&self, id: Id) -> Vec<String> {
        let own = parse_hex(self.value(id)).or_else(|| self.kept_colours.get(&id).copied());
        let presets: &[&str] = if id == Id::Background {
            &BACKGROUNDS
        } else {
            &[""]
        };
        presets
            .iter()
            .map(|p| (*p).into())
            .chain(own.map(hex))
            .collect()
    }
    /// A on a colour row opens the editor at the colour in effect.
    pub(super) fn open_custom(&mut self, id: Id) {
        self.custom_kept = self.custom;
        self.custom = Custom::from_rgb(self.colour_in_effect(id));
        self.custom_target = id;
        self.open(Page::AccentColor);
    }
    pub(super) fn custom_input(&mut self, button: Button) -> Effect {
        let row = self.cursor;
        let sliding = row < ROWS - 1;
        match button {
            Button::B => {
                self.custom = self.custom_kept;
                return self.back();
            }
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
                self.values
                    .insert(self.custom_target, hex(self.custom.rgb()));
                // The accent's remembered custom colour stays its own.
                if self.custom_target != Id::Accent {
                    self.custom = self.custom_kept;
                }
                return self.back();
            }
            Button::A => {
                let digits = hex(self.custom.rgb())[1..].to_owned();
                self.open_keyboard(self.custom_target, Mode::Hex, digits);
            }
            _ => {}
        }
        Effect::None
    }
}
