//! In-game hotkeys (Atalhos-jogos): the system.cfg keys input_sense reads at
//! start, in evdev names. Exit, MangoHud and screenshot are held with the
//! hotkey modifier (key.hotkey.a), volume and brightness with the function
//! modifier (key.function.a, BTN_SELECT on the RK3326).
use super::{described, row, Capture, Effect, Id, Kind, Row, Settings};
use crate::pad::Pad;
use crate::state::Button;

pub struct Hotkey {
    pub label: &'static str,
    pub key: &'static str,
    pub enabled: &'static str,
    /// Two buttons joined by '+' when it takes a pair.
    pub default: &'static str,
    /// Held with the function modifier, under the key.dpad.events switch.
    pub function: bool,
}
const fn hotkey(
    label: &'static str,
    key: &'static str,
    enabled: &'static str,
    default: &'static str,
    function: bool,
) -> Hotkey {
    Hotkey {
        label,
        key,
        enabled,
        default,
        function,
    }
}
pub const HOTKEYS: [Hotkey; 5] = [
    hotkey(
        "controls.exit",
        "hotkey.exit",
        "hotkey.exit.enabled",
        "BTN_SELECT+BTN_START",
        false,
    ),
    hotkey(
        "controls.mangohud",
        "hotkey.mangohud",
        "hotkey.mangohud.enabled",
        "BTN_WEST",
        false,
    ),
    hotkey(
        "controls.screenshot",
        "hotkey.screenshot",
        "hotkey.screenshot.enabled",
        "BTN_EAST",
        false,
    ),
    hotkey(
        "controls.volume",
        "hotkey.volume",
        "hotkey.volume.enabled",
        "BTN_DPAD_UP+BTN_DPAD_DOWN",
        true,
    ),
    hotkey(
        "controls.brightness",
        "hotkey.brightness",
        "hotkey.brightness.enabled",
        "BTN_DPAD_RIGHT+BTN_DPAD_LEFT",
        true,
    ),
];
/// The modifiers offered, in evdev names. FN has no SDL mapping, but
/// input_sense reads evdev, so it can still hold a hotkey.
pub const MODIFIERS: [&str; 9] = [
    "BTN_TL",
    "BTN_TR",
    "BTN_TL2",
    "BTN_TR2",
    "BTN_SELECT",
    "BTN_START",
    "BTN_THUMBL",
    "BTN_THUMBR",
    "BTN_MODE",
];
/// input_sense opens the game guide on modifier + BTN_NORTH.
const GUIDE: &str = "BTN_NORTH";
pub const HOTKEYS_PAGE: &[Row] = &[
    described(
        Id::HotkeyModifier,
        "controls.hotkey_button",
        "controls.hotkey_button_detail",
    ),
    row(Id::Hotkey(0), HOTKEYS[0].label, Kind::Action),
    row(Id::Hotkey(1), HOTKEYS[1].label, Kind::Action),
    row(Id::Hotkey(2), HOTKEYS[2].label, Kind::Action),
    row(Id::Hotkey(3), HOTKEYS[3].label, Kind::Action),
    row(Id::Hotkey(4), HOTKEYS[4].label, Kind::Action),
    // RetroArch's own hotkeys, rewritten only when asked.
    described(
        Id::RaHotkeys,
        "controls.ra_hotkeys",
        "controls.ra_hotkeys_detail",
    )
    .kind(Kind::Action),
];

/// The shown name of an evdev key: the pad's, or the key without "BTN_".
pub fn key_name(key: &str) -> &str {
    match Pad::from_evdev(key) {
        Some(pad) => pad.name(),
        None => key.trim_start_matches("BTN_"),
    }
}

/// Saved values as input_sense reads them: malformed buttons fall back to
/// the default, an unset exit pair to the older key.hotkey.b/.c, and volume
/// and brightness are on only under key.dpad.events.
pub fn hotkey_values(get: impl Fn(&str) -> Option<String>) -> Vec<(Id, String)> {
    let word = |v: &str| {
        !v.is_empty()
            && v.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    };
    let dpad = get("key.dpad.events").is_some_and(|v| !v.is_empty() && v != "0");
    let mut values = Vec::new();
    for (n, hotkey) in HOTKEYS.iter().enumerate() {
        let pair = hotkey.default.contains('+');
        let saved = get(hotkey.key).filter(|v| {
            let parts: Vec<&str> = v.split('+').collect();
            parts.len() == if pair { 2 } else { 1 } && parts.iter().all(|p| word(p))
        });
        let value = saved.unwrap_or_else(|| match n {
            0 => {
                let old =
                    |key, default: &str| get(key).filter(|v| word(v)).unwrap_or(default.into());
                format!(
                    "{}+{}",
                    old("key.hotkey.b", "BTN_SELECT"),
                    old("key.hotkey.c", "BTN_START")
                )
            }
            _ => hotkey.default.into(),
        });
        let on = get(hotkey.enabled).as_deref() != Some("0") && (dpad || !hotkey.function);
        let n = n as u8;
        values.push((Id::Hotkey(n), value));
        values.push((Id::HotkeyOn(n), if on { "1" } else { "0" }.into()));
    }
    for id in [Id::HotkeyModifier, Id::FunctionModifier] {
        if let Some(value) = get(id.key()).filter(|v| word(v)) {
            values.push((id, value));
        }
    }
    values
}

impl Settings {
    fn hotkey_buttons(&self, n: usize) -> Vec<&str> {
        self.value(Id::Hotkey(n as u8)).split('+').collect()
    }
    /// The capsules of a hotkey row: its modifier, then its buttons; two
    /// directions share one capsule ("↑ ↓").
    pub fn hotkey_pads(&self, n: usize) -> Vec<String> {
        let modifier = if HOTKEYS[n].function {
            Id::FunctionModifier
        } else {
            Id::HotkeyModifier
        };
        let mut keys: Vec<&str> = self.hotkey_buttons(n);
        let arrows = keys
            .iter()
            .all(|k| Pad::from_evdev(k).is_some_and(Pad::is_direction));
        let mut out = vec![key_name(self.value(modifier)).to_owned()];
        if arrows {
            keys.sort_by_key(|k| Pad::from_evdev(k).map(|p| p as u8));
            out.push(
                keys.iter()
                    .map(|k| key_name(k))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        } else {
            out.extend(keys.iter().map(|k| key_name(k).to_owned()));
        }
        out
    }
    /// Who already holds `key`, as a label, for hotkey `own` (or, with
    /// `None`, for the hotkey modifier): input_sense takes the first match,
    /// so every button differs from the others and from its modifier.
    fn hotkey_owner(&self, own: Option<usize>, key: &str) -> Option<&'static str> {
        let function = own.is_some_and(|n| HOTKEYS[n].function);
        if own.is_some() && !function && (key == self.value(Id::HotkeyModifier) || key == GUIDE) {
            return Some(if key == GUIDE {
                "controls.guide"
            } else {
                "controls.hotkey_button"
            });
        }
        if function && key == self.value(Id::FunctionModifier) {
            return Some("controls.function_button");
        }
        (0..HOTKEYS.len())
            .filter(|n| Some(*n) != own)
            .find(|n| self.hotkey_buttons(*n).contains(&key))
            .map(|n| HOTKEYS[n].label)
    }
    pub(super) fn hotkeys_input(&mut self, button: Button) -> Option<Effect> {
        if self.pending.is_some() || self.loading {
            return None;
        }
        let id = self.visible_rows().get(self.cursor)?.id;
        let effect = match (id, button) {
            (Id::Hotkey(_), Button::A) => {
                self.capture = Some(Capture::new(id));
                Effect::None
            }
            (Id::RaHotkeys, Button::A) => Effect::Set(Id::RaHotkeys, String::new()),
            (Id::Hotkey(n), Button::X) => {
                let on = self.value(Id::HotkeyOn(n)) == "0";
                Effect::Set(Id::HotkeyOn(n), if on { "1" } else { "0" }.into())
            }
            // Only free buttons are offered, so the modifier never clashes.
            (Id::HotkeyModifier, Button::L | Button::R) => {
                let current = self.value(Id::HotkeyModifier);
                let at = MODIFIERS.iter().position(|m| *m == current).unwrap_or(0);
                let step = if button == Button::L {
                    MODIFIERS.len() - 1
                } else {
                    1
                };
                let next = (1..MODIFIERS.len())
                    .map(|k| MODIFIERS[(at + k * step) % MODIFIERS.len()])
                    .find(|m| self.hotkey_owner(None, m).is_none())?;
                Effect::Set(Id::HotkeyModifier, next.into())
            }
            _ => return None,
        };
        if let Effect::Set(id, _) = effect {
            self.pending = Some(id);
        }
        Some(effect)
    }
    /// A press for a hotkey prompt: refused with the reason while it clashes,
    /// kept as the first of a pair, or saved.
    pub(super) fn hotkey_captured(&mut self, mut capture: Capture, pad: Pad) -> Effect {
        let Id::Hotkey(n) = capture.id else {
            return Effect::None;
        };
        let own = usize::from(n);
        let key = pad.evdev();
        capture.conflict = if capture.first == Some(pad) {
            Some((pad, "controls.same_button"))
        } else {
            self.hotkey_owner(Some(own), key).map(|owner| (pad, owner))
        };
        let pair = HOTKEYS[own].default.contains('+');
        if capture.conflict.is_some() || (pair && capture.first.is_none()) {
            if capture.conflict.is_none() {
                capture.first = Some(pad);
            }
            self.capture = Some(capture);
            return Effect::None;
        }
        let value = match capture.first {
            Some(first) => format!("{}+{key}", first.evdev()),
            None => key.into(),
        };
        if value == self.value(Id::Hotkey(n)) {
            return Effect::None;
        }
        self.pending = Some(Id::Hotkey(n));
        Effect::Set(Id::Hotkey(n), value)
    }
    /// Title and explanation of the open hotkey prompt, as catalog references.
    pub fn hotkey_prompt(&self) -> Option<(String, String)> {
        let capture = self.capture.as_ref()?;
        let Id::Hotkey(n) = capture.id else {
            return None;
        };
        let hotkey = &HOTKEYS[usize::from(n)];
        let modifier = key_name(self.value(if hotkey.function {
            Id::FunctionModifier
        } else {
            Id::HotkeyModifier
        }));
        let title = format!("controls.press_title|action={}", hotkey.label);
        let detail = match (capture.conflict, capture.first) {
            (Some((_, "controls.same_button")), _) => "controls.same_button".into(),
            (Some((pad, owner)), _) => {
                format!("controls.conflict|button={}|action={owner}", pad.name())
            }
            (None, Some(first)) => format!(
                "{}|combo={modifier} + {}",
                if hotkey.function {
                    "controls.press_down"
                } else {
                    "controls.press_second"
                },
                first.name()
            ),
            (None, None) if hotkey.function => format!("controls.press_up|modifier={modifier}"),
            (None, None) if hotkey.default.contains('+') => {
                format!("controls.press_first|modifier={modifier}")
            }
            (None, None) => format!("controls.press_one|modifier={modifier}"),
        };
        Some((title, detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Page;
    use std::collections::BTreeMap;

    fn page(saved: &[(&str, &str)]) -> Settings {
        let saved: BTreeMap<&str, &str> = saved.iter().copied().collect();
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state
            .values
            .extend(hotkey_values(|k| saved.get(k).map(|v| (*v).to_owned())));
        state.open(Page::Controls);
        state.cursor = 1;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Hotkeys);
        state
    }
    fn press(state: &mut Settings, row: usize, pads: &[Pad]) -> Effect {
        state.cursor = row;
        state.handle(Button::A);
        assert!(state.capturing());
        let mut effect = Effect::None;
        for pad in pads {
            effect = state.captured(*pad);
        }
        effect
    }

    #[test]
    fn saved_keys_read_as_input_sense_reads_them() {
        let state = page(&[]);
        assert_eq!(state.value(Id::Hotkey(0)), "BTN_SELECT+BTN_START");
        assert_eq!(state.value(Id::Hotkey(1)), "BTN_WEST");
        assert_eq!(state.value(Id::HotkeyModifier), "BTN_TL");
        assert_eq!(state.value(Id::FunctionModifier), "BTN_SELECT");
        // Volume and brightness wait for key.dpad.events.
        assert_eq!(state.value(Id::HotkeyOn(2)), "1");
        assert_eq!(state.value(Id::HotkeyOn(3)), "0");
        let state = page(&[
            ("key.hotkey.b", "BTN_TR"),
            ("hotkey.screenshot", "btn east"),
            ("hotkey.mangohud", "BTN_NORTH+BTN_EAST"),
            ("hotkey.mangohud.enabled", "0"),
            ("key.dpad.events", "1"),
            ("hotkey.brightness.enabled", "0"),
            ("key.hotkey.a", "BTN_TR2"),
        ]);
        assert_eq!(state.value(Id::Hotkey(0)), "BTN_TR+BTN_START");
        assert_eq!(state.value(Id::Hotkey(2)), "BTN_EAST");
        assert_eq!(state.value(Id::Hotkey(1)), "BTN_WEST");
        assert_eq!(state.value(Id::HotkeyOn(1)), "0");
        assert_eq!(state.value(Id::HotkeyOn(3)), "1");
        assert_eq!(state.value(Id::HotkeyOn(4)), "0");
        assert_eq!(state.hotkey_pads(0), ["R2", "R1", "START"]);
        assert_eq!(state.hotkey_pads(3), ["SELECT", "↑ ↓"]);
        assert_eq!(state.hotkey_pads(4), ["SELECT", "← →"]);
    }

    #[test]
    fn a_new_button_is_saved_in_evdev_names() {
        let mut state = page(&[]);
        assert_eq!(
            press(&mut state, 2, &[Pad::R2]),
            Effect::Set(Id::Hotkey(1), "BTN_TR2".into())
        );
        assert!(!state.capturing());
        state.complete(Id::Hotkey(1), Ok("BTN_TR2".into()));
        assert_eq!(
            press(&mut state, 1, &[Pad::R1, Pad::Start]),
            Effect::Set(Id::Hotkey(0), "BTN_TR+BTN_START".into())
        );
        state.complete(Id::Hotkey(0), Ok("BTN_TR+BTN_START".into()));
        assert_eq!(
            press(&mut state, 4, &[Pad::L2, Pad::R2]),
            Effect::None,
            "R2 belongs to MangoHud now"
        );
        assert_eq!(
            state.captured(Pad::R3),
            Effect::Set(Id::Hotkey(3), "BTN_TL2+BTN_THUMBR".into())
        );
    }

    #[test]
    fn clashes_are_refused_with_who_holds_the_button() {
        let mut state = page(&[]);
        for (row, pads, owner) in [
            (2, &[Pad::A][..], "controls.screenshot"),
            (2, &[Pad::L1][..], "controls.hotkey_button"),
            (2, &[Pad::X][..], "controls.guide"),
            (3, &[Pad::Up][..], "controls.volume"),
            (4, &[Pad::Select][..], "controls.function_button"),
            (1, &[Pad::Y][..], "controls.mangohud"),
            (1, &[Pad::R1, Pad::R1][..], "controls.same_button"),
        ] {
            assert_eq!(press(&mut state, row, pads), Effect::None);
            let capture = state.capture.clone().expect("still waiting");
            assert_eq!(capture.conflict.map(|c| c.1), Some(owner), "{pads:?}");
            let (_, detail) = state.hotkey_prompt().unwrap();
            assert!(detail.contains(owner), "{detail}");
            // The back control gives up.
            assert_eq!(state.captured(Pad::B), Effect::None);
            assert!(!state.capturing());
        }
        // Volume may use L1: it is held with SELECT, not the hotkey button.
        assert_eq!(
            press(&mut state, 4, &[Pad::L1, Pad::R1]),
            Effect::Set(Id::Hotkey(3), "BTN_TL+BTN_TR".into())
        );
    }

    #[test]
    fn the_modifier_skips_buttons_in_use_and_x_switches_a_hotkey() {
        let mut state = page(&[]);
        state.cursor = 0;
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::HotkeyModifier, "BTN_TR".into())
        );
        state.complete(Id::HotkeyModifier, Ok("BTN_TR2".into()));
        // TR2 → SELECT and START hold the exit pair, so THUMBL is next.
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::HotkeyModifier, "BTN_THUMBL".into())
        );
        state.complete(Id::HotkeyModifier, Ok("BTN_TL".into()));
        assert_eq!(
            state.handle(Button::L),
            Effect::Set(Id::HotkeyModifier, "BTN_MODE".into())
        );
        state.complete(Id::HotkeyModifier, Ok("BTN_TL".into()));
        state.cursor = 4;
        assert_eq!(
            state.handle(Button::X),
            Effect::Set(Id::HotkeyOn(3), "1".into())
        );
    }

    #[test]
    fn prompts_walk_through_both_buttons_of_a_pair() {
        let mut state = page(&[]);
        press(&mut state, 4, &[]);
        assert_eq!(
            state.hotkey_prompt().unwrap(),
            (
                "controls.press_title|action=controls.volume".into(),
                "controls.press_up|modifier=SELECT".into()
            )
        );
        state.captured(Pad::R1);
        assert_eq!(
            state.hotkey_prompt().unwrap().1,
            "controls.press_down|combo=SELECT + R1"
        );
        state.captured(Pad::B);
        press(&mut state, 3, &[]);
        assert_eq!(
            state.hotkey_prompt().unwrap().1,
            "controls.press_one|modifier=L1"
        );
    }
    #[test]
    fn retroarch_hotkeys_are_restored_only_on_request() {
        let mut state = page(&[]);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::RaHotkeys)
            .unwrap();
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::RaHotkeys, String::new())
        );
        state.complete(Id::RaHotkeys, Ok(String::new()));
        assert_eq!(state.error, Some((None, "settings.ra_hotkeys_done".into())));
    }
}
