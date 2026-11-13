//! Serial settings worker. No process, config lock or sysfs access on SDL's thread.
mod bluetooth;
mod cheevos;
pub use cheevos::{TOKEN as CHEEVOS_TOKEN, USER as CHEEVOS_USER};
mod controls;
mod diag;
mod games;
mod info;
mod levels;
mod mangohud;
pub use mangohud::supported as mangohud_supported;
mod pages;
mod raimport;
mod roms;
pub(crate) use pages::{scrape_outcome, start_scrape};
pub use roms::migration_notice;
mod runtime;
mod storage;
mod updates;
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_ui::settings::{Id, Information, Page, Settings};
pub use runtime::Runtime;
use std::{
    collections::BTreeMap,
    fs,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc,
    thread,
    time::Duration,
};

pub struct Backend {
    paths: Paths,
    identity: std::sync::OnceLock<info::Identity>,
    cpu_ticks: std::sync::Mutex<BTreeMap<u32, (u64, u64)>>,
}
impl Backend {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            identity: std::sync::OnceLock::new(),
            cpu_ticks: std::sync::Mutex::new(BTreeMap::new()),
        }
    }
    fn read(&self, path: &str) -> String {
        archr_io::read_utf8(&self.paths.at(path), 64 * 1024, archr_io::Symlinks::Follow)
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
    fn level_command(&self, id: Id, args: &[&str]) -> Result<levels::Reading, String> {
        let text = self.command_with_timeout("archr-levels", args, Duration::from_secs(3))?;
        levels::Reading::parse(id, &text).ok_or_else(|| "Invalid level response".into())
    }
    fn set_level(&self, id: Id, value: &str) -> Result<levels::Reading, String> {
        if !value
            .parse::<u8>()
            .is_ok_and(|n| n <= 100 && (id != Id::Brightness || n >= 5))
        {
            return Err("Invalid setting value".into());
        }
        self.level_command(id, &[Self::level_kind(id), value, "seq"])
    }
    /// A level hold on archr-levels; returns the level and its source version.
    fn level(&self, id: Id, level: archr_ui::settings::Level) -> Result<levels::Reading, String> {
        use archr_ui::settings::Level;
        let kind = Self::level_kind(id);
        let short = Duration::from_secs(3);
        match level {
            Level::Hold(up) => {
                let direction = if up { "up" } else { "down" };
                return self.level_command(id, &[kind, "hold", direction, "seq"]);
            }
            Level::Renew => self.command_with_timeout("archr-levels", &["renew"], short)?,
            Level::Release => self.command_with_timeout("archr-levels", &["release"], short)?,
        };
        self.level_command(id, &[kind, "seq"])
    }
    fn level_kind(id: Id) -> &'static str {
        if id == Id::Volume {
            "volume"
        } else {
            "brightness"
        }
    }
    fn cfg(&self) -> Result<SystemCfg, String> {
        SystemCfg::load(&self.paths.config).map_err(|e| e.to_string())
    }
    fn save(&self, id: Id, value: &str) -> Result<(), String> {
        SystemCfg::set_and_save(&self.paths.config, &self.paths.lock, id.key(), value)
            .map_err(|e| e.to_string())
    }
    /// Fixtures have a private executable namespace: missing scripts cannot fall
    /// through to host programs. Capture to files to avoid pipe backpressure.
    /// A system tool, in the fixture namespace when tests set a root.
    fn tool(&self, name: &str) -> Command {
        let mut command = Command::new(self.paths.at(&format!("/usr/bin/{name}")));
        if let Some(root) = &self.paths.root {
            command
                .env("ARCHR_LAUNCHER_ROOT", root)
                .env("PATH", self.paths.at("/usr/bin"));
        }
        command
    }
    fn command(&self, name: &str, args: &[&str]) -> Result<String, String> {
        self.command_with_timeout(name, args, Duration::from_secs(20))
    }
    fn command_with_timeout(
        &self,
        name: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<String, String> {
        self.command_with_input(name, args, None, timeout)
    }
    fn command_with_input(
        &self,
        name: &str,
        args: &[&str],
        input: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<String, String> {
        self.command_until(name, args, input, timeout, &AtomicBool::new(false))
    }
    /// A command whose process group is killed once `cancel` is set.
    fn command_until(
        &self,
        name: &str,
        args: &[&str],
        input: Option<&[u8]>,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let mut command = self.tool(name);
        command.args(args);
        let output =
            crate::process::capture_with_input(&mut command, input, timeout, cancel, 64 * 1024)
                .map_err(|e| format!("{name}: {e}"))?;
        if output.status.success() {
            String::from_utf8(output.stdout)
                .map(|s| s.trim().to_owned())
                .map_err(|e| e.to_string())
        } else {
            Err(format!(
                "{name}: {}; {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
    /// What archr-usb-advisor wrote: plugged now, seen before, the table.
    fn adapters(&self, forget: Option<String>) -> Result<Box<archr_ui::usb::Adapters>, String> {
        // Reading never fails: missing files are an empty page.
        let forgot = forget.map(|id| self.command("archr-usb-advisor", &["forget", &id]));
        let read = |path: &str| {
            archr_io::read_utf8(&self.paths.at(path), 1 << 20, archr_io::Symlinks::Follow)
                .unwrap_or_default()
        };
        let adapters = archr_ui::usb::Adapters::new(
            &read(archr_ui::usb::ADVICE),
            &read(archr_ui::usb::SEEN),
            &read(archr_ui::usb::TABLE),
        );
        // A failed forget leaves the adapter listed: the page shows that.
        if let Some(Err(error)) = forgot {
            log::warn!("Cannot forget the adapter: {error}");
        }
        Ok(Box::new(adapters))
    }
    pub fn load(&self) -> Result<Settings, String> {
        self.load_with_levels().map(|(state, _)| state)
    }
    fn load_with_levels(&self) -> Result<Loaded, String> {
        let cfg = self.cfg()?;
        let mut state = Settings::default();
        let ids = [
            Page::Sound,
            Page::System,
            Page::Region,
            Page::Resume,
            Page::Power,
        ]
        .iter()
        .flat_map(|page| page.rows().iter().map(|row| row.id))
        .chain([Id::Buttons, Id::ModulesAuto, Id::ErrorReports]);
        for id in ids {
            if let Some(value) = cfg.get_setting(id.key(), None, None) {
                state.values.insert(id, id.normalize(&value));
            }
        }
        self.load_pages(&cfg, &mut state);
        self.load_storage(&cfg, &mut state);
        self.load_mangohud(&cfg, &mut state);
        self.load_cheevos(&cfg, &mut state);
        self.load_controls(&cfg, &mut state);
        state.values.insert(
            Id::DebugMode,
            diag::debug_value(cfg.get_setting("system.loglevel", None, None)),
        );
        // Keep source versions with the snapshot: notices may overtake this load.
        let mut readings = BTreeMap::new();
        for id in [Id::Volume, Id::Brightness] {
            if let Ok(reading) = self.level_command(id, &[Self::level_kind(id), "seq"]) {
                state.values.insert(id, reading.value.clone());
                readings.insert(id, reading);
            }
        }
        let mut errors = Vec::new();
        for (id, program, args) in [
            (Id::Audio, "set-audio", &["list"][..]),
            (Id::Timezone, "timeinfo", &["timezones"][..]),
        ] {
            match self.command(program, args) {
                Ok(output) => {
                    // set-audio lists one sink name per line; names may hold commas.
                    let separators: &[char] = if id == Id::Audio {
                        &['\n']
                    } else {
                        &['\n', ',']
                    };
                    let mut options: Vec<String> = output
                        .split(separators)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    if id == Id::Timezone {
                        options.push("UTC".into());
                    }
                    // The sink list is authoritative: a stale saved name
                    // (an old ES "headphone") is never offered.
                    let current = state.value(id).to_owned();
                    if id != Id::Audio && !current.is_empty() {
                        options.push(current);
                    }
                    options.sort();
                    options.dedup();
                    state.options.insert(id, options);
                }
                Err(error) => errors.push(error),
            }
        }
        let sinks = state.options.get(&Id::Audio).cloned().unwrap_or_default();
        let current = self
            .command("set-audio", &["get"])
            .ok()
            .filter(|sink| sinks.contains(sink))
            .or_else(|| sinks.first().cloned());
        match current {
            Some(sink) => state.values.insert(Id::Audio, sink),
            None => state.values.remove(&Id::Audio),
        };
        let current = state.value(Id::Suspend).to_owned();
        let mut modes: Vec<String> = self
            .read("/sys/power/state")
            .split_whitespace()
            .filter(|v| *v != "disk")
            .map(str::to_owned)
            .collect();
        modes.extend(["default".into(), "off".into(), current]);
        modes.sort();
        modes.dedup();
        state.options.insert(Id::Suspend, modes);
        let available =
            self.read("/sys/devices/system/cpu/cpufreq/policy0/scaling_available_governors");
        let governors = Id::Governor
            .choices()
            .iter()
            .filter(|v| available.split_whitespace().any(|g| g == **v))
            .map(|v| (*v).into())
            .collect();
        state.options.insert(Id::Governor, governors);
        state.info = self.refresh_info();
        state.values.insert(Id::Update, state.info.version.clone());
        state.loading = false;
        if !errors.is_empty() {
            let error = errors.join("; ");
            log::warn!("Settings discovery failed: {error}");
            state.fail(None, error);
        }
        Ok((state, readings))
    }
    pub fn apply(&self, id: Id, value: &str) -> Result<String, String> {
        if matches!(id, Id::Volume | Id::Brightness) {
            return self.set_level(id, value).map(|reading| reading.value);
        }
        if id == Id::DebugMode {
            return self.apply_debug(value);
        }
        if let Some(result) = self
            .apply_page(id, value)
            .or_else(|| self.apply_mangohud(id, value))
            .or_else(|| self.apply_cheevos(id, value))
            .or_else(|| self.apply_controls(id, value))
        {
            return result;
        }
        let valid = match id {
            Id::SoundsVolume => value.parse::<u8>().is_ok_and(|n| n <= 100),
            Id::Sounds | Id::Turbo | Id::Gpu | Id::Governor | Id::Language | Id::ErrorReports => {
                id.choices().contains(&value)
            }
            Id::Buttons => archr_ui::pad::bindings_text(&archr_ui::pad::bindings(value)) == value,
            Id::Suspend => {
                matches!(value, "default" | "off")
                    || self
                        .read("/sys/power/state")
                        .split_whitespace()
                        .any(|v| v == value && v != "disk")
            }
            Id::Timezone => {
                !value.is_empty()
                    && !value
                        .split('/')
                        .any(|v| v.is_empty() || v == "." || v == "..")
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
            }
            Id::Audio => {
                !value.is_empty() && !value.contains(['\n', '\r', '\0']) && value.len() <= 1024
            }
            Id::Update => false,
            _ => false,
        };
        if !valid {
            return Err("Invalid setting value".into());
        }
        // Legacy scripts own these keys. Never pre-save, restore or write twice.
        let script = match id {
            Id::Suspend => Some(("suspendmode", value)),
            Id::Gpu => Some(("gpudriver", value)),
            Id::Turbo => Some(("turbomode", if value == "1" { "enable" } else { "disable" })),
            _ => None,
        };
        if let Some((name, argument)) = script {
            self.command(name, &[argument])?;
            let stored = self.cfg()?.get_setting(id.key(), None, None);
            if id == Id::Suspend && value == "default" {
                return Ok(stored.unwrap_or_else(|| "off".into()));
            }
            if stored.as_deref() != Some(value) {
                return Err(format!(
                    "{name}: successful exit without persisting {}={value}",
                    id.key()
                ));
            }
            return Ok(value.into());
        }
        // Runtime-only operations must succeed before the single atomic publish.
        match id {
            Id::Audio => {
                self.command("set-audio", &["set", value])?;
            }
            Id::Timezone => {
                self.command("archr-config", &["tz", value])?;
            }
            Id::Governor => {
                self.command(
                    "bash",
                    &[
                        "-lc",
                        ". \"$1\"; \"$2\"",
                        "archr-settings",
                        self.paths
                            .at("/etc/profile.d/099-freqfunctions")
                            .to_str()
                            .ok_or("Invalid path")?,
                        value,
                    ],
                )?;
            }
            _ => {}
        }
        self.save(id, value)?;
        if id == Id::ErrorReports && value == "0" {
            crate::reports::purge(&self.paths);
        }
        if id == Id::Timezone && self.paths.root.is_none() {
            unsafe extern "C" {
                fn tzset();
            }
            // SAFETY: no pointer arguments; libc serializes timezone reloads
            // with localtime_r. Environment variables are not mutated here.
            unsafe {
                tzset();
            }
        }
        Ok(value.into())
    }
}

pub(super) enum Work {
    Load,
    Set(Id, String),
    Level(Id, archr_ui::settings::Level),
    /// A Games page row of the named system.
    Game(String, Id, String),
    Info,
    /// `usbgadget status`, polled while Services shows a network USB mode.
    Usb,
    Scan,
    BtScan,
    Bt(archr_ui::settings::bluetooth::Action, String),
    Connect(String, String),
    /// The saved network after the Wi-Fi switch went on, for that switch
    /// generation, on the Wi-Fi lane: the network is read when its turn
    /// comes, and the flag cancels it (skipped, or wifictl killed).
    Rejoin(u64, std::sync::Arc<AtomicBool>),
    Forget(String),
    Variants(String),
    Login(String, String),
    CheevosLogin(String, String),
    CheevosLogout,
    CheevosKey(String),
    /// The account card's points; its own lane, like the radio scans.
    CheevosProfile,
    Scrape,
    Theme(String),
    Appearance(Vec<(Id, String)>),
    Storage(archr_ui::settings::storage::Action),
    Store(archr_ui::settings::store::Action),
    /// Import from RetroArch: look at the folder, or import (backup first?).
    RaScan,
    RaImport(bool, Box<archr_ui::settings::raimport::Found>),
    /// ROMs on the PC: the state, the preview and the names.
    Roms,
    /// Network › USB adapters, after forgetting this one if given.
    Adapters(Option<String>),
    /// System › Error reports: the reporter's folders.
    Reports,
    /// Diagnostics and BIOS: their own lane (`diag::Slow`).
    Diag,
    Report,
    SendReport(String),
    Bios,
    BiosInstall(String),
}
/// A storage action's outcome and the page's refreshed facts.
type StorageReply = (
    Result<&'static str, String>,
    Box<archr_ui::settings::storage::Storage>,
);
type Loaded = (Settings, BTreeMap<Id, levels::Reading>);
enum Reply {
    Loaded(Result<Box<Loaded>, String>),
    Set(Id, Result<String, String>),
    SetLevel(Id, Result<levels::Reading, String>),
    Level(Id, Result<levels::Reading, String>),
    LiveLevel(Id, levels::Reading),
    Info(Information),
    Usb(archr_ui::settings::network::Usb),
    Scan(
        u64,
        Result<(Vec<archr_ui::settings::network::Network>, String), String>,
    ),
    BtScan(Result<Vec<archr_ui::settings::bluetooth::Device>, String>),
    Bt(Result<(), String>),
    Connect(Result<(), String>),
    /// The rejoin started, with this saved network.
    Rejoining(u64, String),
    Rejoined(u64, String, Result<(), String>),
    Forget(Result<(), String>),
    Variants(Vec<(String, String)>),
    Login(Result<(String, usize), String>),
    CheevosLogin(Result<(String, archr_ui::settings::cheevos::Profile), String>),
    CheevosLogout(Result<(), String>),
    CheevosKey(Result<Option<archr_cheevos::Summary>, String>),
    CheevosProfile(Result<(archr_ui::settings::cheevos::Profile, bool), archr_cheevos::Error>),
    Scrape(Result<(archr_scraper::Job, Vec<String>), String>),
    Theme(Box<(archr_ui::Theme, Option<std::path::PathBuf>)>),
    Appearance(Result<(), String>),
    Storage(StorageReply),
    /// The Storage page's facts after unified storage was switched.
    Facts(Box<archr_ui::settings::storage::Storage>),
    Store(archr_ui::settings::store::Reply),
    RaScan(archr_ui::settings::raimport::Found),
    RaImport(Result<String, String>),
    Adapters(Result<Box<archr_ui::usb::Adapters>, String>),
    Roms(
        Box<(
            archr_ui::settings::roms::Status,
            archr_ui::settings::roms::Scan,
        )>,
    ),
    Diag(Box<archr_ui::settings::diag::Facts>),
    Reports(Box<archr_ui::settings::reports::Facts>),
    Report(Result<archr_ui::settings::diag::Report, String>),
    /// `archr-report send`'s exit code.
    ReportSent(Result<i32, String>),
    Bios(Result<Vec<archr_ui::settings::bios::System>, String>),
    BiosInstall(Result<(), String>),
}
/// A radio scan takes ~20 s on the R36S: it runs on its own lane so it never
/// delays the Wi-Fi switch or any other action.
const SCAN_TIMEOUT: Duration = Duration::from_secs(45);
pub struct Worker {
    tx: mpsc::Sender<Work>,
    /// Joins and forgets, one at a time: never two wifictl connects at once,
    /// and a slow join never holds the switch or any other setting.
    wifi: mpsc::Sender<Work>,
    scans: mpsc::Sender<u64>,
    bt_scans: mpsc::Sender<()>,
    profiles: mpsc::Sender<()>,
    store: mpsc::Sender<archr_ui::settings::store::Action>,
    slow: mpsc::Sender<diag::Slow>,
    rx: mpsc::Receiver<Reply>,
    /// Dropping this stops the socket listener off-thread.
    _level_notices: std::sync::Arc<()>,
    level_sequences: BTreeMap<Id, u64>,
    pending: usize,
    scanning: bool,
    /// A scan was requested while one ran: dispatch again when it ends.
    rescan: bool,
    bt_scanning: bool,
    bt_rescan: bool,
    /// Bumped on every Wi-Fi switch change; older scans' replies are stale.
    generation: u64,
    /// Set when the switch changes: the rejoin queued or running stops.
    rejoin: Option<std::sync::Arc<AtomicBool>>,
    /// Level holds sent and not answered: a renewal waits for none.
    pub(super) levels: usize,
}
impl Worker {
    pub fn start(paths: Paths) -> Result<Self, String> {
        let (tx, requests) = mpsc::channel();
        let (results, rx) = mpsc::channel();
        let level_notices = levels::start(&paths, results.clone())?;
        let (scans, scan_requests) = mpsc::channel();
        let (bt_scans, bt_requests) = mpsc::channel();
        let (profiles, profile_requests) = mpsc::channel::<()>();
        let scan_results = results.clone();
        let bt_results = results.clone();
        let profile_results = results.clone();
        let (scan_paths, bt_paths) = (paths.clone(), paths.clone());
        let profile_paths = paths.clone();
        let (store, store_requests) = mpsc::channel();
        let (store_results, store_paths) = (results.clone(), paths.clone());
        let (slow, slow_requests) = mpsc::channel();
        let (slow_results, slow_paths) = (results.clone(), paths.clone());
        let (wifi, wifi_requests) = mpsc::channel();
        let (wifi_results, wifi_paths) = (results.clone(), paths.clone());
        thread::Builder::new()
            .name("launcher-wifi".into())
            .spawn(move || {
                let backend = Backend::new(wifi_paths);
                while let Ok(work) = wifi_requests.recv() {
                    let reply = match work {
                        Work::Connect(ssid, password) => {
                            Reply::Connect(backend.connect(&ssid, &password))
                        }
                        Work::Rejoin(generation, cancel) => {
                            // Read now, after earlier joins and forgets: a
                            // forgotten network is not rejoined.
                            let target = backend.rejoin_target();
                            let Some(ssid) = target.filter(|_| !cancel.load(Ordering::Relaxed))
                            else {
                                continue;
                            };
                            let _ = wifi_results.send(Reply::Rejoining(generation, ssid.clone()));
                            let result = backend.rejoin(&ssid, &cancel);
                            Reply::Rejoined(generation, ssid, result)
                        }
                        Work::Forget(ssid) => Reply::Forget(backend.forget(&ssid)),
                        _ => unreachable!("only joins and forgets use the Wi-Fi lane"),
                    };
                    if wifi_results.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        // Reports, checksums and pacman take seconds to minutes.
        thread::Builder::new()
            .name("launcher-diag".into())
            .spawn(move || {
                let backend = Backend::new(slow_paths);
                while let Ok(work) = slow_requests.recv() {
                    if slow_results.send(backend.slow(work)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        thread::Builder::new()
            .name("launcher-store".into())
            .spawn(move || store_lane(&store_paths, &store_requests, &store_results))
            .map_err(|e| e.to_string())?;
        thread::Builder::new()
            .name("launcher-cheevos".into())
            .spawn(move || {
                let backend = Backend::new(profile_paths);
                while profile_requests.recv().is_ok() {
                    let profile = backend.cheevos_profile();
                    if profile_results
                        .send(Reply::CheevosProfile(profile))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        // Discovery listens for ~10 s: its own lane, like the Wi-Fi scan.
        thread::Builder::new()
            .name("launcher-bluetooth".into())
            .spawn(move || {
                let backend = Backend::new(bt_paths);
                while bt_requests.recv().is_ok() {
                    if bt_results.send(Reply::BtScan(backend.bt_scan())).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        thread::Builder::new()
            .name("launcher-scan".into())
            .spawn(move || {
                let backend = Backend::new(scan_paths);
                while let Ok(generation) = scan_requests.recv() {
                    if scan_results
                        .send(Reply::Scan(generation, backend.scan()))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        thread::Builder::new()
            .name("launcher-settings".into())
            .spawn(move || {
                let backend = Backend::new(paths);
                // archr-console-id answers the same until the system changes.
                let mut console_id = None;
                while let Ok(work) = requests.recv() {
                    let reply = match work {
                        Work::Load => Reply::Loaded(backend.load_with_levels().map(Box::new)),
                        Work::Set(id @ (Id::Volume | Id::Brightness), value) => {
                            Reply::SetLevel(id, backend.set_level(id, &value))
                        }
                        Work::Set(id, value) => {
                            let reply = Reply::Set(id, backend.apply(id, &value));
                            if id == Id::MergedStorage {
                                let _ = results.send(reply);
                                Reply::Facts(Box::new(backend.storage()))
                            } else {
                                reply
                            }
                        }
                        Work::Game(system, id, value) => {
                            Reply::Set(id, backend.apply_game(&system, id, &value))
                        }
                        Work::Level(id, level) => Reply::Level(id, backend.level(id, level)),
                        Work::Info => Reply::Info(backend.refresh_info()),
                        Work::Usb => Reply::Usb(backend.usb()),
                        Work::Scan | Work::BtScan => unreachable!("scans have their own lane"),
                        Work::Bt(action, addr) => Reply::Bt(backend.bt_act(action, &addr)),
                        Work::Connect(..) | Work::Rejoin(..) | Work::Forget(_) => {
                            unreachable!("joins have the Wi-Fi lane")
                        }
                        Work::Variants(layout) => {
                            Reply::Variants(backend.keyboard_names(&["list-variants", &layout]))
                        }
                        Work::Login(user, password) => {
                            Reply::Login(backend.login(&user, &password))
                        }
                        Work::CheevosLogin(user, password) => {
                            Reply::CheevosLogin(backend.cheevos_login(&user, &password))
                        }
                        Work::CheevosLogout => Reply::CheevosLogout(backend.cheevos_logout()),
                        Work::CheevosKey(key) => Reply::CheevosKey(backend.cheevos_key(&key)),
                        Work::CheevosProfile
                        | Work::Store(_)
                        | Work::Diag
                        | Work::Report
                        | Work::SendReport(_)
                        | Work::Bios
                        | Work::BiosInstall(_) => {
                            unreachable!("these have their own lanes")
                        }
                        Work::Scrape => Reply::Scrape(backend.scrape()),
                        Work::Theme(name) => Reply::Theme(Box::new(crate::appearance::load_theme(
                            &backend.paths,
                            &name,
                        ))),
                        Work::Appearance(choices) => {
                            Reply::Appearance(backend.save_appearance(&choices))
                        }
                        Work::Storage(action) => {
                            let result = backend.run_storage(&action);
                            if let Err(error) = &result {
                                log::warn!("Storage {action:?} failed: {error}");
                            }
                            Reply::Storage((result, Box::new(backend.storage())))
                        }
                        Work::RaScan => Reply::RaScan(backend.ra_scan()),
                        Work::Roms => Reply::Roms(Box::new(backend.roms_scan())),
                        Work::Reports => {
                            let mut facts = crate::reports::facts(&backend.paths);
                            facts.console_id = console_id
                                .get_or_insert_with(|| backend.console_id())
                                .clone();
                            Reply::Reports(Box::new(facts))
                        }
                        Work::Adapters(forget) => Reply::Adapters(backend.adapters(forget)),
                        Work::RaImport(backup, seen) => {
                            Reply::RaImport(backend.ra_import(backup, &seen))
                        }
                    };
                    if results.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx,
            wifi,
            scans,
            bt_scans,
            profiles,
            store,
            slow,
            rx,
            _level_notices: level_notices,
            level_sequences: BTreeMap::new(),
            pending: 0,
            scanning: false,
            rescan: false,
            bt_scanning: false,
            bt_rescan: false,
            generation: 0,
            rejoin: None,
            levels: 0,
        })
    }
    pub fn busy(&self) -> bool {
        self.pending > 0
    }
    pub fn scanning(&self) -> bool {
        self.scanning
    }
    pub fn bt_scanning(&self) -> bool {
        self.bt_scanning
    }
    fn bt_scan(&mut self) -> Result<(), String> {
        if self.bt_scanning {
            self.bt_rescan = true;
            return Ok(());
        }
        self.bt_scans.send(()).map_err(|e| e.to_string())?;
        self.bt_scanning = true;
        Ok(())
    }
    fn scan(&mut self) -> Result<(), String> {
        if self.scanning {
            self.rescan = true;
            return Ok(());
        }
        self.scans
            .send(self.generation)
            .map_err(|e| e.to_string())?;
        self.scanning = true;
        Ok(())
    }
    pub(super) fn send(&mut self, work: Work) -> Result<(), String> {
        if matches!(work, Work::Scan) {
            return self.scan();
        }
        if matches!(work, Work::BtScan) {
            return self.bt_scan();
        }
        if matches!(work, Work::CheevosProfile) {
            return self.profiles.send(()).map_err(|e| e.to_string());
        }
        if let Work::Store(action) = work {
            return self.store.send(action).map_err(|e| e.to_string());
        }
        let slow = match work {
            Work::Diag => Some(diag::Slow::Diag),
            Work::Report => Some(diag::Slow::Report),
            Work::SendReport(ref name) => Some(diag::Slow::Send(name.clone())),
            Work::Bios => Some(diag::Slow::Bios),
            Work::BiosInstall(ref free) => Some(diag::Slow::BiosInstall(free.clone())),
            _ => None,
        };
        if let Some(slow) = slow {
            return self.slow.send(slow).map_err(|e| e.to_string());
        }
        if let Work::Rejoin(_, cancel) = &work {
            self.rejoin = Some(cancel.clone());
            return self.wifi.send(work).map_err(|e| e.to_string());
        }
        if matches!(work, Work::Connect(..) | Work::Forget(_)) {
            self.wifi.send(work).map_err(|e| e.to_string())?;
            self.pending += 1;
            return Ok(());
        }
        if matches!(work, Work::Set(Id::WifiSwitch, _)) {
            self.switched();
        }
        if matches!(work, Work::Level(..)) {
            self.levels += 1;
        }
        self.tx.send(work).map_err(|e| e.to_string())?;
        self.pending += 1;
        Ok(())
    }
    /// The Wi-Fi switch changed: older scans and the rejoin are stale.
    fn switched(&mut self) {
        self.generation += 1;
        if let Some(cancel) = self.rejoin.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
    /// A join's outcome: a brief confirmation and a fresh list, or the failure.
    fn joined(&mut self, state: &mut Settings, ssid: &str, result: Result<(), String>) {
        match result {
            Ok(()) => {
                state.note(archr_ui::settings::network::about(
                    "settings.wifi_joined",
                    ssid,
                ));
                let _ = self.send(Work::Scan);
            }
            Err(error) => state.fail(None, error),
        }
    }
    /// Arrival order is not state order: the daemon owns the version.
    fn apply_level(&mut self, state: &mut Settings, id: Id, reading: &levels::Reading) -> bool {
        if self
            .level_sequences
            .get(&id)
            .is_some_and(|seq| reading.seq < *seq)
        {
            return false;
        }
        self.level_sequences.insert(id, reading.seq);
        let changed = state.value(id) != reading.value;
        if changed {
            state.values.insert(id, reading.value.clone());
        }
        changed
    }
    /// Language is returned only after durable success, never speculatively.
    pub(super) fn poll(
        &mut self,
        state: &mut Settings,
        job: &mut Option<(archr_scraper::Job, Vec<String>)>,
    ) -> (bool, Option<String>) {
        let mut changed = false;
        let mut language = None;
        while let Ok(reply) = self.rx.try_recv() {
            // A notice is not a completed request or a setting effect: no write-back.
            if let Reply::LiveLevel(id, ref reading) = reply {
                changed |= self.apply_level(state, id, reading);
                continue;
            }
            changed = true;
            if matches!(reply, Reply::BtScan(_)) {
                self.bt_scanning = false;
                if std::mem::take(&mut self.bt_rescan) {
                    let _ = self.bt_scan();
                }
            } else if let Reply::Scan(generation, _) = &reply {
                self.scanning = false;
                let stale = *generation != self.generation;
                if std::mem::take(&mut self.rescan) {
                    let _ = self.scan();
                }
                if stale {
                    continue;
                }
            } else if !matches!(
                reply,
                Reply::CheevosProfile(_)
                    | Reply::Store(_)
                    | Reply::Facts(_)
                    | Reply::Rejoining(..)
                    | Reply::Rejoined(..)
                    | Reply::Diag(_)
                    | Reply::Report(_)
                    | Reply::ReportSent(_)
                    | Reply::Bios(_)
                    | Reply::BiosInstall(_)
            ) {
                self.pending = self.pending.saturating_sub(1);
            }
            match reply {
                Reply::LiveLevel(..) => unreachable!("notices were handled above"),
                Reply::Loaded(Ok(loaded)) => {
                    let (mut loaded, readings) = *loaded;
                    for id in [Id::Volume, Id::Brightness] {
                        if let Some(reading) = readings.get(&id) {
                            self.apply_level(state, id, reading);
                        } else if !self.level_sequences.contains_key(&id) {
                            continue;
                        }
                        loaded.values.insert(id, state.value(id).to_owned());
                    }
                    std::mem::swap(&mut state.values, &mut loaded.values);
                    // Module switches and rows live in the values too.
                    let modules = state.modules.view.clone();
                    state.set_modules(modules);
                    state.options = loaded.options;
                    state.labels = loaded.labels;
                    state.theme_names = loaded.theme_names;
                    state.access_ip = loaded.access_ip;
                    state.usb_busy = loaded.usb_busy;
                    state.usb = loaded.usb;
                    state.info = loaded.info;
                    state.error = loaded.error;
                    state.scrape.available = loaded.scrape.available;
                    state.mangohud_supported = loaded.mangohud_supported;
                    state.games.replace(loaded.games.systems);
                    state.sync_games();
                    state.values_loaded();
                    refresh_storage(state, loaded.storage);
                    if !state.scrape.verifying {
                        state.scrape.user = loaded.scrape.user;
                    }
                    if !state.cheevos.verifying {
                        state.cheevos.user = loaded.cheevos.user;
                        state.cheevos.web_key = loaded.cheevos.web_key;
                    }
                    state.loading = false;
                }
                Reply::Loaded(Err(error)) => {
                    log::warn!("Loading settings failed: {error}");
                    state.fail(None, error);
                    state.loading = false;
                }
                Reply::Set(id, result) => {
                    if id == Id::WifiSwitch {
                        self.switched();
                    }
                    if let Err(error) = &result {
                        log::warn!("Setting {id:?} failed: {error}");
                    }
                    if id == Id::UsbMode {
                        state.usb_busy = matches!(&result, Err(e) if e == "settings.usb_busy");
                    }
                    if matches!(&result, Err(e) if e == "settings.usb_rebuild_failed") {
                        state.values.insert(Id::UsbMode, "disabled".into());
                    }
                    if id == Id::Language {
                        language = result.as_ref().ok().cloned();
                    }
                    let wifi = (id == Id::WifiSwitch)
                        .then(|| result.as_deref().ok() == Some("1"))
                        .filter(|_| result.is_ok());
                    let bluetooth = (id == Id::BtSwitch)
                        .then(|| result.as_deref().ok().map(|v| v == "1"))
                        .flatten();
                    let new_layout = (id == Id::KeyboardLayout)
                        .then(|| result.as_ref().ok().cloned())
                        .flatten();
                    if let Some(next) = state.settle(id, result) {
                        let _ = self.send(Work::Set(id, next));
                    }
                    if let Some(layout) = new_layout {
                        state.values.insert(Id::KeyboardVariant, String::new());
                        let _ = self.send(Work::Variants(layout));
                    }
                    match bluetooth {
                        Some(true) => {
                            let _ = self.send(Work::BtScan);
                        }
                        Some(false) => {
                            state.bt.clear();
                            state.bt_scanned = None;
                            state.cursor = state.cursor.min(state.bt_rows() - 1);
                        }
                        None => {}
                    }
                    match wifi {
                        Some(true) => {
                            let _ = self.send(Work::Scan);
                            // The Wi-Fi lane rejoins the saved network, if any.
                            let cancel = std::sync::Arc::new(AtomicBool::new(false));
                            let _ = self.send(Work::Rejoin(self.generation, cancel));
                        }
                        Some(false) => {
                            state.networks.clear();
                            state.scanned = None;
                            state.cursor = state.cursor.min(state.wifi_rows() - 1);
                        }
                        None => {}
                    }
                }
                Reply::SetLevel(id, result) => {
                    if let Err(error) = &result {
                        log::warn!("Setting {id:?} failed: {error}");
                    }
                    let result = result.map(|reading| {
                        self.apply_level(state, id, &reading);
                        state.value(id).to_owned()
                    });
                    if let Some(next) = state.settle(id, result) {
                        let _ = self.send(Work::Set(id, next));
                    }
                }
                Reply::Level(id, result) => {
                    self.levels = self.levels.saturating_sub(1);
                    match result {
                        Ok(reading) => {
                            self.apply_level(state, id, &reading);
                        }
                        Err(error) => {
                            log::warn!("Level {id:?} failed: {error}");
                            state.fail(Some(id), error);
                        }
                    }
                }
                Reply::Info(info) => state.info = info,
                Reply::Usb(usb) => state.usb = usb,
                Reply::Store(reply) => state.store_reply(reply),
                Reply::Theme(loaded) => (state.base_theme, state.base_dir) = *loaded,
                Reply::RaScan(found) => {
                    state.raimport.scanning = false;
                    state.raimport.found = Some(found);
                }
                Reply::RaImport(result) => state.raimport_done(result),
                Reply::Adapters(Ok(adapters)) => {
                    let len = adapters.len();
                    state.adapters = Some(*adapters);
                    state.cursor = state.cursor.min(len.saturating_sub(1));
                }
                Reply::Adapters(Err(error)) => {
                    log::warn!("USB adapters: {error}");
                    state.adapters = Some(Default::default());
                    state.fail(None, error);
                }
                Reply::Diag(facts) => {
                    state.diag.scanning = false;
                    state.diag.facts = Some(*facts);
                }
                Reply::Reports(facts) => {
                    state.reports.scanning = false;
                    state.reports.facts = Some(*facts);
                }
                Reply::Report(result) => state.report_done(result),
                Reply::ReportSent(result) => state.report_sent(result),
                Reply::Bios(Ok(systems)) => state.bios_scanned(systems),
                Reply::Bios(Err(error)) => {
                    log::warn!("archr-systems --json: {error}");
                    state.bios_scanned(Vec::new());
                    state.fail(None, "bios.check_failed".into());
                }
                Reply::BiosInstall(result) => state.bios_installed(result),
                Reply::Roms(facts) => {
                    let (status, scan) = *facts;
                    state.storage.roms = status;
                    state.roms.scanning = false;
                    state.roms.scan = Some(scan);
                }
                Reply::Appearance(result) => {
                    state.pending = None;
                    match result {
                        Ok(()) => state.applied = Some(state.appearance()),
                        Err(error) => state.fail(None, error),
                    }
                }
                Reply::Variants(names) => {
                    pages::keyboard_choices(state, Id::KeyboardVariant, names);
                }
                Reply::Scan(_, result) => match result {
                    Ok((networks, address)) => {
                        // A late scan never repopulates a list the user switched off.
                        if state.value(Id::WifiSwitch) == "1" {
                            state.networks = networks;
                            state.scanned = Some(std::time::Instant::now());
                        }
                        state.access_ip = address;
                        if state.page() == Page::Wifi {
                            state.cursor = state.cursor.min(state.wifi_rows() - 1);
                        }
                    }
                    // A scan cut short by switching Wi-Fi off is not an error.
                    Err(error) if state.value(Id::WifiSwitch) == "1" => {
                        state.fail(None, error);
                    }
                    Err(_) => {}
                },
                Reply::BtScan(result) => match result {
                    // A late scan never repopulates a list the user switched off.
                    Ok(devices) if state.value(Id::BtSwitch) == "1" => {
                        state.bt = devices;
                        state.bt_scanned = Some(std::time::Instant::now());
                        if state.page() == Page::Bluetooth {
                            state.cursor = state.cursor.min(state.bt_rows() - 1);
                        }
                    }
                    Err(error) if state.value(Id::BtSwitch) == "1" => state.fail(None, error),
                    _ => {}
                },
                Reply::Bt(result) => {
                    state.connecting = None;
                    match result {
                        Ok(()) => {
                            state.error = None;
                            let _ = self.send(Work::BtScan);
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
                Reply::Connect(result) => {
                    let ssid = state.connecting.take().unwrap_or_default();
                    self.joined(state, &ssid, result);
                }
                // Switched since (off, or off and on again): not this join's page.
                Reply::Rejoining(generation, _) | Reply::Rejoined(generation, ..)
                    if generation != self.generation => {}
                Reply::Rejoining(_, ssid) => {
                    if state.connecting.is_none() {
                        state.connecting = Some(ssid);
                        state.rejoining = true;
                    }
                }
                Reply::Rejoined(_, ssid, result) => {
                    self.rejoin = None;
                    if state.rejoining {
                        state.connecting = None;
                        state.rejoining = false;
                    }
                    self.joined(state, &ssid, result);
                }
                Reply::Forget(result) => {
                    state.connecting = None;
                    match result {
                        Ok(()) => {
                            state.error = None;
                            let _ = self.send(Work::Scan);
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
                Reply::Login(result) => {
                    state.scrape.verifying = false;
                    match result {
                        // Only a verified, saved login becomes the shown account.
                        Ok((user, threads)) => {
                            state.scrape.user = user;
                            state.scrape.threads = threads;
                            state.error = None;
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
                Reply::CheevosLogin(result) => {
                    state.cheevos.verifying = false;
                    match result {
                        Ok((user, profile)) => {
                            state.cheevos.user = user;
                            state.cheevos.draft.clear();
                            state.cheevos.profile = Some(profile);
                            state.cheevos.expired = false;
                            state.values.insert(Id::Cheevos, "1".into());
                            state.error = None;
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
                Reply::CheevosLogout(result) => {
                    state.cheevos.verifying = false;
                    match result {
                        Ok(()) => {
                            state.cheevos = archr_ui::settings::cheevos::Cheevos::default();
                            state.cursor = state.cursor.min(state.visible_rows().len() - 1);
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
                Reply::CheevosKey(result) => {
                    state.cheevos.verifying = false;
                    match result {
                        Ok(summary) => {
                            state.cheevos.web_key = summary.is_some();
                            state.cheevos.key_refused = false;
                            let summary = summary.unwrap_or_default();
                            if let Some(profile) = &mut state.cheevos.profile {
                                (profile.since, profile.last_game) =
                                    (summary.since, summary.last_game);
                            }
                            state.cursor = state.cursor.min(state.visible_rows().len() - 1);
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
                // Offline or a service error changes nothing; a refused token
                // keeps the account and asks for the password again.
                Reply::CheevosProfile(result) if !state.cheevos.user.is_empty() => match result {
                    Ok((profile, refused)) => {
                        state.cheevos.profile = Some(profile);
                        state.cheevos.key_refused = refused;
                        state.cheevos.expired = false;
                    }
                    Err(archr_cheevos::Error::Auth) => state.cheevos.expired = true,
                    Err(_) => {}
                },
                Reply::CheevosProfile(_) => {}
                Reply::Facts(facts) => refresh_storage(state, *facts),
                Reply::Storage((result, facts)) => {
                    refresh_storage(state, *facts);
                    match result {
                        // factoryreset reboots: its notice stays until then.
                        Ok("settings.resetting") => {}
                        result => state.storage_done(result),
                    }
                }
                Reply::Scrape(result) => {
                    state.scrape.starting = false;
                    match result {
                        // Never replace (and so cancel) a job already running.
                        Ok(started) if job.is_some() => drop(started),
                        Ok(started) => {
                            state.scrape.running = true;
                            *job = Some(started);
                        }
                        Err(error) => state.fail(None, error),
                    }
                }
            }
        }
        (changed, language)
    }
}
/// Host facts replace the old ones; the open question and running action stay.
/// The theme store's own lane: GitHub can be slow, settings never wait on it.
/// Covers are fetched between requests, a new request always first.
fn store_lane(
    paths: &Paths,
    requests: &mpsc::Receiver<archr_ui::settings::store::Action>,
    results: &mpsc::Sender<Reply>,
) {
    let shop = crate::themestore::Shop::production(
        paths.cache.join("store"),
        paths.at(crate::appearance::USER_THEMES),
    );
    let mut queue = std::collections::VecDeque::new();
    let mut next = None;
    loop {
        let action = match next.take().map_or_else(|| requests.try_recv(), Ok) {
            Ok(action) => Some(action),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => return,
        };
        let replies = match (action, queue.pop_front()) {
            (Some(action), card) => {
                queue.extend(card);
                let active = SystemCfg::load(&paths.config)
                    .ok()
                    .and_then(|cfg| cfg.get_setting(Id::Theme.key(), None, None))
                    .unwrap_or_default();
                shop.handle(action, &active, &mut queue)
            }
            (None, Some(card)) => shop
                .decorate(&card)
                .map(archr_ui::settings::store::Reply::Card)
                .into_iter()
                .collect(),
            (None, None) => match requests.recv() {
                Ok(action) => {
                    next = Some(action);
                    continue;
                }
                Err(_) => return,
            },
        };
        for reply in replies {
            if results.send(Reply::Store(reply)).is_err() {
                return;
            }
        }
    }
}
fn refresh_storage(state: &mut Settings, facts: archr_ui::settings::storage::Storage) {
    let ui = std::mem::take(&mut state.storage);
    state.storage = archr_ui::settings::storage::Storage {
        confirm: ui.confirm,
        working: ui.working,
        ..facts
    };
}

#[cfg(test)]
mod level_order_tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Worker, mpsc::Sender<Reply>) {
        let dir = tempfile::tempdir().unwrap();
        let mut worker = Worker::start(Paths::new(Some(dir.path().into()))).unwrap();
        let (tx, rx) = mpsc::channel();
        worker.rx = rx;
        (dir, worker, tx)
    }
    fn reading(value: &str, seq: u64) -> levels::Reading {
        levels::Reading {
            value: value.into(),
            seq,
        }
    }
    fn snapshot(values: &[(Id, &str, u64)]) -> Reply {
        let mut state = Settings::default();
        let mut readings = BTreeMap::new();
        for &(id, value, seq) in values {
            state.values.insert(id, value.into());
            readings.insert(id, reading(value, seq));
        }
        Reply::Loaded(Ok(Box::new((state, readings))))
    }

    #[test]
    fn a_late_loaded_value_cannot_replace_a_newer_notice_of_the_same_level() {
        let (_dir, mut worker, tx) = fixture();
        let mut state = Settings::default();
        tx.send(Reply::LiveLevel(Id::Volume, reading("70", 2)))
            .unwrap();
        worker.poll(&mut state, &mut None);
        tx.send(snapshot(&[
            (Id::Volume, "50", 1),
            (Id::Brightness, "35", 1),
        ]))
        .unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "70");
        assert_eq!(state.value(Id::Brightness), "35", "versions are per level");
        // A failed live read on reopening must not replace known state with config.
        tx.send(snapshot(&[])).unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "70");
        assert_eq!(state.value(Id::Brightness), "35");
    }

    #[test]
    fn late_hold_and_release_values_cannot_replace_newer_notices_of_the_same_level() {
        let (_dir, mut worker, tx) = fixture();
        let mut state = Settings::default();
        worker.pending = 2;
        worker.levels = 2;
        tx.send(Reply::LiveLevel(Id::Volume, reading("70", 3)))
            .unwrap();
        tx.send(Reply::Level(Id::Volume, Ok(reading("50", 2))))
            .unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "70", "late hold");
        assert_eq!((worker.pending, worker.levels), (1, 1));
        tx.send(Reply::LiveLevel(Id::Volume, reading("80", 5)))
            .unwrap();
        tx.send(Reply::Level(Id::Volume, Ok(reading("60", 4))))
            .unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "80", "late release");
        assert_eq!((worker.pending, worker.levels), (0, 0));
        // Arrival can invert the other way too: the notice may be late.
        tx.send(Reply::Level(Id::Volume, Ok(reading("90", 8))))
            .unwrap();
        tx.send(Reply::LiveLevel(Id::Volume, reading("85", 7)))
            .unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "90", "late notice");
    }

    #[test]
    fn a_notice_advances_the_version_even_when_the_value_does_not_change() {
        let (_dir, mut worker, tx) = fixture();
        let mut state = Settings::default();
        tx.send(Reply::LiveLevel(Id::Volume, reading("70", 1)))
            .unwrap();
        worker.poll(&mut state, &mut None);
        tx.send(Reply::LiveLevel(Id::Volume, reading("70", 10)))
            .unwrap();
        assert!(!worker.poll(&mut state, &mut None).0);
        tx.send(snapshot(&[(Id::Volume, "60", 9)])).unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "70");
    }

    #[test]
    fn a_late_set_reply_completes_without_overwriting_a_newer_notice() {
        let (_dir, mut worker, tx) = fixture();
        let mut state = Settings::default();
        state.pending = Some(Id::Volume);
        worker.pending = 1;
        tx.send(Reply::LiveLevel(Id::Volume, reading("70", 5)))
            .unwrap();
        tx.send(Reply::SetLevel(Id::Volume, Ok(reading("50", 4))))
            .unwrap();
        worker.poll(&mut state, &mut None);
        assert_eq!(state.value(Id::Volume), "70");
        assert_eq!(state.pending, None);
        assert!(!worker.busy());
    }
}
