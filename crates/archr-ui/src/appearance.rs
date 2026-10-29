//! The user's look-and-feel choices, layered over the selected theme.
use crate::theme::{Rgba, Theme};

/// Choice codes, in the order the Appearance page cycles them. An empty
/// value keeps whatever the theme defines.
pub const ACCENTS: [&str; 6] = ["blue", "teal", "violet", "amber", "green", "pink"];
pub const ACCENT_RGB: [u32; 6] = [0x1E88FF, 0x14B8A6, 0x8B5CF6, 0xF59E0B, 0x22C55E, 0xEC4899];
pub const BACKGROUNDS: [&str; 3] = ["", "black", "slate"];
pub const FONTS: [&str; 3] = ["", "mono", "system"];
pub const TEXT_SIZES: [&str; 3] = ["small", "normal", "large"];

/// background, surface, surface_elevated, border, badge.
const SURFACES: [[u32; 5]; 2] = [
    [0x000000, 0x0A0A0A, 0x101010, 0x1A1A1A, 0x1A1A1A],
    [0x0A1020, 0x101A2E, 0x14203A, 0x1B2A47, 0x1B2A47],
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub theme: String,
    pub accent: String,
    pub background: String,
    pub font: String,
    pub text_size: String,
    pub animations: bool,
}

pub(crate) fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
        a: 255,
    }
}
fn mix(from: Rgba, to: Rgba, part: f32) -> Rgba {
    let blend = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * part).round() as u8;
    Rgba {
        r: blend(from.r, to.r),
        g: blend(from.g, to.g),
        b: blend(from.b, to.b),
        a: from.a,
    }
}
fn luminance(c: Rgba) -> f32 {
    let linear = |v: u8| {
        let v = f32::from(v) / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}

impl Appearance {
    /// Multiplier for every interface text size.
    pub fn text_scale(&self) -> f32 {
        match self.text_size.as_str() {
            "small" => 0.9,
            "large" => 1.1,
            _ => 1.,
        }
    }
    /// `theme` with the chosen accent, background and font laid over it.
    pub fn apply(&self, theme: &Theme) -> Theme {
        let mut theme = theme.clone();
        if let Some(n) = ACCENTS.iter().position(|a| *a == self.accent) {
            let accent = rgb(ACCENT_RGB[n]);
            // Light accents (amber, green) need dark ink to stay legible.
            let ink = if luminance(accent) > 0.4 {
                theme.background
            } else {
                rgb(0xFFFFFF)
            };
            theme.highlight_bg = accent;
            theme.highlight_text = ink;
            theme.accent_text_secondary = mix(accent, ink, 0.8);
        }
        if let Some(n) = BACKGROUNDS[1..].iter().position(|b| *b == self.background) {
            let [background, surface, elevated, border, badge] = SURFACES[n].map(rgb);
            theme.background = background;
            theme.surface = surface;
            theme.surface_elevated = elevated;
            theme.border = border;
            theme.badge = badge;
        }
        match self.font.as_str() {
            "mono" => {
                theme.font = theme.font_mono.clone();
                theme.font_semibold = theme.font_mono_medium.clone();
            }
            "system" => {
                theme.font = None;
                theme.font_semibold = None;
            }
            _ => {}
        }
        theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(accent: &str, background: &str, font: &str) -> Appearance {
        Appearance {
            accent: accent.into(),
            background: background.into(),
            font: font.into(),
            ..Appearance::default()
        }
    }
    #[test]
    fn empty_choices_keep_the_theme() {
        let theme = Theme::builtin();
        assert_eq!(look("", "", "").apply(&theme), theme);
        assert_eq!(
            look("blue", "", "").apply(&theme).highlight_bg,
            theme.highlight_bg
        );
    }
    #[test]
    fn accents_pick_legible_ink_and_a_matching_secondary() {
        let theme = Theme::builtin();
        let violet = look("violet", "", "").apply(&theme);
        assert_eq!(violet.highlight_bg, rgb(0x8B5CF6));
        assert_eq!(violet.highlight_text, rgb(0xFFFFFF));
        let amber = look("amber", "", "").apply(&theme);
        assert_eq!(amber.highlight_text, theme.background);
    }
    #[test]
    fn backgrounds_replace_every_surface_and_fonts_follow_the_theme_faces() {
        let theme = Theme::builtin();
        let black = look("", "black", "").apply(&theme);
        assert_eq!(black.background, rgb(0));
        assert_ne!(black.surface, theme.surface);
        let mono = look("", "", "mono").apply(&theme);
        assert_eq!(mono.font, theme.font_mono);
        assert_eq!(mono.font_semibold, theme.font_mono_medium);
        assert_eq!(look("", "", "system").apply(&theme).font, None);
    }
    #[test]
    fn text_sizes_scale_around_normal() {
        let size = |name: &str| {
            Appearance {
                text_size: name.into(),
                ..Appearance::default()
            }
            .text_scale()
        };
        assert!(size("small") < size("normal") && size("normal") < size("large"));
        assert_eq!(size("unknown"), 1.);
    }
}
