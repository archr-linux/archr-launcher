//! SDL event translation and deterministic held-direction repeat.
use crate::{controllers::Controllers, state::Button};
use sdl2::{controller::Button as Pad, event::Event, keyboard::Keycode};
use std::time::{Duration, Instant};

/// The launcher button of an event with the default controls.
pub fn map_event(e: &Event) -> Option<Button> {
    crate::pad::Remap::default().map(e)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Key(Keycode),
    ControllerButton(u32, Pad),
    JoystickButton(u32, u8),
    Hat(u32, u8),
    Axis(u32, u8),
}

impl Source {
    /// The SDL instance it comes from; None for the keyboard.
    fn device(self) -> Option<u32> {
        match self {
            Self::Key(_) => None,
            Self::ControllerButton(which, _)
            | Self::JoystickButton(which, _)
            | Self::Hat(which, _)
            | Self::Axis(which, _) => Some(which),
        }
    }
}

#[derive(Default)]
pub struct Repeat {
    held: Option<(Button, Instant)>,
    sources: Vec<(Source, Button)>,
    repeats: u32,
    /// FN (an unmapped raw button) held: L1 and R1 are input_sense's
    /// report shortcut then, never the launcher's.
    modifiers: Vec<(u32, u8)>,
}
impl Repeat {
    pub fn press(&mut self, button: Button, now: Instant) {
        if matches!(button, Button::Up | Button::Down | Button::L | Button::R)
            && self.held.map(|(b, _)| b) != Some(button)
        {
            self.held = Some((button, now + Duration::from_millis(300)));
            self.repeats = 0;
        }
    }
    pub fn is_held(&self, button: Button) -> bool {
        self.held.map(|(held, _)| held) == Some(button)
    }
    /// A direction or shoulder is held (its repeat is armed).
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }
    pub fn release(&mut self, button: Button) {
        if self.held.map(|(b, _)| b) == Some(button) {
            self.held = None;
        }
    }
    pub fn clear(&mut self) {
        self.held = None;
        self.sources.clear();
        self.modifiers.clear();
    }
    /// FN went down or up on a pad. Down: that pad's L1 and R1 are dropped,
    /// so neither the page turn repeating nor a later release brings them
    /// back.
    fn modifier(&mut self, key: (u32, u8), down: bool) {
        self.modifiers.retain(|k| *k != key);
        if !down {
            return;
        }
        self.modifiers.push(key);
        let shoulder = |b: &Button| matches!(b, Button::L | Button::R);
        let pad = key.0;
        let before = self.sources.len();
        self.sources
            .retain(|(s, b)| !(shoulder(b) && s.device() == Some(pad)));
        if self.sources.len() != before
            && self.held.is_some_and(|(b, _)| {
                shoulder(&b) && !self.sources.iter().any(|(_, held)| *held == b)
            })
        {
            self.held = None;
        }
    }
    /// L1 or R1 from a pad whose FN is down.
    fn suppressed(&self, source: Source, button: Button) -> bool {
        matches!(button, Button::L | Button::R)
            && source
                .device()
                .is_some_and(|pad| self.modifiers.iter().any(|(m, _)| *m == pad))
    }
    fn event(&mut self, event: &Event, mut button: Option<Button>, now: Instant) -> Option<Button> {
        let (source, axis) = match *event {
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            }
            | Event::KeyUp {
                keycode: Some(key), ..
            } => (Source::Key(key), None),
            Event::ControllerButtonDown { which, button, .. }
            | Event::ControllerButtonUp { which, button, .. } => {
                (Source::ControllerButton(which, button), None)
            }
            Event::JoyButtonDown {
                which, button_idx, ..
            }
            | Event::JoyButtonUp {
                which, button_idx, ..
            } => (Source::JoystickButton(which, button_idx), None),
            Event::JoyHatMotion { which, hat_idx, .. } => (Source::Hat(which, hat_idx), None),
            Event::JoyAxisMotion {
                which,
                axis_idx,
                value,
                ..
            } => (Source::Axis(which, axis_idx), Some(value)),
            Event::ControllerAxisMotion {
                which, axis, value, ..
            } => (Source::Axis(which, axis as u8), Some(value)),
            _ => return None,
        };
        let previous = self.sources.iter().position(|(s, _)| *s == source);
        if matches!(
            event,
            Event::KeyUp { .. } | Event::ControllerButtonUp { .. } | Event::JoyButtonUp { .. }
        ) {
            button = None;
        }
        if let Some(value) = axis {
            let magnitude = value.unsigned_abs();
            // Enter above 60%; retain the same direction down to 40%.
            if magnitude < 13107
                || (magnitude <= 19660 && previous.map(|i| self.sources[i].1) != button)
            {
                button = None;
            }
        }
        if previous.map(|i| self.sources[i].1) == button {
            return None;
        }
        if let Some(index) = previous {
            let (_, old) = self.sources.remove(index);
            if !self.sources.iter().any(|(_, b)| *b == old) {
                self.release(old);
                if self.held.is_none() {
                    if let Some((_, remaining)) = self.sources.iter().rev().find(|(_, b)| {
                        matches!(b, Button::Up | Button::Down | Button::L | Button::R)
                    }) {
                        self.press(*remaining, now);
                    }
                }
            }
        }
        let button = button?;
        if self.suppressed(source, button) {
            return None;
        }
        let duplicate = self.sources.iter().any(|(_, b)| *b == button);
        self.sources.push((source, button));
        if duplicate {
            return None;
        }
        self.press(button, now);
        Some(button)
    }
    pub fn poll(&mut self, now: Instant) -> Option<Button> {
        let (button, deadline) = self.held?;
        if now < deadline {
            return None;
        }
        self.held = Some((button, now + Duration::from_millis(80)));
        self.repeats += 1;
        Some(button)
    }
    /// Repeats of the held button so far: 0 for its press, 1 for the first
    /// repeat (300 ms in), then one every 80 ms.
    pub fn repeats(&self) -> u32 {
        self.repeats
    }
    pub fn wait_ms(&self, now: Instant) -> u32 {
        self.held
            .map(|(_, deadline)| {
                deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .clamp(1, 100) as u32
            })
            .unwrap_or(100)
    }
}

pub fn button_event(
    event: &Event,
    controllers: &Controllers,
    repeat: &mut Repeat,
) -> Option<Button> {
    if let Some((key, down)) = controllers.modifier(event) {
        repeat.modifier(key, down);
        return None;
    }
    repeat.event(event, controllers.read(event)?, Instant::now())
}

/// SDL instance identity and stream kind, shared by routing and translation.
pub(crate) fn device_event(event: &Event) -> Option<(u32, bool)> {
    match *event {
        Event::JoyButtonDown { which, .. }
        | Event::JoyButtonUp { which, .. }
        | Event::JoyHatMotion { which, .. }
        | Event::JoyAxisMotion { which, .. } => Some((which, true)),
        Event::ControllerButtonDown { which, .. }
        | Event::ControllerButtonUp { which, .. }
        | Event::ControllerAxisMotion { which, .. } => Some((which, false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl2::joystick::HatState;

    #[test]
    fn native_stick_navigates_and_repeats_like_dpad() {
        use sdl2::controller::Axis::{LeftX, LeftY};
        for (axis_id, value, button) in [
            (LeftX, -32768, Button::L),
            (LeftX, 32767, Button::R),
            (LeftY, -32768, Button::Up),
            (LeftY, 32767, Button::Down),
        ] {
            let now = Instant::now();
            let mut repeat = Repeat::default();
            let event = axis(axis_id, value);
            assert_eq!(repeat.event(&event, map_event(&event), now), Some(button));
            assert_eq!(repeat.poll(now + Duration::from_millis(299)), None);
            assert_eq!(repeat.poll(now + Duration::from_millis(300)), Some(button));
            assert_eq!(repeat.poll(now + Duration::from_millis(379)), None);
            assert_eq!(repeat.poll(now + Duration::from_millis(380)), Some(button));
            let centered = axis(axis_id, 0);
            assert_eq!(repeat.event(&centered, None, now), None);
            assert_eq!(repeat.poll(now + Duration::from_secs(1)), None);
        }
    }

    #[test]
    fn l1_and_r1_wait_while_fn_is_held() {
        let shoulder = |button, down| {
            if down {
                Event::ControllerButtonDown {
                    timestamp: 0,
                    which: 1,
                    button,
                }
            } else {
                Event::ControllerButtonUp {
                    timestamp: 0,
                    which: 1,
                    button,
                }
            }
        };
        let now = Instant::now();
        let mut repeat = Repeat::default();
        // L1 already repeating when FN comes: the repeat stops.
        let l1 = shoulder(Pad::LeftShoulder, true);
        assert_eq!(repeat.event(&l1, map_event(&l1), now), Some(Button::L));
        repeat.modifier((1, 10), true);
        assert_eq!(repeat.poll(now + Duration::from_secs(1)), None);
        // R1 pressed with FN held: nothing, now or repeating.
        let r1 = shoulder(Pad::RightShoulder, true);
        assert_eq!(repeat.event(&r1, map_event(&r1), now), None);
        assert_eq!(repeat.poll(now + Duration::from_secs(2)), None);
        // Other buttons still work.
        let a = shoulder(Pad::A, true);
        assert_eq!(repeat.event(&a, map_event(&a), now), Some(Button::A));
        // All let go, FN up: R1 pages again.
        for (pad, _) in [(Pad::LeftShoulder, 0), (Pad::RightShoulder, 0), (Pad::A, 0)] {
            let up = shoulder(pad, false);
            repeat.event(&up, map_event(&up), now);
        }
        repeat.modifier((1, 10), false);
        assert_eq!(repeat.event(&r1, map_event(&r1), now), Some(Button::R));
    }

    fn shoulder(which: u32, button: Pad, down: bool) -> Event {
        if down {
            Event::ControllerButtonDown {
                timestamp: 0,
                which,
                button,
            }
        } else {
            Event::ControllerButtonUp {
                timestamp: 0,
                which,
                button,
            }
        }
    }

    #[test]
    fn releasing_l1_does_not_rearm_r1_while_fn_is_held() {
        let now = Instant::now();
        let mut repeat = Repeat::default();
        for event in [
            shoulder(1, Pad::LeftShoulder, true),
            shoulder(1, Pad::RightShoulder, true),
        ] {
            repeat.event(&event, map_event(&event), now);
        }
        repeat.modifier((1, 10), true);
        let up = shoulder(1, Pad::LeftShoulder, false);
        assert_eq!(repeat.event(&up, map_event(&up), now), None);
        assert_eq!(repeat.poll(now + Duration::from_secs(1)), None);
        assert_eq!(repeat.poll(now + Duration::from_secs(2)), None);
    }

    #[test]
    fn fn_on_one_pad_leaves_another_pads_shoulders_alone() {
        let now = Instant::now();
        let mut repeat = Repeat::default();
        repeat.modifier((1, 10), true);
        let r1 = shoulder(2, Pad::RightShoulder, true);
        assert_eq!(repeat.event(&r1, map_event(&r1), now), Some(Button::R));
        let l1 = shoulder(1, Pad::LeftShoulder, true);
        assert_eq!(repeat.event(&l1, map_event(&l1), now), None);
    }

    fn axis(axis: sdl2::controller::Axis, value: i16) -> Event {
        Event::ControllerAxisMotion {
            timestamp: 0,
            which: 1,
            axis,
            value,
        }
    }

    #[test]
    fn remaining_direction_repeats_after_either_source_is_released() {
        use sdl2::controller::Axis::{LeftX, LeftY};
        for (axis_id, pad, buttons) in [
            (LeftY, Pad::DPadDown, [Button::Up, Button::Down]),
            (LeftX, Pad::DPadDown, [Button::L, Button::Down]),
            (LeftY, Pad::DPadRight, [Button::Up, Button::R]),
        ] {
            for stick_first in [false, true] {
                for release_active in [false, true] {
                    let now = Instant::now();
                    let mut repeat = Repeat::default();
                    let pressed = [
                        axis(axis_id, -32768),
                        Event::ControllerButtonDown {
                            timestamp: 0,
                            which: 1,
                            button: pad,
                        },
                    ];
                    let released = [
                        axis(axis_id, 0),
                        Event::ControllerButtonUp {
                            timestamp: 0,
                            which: 1,
                            button: pad,
                        },
                    ];
                    let first = usize::from(!stick_first);
                    let second = 1 - first;
                    for (index, elapsed) in [(first, 0), (second, 100)] {
                        let event = &pressed[index];
                        let button = map_event(event);
                        assert_eq!(
                            repeat.event(event, button, now + Duration::from_millis(elapsed)),
                            Some(buttons[index])
                        );
                    }
                    let released_index = if release_active { second } else { first };
                    let remaining = 1 - released_index;
                    let release_time = now + Duration::from_millis(200);
                    let event = &released[released_index];
                    assert_eq!(repeat.event(event, map_event(event), release_time), None);
                    // Resumed input gets 300 ms; untouched active input keeps its deadline.
                    let deadline =
                        now + Duration::from_millis(if release_active { 500 } else { 400 });
                    let expected = Some(buttons[remaining]);
                    assert_eq!(repeat.poll(deadline - Duration::from_millis(1)), None);
                    assert_eq!(repeat.poll(deadline), expected);
                    assert_eq!(repeat.poll(deadline + Duration::from_millis(79)), None);
                    assert_eq!(repeat.poll(deadline + Duration::from_millis(80)), expected);
                    let event = &released[remaining];
                    assert_eq!(repeat.event(event, map_event(event), deadline), None);
                    assert_eq!(repeat.poll(deadline + Duration::from_secs(1)), None);
                }
            }
        }
    }

    #[test]
    fn stick_hysteresis_rearms_only_below_forty_percent() {
        let now = Instant::now();
        let mut repeat = Repeat::default();
        for (value, expected) in [
            (0, None),
            (-19660, None),
            (-19661, Some(Button::Up)),
            (-18000, None),
            (-13107, None),
            (-13106, None),
            (-19000, None),
            (-32768, Some(Button::Up)),
            (32767, Some(Button::Down)),
            (0, None),
        ] {
            let event = axis(sdl2::controller::Axis::LeftY, value);
            assert_eq!(
                repeat.event(&event, map_event(&event), now),
                expected,
                "value={value}"
            );
        }
        assert_eq!(repeat.poll(now + Duration::from_secs(1)), None);
    }

    #[test]
    fn hat_and_axis_share_one_press_and_repeat_deadline() {
        for axis_first in [false, true] {
            let now = Instant::now();
            let mut repeat = Repeat::default();
            let stick = Event::JoyAxisMotion {
                timestamp: 0,
                which: 1,
                axis_idx: 1,
                value: -32768,
            };
            let hat = Event::JoyHatMotion {
                timestamp: 0,
                which: 1,
                hat_idx: 0,
                state: HatState::Up,
            };
            let events = if axis_first {
                [&stick, &hat]
            } else {
                [&hat, &stick]
            };
            assert_eq!(
                repeat.event(events[0], map_event(events[0]), now),
                Some(Button::Up)
            );
            assert_eq!(
                repeat.event(
                    events[1],
                    map_event(events[1]),
                    now + Duration::from_millis(100)
                ),
                None
            );
            let centered = Event::JoyHatMotion {
                timestamp: 0,
                which: 1,
                hat_idx: 0,
                state: HatState::Centered,
            };
            assert_eq!(repeat.event(&centered, None, now), None);
            for axis_id in [
                sdl2::controller::Axis::LeftX,
                sdl2::controller::Axis::RightY,
            ] {
                let event = axis(axis_id, 0);
                assert_eq!(repeat.event(&event, map_event(&event), now), None);
            }
            assert_eq!(repeat.poll(now + Duration::from_millis(299)), None);
            assert_eq!(
                repeat.poll(now + Duration::from_millis(300)),
                Some(Button::Up)
            );
            let centered = Event::JoyAxisMotion {
                timestamp: 0,
                which: 1,
                axis_idx: 1,
                value: 0,
            };
            assert_eq!(repeat.event(&centered, None, now), None);
            assert_eq!(repeat.poll(now + Duration::from_secs(1)), None);
        }
    }
}
