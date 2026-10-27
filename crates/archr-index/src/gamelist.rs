use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Component, Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::IndexError;

#[derive(Debug, Default)]
pub(crate) struct Metadata {
    path: String,
    pub name: String,
    pub image: String,
    pub thumbnail: String,
    pub video: String,
    pub favorite: bool,
    pub last_played: Option<u64>,
}

impl Metadata {
    fn set(&mut self, field: &[u8], text: &str) {
        let text = text.trim();
        match field {
            b"path" => self.path = text.to_owned(),
            b"name" => self.name = text.to_owned(),
            b"image" => self.image = text.to_owned(),
            b"thumbnail" => self.thumbnail = text.to_owned(),
            b"video" => self.video = text.to_owned(),
            b"favorite" => self.favorite = text == "true",
            b"lastplayed" => self.last_played = parse_last_played(text),
            _ => {}
        }
    }
}

fn known_field(name: &[u8]) -> bool {
    matches!(
        name,
        b"path" | b"name" | b"image" | b"thumbnail" | b"video" | b"favorite" | b"lastplayed"
    )
}

/// Stream the complete document before returning any metadata. A parse error
/// discards every entry, including entries preceding the malformed section.
pub(crate) fn load_gamelist(system_dir: &Path) -> Result<HashMap<PathBuf, Metadata>, IndexError> {
    let mut reader =
        Reader::from_reader(BufReader::new(File::open(system_dir.join("gamelist.xml"))?));
    reader.config_mut().expand_empty_elements = true;
    let mut buffer = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut found_root = false;
    let mut entries = HashMap::new();
    let mut game = Metadata::default();
    let mut text = String::new();

    loop {
        // Only direct children of game records are fields. Unknown subtrees
        // are parsed for validity but never populate metadata.
        let in_game = stack.len() >= 2 && stack[1] == b"game";
        let in_field = in_game && stack.len() == 3 && known_field(&stack[2]);
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) => {
                for attribute in element.attributes() {
                    attribute
                        .map_err(quick_xml::Error::from)?
                        .decode_and_unescape_value(reader.decoder())?;
                }
                let name = element.name().as_ref().to_vec();
                if stack.is_empty() {
                    if found_root || name != b"gameList" {
                        return Err(IndexError::InvalidGamelist("expected one gameList root"));
                    }
                    found_root = true;
                } else if in_field {
                    return Err(IndexError::InvalidGamelist("expected a text field"));
                } else if stack.len() == 1 && name == b"game" {
                    game = Metadata::default();
                }
                if in_game && stack.len() == 2 {
                    text.clear();
                }
                stack.push(name);
            }
            Event::End(element) => {
                if stack.last().map(Vec::as_slice) != Some(element.name().as_ref()) {
                    return Err(IndexError::InvalidGamelist("unexpected closing tag"));
                }
                if in_field {
                    game.set(&stack[2], &text);
                } else if in_game && stack.len() == 2 && !game.path.is_empty() {
                    if let Some(path) = resolve_path(system_dir, &game.path)
                        .filter(|path| path.exists())
                        .and_then(|path| game_path_key(&path))
                    {
                        entries.insert(path, std::mem::take(&mut game));
                    }
                }
                stack.pop();
            }
            Event::Text(part) => {
                let part = part.unescape()?;
                if in_field {
                    text.push_str(&part);
                } else if stack.len() <= 1 && !part.trim().is_empty() {
                    return Err(IndexError::InvalidGamelist(
                        "unexpected text outside a record",
                    ));
                }
            }
            Event::CData(part) => {
                let part = part.decode().map_err(quick_xml::Error::from)?;
                if in_field {
                    text.push_str(&part);
                } else if stack.len() <= 1 {
                    return Err(IndexError::InvalidGamelist(
                        "unexpected CDATA outside a record",
                    ));
                }
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_) => {}
            Event::Eof if found_root && stack.is_empty() => return Ok(entries),
            _ => return Err(IndexError::InvalidGamelist("incomplete XML document")),
        }
        buffer.clear();
    }
}

pub(crate) fn resolve_path(system_dir: &Path, text: &str) -> Option<PathBuf> {
    if text.is_empty() {
        None
    } else if let Some(relative) = text.strip_prefix("~/") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(relative))
    } else {
        Some(system_dir.join(text.strip_prefix("./").unwrap_or(text)))
    }
}

// Normalize spelling, not filesystem identity: symlink aliases can have their
// own gamelist records even when they launch the same underlying ROM.
pub(crate) fn game_path_key(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    Some(normalized)
}

fn parse_last_played(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != 15
        || bytes[8] != b'T'
        || !bytes[..8].iter().chain(&bytes[9..]).all(u8::is_ascii_digit)
    {
        return None;
    }
    let year = text[..4].parse::<u64>().ok()?;
    let month = text[4..6].parse::<usize>().ok()?;
    let day = text[6..8].parse::<u64>().ok()?;
    let hour = text[9..11].parse::<u64>().ok()?;
    let minute = text[11..13].parse::<u64>().ok()?;
    let second = text[13..].parse::<u64>().ok()?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year < 1970
        || !(1..=12).contains(&month)
        || day == 0
        || day > month_days[month - 1]
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // Gregorian leap days before this year, minus those before the Unix epoch.
    let previous = year - 1;
    let leap_days = previous / 4 - previous / 100 + previous / 400 - 477;
    let days =
        (year - 1970) * 365 + leap_days + month_days[..month - 1].iter().sum::<u64>() + day - 1;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}
