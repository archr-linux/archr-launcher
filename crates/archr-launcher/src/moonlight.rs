//! CLI contract verified against moonlight-embedded a6bf7154a743d4f74a1b377e730f188352a1b80c:
//! src/main.c (list, PIN, success markers) and src/config.c (getopt_long_only).
//! The CLI prints pairing failures but still exits zero: never trust status alone.
use crate::{handoff::assignment, paths::Paths};
use archr_config::SystemCfg;
use archr_ui::{
    moonlight::{Codec, Keyboard, Options, Row, State, Status, Stream},
    state::{Button, Outcome},
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read},
    net::Ipv4Addr,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

/// A virtual row, never a cached ROM system or an EmulationStation dependency.
pub(crate) fn collection(overview: &mut archr_index::cache::Overview, available: bool) {
    overview
        .systems
        .retain(|s| s.name != "moonlight" && s.name != archr_ui::moonlight::SYSTEM);
    if available {
        overview.systems.insert(
            0,
            archr_index::cache::SystemEntry {
                name: archr_ui::moonlight::SYSTEM.into(),
                fullname: "Moonlight".into(),
                game_count: 1,
                dir_mtime: 0,
                source_fingerprint: 0,
            },
        );
    }
}

const CONFIG: &str = "/storage/.config/moonlight/moonlight.conf";
const LIMIT: u64 = 256 * 1024;
const MAX_HOSTS: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    Apps(Vec<String>),
    NeedsPair,
    Paired,
    Unpaired,
}
pub fn parse_output(
    output: &str,
    error: &str,
    action: &str,
    success: bool,
) -> Result<Reply, String> {
    if action == "list"
        && error
            .lines()
            .any(|l| l.trim() == "You must pair with the PC first")
    {
        return Ok(Reply::NeedsPair);
    }
    if !success {
        return Err(error.to_owned());
    }
    match action {
        "pair" if output.lines().any(|l| l.trim() == "Succesfully paired") => Ok(Reply::Paired),
        "unpair" if output.lines().any(|l| l.trim() == "Succesfully unpaired") => {
            Ok(Reply::Unpaired)
        }
        "list" => {
            if error.contains("Can't get app list")
                || error.contains("Can't connect")
                || error.contains("Gamestream error:")
            {
                return Err(error.to_owned());
            }
            let apps = output
                .lines()
                .filter_map(|line| {
                    let (number, name) = line.trim_end_matches('\r').split_once(". ")?;
                    let number: u32 = number.parse().ok()?;
                    (number > 0 && !name.is_empty() && name.len() <= 4096 && !name.contains('\0'))
                        .then(|| name.to_owned())
                })
                .take(1024)
                .collect();
            Ok(Reply::Apps(apps))
        }
        _ => Err(error.to_owned()),
    }
}
fn address(host: &str) -> Result<String, String> {
    host.parse::<Ipv4Addr>()
        .map(|ip| ip.to_string())
        .map_err(|_| "moonlight.invalid_ip".into())
}
/// The supervisor interprets one fixed command. Host/app/path data are exported
/// and expanded exactly once, as individual arguments, like catalog commands.
pub fn stream_command(paths: &Paths, stream: &Stream) -> Result<String, String> {
    address(&stream.host)?;
    if !stream.options.valid()
        || stream.app.is_empty()
        || stream.app.contains('\0')
        || stream.app.len() > 4096
    {
        return Err("moonlight.failed".into());
    }
    let bin = paths.at("/usr/bin/moonlight");
    let config = paths.at(CONFIG);
    let keys = paths.at("/storage/.cache/moonlight");
    let mut line = String::new();
    for (name, value) in [
        (
            "ARCHR_MOONLIGHT_BIN",
            bin.to_str().ok_or("invalid executable path")?,
        ),
        (
            "ARCHR_MOONLIGHT_CONFIG",
            config.to_str().ok_or("invalid config path")?,
        ),
        (
            "ARCHR_MOONLIGHT_KEYS",
            keys.to_str().ok_or("invalid key path")?,
        ),
        ("ARCHR_MOONLIGHT_HOST", &stream.host),
        ("ARCHR_MOONLIGHT_APP", &stream.app),
    ] {
        line.push_str(&assignment(name, value));
    }
    line.push_str("export ARCHR_MOONLIGHT_BIN ARCHR_MOONLIGHT_CONFIG ARCHR_MOONLIGHT_KEYS ARCHR_MOONLIGHT_HOST ARCHR_MOONLIGHT_APP; ");
    let o = &stream.options;
    line.push_str(&format!("exec \"$ARCHR_MOONLIGHT_BIN\" stream -config \"$ARCHR_MOONLIGHT_CONFIG\" -keydir \"$ARCHR_MOONLIGHT_KEYS\" -app \"$ARCHR_MOONLIGHT_APP\" -width {} -height {} -fps {} -bitrate {} -codec {} -nosops \"$ARCHR_MOONLIGHT_HOST\"", o.width, o.height, o.fps, o.bitrate, o.codec.argument()));
    Ok(line)
}

// OS entropy and CLI work both stay on the worker, not on the render thread.
fn random_pin() -> io::Result<String> {
    let mut rng = fs::File::open("/dev/urandom")?;
    loop {
        let mut bytes = [0; 2];
        rng.read_exact(&mut bytes)?;
        let value = u16::from_ne_bytes(bytes);
        if value < 59994 {
            // rejection avoids modulo bias; upstream rejects zero
            return Ok(format!("{:04}", value % 9999 + 1));
        }
    }
}
fn execute(
    paths: &Paths,
    action: &'static str,
    host: &str,
    timeout: Duration,
    cancel: &AtomicBool,
    pin: Option<&str>,
) -> Result<Reply, String> {
    address(host)?;
    let mut command = Command::new(paths.at("/usr/bin/moonlight"));
    command
        .arg(action)
        .arg("-config")
        .arg(paths.at(CONFIG))
        .arg("-keydir")
        .arg(paths.at("/storage/.cache/moonlight"))
        .env("LC_ALL", "C");
    if let Some(pin) = pin {
        command.args(["-pin", pin]);
    }
    // Upstream does not consume the host operand after `--`.
    command.arg(host);
    let output =
        crate::process::capture(&mut command, timeout, cancel, LIMIT).map_err(|e| e.to_string())?;
    parse_output(
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
        action,
        output.status.success(),
    )
}

fn load(paths: &Paths, panel: (u32, u32)) -> Result<(Vec<String>, Options), String> {
    let path = paths.store.join("moonlight.cfg");
    let cfg = match SystemCfg::load(&path) {
        Ok(cfg) => cfg,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok((vec![], Options::new(panel.0, panel.1)))
        }
        Err(e) => return Err(e.to_string()),
    };
    let mut hosts = vec![];
    for host in cfg
        .get_local("moonlight", None, "hosts")
        .unwrap_or_default()
        .split(',')
    {
        if let Ok(host) = address(host) {
            if !hosts.contains(&host) && hosts.len() < MAX_HOSTS {
                hosts.push(host);
            }
        }
    }
    let mut options = Options::new(panel.0, panel.1);
    let get = |key| cfg.get_local("moonlight", None, key).unwrap_or_default();
    options.width = get("width").parse().unwrap_or(options.width);
    options.height = get("height").parse().unwrap_or(options.height);
    options.fps = get("fps").parse().unwrap_or(options.fps);
    options.bitrate = get("bitrate").parse().unwrap_or(options.bitrate);
    options.codec = match get("codec").as_str() {
        "h265" => Codec::H265,
        "auto" => Codec::Auto,
        _ => Codec::H264,
    };
    if !options.valid() {
        options = Options::new(panel.0, panel.1);
    }
    Ok((hosts, options))
}
fn save(paths: &Paths, hosts: &[String], options: &Options) -> Result<(), String> {
    fs::create_dir_all(&paths.store).map_err(|e| e.to_string())?;
    let path = paths.store.join("moonlight.cfg");
    // Only initialize an absent file; archr-config owns all subsequent updates.
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(_) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    let values = [
        hosts.join(","),
        options.width.to_string(),
        options.height.to_string(),
        options.fps.to_string(),
        options.bitrate.to_string(),
        options.codec.argument().to_owned(),
    ];
    let keys = [
        "moonlight.hosts",
        "moonlight.width",
        "moonlight.height",
        "moonlight.fps",
        "moonlight.bitrate",
        "moonlight.codec",
    ];
    let entries: Vec<_> = keys
        .into_iter()
        .zip(values.iter().map(String::as_str))
        .collect();
    SystemCfg::set_many_and_save(&path, &paths.store.join(".moonlight.lock"), &entries)
        .map_err(|e| e.to_string())
}

enum Event {
    Loaded(Result<(Vec<String>, Options), String>),
    Pairing(String),
    Forgotten(Result<(Vec<String>, bool), String>),
    Done(Result<Option<Reply>, String>),
}
pub struct Runtime {
    pub active: bool,
    pub state: State,
    paths: Paths,
    panel: (u32, u32),
    rx: Receiver<Event>,
    tx: Sender<Event>,
    cancel: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    loaded: bool,
}
impl Runtime {
    pub fn new(paths: Paths, panel: (u32, u32)) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut runtime = Self {
            active: false,
            state: State::new(panel),
            paths,
            panel,
            rx,
            tx,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: None,
            loaded: false,
        };
        let tx = runtime.tx.clone();
        let paths = runtime.paths.clone();
        match thread::Builder::new()
            .name("moonlight-store".into())
            .spawn(move || {
                let _ = tx.send(Event::Loaded(load(&paths, panel)));
            }) {
            Ok(worker) => runtime.worker = Some(worker),
            Err(e) => {
                runtime.fail(e.to_string());
                runtime.loaded = true;
            }
        }
        runtime
    }
    pub fn open(&mut self) {
        self.active = true;
        if self.loaded && !self.state.busy {
            self.refresh();
        }
    }
    pub fn close(&mut self) {
        self.active = false;
        self.cancel.store(true, Ordering::Relaxed);
        self.state.keyboard = None;
        self.state.option = None;
        self.state.confirm_unpair = false;
    }
    fn fail(&mut self, error: String) {
        log::warn!("Moonlight: {error}");
        self.state.error = Some("moonlight.failed");
        self.state.busy = false;
        self.state.pin = None;
    }
    pub fn poll(&mut self) -> bool {
        let mut dirty = false;
        while let Ok(event) = self.rx.try_recv() {
            dirty = true;
            match event {
                Event::Loaded(result) => {
                    self.loaded = true;
                    self.state.busy = false;
                    match result {
                        Ok((hosts, options)) => {
                            self.state.hosts = hosts;
                            self.state.options = options;
                            if self.active {
                                self.refresh();
                            }
                        }
                        Err(e) => self.fail(e),
                    }
                }
                Event::Pairing(pin) => self.state.pin = Some(pin),
                Event::Forgotten(result) => {
                    self.state.busy = false;
                    match result {
                        Ok((hosts, remote_ok)) => {
                            self.state.hosts = hosts;
                            self.state.host = self
                                .state
                                .host
                                .min(self.state.hosts.len().saturating_sub(1));
                            self.state.apps.clear();
                            self.state.status = Status::Unknown;
                            self.state.cursor = 0;
                            self.state.option = None;
                            // Leave the warning visible; X refreshes the remaining PC.
                            self.state.error =
                                (!remote_ok).then_some("moonlight.remote_unpair_failed");
                        }
                        Err(e) => self.fail(e),
                    }
                }
                Event::Done(result) => {
                    self.state.busy = false;
                    self.state.pin = None;
                    if self.cancel.load(Ordering::Relaxed) {
                        if self.active {
                            self.refresh();
                        }
                        continue;
                    }
                    match result {
                        Ok(Some(Reply::Apps(apps))) => {
                            self.state.apps = apps;
                            self.state.status = Status::Online;
                        }
                        Ok(Some(Reply::NeedsPair | Reply::Unpaired)) => {
                            self.state.apps.clear();
                            self.state.status = Status::NeedsPair;
                        }
                        Ok(Some(Reply::Paired)) => {
                            if self.active {
                                self.refresh();
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.state.apps.clear();
                            self.state.status = Status::Offline;
                            self.fail(e);
                        }
                    }
                    self.state.cursor = self
                        .state
                        .cursor
                        .min(self.state.rows().len().saturating_sub(1));
                }
            }
        }
        dirty
    }
    fn job(&mut self, action: Option<&'static str>, persist: bool) {
        if self.state.busy {
            return;
        }
        let host = self.state.address().unwrap_or_default().to_owned();
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        let mut hosts = self.state.hosts.clone();
        let options = self.state.options.clone();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.state.busy = true;
        self.state.error = None;
        match thread::Builder::new()
            .name("moonlight-cli".into())
            .spawn(move || {
                let result = (|| {
                    if persist {
                        save(&paths, &hosts, &options)?;
                    }
                    action
                        .map(|action| {
                            let pin = if action == "pair" {
                                let pin = random_pin().map_err(|e| e.to_string())?;
                                let _ = tx.send(Event::Pairing(pin.clone()));
                                Some(pin)
                            } else {
                                None
                            };
                            execute(
                                &paths,
                                action,
                                &host,
                                Duration::from_secs(if action == "pair" { 120 } else { 15 }),
                                &cancel,
                                pin.as_deref(),
                            )
                        })
                        .transpose()
                })();
                if action == Some("unpair") {
                    // Confirmation authorizes local forgetting even if the PC is offline.
                    let remote_ok = matches!(result, Ok(Some(Reply::Unpaired)));
                    if let Err(ref e) = result {
                        log::warn!("Moonlight remote unpair: {e}");
                    }
                    hosts.retain(|h| h != &host);
                    let forgotten = save(&paths, &hosts, &options).map(|()| (hosts, remote_ok));
                    let _ = tx.send(Event::Forgotten(forgotten));
                    return;
                }
                let _ = tx.send(Event::Done(result));
            }) {
            Ok(worker) => self.worker = Some(worker),
            Err(e) => self.fail(e.to_string()),
        }
    }
    fn refresh(&mut self) {
        if self.state.address().is_some() {
            self.state.apps.clear();
            self.state.status = Status::Unknown;
            self.state.cursor = 0;
            self.job(Some("list"), false);
        }
    }
    pub fn handle(&mut self, button: Button) -> Outcome {
        if self.state.busy {
            if button == Button::B {
                self.close();
            }
            return Outcome::None;
        }
        if self.state.confirm_unpair {
            match button {
                Button::B => self.state.confirm_unpair = false,
                Button::A => {
                    self.state.confirm_unpair = false;
                    self.job(Some("unpair"), false);
                }
                _ => {}
            }
        } else if let Some(keyboard) = &mut self.state.keyboard {
            if button == Button::B {
                self.state.keyboard = None;
                self.state.error = None;
            } else if let Some(input) = keyboard.handle(button) {
                match address(&input) {
                    Ok(host) => {
                        let index = self.state.hosts.iter().position(|h| h == &host);
                        if index.is_none() && self.state.hosts.len() >= MAX_HOSTS {
                            self.state.error = Some("moonlight.host_limit");
                        } else {
                            self.state.host = index.unwrap_or_else(|| {
                                self.state.hosts.push(host);
                                self.state.hosts.len() - 1
                            });
                            self.state.keyboard = None;
                            self.state.apps.clear();
                            self.state.status = Status::Unknown;
                            self.state.cursor = 0;
                            self.job(Some("list"), true);
                        }
                    }
                    Err(_) => self.state.error = Some("moonlight.invalid_ip"),
                }
            }
        } else if let Some(row) = self.state.option {
            match button {
                Button::B | Button::Y => {
                    self.state.option = None;
                    self.job(None, true);
                }
                Button::Up => self.state.option = Some(row.saturating_sub(1)),
                Button::Down => {
                    self.state.option =
                        Some((row + 1).min(if self.state.address().is_some() { 4 } else { 3 }))
                }
                Button::L | Button::R | Button::A if row < 4 => {
                    self.state
                        .options
                        .adjust(row, button != Button::L, self.panel)
                }
                Button::A if row == 4 => self.state.confirm_unpair = true,
                _ => {}
            }
        } else {
            match button {
                Button::B => self.close(),
                Button::Y => self.state.option = Some(0),
                Button::X => self.refresh(),
                Button::Up => self.state.cursor = self.state.cursor.saturating_sub(1),
                Button::Down => {
                    self.state.cursor = (self.state.cursor + 1).min(self.state.rows().len() - 1)
                }
                Button::A => match self.state.rows()[self.state.cursor] {
                    Row::App(i) => {
                        return Outcome::MoonlightStream(Stream {
                            host: self.state.address().unwrap_or_default().into(),
                            app: self.state.apps[i].clone(),
                            options: self.state.options.clone(),
                        })
                    }
                    Row::Pair => self.job(Some("pair"), false),
                    Row::Host(i) => {
                        self.state.host = i;
                        self.refresh();
                    }
                    Row::Add => self.state.keyboard = Some(Keyboard::default()),
                },
                _ => {}
            }
        }
        Outcome::None
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // Only at process teardown (including fatal SDL/script errors), never
        // on screen-close: let the worker reap its CLI before the process exits.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests;
