//! Deleting a game or a port from the cards (the game menu's Delete): what
//! goes, found before the question, and the deletion after it. Saves and
//! states stay where they are.
use archr_index::{Game, MediaOwners};
use std::{
    collections::HashSet,
    fs, io,
    path::{Component, Path, PathBuf},
};

/// Playlists whose files are the same game.
const LISTS: [&str; 3] = ["cue", "gdi", "m3u"];
/// The scraper's folders under `<system>/media`, and their files' kinds.
const MEDIA: [&str; 8] = [
    "images",
    "box3d",
    "mix",
    "screenshots",
    "titlescreens",
    "wheel",
    "videos",
    "manuals",
];
const MEDIA_EXTS: [&str; 4] = ["png", "jpg", "mp4", "pdf"];
/// Larger is no playlist and no launch script.
const TEXT_LIMIT: u64 = 1 << 20;

/// What deleting a game takes: the game's own files and folders first,
/// then its scraped media.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub paths: Vec<PathBuf>,
    /// How many of `paths`, at their end, are scraped media.
    pub media: usize,
}
impl Plan {
    /// The game's own paths relative to `roms` (the card's roms folder),
    /// a folder ending in "/".
    pub fn shown(&self, roms: &Path) -> Vec<String> {
        let own = self.paths.len() - self.media;
        self.paths[..own]
            .iter()
            .map(|path| {
                let relative = path.strip_prefix(roms).unwrap_or(path).display();
                if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
                    format!("{relative}/")
                } else {
                    relative.to_string()
                }
            })
            .collect()
    }
}

/// What goes with `game`, found in `root` (its system's folder on its card).
pub fn plan(root: &Path, game: &Game, port: bool) -> Plan {
    let rom = &game.path;
    let mut own = vec![rom.clone()];
    if port {
        own.extend(port_folder(root, rom));
    } else if !rom.is_dir() {
        own.extend(tracks(root, rom));
    }
    let media = media(root, game, &own);
    Plan {
        media: media.len(),
        paths: own.into_iter().chain(media).collect(),
    }
}

/// Below `root`, never `root` itself, and without `..`.
pub fn inside(root: &Path, path: &Path) -> bool {
    path != root && path.starts_with(root) && !path.components().any(|c| c == Component::ParentDir)
}

/// Deletes `paths` in order; one already gone counts as deleted. The first
/// is the game: when it cannot go, nothing else is touched. The error says
/// whether the game went all the same.
pub fn delete(paths: &[PathBuf]) -> Result<(), (bool, io::Error)> {
    let mut later = None;
    for (n, path) in paths.iter().enumerate() {
        let result = match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if n == 0 => return Err((false, error)),
            Err(error) => {
                log::warn!("Cannot delete {}: {error}", path.display());
                later.get_or_insert(error);
            }
        }
    }
    later.map_or(Ok(()), |error| Err((true, error)))
}

fn ext(path: &Path) -> String {
    path.extension()
        .map_or_else(String::new, |e| e.to_string_lossy().to_lowercase())
}

fn read(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > TEXT_LIMIT {
        return None;
    }
    Some(String::from_utf8_lossy(&fs::read(path).ok()?).into_owned())
}

/// `FILE "Game (Track 1).bin" BINARY`.
fn cue_file(line: &str) -> Option<&str> {
    let rest = line
        .get(..5)?
        .eq_ignore_ascii_case("FILE ")
        .then(|| line[5..].trim())?;
    match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next(),
        None => rest.rsplit_once(' ').map(|(name, _)| name),
    }
}
/// `1 0 4 2352 "track01.bin" 0`, or the name unquoted.
fn gdi_file(line: &str) -> Option<&str> {
    match line.split_once('"') {
        Some((_, rest)) => rest.split('"').next(),
        None => line.split_whitespace().nth(4),
    }
}

/// The files `list` names inside `root`, a disc's own tracks after it.
fn named(root: &Path, list: &Path, nested: bool) -> Vec<PathBuf> {
    let (Some(text), Some(dir)) = (read(list), list.parent()) else {
        return Vec::new();
    };
    let kind = ext(list);
    let mut found = Vec::new();
    for line in text.lines().map(str::trim) {
        let name = match kind.as_str() {
            "cue" => cue_file(line),
            "gdi" => gdi_file(line),
            _ => (!line.is_empty() && !line.starts_with('#')).then_some(line),
        };
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            continue;
        };
        let path = dir.join(name);
        if !inside(root, &path) || path == list || found.contains(&path) {
            continue;
        }
        let disc = !nested && matches!(ext(&path).as_str(), "cue" | "gdi");
        let tracks = if disc {
            named(root, &path, true)
        } else {
            Vec::new()
        };
        found.push(path);
        for track in tracks {
            if !found.contains(&track) {
                found.push(track);
            }
        }
    }
    found
}

/// The tracks and discs a .cue, .gdi or .m3u names, there on the card,
/// that no other playlist beside them names too.
fn tracks(root: &Path, rom: &Path) -> Vec<PathBuf> {
    if !LISTS.contains(&ext(rom).as_str()) {
        return Vec::new();
    }
    let ours = named(root, rom, false);
    let dirs: HashSet<&Path> = std::iter::once(rom)
        .chain(ours.iter().map(PathBuf::as_path))
        .filter_map(Path::parent)
        .collect();
    let mut kept = HashSet::new();
    for dir in dirs {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let other = entry.path();
            if LISTS.contains(&ext(&other).as_str()) && other != rom && !ours.contains(&other) {
                kept.extend(named(root, &other, false));
            }
        }
    }
    ours.into_iter()
        .filter(|path| !kept.contains(path) && fs::symlink_metadata(path).is_ok())
        .collect()
}

/// The game's scraped files: the ones its list entry uses and the
/// scraper's under its name, inside `root`, outside what goes already,
/// and none the gamelist gives another game.
fn media(root: &Path, game: &Game, own: &[PathBuf]) -> Vec<PathBuf> {
    let owners = MediaOwners::read(root);
    let stem = archr_io::exfat_name(&game.path.file_stem().unwrap_or_default().to_string_lossy());
    let listed = [
        &game.image,
        &game.video,
        &game.details.screenshot,
        &game.details.logo,
    ]
    .into_iter()
    .flatten()
    .cloned();
    let usual = MEDIA.iter().flat_map(|dir| {
        MEDIA_EXTS
            .iter()
            .map(|ext| root.join("media").join(dir).join(format!("{stem}.{ext}")))
            .collect::<Vec<_>>()
    });
    let mut found: Vec<PathBuf> = Vec::new();
    for path in listed.chain(usual) {
        let file = fs::symlink_metadata(&path).is_ok_and(|m| !m.is_dir());
        if file
            && inside(root, &path)
            && !own.iter().any(|o| path.starts_with(o))
            && owners.owned_by(&path, &game.path) != Some(false)
            && !found.contains(&path)
        {
            found.push(path);
        }
    }
    found
}

/// A name as people spell it differently: "OldBits Hub" and "oldbits-hub".
fn plain(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, ' ' | '-' | '.' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

/// The port folders in `ports`, PortMaster's own left out, and the
/// scripts each one's port.json lists.
fn folders(ports: &Path) -> Vec<(String, Vec<String>)> {
    let mut found: Vec<_> = fs::read_dir(ports)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.eq_ignore_ascii_case("portmaster") && !name.starts_with('.'))
        .map(|name| {
            let items = read(&ports.join(&name).join("port.json"))
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .and_then(|json| {
                    json.get("items")?.as_array().map(|items| {
                        items
                            .iter()
                            .filter_map(|i| i.as_str().map(str::to_owned))
                            .collect()
                    })
                })
                .unwrap_or_default();
            (name, items)
        })
        .collect();
    found.sort();
    found
}

/// The folders a launch script names: a port.json listing it, then its
/// GAMEDIR, then any `…/ports/<x>` or `$controlfolder/../<x>`.
fn named_by(ports: &Path, folders: &[(String, Vec<String>)], script: &str) -> Vec<String> {
    let mut found: Vec<String> = folders
        .iter()
        .filter(|(_, items)| items.iter().any(|i| i == script))
        .map(|(name, _)| name.clone())
        .collect();
    let text = read(&ports.join(script)).unwrap_or_default();
    let mut add = |word: &str| {
        let hit = folders.iter().find(|(f, _)| f.eq_ignore_ascii_case(word));
        if let Some((name, _)) = hit.filter(|(f, _)| !found.contains(f)) {
            found.push(name.clone());
        }
    };
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .collect();
    for line in &lines {
        let line = line.strip_prefix("export ").unwrap_or(line);
        if let Some(value) = line.strip_prefix("GAMEDIR=") {
            let value = value.trim().trim_matches(['"', '\'']).trim_end_matches('/');
            add(value.rsplit('/').next().unwrap_or(""));
        }
    }
    for line in &lines {
        for marker in ["/ports/", "controlfolder/../", "controlfolder}/../"] {
            for (at, _) in line.match_indices(marker) {
                let word: String = line[at + marker.len()..]
                    .chars()
                    .take_while(|c| {
                        !matches!(c, '/' | '"' | '\'' | '$' | ';' | ')' | '`') && !c.is_whitespace()
                    })
                    .collect();
                add(&word);
            }
        }
    }
    found
}

/// The folder of the port `script` in `ports`: the one a port.json or the
/// script names, else the one whose name reads as the script's. None when
/// there is none, or another script names it too (it stays). Never
/// PortMaster's, never outside `ports`.
pub fn port_folder(ports: &Path, script: &Path) -> Option<PathBuf> {
    let name = script.file_name()?.to_str()?;
    let folders = folders(ports);
    let mine = named_by(ports, &folders, name)
        .into_iter()
        .next()
        .or_else(|| {
            let stem = plain(script.file_stem()?.to_str()?);
            folders
                .iter()
                .find(|(f, _)| plain(f) == stem)
                .map(|(f, _)| f.clone())
        })?;
    let shared = fs::read_dir(ports)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|other| other != name && ext(Path::new(other)) == "sh")
        .any(|other| named_by(ports, &folders, &other).contains(&mine));
    let path = ports.join(&mine);
    (!shared && inside(ports, &path)).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(path: PathBuf) -> Game {
        Game {
            path,
            name: "Game".into(),
            image: None,
            video: None,
            favorite: false,
            last_played: None,
            year: None,
            players: None,
            details: Default::default(),
        }
    }
    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    const HEADER: &str = "#!/bin/bash\nif [ -d \"/opt/system/Tools/PortMaster/\" ]; then\n  controlfolder=\"/opt/system/Tools/PortMaster\"\nelse\n  controlfolder=\"/roms/ports/PortMaster\"\nfi\nsource $controlfolder/control.txt\n";

    /// A ports folder as people have it: ports found by name only, by
    /// port.json, by GAMEDIR, one folder two scripts use, PortMaster.
    fn ports() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        for (script, folder) in [
            ("100 Lil Jumps.sh", "100liljumps"),
            ("A7Xpg.sh", "a7xpg"),
            ("Billy Frontier.sh", "billyfrontier"),
            ("OldBits Hub.sh", "oldbits-hub"),
        ] {
            write(&p.join(script), &format!("{HEADER}./run\n"));
            write(&p.join(folder).join("Data.bin"), "x");
        }
        // port.json names the script; the folder's name is nothing like it.
        write(&p.join("Cave Story.sh"), HEADER);
        write(
            &p.join("cavestory-nx/port.json"),
            r#"{"version": 4, "name": "cavestory.zip", "items": ["Cave Story.sh", "cavestory-nx"]}"#,
        );
        // GAMEDIR names it.
        write(
            &p.join("Some Game.sh"),
            &format!("{HEADER}GAMEDIR=\"/$directory/ports/sgdata\"\ncd $GAMEDIR\n"),
        );
        write(&p.join("sgdata/game.pck"), "x");
        // Two scripts, one folder.
        write(
            &p.join("Quake.sh"),
            &format!("{HEADER}GAMEDIR=/$directory/ports/quake/\n"),
        );
        write(
            &p.join("Quake Mission Pack.sh"),
            &format!("{HEADER}cd $controlfolder/../quake\n"),
        );
        write(&p.join("quake/id1/pak0.pak"), "x");
        // PortMaster itself, and a port with no folder at all.
        write(&p.join("PortMaster.sh"), HEADER);
        write(&p.join("PortMaster/control.txt"), "x");
        write(&p.join("Lonely.sh"), HEADER);
        dir
    }

    #[test]
    fn a_ports_folder_is_found_in_layers_and_never_a_shared_one() {
        let dir = ports();
        let p = dir.path();
        let folder = |script: &str| {
            port_folder(p, &p.join(script)).map(|f| f.file_name().unwrap().to_owned())
        };
        for (script, expected) in [
            ("100 Lil Jumps.sh", "100liljumps"),
            ("A7Xpg.sh", "a7xpg"),
            ("Billy Frontier.sh", "billyfrontier"),
            ("OldBits Hub.sh", "oldbits-hub"),
            ("Cave Story.sh", "cavestory-nx"),
            ("Some Game.sh", "sgdata"),
        ] {
            assert_eq!(folder(script), Some(expected.into()), "{script}");
        }
        // Quake's folder is also the mission pack's: only the script goes.
        assert_eq!(folder("Quake.sh"), None);
        assert_eq!(folder("Quake Mission Pack.sh"), None);
        assert_eq!(folder("PortMaster.sh"), None);
        assert_eq!(folder("Lonely.sh"), None);
    }

    #[test]
    fn a_port_takes_its_script_and_its_folder_and_portmaster_stays() {
        let dir = ports();
        let p = dir.path();
        let plan = plan(p, &game(p.join("OldBits Hub.sh")), true);
        assert_eq!(
            plan.paths,
            [p.join("OldBits Hub.sh"), p.join("oldbits-hub")]
        );
        let roms = p.parent().unwrap();
        let name = p.file_name().unwrap().to_string_lossy();
        assert_eq!(
            plan.shown(roms),
            [
                format!("{name}/OldBits Hub.sh"),
                format!("{name}/oldbits-hub/")
            ]
        );
        delete(&plan.paths).unwrap();
        assert!(!p.join("oldbits-hub").exists() && !p.join("OldBits Hub.sh").exists());
        assert!(p.join("PortMaster/control.txt").exists());
        assert!(p.join("quake/id1/pak0.pak").exists());
        let lonely = super::plan(p, &game(p.join("Lonely.sh")), true);
        assert_eq!(lonely.paths, [p.join("Lonely.sh")]);
    }

    #[test]
    fn a_disc_game_takes_the_tracks_no_other_playlist_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("psx");
        write(
            &root.join("Game.cue"),
            "FILE \"Game (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\nFILE \"Game (Track 2).bin\" BINARY\n",
        );
        write(&root.join("Game (Track 1).bin"), "x");
        write(&root.join("Game (Track 2).bin"), "x");
        // Another game's cue names track 2 as well; one names a file outside.
        write(
            &root.join("Other.cue"),
            "FILE \"Game (Track 2).bin\" BINARY\n",
        );
        write(&root.join("Bad.cue"), "FILE \"../outside.bin\" BINARY\n");
        let plan = plan(&root, &game(root.join("Game.cue")), false);
        assert_eq!(
            plan.paths,
            [root.join("Game.cue"), root.join("Game (Track 1).bin")]
        );
        assert!(
            super::plan(&root, &game(root.join("Bad.cue")), false)
                .paths
                .len()
                == 1
        );
        // An m3u: its discs and their tracks.
        write(
            &root.join("FF7.m3u"),
            "#EXTM3U\n.ff7/FF7 (Disc 1).cue\n.ff7/FF7 (Disc 2).chd\n",
        );
        write(
            &root.join(".ff7/FF7 (Disc 1).cue"),
            "FILE \"FF7 (Disc 1).bin\" BINARY\n",
        );
        write(&root.join(".ff7/FF7 (Disc 1).bin"), "x");
        write(&root.join(".ff7/FF7 (Disc 2).chd"), "x");
        let plan = super::plan(&root, &game(root.join("FF7.m3u")), false);
        assert_eq!(
            plan.paths,
            [
                root.join("FF7.m3u"),
                root.join(".ff7/FF7 (Disc 1).cue"),
                root.join(".ff7/FF7 (Disc 1).bin"),
                root.join(".ff7/FF7 (Disc 2).chd"),
            ]
        );
        // A .gdi's tracks.
        write(
            &root.join("dc/Game.gdi"),
            "2\n1 0 4 2352 track01.bin 0\n2 600 0 2352 \"track 02.raw\" 0\n",
        );
        write(&root.join("dc/track01.bin"), "x");
        write(&root.join("dc/track 02.raw"), "x");
        let plan = super::plan(&root, &game(root.join("dc/Game.gdi")), false);
        assert_eq!(plan.paths.len(), 3);
    }

    /// The gamelist's name, spelled so the catalog test does not take it
    /// for a text key.
    const GAMELIST: &str = concat!("gamelist", ".xml");

    #[test]
    fn a_game_takes_its_own_media_never_another_games() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("snes");
        write(&root.join("Mario.sfc"), "x");
        write(&root.join("media/images/Mario.png"), "x");
        write(&root.join("media/videos/Mario.mp4"), "x");
        write(&root.join("media/manuals/Mario.pdf"), "x");
        write(&root.join("media/images/shared.png"), "x");
        write(
            &root.join(GAMELIST),
            "<gameList><game><path>./Zelda.sfc</path><image>./media/images/shared.png</image></game></gameList>",
        );
        let mut mario = game(root.join("Mario.sfc"));
        mario.image = Some(root.join("media/images/shared.png"));
        let plan = plan(&root, &mario, false);
        assert_eq!(plan.media, 3);
        assert_eq!(plan.paths[0], root.join("Mario.sfc"));
        assert!(!plan.paths.contains(&root.join("media/images/shared.png")));
        assert_eq!(plan.shown(dir.path()), ["snes/Mario.sfc"]);
    }

    #[test]
    fn nothing_after_the_game_goes_when_the_game_cannot() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (locked, cover) = (dir.path().join("locked"), dir.path().join("Cover.png"));
        write(&locked.join("Game.sfc"), "x");
        write(&cover, "x");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let result = delete(&[locked.join("Game.sfc"), cover.clone()]);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        // Root may delete anyway; a user cannot.
        if locked.join("Game.sfc").exists() {
            assert!(matches!(result, Err((false, _))));
            assert!(cover.exists());
        }
        // Already gone counts as deleted.
        assert!(delete(&[dir.path().join("Gone.sfc"), cover.clone()]).is_ok());
        assert!(!cover.exists());
        assert!(inside(
            Path::new("/roms/snes"),
            Path::new("/roms/snes/a.sfc")
        ));
        assert!(!inside(Path::new("/roms/snes"), Path::new("/roms/snes")));
        assert!(!inside(
            Path::new("/roms/snes"),
            Path::new("/roms/snes/../nes/a.nes")
        ));
    }
}
