//! Serial settings worker. No process, config lock or sysfs access on SDL's thread.
mod info;
mod pages;
mod runtime;
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_ui::settings::{Id, Information, Page, Settings};
pub use runtime::Runtime;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub struct Backend {
    paths: Paths,
    identity: std::sync::OnceLock<Information>,
}
impl Backend {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            identity: std::sync::OnceLock::new(),
        }
    }
    fn read(&self, path: &str) -> String {
        archr_io::read_utf8(&self.paths.at(path), 64 * 1024, archr_io::Symlinks::Follow)
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
    fn backlight(&self) -> Result<std::path::PathBuf, String> {
        fs::read_dir(self.paths.at("/sys/class/backlight"))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .min()
            .ok_or_else(|| "No backlight available".into())
    }
    fn number(path: &std::path::Path) -> Result<u64, String> {
        archr_io::read_utf8(path, 4096, archr_io::Symlinks::Follow)
            .map_err(|e| e.to_string())?
            .trim()
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())
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
    fn command(&self, name: &str, args: &[&str]) -> Result<String, String> {
        self.command_with_timeout(name, args, Duration::from_secs(20))
    }
    fn command_with_timeout(
        &self,
        name: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<String, String> {
        let mut stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
        let mut stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
        let mut command = Command::new(self.paths.at(&format!("/usr/bin/{name}")));
        command
            .args(args)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
            .stderr(stderr.try_clone().map_err(|e| e.to_string())?);
        if let Some(root) = &self.paths.root {
            command
                .env("ARCHR_LAUNCHER_ROOT", root)
                .env("PATH", self.paths.at("/usr/bin"));
        }
        let mut child = command.spawn().map_err(|e| format!("{name}: {e}"))?;
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                None => {
                    // SAFETY: the child owns its new process group; never signal
                    // the launcher's group or an arbitrary configured PID.
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                    let _ = child.wait();
                    return Err(format!("{name}: timeout"));
                }
            }
        };
        let capture = |file: &mut fs::File| -> Result<String, String> {
            file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            let mut text = String::new();
            file.take(64 * 1024)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            Ok(text.trim().to_owned())
        };
        if status.success() {
            capture(&mut stdout)
        } else {
            Err(format!("{name}: {status}; {}", capture(&mut stderr)?))
        }
    }
    pub fn load(&self) -> Result<Settings, String> {
        let cfg = self.cfg()?;
        let mut state = Settings::default();
        for page in [Page::Sound, Page::System, Page::Region] {
            for row in page.rows() {
                if let Some(value) = cfg.get_setting(row.id.key(), None, None) {
                    state.values.insert(row.id, row.id.normalize(&value));
                }
            }
        }
        self.load_pages(&cfg, &mut state);
        if let Ok(output) = self.command("pactl", &["get-sink-volume", "@DEFAULT_SINK@"]) {
            if let Some(volume) = output
                .split_whitespace()
                .find_map(|token| token.strip_suffix('%')?.parse::<u8>().ok())
                .filter(|v| *v <= 100)
            {
                state.values.insert(Id::Volume, volume.to_string());
            }
        }
        if let Ok(panel) = self.backlight() {
            let number = |name| Self::number(&panel.join(name)).ok();
            if let (Some(raw), Some(max)) = (number("brightness"), number("max_brightness")) {
                if let Some(percent) = raw
                    .saturating_mul(100)
                    .saturating_add(max / 2)
                    .checked_div(max)
                {
                    state
                        .values
                        .insert(Id::Brightness, percent.clamp(5, 100).to_string());
                }
            }
        }
        let mut errors = Vec::new();
        for (id, program, args) in [
            (Id::Audio, "set-audio", &["list"][..]),
            (Id::Timezone, "timeinfo", &["timezones"][..]),
        ] {
            match self.command(program, args) {
                Ok(output) => {
                    let mut options: Vec<String> = output
                        .split(['\n', ','])
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    if id == Id::Timezone {
                        options.push("UTC".into());
                    }
                    let current = state.value(id).to_owned();
                    if !current.is_empty() {
                        options.push(current);
                    }
                    options.sort();
                    options.dedup();
                    state.options.insert(id, options);
                }
                Err(error) => errors.push(error),
            }
        }
        if let Ok(current) = self.command("set-audio", &["get"]) {
            if !current.is_empty() {
                state.values.insert(Id::Audio, current);
            }
        }
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
            state.error = Some((None, error));
        }
        Ok(state)
    }
    pub fn apply(&self, id: Id, value: &str) -> Result<String, String> {
        if let Some(result) = self.apply_page(id, value) {
            return result;
        }
        let valid = match id {
            Id::Volume | Id::Brightness => value
                .parse::<u8>()
                .is_ok_and(|n| n <= 100 && (id != Id::Brightness || n >= 5)),
            Id::Turbo | Id::Gpu | Id::Governor | Id::Language | Id::Idle | Id::IdleGame => {
                id.choices().contains(&value)
            }
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
            Id::Volume => {
                self.command(
                    "pactl",
                    &[
                        "--",
                        "set-sink-volume",
                        "@DEFAULT_SINK@",
                        &format!("{value}%"),
                    ],
                )?;
            }
            Id::Brightness => {
                let panel = self.backlight()?;
                let max = Self::number(&panel.join("max_brightness"))?;
                let raw = max.saturating_mul(value.parse().expect("validated percent")) / 100;
                fs::write(panel.join("brightness"), raw.max(1).to_string())
                    .map_err(|e| format!("brightness: {e}"))?;
            }
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
    Info,
    Scan,
    Connect(String, String),
    Forget(String),
    Variants(String),
}
enum Reply {
    Loaded(Result<Box<Settings>, String>),
    Set(Id, Result<String, String>),
    Info(Information),
    Scan(Result<Vec<archr_ui::settings::network::Network>, String>),
    Connect(Result<(), String>),
    Forget(Result<(), String>),
    Variants(Vec<String>),
}
pub struct Worker {
    tx: mpsc::Sender<Work>,
    rx: mpsc::Receiver<Reply>,
    pending: usize,
}
impl Worker {
    pub fn start(paths: Paths) -> Result<Self, String> {
        let (tx, requests) = mpsc::channel();
        let (results, rx) = mpsc::channel();
        thread::Builder::new()
            .name("launcher-settings".into())
            .spawn(move || {
                let backend = Backend::new(paths);
                while let Ok(work) = requests.recv() {
                    let reply = match work {
                        Work::Load => Reply::Loaded(backend.load().map(Box::new)),
                        Work::Set(id, value) => Reply::Set(id, backend.apply(id, &value)),
                        Work::Info => Reply::Info(backend.refresh_info()),
                        Work::Scan => Reply::Scan(backend.scan()),
                        Work::Connect(ssid, password) => {
                            Reply::Connect(backend.connect(&ssid, &password))
                        }
                        Work::Forget(ssid) => Reply::Forget(backend.forget(&ssid)),
                        Work::Variants(layout) => {
                            Reply::Variants(backend.keyboard_variants(&layout))
                        }
                    };
                    if results.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, rx, pending: 0 })
    }
    pub fn busy(&self) -> bool {
        self.pending > 0
    }
    pub(super) fn send(&mut self, work: Work) -> Result<(), String> {
        self.tx.send(work).map_err(|e| e.to_string())?;
        self.pending += 1;
        Ok(())
    }
    /// Language is returned only after durable success, never speculatively.
    pub fn poll(&mut self, state: &mut Settings) -> (bool, Option<String>) {
        let mut changed = false;
        let mut language = None;
        while let Ok(reply) = self.rx.try_recv() {
            changed = true;
            self.pending = self.pending.saturating_sub(1);
            match reply {
                Reply::Loaded(Ok(mut loaded)) => {
                    std::mem::swap(&mut state.values, &mut loaded.values);
                    state.options = loaded.options;
                    state.access_ip = loaded.access_ip;
                    state.info = loaded.info;
                    state.error = loaded.error;
                    state.loading = false;
                }
                Reply::Loaded(Err(error)) => {
                    log::warn!("Loading settings failed: {error}");
                    state.error = Some((None, error));
                    state.loading = false;
                }
                Reply::Set(id, result) => {
                    if let Err(error) = &result {
                        log::warn!("Setting {} failed: {error}", id.key());
                    }
                    if id == Id::Language {
                        language = result.as_ref().ok().cloned();
                    }
                    let wifi_enabled = id == Id::WifiSwitch && result.as_deref() == Ok("1");
                    let wifi_disabled = id == Id::WifiSwitch && result.as_deref() == Ok("0");
                    let new_layout = (id == Id::KeyboardLayout)
                        .then(|| result.as_ref().ok().cloned())
                        .flatten();
                    state.complete(id, result);
                    if let Some(layout) = new_layout {
                        state.values.insert(Id::KeyboardVariant, String::new());
                        let _ = self.tx.send(Work::Variants(layout));
                        self.pending += 1;
                    }
                    if wifi_disabled {
                        state.networks.clear();
                    }
                    if wifi_enabled {
                        let _ = self.tx.send(Work::Scan);
                        self.pending += 1;
                    }
                }
                Reply::Info(info) => state.info = info,
                Reply::Variants(mut options) => {
                    options.push(String::new());
                    options.sort();
                    options.dedup();
                    state.options.insert(Id::KeyboardVariant, options);
                }
                Reply::Scan(result) => match result {
                    Ok(networks) => {
                        state.networks = networks;
                        if state.page() == Page::Wifi {
                            state.cursor = state.cursor.min(state.networks.len() + 3);
                        }
                        state.error = None;
                    }
                    Err(error) => state.error = Some((None, error)),
                },
                Reply::Connect(result) | Reply::Forget(result) => match result {
                    Ok(()) => {
                        state.error = None;
                        let _ = self.tx.send(Work::Scan);
                        self.pending += 1;
                    }
                    Err(error) => state.error = Some((None, error)),
                },
            }
        }
        (changed, language)
    }
}
