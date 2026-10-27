//! SDL event translation and deterministic held-direction repeat.
use crate::{controllers::Controllers, state::Button};
use sdl2::{controller::Button as Pad, event::Event, joystick::HatState, keyboard::Keycode};
use std::time::{Duration, Instant};

pub fn map_event(e: &Event) -> Option<Button> {
    match *e {
        Event::KeyDown {
            keycode: Some(key),
            repeat: false,
            ..
        }
        | Event::KeyUp {
            keycode: Some(key), ..
        } => match key {
            Keycode::Return => Some(Button::A),
            Keycode::Backspace => Some(Button::B),
            Keycode::X => Some(Button::X),
            Keycode::Y => Some(Button::Y),
            Keycode::PageUp | Keycode::Left => Some(Button::L),
            Keycode::PageDown | Keycode::Right => Some(Button::R),
            Keycode::Up => Some(Button::Up),
            Keycode::Down => Some(Button::Down),
            _ => None,
        },
        Event::ControllerButtonDown { button, .. } | Event::ControllerButtonUp { button, .. } => {
            match button {
                Pad::A => Some(Button::A),
                Pad::B => Some(Button::B),
                Pad::X => Some(Button::X),
                Pad::Y => Some(Button::Y),
                Pad::LeftShoulder | Pad::DPadLeft => Some(Button::L),
                Pad::RightShoulder | Pad::DPadRight => Some(Button::R),
                Pad::DPadUp => Some(Button::Up),
                Pad::DPadDown => Some(Button::Down),
                Pad::Start => Some(Button::Start),
                Pad::Back => Some(Button::Select),
                _ => None,
            }
        }
        // Conventional SDL joystick fallback, only used for unmapped devices.
        Event::JoyButtonDown { button_idx, .. } | Event::JoyButtonUp { button_idx, .. } => {
            match button_idx {
                0 => Some(Button::A),
                1 => Some(Button::B),
                2 => Some(Button::X),
                3 => Some(Button::Y),
                4 => Some(Button::L),
                5 => Some(Button::R),
                6 => Some(Button::Select),
                7 => Some(Button::Start),
                _ => None,
            }
        }
        Event::JoyHatMotion { state, .. } => match state {
            HatState::Up | HatState::LeftUp | HatState::RightUp => Some(Button::Up),
            HatState::Down | HatState::LeftDown | HatState::RightDown => Some(Button::Down),
            HatState::Left => Some(Button::L),
            HatState::Right => Some(Button::R),
            _ => None,
        },
        Event::JoyAxisMotion {
            axis_idx: 1, value, ..
        }
        | Event::ControllerAxisMotion {
            axis: sdl2::controller::Axis::LeftY,
            value,
            ..
        } => {
            if value < -16000 {
                Some(Button::Up)
            } else if value > 16000 {
                Some(Button::Down)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Default)]
pub struct Repeat {
    held: Option<(Button, Instant)>,
}
impl Repeat {
    pub fn press(&mut self, button: Button, now: Instant) {
        if matches!(button, Button::Up | Button::Down) && self.held.map(|(b, _)| b) != Some(button)
        {
            self.held = Some((button, now + Duration::from_millis(300)));
        }
    }
    pub fn is_held(&self, button: Button) -> bool {
        self.held.map(|(held, _)| held) == Some(button)
    }
    pub fn release(&mut self, button: Button) {
        if self.held.map(|(b, _)| b) == Some(button) {
            self.held = None;
        }
    }
    pub fn clear(&mut self) {
        self.held = None;
    }
    pub fn poll(&mut self, now: Instant) -> Option<Button> {
        let (button, deadline) = self.held?;
        if now < deadline {
            return None;
        }
        self.held = Some((button, now + Duration::from_millis(80)));
        Some(button)
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
    match *event {
        Event::ControllerButtonDown { which, .. }
        | Event::ControllerButtonUp { which, .. }
        | Event::ControllerAxisMotion { which, .. }
            if !controllers.accepts_controller(which) =>
        {
            return None
        }
        Event::JoyButtonDown { which, .. }
        | Event::JoyButtonUp { which, .. }
        | Event::JoyHatMotion { which, .. }
        | Event::JoyAxisMotion { which, .. }
            if !controllers.accepts_raw(which) =>
        {
            return None
        }
        Event::KeyUp { .. } | Event::ControllerButtonUp { .. } | Event::JoyButtonUp { .. } => {
            if let Some(button) = mapped_button(event, controllers) {
                repeat.release(button);
            }
            return None;
        }
        Event::JoyHatMotion {
            state: sdl2::joystick::HatState::Centered,
            ..
        }
        | Event::JoyAxisMotion {
            value: -16000..=16000,
            ..
        }
        | Event::ControllerAxisMotion {
            axis: sdl2::controller::Axis::LeftY,
            value: -16000..=16000,
            ..
        } => {
            repeat.clear();
            return None;
        }
        _ => {}
    }
    let button = mapped_button(event, controllers)?;
    if repeat.is_held(button) {
        return None;
    }
    repeat.press(button, Instant::now());
    Some(button)
}

fn mapped_button(event: &Event, controllers: &Controllers) -> Option<Button> {
    match event {
        Event::JoyButtonDown { .. }
        | Event::JoyButtonUp { .. }
        | Event::JoyHatMotion { .. }
        | Event::JoyAxisMotion { .. }
        | Event::ControllerButtonDown { .. }
        | Event::ControllerButtonUp { .. }
        | Event::ControllerAxisMotion { .. } => controllers.map_device_event(event),
        _ => map_event(event),
    }
}
