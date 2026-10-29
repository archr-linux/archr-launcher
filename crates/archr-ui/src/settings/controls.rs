//! Controls pages (Controles-launcher): which control does each launcher
//! action. Choices apply to the launcher as soon as they are saved. The
//! in-game hotkeys page lives in `hotkeys`.
use super::{described, row, Effect, Id, Kind, Page, Row, Settings};
use crate::pad::{bindings, bindings_text, Pad, Remap, ACTIONS};
use crate::state::Button;

pub const CONTROLS: &[Row] = &[
    described(
        Id::LauncherLink,
        "controls.launcher",
        "controls.launcher_detail",
    )
    .kind(Kind::Action),
    described(
        Id::HotkeysLink,
        "controls.hotkeys",
        "controls.hotkeys_detail",
    )
    .kind(Kind::Action),
];
pub const LAUNCHER: &[Row] = &[
    row(Id::Bind(0), ACTIONS[0].2, Kind::Action),
    row(Id::Bind(1), ACTIONS[1].2, Kind::Action),
    row(Id::Bind(2), ACTIONS[2].2, Kind::Action),
    row(Id::Bind(3), ACTIONS[3].2, Kind::Action),
    row(Id::Bind(4), ACTIONS[4].2, Kind::Action),
    row(Id::Bind(5), ACTIONS[5].2, Kind::Action),
    row(Id::Bind(6), ACTIONS[6].2, Kind::Action),
    row(Id::ButtonsRestore, "settings.restore", Kind::Action),
];
/// The index of the "back" action, whose control cancels a prompt.
const BACK: usize = 1;

/// A "press a button" prompt waiting for the next press.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub id: Id,
    /// The first of two buttons, once pressed.
    pub first: Option<Pad>,
    /// Why the last press was refused: the control and who has it.
    pub conflict: Option<(Pad, &'static str)>,
}
impl Capture {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            first: None,
            conflict: None,
        }
    }
}

impl Settings {
    pub fn bindings(&self) -> [Pad; ACTIONS.len()] {
        bindings(self.value(Id::Buttons))
    }
    /// What the launcher's controls do now.
    pub fn remap(&self) -> Remap {
        Remap::saved(self.value(Id::Buttons))
    }
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }
    /// The control that cancels a prompt: the one going back.
    pub fn cancel_pad(&self) -> Pad {
        self.bindings()[BACK]
    }
    pub(super) fn default_bindings(&self) -> bool {
        self.bindings() == ACTIONS.map(|a| a.1)
    }
    fn save_bindings(&mut self, bound: &[Pad]) -> Effect {
        let text = bindings_text(bound);
        if text == bindings_text(&self.bindings()) {
            return Effect::None;
        }
        self.pending = Some(Id::Buttons);
        Effect::Set(Id::Buttons, text)
    }
    pub(super) fn launcher_controls_input(&mut self, button: Button) -> Option<Effect> {
        if button != Button::A || self.pending.is_some() || self.loading {
            return None;
        }
        Some(match self.visible_rows().get(self.cursor)?.id {
            id @ Id::Bind(_) => {
                self.capture = Some(Capture::new(id));
                Effect::None
            }
            Id::ButtonsRestore => self.save_bindings(&ACTIONS.map(|a| a.1)),
            _ => return None,
        })
    }
    /// The press a prompt waited for. The back control cancels; a control
    /// another action has trades places with this one's.
    pub fn captured(&mut self, pad: Pad) -> Effect {
        let Some(capture) = self.capture.take() else {
            return Effect::None;
        };
        if pad == self.cancel_pad() {
            return Effect::None;
        }
        let Id::Bind(action) = capture.id else {
            return self.hotkey_captured(capture, pad);
        };
        let action = usize::from(action);
        if pad.is_direction() || pad == Pad::Fn {
            self.capture = Some(capture);
            return Effect::None;
        }
        let mut bound = self.bindings();
        if let Some(other) = bound.iter().position(|p| *p == pad) {
            bound[other] = bound[action];
        }
        bound[action] = pad;
        self.save_bindings(&bound)
    }
    pub(super) fn controls_hints(&self) -> &'static [(crate::HintButton, &'static str)] {
        use crate::HintButton::Key;
        match self.visible_rows().get(self.cursor).map(|r| r.id) {
            Some(Id::ButtonsRestore) if self.default_bindings() => &[(Key('B'), "hint.back")],
            Some(Id::ButtonsRestore) => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
            Some(Id::RaHotkeys) => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
            Some(Id::HotkeyModifier) => &[
                (crate::HintButton::Pair('L', 'R'), "settings.change"),
                (Key('B'), "hint.back"),
            ],
            Some(Id::Hotkey(n)) if self.value(Id::HotkeyOn(n)) == "0" => &[
                (Key('A'), "settings.change"),
                (Key('X'), "controls.enable"),
                (Key('B'), "hint.back"),
            ],
            Some(Id::Hotkey(_)) => &[
                (Key('A'), "settings.change"),
                (Key('X'), "controls.disable"),
                (Key('B'), "hint.back"),
            ],
            _ if self.page() == Page::Controls => {
                &[(Key('A'), "hint.open"), (Key('B'), "hint.back")]
            }
            _ => &[(Key('A'), "settings.change"), (Key('B'), "hint.back")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state.open(Page::Controls);
        state.handle(Button::A);
        assert_eq!(state.page(), Page::LauncherControls);
        state
    }
    fn saved(state: &mut Settings, effect: Effect) -> String {
        let Effect::Set(Id::Buttons, value) = effect else {
            panic!("expected a save, got {effect:?}");
        };
        state.complete(Id::Buttons, Ok(value.clone()));
        value
    }

    #[test]
    fn a_prompts_and_the_next_press_binds_swapping_with_its_owner() {
        let mut state = page();
        state.cursor = 3;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.capturing());
        let effect = state.captured(Pad::R2);
        assert_eq!(saved(&mut state, effect), "A,B,X,R2,L1,R1,START");
        assert!(!state.capturing());
        // Confirm takes B from back, which gets A: a swap in one step.
        state.cursor = 1;
        state.handle(Button::A);
        let effect = state.captured(Pad::A);
        assert_eq!(saved(&mut state, effect), "B,A,X,R2,L1,R1,START");
        assert_eq!(state.remap().button(Pad::A), Some(Button::B));
        assert_eq!(state.cancel_pad(), Pad::A);
    }

    #[test]
    fn the_back_control_cancels_and_directions_keep_waiting() {
        let mut state = page();
        state.handle(Button::A);
        assert_eq!(state.captured(Pad::Up), Effect::None);
        assert_eq!(state.captured(Pad::Fn), Effect::None);
        assert!(state.capturing());
        assert_eq!(state.captured(Pad::B), Effect::None);
        assert!(!state.capturing());
        assert_eq!(state.value(Id::Buttons), "A,B,X,Y,L1,R1,START");
        // Its own control again changes nothing.
        state.handle(Button::A);
        assert_eq!(state.captured(Pad::A), Effect::None);
    }

    #[test]
    fn restore_is_offered_only_when_something_moved() {
        let mut state = page();
        state.cursor = LAUNCHER.len() - 1;
        assert!(state.disabled(Id::ButtonsRestore));
        assert_eq!(state.handle(Button::A), Effect::None);
        state
            .values
            .insert(Id::Buttons, "B,A,X,Y,L1,R1,START".into());
        assert!(!state.disabled(Id::ButtonsRestore));
        let effect = state.handle(Button::A);
        assert_eq!(saved(&mut state, effect), "A,B,X,Y,L1,R1,START");
    }
}
