use archr_ui::{joystick_mapping::RawMappings, state::Button};
use sdl2::event::Event;
#[test]
fn unmapped_pad_uses_es_button_axis_and_hat_bindings() {
    let mappings = RawMappings::parse(r#"<inputList><inputConfig type="joystick" deviceGUID="abcd"><input name="a" type="button" id="6" value="1"/><input name="up" type="axis" id="1" value="-1"/><input name="down" type="hat" id="0" value="4"/></inputConfig></inputList>"#).unwrap();
    assert!(mappings.contains("abcd"));
    assert_eq!(
        mappings.map(
            "abcd",
            &Event::JoyButtonDown {
                timestamp: 0,
                which: 7,
                button_idx: 6
            }
        ),
        Some(Button::A)
    );
    assert_eq!(
        mappings.map(
            "abcd",
            &Event::JoyButtonDown {
                timestamp: 0,
                which: 7,
                button_idx: 0
            }
        ),
        None
    );
    assert_eq!(
        mappings.map(
            "abcd",
            &Event::JoyAxisMotion {
                timestamp: 0,
                which: 7,
                axis_idx: 1,
                value: -20000
            }
        ),
        Some(Button::Up)
    );
    assert_eq!(
        mappings.map(
            "abcd",
            &Event::JoyHatMotion {
                timestamp: 0,
                which: 7,
                hat_idx: 0,
                state: sdl2::joystick::HatState::Down
            }
        ),
        Some(Button::Down)
    );
}
#[test]
fn controller_arguments_preserve_es_spaces_and_device_indices() {
    assert_eq!(
        archr_ui::controllers::controller_arguments([(2, "abc".into()), (5, "def".into())]),
        " -p1index 2 -p1guid abc  -p2index 5 -p2guid def "
    );
}
