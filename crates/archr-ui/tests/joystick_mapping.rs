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

#[test]
fn es_mapping_overrides_sdl_controller_by_guid_or_name_without_duplicates() {
    use archr_ui::state::Button;
    let mappings=RawMappings::parse(r#"<inputList><inputConfig type="joystick" deviceName="InputPlumber GameController" deviceGUID="0300f5a35e040000120b000001000000"><input name="a" type="button" id="1" value="1"/><input name="pageup" type="button" id="4" value="1"/><input name="pagedown" type="button" id="5" value="1"/><input name="up" type="hat" id="0" value="1"/></inputConfig></inputList>"#).unwrap();
    let raw = Event::JoyButtonDown {
        timestamp: 0,
        which: 1,
        button_idx: 1,
    };
    let native = Event::ControllerButtonDown {
        timestamp: 0,
        which: 1,
        button: sdl2::controller::Button::B,
    };
    let guid = "0300f5a35e040000120b000001000000";
    assert_eq!(
        mappings.map_device(guid, "different name", true, &raw),
        Some(Button::A)
    );
    assert_eq!(
        mappings.map_device(guid, "different name", true, &native),
        None
    );
    assert_eq!(
        mappings.map_device("other guid", "InputPlumber GameController", true, &raw),
        Some(Button::A)
    );
    assert_eq!(
        mappings.map_device("other guid", "other name", true, &native),
        Some(Button::B)
    );
    assert_eq!(
        mappings.map_device("other guid", "other name", true, &raw),
        None
    );
    assert_eq!(
        mappings.map_device(
            guid,
            "",
            true,
            &Event::JoyButtonDown {
                timestamp: 0,
                which: 1,
                button_idx: 4
            }
        ),
        Some(Button::L)
    );
    assert_eq!(
        mappings.map_device(
            guid,
            "",
            true,
            &Event::JoyButtonDown {
                timestamp: 0,
                which: 1,
                button_idx: 5
            }
        ),
        Some(Button::R)
    );
}

#[test]
fn matching_es_entry_without_bindings_does_not_fall_back_to_native_buttons() {
    let mappings=RawMappings::parse(r#"<inputList><inputConfig type="joystick" deviceName="Named pad" deviceGUID="guid"></inputConfig></inputList>"#).unwrap();
    let native = Event::ControllerButtonDown {
        timestamp: 0,
        which: 1,
        button: sdl2::controller::Button::A,
    };
    assert_eq!(
        mappings.map_device("guid", "Named pad", true, &native),
        None
    );
}
