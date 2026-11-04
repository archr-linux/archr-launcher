//! Cartridge saves taken along when a system or a game changes emulator
//! (Converter-save). Only formats checked against real saves convert:
//!
//! - NDS: DraStic and DeSmuME `.dsv` (the raw save plus DeSmuME's 122-byte
//!   footer) and melonDS `.sav` (the raw save);
//! - PlayStation: RetroArch's `.srm` (PCSX ReARMed, SwanStation, Beetle)
//!   and DuckStation's card 1, both the raw 128 KiB card. DuckStation must
//!   name its cards after the game file (`Card1Type = PerGameFileTitle`):
//!   `<MemoryCards Directory>/<file title>_1.mcd`. With the database title
//!   (`PerGameTitle`) nothing is offered;
//! - Nintendo 64: standalone Mupen64Plus `.eep` and Mupen64Plus-Next's
//!   `.srm` (EEPROM 2 KiB, four Controller Paks, SRAM 32 KiB, FlashRAM
//!   128 KiB). Only EEPROM games, whose size comes from mupen64plus.ini's
//!   `SaveType`; a game with Controller Pak, SRAM or FlashRAM data is left
//!   alone until those are checked against real saves.
//!
//! Formats that already match need nothing: every RetroArch core keeps its
//! `.srm` next to the ROM, and both Flycasts share bios/dc/vmu_save_*.bin.
//! The original is never touched: a verified copy goes to `.antes-<new>/`
//! first and the converted save is a new file, created only where none
//! exists (rename without replace: a save the emulator wrote meanwhile
//! wins). An older save at the destination is replaced only after it was
//! copied to `.antes-<new>/` too.
use archr_ui::convert::Offer;
use std::{
    collections::{BTreeMap, VecDeque},
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc,
};

/// Largest save read: an 8 Mbit DS cartridge with its footer.
const MAX_SAVE: u64 = (1 << 20) + DSV_FOOTER as u64;
const DSV_FOOTER: usize = 122;
const DSV_MAGIC: &[u8] = b"|-DESMUME SAVE-|";
const DSV_SNIP: &[u8] =
    b"|<--Snip above here to create a raw sav by excluding this DeSmuME savedata footer:";
const CARD: usize = 128 * 1024;
const EEPROM: usize = 0x800;
const PAK: usize = 0x8000;
const N64_SRM: usize = EEPROM + 4 * PAK + 0x8000 + 0x20000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Dsv,
    Sav,
    PsxSrm,
    Mcd,
    MupenSa,
    N64Srm,
}
impl Format {
    fn of(system: &str, (emulator, core): (&str, &str)) -> Option<Self> {
        Some(match (system, emulator, core) {
            ("nds", "drastic", _) | ("nds", "retroarch", "desmume") => Self::Dsv,
            ("nds", "melonds", _) => Self::Sav,
            (
                "psx",
                "retroarch",
                "pcsx_rearmed" | "pcsx_rearmed32" | "swanstation" | "beetle_psx",
            ) => Self::PsxSrm,
            ("psx", "duckstation", _) => Self::Mcd,
            ("n64", "mupen64plus", _) => Self::MupenSa,
            ("n64", "retroarch", "mupen64plus_next") => Self::N64Srm,
            _ => return None,
        })
    }
    fn system(self) -> &'static str {
        match self {
            Self::Dsv | Self::Sav => "nds",
            Self::PsxSrm | Self::Mcd => "psx",
            Self::MupenSa | Self::N64Srm => "n64",
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Self::Dsv => ".dsv",
            Self::Sav => ".sav",
            Self::PsxSrm | Self::N64Srm => ".srm",
            Self::Mcd => ".mcd",
            Self::MupenSa => ".eep",
        }
    }
    fn pair(from: Self, to: Self) -> bool {
        from != to && from.system() == to.system()
    }
}
/// The emulator of an emulator/core pair: RetroArch's is its core.
fn engine<'a>((emulator, core): (&'a str, &'a str)) -> &'a str {
    if emulator == "retroarch" {
        core
    } else {
        emulator
    }
}
/// The emulator as people know it.
pub fn name(pair: (&str, &str)) -> String {
    match engine(pair) {
        "drastic" => "DraStic",
        "melonds" => "melonDS",
        "desmume" => "DeSmuME",
        "duckstation" => "DuckStation",
        "pcsx_rearmed" | "pcsx_rearmed32" => "PCSX ReARMed",
        "swanstation" => "SwanStation",
        "beetle_psx" => "Beetle PSX",
        "mupen64plus" => "Mupen64Plus",
        "mupen64plus_next" => "Mupen64Plus-Next",
        other => other,
    }
    .into()
}
/// The folder name the originals go to: the new emulator's.
fn keep_dir(pair: (&str, &str)) -> String {
    format!(".antes-{}", engine(pair))
}

/// DraStic/DeSmuME `.dsv` → raw: the save without its footer.
fn dsv_to_raw(dsv: &[u8]) -> Option<Vec<u8>> {
    let body = dsv.len().checked_sub(DSV_FOOTER)?;
    (dsv.ends_with(DSV_MAGIC) && dsv[body..].starts_with(DSV_SNIP)).then(|| dsv[..body].to_vec())
}
/// Raw → `.dsv`: DeSmuME's footer, with the address size its save type uses.
fn raw_to_dsv(raw: &[u8]) -> Vec<u8> {
    let size = raw.len() as u32;
    let address = match raw.len() {
        0..=512 => 1u32,
        513..=0x10000 => 2,
        _ => 3,
    };
    let mut out = raw.to_vec();
    out.extend_from_slice(DSV_SNIP);
    // Used size, padded size, type (0: autodetect), address size, memory size, version.
    for value in [size, size, 0, address, size, 0] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(DSV_MAGIC);
    out
}
/// A Controller Pak as Mupen64Plus formats it, never written by a game.
fn blank_pak() -> Vec<u8> {
    let id: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0x05, 0x1a, 0x5f, 0x13, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, 0xff, 0x66, 0x25, 0x99, 0xcd,
    ];
    let mut pak = vec![0u8; PAK];
    pak[0] = 0x81;
    for (n, byte) in pak[1..0x20].iter_mut().enumerate() {
        *byte = n as u8 + 1;
    }
    for at in [0x20, 0x60, 0x80, 0xc0] {
        pak[at..at + 32].copy_from_slice(&id);
    }
    for byte in pak[0x100..].iter_mut().skip(1).step_by(2) {
        *byte = 3;
    }
    pak[0x101] = 0x71;
    pak
}
/// A Controller Pak no game wrote to: Mupen64Plus-Next's blank pak, or the
/// standalone's (its own serials) with every page free, no note and no data.
fn pak_unused(pak: &[u8]) -> bool {
    pak == blank_pak()
        || (pak.len() == PAK
            && pak[0x100..0x200].chunks(2).skip(5).all(|i| i == [0, 3])
            && pak[0x300..].iter().all(|&b| b == 0))
}
/// Mupen64Plus-Next `.srm` → the EEPROM file of `size` bytes (0x200 for a
/// 4 Kbit game, 0x800 for 16 Kbit), when nothing else is saved.
fn srm_to_eep(srm: &[u8], size: usize) -> Option<Vec<u8>> {
    if srm.len() != N64_SRM || !matches!(size, 0x200 | EEPROM) {
        return None;
    }
    let paks = &srm[EEPROM..EEPROM + 4 * PAK];
    let rest = &srm[EEPROM + 4 * PAK..];
    let untouched = |b: &[u8]| b.iter().all(|&x| x == 0) || b.iter().all(|&x| x == 0xff);
    let eeprom = &srm[..size];
    if !paks.chunks(PAK).all(pak_unused)
        || !untouched(&rest[..0x8000])
        || !untouched(&rest[0x8000..])
        || untouched(eeprom)
    {
        return None;
    }
    Some(eeprom.to_vec())
}
/// The EEPROM file → `.srm`, with blank paks, SRAM and FlashRAM.
fn eep_to_srm(eep: &[u8], size: usize) -> Option<Vec<u8>> {
    // The core writes `size` bytes; an older file may hold the whole 2 KiB.
    if eep.len() != size && eep.len() != EEPROM {
        return None;
    }
    let mut srm = vec![0xff; EEPROM];
    srm[..size].copy_from_slice(eep.get(..size)?);
    for _ in 0..4 {
        srm.extend_from_slice(&blank_pak());
    }
    srm.extend(std::iter::repeat_n(0, 0x8000));
    srm.extend(std::iter::repeat_n(0xff, 0x20000));
    Some(srm)
}

/// Where the emulators look: the ROMs, Mupen64Plus's game list and
/// DuckStation's settings.
#[derive(Clone, Debug)]
pub struct Env {
    pub roms: PathBuf,
    pub mupen_ini: PathBuf,
    /// DuckStation's settings.ini; relative card folders start beside it.
    pub duckstation: PathBuf,
}
impl Env {
    pub fn new(paths: &crate::paths::Paths) -> Self {
        let user = paths.at("/storage/.config/duckstation/settings.ini");
        Self {
            roms: paths.at("/storage/roms"),
            mupen_ini: paths.at("/usr/local/share/mupen64plus/mupen64plus.ini"),
            duckstation: if user.exists() {
                user
            } else {
                paths.at("/usr/config/duckstation/settings.ini")
            },
        }
    }
    /// DuckStation's card folder, when it names cards after the game file.
    fn cards(&self) -> Option<PathBuf> {
        let text =
            archr_io::read_utf8(&self.duckstation, 1 << 20, archr_io::Symlinks::Follow).ok()?;
        let mut section = "";
        let mut values = BTreeMap::new();
        for line in text.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name;
            } else if let Some((key, value)) = line.split_once('=') {
                if section == "MemoryCards" {
                    values.insert(key.trim(), value.trim());
                }
            }
        }
        if values.get("Card1Type") != Some(&"PerGameFileTitle") {
            return None;
        }
        let dir = Path::new(values.get("Directory").copied().unwrap_or("memcards"));
        Some(match self.duckstation.parent() {
            Some(base) if dir.is_relative() => base.join(dir),
            _ => self
                .roms
                .join(dir.strip_prefix("/storage/roms").unwrap_or(dir)),
        })
    }
}
/// DuckStation's `Path::SanitizeFileName` on Linux: controls, `/` and `*`
/// become `_`.
fn duckstation_title(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c <= '\u{1f}' || c == '/' || c == '*' {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// A game's entry in mupen64plus.ini, following `RefMD5`.
struct Mupen {
    md5: String,
    good: String,
    /// EEPROM bytes the core writes, None for any other save type.
    eeprom: Option<usize>,
}
fn mupen_entry(ini: &str, md5: &str) -> Option<Mupen> {
    let section = |md5: &str| -> BTreeMap<&str, &str> {
        let head = format!("[{md5}]");
        ini.lines()
            .skip_while(|l| l.trim() != head)
            .skip(1)
            .take_while(|l| !l.trim_start().starts_with('['))
            .filter_map(|l| l.trim().split_once('='))
            .collect()
    };
    let own = section(md5);
    let good = own.get("GoodName")?.to_string();
    let save = own.get("SaveType").map(|s| s.to_string()).or_else(|| {
        section(own.get("RefMD5")?)
            .get("SaveType")
            .map(|s| s.to_string())
    });
    let eeprom = match save.as_deref() {
        Some("Eeprom 4KB") => Some(0x200),
        Some("Eeprom 16KB") => Some(EEPROM),
        _ => None,
    };
    Some(Mupen {
        md5: md5.into(),
        good,
        eeprom,
    })
}
/// The core's save name (mupen64plus-core main.c, get_save_filename):
/// `snprintf("%.32s-%.8s", goodname, MD5)`, then `:<>"/\|?*` become `_`.
fn mupen_name(good: &str, md5: &str) -> Option<String> {
    let cut = &good.as_bytes()[..good.len().min(32)];
    let name = format!("{}-{}", std::str::from_utf8(cut).ok()?, md5.get(..8)?);
    Some(
        name.chars()
            .map(|c| if ":<>\"/\\|?*".contains(c) { '_' } else { c })
            .collect(),
    )
}

/// One save to convert: where it is, where the new one goes, and whether
/// an older save there is replaced (after its own copy).
#[derive(Debug)]
struct Step {
    source: PathBuf,
    target: PathBuf,
    replace: bool,
    /// N64 EEPROM size.
    eeprom: usize,
}
/// The game files of the system folder, one per stem (a .cue and its .bin
/// are one game).
fn roms(dir: &Path, only: Option<&Path>) -> Vec<(String, PathBuf)> {
    const NOT_GAMES: &[&str] = &[
        "srm", "dsv", "sav", "mcd", "eep", "mpk", "sra", "fla", "state", "png", "jpg", "xml",
        "txt", "cfg", "bak", "auto", "st0", "st1",
    ];
    let mut files: Vec<PathBuf> = match only {
        Some(rom) => vec![rom.to_owned()],
        None => fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect(),
    };
    // The track files sort after the sheet that names them.
    files.sort_by_key(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bin")));
    let mut seen = std::collections::BTreeSet::new();
    files
        .into_iter()
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            !ext.is_empty() && !NOT_GAMES.contains(&ext.to_ascii_lowercase().as_str())
        })
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_owned(), p)))
        .filter(|(stem, _)| seen.insert(stem.clone()))
        .collect()
}
/// Source to target, unless the target is there already. A target older
/// than the source is replaced (after its own copy).
fn step(source: PathBuf, target: PathBuf, eeprom: usize) -> Option<Step> {
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    if !source.is_file() {
        return None;
    }
    let replace = match (modified(&target), target.exists()) {
        (_, false) => false,
        (Some(old), true) => modified(&source).is_some_and(|new| new > old),
        (None, true) => return None,
    };
    if target.exists() && !replace {
        return None;
    }
    Some(Step {
        source,
        target,
        replace,
        eeprom,
    })
}
/// The N64 step of one game: reads the ROM for its MD5.
fn n64_step(dir: &Path, stem: &str, rom: &Path, from: Format, ini: &str) -> Option<Step> {
    let md5 = archr_cheevos::rom_hash("n64", rom).ok()??.to_uppercase();
    let game = mupen_entry(ini, &md5)?;
    let size = game.eeprom?;
    let name = mupen_name(&game.good, &game.md5)?;
    // Anything beyond the EEPROM was never tested: leave the game.
    let other = ["mpk", "sra", "fla"].iter().any(|ext| {
        let path = dir.join(format!("{name}.{ext}"));
        path.exists()
            && !(*ext == "mpk"
                && fs::read(&path)
                    .is_ok_and(|b| b.len() == 4 * PAK && b.chunks(PAK).all(pak_unused)))
    });
    if other {
        return None;
    }
    let srm = dir.join(format!("{stem}.srm"));
    // The core still reads `<goodname>.eep` when that older file is there.
    let old = dir.join(format!("{}.eep", game.good));
    let old =
        (!game.good.contains('/') && fs::metadata(&old).is_ok_and(|m| m.len() > 0)).then_some(old);
    let eep = old.unwrap_or_else(|| dir.join(format!("{name}.eep")));
    if from == Format::MupenSa {
        step(eep, srm, size)
    } else {
        step(srm, eep, size)
    }
}
struct Plan {
    from: Format,
    to: Format,
    steps: Vec<Step>,
    /// N64 games not read yet: the count is "up to".
    unread: usize,
}
/// What would convert. `hash` allows reading N64 ROMs (slow): without it a
/// whole-system count is an upper bound.
fn plan(env: &Env, offer: &Offer, hash: bool) -> Option<Plan> {
    let from = Format::of(&offer.system, (&offer.from.0, &offer.from.1))?;
    let to = Format::of(&offer.system, (&offer.to.0, &offer.to.1))?;
    if !Format::pair(from, to) {
        return None;
    }
    let dir = env.roms.join(&offer.system);
    let cards = if from == Format::Mcd || to == Format::Mcd {
        Some(env.cards()?)
    } else {
        None
    };
    let card = |stem: &str| {
        let dir = cards.as_deref().unwrap_or(&dir);
        dir.join(format!("{}_1.mcd", duckstation_title(stem)))
    };
    let ini = if from.system() == "n64" {
        archr_io::read_utf8(&env.mupen_ini, 16 << 20, archr_io::Symlinks::Follow)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let eeps = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".eep")));
    let mut plan = Plan {
        from,
        to,
        steps: Vec::new(),
        unread: 0,
    };
    for (stem, rom) in roms(&dir, offer.rom.as_deref()) {
        let simple = |ext: &str| dir.join(format!("{stem}{ext}"));
        if from.system() == "n64" {
            // A game with something to take: an .srm, or any .eep at all.
            let wanted = if from == Format::N64Srm {
                simple(".srm").is_file()
            } else {
                eeps
            };
            if !wanted {
                continue;
            }
            if hash {
                plan.steps.extend(n64_step(&dir, &stem, &rom, from, &ini));
            } else {
                plan.unread += 1;
            }
            continue;
        }
        let (source, target) = match (from, to) {
            (Format::Mcd, _) => (card(&stem), simple(to.ext())),
            (_, Format::Mcd) => (simple(from.ext()), card(&stem)),
            _ => (simple(from.ext()), simple(to.ext())),
        };
        plan.steps.extend(step(source, target, 0));
    }
    Some(plan)
}

/// The converted bytes of one save, or None when it cannot be taken.
fn convert(from: Format, to: Format, bytes: &[u8], eeprom: usize) -> Option<Vec<u8>> {
    match (from, to) {
        (Format::Dsv, Format::Sav) => dsv_to_raw(bytes),
        (Format::Sav, Format::Dsv) => (!bytes.is_empty()).then(|| raw_to_dsv(bytes)),
        (Format::PsxSrm, Format::Mcd) | (Format::Mcd, Format::PsxSrm) => {
            (bytes.len() == CARD && bytes.starts_with(b"MC")).then(|| bytes.to_vec())
        }
        (Format::N64Srm, Format::MupenSa) => srm_to_eep(bytes, eeprom),
        (Format::MupenSa, Format::N64Srm) => eep_to_srm(bytes, eeprom),
        _ => None,
    }
}
/// A new file with `bytes`, synced; never over an existing one
/// (`AlreadyExists` when something is there).
fn create_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = archr_io::parent_dir(path);
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    io::Write::write_all(temporary.as_file_mut(), bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()
}
/// A verified copy in the backup folder, under a free name.
fn back_up(dir: &Path, path: &Path) -> io::Result<()> {
    let bytes = archr_io::read_bytes(path, MAX_SAVE, archr_io::Symlinks::Follow)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("save");
    for n in 1.. {
        let copy = match n {
            1 => dir.join(name),
            n => dir.join(format!("{name}.{n}")),
        };
        match create_new(&copy, &bytes) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
            Ok(()) => {
                let back = archr_io::read_bytes(&copy, MAX_SAVE, archr_io::Symlinks::Follow)?;
                return if back == bytes {
                    Ok(())
                } else {
                    Err(archr_io::invalid_data("backup differs"))
                };
            }
        }
    }
    unreachable!("a free name")
}
/// One save: back up, convert, write. Ok(false) when the emulator's own
/// save appeared meanwhile.
fn apply(plan: &Plan, step: &Step, keep: &Path) -> io::Result<bool> {
    let bytes = archr_io::read_bytes(&step.source, MAX_SAVE, archr_io::Symlinks::Follow)?;
    let converted = convert(plan.from, plan.to, &bytes, step.eeprom)
        .ok_or_else(|| archr_io::invalid_data("not a save of that format"))?;
    back_up(keep, &step.source)?;
    if step.replace {
        back_up(keep, &step.target)?;
        archr_io::atomic_publish(&step.target, true, |file| {
            io::Write::write_all(file, &converted)
        })?;
        return Ok(true);
    }
    match create_new(&step.target, &converted) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        result => result.map(|()| true),
    }
}

/// The offer for a change of emulator, or None when nothing would convert.
/// Whole N64 systems are counted without reading the ROMs: "up to".
pub fn offer(
    env: &Env,
    system: &str,
    rom: Option<PathBuf>,
    from: (String, String),
    to: (String, String),
) -> Option<Offer> {
    let source = Format::of(system, (&from.0, &from.1))?;
    let target = Format::of(system, (&to.0, &to.1))?;
    let mut offer = Offer {
        system: system.into(),
        rom,
        from_name: name((&from.0, &from.1)),
        to_name: name((&to.0, &to.1)),
        from_ext: source.ext().into(),
        to_ext: target.ext().into(),
        keep: format!("roms/{system}/{}", keep_dir((&to.0, &to.1))),
        from,
        to,
        ..Offer::default()
    };
    let plan = plan(env, &offer, offer.rom.is_some())?;
    offer.count = plan.steps.len() + plan.unread;
    offer.up_to = plan.unread > 0;
    offer.replace = plan.steps.iter().filter(|s| s.replace).count();
    (offer.count > 0).then_some(offer)
}
/// How a conversion went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub done: usize,
    pub failed: usize,
}
/// Convert every save of the offer; a bad one is logged and counted, and
/// the others still go.
pub fn run(env: &Env, offer: &Offer) -> Report {
    let mut report = Report::default();
    let Some(plan) = plan(env, offer, true) else {
        report.failed = 1;
        return report;
    };
    let keep = env
        .roms
        .join(&offer.system)
        .join(keep_dir((&offer.to.0, &offer.to.1)));
    for step in &plan.steps {
        match apply(&plan, step, &keep) {
            Ok(true) => report.done += 1,
            Ok(false) => {}
            Err(error) => {
                log::warn!("Save not converted: {}: {error}", step.source.display());
                report.failed += 1;
            }
        }
    }
    report
}

/// Planning and converting, one job at a time on one thread.
pub enum Request {
    Plan {
        system: String,
        rom: Option<PathBuf>,
        from: (String, String),
        to: (String, String),
    },
    Run(Box<Offer>),
}
pub enum Event {
    Offer(Option<Box<Offer>>),
    Done(Report),
}
/// The one owner of the conversion work: requests queue in order and
/// every one answers, even when the thread cannot start.
pub struct Runner {
    env: Env,
    jobs: Option<mpsc::Sender<Request>>,
    events: Option<mpsc::Receiver<Event>>,
    /// Answers given without the thread.
    local: VecDeque<Event>,
}
impl Runner {
    pub fn new(env: Env) -> Self {
        Self {
            env,
            jobs: None,
            events: None,
            local: VecDeque::new(),
        }
    }
    fn start(&mut self) -> io::Result<()> {
        let (jobs, requests) = mpsc::channel::<Request>();
        let (answers, events) = mpsc::channel();
        let env = self.env.clone();
        std::thread::Builder::new()
            .name("launcher-convert".into())
            .spawn(move || {
                for request in requests {
                    let event = match request {
                        Request::Plan {
                            system,
                            rom,
                            from,
                            to,
                        } => Event::Offer(offer(&env, &system, rom, from, to).map(Box::new)),
                        Request::Run(offer) => Event::Done(run(&env, &offer)),
                    };
                    if answers.send(event).is_err() {
                        break;
                    }
                }
            })?;
        (self.jobs, self.events) = (Some(jobs), Some(events));
        Ok(())
    }
    pub fn send(&mut self, request: Request) {
        if self.jobs.is_none() {
            if let Err(error) = self.start() {
                log::warn!("Cannot start the save conversion: {error}");
            }
        }
        let refused = match &self.jobs {
            Some(jobs) => jobs.send(request).err().map(|e| e.0),
            None => Some(request),
        };
        if let Some(request) = refused {
            self.jobs = None;
            self.local.push_back(match request {
                Request::Plan { .. } => Event::Offer(None),
                Request::Run(_) => Event::Done(Report { done: 0, failed: 1 }),
            });
        }
    }
    pub fn poll(&mut self) -> Option<Event> {
        self.local
            .pop_front()
            .or_else(|| self.events.as_ref()?.try_recv().ok())
    }
}

#[cfg(test)]
#[path = "../tests/support/convert_tests.rs"]
mod tests;
