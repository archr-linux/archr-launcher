use archr_io::{read_bytes, Symlinks::Follow};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::IndexError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemDef {
    pub name: String,
    pub fullname: String,
    pub manufacturer: String,
    pub release: String,
    pub path: PathBuf,
    pub extensions: Vec<String>,
    pub emulators: Vec<Emulator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emulator {
    pub name: String,
    pub cores: Vec<Core>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Core {
    pub name: String,
    pub default: bool,
}

impl SystemDef {
    /// Select the first explicitly default core, or the first available pair.
    /// Systems without any cores have no default pair.
    pub fn default_emulator_core(&self) -> Option<(&str, &str)> {
        let mut pairs = self.emulators.iter().flat_map(|emulator| {
            emulator
                .cores
                .iter()
                .map(move |core| (emulator.name.as_str(), core))
        });
        pairs
            .clone()
            .find(|(_, core)| core.default)
            .or_else(|| pairs.next())
            .map(|(emulator, core)| (emulator, core.name.as_str()))
    }
}

/// Read EmulationStation's system list without building an XML DOM.
/// Unknown elements are skipped, preserving the order of systems and cores.
pub fn load_systems(es_systems: &Path) -> Result<Vec<SystemDef>, IndexError> {
    let source = read_bytes(es_systems, 1024 * 1024, Follow)?;
    let mut reader = Reader::from_reader(source.as_slice());
    reader.config_mut().expand_empty_elements = true;
    let mut buffer = Vec::new();
    let mut systems = Vec::new();
    let mut found_list = false;

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) if element.name().as_ref() == b"systemList" && !found_list => {
                found_list = true;
                read_children(&mut reader, b"systemList", |reader, child| {
                    if child.name().as_ref() == b"system" {
                        systems.push(read_system(reader)?);
                        Ok(())
                    } else {
                        skip_element(reader, child)
                    }
                })?;
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_) => {}
            Event::Text(text) if text.unescape()?.trim().is_empty() => {}
            Event::Eof if found_list => return Ok(systems),
            _ => return Err(IndexError::InvalidSystems("expected one systemList root")),
        }
        buffer.clear();
    }
}

fn read_system<R: BufRead>(reader: &mut Reader<R>) -> Result<SystemDef, IndexError> {
    let mut system = SystemDef::default();
    read_children(reader, b"system", |reader, child| {
        match child.name().as_ref() {
            b"name" => system.name = read_text(reader)?,
            b"fullname" => system.fullname = read_text(reader)?,
            b"manufacturer" => system.manufacturer = read_text(reader)?,
            b"release" => system.release = read_text(reader)?,
            b"path" => system.path = PathBuf::from(read_text(reader)?),
            b"extension" => {
                system.extensions = read_text(reader)?
                    .split_whitespace()
                    .map(str::to_lowercase)
                    .collect();
            }
            b"emulators" => read_children(reader, b"emulators", |reader, child| {
                if child.name().as_ref() == b"emulator" {
                    system.emulators.push(read_emulator(reader, child)?);
                    Ok(())
                } else {
                    skip_element(reader, child)
                }
            })?,
            _ => skip_element(reader, child)?,
        }
        Ok(())
    })?;
    Ok(system)
}

fn read_emulator<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart<'_>,
) -> Result<Emulator, IndexError> {
    let mut emulator = Emulator {
        name: attribute(reader, element, b"name")?.unwrap_or_default(),
        cores: Vec::new(),
    };
    read_children(reader, b"emulator", |reader, child| {
        if child.name().as_ref() == b"cores" {
            read_children(reader, b"cores", |reader, child| {
                if child.name().as_ref() == b"core" {
                    let default = attribute(reader, child, b"default")?.as_deref() == Some("true");
                    emulator.cores.push(Core {
                        name: read_text(reader)?,
                        default,
                    });
                    Ok(())
                } else {
                    skip_element(reader, child)
                }
            })
        } else {
            skip_element(reader, child)
        }
    })?;
    Ok(emulator)
}

fn attribute<R: BufRead>(
    reader: &Reader<R>,
    element: &BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, IndexError> {
    element
        .try_get_attribute(name)
        .map_err(quick_xml::Error::from)?
        .map(|attr| {
            attr.decode_and_unescape_value(reader.decoder())
                .map(|value| value.into_owned())
                .map_err(IndexError::from)
        })
        .transpose()
}

// Each visitor consumes its entire child, including the matching end event.
fn read_children<R: BufRead>(
    reader: &mut Reader<R>,
    parent: &[u8],
    mut visit: impl FnMut(&mut Reader<R>, &BytesStart<'_>) -> Result<(), IndexError>,
) -> Result<(), IndexError> {
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(child) => visit(reader, &child)?,
            Event::End(end) if end.name().as_ref() == parent => return Ok(()),
            Event::Comment(_) | Event::PI(_) => {}
            Event::Text(text) if text.unescape()?.trim().is_empty() => {}
            _ => {
                return Err(IndexError::InvalidSystems(
                    "unexpected content or end of XML",
                ))
            }
        }
        buffer.clear();
    }
}

fn read_text<R: BufRead>(reader: &mut Reader<R>) -> Result<String, IndexError> {
    let mut buffer = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Text(part) => text.push_str(&part.unescape()?),
            Event::CData(part) => text.push_str(&part.decode().map_err(quick_xml::Error::from)?),
            Event::Comment(_) | Event::PI(_) => {}
            Event::End(_) => return Ok(text.trim().to_owned()),
            _ => {
                return Err(IndexError::InvalidSystems(
                    "expected text and a closing tag",
                ))
            }
        }
        buffer.clear();
    }
}

fn skip_element<R: BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart<'_>,
) -> Result<(), IndexError> {
    reader.read_to_end_into(element.name(), &mut Vec::new())?;
    Ok(())
}
