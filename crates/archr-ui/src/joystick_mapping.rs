//! ES bindings for joystick devices lacking an SDL GameController mapping.
use crate::state::Button;
use quick_xml::{events::Event as Xml, Reader};
use sdl2::event::Event;
use std::collections::HashMap;

#[derive(Default)]
pub struct RawMappings(HashMap<String, Vec<Binding>>, HashMap<String, String>);
struct Binding {
    button: Button,
    kind: String,
    id: u8,
    value: i16,
}
impl RawMappings {
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut reader = Reader::from_str(source);
        let mut mappings = Self::default();
        let mut guid = None;
        loop {
            match reader.read_event().map_err(|error| error.to_string())? {
                Xml::Start(element) if element.name().as_ref() == b"inputConfig" => {
                    let attrs = attributes(&element)?;
                    guid = if attrs.get("type").is_some_and(|kind| kind == "joystick") {
                        attrs
                            .get("deviceGUID")
                            .filter(|guid| !guid.is_empty())
                            .cloned()
                            .or_else(|| attrs.get("deviceName").map(|name| format!("name:{name}")))
                    } else {
                        None
                    };
                    if let Some(key) = &guid {
                        mappings.0.entry(key.clone()).or_default();
                    }
                    if let (Some(name), Some(key)) = (attrs.get("deviceName"), &guid) {
                        mappings.1.insert(name.clone(), key.clone());
                    }
                }
                Xml::Empty(element) | Xml::Start(element)
                    if element.name().as_ref() == b"input" =>
                {
                    let Some(guid) = &guid else {
                        continue;
                    };
                    let attrs = attributes(&element)?;
                    let Some(button) = attrs.get("name").and_then(|name| match name.as_str() {
                        "a" => Some(Button::A),
                        "b" => Some(Button::B),
                        "x" => Some(Button::X),
                        "y" => Some(Button::Y),
                        "leftshoulder" | "left" | "pageup" => Some(Button::L),
                        "rightshoulder" | "right" | "pagedown" => Some(Button::R),
                        "up" => Some(Button::Up),
                        "down" => Some(Button::Down),
                        "start" => Some(Button::Start),
                        "select" => Some(Button::Select),
                        _ => None,
                    }) else {
                        continue;
                    };
                    if let (Some(kind), Some(id), Some(value)) = (
                        attrs.get("type"),
                        attrs.get("id").and_then(|id| id.parse().ok()),
                        attrs.get("value").and_then(|value| value.parse().ok()),
                    ) {
                        mappings.0.entry(guid.clone()).or_default().push(Binding {
                            button,
                            kind: kind.clone(),
                            id,
                            value,
                        });
                    }
                }
                Xml::End(element) if element.name().as_ref() == b"inputConfig" => guid = None,
                Xml::Eof => return Ok(mappings),
                _ => {}
            }
        }
    }
    fn key<'a>(&'a self, guid: &'a str, name: &str) -> Option<&'a str> {
        if self.0.contains_key(guid) {
            Some(guid)
        } else {
            self.1
                .get(name)
                .filter(|key| self.0.contains_key(*key))
                .map(String::as_str)
        }
    }
    pub fn contains_device(&self, guid: &str, name: &str) -> bool {
        self.key(guid, name).is_some()
    }
    /// ES takes precedence, including on SDL GameControllers. Exactly one event
    /// stream is accepted for each device, so SDL's raw/native duplicates vanish.
    pub fn map_device(
        &self,
        guid: &str,
        name: &str,
        controller: bool,
        event: &Event,
    ) -> Option<Button> {
        let raw = matches!(
            event,
            Event::JoyButtonDown { .. }
                | Event::JoyButtonUp { .. }
                | Event::JoyAxisMotion { .. }
                | Event::JoyHatMotion { .. }
        );
        if let Some(key) = self.key(guid, name) {
            if raw {
                self.map(key, event)
            } else {
                None
            }
        } else if raw != controller {
            crate::input::map_event(event)
        } else {
            None
        }
    }
    pub fn contains(&self, guid: &str) -> bool {
        self.0.contains_key(guid)
    }
    pub fn map(&self, guid: &str, event: &Event) -> Option<Button> {
        self.0
            .get(guid)?
            .iter()
            .find(|binding| match *event {
                Event::JoyButtonDown { button_idx, .. } | Event::JoyButtonUp { button_idx, .. } => {
                    binding.kind == "button" && binding.id == button_idx
                }
                Event::JoyAxisMotion {
                    axis_idx, value, ..
                } => {
                    binding.kind == "axis"
                        && binding.id == axis_idx
                        && ((binding.value < 0 && value < -16000)
                            || (binding.value > 0 && value > 16000))
                }
                Event::JoyHatMotion { hat_idx, state, .. } => {
                    binding.kind == "hat"
                        && binding.id == hat_idx
                        && (state.to_raw() as i16 & binding.value) != 0
                }
                _ => false,
            })
            .map(|binding| binding.button)
    }
}
fn attributes(
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<HashMap<String, String>, String> {
    element
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(|error| error.to_string())?;
            Ok((
                String::from_utf8_lossy(attribute.key.as_ref()).into_owned(),
                attribute
                    .unescape_value()
                    .map_err(|error| error.to_string())?
                    .into_owned(),
            ))
        })
        .collect()
}
