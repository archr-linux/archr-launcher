//! Netplay lobby host: the RetroArch room list plus LAN discovery, matched against the
//! local library the way EmulationStation's GuiNetPlay did. One short-lived thread per
//! refresh; the UI thread only polls for the result.
use crate::{handoff::is_ip, paths::Paths, worker::definitions};
use archr_config::SystemCfg;
use archr_index::{cache, clean_name, SystemDef};
use archr_ui::netplay::{Group, Join, Lobby, Rom, Room};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    net::UdpSocket,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        mpsc::{channel, Receiver},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};

const DEFAULT_LOBBY: &str = "https://lobby.libretro.com/list/";
const DEFAULT_PORT: u16 = 55435;
const QUERY_MAGIC: [u8; 4] = *b"RANQ";
const RESPONSE_MAGIC: [u8; 4] = *b"RANS";
const LAN_WAIT: Duration = Duration::from_millis(2500);
/// Larger ROMs are not hashed; their rooms show an unknown version.
const HASH_LIMIT: u64 = 64 << 20;
/// Per refresh: ROM bytes read and time spent hashing. Beyond it, rooms show an unknown version.
const HASH_BUDGET: (u64, Duration) = (256 << 20, Duration::from_secs(8));

/// Lobby core names whose alphanumerics differ from the ArchR catalog id.
const CORE_ALIASES: [(&str, &str); 5] = [
    ("eightyone", "81"),
    ("finalburnneo", "fbneo"),
    ("gamewatch", "gw"),
    ("beetleneopop", "beetlengp"),
    ("beetlewonderswan", "beetlewswan"),
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
    /// Relay address, port and session id (host_method 3).
    pub relay: Option<(String, u16, String)>,
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
                relay: (f["host_method"] == 3)
                    .then(|| (text("mitm_ip"), port("mitm_port"), text("mitm_session"))),
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

/// Lowercase alphanumerics: "Genesis Plus GX" and `genesis_plus_gx` compare equal.
fn core_key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// A system's RetroArch cores (the only ones that speak netplay) and its library,
/// indexed by normalized title (plain and without spaces).
pub struct Local {
    pub name: String,
    pub cores: Vec<(String, String)>,
    games: HashMap<String, PathBuf>,
}

impl Local {
    pub fn new(system: &SystemDef, games: Vec<archr_index::Game>) -> Self {
        let cores = system
            .emulators
            .iter()
            .flat_map(|e| e.cores.iter().map(|c| (e.name.clone(), c.name.clone())))
            .collect();
        Self::from_parts(
            &system.name,
            cores,
            games.into_iter().map(|g| (g.path, g.name)),
        )
    }
    pub fn from_parts(
        name: &str,
        cores: Vec<(String, String)>,
        games: impl Iterator<Item = (PathBuf, String)>,
    ) -> Self {
        let mut index = HashMap::new();
        for (path, title) in games {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            for name in [normalize(&title), normalize(stem)] {
                index
                    .entry(name.replace(' ', ""))
                    .or_insert_with(|| path.clone());
            }
        }
        Self {
            name: name.to_owned(),
            cores: cores
                .into_iter()
                .filter(|(e, _)| e == "retroarch")
                .collect(),
            games: index,
        }
    }
    fn find(&self, normalized: &str) -> Option<&PathBuf> {
        self.games.get(&normalized.replace(' ', ""))
    }
}

fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["zip", "7z", "rar"].contains(&e.to_ascii_lowercase().as_str()))
}

/// Matches a room with the library. `hash` yields a ROM's CRC32 when it can be read
/// (for a zip, the CRC of its first entry).
pub fn resolve(
    entry: &Entry,
    locals: &[Local],
    hash: &mut dyn FnMut(&Path) -> Option<u32>,
) -> Room {
    let lobby_core = core_key(&entry.core);
    let wanted_core = CORE_ALIASES
        .iter()
        .find(|(name, _)| *name == lobby_core)
        .map_or(lobby_core, |(_, id)| (*id).to_owned());
    let wanted = normalize(&entry.game);
    let capable: Vec<_> = locals
        .iter()
        .filter_map(|l| {
            let found = l.cores.iter().find(|(_, c)| core_key(c) == wanted_core)?;
            Some((l, found))
        })
        .collect();
    let found = capable
        .iter()
        .find_map(|(local, (emulator, core))| Some((local, emulator, core, local.find(&wanted)?)));
    // Relay rooms dial the relay and need a valid session id.
    let (dial_ip, dial_port, session) = match &entry.relay {
        Some((ip, port, session)) => (ip.as_str(), *port, session.as_str()),
        None => (entry.ip.as_str(), entry.port, ""),
    };
    let dialable = is_ip(dial_ip)
        && dial_port != 0
        && session.bytes().all(|b| b.is_ascii_alphanumeric())
        && (entry.relay.is_none() || !session.is_empty());
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
                // An archive's CRC is not RetroArch's content CRC.
                Some(_) if !is_archive(path) => Rom::Different,
                _ => Rom::Unknown,
            };
            let join = dialable.then(|| Join {
                game: entry.game.clone(),
                system: local.name.clone(),
                rom: path.clone(),
                emulator: emulator.clone(),
                core: core.clone(),
                ip: dial_ip.to_owned(),
                port: dial_port,
                session: session.to_owned(),
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
            .map(|(ip, ..)| ip.clone())
            .unwrap_or_default(),
        latency_ms: entry.latency_ms,
        locked: entry.locked,
        rom,
        join,
    }
}

/// The two ES room filters, as the settings page stores them. Unset: all rooms shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filters {
    pub relay_only: bool,
    pub show_missing: bool,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            relay_only: false,
            show_missing: true,
        }
    }
}
impl Filters {
    pub fn from_config(config: &SystemCfg) -> Self {
        let flag = |key, default| {
            config
                .get_setting(key, None, None)
                .map_or(default, |v| matches!(v.as_str(), "1" | "true"))
        };
        Self {
            relay_only: flag("netplay.relay_only", false),
            show_missing: flag("netplay.show_missing", true),
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

/// ROM CRCs remembered across refreshes, valid while size and mtime are unchanged.
pub type HashCache = HashMap<PathBuf, (u64, Option<SystemTime>, Option<u32>)>;

/// CRC32 of the first entry of a zip, from its central directory.
fn zip_crc(file: &mut File, len: u64) -> Option<u32> {
    let tail = len.min(65557);
    let mut buffer = vec![0; tail as usize];
    file.seek(SeekFrom::Start(len - tail)).ok()?;
    file.read_exact(&mut buffer).ok()?;
    let eocd = buffer.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    let offset = u32::from_le_bytes(buffer.get(eocd + 16..eocd + 20)?.try_into().ok()?);
    let mut header = [0; 20];
    file.seek(SeekFrom::Start(offset.into())).ok()?;
    file.read_exact(&mut header).ok()?;
    (&header[..4] == b"PK\x01\x02").then(|| u32::from_le_bytes(header[16..].try_into().expect("4")))
}

fn crc32_of(path: &Path, len: u64) -> Option<u32> {
    let mut file = File::open(path).ok()?;
    if is_archive(path) {
        let zip = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        return if zip { zip_crc(&mut file, len) } else { None };
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

/// Cached CRC lookup with a per-refresh read budget; over budget gives None (unknown).
struct Hasher<'a> {
    cache: &'a mut HashCache,
    bytes_left: u64,
    deadline: Instant,
}
impl Hasher<'_> {
    fn crc(&mut self, path: &Path) -> Option<u32> {
        let meta = std::fs::metadata(path).ok()?;
        let key = (meta.len(), meta.modified().ok());
        if let Some((len, mtime, crc)) = self.cache.get(path) {
            if (*len, *mtime) == key {
                return *crc;
            }
        }
        let archive = is_archive(path);
        let cost = if archive { 0 } else { meta.len() };
        if cost > HASH_LIMIT || cost > self.bytes_left || Instant::now() > self.deadline {
            return None;
        }
        self.bytes_left -= cost;
        let crc = crc32_of(path, meta.len());
        self.cache.insert(path.to_owned(), (key.0, key.1, crc));
        crc
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
    refresh_with(paths, &mut HashCache::new())
}

pub fn refresh_with(paths: &Paths, hashes: &mut HashCache) -> (Vec<Room>, bool) {
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
    let filters = config
        .as_ref()
        .map_or_else(Filters::default, Filters::from_config);
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
    let mut hasher = Hasher {
        cache: hashes,
        bytes_left: HASH_BUDGET.0,
        deadline: Instant::now() + HASH_BUDGET.1,
    };
    let rooms = entries
        .iter()
        .map(|e| resolve(e, &locals, &mut |path| hasher.crc(path)))
        .filter(|room| filters.keeps_room(room))
        .collect();
    (arrange(rooms), failed)
}

/// Event-loop adapter: at most one refresh in flight.
pub struct Runtime {
    paths: Paths,
    pending: Option<Receiver<(Vec<Room>, bool)>>,
    enabled: Option<Receiver<bool>>,
    hashes: Arc<Mutex<HashCache>>,
}
impl Runtime {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            pending: None,
            enabled: None,
            hashes: Arc::default(),
        }
    }
    /// At most one refresh runs at a time, even across closing and reopening the
    /// lobby: a running one is reused, a finished and unread one is replaced.
    pub fn refresh(&mut self) {
        if let Some(running) = &self.pending {
            if matches!(
                running.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ) {
                return;
            }
        }
        let (send, receive) = channel();
        let paths = self.paths.clone();
        let hashes = Arc::clone(&self.hashes);
        std::thread::spawn(move || {
            let mut cache = hashes.lock().unwrap_or_else(|e| e.into_inner());
            let _ = send.send(refresh_with(&paths, &mut cache));
        });
        self.pending = Some(receive);
    }
    /// Test hook: a refresh whose thread ended without sending a result.
    #[doc(hidden)]
    pub fn simulate_dead_refresh(&mut self) {
        self.pending = Some(channel().1);
    }
    /// True when the lobby changed. A closed lobby drops the reply.
    pub fn poll(&mut self, lobby: &mut Lobby) -> bool {
        let (rooms, failed) = match self.pending.as_ref().map(Receiver::try_recv) {
            Some(Ok(done)) => done,
            // The refresh thread died without answering: report a failed lobby.
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => (Vec::new(), true),
            _ => return false,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_budget_limits_reads_and_leaves_the_rest_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.rom"), dir.path().join("b.rom"));
        std::fs::write(&a, vec![1; 100]).unwrap();
        std::fs::write(&b, vec![2; 100]).unwrap();
        let mut cache = HashCache::new();
        let mut hasher = Hasher {
            cache: &mut cache,
            bytes_left: 150,
            deadline: Instant::now() + Duration::from_secs(60),
        };
        assert!(hasher.crc(&a).is_some());
        assert_eq!(hasher.crc(&b), None, "over the byte budget");
        assert!(hasher.crc(&a).is_some(), "cached files are free");
        hasher.bytes_left = 1000;
        hasher.deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(hasher.crc(&b), None, "past the deadline");
    }
}
