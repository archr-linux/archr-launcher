//! Physical controls of the handheld: the names the launcher saves, the evdev
//! key names input_sense reads, and the SDL events behind them. SDL numbers the
//! R36S buttons in evdev order and gamecontrollerdb.txt maps r36s_Gamepad from
//! that (a:b1 is BTN_EAST, b:b0 BTN_SOUTH, x:b2 BTN_NORTH, y:b3 BTN_WEST).
use crate::state::Button;
use sdl2::{
    controller::{Axis, Button as Sdl},
    event::Event,
    joystick::HatState,
    keyboard::Keycode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pad {
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Select,
    Start,
    L3,
    R3,
    /// BTN_MODE: no SDL mapping, so it is only offered as a modifier.
    Fn,
    Up,
    Down,
    Left,
    Right,
}
/// Every control with its shown name and evdev key name.
const PADS: [(Pad, &str, &str); 17] = [
    (Pad::A, "A", "BTN_EAST"),
    (Pad::B, "B", "BTN_SOUTH"),
    (Pad::X, "X", "BTN_NORTH"),
    (Pad::Y, "Y", "BTN_WEST"),
    (Pad::L1, "L1", "BTN_TL"),
    (Pad::R1, "R1", "BTN_TR"),
    (Pad::L2, "L2", "BTN_TL2"),
    (Pad::R2, "R2", "BTN_TR2"),
    (Pad::Select, "SELECT", "BTN_SELECT"),
    (Pad::Start, "START", "BTN_START"),
    (Pad::L3, "L3", "BTN_THUMBL"),
    (Pad::R3, "R3", "BTN_THUMBR"),
    (Pad::Fn, "FN", "BTN_MODE"),
    (Pad::Up, "↑", "BTN_DPAD_UP"),
    (Pad::Down, "↓", "BTN_DPAD_DOWN"),
    (Pad::Left, "←", "BTN_DPAD_LEFT"),
    (Pad::Right, "→", "BTN_DPAD_RIGHT"),
];
/// The launcher's remappable actions: the button each one stands for, its
/// default control and its label, in the Controls page order.
pub const ACTIONS: [(Button, Pad, &str); 7] = [
    (Button::A, Pad::A, "controls.confirm"),
    (Button::B, Pad::B, "controls.back"),
    (Button::X, Pad::X, "controls.favorite"),
    (Button::Y, Pad::Y, "controls.quick_menu"),
    (Button::L, Pad::L1, "controls.page_previous"),
    (Button::R, Pad::R1, "controls.page_next"),
    (Button::Start, Pad::Start, "controls.start_menu"),
];
impl Pad {
    pub fn name(self) -> &'static str {
        PADS[self as usize].1
    }
    pub fn evdev(self) -> &'static str {
        PADS[self as usize].2
    }
    pub fn from_name(name: &str) -> Option<Self> {
        PADS.iter().find(|p| p.1 == name).map(|p| p.0)
    }
    pub fn from_evdev(key: &str) -> Option<Self> {
        PADS.iter().find(|p| p.2 == key).map(|p| p.0)
    }
    pub fn is_direction(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }
    /// The control behind a button-like event, pressed or released. Hats and
    /// sticks are directions only (`Remap::map`).
    pub(crate) fn of(event: &Event) -> Option<Self> {
        Some(match *event {
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            }
            | Event::KeyUp {
                keycode: Some(key), ..
            } => match key {
                Keycode::Return => Self::A,
                Keycode::Backspace => Self::B,
                Keycode::X => Self::X,
                Keycode::Y => Self::Y,
                Keycode::PageUp => Self::L1,
                Keycode::PageDown => Self::R1,
                Keycode::Up => Self::Up,
                Keycode::Down => Self::Down,
                Keycode::Left => Self::Left,
                Keycode::Right => Self::Right,
                _ => return None,
            },
            Event::ControllerButtonDown { button, .. }
            | Event::ControllerButtonUp { button, .. } => match button {
                Sdl::A => Self::A,
                Sdl::B => Self::B,
                Sdl::X => Self::X,
                Sdl::Y => Self::Y,
                Sdl::LeftShoulder => Self::L1,
                Sdl::RightShoulder => Self::R1,
                Sdl::Back => Self::Select,
                Sdl::Start => Self::Start,
                Sdl::LeftStick => Self::L3,
                Sdl::RightStick => Self::R3,
                Sdl::DPadUp => Self::Up,
                Sdl::DPadDown => Self::Down,
                Sdl::DPadLeft => Self::Left,
                Sdl::DPadRight => Self::Right,
                _ => return None,
            },
            // Conventional SDL joystick fallback, only used for unmapped devices.
            Event::JoyButtonDown { button_idx, .. } | Event::JoyButtonUp { button_idx, .. } => *[
                Self::A,
                Self::B,
                Self::X,
                Self::Y,
                Self::L1,
                Self::R1,
                Self::Select,
                Self::Start,
            ]
            .get(usize::from(button_idx))?,
            // gamecontrollerdb maps the digital L2/R2 to the trigger axes.
            Event::ControllerAxisMotion {
                axis: Axis::TriggerLeft,
                value,
                ..
            } if value > 0 => Self::L2,
            Event::ControllerAxisMotion {
                axis: Axis::TriggerRight,
                value,
                ..
            } if value > 0 => Self::R2,
            _ => return None,
        })
    }
    /// A fresh press, for the "press a button" prompts.
    pub(crate) fn pressed(event: &Event) -> Option<Self> {
        match *event {
            Event::KeyDown { .. }
            | Event::ControllerButtonDown { .. }
            | Event::JoyButtonDown { .. } => Self::of(event),
            Event::ControllerAxisMotion { value, .. } if value > 19660 => Self::of(event),
            Event::JoyHatMotion { state, .. } => match state {
                HatState::Up => Some(Self::Up),
                HatState::Down => Some(Self::Down),
                HatState::Left => Some(Self::Left),
                HatState::Right => Some(Self::Right),
                _ => None,
            },
            _ => None,
        }
    }
}

/// What each control does in the launcher: directions and SELECT are fixed
/// unless an action takes SELECT, the rest follow the Controls page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remap([Option<Button>; PADS.len()]);
impl Default for Remap {
    fn default() -> Self {
        Self::new(ACTIONS.map(|(button, pad, _)| (pad, button)))
    }
}
impl Remap {
    pub fn new(bound: impl IntoIterator<Item = (Pad, Button)>) -> Self {
        let mut map = [None; PADS.len()];
        for (pad, button) in [
            (Pad::Up, Button::Up),
            (Pad::Down, Button::Down),
            (Pad::Left, Button::L),
            (Pad::Right, Button::R),
            (Pad::Select, Button::Select),
        ] {
            map[pad as usize] = Some(button);
        }
        for (pad, button) in bound {
            if !pad.is_direction() {
                map[pad as usize] = Some(button);
            }
        }
        Self(map)
    }
    pub fn button(&self, pad: Pad) -> Option<Button> {
        self.0[pad as usize]
    }
    /// The launcher button of an event: its control's action, or a direction.
    pub fn map(&self, event: &Event) -> Option<Button> {
        if let Some(pad) = Pad::of(event) {
            return self.button(pad);
        }
        match *event {
            Event::JoyHatMotion { state, .. } => match state {
                HatState::Up | HatState::LeftUp | HatState::RightUp => Some(Button::Up),
                HatState::Down | HatState::LeftDown | HatState::RightDown => Some(Button::Down),
                HatState::Left => Some(Button::L),
                HatState::Right => Some(Button::R),
                _ => None,
            },
            Event::JoyAxisMotion {
                axis_idx: 0, value, ..
            }
            | Event::ControllerAxisMotion {
                axis: Axis::LeftX,
                value,
                ..
            } => axis_button(value, Button::L, Button::R),
            Event::JoyAxisMotion {
                axis_idx: 1, value, ..
            }
            | Event::ControllerAxisMotion {
                axis: Axis::LeftY,
                value,
                ..
            } => axis_button(value, Button::Up, Button::Down),
            _ => None,
        }
    }
}

fn axis_button(value: i16, negative: Button, positive: Button) -> Option<Button> {
    match value {
        ..0 => Some(negative),
        1.. => Some(positive),
        0 => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(button: Sdl) -> Event {
        Event::ControllerButtonDown {
            timestamp: 0,
            which: 0,
            button,
        }
    }

    #[test]
    fn r36s_buttons_carry_the_evdev_names_of_its_device_tree() {
        for (button, evdev) in [
            (Sdl::A, "BTN_EAST"),
            (Sdl::B, "BTN_SOUTH"),
            (Sdl::X, "BTN_NORTH"),
            (Sdl::Y, "BTN_WEST"),
            (Sdl::LeftShoulder, "BTN_TL"),
            (Sdl::RightShoulder, "BTN_TR"),
            (Sdl::Back, "BTN_SELECT"),
            (Sdl::Start, "BTN_START"),
            (Sdl::LeftStick, "BTN_THUMBL"),
            (Sdl::RightStick, "BTN_THUMBR"),
            (Sdl::DPadUp, "BTN_DPAD_UP"),
            (Sdl::DPadDown, "BTN_DPAD_DOWN"),
            (Sdl::DPadLeft, "BTN_DPAD_LEFT"),
            (Sdl::DPadRight, "BTN_DPAD_RIGHT"),
        ] {
            let found = Pad::pressed(&pad(button)).expect("mapped");
            assert_eq!(found.evdev(), evdev, "{button:?}");
            assert_eq!(Pad::from_evdev(evdev), Some(found));
            assert_eq!(Pad::from_name(found.name()), Some(found));
        }
        for (axis, evdev) in [
            (Axis::TriggerLeft, "BTN_TL2"),
            (Axis::TriggerRight, "BTN_TR2"),
        ] {
            let pull = |value| Event::ControllerAxisMotion {
                timestamp: 0,
                which: 0,
                axis,
                value,
            };
            assert_eq!(Pad::pressed(&pull(32767)).map(Pad::evdev), Some(evdev));
            assert_eq!(Pad::pressed(&pull(9000)), None);
        }
        assert_eq!(Pad::from_evdev("BTN_MODE"), Some(Pad::Fn));
        assert_eq!(Pad::from_evdev("KEY_POWER"), None);
        assert_eq!(Pad::pressed(&pad(Sdl::Guide)), None);
    }

    #[test]
    fn remap_moves_actions_and_keeps_directions() {
        let default = Remap::default();
        assert_eq!(default.map(&pad(Sdl::A)), Some(Button::A));
        assert_eq!(default.map(&pad(Sdl::LeftShoulder)), Some(Button::L));
        assert_eq!(default.map(&pad(Sdl::DPadLeft)), Some(Button::L));
        assert_eq!(default.map(&pad(Sdl::Back)), Some(Button::Select));
        assert_eq!(default.map(&pad(Sdl::LeftStick)), None);
        let swapped = Remap::new([
            (Pad::B, Button::A),
            (Pad::A, Button::B),
            (Pad::R2, Button::R),
            (Pad::Up, Button::Start),
        ]);
        assert_eq!(swapped.map(&pad(Sdl::B)), Some(Button::A));
        assert_eq!(swapped.map(&pad(Sdl::A)), Some(Button::B));
        assert_eq!(swapped.map(&pad(Sdl::DPadUp)), Some(Button::Up));
        assert_eq!(swapped.map(&pad(Sdl::RightShoulder)), None);
        let trigger = Event::ControllerAxisMotion {
            timestamp: 0,
            which: 0,
            axis: Axis::TriggerRight,
            value: 30000,
        };
        assert_eq!(swapped.map(&trigger), Some(Button::R));
        let enter = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Return),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        };
        assert_eq!(swapped.map(&enter), Some(Button::B));
    }
}
