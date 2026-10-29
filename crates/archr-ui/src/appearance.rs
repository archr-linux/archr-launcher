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
pub(crate) fn mix(from: Rgba, to: Rgba, part: f32) -> Rgba {
    let blend = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * part).round() as u8;
    Rgba {
        r: blend(from.r, to.r),
        g: blend(from.g, to.g),
        b: blend(from.b, to.b),
        a: from.a,
    }
}
/// Relative luminance (WCAG 2.x).
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
/// White ink is kept while it holds at least this contrast against the
/// accent (the stock blue, teal, violet and pink all pass); lighter accents get
/// black, which then reads at 8:1 or better.
const WHITE_INK_CONTRAST: f32 = 2.4;
/// Black or white, whichever keeps text on `accent` legible.
pub fn ink_on(accent: Rgba) -> Rgba {
    if 1.05 / (luminance(accent) + 0.05) >= WHITE_INK_CONTRAST {
        rgb(0xFFFFFF)
    } else {
        rgb(0x000000)
    }
}
/// `#RRGGBB` (or `RRGGBB`) as 0xRRGGBB.
pub fn parse_hex(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    (digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
}
pub fn hex(rgb: u32) -> String {
    format!("#{rgb:06X}")
}
/// Hue in degrees (0..360), saturation and value in percent.
pub fn hsv_to_rgb(hue: u16, sat: u8, val: u8) -> u32 {
    let (s, v) = (
        f32::from(sat.min(100)) / 100.,
        f32::from(val.min(100)) / 100.,
    );
    let h = f32::from(hue % 360) / 60.;
    let channel = |offset: f32| {
        let k = (offset + h) % 6.;
        let part = v * (1. - s * k.min(4. - k).clamp(0., 1.));
        (part * 255.).round() as u32
    };
    channel(5.) << 16 | channel(3.) << 8 | channel(1.)
}
pub fn rgb_to_hsv(rgb: u32) -> (u16, u8, u8) {
    let [r, g, b] = [16, 8, 0].map(|shift| f32::from((rgb >> shift) as u8) / 255.);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let delta = max - min;
    let hue = if delta == 0. {
        0.
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.)
    } else if max == g {
        (b - r) / delta + 2.
    } else {
        (r - g) / delta + 4.
    } * 60.;
    let sat = if max == 0. { 0. } else { delta / max };
    (
        (hue.round() as u16) % 360,
        (sat * 100.).round() as u8,
        (max * 100.).round() as u8,
    )
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
        let preset = ACCENTS.iter().position(|a| *a == self.accent);
        // Anything else that is not a valid `#RRGGBB` keeps the theme's accent.
        if let Some(hex) = preset
            .map(|n| ACCENT_RGB[n])
            .or_else(|| parse_hex(&self.accent))
        {
            let accent = rgb(hex);
            let ink = ink_on(accent);
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
        assert_eq!(amber.highlight_text, rgb(0));
    }
    #[test]
    fn ink_follows_contrast_and_keeps_the_stock_accents() {
        for (n, name) in ACCENTS.iter().enumerate() {
            let dark_ink = matches!(*name, "amber" | "green");
            let ink = ink_on(rgb(ACCENT_RGB[n]));
            assert_eq!(ink, rgb(if dark_ink { 0 } else { 0xFFFFFF }), "{name}");
        }
        assert_eq!(ink_on(rgb(0xFFE14D)), rgb(0));
        assert_eq!(ink_on(rgb(0xFFFFFF)), rgb(0));
        assert_eq!(ink_on(rgb(0x000000)), rgb(0xFFFFFF));
        assert_eq!(ink_on(rgb(0xFF6B4A)), rgb(0xFFFFFF));
    }
    #[test]
    fn hex_accents_apply_and_invalid_ones_keep_the_theme() {
        let theme = Theme::builtin();
        let custom = look("#FFE14D", "", "").apply(&theme);
        assert_eq!(custom.highlight_bg, rgb(0xFFE14D));
        assert_eq!(custom.highlight_text, rgb(0));
        for bad in ["#12345", "#GGGGGG", "#1234567", "chartreuse"] {
            assert_eq!(look(bad, "", "").apply(&theme), theme, "{bad}");
        }
    }
    #[test]
    fn hex_codes_parse_with_or_without_the_hash() {
        assert_eq!(parse_hex("#ff6b4a"), Some(0xFF6B4A));
        assert_eq!(parse_hex("FF6B4A"), Some(0xFF6B4A));
        assert_eq!(parse_hex("#FF6B4"), None);
        assert_eq!(parse_hex("#FF6B4AA"), None);
        assert_eq!(parse_hex("+F6B4A"), None);
        assert_eq!(parse_hex(""), None);
        assert_eq!(hex(0x0A0B0C), "#0A0B0C");
    }
    #[test]
    fn hsv_and_rgb_convert_both_ways() {
        assert_eq!(hsv_to_rgb(0, 100, 100), 0xFF0000);
        assert_eq!(hsv_to_rgb(120, 100, 100), 0x00FF00);
        assert_eq!(hsv_to_rgb(240, 100, 100), 0x0000FF);
        assert_eq!(hsv_to_rgb(60, 100, 50), 0x808000);
        assert_eq!(hsv_to_rgb(200, 0, 100), 0xFFFFFF);
        assert_eq!(hsv_to_rgb(200, 50, 0), 0);
        assert_eq!(rgb_to_hsv(0xFF0000), (0, 100, 100));
        assert_eq!(rgb_to_hsv(0x808000), (60, 100, 50));
        assert_eq!(rgb_to_hsv(0x7F7F7F), (0, 0, 50));
        assert_eq!(rgb_to_hsv(0xFF6B4A), (11, 71, 100));
        for hue in (0..360).step_by(7) {
            for (sat, val) in [(100, 100), (60, 80), (30, 40)] {
                let back = rgb_to_hsv(hsv_to_rgb(hue, sat, val));
                let drift = (i32::from(back.0) - i32::from(hue))
                    .rem_euclid(360)
                    .min((i32::from(hue) - i32::from(back.0)).rem_euclid(360));
                assert!(drift <= 2 && back.1.abs_diff(sat) <= 2 && back.2.abs_diff(val) <= 1);
            }
        }
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
