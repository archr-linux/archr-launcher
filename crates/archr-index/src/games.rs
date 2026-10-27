use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::gamelist::{game_path_key, load_gamelist, resolve_path};
use crate::SystemDef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub path: PathBuf,
    pub name: String,
    pub image: Option<PathBuf>,
    pub video: Option<PathBuf>,
    pub favorite: bool,
    pub last_played: Option<u64>,
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
    let Ok(root) = sys.path.canonicalize() else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    let mut ancestors = HashSet::from([root]);
    scan_directory(&sys.path, sys, 0, false, &mut ancestors, &mut paths);
    let metadata = load_gamelist(&sys.path).unwrap_or_default();
    let media = MediaLookup::new(&sys.path);
    let mut games: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let stem = path.file_stem().unwrap_or_else(|| path.as_os_str());
            let entry = if metadata.is_empty() {
                None
            } else {
                game_path_key(&path).and_then(|key| metadata.get(&key))
            };
            let referenced =
                |text: &str| resolve_path(&sys.path, text).filter(|path| path.is_file());
            let name = entry
                .filter(|entry| !entry.name.is_empty())
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| clean_name(&stem.to_string_lossy()));
            let image = entry
                .and_then(|entry| referenced(&entry.image).or_else(|| referenced(&entry.thumbnail)))
                .or_else(|| media.image(stem));
            let video = entry
                .and_then(|entry| referenced(&entry.video))
                .or_else(|| media.video(stem));
            Game {
                path,
                name,
                image,
                video,
                favorite: entry.is_some_and(|entry| entry.favorite),
                last_played: entry.and_then(|entry| entry.last_played),
            }
        })
        .collect();
    games.sort_by_cached_key(|game| (game.name.to_lowercase(), game.path.clone()));
    games
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

struct MediaDirectory {
    path: PathBuf,
    filenames: HashSet<OsString>,
}

impl MediaDirectory {
    fn read(path: PathBuf) -> Self {
        // One directory read per scan, never one stat per game/candidate.
        let filenames = fs::read_dir(&path)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                entry.file_type().is_ok_and(|kind| {
                    kind.is_file() || (kind.is_symlink() && entry.path().is_file())
                })
            })
            .map(|entry| entry.file_name())
            .collect();
        Self { path, filenames }
    }

    fn find(&self, stem: &OsStr, suffixes: &[&str]) -> Option<PathBuf> {
        suffixes.iter().find_map(|suffix| {
            let mut filename = stem.to_os_string();
            filename.push(suffix);
            self.filenames
                .contains(&filename)
                .then(|| self.path.join(filename))
        })
    }
}

struct MediaLookup {
    images: [MediaDirectory; 5],
    videos: [MediaDirectory; 2],
}

impl MediaLookup {
    fn new(system_dir: &Path) -> Self {
        Self {
            images: [
                "media/images",
                "media/covers",
                "media/boxart",
                "media/thumbnails",
                "images",
            ]
            .map(|directory| MediaDirectory::read(system_dir.join(directory))),
            videos: ["media/videos", "videos"]
                .map(|directory| MediaDirectory::read(system_dir.join(directory))),
        }
    }

    fn image(&self, stem: &OsStr) -> Option<PathBuf> {
        self.images
            .iter()
            .enumerate()
            .find_map(|(index, directory)| {
                let suffixes: &[&str] = if index == 4 {
                    &[".png", ".jpg", ".jpeg", "-image.png", "-image.jpg"]
                } else {
                    &[".png", ".jpg", ".jpeg"]
                };
                directory.find(stem, suffixes)
            })
    }

    fn video(&self, stem: &OsStr) -> Option<PathBuf> {
        self.videos[0]
            .find(stem, &[".mp4"])
            .or_else(|| self.videos[1].find(stem, &["-video.mp4"]))
    }
}
