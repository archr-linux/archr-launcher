//! Serial settings worker. No process, config lock or sysfs access on SDL's thread.
mod info;
mod runtime;
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_ui::settings::{Id, Page, Settings};
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
}
impl Backend {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }
    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.paths.at(path))
            .unwrap_or_default()
            .trim()
            .to_owned()
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
        let deadline = Instant::now() + Duration::from_secs(20);
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
                    state.values.insert(row.id, value);
                }
            }
        }
        if let Ok(output) = self.command("pactl", &["get-sink-volume", "@DEFAULT_SINK@"]) {
            if let Some(volume) = output
                .split_whitespace()
                .find_map(|token| token.strip_suffix('%')?.parse::<u8>().ok())
                .filter(|v| *v <= 100)
            {
                state.values.insert(Id::Volume, volume.to_string());
            }
        }
        let mut backlights: Vec<_> = fs::read_dir(self.paths.at("/sys/class/backlight"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .collect();
        backlights.sort_by_key(|d| d.file_name());
        if let Some(panel) = backlights.first() {
            let number = |name| {
                fs::read_to_string(panel.path().join(name))
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()
            };
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
            (Id::Audio, "set-audio", vec!["list"]),
            (Id::Timezone, "timeinfo", vec!["timezones"]),
        ] {
            match self.command(program, &args) {
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
        let governors: Vec<String> = ["ondemand", "performance", "schedutil", "powersave"]
            .into_iter()
            .filter(|v| available.split_whitespace().any(|g| g == *v))
            .map(str::to_owned)
            .collect();
        state.options.insert(Id::Governor, governors);
        self.refresh_info(&mut state);
        state.values.insert(Id::Update, state.version.clone());
        state.loading = false;
        if !errors.is_empty() {
            state.error = Some((None, errors.join("; ")));
        }
        Ok(state)
    }
    pub fn apply(&self, id: Id, value: &str) -> Result<String, String> {
        let valid = match id {
            Id::Volume | Id::Brightness => value
                .parse::<u8>()
                .is_ok_and(|n| n <= 100 && (id != Id::Brightness || n >= 5)),
            Id::UiSound | Id::Turbo => matches!(value, "0" | "1"),
            Id::Gpu => matches!(value, "panfrost" | "libmali"),
            Id::Governor => matches!(
                value,
                "ondemand" | "performance" | "schedutil" | "powersave"
            ),
            Id::Language => matches!(value, "en_US" | "pt_BR" | "es_ES" | "ru_RU"),
            Id::Idle | Id::IdleGame => matches!(value, "0" | "1" | "3" | "5" | "10" | "15" | "30"),
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
        };
        if !valid {
            return Err("Invalid setting value".into());
        }
        // suspendmode compares the stored value itself; pre-saving makes it a no-op.
        if id == Id::Suspend {
            self.command("suspendmode", &[value])?;
            if value != "default" {
                self.save(id, value)?;
            }
            return Ok(if value == "default" {
                self.cfg()?
                    .get_setting(id.key(), None, None)
                    .unwrap_or_else(|| "off".into())
            } else {
                value.into()
            });
        }
        let old = self
            .cfg()?
            .get_setting(id.key(), None, None)
            .unwrap_or_else(|| id.default_value().into());
        self.save(id, value)?;
        let result = match id {
            Id::Volume => self.command("volume", &[value]),
            Id::Brightness => self.command("brightness", &["set", value]),
            Id::Audio => self.command("set-audio", &["set", value]),
            Id::Gpu => self.command("gpudriver", &[value]),
            Id::Turbo => self.command(
                "turbomode",
                &[if value == "1" { "enable" } else { "disable" }],
            ),
            Id::Timezone => self.command("archr-config", &["tz", value]),
            Id::Governor => self.command(
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
            ),
            _ => Ok(String::new()),
        };
        if let Err(error) = result {
            return Err(match self.save(id, &old) {
                Ok(()) => error,
                Err(restore) => format!("{error}; config restore: {restore}"),
            });
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
    Info,
}
enum Reply {
    Loaded(Result<Box<Settings>, String>),
    Set(Id, Result<String, String>),
    Info(Settings),
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
                        Work::Info => {
                            let mut state = Settings::default();
                            backend.refresh_info(&mut state);
                            Reply::Info(state)
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
                    state.cards = loaded.cards;
                    state.version = loaded.version;
                    state.device = loaded.device;
                    state.kernel = loaded.kernel;
                    state.error = loaded.error;
                    state.loading = false;
                }
                Reply::Loaded(Err(error)) => {
                    state.error = Some((None, error));
                    state.loading = false;
                }
                Reply::Set(id, result) => {
                    if id == Id::Language {
                        language = result.as_ref().ok().cloned();
                    }
                    state.complete(id, result);
                }
                Reply::Info(info) => {
                    state.cards = info.cards;
                    state.device = info.device;
                    state.kernel = info.kernel;
                    state.version = info.version;
                }
            }
        }
        (changed, language)
    }
}
