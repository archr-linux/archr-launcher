//! Cartridge saves taken along when a system or a game changes emulator
//! (Converter-save). Only formats checked against real saves convert:
//!
//! - NDS: DraStic and DeSmuME `.dsv` (the raw save plus DeSmuME's 122-byte
//!   footer) and melonDS `.sav` (the raw save);
//! - PlayStation: RetroArch's `.srm` (PCSX ReARMed, SwanStation, Beetle)
//!   and DuckStation's `duckstation/memcards/<game>_1.mcd`, both the raw
//!   128 KiB card;
//! - Nintendo 64: standalone Mupen64Plus `<good name>-<MD5>.eep` and
//!   Mupen64Plus-Next's `.srm` (EEPROM 2 KiB, four Controller Paks, SRAM
//!   32 KiB, FlashRAM 128 KiB). Only the EEPROM was checked against a real
//!   save of both: a game with Controller Pak, SRAM or FlashRAM data is
//!   left alone.
//!
//! Formats that already match need nothing: every RetroArch core keeps its
//! `.srm` next to the ROM, and both Flycasts share bios/dc/vmu_save_*.bin.
//! The original is never touched: a copy goes to `.antes-<new>/` first and
//! the converted save is a new file, written only where none exists.
use archr_ui::convert::Offer;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
};

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
        matches!(
            (from, to),
            (Self::Dsv, Self::Sav)
                | (Self::Sav, Self::Dsv)
                | (Self::PsxSrm, Self::Mcd)
                | (Self::Mcd, Self::PsxSrm)
                | (Self::MupenSa, Self::N64Srm)
                | (Self::N64Srm, Self::MupenSa)
        )
    }
}
/// The emulator as people know it.
pub fn name((emulator, core): (&str, &str)) -> String {
    let name = if emulator == "retroarch" {
        core
    } else {
        emulator
    };
    match name {
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
fn keep_dir((emulator, core): (&str, &str)) -> String {
    let name = if emulator == "retroarch" {
        core
    } else {
        emulator
    };
    format!(".antes-{name}")
}

const DSV_FOOTER: usize = 122;
const DSV_MAGIC: &[u8] = b"|-DESMUME SAVE-|";
const DSV_SNIP: &[u8] =
    b"|<--Snip above here to create a raw sav by excluding this DeSmuME savedata footer:";
const CARD: usize = 128 * 1024;
const EEPROM: usize = 0x800;
const PAK: usize = 0x8000;
const N64_SRM: usize = EEPROM + 4 * PAK + 0x8000 + 0x20000;

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
/// Mupen64Plus-Next `.srm` → the EEPROM file, when nothing else is saved.
fn srm_to_eep(srm: &[u8]) -> Option<Vec<u8>> {
    if srm.len() != N64_SRM {
        return None;
    }
    let blank = blank_pak();
    let paks = &srm[EEPROM..EEPROM + 4 * PAK];
    let rest = &srm[EEPROM + 4 * PAK..];
    let untouched = |b: &[u8]| b.iter().all(|&x| x == 0) || b.iter().all(|&x| x == 0xff);
    let eeprom = &srm[..EEPROM];
    if !paks.chunks(PAK).all(|pak| pak == blank)
        || !untouched(&rest[..0x8000])
        || !untouched(&rest[0x8000..])
        || untouched(eeprom)
    {
        return None;
    }
    // A 4 Kbit game leaves the rest of the 16 Kbit area erased.
    let short = eeprom[0x200..].iter().all(|&x| x == 0xff);
    Some(eeprom[..if short { 0x200 } else { EEPROM }].to_vec())
}
/// The EEPROM file → `.srm`, with blank paks, SRAM and FlashRAM.
fn eep_to_srm(eep: &[u8]) -> Option<Vec<u8>> {
    if !matches!(eep.len(), 0x200 | EEPROM) {
        return None;
    }
    let mut srm = vec![0xff; EEPROM];
    srm[..eep.len()].copy_from_slice(eep);
    for _ in 0..4 {
        srm.extend_from_slice(&blank_pak());
    }
    srm.extend(std::iter::repeat_n(0, 0x8000));
    srm.extend(std::iter::repeat_n(0xff, 0x20000));
    Some(srm)
}

/// One save to convert: where it is, where the new one goes.
#[derive(Debug)]
struct Step {
    source: PathBuf,
    target: PathBuf,
}
/// Files of the system folder that are games, by stem.
fn roms(dir: &Path, only: Option<&Path>) -> Vec<(String, PathBuf)> {
    const NOT_GAMES: &[&str] = &[
        "srm", "dsv", "sav", "mcd", "eep", "mpk", "sra", "fla", "state", "png", "jpg", "xml",
        "txt", "cfg", "bak", "auto", "st0", "st1",
    ];
    let files: Vec<PathBuf> = match only {
        Some(rom) => vec![rom.to_owned()],
        None => fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect(),
    };
    files
        .into_iter()
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            !ext.is_empty() && !NOT_GAMES.contains(&ext.to_ascii_lowercase().as_str())
        })
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_owned(), p)))
        .collect()
}
/// Mupen64Plus's name for a game's saves: its good name and the start of
/// the ROM's MD5 (of the big-endian image), as the core writes them.
fn mupen_stem(rom: &Path, ini: &str) -> Option<String> {
    let md5 = archr_cheevos::rom_hash("n64", rom).ok()??.to_uppercase();
    let section = format!("[{md5}]");
    let mut lines = ini.lines().skip_while(|l| l.trim() != section).skip(1);
    let good = lines
        .find_map(|l| l.trim().strip_prefix("GoodName="))
        .map(str::to_owned)?;
    (!good.contains('/')).then(|| format!("{good}-{}", &md5[..8]))
}
struct Plan {
    from: Format,
    to: Format,
    steps: Vec<Step>,
}
/// What would convert. `hash` allows reading N64 ROMs (slow): without it a
/// whole-system count from standalone saves is only an estimate.
fn plan(roms_dir: &Path, offer: &Offer, ini: &Path, hash: bool) -> Option<(Plan, usize)> {
    let from = Format::of(&offer.system, (&offer.from.0, &offer.from.1))?;
    let to = Format::of(&offer.system, (&offer.to.0, &offer.to.1))?;
    if !Format::pair(from, to) {
        return None;
    }
    let dir = roms_dir.join(&offer.system);
    let memcard = |stem: &str| dir.join(format!("duckstation/memcards/{stem}_1.mcd"));
    let mut steps = Vec::new();
    let mut estimate = 0;
    let ini = if matches!(from, Format::MupenSa) || matches!(to, Format::MupenSa) {
        archr_io::read_utf8(ini, 16 << 20, archr_io::Symlinks::Follow).unwrap_or_default()
    } else {
        String::new()
    };
    let eeps = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".eep")))
        .count();
    let games = roms(&dir, offer.rom.as_deref());
    if !hash && from == Format::MupenSa {
        estimate = eeps.min(games.len());
    }
    for (stem, rom) in games {
        let simple = |ext: &str| dir.join(format!("{stem}{ext}"));
        let (source, target) = match (from, to) {
            (Format::Dsv | Format::Sav | Format::PsxSrm | Format::N64Srm, Format::Mcd) => {
                (simple(from.ext()), memcard(&stem))
            }
            (Format::Mcd, _) => (memcard(&stem), simple(to.ext())),
            (Format::MupenSa, _) | (_, Format::MupenSa) => {
                let srm = simple(".srm");
                // The standalone names saves by the ROM's MD5: read the ROM
                // only when converting, and only when something can go.
                let wanted = if from == Format::MupenSa {
                    eeps > 0 && !srm.exists()
                } else {
                    srm.is_file()
                };
                if !wanted {
                    continue;
                }
                if !hash {
                    estimate += usize::from(from == Format::N64Srm);
                    continue;
                }
                let Some(sa) = mupen_stem(&rom, &ini) else {
                    continue;
                };
                let eep = dir.join(format!("{sa}.eep"));
                // Anything beyond the EEPROM was never tested: leave the game.
                if ["mpk", "sra", "fla"].iter().any(|ext| {
                    let path = dir.join(format!("{sa}.{ext}"));
                    path.exists()
                        && !(*ext == "mpk"
                            && fs::read(&path).is_ok_and(|b| {
                                b.len() == 4 * PAK && b.chunks(PAK).all(|p| p == blank_pak())
                            }))
                }) {
                    continue;
                }
                if from == Format::MupenSa {
                    (eep, srm)
                } else {
                    (srm, eep)
                }
            }
            _ => (simple(from.ext()), simple(to.ext())),
        };
        if source.is_file() && !target.exists() {
            steps.push(Step { source, target });
        }
    }
    let count = steps.len() + estimate;
    Some((Plan { from, to, steps }, count))
}

/// The converted bytes of one save, or None when it cannot be taken.
fn convert(from: Format, to: Format, bytes: &[u8]) -> Option<Vec<u8>> {
    match (from, to) {
        (Format::Dsv, Format::Sav) => dsv_to_raw(bytes),
        (Format::Sav, Format::Dsv) => (!bytes.is_empty()).then(|| raw_to_dsv(bytes)),
        (Format::PsxSrm, Format::Mcd) | (Format::Mcd, Format::PsxSrm) => {
            (bytes.len() == CARD && bytes.starts_with(b"MC")).then(|| bytes.to_vec())
        }
        (Format::N64Srm, Format::MupenSa) => srm_to_eep(bytes),
        (Format::MupenSa, Format::N64Srm) => eep_to_srm(bytes),
        _ => None,
    }
}
/// A free name in the backup folder: never over an older copy.
fn backup_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    (1..)
        .map(|n| match n {
            1 => first.clone(),
            n => dir.join(format!("{name}.{n}")),
        })
        .find(|p| !p.exists())
        .expect("a free name")
}

/// Saves the offer would take, for the dialog. Whole N64 systems are
/// estimated (one per save that has no `.srm`), never hashed here.
pub fn count(roms_dir: &Path, offer: &Offer, ini: &Path) -> usize {
    plan(roms_dir, offer, ini, offer.rom.is_some()).map_or(0, |(_, n)| n)
}
/// The offer for a change of emulator, or None when nothing would convert.
pub fn offer(
    roms_dir: &Path,
    ini: &Path,
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
    offer.count = count(roms_dir, &offer, ini);
    (offer.count > 0).then_some(offer)
}
/// Convert: copy each original to the backup folder, then write the new
/// save beside it. Returns how many converted.
pub fn run(roms_dir: &Path, offer: &Offer, ini: &Path) -> Result<usize, String> {
    let (plan, _) = plan(roms_dir, offer, ini, true).ok_or("convert.failed")?;
    let keep = roms_dir
        .join(&offer.system)
        .join(keep_dir((&offer.to.0, &offer.to.1)));
    let mut done = 0;
    for step in &plan.steps {
        let bytes = archr_io::read_bytes(&step.source, 1 << 20, archr_io::Symlinks::Follow)
            .map_err(|e| e.to_string())?;
        let Some(converted) = convert(plan.from, plan.to, &bytes) else {
            log::warn!("Save not converted: {}", step.source.display());
            continue;
        };
        let name = step
            .source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("save");
        let copy = backup_path(&keep, name);
        archr_io::atomic_publish(&copy, true, |file| std::io::Write::write_all(file, &bytes))
            .map_err(|e| e.to_string())?;
        // Checked again right before: a save the emulator wrote meanwhile wins.
        if step.target.exists() {
            continue;
        }
        archr_io::atomic_publish(&step.target, true, |file| {
            std::io::Write::write_all(file, &converted)
        })
        .map_err(|e| e.to_string())?;
        done += 1;
    }
    Ok(done)
}

/// Planning and converting off the draw thread, one job at a time.
pub enum Event {
    Offer(Option<Box<Offer>>),
    Done(Result<usize, String>),
}
#[derive(Default)]
pub struct Runner {
    rx: Option<mpsc::Receiver<Event>>,
}
impl Runner {
    fn spawn(&mut self, job: impl FnOnce() -> Event + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("launcher-convert".into())
            .spawn(move || {
                let _ = tx.send(job());
            });
        match spawned {
            Ok(_) => self.rx = Some(rx),
            Err(error) => log::warn!("Cannot start the save conversion: {error}"),
        }
    }
    pub fn plan(
        &mut self,
        paths: &crate::paths::Paths,
        system: String,
        rom: Option<PathBuf>,
        from: (String, String),
        to: (String, String),
    ) {
        let (roms_dir, ini) = (paths.at(ROMS), paths.at(MUPEN_INI));
        self.spawn(move || {
            Event::Offer(offer(&roms_dir, &ini, &system, rom, from, to).map(Box::new))
        });
    }
    pub fn run(&mut self, paths: &crate::paths::Paths, offer: Offer) {
        let (roms_dir, ini) = (paths.at(ROMS), paths.at(MUPEN_INI));
        self.spawn(move || Event::Done(run(&roms_dir, &offer, &ini)));
    }
    pub fn poll(&mut self) -> Option<Event> {
        let event = self.rx.as_ref()?.try_recv().ok()?;
        self.rx = None;
        Some(event)
    }
}
const ROMS: &str = "/storage/roms";
/// Where the standalone Mupen64Plus finds games' good names.
const MUPEN_INI: &str = "/usr/local/share/mupen64plus/mupen64plus.ini";

#[cfg(test)]
mod tests {
    use super::*;
    fn pair(e: &str, c: &str) -> (String, String) {
        (e.into(), c.into())
    }
    #[test]
    fn dsv_footer_comes_off_and_goes_back_byte_for_byte() {
        let raw: Vec<u8> = (0..8192u32).map(|n| (n * 7) as u8).collect();
        let dsv = raw_to_dsv(&raw);
        assert_eq!(dsv.len(), 8192 + DSV_FOOTER);
        assert_eq!(dsv_to_raw(&dsv).unwrap(), raw);
        // The footer DraStic itself wrote for an 8 KiB EEPROM.
        let tail = &dsv[dsv.len() - DSV_FOOTER + DSV_SNIP.len()..];
        assert_eq!(
            &tail[..24],
            [0, 32, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(dsv_to_raw(&raw), None, "no footer: not a .dsv");
    }
    #[test]
    fn the_blank_pak_is_mupen64plus_format() {
        use sha2::{Digest, Sha256};
        // Measured on the Controller Paks of a real Mupen64Plus-Next .srm.
        let digest: String = Sha256::digest(blank_pak())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            digest,
            "056d71a353d325330a0f44115c0d7ba91c64ef6a47ab666602d5380a97f3703b"
        );
    }
    #[test]
    fn n64_takes_the_eeprom_and_refuses_anything_else() {
        let mut eep = vec![0u8; 512];
        eep[52] = b'D';
        eep[53] = b'A';
        let srm = eep_to_srm(&eep).unwrap();
        assert_eq!(srm.len(), N64_SRM);
        assert_eq!(srm_to_eep(&srm).unwrap(), eep, "4 Kbit stays 512 bytes");
        let mut sram = srm.clone();
        sram[EEPROM + 4 * PAK + 10] = 1;
        assert_eq!(srm_to_eep(&sram), None, "SRAM data was never tested");
        let mut pak = srm.clone();
        pak[EEPROM + 0x300] = 9;
        assert_eq!(srm_to_eep(&pak), None, "a pak in use stays");
        assert_eq!(srm_to_eep(&eep_to_srm(&[0xff; 512]).unwrap()), None);
    }
    fn tree() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let roms = root.path().join("roms");
        fs::create_dir_all(roms.join("nds")).unwrap();
        fs::create_dir_all(roms.join("psx/duckstation/memcards")).unwrap();
        (root, roms)
    }
    #[test]
    fn drastic_to_melonds_keeps_the_original_and_skips_saves_already_there() {
        let (_root, roms) = tree();
        let nds = roms.join("nds");
        for game in ["Mario Kart DS", "Zelda", "Nothing"] {
            fs::write(nds.join(format!("{game}.nds")), b"rom").unwrap();
        }
        let dsv = raw_to_dsv(&[7u8; 512]);
        fs::write(nds.join("Mario Kart DS.dsv"), &dsv).unwrap();
        fs::write(nds.join("Zelda.dsv"), &dsv).unwrap();
        fs::write(nds.join("Zelda.sav"), b"newer").unwrap();
        let ini = roms.join("none-ini");
        let offer = offer(
            &roms,
            &ini,
            "nds",
            None,
            pair("drastic", "drastic-arch"),
            pair("melonds", "melonds-sa"),
        )
        .unwrap();
        assert_eq!(offer.count, 1);
        assert_eq!(
            (offer.from_name.as_str(), offer.to_name.as_str()),
            ("DraStic", "melonDS")
        );
        assert_eq!(offer.keep, "roms/nds/.antes-melonds");
        assert_eq!(run(&roms, &offer, &ini), Ok(1));
        assert_eq!(fs::read(nds.join("Mario Kart DS.sav")).unwrap(), [7u8; 512]);
        assert_eq!(fs::read(nds.join("Mario Kart DS.dsv")).unwrap(), dsv);
        assert_eq!(
            fs::read(nds.join(".antes-melonds/Mario Kart DS.dsv")).unwrap(),
            dsv
        );
        assert_eq!(fs::read(nds.join("Zelda.sav")).unwrap(), b"newer");
        // And back: the melonDS save becomes a .dsv only where none is.
        assert!(super::offer(
            &roms,
            &ini,
            "nds",
            None,
            pair("melonds", "melonds-sa"),
            pair("drastic", "drastic-arch"),
        )
        .is_none());
    }
    #[test]
    fn psx_cards_follow_the_game_file_name() {
        let (_root, roms) = tree();
        let psx = roms.join("psx");
        fs::write(psx.join("Crash.chd"), b"rom").unwrap();
        let mut card = vec![0u8; CARD];
        card[..2].copy_from_slice(b"MC");
        fs::write(psx.join("Crash.srm"), &card).unwrap();
        let ini = roms.join("none-ini");
        let offer = offer(
            &roms,
            &ini,
            "psx",
            Some(psx.join("Crash.chd")),
            pair("retroarch", "pcsx_rearmed32"),
            pair("duckstation", "duckstation-sa"),
        )
        .unwrap();
        assert_eq!(offer.keep, "roms/psx/.antes-duckstation");
        assert_eq!(run(&roms, &offer, &ini), Ok(1));
        assert_eq!(
            fs::read(psx.join("duckstation/memcards/Crash_1.mcd")).unwrap(),
            card
        );
        // Same emulator family, or a pair never checked: no offer.
        assert!(super::offer(
            &roms,
            &ini,
            "psx",
            None,
            pair("retroarch", "pcsx_rearmed32"),
            pair("retroarch", "pcsx_rearmed"),
        )
        .is_none());
    }
    /// Real saves, when a folder of them is given: ARCHR_REAL_SAVES holds
    /// a DraStic .dsv, a PCSX ReARMed card, a Mupen64Plus-Next .srm and
    /// the standalone .eep of the same game (Super Mario 64), its zipped ROM
    /// and the device's mupen64plus.ini, as copied from a device.
    #[test]
    fn real_saves_convert_both_ways() {
        let Some(dir) = std::env::var_os("ARCHR_REAL_SAVES").map(PathBuf::from) else {
            return;
        };
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let bytes = fs::read(&path).unwrap();
            match path.extension().and_then(|e| e.to_str()) {
                Some("dsv") => {
                    let raw = dsv_to_raw(&bytes).expect("a DraStic footer");
                    assert_eq!(raw_to_dsv(&raw), bytes, "{}", path.display());
                }
                Some("srm") if bytes.len() == CARD => {
                    let card = convert(Format::PsxSrm, Format::Mcd, &bytes).expect("a card");
                    assert_eq!(convert(Format::Mcd, Format::PsxSrm, &card).unwrap(), bytes);
                }
                Some("srm") if bytes.len() == N64_SRM => {
                    let eep = srm_to_eep(&bytes).expect("EEPROM only");
                    let sa = fs::read_dir(&dir)
                        .unwrap()
                        .flatten()
                        .find(|e| e.path().extension().is_some_and(|x| x == "eep"))
                        .map(|e| fs::read(e.path()).unwrap());
                    if let Some(sa) = sa {
                        assert_eq!(eep, sa, "the standalone's own .eep");
                    }
                    assert_eq!(srm_to_eep(&eep_to_srm(&eep).unwrap()).unwrap(), eep);
                    assert_eq!(&eep_to_srm(&eep).unwrap()[..], &bytes[..], "byte for byte");
                }
                _ => {}
            }
        }
        // Both ways through the whole path: the ROM's MD5 and good name
        // find the standalone's own file name.
        let file = |ext: &str| {
            fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .find(|p| ext != "srm" || fs::metadata(p).is_ok_and(|m| m.len() == N64_SRM as u64))
        };
        let (Some(zip), Some(srm), Some(eep), Some(ini)) =
            (file("zip"), file("srm"), file("eep"), file("ini"))
        else {
            return;
        };
        for (from, to, keep) in [
            (
                pair("retroarch", "mupen64plus_next"),
                pair("mupen64plus", "mupen64plus-arch"),
                &srm,
            ),
            (
                pair("mupen64plus", "mupen64plus-arch"),
                pair("retroarch", "mupen64plus_next"),
                &eep,
            ),
        ] {
            let root = tempfile::tempdir().unwrap();
            let n64 = root.path().join("n64");
            fs::create_dir_all(&n64).unwrap();
            fs::copy(&zip, n64.join(zip.file_name().unwrap())).unwrap();
            fs::copy(keep, n64.join(keep.file_name().unwrap())).unwrap();
            let game = Some(n64.join(zip.file_name().unwrap()));
            let offer = offer(root.path(), &ini, "n64", game, from, to).expect("an offer");
            assert_eq!(run(root.path(), &offer, &ini), Ok(1));
            let made = if keep == &srm { &eep } else { &srm };
            assert_eq!(
                fs::read(n64.join(made.file_name().unwrap())).unwrap(),
                fs::read(made).unwrap(),
                "{}",
                made.display()
            );
        }
    }
}
