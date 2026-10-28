//! Launch options each emulator and core accepts, from `es_features.cfg`.
//! Only the `features="a, b"` attributes matter here; per-feature choices
//! nested under `<features>` belong to individual emulators.
use crate::{systems::attribute, IndexError};
use archr_io::{read_bytes, Symlinks::Follow};
use quick_xml::{events::Event, Reader};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Features {
    /// (emulator, core) to its flags; the emulator's own flags use core "".
    flags: BTreeMap<(String, String), Vec<String>>,
}
impl Features {
    /// Declared on the emulator itself or on this core of it.
    pub fn has(&self, emulator: &str, core: &str, feature: &str) -> bool {
        ["", core].iter().any(|core| {
            self.flags
                .get(&(emulator.to_owned(), (*core).to_owned()))
                .is_some_and(|flags| flags.iter().any(|f| f == feature))
        })
    }
}

pub fn load_features(path: &Path) -> Result<Features, IndexError> {
    parse(&read_bytes(path, 1024 * 1024, Follow)?)
}

fn parse(source: &[u8]) -> Result<Features, IndexError> {
    let mut reader = Reader::from_reader(source);
    let mut buffer = Vec::new();
    let mut features = Features::default();
    let mut emulator: Option<String> = None;
    loop {
        let (element, open) = match reader.read_event_into(&mut buffer)? {
            Event::Start(element) => (element.into_owned(), true),
            Event::Empty(element) => (element.into_owned(), false),
            Event::End(end) if end.name().as_ref() == b"emulator" => {
                emulator = None;
                buffer.clear();
                continue;
            }
            Event::Eof => return Ok(features),
            _ => {
                buffer.clear();
                continue;
            }
        };
        let name = attribute(&reader, &element, b"name")?.unwrap_or_default();
        let flags: Vec<String> = attribute(&reader, &element, b"features")?
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|flag| !flag.is_empty())
            .map(str::to_owned)
            .collect();
        let key = match (element.name().as_ref(), &emulator) {
            (b"emulator", _) => {
                if open {
                    emulator = Some(name.clone());
                }
                Some((name, String::new()))
            }
            (b"core", Some(parent)) => Some((parent.clone(), name)),
            _ => None,
        };
        if let Some(key) = key.filter(|_| !flags.is_empty()) {
            features.flags.entry(key).or_default().extend(flags);
        }
        buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = r#"<?xml version="1.0"?>
<features>
  <emulator name="mednafen">
    <features><feature name="shader"><choice name="none"/></feature></features>
    <cores><core name="gb"><features><feature name="ratio"/></features></core></cores>
  </emulator>
  <emulator name="pico-8" features="pixel_perfect" />
  <emulator name="retroarch" features="decoration, ratio, smooth">
    <cores>
      <core name="snes9x" features="netplay, rewind, autosave, cheevos" />
      <core name="bsnes" features=" autosave" />
      <core name="flycast" features="netplay, autosave"><features><feature name="x"/></features></core>
      <core name="gpsp"/>
    </cores>
  </emulator>
  <core name="orphan" features="autosave"/>
</features>"#;
    #[test]
    fn emulator_flags_apply_to_every_core_and_core_flags_only_to_their_own() {
        let features = parse(SAMPLE.as_bytes()).unwrap();
        assert!(features.has("retroarch", "gpsp", "ratio"));
        assert!(features.has("retroarch", "gpsp", "smooth"));
        assert!(!features.has("retroarch", "gpsp", "autosave"));
        assert!(features.has("retroarch", "snes9x", "autosave"));
        assert!(features.has("retroarch", "bsnes", "autosave"));
        assert!(features.has("retroarch", "flycast", "autosave"));
        assert!(features.has("pico-8", "", "pixel_perfect"));
    }
    #[test]
    fn nested_feature_names_and_cores_outside_an_emulator_are_not_flags() {
        let features = parse(SAMPLE.as_bytes()).unwrap();
        assert!(!features.has("mednafen", "gb", "ratio"));
        assert!(!features.has("mednafen", "gb", "shader"));
        assert!(!features.has("", "orphan", "autosave"));
        // An empty emulator element does not capture the cores that follow it.
        assert!(!features.has("pico-8", "snes9x", "autosave"));
    }
    #[test]
    fn broken_xml_is_an_error() {
        assert!(parse(b"<features><emulator name=\"a\"></features>").is_err());
    }
    #[test]
    fn the_distribution_copy_parses() {
        let copy = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../distribution-wt-sem-es/projects/ArchR/packages/archr/config/emulationstation/es_features.cfg",
        );
        if !copy.is_file() {
            eprintln!("skipped: no distribution checkout at {}", copy.display());
            return;
        }
        let features = load_features(&copy).unwrap();
        assert!(features.has("retroarch", "snes9x", "ratio"));
        assert!(features.has("retroarch", "snes9x", "smooth"));
        assert!(features.has("retroarch", "snes9x", "autosave"));
        assert!(!features.has("mupen64plus", "glide64mk2", "autosave"));
    }
}
