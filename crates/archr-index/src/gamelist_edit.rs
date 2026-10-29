//! Merge scraped fields into a system's gamelist.xml in ES's format.
//!
//! Everything the update does not touch is kept byte for byte: other games,
//! unknown tags, attributes, folders and comments. Only indentation between
//! elements is normalized. A gamelist that does not parse is never replaced.
use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::Mutex,
};

use archr_io::{atomic_publish, invalid_data, read_bytes, Symlinks::Follow};
use quick_xml::{escape::escape, events::Event, Reader};

use crate::gamelist::{game_path_key, resolve_path};

/// Tags whose text is a path: kept only while the file they name exists.
const PATH_TAGS: &[&str] = &[
    "image",
    "thumbnail",
    "marquee",
    "video",
    "titleshot",
    "manual",
    "boxart",
    "mix",
];
const LIMIT: u64 = 32 << 20;

/// New values for one ROM, as (ES tag, text). Path values are absolute.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameUpdate {
    pub rom: PathBuf,
    pub fields: Vec<(&'static str, String)>,
}

/// Fields already holding a value (an existing file, for paths) are kept
/// unless `overwrite`: the user may have edited them.
pub fn update_gamelist(
    system_dir: &Path,
    updates: &[GameUpdate],
    overwrite: bool,
) -> io::Result<()> {
    // One writer per process: scraper threads may finish games of one system together.
    static WRITE: Mutex<()> = Mutex::new(());
    let _one = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let path = system_dir.join("gamelist.xml");
    let (source, mode) = match read_bytes(&path, LIMIT, Follow) {
        Ok(bytes) => {
            let mode = fs::metadata(&path)?.permissions().mode() & 0o7777;
            (
                String::from_utf8(bytes).map_err(|_| invalid_data("gamelist is not UTF-8"))?,
                mode,
            )
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => (
            "<?xml version=\"1.0\"?>\n<gameList>\n</gameList>\n".into(),
            0o644,
        ),
        Err(error) => return Err(error),
    };
    let mut doc = Doc::parse(&source).ok_or_else(|| invalid_data("gamelist does not parse"))?;
    for update in updates {
        let key = game_path_key(&update.rom);
        let found = doc.items.iter_mut().find_map(|item| match item {
            Item::Game(game) if key.is_some() && game.path(system_dir) == key => Some(game),
            _ => None,
        });
        let game = match found {
            Some(game) => game,
            None => {
                let relative = relative(system_dir, &update.rom);
                doc.items.push(Item::Game(Game {
                    open: "<game>".into(),
                    children: vec![Child::new("path", &relative)],
                }));
                let Some(Item::Game(game)) = doc.items.last_mut() else {
                    unreachable!()
                };
                game
            }
        };
        for (tag, value) in &update.fields {
            if value.trim().is_empty() {
                continue;
            }
            let value = if PATH_TAGS.contains(tag) {
                relative(system_dir, Path::new(value))
            } else {
                value.clone()
            };
            match game.children.iter_mut().find(|c| c.tag == *tag) {
                Some(child) if !overwrite && child.occupied(system_dir) => {}
                Some(child) => *child = Child::new(tag, &value),
                None => game.children.push(Child::new(tag, &value)),
            }
        }
    }
    let text = doc.render();
    atomic_publish(&path, true, |file| {
        use io::Write;
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        file.write_all(text.as_bytes())
    })
}

/// `./...` for files inside the system folder, as ES writes them.
fn relative(system_dir: &Path, path: &Path) -> String {
    path.strip_prefix(system_dir).map_or_else(
        |_| path.display().to_string(),
        |rest| format!("./{}", rest.display()),
    )
}

struct Doc {
    head: String,
    items: Vec<Item>,
    tail: String,
}

enum Item {
    Raw(String),
    Game(Game),
}

struct Game {
    /// The original start tag, attributes included.
    open: String,
    children: Vec<Child>,
}

struct Child {
    tag: String,
    raw: String,
    text: String,
}

impl Child {
    fn new(tag: &str, text: &str) -> Self {
        Self {
            tag: tag.into(),
            raw: format!("<{tag}>{}</{tag}>", escape(text)),
            text: text.into(),
        }
    }
    fn occupied(&self, system_dir: &Path) -> bool {
        let text = self.text.trim();
        !text.is_empty()
            && (!PATH_TAGS.contains(&self.tag.as_str())
                || resolve_path(system_dir, text).is_some_and(|path| path.is_file()))
    }
}

impl Game {
    fn path(&self, system_dir: &Path) -> Option<PathBuf> {
        let child = self.children.iter().find(|c| c.tag == "path")?;
        game_path_key(&resolve_path(system_dir, child.text.trim())?)
    }
}

impl Doc {
    /// `None` for anything that is not one complete gameList document.
    fn parse(source: &str) -> Option<Self> {
        let mut reader = Reader::from_str(source);
        reader.config_mut().check_comments = true;
        let at = |reader: &Reader<&[u8]>| reader.buffer_position() as usize;
        let blank = |t: &[u8]| t.iter().all(u8::is_ascii_whitespace);
        let (mut head, mut tail) = (None, None);
        let mut items = Vec::new();
        let mut game: Option<Game> = None;
        loop {
            let start = at(&reader);
            let event = reader.read_event().ok()?;
            let raw = source[start..at(&reader)].to_owned();
            let open = head.is_some() && tail.is_none();
            match (&mut game, event) {
                (_, Event::Eof) => break,
                (_, Event::Text(t)) if blank(&t) => {}
                (None, Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_))
                    if !open => {}
                (None, Event::Start(e)) if head.is_none() && e.name().as_ref() == b"gameList" => {
                    head = Some(source[..at(&reader)].trim_end().to_owned())
                }
                (None, Event::Empty(e)) if head.is_none() && e.name().as_ref() == b"gameList" => {
                    head = Some(format!("{}<gameList>", &source[..start]).trim().to_owned());
                    tail = Some("</gameList>".to_owned());
                }
                _ if !open => return None,
                // Inside a game: every direct child is kept as written.
                (Some(g), Event::Start(e)) => {
                    let inner = reader.read_text(e.name()).ok()?;
                    g.children.push(Child {
                        tag: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
                        text: quick_xml::escape::unescape(&inner)
                            .map_or_else(|_| String::new(), |t| t.into_owned()),
                        raw: source[start..at(&reader)].to_owned(),
                    });
                }
                (Some(g), Event::Empty(e)) => g.children.push(Child {
                    tag: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
                    raw,
                    text: String::new(),
                }),
                (Some(g), Event::Comment(_) | Event::PI(_)) => g.children.push(Child {
                    tag: String::new(),
                    raw,
                    text: String::new(),
                }),
                (Some(_), Event::End(e)) if e.name().as_ref() == b"game" => {
                    items.push(Item::Game(game.take()?))
                }
                (Some(_), _) => return None,
                // Directly under gameList.
                (None, Event::Start(e)) if e.name().as_ref() == b"game" => {
                    game = Some(Game {
                        open: raw,
                        children: Vec::new(),
                    })
                }
                (None, Event::Start(e)) => {
                    reader.read_to_end(e.name()).ok()?;
                    items.push(Item::Raw(source[start..at(&reader)].to_owned()));
                }
                (None, Event::Empty(_) | Event::Comment(_) | Event::PI(_)) => {
                    items.push(Item::Raw(raw))
                }
                (None, Event::End(_)) => tail = Some(source[start..].trim().to_owned()),
                (None, _) => return None,
            }
        }
        Some(Self {
            head: head?,
            items,
            tail: tail?,
        })
    }

    fn render(&self) -> String {
        let mut out = format!("{}\n", self.head);
        for item in &self.items {
            match item {
                Item::Raw(raw) => out += &format!("\t{raw}\n"),
                Item::Game(game) => {
                    out += &format!("\t{}\n", game.open);
                    for child in &game.children {
                        out += &format!("\t\t{}\n", child.raw);
                    }
                    out += "\t</game>\n";
                }
            }
        }
        out + &self.tail + "\n"
    }
}
