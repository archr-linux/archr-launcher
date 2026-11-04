//! What "save and exit" (the distribution's archr-game-save-exit) left for
//! the switcher and for the resume at power on: one tab separated line per
//! game, `time reason platform emulator core state picture rom`, newest
//! first, and the last game's line apart. Read off the draw thread.
use crate::{library::Library, paths::Paths, state::DataSource};
use archr_config::SystemCfg;
use archr_ui::resume::{Boot, Recent};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{channel, Receiver},
    thread,
    time::{Duration, Instant},
};

const RECORDS: &str = "/storage/.config/archr/resume.tsv";
const LAST: &str = "/storage/.config/archr/resume-last";
/// Left by "save and exit" from the switcher shortcut: open the switcher.
const SWITCHER: &str = "/run/archr/switcher";
/// The PID of a "save and exit" still writing its record.
const SAVING: &str = "/run/archr/saving";
/// Present after the first start since power on.
const BOOTED: &str = "booted";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub time: u64,
    pub reason: String,
    /// The launcher's system the game started from (runemu.sh -P).
    pub system: String,
    pub emulator: String,
    pub core: String,
    /// The state file, when it exists.
    pub state: Option<PathBuf>,
    /// The screen when it was left, when it exists.
    pub picture: Option<PathBuf>,
    pub rom: PathBuf,
}
impl Record {
    fn parse(line: &str, paths: &Paths) -> Option<Self> {
        let fields: Vec<&str> = line.trim_end_matches('\n').split('\t').collect();
        let [time, reason, system, emulator, core, state, picture, rom] = fields[..] else {
            return None;
        };
        let file = |path: &str| {
            Some(paths.remap(Path::new(path))).filter(|p| !path.is_empty() && p.is_file())
        };
        Some(Self {
            time: time.parse().ok()?,
            reason: reason.to_owned(),
            system: system.to_owned(),
            emulator: emulator.to_owned(),
            core: core.to_owned(),
            state: file(state),
            picture: file(picture),
            rom: paths.remap(Path::new(rom)),
        })
    }
    /// runemu.sh's argument to start from the state, when the game still
    /// runs on the emulator and core that wrote it.
    pub fn entry(&self, current: Option<&(String, String)>) -> Option<String> {
        let state = self.state.as_ref()?;
        if current != Some(&(self.emulator.clone(), self.core.clone())) {
            return None;
        }
        Some(if self.emulator == "retroarch" {
            "--entryslot=auto".into()
        } else {
            format!("--entrystate={}", state.to_str()?)
        })
    }
}

/// How the launcher opens.
#[derive(Debug, PartialEq)]
pub enum Start {
    Plain,
    Switcher,
    Resume(Record),
}

/// Decided at start, in the startup worker: the switcher when its shortcut
/// closed the game; at power on, the last game when "save and exit" closed
/// it and Resume at power on is on.
pub fn start(paths: &Paths, config: Option<&SystemCfg>) -> Start {
    let setting = |key: &str, default: &str| {
        config
            .and_then(|cfg| cfg.get_setting(key, None, None))
            .unwrap_or_else(|| default.into())
    };
    if fs::remove_file(paths.at(SWITCHER)).is_ok() {
        return Start::Switcher;
    }
    let booted = paths.run.join(BOOTED);
    if booted.exists() {
        return Start::Plain;
    }
    let _ = fs::create_dir_all(&paths.run).and_then(|()| fs::write(&booted, ""));
    if setting("launcher.resume", "last") == "off" {
        return Start::Plain;
    }
    // At or under the critical level on battery the guard would power off
    // again in the middle of the game: the menu instead.
    let critical: u8 = setting("power.battery.critical", "3").parse().unwrap_or(3);
    if critical > 0 && battery_low(paths, critical) {
        return Start::Plain;
    }
    let record = fs::read_to_string(paths.at(LAST))
        .ok()
        .and_then(|line| Record::parse(&line, paths))
        .filter(|r| r.rom.is_file());
    match record {
        Some(r)
            if r.state.is_some() || setting("launcher.resume.standalone", "start") != "skip" =>
        {
            Start::Resume(r)
        }
        _ => Start::Plain,
    }
}

/// The battery discharges at or under `limit` percent ("Not charging" means
/// a charger is in).
fn battery_low(paths: &Paths, limit: u8) -> bool {
    let Some(dir) = archr_ui::status::find_battery(&paths.at("/")) else {
        return false;
    };
    let read = |name: &str| fs::read_to_string(dir.join(name)).unwrap_or_default();
    read("status").trim() == "Discharging"
        && read("capacity")
            .trim()
            .parse::<u8>()
            .is_ok_and(|level| level <= limit)
}

/// runemu.sh's start from this saved state, for the next launch of the game.
pub fn entry(library: &Library, game: &Recent) -> Option<crate::saves::Entry> {
    Some(crate::saves::Entry {
        system: game.system.clone(),
        rom: library.games(&game.system).get(game.game)?.path.clone(),
        argument: game.entry.clone()?,
        backup: Vec::new(),
    })
}

/// The resume refused: it is not offered at the next power on.
pub fn skip(paths: &Paths) {
    let last = paths.at(LAST);
    thread::spawn(move || {
        let _ = fs::remove_file(last);
    });
}

/// The records, read once a "save and exit" still running has written its
/// own (at most 10 s).
pub struct Records(Receiver<Vec<Record>>);
impl Records {
    pub fn read(paths: &Paths) -> Self {
        let (send, receive) = channel();
        let paths = paths.clone();
        thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(10);
            let saving = paths.at(SAVING);
            while Instant::now() < until
                && fs::read_to_string(&saving)
                    .is_ok_and(|pid| Path::new(&format!("/proc/{}", pid.trim())).exists())
            {
                thread::sleep(Duration::from_millis(50));
            }
            let records = fs::read_to_string(paths.at(RECORDS))
                .unwrap_or_default()
                .lines()
                .filter_map(|line| Record::parse(line, &paths))
                .collect();
            let _ = send.send(records);
        });
        Self(receive)
    }
    pub fn poll(&self) -> Option<Vec<Record>> {
        self.0.try_recv().ok()
    }
}

/// The game as the switcher and the resume screen show it.
fn recent(library: &Library, (system, game): (String, usize), record: Option<&Record>) -> Recent {
    let found = library.games(&system).get(game);
    let current = library.current_core(&system, game);
    Recent {
        name: found.map(|g| g.name.clone()).unwrap_or_default(),
        picture: record.and_then(|r| r.picture.clone()).or_else(|| {
            found.and_then(|g| g.details.screenshot.clone().or_else(|| g.image.clone()))
        }),
        left: record.map(|r| r.time),
        entry: record.and_then(|r| r.entry(current.as_ref())),
        system,
        game,
    }
}

/// The first `limit` recent games, once their systems are loaded.
pub fn switcher(library: &mut Library, records: &[Record], limit: usize) -> Option<Vec<Recent>> {
    library.ensure("@recent");
    if library.loading("@recent") {
        return None;
    }
    let count = library.games("@recent").len().min(limit);
    Some(
        (0..count)
            .filter_map(|n| library.origin("@recent", n))
            .map(|origin| {
                let rom = library
                    .games(&origin.0)
                    .get(origin.1)
                    .map(|g| g.path.clone());
                let record = records.iter().find(|r| Some(&r.rom) == rom.as_ref());
                recent(library, origin, record)
            })
            .collect(),
    )
}

/// The resume screen's game, once its system is loaded; None while it
/// loads, and Some(None) when the game is gone.
pub fn boot(library: &mut Library, record: &Record) -> Option<Option<Boot>> {
    let system = record.system.clone();
    library.ensure(&system);
    if library.loading(&system) {
        return None;
    }
    let game = library
        .games(&system)
        .iter()
        .position(|g| g.path == record.rom);
    Some(game.map(|game| Boot {
        game: recent(library, (system, game), Some(record)),
        ready: true,
        power_off: record.reason != "switch",
        progress: 0.,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(state: &str, rom: &str) -> String {
        format!("1700000000\tswitch\tsnes\tretroarch\tsnes9x\t{state}\t\t{rom}")
    }

    #[test]
    fn records_parse_and_keep_only_existing_files() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().into()));
        let state = paths.at("/storage/roms/savestates/snes/A.state.auto");
        fs::create_dir_all(state.parent().unwrap()).unwrap();
        fs::write(&state, "x").unwrap();
        let r = Record::parse(&line(&state.display().to_string(), "/roms/A.sfc"), &paths).unwrap();
        assert_eq!(r.time, 1_700_000_000);
        assert_eq!(r.state, Some(state));
        assert_eq!(r.picture, None);
        assert_eq!(r.rom, paths.at("/roms/A.sfc"));
        let gone = Record::parse(&line("/nowhere.state.auto", "/roms/A.sfc"), &paths).unwrap();
        assert_eq!(gone.state, None);
        assert!(Record::parse("garbage", &paths).is_none());
        assert!(Record::parse(&line("", "/r").replace("1700000000", "x"), &paths).is_none());
    }

    #[test]
    fn the_entry_needs_the_same_emulator_and_core() {
        let record = Record {
            emulator: "retroarch".into(),
            core: "snes9x".into(),
            state: Some("/s/A.state.auto".into()),
            ..Record::default()
        };
        let same = ("retroarch".to_owned(), "snes9x".to_owned());
        let other = ("retroarch".to_owned(), "bsnes".to_owned());
        assert_eq!(
            record.entry(Some(&same)).as_deref(),
            Some("--entryslot=auto")
        );
        assert_eq!(record.entry(Some(&other)), None);
        let mupen = Record {
            emulator: "mupen64plus".into(),
            core: "mupen64plus".into(),
            state: Some("/roms/n64/M-1234ABCD.st0".into()),
            ..Record::default()
        };
        let current = ("mupen64plus".to_owned(), "mupen64plus".to_owned());
        assert_eq!(
            mupen.entry(Some(&current)).as_deref(),
            Some("--entrystate=/roms/n64/M-1234ABCD.st0")
        );
        assert_eq!(
            Record {
                state: None,
                ..mupen
            }
            .entry(Some(&current)),
            None
        );
    }

    #[test]
    fn start_opens_the_switcher_then_resumes_once_per_power_on() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().into()));
        let rom = paths.at("/storage/roms/snes/A.sfc");
        fs::create_dir_all(rom.parent().unwrap()).unwrap();
        fs::write(&rom, "rom").unwrap();
        fs::create_dir_all(paths.at(LAST).parent().unwrap()).unwrap();
        fs::write(paths.at(LAST), line("", "/storage/roms/snes/A.sfc")).unwrap();
        fs::create_dir_all(paths.at(SWITCHER).parent().unwrap()).unwrap();
        fs::write(paths.at(SWITCHER), "").unwrap();
        assert_eq!(start(&paths, None), Start::Switcher);
        assert!(matches!(start(&paths, None), Start::Resume(r) if r.rom == rom));
        assert_eq!(start(&paths, None), Start::Plain, "only the first start");
        fs::remove_file(paths.run.join(BOOTED)).unwrap();
        let cfg = |text: &str| {
            let file = root.path().join("system.cfg");
            fs::write(&file, text).unwrap();
            SystemCfg::load(&file).unwrap()
        };
        let skip = cfg("launcher.resume.standalone=skip\n");
        assert_eq!(
            start(&paths, Some(&skip)),
            Start::Plain,
            "no state, not opened"
        );
        fs::remove_file(paths.run.join(BOOTED)).unwrap();
        let off = cfg("launcher.resume=off\n");
        assert_eq!(start(&paths, Some(&off)), Start::Plain);
        // On battery at the critical level: the menu; with a charger: resumed.
        let battery = paths.at("/sys/class/power_supply/battery");
        fs::create_dir_all(&battery).unwrap();
        for (status, level, resumed) in [
            ("Discharging", "3", false),
            ("Discharging", "4", true),
            ("Not charging", "2", true),
            ("Charging", "1", true),
        ] {
            fs::write(battery.join("status"), status).unwrap();
            fs::write(battery.join("capacity"), level).unwrap();
            fs::remove_file(paths.run.join(BOOTED)).unwrap();
            let got = matches!(start(&paths, None), Start::Resume(_));
            assert_eq!(got, resumed, "{status} {level}%");
        }
    }
}
