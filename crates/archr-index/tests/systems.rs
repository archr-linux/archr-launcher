use std::fs;
use std::path::{Path, PathBuf};

use archr_index::{load_systems, IndexError};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/systems.cfg")
}

#[test]
fn parses_three_systems() {
    let systems = load_systems(&fixture()).unwrap();

    assert_eq!(systems.len(), 3);
    assert_eq!(systems[0].name, "3do");
    assert_eq!(systems[0].fullname, "3DO");
    assert_eq!(systems[0].path, Path::new("/storage/roms/3do"));
    assert_eq!(systems[1].name, "amiga");
    assert_eq!(systems[1].fullname, "Amiga");
    assert_eq!(systems[1].path, Path::new("/storage/roms/amiga"));
    assert_eq!(systems[1].emulators.len(), 2);
    assert_eq!(systems[1].emulators[0].name, "retroarch");
    assert_eq!(systems[1].emulators[0].cores[0].name, "puae");
    assert_eq!(systems[1].emulators[0].cores[1].name, "uae4arm");
    assert_eq!(systems[1].emulators[1].name, "amiberry");
    assert_eq!(systems[2].name, "nes");
    assert_eq!(systems[2].fullname, "Nintendo Entertainment System");
    assert_eq!(systems[2].path, Path::new("/storage/roms/nes"));
}

#[test]
fn extensions_lowercased_and_split() {
    let systems = load_systems(&fixture()).unwrap();

    assert_eq!(systems[0].extensions, [".iso", ".bin", ".chd", ".cue"]);
    assert_eq!(systems[1].extensions, [".zip", ".adf", ".uae", ".lha"]);
    assert!(systems[2].extensions.iter().all(|ext| ext.starts_with('.')));
}

#[test]
fn default_core_is_flagged_one() {
    let systems = load_systems(&fixture()).unwrap();

    assert!(systems[0].emulators[0].cores[0].default);
    assert!(!systems[1].emulators[0].cores[0].default);
    assert!(!systems[1].emulators[0].cores[1].default);
    assert!(systems[1].emulators[1].cores[0].default);
    assert_eq!(
        systems[1].default_emulator_core(),
        Some(("amiberry", "amiberry"))
    );
}

#[test]
fn missing_file_is_error_not_panic() {
    let directory = tempfile::tempdir().unwrap();
    let result = load_systems(&directory.path().join("missing.cfg"));

    assert!(matches!(result, Err(IndexError::Io(_))));
}

#[test]
fn no_flagged_default_falls_back_to_first_emulator_and_core() {
    let systems = load_systems(&fixture()).unwrap();

    assert_eq!(
        systems[2].default_emulator_core(),
        Some(("retroarch", "fceumm"))
    );
    assert!(systems[2].emulators[0]
        .cores
        .iter()
        .all(|core| !core.default));
}

#[test]
fn no_available_core_returns_none() {
    for emulators in [
        "<emulators/>",
        "<emulators><emulator name=\"standalone\"><cores/></emulator></emulators>",
    ] {
        let systems = parse_xml(&format!(
            "<systemList><system><name>ports</name>{emulators}</system></systemList>"
        ))
        .unwrap();

        assert_eq!(systems[0].default_emulator_core(), None);
    }
}

#[test]
fn text_and_attributes_are_unescaped_and_cdata_is_preserved() {
    let systems = parse_xml(
        r#"<systemList><system>
        <name>demo</name>
        <fullname> Café &amp; <![CDATA[Games & More]]> &#x26; Test </fullname>
        <path>/storage/roms/a&amp;b</path>
        <extension><![CDATA[.ROM .Zip]]></extension>
        <emulators><emulator name="stand&amp;alone"><cores>
        <core default="true"> core&amp;one </core>
        </cores></emulator></emulators>
        </system></systemList>"#,
    )
    .unwrap();

    assert_eq!(systems[0].fullname, "Café & Games & More & Test");
    assert_eq!(systems[0].path, Path::new("/storage/roms/a&b"));
    assert_eq!(systems[0].extensions, [".rom", ".zip"]);
    assert_eq!(
        systems[0].default_emulator_core(),
        Some(("stand&alone", "core&one"))
    );
}

#[test]
fn unknown_elements_do_not_leak_into_system_fields() {
    let systems = parse_xml(
        r#"<systemList>
        <metadata><system><name>not-a-system</name></system></metadata>
        <system><name>nes</name>
        <metadata><name>wrong-name</name><emulators><emulator name="wrong"/>
        </emulators></metadata>
        <emulators><emulator name="retroarch"><cores>
        <metadata><core default="true">wrong-core</core></metadata>
        <core>fceumm</core></cores></emulator></emulators>
        </system></systemList>"#,
    )
    .unwrap();

    assert_eq!(systems.len(), 1);
    assert_eq!(systems[0].name, "nes");
    assert_eq!(systems[0].emulators[0].cores.len(), 1);
    assert_eq!(
        systems[0].default_emulator_core(),
        Some(("retroarch", "fceumm"))
    );
}

#[test]
fn malformed_or_incomplete_xml_returns_error() {
    for xml in [
        "",
        "<wrongRoot/>",
        "<systemList><system><name>nes</name></system>",
        "<systemList><system></systemList>",
        "<systemList/><systemList/>",
        "<systemList>unexpected text</systemList>",
        "<systemList><system><name>&undefined;</name></system></systemList>",
        "<systemList><system><emulators><emulator name=unquoted/></emulators></system></systemList>",
    ] {
        assert!(parse_xml(xml).is_err(), "accepted invalid XML: {xml}");
    }
}

#[test]
fn empty_system_list_is_valid() {
    assert!(parse_xml("<systemList/>").unwrap().is_empty());
}

fn parse_xml(xml: &str) -> Result<Vec<archr_index::SystemDef>, IndexError> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("systems.cfg");
    std::fs::write(&path, xml).unwrap();
    load_systems(&path)
}

#[test]
fn systems_xml_rejects_bytes_beyond_limit_instead_of_truncating() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("systems.xml");
    let mut source = "<systemList/>".to_owned();
    source.extend(std::iter::repeat_n(' ', 1024 * 1024 - source.len()));
    fs::write(&path, &source).unwrap();
    assert!(load_systems(&path).is_ok());
    source.push(' ');
    fs::write(&path, source).unwrap();
    assert!(load_systems(&path).is_err());
}
