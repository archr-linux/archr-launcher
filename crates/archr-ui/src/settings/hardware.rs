//! Controls › Vibration (Vibracao) and Stick LEDs (LEDs-analogicos), for
//! the consoles that have them (the AISLPC RK3562 handhelds). The links show
//! only where `archr-rumble supported` and `archr-stick-leds supported`
//! succeed; the host saves each value and runs the tool's `apply`.
use super::{described, row, Id, Kind, Row};

pub const RUMBLE: &[Row] = &[
    described(Id::Rumble, "rumble.enabled", "rumble.enabled_detail").kind(Kind::Toggle),
    row(
        Id::RumbleStrength,
        "rumble.strength",
        Kind::Slider {
            min: 0,
            max: 100,
            step: 10,
        },
    ),
    described(Id::RumbleTest, "rumble.test", "rumble.test_detail").kind(Kind::Action),
];
pub const LEDS: &[Row] = &[
    row(Id::LedsMode, "leds.mode", Kind::Value),
    described(Id::LedsColor, "leds.color", "leds.color_detail"),
    row(Id::LedsBrightness, "leds.brightness", Kind::Value),
];
pub const MODES: [&str; 4] = ["off", "static", "breathing", "rainbow"];
pub const COLORS: [&str; 7] = ["red", "green", "blue", "cyan", "yellow", "magenta", "white"];
/// What each colour looks like on the palette.
pub const COLOR_RGB: [u32; 7] = [
    0xEF4444, 0x22C55E, 0x3B82F6, 0x06B6D4, 0xFACC15, 0xD946EF, 0xF8FAFC,
];

/// The modes drawn in the chosen colour; the others ignore it.
pub fn coloured(mode: &str) -> bool {
    matches!(mode, "static" | "breathing")
}

/// A mode or colour in words.
pub fn label(catalog: &crate::i18n::I18n, value: &str) -> String {
    catalog.text(&format!("leds.{value}"))
}

#[cfg(test)]
mod tests {
    use super::super::{Effect, Page, Settings};
    use super::*;
    use crate::state::Button;

    fn controls(rumble: bool, leds: bool) -> Settings {
        let mut state = Settings {
            loading: false,
            rumble_supported: rumble,
            leds_supported: leds,
            ..Default::default()
        };
        state.open(Page::Controls);
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }
    fn at(state: &mut Settings, id: Id) {
        state.cursor = ids(state).iter().position(|r| *r == id).unwrap();
    }
    #[test]
    fn rows_show_only_where_the_console_has_them() {
        let plain = ids(&controls(false, false));
        assert!(!plain.contains(&Id::RumbleLink) && !plain.contains(&Id::LedsLink));
        assert_eq!(
            plain,
            [Id::LauncherLink, Id::HotkeysLink, Id::PadTestLink],
            "the page as before"
        );
        assert!(ids(&controls(true, false)).contains(&Id::RumbleLink));
        assert!(!ids(&controls(true, false)).contains(&Id::LedsLink));
        assert!(ids(&controls(false, true)).contains(&Id::LedsLink));
    }
    #[test]
    fn vibration_saves_each_value_and_tests_on_a() {
        let mut state = controls(true, true);
        at(&mut state, Id::RumbleLink);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Rumble);
        assert_eq!(state.value(Id::RumbleStrength), "100");
        state.cursor = 1;
        // A palette: L and R step through it, A opens no list.
        assert!(!state.listable(Id::LedsColor, Kind::Value));
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(
            state.handle(Button::L),
            Effect::Set(Id::RumbleStrength, "90".into())
        );
        state.settle(Id::RumbleStrength, Ok("90".into()));
        state.cursor = 2;
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::RumbleTest, String::new())
        );
        state.complete(Id::RumbleTest, Ok(String::new()));
        state.cursor = 0;
        assert_eq!(state.handle(Button::A), Effect::Set(Id::Rumble, "0".into()));
        state.complete(Id::Rumble, Ok("0".into()));
        // Off, strength and test mean nothing.
        for cursor in [1, 2] {
            state.cursor = cursor;
            assert_eq!(state.handle(Button::A), Effect::None);
            assert_eq!(state.handle(Button::R), Effect::None);
        }
    }
    #[test]
    fn the_colour_counts_only_for_static_and_breathing() {
        let mut state = controls(true, true);
        at(&mut state, Id::LedsLink);
        state.handle(Button::A);
        assert_eq!(state.page(), Page::StickLeds);
        assert_eq!(state.value(Id::LedsMode), "rainbow");
        assert!(state.disabled(Id::LedsColor));
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::LedsMode, "off".into()),
            "the modes wrap"
        );
        state.complete(Id::LedsMode, Ok("static".into()));
        assert!(!state.disabled(Id::LedsColor));
        state.cursor = 1;
        // A palette: L and R step through it, A opens no list.
        assert!(!state.listable(Id::LedsColor, Kind::Value));
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(
            state.handle(Button::L),
            Effect::Set(Id::LedsColor, "magenta".into())
        );
        state.complete(Id::LedsColor, Ok("magenta".into()));
        state.cursor = 2;
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::LedsBrightness, "4".into())
        );
        let catalog =
            crate::i18n::I18n::load(std::path::Path::new("/nonexistent"), Some("pt_BR"), None);
        for value in MODES.iter().chain(&COLORS) {
            let id = if MODES.contains(value) {
                Id::LedsMode
            } else {
                Id::LedsColor
            };
            state.values.insert(id, (*value).into());
            let shown = state.display_value(id, &catalog);
            assert!(
                !shown.is_empty() && !shown.starts_with("leds."),
                "{value}: {shown}"
            );
        }
    }
}
