use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::gamelist::{game_path_key, load_gamelist, resolve_path};
use crate::SystemDef;

#[cfg(test)]
std::thread_local! {
    static MEDIA_DIRECTORY_READS: std::cell::RefCell<std::collections::HashMap<PathBuf, usize>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Game {
    pub path: PathBuf,
    pub name: String,
    pub image: Option<PathBuf>,
    pub video: Option<PathBuf>,
    pub favorite: bool,
    pub last_played: Option<u64>,
    pub year: Option<u16>,
    pub players: Option<String>,
    pub details: Details,
}

/// Scraped text and art shown on the game details screen (gamelist.xml).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Details {
    pub description: String,
    pub genre: String,
    pub developer: String,
    pub publisher: String,
    /// 0-100, from the gamelist's 0-1 rating.
    pub rating: Option<u8>,
    pub screenshot: Option<PathBuf>,
    pub logo: Option<PathBuf>,
}

/// Remove balanced region/revision groups, retaining the stem if only tags remain.
pub fn clean_name(file_stem: &str) -> String {
    let mut cleaned = String::new();
    let mut brackets = Vec::new();
    let mut group_start = 0;
    let mut copied_until = 0;
    for (offset, character) in file_stem.char_indices() {
        match character {
            '(' | '[' => {
                if brackets.is_empty() {
                    group_start = offset;
                }
                brackets.push(character);
            }
            ')' | ']' => {
                let opening = if character == ')' { '(' } else { '[' };
                if brackets.last() == Some(&opening) {
                    brackets.pop();
                    if brackets.is_empty() {
                        cleaned.push_str(&file_stem[copied_until..group_start]);
                        copied_until = offset + character.len_utf8();
                    }
                }
            }
            _ => {}
        }
    }
    cleaned.push_str(&file_stem[copied_until..]);
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        let fallback = file_stem.trim();
        if fallback.is_empty() {
            "Unknown".to_owned()
        } else {
            fallback.to_owned()
        }
    } else {
        cleaned
    }
}

/// Scan at most three subdirectory levels. Missing/unreadable inputs are
/// optional: malformed gamelists fall back atomically to names and folder media.
pub fn scan_system(sys: &SystemDef) -> Vec<Game> {
    scan_system_with_mapper(sys, &Path::to_owned)
}

/// Resolve metadata paths before touching the filesystem (e.g. a PC ROOT).
/// The system directories themselves must already be resolved by the caller.
/// Every root is read with its own gamelist and media; a ROM at the same
/// relative path on both cards is listed once, from the first card.
pub fn scan_system_with_mapper(sys: &SystemDef, mapper: &dyn Fn(&Path) -> PathBuf) -> Vec<Game> {
    let mut seen = HashSet::new();
    let mut games = Vec::new();
    for root in sys.roots() {
        games.extend(scan_root(sys, root, mapper).into_iter().filter(|game| {
            game.path
                .strip_prefix(root)
                .is_ok_and(|relative| seen.insert(relative.to_owned()))
        }));
    }
    games.sort_by_cached_key(|game| (game.name.to_lowercase(), game.path.clone()));
    games
}

fn scan_root(sys: &SystemDef, dir: &Path, mapper: &dyn Fn(&Path) -> PathBuf) -> Vec<Game> {
    let Ok(root) = dir.canonicalize() else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    let mut ancestors = HashSet::from([root]);
    scan_directory(dir, sys, 0, false, &mut ancestors, &mut paths);
    let metadata = load_gamelist(dir, mapper).unwrap_or_default();
    let mut media = MediaLookup::new(dir);
    paths
        .into_iter()
        .map(|path| {
            let stem = path.file_stem().unwrap_or_else(|| path.as_os_str());
            let entry = if metadata.is_empty() {
                None
            } else {
                game_path_key(&path).and_then(|key| metadata.get(&key))
            };
            let name = entry
                .filter(|entry| !entry.name.is_empty())
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| clean_name(&stem.to_string_lossy()));
            // ES semantics: <thumbnail> is the box, <image> the main (screen) image.
            let thumbnail = entry.and_then(|entry| media.referenced(&entry.thumbnail, mapper));
            let main = entry.and_then(|entry| media.referenced(&entry.image, mapper));
            let (image, screenshot) = match thumbnail {
                Some(thumbnail) => (Some(thumbnail), main),
                None => (main.or_else(|| media.image(stem)), None),
            };
            let screenshot =
                screenshot.or_else(|| media.find("media/screenshots", stem, &[".png", ".jpg"]));
            let logo = entry
                .and_then(|entry| media.referenced(&entry.marquee, mapper))
                .or_else(|| media.find("media/wheel", stem, &[".png", ".jpg"]));
            let video = entry
                .and_then(|entry| media.referenced(&entry.video, mapper))
                .or_else(|| media.video(stem));
            Game {
                path,
                name,
                image,
                video,
                favorite: entry.is_some_and(|entry| entry.favorite),
                last_played: entry.and_then(|entry| entry.last_played),
                year: entry.and_then(|entry| entry.year),
                players: entry.and_then(|entry| entry.players.clone()),
                details: Details {
                    screenshot,
                    logo,
                    ..entry.map(|entry| entry.details.clone()).unwrap_or_default()
                },
            }
        })
        .collect()
}

fn scan_directory(
    directory: &Path,
    sys: &SystemDef,
    depth: usize,
    followed_symlink: bool,
    ancestors: &mut HashSet<PathBuf>,
    games: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(mut kind) = entry.file_type() else {
            continue;
        };
        let is_symlink = kind.is_symlink();
        if is_symlink {
            if followed_symlink {
                continue;
            }
            // Resolve exactly one link, not a chain. Preserve the visible path
            // for launching and media names even when the target lives elsewhere.
            let Ok(target) = fs::read_link(&path) else {
                continue;
            };
            let target = directory.join(target);
            let Ok(metadata) = fs::symlink_metadata(target) else {
                continue;
            };
            kind = metadata.file_type();
            if kind.is_symlink() {
                continue;
            }
        }
        if !(kind.is_file() || kind.is_dir()) {
            continue;
        }
        if kind.is_dir()
            && matches!(
                name.to_str(),
                Some("media" | "images" | "videos" | "manuals" | "downloaded_images")
            )
        {
            continue;
        }
        let matching_extension = path.extension().is_some_and(|extension| {
            let extension = format!(".{}", extension.to_string_lossy().to_lowercase());
            sys.extensions.contains(&extension)
        });
        if matching_extension {
            games.push(path);
        } else if kind.is_dir() && depth < 3 {
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            if ancestors.insert(canonical.clone()) {
                scan_directory(
                    &path,
                    sys,
                    depth + 1,
                    followed_symlink || is_symlink,
                    ancestors,
                    games,
                );
                ancestors.remove(&canonical);
            }
        }
    }
}

struct MediaLookup {
    system_dir: PathBuf,
    directories: HashMap<PathBuf, HashSet<OsString>>,
}

impl MediaLookup {
    fn new(system_dir: &Path) -> Self {
        Self {
            system_dir: system_dir.to_owned(),
            directories: HashMap::new(),
        }
    }

    fn contains_file(&mut self, path: &Path) -> bool {
        let Some(parent) = path.parent() else {
            return false;
        };
        let Some(filename) = path.file_name() else {
            return false;
        };
        // Keep parent traversal intact: a symlink before `..` changes its target.
        // Path hashing/equality already ignores harmless `.` components.
        // References and folder fallbacks share one lazy snapshot per parent,
        // including misses. Only symlinks need a stat while listing the folder.
        self.directories
            .entry(parent.to_owned())
            .or_insert_with_key(|parent| {
                #[cfg(test)]
                MEDIA_DIRECTORY_READS.with(|counts| {
                    *counts.borrow_mut().entry(parent.clone()).or_default() += 1;
                });
                fs::read_dir(parent)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|entry| {
                        entry.file_type().is_ok_and(|kind| {
                            kind.is_file() || (kind.is_symlink() && entry.path().is_file())
                        })
                    })
                    .map(|entry| entry.file_name())
                    .collect()
            })
            .contains(filename)
    }

    fn referenced(&mut self, text: &str, mapper: &dyn Fn(&Path) -> PathBuf) -> Option<PathBuf> {
        let path = mapper(&resolve_path(&self.system_dir, text)?);
        self.contains_file(&path).then_some(path)
    }

    fn find(&mut self, directory: &str, stem: &OsStr, suffixes: &[&str]) -> Option<PathBuf> {
        suffixes.iter().find_map(|suffix| {
            let mut filename = stem.to_os_string();
            filename.push(suffix);
            let path = self.system_dir.join(directory).join(filename);
            self.contains_file(&path).then_some(path)
        })
    }

    fn image(&mut self, stem: &OsStr) -> Option<PathBuf> {
        [
            "media/images",
            "media/covers",
            "media/boxart",
            "media/thumbnails",
            "images",
        ]
        .into_iter()
        .find_map(|directory| {
            let suffixes: &[&str] = if directory == "images" {
                &[
                    ".png",
                    ".jpg",
                    ".jpeg",
                    "-image.png",
                    "-image.jpg",
                    "-image.jpeg",
                ]
            } else {
                &[".png", ".jpg", ".jpeg"]
            };
            self.find(directory, stem, suffixes)
        })
    }

    fn video(&mut self, stem: &OsStr) -> Option<PathBuf> {
        self.find("media/videos", stem, &[".mp4"])
            .or_else(|| self.find("videos", stem, &["-video.mp4"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn media_references_preserve_symlink_parent_traversal() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let system_dir = directory.path().join("snes");
        let external = directory.path().join("external");
        fs::create_dir(&system_dir).unwrap();
        fs::create_dir_all(external.join("nested")).unwrap();
        symlink(external.join("nested"), system_dir.join("linked")).unwrap();
        for name in ["Game.sfc", "Thumb.sfc"] {
            fs::write(system_dir.join(name), []).unwrap();
        }
        for name in ["cover.png", "thumb.jpg", "clip.mp4"] {
            fs::write(external.join(name), []).unwrap();
        }
        fs::write(system_dir.join("gamelist.xml"),
            "<gameList><game><path>Game.sfc</path><image>./linked/../cover.png</image><video>./linked/../clip.mp4</video></game><game><path>Thumb.sfc</path><thumbnail>./linked/../thumb.jpg</thumbnail></game></gameList>")
            .unwrap();
        let sys = SystemDef {
            name: "snes".into(),
            fullname: "Super Nintendo".into(),
            path: system_dir.clone(),
            extensions: vec![".sfc".into()],
            emulators: Vec::new(),
            ..Default::default()
        };

        let games = scan_system(&sys);

        assert_eq!(games.len(), 2);
        assert_eq!(games[0].image, Some(system_dir.join("linked/../cover.png")));
        assert_eq!(games[0].video, Some(system_dir.join("linked/../clip.mp4")));
        assert_eq!(games[1].image, Some(system_dir.join("linked/../thumb.jpg")));
        MEDIA_DIRECTORY_READS.with(|counts| {
            assert_eq!(counts.borrow().get(&system_dir.join("linked/..")), Some(&1));
        });
    }

    #[test]
    fn two_games_share_media_directory_listings_for_references_and_fallbacks() {
        MEDIA_DIRECTORY_READS.with(|counts| counts.borrow_mut().clear());
        let directory = tempfile::tempdir().unwrap();
        let system_dir = directory.path().join("snes");
        let images = system_dir.join("media/images");
        let shared = directory.path().join("shared");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir(&shared).unwrap();
        for path in [
            system_dir.join("First.sfc"),
            system_dir.join("Second.sfc"),
            images.join("chosen.png"),
            images.join("Second.jpg"),
            shared.join("clip.mp4"),
        ] {
            fs::write(path, []).unwrap();
        }
        let clip = shared.join("clip.mp4");
        fs::write(system_dir.join("gamelist.xml"), format!(
            "<gameList><game><path>First.sfc</path><image>./media/images/chosen.png</image><video>{0}</video></game><game><path>Second.sfc</path><image>./media/images/missing.png</image><thumbnail>./media/images/missing-thumb.png</thumbnail><video>{0}</video></game></gameList>",
            clip.display()))
            .unwrap();
        let sys = SystemDef {
            name: "snes".into(),
            fullname: "Super Nintendo".into(),
            path: system_dir,
            extensions: vec![".sfc".into()],
            emulators: Vec::new(),
            ..Default::default()
        };

        let games = scan_system(&sys);

        assert_eq!(games.len(), 2);
        assert_eq!(games[0].image, Some(images.join("chosen.png")));
        assert_eq!(games[1].image, Some(images.join("Second.jpg")));
        assert!(games.iter().all(|game| game.video.as_ref() == Some(&clip)));
        MEDIA_DIRECTORY_READS.with(|counts| {
            let counts = counts.borrow();
            assert_eq!(counts.get(&images), Some(&1));
            assert_eq!(counts.get(&shared), Some(&1));
        });
    }
}
