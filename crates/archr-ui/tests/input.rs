use archr_ui::{
    input::{map_event, Repeat},
    state::Button,
};
use sdl2::{controller::Button as Pad, event::Event, keyboard::Keycode};
use std::time::{Duration, Instant};

#[test]
fn keyboard_controller_and_raw_events_map() {
    let key = Event::KeyDown {
        timestamp: 0,
        window_id: 0,
        keycode: Some(Keycode::Return),
        scancode: None,
        keymod: sdl2::keyboard::Mod::NOMOD,
        repeat: false,
    };
    assert_eq!(map_event(&key), Some(Button::A));
    assert_eq!(
        map_event(&Event::ControllerButtonDown {
            timestamp: 0,
            which: 1,
            button: Pad::Back
        }),
        Some(Button::Select)
    );
    assert_eq!(
        map_event(&Event::JoyButtonDown {
            timestamp: 0,
            which: 1,
            button_idx: 0
        }),
        Some(Button::A)
    );
    assert_eq!(
        map_event(&Event::JoyHatMotion {
            timestamp: 0,
            which: 1,
            hat_idx: 0,
            state: sdl2::joystick::HatState::Down
        }),
        Some(Button::Down)
    );
}

#[test]
fn held_navigation_repeats_after_300_then_80_ms() {
    let now = Instant::now();
    let mut repeat = Repeat::default();
    repeat.press(Button::Down, now);
    assert_eq!(repeat.poll(now + Duration::from_millis(299)), None);
    assert_eq!(
        repeat.poll(now + Duration::from_millis(300)),
        Some(Button::Down)
    );
    assert_eq!(repeat.poll(now + Duration::from_millis(379)), None);
    assert_eq!(
        repeat.poll(now + Duration::from_millis(380)),
        Some(Button::Down)
    );
    repeat.release(Button::Down);
    assert_eq!(repeat.poll(now + Duration::from_secs(1)), None);
}
