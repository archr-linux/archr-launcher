//! A game's save states and screenshots on the card: found, deleted and
//! backed up off the draw thread. Where emulators keep them is decided in
//! `Places` and `cartridge` only, so moving them is a change here.
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_index::{update_gamelist, GameUpdate};
use archr_io::{atomic_publish, remove_durable};
use archr_ui::saves::{Blocked, Kind, Progress, Save, Shot};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{channel, Receiver, Sender},
    thread,
    time::UNIX_EPOCH,
};

/// Where the emulators keep progress today (all on the ROMs card, which a
/// PC can read: plain files, no links).
#[derive(Clone)]
pub struct Places {
    /// RetroArch states and their pictures: `<states>/<system>/`
    /// (setsettings.sh sets savestate_directory per system).
    states: PathBuf,
    /// RetroArch's captures and the screenshot shortcut's, every game.
    shots: PathBuf,
    /// mupen64plus-arch: SaveStatePath and SaveSRAMPath (mupen64plus.cfg).
    mupen: PathBuf,
    /// Where the hashes the achievements already computed are kept.
    hashes: PathBuf,
    /// system.cfg, read for the game's hardcore and netplay settings.
    config: PathBuf,
}
impl Places {
    pub fn new(paths: &Paths) -> Self {
        Self {
            states: paths.at("/storage/roms/savestates"),
            shots: paths.at("/storage/roms/screenshots"),
            mupen: paths.at("/storage/roms/n64"),
            hashes: paths.cache.join("cheevos"),
            config: paths.config.clone(),
        }
    }
    /// What this screen may delete: a state or its picture straight in a
    /// system's states folder, a mupen64plus state, a capture. Never a ROM,
    /// a cartridge save or anything in a deeper folder.
    fn holds(&self, path: &Path) -> bool {
        let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
            return false;
        };
        let name = name.to_string_lossy();
        let state = |name: &str| {
            name.rsplit_once(".state").is_some_and(|(stem, slot)| {
                !stem.is_empty() && retroarch_slot(&format!("x.state{slot}"), "x").is_some()
            })
        };
        if dir.parent() == Some(&*self.states) {
            state(&name) || name.strip_suffix(".png").is_some_and(state)
        } else if dir == self.mupen {
            mupen_slot(&name).is_some()
        } else {
            dir == self.shots && name.ends_with(".png")
        }
    }
}

/// RetroArch writes the cartridge save beside the ROM
/// (savefiles_in_content_dir).
fn cartridge(rom: &Path) -> PathBuf {
    rom.with_extension("srm")
}

/// What the scan needs to know about the game.
#[derive(Clone, Debug)]
pub struct Game {
    pub system: String,
    pub index: usize,
    pub rom: PathBuf,
    /// The emulator and core a launch would use now.
    pub emulator: Option<(String, String)>,
    /// The picture the details screen shows, and the folder holding the
    /// system's gamelist and media.
    pub details: Option<PathBuf>,
    pub root: PathBuf,
}

/// The ROM file name without its last extension, as RetroArch names states
/// and captures; the second form is the one the screenshot shortcut writes
/// when the name has characters a FAT card refuses. Two ROMs that differ
/// only in those characters ("A:B", "A?B") share that form, so the
/// shortcut's captures of either show under both; no file is lost, the
/// time is in each name.
fn stems(rom: &Path) -> Vec<String> {
    let stem = rom
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let plain: String = stem
        .chars()
        .map(|c| if ":?*\"<>|\\".contains(c) { '_' } else { c })
        .collect();
    let mut stems = vec![stem];
    if plain != stems[0] {
        stems.push(plain);
    }
    stems
}

fn seconds(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// `.state` is slot 0, `.stateN` slot N, `.state.auto` the automatic one.
fn retroarch_slot(name: &str, stem: &str) -> Option<Option<u16>> {
    match name.strip_prefix(stem)?.strip_prefix(".state")? {
        "" => Some(Some(0)),
        ".auto" => Some(None),
        n if (1..=3).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()) => {
            n.parse().ok().map(Some)
        }
        _ => None,
    }
}

/// mupen64plus names states `<good name>-<MD5 8>.st<N>`: the slot and the
/// name without `.st<N>`.
fn mupen_slot(name: &str) -> Option<(u16, &str)> {
    let (base, n) = name.rsplit_once(".st")?;
    let slot = n
        .parse()
        .ok()
        .filter(|_| n.bytes().all(|b| b.is_ascii_digit()))?;
    let (_, md5) = base.rsplit_once('-')?;
    (md5.len() == 8 && md5.bytes().all(|b| b.is_ascii_hexdigit())).then_some((slot, base))
}

/// `<rom>-AAMMDD-HHMMSS.png`: the local time in the name.
fn shot_time(name: &str, stem: &str) -> Option<u64> {
    let rest = name
        .strip_prefix(stem)?
        .strip_prefix('-')?
        .strip_suffix(".png")?;
    let (date, time) = rest.split_once('-')?;
    if date.len() != 6
        || time.len() != 6
        || !(date.to_owned() + time).bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let n = |s: &str, at: usize| s[at..at + 2].parse::<i32>().unwrap_or(0);
    // SAFETY: a zeroed tm is a valid input; mktime reads and normalizes it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = 100 + n(date, 0);
    tm.tm_mon = n(date, 2) - 1;
    tm.tm_mday = n(date, 4);
    tm.tm_hour = n(time, 0);
    tm.tm_min = n(time, 2);
    tm.tm_sec = n(time, 4);
    tm.tm_isdst = -1;
    // SAFETY: tm is a live, initialized struct; mktime keeps no pointer.
    let unix = unsafe { libc::mktime(&mut tm) };
    u64::try_from(unix).ok()
}

/// Width and height from a PNG header.
fn png_size(path: &Path) -> Option<(u32, u32)> {
    use io::Read;
    let mut head = [0u8; 24];
    fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    let be = |at: usize| u32::from_be_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    (head[..8] == *b"\x89PNG\r\n\x1a\n" && head[12..16] == *b"IHDR").then(|| (be(16), be(20)))
}

/// The same bytes (the details picture is a copy of a screenshot).
fn same_file(a: &Path, b: &Path, len: u64) -> bool {
    len < 16 << 20
        && fs::metadata(b).is_ok_and(|m| m.len() == len)
        && matches!((fs::read(a), fs::read(b)), (Ok(x), Ok(y)) if x == y)
}

/// Everything the card holds for the game, newest first.
pub fn scan(places: &Places, game: &Game) -> Progress {
    let stems = stems(&game.rom);
    let (emulator, _) = game.emulator.clone().unwrap_or_default();
    let retroarch = emulator == "retroarch";
    let mut saves = Vec::new();

    let dir = places.states.join(game.system.to_ascii_lowercase());
    let srm = Some(cartridge(&game.rom)).filter(|p| p.is_file());
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(slot) = retroarch_slot(&name, &stems[0]) else {
            continue;
        };
        let Some(meta) = entry.metadata().ok().filter(fs::Metadata::is_file) else {
            continue;
        };
        let picture = dir.join(format!("{name}.png"));
        saves.push(Save {
            path: entry.path(),
            picture: picture.is_file().then_some(picture),
            slot,
            saved: seconds(&meta),
            bytes: meta.len(),
            kind: Kind::RetroArch,
            // The state does not say which core wrote it.
            emulator: "RetroArch".into(),
            playable: retroarch,
            backup: srm.clone().into_iter().collect(),
        });
    }

    // mupen64plus-arch: matched by the ROM's MD5 in the file name, worked
    // out only when such files exist.
    if game.system.eq_ignore_ascii_case("n64") {
        let names: Vec<_> = fs::read_dir(&places.mupen)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let md5 = names
            .iter()
            .any(|n| mupen_slot(n).is_some())
            .then(|| crate::cheevos::cached_hash(&places.hashes, "n64", &game.rom))
            .flatten()
            .map(|h| h[..8].to_ascii_uppercase());
        for name in names.iter().filter(|_| md5.is_some()) {
            let Some((slot, base)) = mupen_slot(name) else {
                continue;
            };
            if !base
                .to_ascii_uppercase()
                .ends_with(md5.as_deref().unwrap_or_default())
            {
                continue;
            }
            let path = places.mupen.join(name);
            let Some(meta) = fs::metadata(&path).ok().filter(fs::Metadata::is_file) else {
                continue;
            };
            let backup = ["eep", "sra", "fla", "mpk"]
                .iter()
                .map(|ext| places.mupen.join(format!("{base}.{ext}")))
                .filter(|p| p.is_file())
                .collect();
            saves.push(Save {
                path,
                picture: None,
                slot: Some(slot),
                saved: seconds(&meta),
                bytes: meta.len(),
                kind: Kind::Standalone,
                emulator: "Mupen64Plus".into(),
                playable: emulator == "mupen64plus",
                backup,
            });
        }
    }
    saves.sort_by_key(|s| std::cmp::Reverse(s.saved));

    let mut shots = Vec::new();
    for entry in fs::read_dir(&places.shots).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(named) = stems.iter().find_map(|stem| shot_time(&name, stem)) else {
            continue;
        };
        let Some(meta) = entry.metadata().ok().filter(fs::Metadata::is_file) else {
            continue;
        };
        let path = entry.path();
        shots.push(Shot {
            size: png_size(&path),
            details: game
                .details
                .as_deref()
                .is_some_and(|d| same_file(d, &path, meta.len())),
            taken: if named > 0 { named } else { seconds(&meta) },
            bytes: meta.len(),
            path,
        });
    }
    shots.sort_by_key(|s| std::cmp::Reverse(s.taken));
    Progress {
        game: Some((game.system.clone(), game.index)),
        saves,
        shots,
        blocked: blocked(places, game),
    }
}

/// RetroArch refuses a state in netplay and in RetroAchievements hardcore
/// mode; each setting resolves per game, then system, then globally, as
/// setsettings.sh reads them for the launch.
fn blocked(places: &Places, game: &Game) -> Option<Blocked> {
    let cfg = SystemCfg::load(&places.config).ok()?;
    let rom = game.rom.file_name()?.to_str()?;
    let on = |key| {
        cfg.get_setting(key, Some(&game.system), Some(rom))
            .as_deref()
            == Some("1")
    };
    if on("netplay") {
        Some(Blocked::Netplay)
    } else if on("retroachievements") && on("retroachievements.hardcore") {
        Some(Blocked::Hardcore)
    } else {
        None
    }
}

/// A launch from a state: runemu.sh's argument and the cartridge saves to
/// back up first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The game it belongs to: a launch of another game ignores it.
    pub system: String,
    pub rom: PathBuf,
    pub argument: String,
    pub backup: Vec<PathBuf>,
}
impl Entry {
    pub fn of(save: &Save, game: &Game) -> Option<Self> {
        let argument = match (save.kind, save.slot) {
            (Kind::RetroArch, Some(n)) => format!("--entryslot={n}"),
            (Kind::RetroArch, None) => "--entryslot=auto".into(),
            (Kind::Standalone, _) => format!("--entrystate={}", save.path.to_str()?),
        };
        Some(Self {
            system: game.system.clone(),
            rom: game.rom.clone(),
            argument,
            backup: save.backup.clone(),
        })
    }
}

/// Cartridge copies kept beside each save, newest first: `.bak.1`..`.bak.3`.
const BACKUPS: usize = 3;

fn backup_name(file: &Path, n: usize) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(format!(".bak.{n}"));
    name.into()
}

/// Keeps the cartridge save before a launch from a state: written whole as
/// `.bak.1`, the older copies moved down to `.bak.3`. An empty save, or one
/// equal to the newest copy (two launches in a row), leaves them as they
/// are, so a copy is never replaced by the same or by nothing.
pub fn back_up(files: &[PathBuf]) -> io::Result<()> {
    for file in files {
        let bytes = fs::read(file)?;
        if bytes.is_empty() || fs::read(backup_name(file, 1)).is_ok_and(|last| last == bytes) {
            continue;
        }
        let fresh = backup_name(file, 0);
        atomic_publish(&fresh, true, |out| io::Write::write_all(out, &bytes))?;
        for n in (1..BACKUPS).rev() {
            match fs::rename(backup_name(file, n), backup_name(file, n + 1)) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
        }
        fs::rename(&fresh, backup_name(file, 1))?;
    }
    Ok(())
}

/// The screenshot becomes the details picture: copied to the system's
/// `media/screenshots/<rom>.png`, which the details screen reads, and named
/// in the gamelist when the gamelist pointed the screen elsewhere.
fn use_shot(game: &Game, shot: &Path) -> io::Result<()> {
    let stem = game.rom.file_stem().unwrap_or_default();
    let mut name = stem.to_owned();
    name.push(".png");
    let target = game.root.join("media/screenshots").join(name);
    let mut source = fs::File::open(shot)?;
    atomic_publish(&target, true, |out| io::copy(&mut source, out).map(drop))?;
    if game.details.as_ref().is_some_and(|d| *d != target) {
        let update = GameUpdate {
            rom: game.rom.clone(),
            fields: vec![("image", target.display().to_string())],
        };
        update_gamelist(&game.root, &[update], true)?;
    }
    Ok(())
}

pub enum Task {
    Scan,
    Delete(Vec<PathBuf>),
    UseShot(PathBuf),
    /// Back up the cartridge save, then hand the launch back.
    Play(Entry),
}

/// What a task left: the game's progress, read again, and a notice.
pub struct Done {
    pub progress: Progress,
    pub notice: Option<&'static str>,
    /// The gamelist or the media changed: the system needs reading again.
    pub reload: Option<String>,
    /// The cartridge save is backed up: the game may start from the state.
    pub play: Option<Entry>,
}

/// Runs one task at a time per request; only the newest answer is kept.
pub struct Worker {
    places: Places,
    send: Sender<(u64, Done)>,
    done: Receiver<(u64, Done)>,
    latest: u64,
}
impl Worker {
    pub fn new(paths: &Paths) -> Self {
        let (send, done) = channel();
        Self {
            places: Places::new(paths),
            send,
            done,
            latest: 0,
        }
    }
    pub fn start(&mut self, task: Task, game: Game) {
        self.latest += 1;
        let (id, places, send) = (self.latest, self.places.clone(), self.send.clone());
        thread::spawn(move || {
            let mut notice = None;
            let mut reload = None;
            let mut play = None;
            match task {
                Task::Scan => {}
                Task::Delete(paths) => {
                    for path in paths.iter().filter(|p| places.holds(p)) {
                        if let Err(error) = remove_durable(path) {
                            log::warn!("Cannot delete {}: {error}", path.display());
                            notice = Some("error.operation");
                        }
                    }
                }
                // Loading a state rewrites the cartridge save when the game
                // closes: without a copy of it, the game does not start.
                Task::Play(entry) => match back_up(&entry.backup) {
                    Ok(()) => play = Some(entry),
                    Err(error) => {
                        log::warn!("Cannot back up the cartridge save: {error}");
                        notice = Some("error.save_backup");
                    }
                },
                Task::UseShot(shot) => match use_shot(&game, &shot) {
                    Ok(()) => {
                        notice = Some("shots.used");
                        reload = Some(game.system.clone());
                    }
                    Err(error) => {
                        log::warn!("Cannot use {} on the details: {error}", shot.display());
                        notice = Some("error.operation");
                    }
                },
            }
            let progress = scan(&places, &game);
            let _ = send.send((
                id,
                Done {
                    progress,
                    notice,
                    reload,
                    play,
                },
            ));
        });
    }
    /// The newest task's answer, once; never blocks.
    pub fn poll(&mut self) -> Option<Done> {
        let mut answer = None;
        while let Ok((id, done)) = self.done.try_recv() {
            // A task's side effects stand; an older scan's list does not.
            if id == self.latest
                || done.reload.is_some()
                || done.notice.is_some()
                || done.play.is_some()
            {
                answer = Some(done);
            }
        }
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_their_rom_only() {
        assert_eq!(retroarch_slot("Mario.state", "Mario"), Some(Some(0)));
        assert_eq!(retroarch_slot("Mario.state3", "Mario"), Some(Some(3)));
        assert_eq!(retroarch_slot("Mario.state999", "Mario"), Some(Some(999)));
        assert_eq!(retroarch_slot("Mario.state.auto", "Mario"), Some(None));
        for other in [
            "Mario.state3.png",
            "Mario.state1.entry",
            "Mario.state1000",
            "Mario World.state",
            "Mario.srm",
        ] {
            assert_eq!(retroarch_slot(other, "Mario"), None, "{other}");
        }
        assert_eq!(
            mupen_slot("Super Mario 64 (U) [!]-635A42C5.st1"),
            Some((1, "Super Mario 64 (U) [!]-635A42C5"))
        );
        assert_eq!(mupen_slot("Super Mario 64-635A42C5.eep"), None);
        assert_eq!(mupen_slot("Readme.st1"), None);
        assert!(shot_time("Mario-261028-201500.png", "Mario").is_some());
        for other in [
            "Mario World-261028-201500.png",
            "Mario-2610-201500.png",
            "Mario-261028-201500.jpg",
            "2026-10-28-201500.png",
        ] {
            assert_eq!(shot_time(other, "Mario"), None, "{other}");
        }
        assert_eq!(
            stems(Path::new("/r/a:b?.sfc")),
            vec!["a:b?".to_owned(), "a_b_".to_owned()]
        );
    }

    fn game(paths: &Paths, system: &str, rom: &str, emulator: (&str, &str)) -> Game {
        let root = paths.at(&format!("/storage/roms/{system}"));
        fs::create_dir_all(&root).unwrap();
        Game {
            system: system.into(),
            index: 0,
            rom: root.join(rom),
            emulator: Some((emulator.0.into(), emulator.1.into())),
            details: None,
            root,
        }
    }

    #[test]
    fn states_and_screenshots_of_the_game_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().to_owned()));
        let places = Places::new(&paths);
        let mut game = game(&paths, "snes", "Mario.sfc", ("retroarch", "snes9x"));
        fs::write(&game.rom, "rom").unwrap();
        let states = paths.at("/storage/roms/savestates/snes");
        let shots = paths.at("/storage/roms/screenshots");
        fs::create_dir_all(&states).unwrap();
        fs::create_dir_all(&shots).unwrap();
        for name in [
            "Mario.state",
            "Mario.state2",
            "Mario.state.auto",
            "Mario World.state1",
        ] {
            fs::write(states.join(name), name).unwrap();
        }
        fs::write(states.join("Mario.state2.png"), "png").unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        fs::File::options()
            .write(true)
            .open(states.join("Mario.state"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        for name in [
            "Mario-261028-201500.png",
            "Mario-261029-080000.png",
            "Mario World-261028-201500.png",
            "2026-10-28-201500.png",
        ] {
            fs::write(shots.join(name), name).unwrap();
        }
        let details = game.root.join("media/screenshots/Mario.png");
        fs::create_dir_all(details.parent().unwrap()).unwrap();
        fs::write(&details, "Mario-261028-201500.png").unwrap();
        game.details = Some(details);

        let progress = scan(&places, &game);
        assert_eq!(progress.game, Some(("snes".into(), 0)));
        let slots: Vec<_> = progress.saves.iter().map(|s| s.slot).collect();
        assert_eq!(slots.len(), 3, "not Mario World's");
        assert_eq!(slots.last(), Some(&Some(0)), "the oldest last");
        let two = progress.saves.iter().find(|s| s.slot == Some(2)).unwrap();
        assert_eq!(two.picture, Some(states.join("Mario.state2.png")));
        assert!(two.backup.is_empty(), "no cartridge save yet");
        let names: Vec<_> = progress
            .shots
            .iter()
            .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["Mario-261029-080000.png", "Mario-261028-201500.png"]
        );
        assert!(progress.shots[1].details && !progress.shots[0].details);

        // Another emulator: listed, deletable, not playable.
        game.emulator = Some(("mupen64plus".into(), "mupen64plus-arch".into()));
        assert!(scan(&places, &game).saves.iter().all(|s| !s.playable));
    }

    #[test]
    fn hardcore_and_netplay_resolve_per_game_then_system_then_globally() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().to_owned()));
        let places = Places::new(&paths);
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        let mario = game(&paths, "snes", "Mario.sfc", ("retroarch", "snes9x"));
        let zelda = game(&paths, "snes", "Zelda.sfc", ("retroarch", "snes9x"));
        let nes = game(&paths, "nes", "Zelda.nes", ("retroarch", "fceumm"));
        let check = |cfg: &str, want: [Option<Blocked>; 3]| {
            fs::write(&paths.config, cfg).unwrap();
            let got = [&mario, &zelda, &nes].map(|g| scan(&places, g).blocked);
            assert_eq!(got, want, "{cfg}");
        };
        check("global.retroachievements=1\n", [None; 3]);
        check(
            "global.retroachievements=1\nsnes[\"Mario.sfc\"].retroachievements.hardcore=1\n",
            [Some(Blocked::Hardcore), None, None],
        );
        check(
            "global.retroachievements=1\nsnes.retroachievements.hardcore=1\n",
            [Some(Blocked::Hardcore), Some(Blocked::Hardcore), None],
        );
        check(
            "global.retroachievements=1\nglobal.retroachievements.hardcore=1\nsnes.retroachievements=0\n",
            [None, None, Some(Blocked::Hardcore)],
        );
        check("global.netplay=1\n", [Some(Blocked::Netplay); 3]);
    }

    #[test]
    fn mupen_states_are_matched_by_the_rom_md5() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().to_owned()));
        let places = Places::new(&paths);
        let game = game(
            &paths,
            "n64",
            "Mario 64.z64",
            ("mupen64plus", "mupen64plus-arch"),
        );
        let rom = [0x80u8, 0x37, 0x12, 0x40, 1, 2, 3, 4];
        fs::write(&game.rom, rom).unwrap();
        let md5 = archr_cheevos::rom_hash("n64", &game.rom).unwrap().unwrap();
        let tag = md5[..8].to_ascii_uppercase();
        let n64 = paths.at("/storage/roms/n64");
        fs::write(n64.join(format!("SUPER MARIO 64-{tag}.st1")), "s").unwrap();
        fs::write(n64.join(format!("SUPER MARIO 64-{tag}.eep")), "e").unwrap();
        fs::write(n64.join("OTHER-0123ABCD.st0"), "s").unwrap();
        let progress = scan(&places, &game);
        assert_eq!(progress.saves.len(), 1);
        let save = &progress.saves[0];
        assert_eq!(
            (save.slot, save.kind, save.playable),
            (Some(1), Kind::Standalone, true)
        );
        assert_eq!(save.backup, [n64.join(format!("SUPER MARIO 64-{tag}.eep"))]);
        let entry = Entry::of(save, &game).unwrap();
        assert_eq!(
            entry.argument,
            format!("--entrystate={}", save.path.display())
        );
    }

    #[test]
    fn the_launch_waits_for_the_backup_on_the_worker() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().to_owned()));
        let game = game(&paths, "snes", "Mario.sfc", ("retroarch", "snes9x"));
        let srm = cartridge(&game.rom);
        fs::write(&srm, "progress").unwrap();
        let mut worker = Worker::new(&paths);
        let mut wait = |entry: Entry| {
            worker.start(Task::Play(entry), game.clone());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if let Some(done) = worker.poll() {
                    return done;
                }
                assert!(std::time::Instant::now() < deadline);
                thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        let entry = Entry {
            system: "snes".into(),
            rom: game.rom.clone(),
            argument: "--entryslot=1".into(),
            backup: vec![srm.clone()],
        };
        let done = wait(entry.clone());
        assert_eq!(done.play, Some(entry.clone()));
        assert_eq!(
            fs::read_to_string(backup_name(&srm, 1)).unwrap(),
            "progress"
        );
        let missing = Entry {
            backup: vec![dir.path().join("none.srm")],
            ..entry
        };
        let done = wait(missing);
        assert_eq!((done.play, done.notice), (None, Some("error.save_backup")));
    }

    #[test]
    fn backups_keep_three_copies_and_never_lose_one() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().to_owned()));
        let srm = dir.path().join("Game.srm");
        let read = |n: usize| fs::read_to_string(backup_name(&srm, n)).ok();
        fs::write(&srm, "one").unwrap();
        back_up(std::slice::from_ref(&srm)).unwrap();
        // Two "play from here" in a row: the second keeps the copy.
        back_up(std::slice::from_ref(&srm)).unwrap();
        assert_eq!((read(1), read(2)), (Some("one".into()), None));
        for text in ["two", "three", "four"] {
            fs::write(&srm, text).unwrap();
            back_up(std::slice::from_ref(&srm)).unwrap();
        }
        assert_eq!(
            [read(1), read(2), read(3), read(4)],
            [
                Some("four".into()),
                Some("three".into()),
                Some("two".into()),
                None
            ]
        );
        // An emptied save is never copied over the good ones.
        fs::write(&srm, "").unwrap();
        back_up(std::slice::from_ref(&srm)).unwrap();
        assert_eq!(read(1), Some("four".into()));
        assert!(!backup_name(&srm, 0).exists(), "no temporary left");
        // No save at all: the launch stops.
        assert!(back_up(&[dir.path().join("none.srm")]).is_err());
        let places = Places::new(&paths);
        for inside in [
            "/storage/roms/savestates/snes/A.state",
            "/storage/roms/savestates/snes/A.state12",
            "/storage/roms/savestates/snes/A.state.auto.png",
            "/storage/roms/n64/MARIO-635A42C5.st1",
            "/storage/roms/screenshots/A-261028-201500.png",
        ] {
            assert!(places.holds(&paths.at(inside)), "{inside}");
        }
        for outside in [
            srm.clone(),
            paths.at("/storage/roms/snes/A.srm"),
            paths.at("/storage/roms/screenshots/../snes/A.srm"),
            paths.at("/storage/roms/savestates/snes/A.srm"),
            paths.at("/storage/roms/savestates/snes/A.sfc"),
            paths.at("/storage/roms/savestates/snes/deeper/A.state"),
            paths.at("/storage/roms/savestates/A.state"),
            paths.at("/storage/roms/n64/Mario.z64"),
            paths.at("/storage/roms/n64/MARIO-635A42C5.eep"),
            paths.at("/storage/roms/screenshots/A.sfc"),
        ] {
            assert!(!places.holds(&outside), "{}", outside.display());
        }
    }
}
