//! Netplay lobby host: the RetroArch room list plus LAN discovery, matched against the
//! local library the way EmulationStation's GuiNetPlay did. One short-lived thread per
//! refresh; the UI thread only polls for the result.
use crate::{paths::Paths, worker::definitions};
use archr_config::SystemCfg;
use archr_index::{cache, clean_name, SystemDef};
use archr_ui::netplay::{Group, Join, Lobby, Rom, Room};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    net::UdpSocket,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{channel, Receiver},
    time::{Duration, Instant},
};

const DEFAULT_LOBBY: &str = "http://lobby.libretro.com/list/";
const DEFAULT_PORT: u16 = 55435;
const QUERY_MAGIC: [u8; 4] = *b"RANQ";
const RESPONSE_MAGIC: [u8; 4] = *b"RANS";
const LAN_WAIT: Duration = Duration::from_millis(2500);
/// Larger ROMs are not hashed; their rooms show an unknown version.
const HASH_LIMIT: u64 = 64 << 20;

/// Lobby display names that differ from the catalog's core names (as in ES).
const CORES: [(&str, &str); 23] = [
    ("Beetle NeoPop", "mednafen_ngp"),
    ("Beetle PCE Fast", "pce"),
    ("Beetle SuperGrafx", "mednafen_supergrafx"),
    ("Beetle VB", "vb"),
    ("Beetle WonderSwan", "mednafen_wswan"),
    ("DOSBox-SVN", "dosbox"),
    ("EightyOne", "81"),
    ("FB Alpha 2012 Neo Geo", "fbalpha2012_neogeo"),
    ("FB Alpha 2012", "fbalpha2012"),
    ("FB Alpha", "fbalpha"),
    ("FinalBurn Neo", "fbneo"),
    ("Game & Watch", "gw"),
    ("Genesis Plus GX", "genesisplusgx"),
    ("MAME 2000", "imame4all"),
    ("MAME 2003 (0.78)", "mame2003"),
    ("MAME 2003-Plus", "mame078plus"),
    ("MAME 2010", "mame0139"),
    ("MAME 2014", "mame2014"),
    ("PCSX-ReARMed", "pcsx_rearmed"),
    ("Snes9x 2002", "pocketsnes"),
    ("Snes9x 2010", "snes9x_next"),
    ("TGB Dual", "tgbdual"),
    ("VICE x64", "vice"),
];

/// One advertised room, from the internet lobby or from a LAN reply.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub game: String,
    pub crc: u32,
    pub core: String,
    pub host: String,
    pub country: String,
    pub ip: String,
    pub port: u16,
    pub relay: Option<(String, u16)>,
    pub lan: bool,
    pub locked: bool,
    pub latency_ms: Option<u32>,
}

pub fn parse_lobby(json: &str) -> Result<Vec<Entry>, String> {
    let rooms: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let rooms = rooms.as_array().ok_or("lobby is not a list")?;
    Ok(rooms
        .iter()
        .filter(|room| room.get("fields").is_some())
        .map(|room| {
            let f = &room["fields"];
            let text = |key: &str| f[key].as_str().unwrap_or_default().to_owned();
            let port = |key: &str| {
                f[key]
                    .as_u64()
                    .and_then(|n| u16::try_from(n).ok())
                    .unwrap_or(0)
            };
            Entry {
                game: text("game_name"),
                crc: u32::from_str_radix(&text("game_crc"), 16).unwrap_or(0),
                core: text("core_name"),
                host: text("username"),
                country: text("country"),
                ip: text("ip"),
                port: port("port"),
                // host_method 3 is the MITM relay: clients dial the relay session.
                locked: f["has_password"] == true || f["has_spectate_password"] == true,
                relay: (f["host_method"] == 3).then(|| (text("mitm_ip"), port("mitm_port"))),
                ..Entry::default()
            }
        })
        .collect())
}

/// RetroArch's `ad_packet`: big-endian header, CRC, port and password flag, then
/// fixed NUL-padded strings (nick, frontend, core, core version, RetroArch
/// version, content, subsystem).
pub fn parse_lan(packet: &[u8], from: &str) -> Option<Entry> {
    const FIELDS: [usize; 7] = [32, 32, 32, 32, 32, 256, 256];
    if packet.len() != 16 + FIELDS.iter().sum::<usize>() || packet[..4] != RESPONSE_MAGIC {
        return None;
    }
    let word = |at: usize| u32::from_be_bytes(packet[at..at + 4].try_into().expect("4 bytes"));
    let mut at = 16;
    let strings: Vec<String> = FIELDS
        .iter()
        .map(|&len| {
            let field = &packet[at..at + len];
            at += len;
            let end = field.iter().position(|&b| b == 0).unwrap_or(len);
            String::from_utf8_lossy(&field[..end]).into_owned()
        })
        .collect();
    Some(Entry {
        game: strings[5].clone(),
        crc: word(4),
        core: strings[2].clone(),
        host: strings[0].clone(),
        ip: from.to_owned(),
        port: u16::try_from(word(8)).ok()?,
        lan: true,
        locked: word(12) != 0,
        ..Entry::default()
    })
}

/// Name normalization shared by room and library titles.
fn normalize(name: &str) -> String {
    let lower = name
        .to_lowercase()
        .replace('_', " ")
        .replace(['.', '\''], "");
    clean_name(&lower).trim().to_owned()
}

fn core_id(lobby_name: &str) -> String {
    CORES
        .iter()
        .find(|(name, _)| *name == lobby_name)
        .map_or_else(
            || lobby_name.to_lowercase().replace(' ', "_"),
            |(_, id)| (*id).to_owned(),
        )
}

/// A system that can play netplay for some core, with its library.
pub struct Local {
    pub name: String,
    pub cores: Vec<(String, String)>,
    pub games: Vec<(PathBuf, String)>,
}

impl Local {
    pub fn new(system: &SystemDef, games: Vec<archr_index::Game>) -> Self {
        Self {
            name: system.name.clone(),
            cores: system
                .emulators
                .iter()
                .flat_map(|e| e.cores.iter().map(|c| (e.name.clone(), c.name.clone())))
                .collect(),
            games: games.into_iter().map(|g| (g.path, g.name)).collect(),
        }
    }
}

fn same_title(game: &(PathBuf, String), wanted: &str) -> bool {
    let stem = game
        .0
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let squash = |s: &str| s.replace(' ', "");
    [normalize(&game.1), normalize(stem)]
        .iter()
        .any(|name| name == wanted || squash(name) == squash(wanted))
}

/// Matches a room with the library. `hash` yields a ROM's CRC32 when it can be read.
pub fn resolve(
    entry: &Entry,
    locals: &[Local],
    hash: &mut dyn FnMut(&Path) -> Option<u32>,
) -> Room {
    let core = core_id(&entry.core);
    let wanted = normalize(&entry.game);
    let capable: Vec<_> = locals
        .iter()
        .filter_map(|l| {
            let found = l.cores.iter().find(|(_, c)| c.to_lowercase() == core)?;
            Some((l, found))
        })
        .collect();
    let found = capable.iter().find_map(|(local, (emulator, core))| {
        let game = local.games.iter().find(|g| same_title(g, &wanted))?;
        Some((local, emulator, core, &game.0))
    });
    let (dial_ip, dial_port) = match &entry.relay {
        Some((ip, port)) => (ip.as_str(), *port),
        None => (entry.ip.as_str(), entry.port),
    };
    let dialable = !dial_ip.is_empty()
        && dial_port != 0
        && !dial_ip.contains("--")
        && dial_ip
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:-_".contains(&b));
    let (rom, join) = match found {
        None => (
            if capable.is_empty() {
                Rom::NoCore
            } else {
                Rom::NoRom
            },
            None,
        ),
        Some((local, emulator, core, path)) => {
            let rom = match hash(path) {
                Some(_) if entry.crc == 0 => Rom::Unknown,
                Some(crc) if crc == entry.crc => Rom::Same,
                Some(_) => Rom::Different,
                None => Rom::Unknown,
            };
            let join = dialable.then(|| Join {
                game: entry.game.clone(),
                system: local.name.clone(),
                rom: path.clone(),
                emulator: emulator.clone(),
                core: core.clone(),
                ip: dial_ip.to_owned(),
                port: dial_port,
                spectate: false,
            });
            (rom, join)
        }
    };
    let group = match (&join, entry.lan) {
        (None, _) => Group::Unavailable,
        (_, true) => Group::Lan,
        _ => Group::Online,
    };
    Room {
        group,
        game: entry.game.clone(),
        host: entry.host.clone(),
        place: if entry.lan {
            entry.ip.clone()
        } else {
            entry.country.to_uppercase()
        },
        core: entry.core.clone(),
        relay: entry
            .relay
            .as_ref()
            .map(|(ip, _)| ip.clone())
            .unwrap_or_default(),
        latency_ms: entry.latency_ms,
        locked: entry.locked,
        rom,
        join,
    }
}

/// The two ES room filters. Unset `show_missing` keeps unavailable rooms visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filters {
    pub relay_only: bool,
    pub show_missing: bool,
}
impl Filters {
    /// Accepts the settings page's names and ES's own (`NetPlayShow...`).
    pub fn from_config(config: &SystemCfg) -> Self {
        let flag = |keys: [&str; 2], default| {
            keys.iter()
                .find_map(|key| config.get_setting(key, None, None))
                .map_or(default, |v| matches!(v.as_str(), "1" | "true"))
        };
        Self {
            relay_only: flag(
                ["netplay.relay_only", "NetPlayShowOnlyRelayServerGames"],
                false,
            ),
            show_missing: flag(["netplay.show_missing", "NetPlayShowMissingGames"], true),
        }
    }
    /// ES applied the relay filter to internet rooms only.
    pub fn keeps_entry(self, entry: &Entry) -> bool {
        entry.lan || !self.relay_only || entry.relay.is_some()
    }
    pub fn keeps_room(self, room: &Room) -> bool {
        self.show_missing || room.group != Group::Unavailable
    }
}

/// Groups in display order; inside a group, matching ROMs first (as ES sorted).
pub fn arrange(mut rooms: Vec<Room>) -> Vec<Room> {
    let rank = |room: &Room| match room.rom {
        Rom::Same => 0,
        Rom::Unknown | Rom::Different => 1,
        _ => 2,
    };
    rooms.sort_by_key(|room| (room.group, rank(room)));
    rooms
}

fn crc32_of(path: &Path) -> Option<u32> {
    let mut file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > HASH_LIMIT {
        return None;
    }
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = vec![0; 64 << 10];
    loop {
        match file.read(&mut buffer).ok()? {
            0 => return Some(hasher.finalize()),
            n => hasher.update(&buffer[..n]),
        }
    }
}

/// Broadcast the discovery query and collect replies for a short while.
fn discover(port: u16) -> Vec<Entry> {
    let mut found: Vec<Entry> = Vec::new();
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        return found;
    };
    let began = Instant::now();
    if socket.set_broadcast(true).is_err()
        || socket
            .send_to(&QUERY_MAGIC, ("255.255.255.255", port))
            .is_err()
    {
        return found;
    }
    let mut buffer = [0u8; 1024];
    while let Some(left) = LAN_WAIT.checked_sub(began.elapsed()) {
        if socket.set_read_timeout(Some(left)).is_err() {
            break;
        }
        let Ok((len, from)) = socket.recv_from(&mut buffer) else {
            break;
        };
        if let Some(mut entry) = parse_lan(&buffer[..len], &from.ip().to_string()) {
            entry.latency_ms = Some(began.elapsed().as_millis() as u32);
            let known = |e: &Entry| {
                (&e.ip, e.port, &e.host, e.crc) == (&entry.ip, entry.port, &entry.host, entry.crc)
            };
            if !found.iter().any(known) {
                found.push(entry);
            }
        }
    }
    found
}

fn download(paths: &Paths, url: &str) -> Result<String, String> {
    let output = Command::new(paths.at("/usr/bin/curl"))
        .args(["-fsS", "--connect-timeout", "5", "--max-time", "12"])
        .args([
            "--max-filesize",
            "4194304",
            "--proto",
            "=http,https",
            "--",
            url,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("curl: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// Rooms ready to display, and whether the internet lobby failed.
pub fn refresh(paths: &Paths) -> (Vec<Room>, bool) {
    let config = SystemCfg::load(&paths.config).ok();
    let setting = |key: &str| config.as_ref().and_then(|c| c.get_setting(key, None, None));
    let url = setting("netplay.lobby").unwrap_or_else(|| DEFAULT_LOBBY.to_owned());
    let port = setting("netplay.port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let lan = std::thread::spawn(move || discover(port));
    let online = download(paths, &url).and_then(|body| parse_lobby(&body));
    let failed = online.is_err();
    if let Err(error) = &online {
        log::warn!("Netplay lobby unavailable: {error}");
    }
    let filters = config.as_ref().map_or(
        Filters {
            relay_only: false,
            show_missing: true,
        },
        Filters::from_config,
    );
    let mut entries = lan.join().unwrap_or_default();
    entries.extend(online.unwrap_or_default());
    entries.retain(|entry| filters.keeps_entry(entry));
    let locals: Vec<_> = definitions(paths)
        .unwrap_or_default()
        .iter()
        .map(|system| {
            let games = cache::load_games_with_mapper(&paths.cache, system, &|p| paths.remap(p))
                .into_iter()
                .map(|mut game| {
                    game.path = paths.remap(&game.path);
                    game
                })
                .collect();
            Local::new(system, games)
        })
        .collect();
    let mut hashes: HashMap<PathBuf, Option<u32>> = HashMap::new();
    let mut hash = |path: &Path| {
        *hashes
            .entry(path.to_owned())
            .or_insert_with(|| crc32_of(path))
    };
    let rooms = entries
        .iter()
        .map(|e| resolve(e, &locals, &mut hash))
        .filter(|room| filters.keeps_room(room))
        .collect();
    (arrange(rooms), failed)
}

/// Event-loop adapter: at most one refresh in flight.
pub struct Runtime {
    paths: Paths,
    pending: Option<Receiver<(Vec<Room>, bool)>>,
    enabled: Option<Receiver<bool>>,
}
impl Runtime {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            pending: None,
            enabled: None,
        }
    }
    pub fn refresh(&mut self) {
        let (send, receive) = channel();
        let paths = self.paths.clone();
        std::thread::spawn(move || {
            let _ = send.send(refresh(&paths));
        });
        self.pending = Some(receive);
    }
    /// True when the lobby changed. A closed lobby drops the reply.
    pub fn poll(&mut self, lobby: &mut Lobby) -> bool {
        let Some(Ok((rooms, failed))) = self.pending.as_ref().map(Receiver::try_recv) else {
            return false;
        };
        self.pending = None;
        lobby.set(rooms, failed);
        true
    }
    /// Re-read `global.netplay` off the UI thread; the answer comes from `enabled`.
    pub fn recheck(&mut self) {
        let (send, receive) = channel();
        let config = self.paths.config.clone();
        std::thread::spawn(move || {
            let on = SystemCfg::load(&config)
                .ok()
                .and_then(|cfg| cfg.get_setting("netplay", None, None))
                .is_some_and(|value| value == "1");
            let _ = send.send(on);
        });
        self.enabled = Some(receive);
    }
    pub fn enabled(&mut self) -> Option<bool> {
        let answer = self.enabled.as_ref()?.try_recv().ok()?;
        self.enabled = None;
        Some(answer)
    }
    pub fn close(&mut self) {
        self.pending = None;
    }
}
