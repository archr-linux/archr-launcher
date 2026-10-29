//! Modules: programs the launcher starts in a sandbox and talks to with
//! JSON-RPC lines (docs/modulos.md). Processes, files and the network live on
//! the host thread and its helpers; the screens only read the `View` it
//! publishes, and the event loop only calls `poll`.
pub mod manifest;
pub mod process;
pub mod registry;
pub mod store;

use archr_ui::modules::{Call, Request, StoreAction, StoreReply, View};
use manifest::Mode;
use process::{Direct, Executor, Plan};
use registry::{Dirs, Installed, State};
use serde_json::{json, Value as Json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant, SystemTime},
};

/// Daemons running at once; more must be one-shots.
pub const DAEMONS: usize = 3;
const INITIALIZE: Duration = Duration::from_secs(3);
const INVOKE: Duration = Duration::from_secs(1);
/// A one-shot's whole run: start, one message, exit.
const ONESHOT: Duration = Duration::from_secs(2);
/// Daemons get this long to leave after `shutdown`.
const SHUTDOWN: Duration = Duration::from_millis(500);
const PUNISHMENTS: usize = 3;
const PUNISH_WINDOW: Duration = Duration::from_secs(600);
const BACKOFF: [u64; 3] = [1, 5, 30];
/// Focus events wait for the cursor to rest.
const DEBOUNCE: Duration = Duration::from_millis(400);
/// `ui.update`s reach the screens at most four times a second.
const PUBLISH: Duration = Duration::from_millis(250);
const NOTIFY: Duration = Duration::from_secs(10);
/// All `launch.before` answers must arrive within this, together.
pub const BEFORE: Duration = Duration::from_millis(300);
/// Automatic updates: the first look this long after the host starts, then
/// once a day; only with `AUTO_KEY` on.
const AUTO_FIRST: Duration = Duration::from_secs(120);
const AUTO_EVERY: u64 = 24 * 3600;
pub const AUTO_KEY: &str = "launcher.modules.autoupdate";
/// Late `launch.before` answers in a row before it counts as a failure.
const LATE: u32 = 3;
/// `game.launch` is honoured this long after the user chose one of the
/// module's actions, once.
const GESTURE: Duration = Duration::from_secs(5);
/// One `config.set` per module in this time, and at most `CONFIG_HOURLY`
/// writes in the last hour.
const CONFIG_SET: Duration = Duration::from_secs(1);
const CONFIG_HOURLY: usize = 10;
const HOUR: Duration = Duration::from_secs(3600);
const LAST_LAUNCH: &str = "last-launch.json";

/// Where and how modules run.
pub struct Config {
    pub dirs: Dirs,
    /// /run/archr-launcher: the hand-off exit code and the last launch.
    pub run: PathBuf,
    /// system.cfg, for `config.get` and `config.set`, and its lock.
    pub system_cfg: PathBuf,
    pub lock: PathBuf,
    pub executor: Arc<dyn Executor>,
    /// Set when the executor is `Direct`, which tracks its process groups.
    pub direct: Option<Arc<Direct>>,
    /// On the device (root): where each module's user record goes
    /// (`/run/userdb`); its data folder and secrets are handed to its uid.
    /// `None` on a development PC: everything stays the launcher's.
    pub userdb: Option<PathBuf>,
    pub language: String,
    pub device: String,
    /// Daemons start this long after the host.
    pub delay: Duration,
    /// The first look for automatic updates, this long after the host.
    pub auto_delay: Duration,
    /// On the device: the file archr-modules-firewall leaves once module
    /// uids cannot reach this device's own addresses. Without it no module
    /// gets the network.
    pub firewall: Option<PathBuf>,
    pub shop: Option<store::Shop>,
}
impl Config {
    /// The device's sandbox, or the plain runner for a fixture root.
    pub fn production(paths: &crate::paths::Paths, language: &str) -> Self {
        let dirs = Dirs::new(&paths.store);
        let shop = store::Shop::production(paths.cache.join("store/modules"), dirs.clone());
        let (executor, direct, userdb): (Arc<dyn Executor>, _, _) = if paths.root.is_some() {
            log::warn!("Modules run without a sandbox (fixture root)");
            let direct = Arc::new(Direct::default());
            (direct.clone(), Some(direct), None)
        } else {
            let systemd = process::Systemd {
                run: "/usr/bin/systemd-run".into(),
                ctl: "/usr/bin/systemctl".into(),
            };
            (Arc::new(systemd), None, Some("/run/userdb".into()))
        };
        let firewall = paths
            .root
            .is_none()
            .then(|| PathBuf::from("/run/archr/modules-firewall"));
        Self {
            dirs,
            run: paths.run.clone(),
            system_cfg: paths.config.clone(),
            lock: paths.lock.clone(),
            executor,
            direct,
            userdb,
            language: language.to_owned(),
            device: String::new(),
            delay: Duration::from_secs(2),
            auto_delay: AUTO_FIRST,
            firewall,
            shop: Some(shop),
        }
    }
}
/// What the event loop applies.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    View(Arc<View>),
    Notice(Notice),
    Store(StoreReply),
    /// A module changed system.cfg: whoever caches it reads it again.
    Config,
    /// A module asks to open this game, as if the user chose it.
    Launch {
        system: String,
        game_key: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The module with this name was switched off after failing.
    Disabled(String),
    /// Its update failed and the previous version is back.
    RolledBack(String),
    /// Another daemon would pass the limit.
    TooMany(String),
    /// Updated on its own: name and version.
    Updated(String, String),
    /// `ui.notify` text.
    Text(String),
}

/// Where the governor chosen for one launch goes.
type Governor = mpsc::Sender<Option<String>>;

enum Input {
    Request(Request),
    Language(String),
    Focus(&'static str, Json),
    GameStarted(Json),
    /// `launch.before` to the modules with the hook; the governor comes back.
    Before(Json, Governor),
    Stop(mpsc::Sender<()>),
    Resume,
    Line {
        id: String,
        generation: u64,
        line: Option<Result<String, ()>>,
    },
    Oneshot {
        id: String,
        request: Option<(u64, String)>,
        lines: Vec<Result<String, ()>>,
        timeout: bool,
        code: Option<i32>,
    },
    Store(Box<(StoreReply, Option<store::Installed>)>),
}

/// The event loop's handle on the host thread.
pub struct Modules {
    tx: mpsc::Sender<Input>,
    rx: mpsc::Receiver<Event>,
    focus: Option<Json>,
}
impl Modules {
    pub fn start(config: Config) -> Self {
        let (tx, inbox) = mpsc::channel();
        let (out, rx) = mpsc::channel();
        let host_tx = tx.clone();
        thread::Builder::new()
            .name("launcher-modules".into())
            .spawn(move || Host::new(config, host_tx, out).run(&inbox))
            .expect("modules thread");
        Self {
            tx,
            rx,
            focus: None,
        }
    }
    pub fn request(&self, request: Request) {
        let _ = self.tx.send(Input::Request(request));
    }
    pub fn language(&self, language: &str) {
        let _ = self.tx.send(Input::Language(language.to_owned()));
    }
    /// The browsed system, or game; sent only when it changes.
    pub fn focus(&mut self, system: &str, game: Option<(&str, &str)>) {
        let (event, data) = match game {
            Some((key, name)) => (
                "focus.game",
                json!({"system": system, "game_key": key, "name": name}),
            ),
            None => ("focus.system", json!({"system": system})),
        };
        if self.focus.as_ref() != Some(&data) {
            self.focus = Some(data.clone());
            let _ = self.tx.send(Input::Focus(event, data));
        }
    }
    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
    /// A game is about to take the screen: ask the modules with the launch
    /// hook (`BEFORE` at most), tell every module, then stop them all.
    /// Returns once nothing runs (500 ms for daemons, then killed), with the
    /// CPU governor a module chose for this launch.
    pub fn hand_off(&self, system: &str, game_key: &str, name: &str) -> Option<String> {
        let game = json!({"system": system, "game_key": game_key, "name": name});
        let mut governor = None;
        if !system.is_empty() {
            let (tx, rx) = mpsc::channel();
            if self.tx.send(Input::Before(game.clone(), tx)).is_ok() {
                governor = rx
                    .recv_timeout(BEFORE + Duration::from_millis(200))
                    .ok()
                    .flatten();
            }
        }
        let _ = self.tx.send(Input::GameStarted(game));
        let (ack, done) = mpsc::channel();
        if self.tx.send(Input::Stop(ack)).is_ok() {
            let _ = done.recv_timeout(Duration::from_secs(5));
        }
        governor
    }
    /// The hand-off failed: modules run again.
    pub fn resume(&self) {
        let _ = self.tx.send(Input::Resume);
    }
}

struct Daemon {
    child: std::process::Child,
    tx: mpsc::SyncSender<String>,
    generation: u64,
    ready: bool,
}
#[derive(Debug)]
enum Waiting {
    Initialize,
    Invoke(String),
    Shutdown,
    Before,
}

struct Host {
    config: Config,
    inbox: mpsc::Sender<Input>,
    out: mpsc::Sender<Event>,
    store: Option<mpsc::Sender<StoreAction>>,
    installed: Vec<Installed>,
    state: State,
    view: View,
    daemons: BTreeMap<String, Daemon>,
    pending: BTreeMap<u64, (String, Waiting, Instant)>,
    next: u64,
    failures: BTreeMap<String, Vec<Instant>>,
    restarts: BTreeMap<String, (Instant, usize)>,
    start_at: Instant,
    started: bool,
    stopped: bool,
    /// Events of this session's start, for daemons as they become ready.
    boot: Vec<(&'static str, Json)>,
    greeted: BTreeSet<String>,
    focus: Option<(Instant, &'static str, Json)>,
    dirty: Option<Instant>,
    published: Instant,
    notified: BTreeMap<String, Instant>,
    /// When each module's `config.set`s were saved, the last hour's.
    written: BTreeMap<String, Vec<Instant>>,
    /// When to look for automatic updates next.
    auto_at: Instant,
    /// Modules still to look at, one store action each, and whether one is
    /// with the store now.
    auto_queue: std::collections::VecDeque<String>,
    auto_busy: bool,
    /// `launch.before` answers each module was late for in a row.
    late: BTreeMap<String, u32>,
    /// When the user last chose an action of each module (`game.launch`).
    gestures: BTreeMap<String, Instant>,
    /// A launch waiting for `launch.before`: where the answer goes and the
    /// governor each module chose.
    before: Option<(Governor, BTreeMap<String, String>)>,
}

impl Host {
    fn new(mut config: Config, inbox: mpsc::Sender<Input>, out: mpsc::Sender<Event>) -> Self {
        if config.direct.is_none() && config.device.is_empty() {
            // Read on the host thread.
            config.device = fs::read_to_string("/etc/hostname")
                .unwrap_or_default()
                .trim()
                .to_owned();
        }
        let store = config.shop.take().map(|shop| {
            let (tx, rx) = mpsc::channel();
            let replies = inbox.clone();
            thread::Builder::new()
                .name("launcher-module-store".into())
                .spawn(move || store::lane(&shop, &rx, &replies))
                .expect("module store thread");
            tx
        });
        let mut host = Self {
            start_at: Instant::now() + config.delay,
            installed: Vec::new(),
            state: State::default(),
            view: View::default(),
            daemons: BTreeMap::new(),
            pending: BTreeMap::new(),
            next: 0,
            failures: BTreeMap::new(),
            restarts: BTreeMap::new(),
            started: false,
            stopped: false,
            boot: Vec::new(),
            greeted: BTreeSet::new(),
            focus: None,
            dirty: None,
            published: Instant::now(),
            notified: BTreeMap::new(),
            written: BTreeMap::new(),
            gestures: BTreeMap::new(),
            late: BTreeMap::new(),
            auto_at: Instant::now() + config.delay.max(config.auto_delay),
            auto_queue: std::collections::VecDeque::new(),
            auto_busy: false,
            before: None,
            config,
            inbox,
            out,
            store,
        };
        host.boot = host.last_launch();
        host.reload();
        host
    }
    /// `launcher.started`, and `game.ended` when a game just closed.
    fn last_launch(&self) -> Vec<(&'static str, Json)> {
        let path = self.config.run.join(LAST_LAUNCH);
        let ended = archr_io::read_bytes(&path, 4096, archr_io::Symlinks::Reject)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Json>(&bytes).ok());
        let _ = fs::remove_file(&path);
        let mut events = vec![("launcher.started", json!({"cold": ended.is_none()}))];
        if let Some(last) = ended {
            let seconds = now_secs().saturating_sub(last["started"].as_u64().unwrap_or(0));
            events.push((
                "game.ended",
                json!({
                    "game": last["game"],
                    "exit_code": crate::handoff::read_last_exit(&self.config.run),
                    "seconds": seconds,
                }),
            ));
        }
        events
    }
    fn run(mut self, inbox: &mpsc::Receiver<Input>) {
        loop {
            let wait = self.deadline().saturating_duration_since(Instant::now());
            match inbox.recv_timeout(wait) {
                Ok(input) => self.input(input),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.stop();
                    return;
                }
            }
            self.tick();
        }
    }
    fn deadline(&self) -> Instant {
        let mut at = Instant::now() + Duration::from_secs(1);
        let mut soon = |t: Instant| at = at.min(t);
        if !self.started && !self.stopped {
            soon(self.start_at);
        }
        self.pending.values().for_each(|p| soon(p.2));
        self.restarts.values().for_each(|r| soon(r.0));
        if let Some((t, ..)) = self.focus {
            soon(t);
        }
        if let Some(t) = self.dirty {
            soon(t);
        }
        at
    }

    // ---- registry ----

    fn reload(&mut self) {
        self.state = State::load(&self.config.dirs.state);
        self.installed = registry::scan(&self.config.dirs, &self.config.language);
        let before = self.state.clone();
        for installed in &self.installed {
            // A folder copied by hand: off until the user accepts it.
            registry::record(&mut self.state, &installed.id);
            if self.state.uid(&installed.id).is_none() {
                log::warn!("Module {}: no uid left", installed.id);
            }
        }
        if self.state != before {
            self.save_state();
        }
        let old = std::mem::take(&mut self.view.modules);
        self.view.modules = self
            .installed
            .iter()
            .map(|i| {
                let mut module = registry::module(
                    &self.config.dirs,
                    i,
                    self.state.modules.get(&i.id),
                    &self.config.language,
                );
                if let Some(seen) = old.iter().find(|m| m.id == module.id) {
                    module.reviewed = seen.reviewed && seen.version == module.version;
                    module.update = seen
                        .update
                        .clone()
                        .filter(|_| seen.version == module.version);
                }
                module
            })
            .collect();
        let ids: BTreeSet<&String> = self.view.modules.iter().map(|m| &m.id).collect();
        self.view.values.retain(|(id, _), _| ids.contains(id));
        self.view.details.retain(|(id, _), _| ids.contains(id));
        self.view.progress.retain(|(id, _), _| ids.contains(id));
        self.view.items.retain(|(id, _), _| ids.contains(id));
        for module in &mut self.view.modules {
            module.failures = self.failures.get(&module.id).map_or(0, |f| {
                f.iter().filter(|at| at.elapsed() < PUNISH_WINDOW).count()
            });
        }
        self.publish();
    }
    fn save_state(&self) {
        if let Err(error) = self.state.save(&self.config.dirs.state) {
            log::warn!("Cannot save modules.toml: {error}");
        }
    }
    fn publish(&mut self) {
        self.dirty = None;
        self.published = Instant::now();
        let _ = self.out.send(Event::View(Arc::new(self.view.clone())));
    }
    fn touch(&mut self) {
        if self.dirty.is_none() {
            self.dirty = Some((self.published + PUBLISH).max(Instant::now()));
        }
    }
    fn find(&self, id: &str) -> Option<&Installed> {
        self.installed.iter().find(|i| i.id == id)
    }
    fn module(&self, id: &str) -> Option<&archr_ui::modules::Module> {
        self.view.modules.iter().find(|m| m.id == id)
    }
    fn active(&self, id: &str) -> Option<(&Installed, &manifest::Manifest)> {
        let installed = self.find(id)?;
        let manifest = installed.manifest.as_ref()?;
        self.module(id)?.active().then_some((installed, manifest))
    }
    fn name(&self, id: &str) -> String {
        self.module(id)
            .map_or_else(|| id.to_owned(), |m| m.name.clone())
    }

    // ---- inputs ----

    fn input(&mut self, input: Input) {
        match input {
            Input::Request(request) => self.request(request),
            Input::Language(language) => {
                self.config.language = language;
                self.reload();
            }
            Input::Focus(event, data) => {
                self.focus = Some((Instant::now() + DEBOUNCE, event, data));
            }
            Input::GameStarted(game) => {
                let last = json!({"game": game, "started": now_secs()});
                let path = self.config.run.join(LAST_LAUNCH);
                if let Err(error) = archr_io::atomic_publish(&path, false, |file| {
                    std::io::Write::write_all(file, last.to_string().as_bytes())
                }) {
                    log::warn!("Cannot record the launch for modules: {error}");
                }
                self.deliver("game.started", &json!({"game": game}));
            }
            Input::Before(game, reply) => self.before(&game, reply),
            Input::Stop(ack) => {
                self.stop();
                let _ = ack.send(());
            }
            Input::Resume => {
                self.stopped = false;
                self.started = false;
                self.start_at = Instant::now();
                self.next_auto();
            }
            Input::Line {
                id,
                generation,
                line,
            } => {
                if self
                    .daemons
                    .get(&id)
                    .is_some_and(|d| d.generation == generation)
                {
                    match line {
                        Some(line) => {
                            if !self.line(&id, line, false) {
                                self.end_daemon(&id, true);
                            }
                        }
                        None => self.daemon_exited(&id),
                    }
                }
            }
            Input::Oneshot {
                id,
                request,
                lines,
                timeout,
                code,
            } => {
                if let Some((_, action)) = &request {
                    self.view.busy.remove(&(id.clone(), action.clone()));
                    self.touch();
                }
                let mut ok = !timeout && code == Some(0);
                for line in lines {
                    ok &= self.line(&id, line, true);
                }
                if self.stopped {
                    return;
                }
                if ok {
                    self.commit(&id);
                } else {
                    let why = if timeout { "timed out" } else { "failed" };
                    self.punish(&id, why);
                }
            }
            Input::Store(answer) => {
                let (reply, installed) = *answer;
                let auto = matches!(reply, StoreReply::Updated(..) | StoreReply::Checked(_));
                match (installed, &reply) {
                    (Some(installed), StoreReply::Updated(..)) => self.auto_updated(installed),
                    (Some(installed), _) => self.installed_from_store(installed),
                    _ => {}
                }
                if auto {
                    self.auto_busy = false;
                    self.next_auto();
                }
                self.store_reply(&reply);
                let _ = self.out.send(Event::Store(reply));
            }
        }
    }
    fn request(&mut self, request: Request) {
        match request {
            Request::Invoke {
                module,
                action,
                call,
            } => self.invoke(&module, &action, call),
            Request::Set { module, key, value } => self.set(&module, &key, &value),
            Request::Enable { module, on, grant } => self.enable(&module, on, grant),
            Request::Remove { module, data } => self.remove(&module, data),
            Request::Store(action) => {
                if let Some(store) = &self.store {
                    let _ = store.send(action);
                }
            }
            Request::Open { .. } => {}
        }
    }
    fn invoke(&mut self, id: &str, action: &str, call: Option<Call>) {
        let Some(mode) = self.active(id).map(|(_, m)| m.mode) else {
            return;
        };
        let context = call.map_or(json!({}), |c| match c.item {
            Some(item) => json!({"item": item}),
            None => json!({"system": c.system, "game_key": c.game_key, "rom": c.rom}),
        });
        let params = json!({"action": action, "context": context});
        self.next += 1;
        let request_id = self.next;
        let message = json!({"jsonrpc": "2.0", "id": request_id, "method": "action.invoke", "params": params});
        let sent = match mode {
            Mode::Oneshot if !self.stopped => {
                self.oneshot(id, message, Some((request_id, action.to_owned())));
                true
            }
            Mode::Daemon if self.daemons.get(id).is_some_and(|d| d.ready) => {
                self.pending.insert(
                    request_id,
                    (
                        id.to_owned(),
                        Waiting::Invoke(action.to_owned()),
                        Instant::now() + INVOKE,
                    ),
                );
                self.send(id, &message);
                true
            }
            _ => false,
        };
        if sent {
            self.gestures.insert(id.to_owned(), Instant::now());
            self.view.busy.insert((id.to_owned(), action.to_owned()));
            self.publish();
        }
    }
    fn set(&mut self, id: &str, key: &str, value: &str) {
        let Some(installed) = self.find(id) else {
            return;
        };
        let Some(manifest) = installed.manifest.clone() else {
            return;
        };
        let Some(row) = manifest
            .pages
            .iter()
            .flat_map(|p| &p.rows)
            .find(|r| r.kind.key() == Some(key))
        else {
            return;
        };
        use archr_ui::modules::RowKind;
        let valid = match &row.kind {
            RowKind::Toggle { .. } | RowKind::Check { .. } => matches!(value, "0" | "1"),
            RowKind::Value { choices, .. } => choices.iter().any(|c| c == value),
            RowKind::Keyboard { max, .. } => {
                value.chars().count() <= usize::from(*max) && !value.chars().any(char::is_control)
            }
            _ => false,
        };
        if !valid {
            log::warn!("Module {id}: setting {key} refused");
            return;
        }
        let owner = self.owner(id);
        let Some(module) = self.view.modules.iter_mut().find(|m| m.id == id) else {
            return;
        };
        module.settings.insert(key.to_owned(), value.to_owned());
        if let Err(error) =
            registry::save_settings(&self.config.dirs, &manifest, &module.settings, owner)
        {
            log::warn!("Module {id}: cannot save settings: {error}");
        }
        let json = registry::settings_json(&manifest, &module.settings);
        self.publish();
        if self.daemons.get(id).is_some_and(|d| d.ready) {
            let data = json!({"key": key, "value": json[key]});
            self.send(id, &event("settings.changed", &data));
        }
    }
    fn enable(&mut self, id: &str, on: bool, grant: bool) {
        let tokens: Vec<String> = match self.find(id).and_then(|i| i.manifest.as_ref()) {
            Some(manifest) => manifest.permissions.tokens().into_iter().collect(),
            None => return,
        };
        let daemon = self
            .find(id)
            .and_then(|i| i.manifest.as_ref())
            .is_some_and(|m| m.mode == Mode::Daemon);
        let running = self
            .view
            .modules
            .iter()
            .filter(|m| m.active() && m.id != id)
            .filter(|m| {
                self.find(&m.id)
                    .and_then(|i| i.manifest.as_ref())
                    .is_some_and(|m| m.mode == Mode::Daemon)
            })
            .count();
        if on && daemon && running >= DAEMONS {
            let _ = self.out.send(Event::Notice(Notice::TooMany(self.name(id))));
            return;
        }
        let record = registry::record(&mut self.state, id);
        if grant {
            record.consent = true;
            record.granted = tokens;
        }
        record.enabled = on;
        if on {
            record.failed = false;
            self.failures.remove(id);
        }
        self.save_state();
        if !on {
            self.end_daemon(id, false);
        }
        self.reload();
        self.restarts.remove(id);
        if on && daemon && self.started && !self.stopped {
            self.spawn_daemon(id);
        }
    }
    fn remove(&mut self, id: &str, data: bool) {
        if self.find(id).is_none() {
            return;
        }
        self.end_daemon(id, false);
        let dirs = &self.config.dirs;
        for dir in [dirs.module(id), dirs.previous(id)] {
            let _ = fs::remove_dir_all(dir);
        }
        if data {
            let _ = fs::remove_dir_all(dirs.data_dir(id));
            let _ = fs::remove_file(dirs.settings_file(id));
            let _ = fs::remove_file(dirs.secrets_file(id));
            self.state.uids.remove(id);
        }
        self.state.modules.remove(id);
        self.save_state();
        self.failures.remove(id);
        self.reload();
    }
    fn installed_from_store(&mut self, installed: store::Installed) {
        let record = registry::record(&mut self.state, &installed.id);
        record.source = installed.source;
        record.version = installed.version;
        record.previous_digest = std::mem::replace(&mut record.digest, installed.digest);
        record.granted = installed.granted;
        record.consent = true;
        record.enabled = true;
        record.failed = false;
        self.save_state();
        self.end_daemon(&installed.id, false);
        self.failures.remove(&installed.id);
        self.reload();
        let daemon = self
            .find(&installed.id)
            .and_then(|i| i.manifest.as_ref())
            .is_some_and(|m| m.mode == Mode::Daemon);
        if daemon && self.started && !self.stopped {
            self.spawn_daemon(&installed.id);
        }
    }
    /// An update the store made on its own. The module keeps the user's
    /// switch and consent; one removed or taken by another source meanwhile
    /// is not brought back.
    fn auto_updated(&mut self, installed: store::Installed) {
        let id = installed.id.clone();
        match self.state.modules.get(&id) {
            Some(record) if record.source == installed.source => {}
            Some(_) => {
                log::warn!("Module {id} changed source during its automatic update");
                self.reload();
                return;
            }
            None => {
                // Removed while the store worked: it stays removed.
                log::warn!("Module {id} was removed during its automatic update");
                let dirs = &self.config.dirs;
                for dir in [dirs.module(&id), dirs.previous(&id)] {
                    let _ = fs::remove_dir_all(dir);
                }
                self.reload();
                return;
            }
        }
        let record = registry::record(&mut self.state, &id);
        record.previous_digest = std::mem::take(&mut record.digest);
        record.version = installed.version.clone();
        record.digest = installed.digest;
        record.auto_updated = now_secs();
        self.save_state();
        self.end_daemon(&id, false);
        self.failures.remove(&id);
        self.reload();
        let daemon = self
            .find(&id)
            .and_then(|i| i.manifest.as_ref())
            .is_some_and(|m| m.mode == Mode::Daemon);
        if daemon && self.started && !self.stopped && self.active(&id).is_some() {
            self.spawn_daemon(&id);
        }
        let notice = Notice::Updated(self.name(&id), installed.version);
        let _ = self.out.send(Event::Notice(notice));
    }
    /// Store cards tell which installed modules are reviewed (the digest of
    /// the installed package is on the reviewed list) or outdated.
    fn store_reply(&mut self, reply: &StoreReply) {
        let cards = match reply {
            // The Installed tab comes from the disk: it knows neither.
            StoreReply::Listed { tab, .. } if *tab == archr_ui::modules::Tab::Installed => return,
            StoreReply::Listed { cards, .. } => cards.as_slice(),
            StoreReply::Card(card) => std::slice::from_ref(card),
            _ => return,
        };
        for card in cards {
            if let Some(module) = self
                .view
                .modules
                .iter_mut()
                .find(|m| m.source == card.source() && m.id == card.id)
            {
                let installed = self
                    .state
                    .modules
                    .get(&module.id)
                    .map_or("", |r| r.digest.as_str());
                module.reviewed = card
                    .digests
                    .iter()
                    .any(|d| store::same_digest(d, installed));
                module.update = archr_ui::settings::store::newer(&card.version, &module.version)
                    .then(|| card.version.clone());
            }
        }
        self.touch();
    }

    // ---- processes ----

    fn plan(&self, id: &str, unit: String) -> Option<Plan> {
        let (installed, manifest) = self.active(id)?;
        let dirs = &self.config.dirs;
        let uid = self.state.uids.get(id).copied()?;
        let (data, settings) = (dirs.data_dir(id), dirs.settings_file(id));
        let secrets = dirs.secrets_file(id);
        let entry = installed.dir.join(&manifest.entry);
        let argv = match manifest.runtime {
            manifest::Runtime::Sh => vec!["/bin/sh".into(), entry.into_os_string()],
            manifest::Runtime::Bash => vec!["/bin/bash".into(), entry.into_os_string()],
            manifest::Runtime::Native => vec![entry.into_os_string()],
        };
        let path = |p: &PathBuf| p.display().to_string();
        let env = [
            ("HOME", path(&data)),
            ("LANG", "C.UTF-8".into()),
            ("ARCHR_API_VERSION", manifest::API_VERSION.to_string()),
            ("ARCHR_LAUNCHER_VERSION", env!("CARGO_PKG_VERSION").into()),
            ("ARCHR_MODULE_ID", id.into()),
            ("ARCHR_MODULE_DIR", path(&installed.dir)),
            ("ARCHR_MODULE_DATA", path(&data)),
            ("ARCHR_MODULE_SETTINGS", path(&settings)),
            ("ARCHR_MODULE_SECRETS", path(&secrets)),
            ("ARCHR_LANG", self.config.language.clone()),
            ("ARCHR_DEVICE", self.config.device.clone()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        Some(Plan {
            unit,
            id: id.to_owned(),
            dir: installed.dir.clone(),
            argv,
            data,
            settings,
            secrets,
            uid,
            env,
            net: manifest.permissions.net && self.firewall_ready(id),
            read: manifest
                .permissions
                .read
                .iter()
                .filter_map(|a| manifest::ALIASES.iter().find(|(n, _)| n == a))
                .map(|(_, p)| PathBuf::from(p))
                .collect(),
            memory_mb: manifest.memory_mb,
            resolv: if manifest.permissions.net {
                archr_io::read_utf8(
                    std::path::Path::new("/etc/resolv.conf"),
                    64 << 10,
                    archr_io::Symlinks::Follow,
                )
                .unwrap_or_default()
            } else {
                String::new()
            },
        })
    }
    /// The device's own addresses are closed to module uids (or there is
    /// no such guard to wait for, off the device).
    fn firewall_ready(&self, id: &str) -> bool {
        let ready = self
            .config
            .firewall
            .as_ref()
            .is_none_or(|marker| marker.exists());
        if !ready {
            log::warn!("Module {id} runs without network: the modules firewall is not up");
        }
        ready
    }
    /// The module's uid when its files are handed to it (on the device).
    fn owner(&self, id: &str) -> Option<u32> {
        self.config.userdb.as_ref()?;
        self.state.uids.get(id).copied()
    }
    /// The module's user, its data folder (0700, its own) and its settings
    /// files must exist before a sandbox can use them.
    fn prepare(&self, id: &str) -> bool {
        use std::os::unix::fs::{chown, PermissionsExt};
        let dirs = &self.config.dirs;
        let (Some(installed), Some(module), Some(&uid)) =
            (self.find(id), self.module(id), self.state.uids.get(id))
        else {
            return false;
        };
        let data = dirs.data_dir(id);
        let mut ok = fs::create_dir_all(&data).is_ok()
            && fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).is_ok();
        if let Some(userdb) = &self.config.userdb {
            ok &= process::user(userdb, uid).is_ok() && chown(&data, Some(uid), Some(uid)).is_ok();
        }
        if let Some(manifest) = &installed.manifest {
            let missing = !dirs.settings_file(id).is_file()
                || registry::has_secrets(manifest) && !dirs.secrets_file(id).is_file();
            if missing {
                ok &= registry::save_settings(dirs, manifest, &module.settings, self.owner(id))
                    .is_ok();
            }
        }
        ok
    }
    /// "Último uso" in the details: saved at most once an hour.
    fn ran(&mut self, id: &str) {
        let now = now_secs();
        let record = registry::record(&mut self.state, id);
        if now.saturating_sub(record.used) < 3600 {
            return;
        }
        record.used = now;
        self.save_state();
        if let Some(module) = self.view.modules.iter_mut().find(|m| m.id == id) {
            module.used = now;
            self.touch();
        }
    }
    fn spawn_daemon(&mut self, id: &str) {
        if self.daemons.contains_key(id) || self.daemons.len() >= DAEMONS || !self.prepare(id) {
            return;
        }
        self.ran(id);
        let Some(plan) = self.plan(id, format!("archr-mod-{id}")) else {
            return;
        };
        self.next += 1;
        let generation = self.next;
        let inbox = self.inbox.clone();
        let owner = id.to_owned();
        let spawned = process::spawn(
            self.config.executor.as_ref(),
            self.config.direct.as_deref(),
            &plan,
            move |line| {
                let _ = inbox.send(Input::Line {
                    id: owner.clone(),
                    generation,
                    line,
                });
            },
        );
        let (child, stdin) = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                log::warn!("Module {id} cannot start: {error}");
                self.punish(id, "cannot start");
                return;
            }
        };
        self.daemons.insert(
            id.to_owned(),
            Daemon {
                child,
                tx: process::writer(stdin),
                generation,
                ready: false,
            },
        );
        let module = self.module(id).cloned().unwrap_or_default();
        let permissions: Vec<String> = self
            .find(id)
            .and_then(|i| i.manifest.as_ref())
            .map(|m| m.permissions.tokens().into_iter().collect())
            .unwrap_or_default();
        let settings = self
            .find(id)
            .and_then(|i| i.manifest.as_ref())
            .map_or(json!({}), |m| registry::settings_json(m, &module.settings));
        self.next += 1;
        let request = self.next;
        self.pending.insert(
            request,
            (
                id.to_owned(),
                Waiting::Initialize,
                Instant::now() + INITIALIZE,
            ),
        );
        let params = json!({
            "api_version": manifest::API_VERSION,
            "launcher_version": env!("CARGO_PKG_VERSION"),
            "language": self.config.language,
            "device": self.config.device,
            "settings": settings,
            "permissions": permissions,
            "capabilities": ["ui.update", "ui.notify", "settings.set", "config.get",
                "config.set", "game.launch", "launch.before", "log"],
        });
        self.send(
            id,
            &json!({"jsonrpc": "2.0", "id": request, "method": "initialize", "params": params}),
        );
    }
    fn oneshot(&mut self, id: &str, message: Json, request: Option<(u64, String)>) {
        if !self.prepare(id) {
            return;
        }
        self.ran(id);
        self.next += 1;
        let Some(plan) = self.plan(id, format!("archr-mod-{id}-{}", self.next)) else {
            return;
        };
        let (exec, direct) = (self.config.executor.clone(), self.config.direct.clone());
        let inbox = self.inbox.clone();
        let id = id.to_owned();
        thread::spawn(move || {
            let (tx, rx) = mpsc::channel();
            let spawned = process::spawn(exec.as_ref(), direct.as_deref(), &plan, move |line| {
                let _ = tx.send(line);
            });
            let (mut lines, mut timeout, mut code) = (Vec::new(), false, None);
            if let Ok((child, stdin)) = spawned {
                let writer = process::writer(stdin);
                let _ = writer.send(message.to_string());
                drop(writer);
                let deadline = Instant::now() + ONESHOT;
                loop {
                    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        Ok(Some(line)) if lines.len() < 64 => lines.push(line),
                        Ok(Some(_)) => {}
                        Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            timeout = true;
                            exec.kill(Some(&plan.unit));
                            break;
                        }
                    }
                }
                code = process::wait_for(child, Duration::from_secs(1));
            } else {
                log::warn!("Module {id} cannot start");
            }
            let _ = inbox.send(Input::Oneshot {
                id,
                request,
                lines,
                timeout,
                code,
            });
        });
    }
    fn send(&mut self, id: &str, message: &Json) {
        let full = self
            .daemons
            .get(id)
            .is_some_and(|d| d.tx.try_send(message.to_string()).is_err());
        if full {
            // It stopped reading its input.
            self.punish(id, "not reading");
        }
    }
    /// Ends a daemon now; `failed` counts it against the module.
    fn end_daemon(&mut self, id: &str, failed: bool) {
        self.pending.retain(|_, p| p.0 != id);
        if let Some(mut daemon) = self.daemons.remove(id) {
            self.config.executor.kill(Some(&format!("archr-mod-{id}")));
            let _ = daemon.child.kill();
            let _ = daemon.child.wait();
        }
        if failed {
            self.punish(id, "broke the protocol");
        }
    }
    fn daemon_exited(&mut self, id: &str) {
        let Some(mut daemon) = self.daemons.remove(id) else {
            return;
        };
        self.pending.retain(|_, p| p.0 != id);
        let mut code = None;
        for _ in 0..20 {
            if let Ok(Some(status)) = daemon.child.try_wait() {
                code = status.code();
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if code.is_none() {
            let _ = daemon.child.kill();
            let _ = daemon.child.wait();
        }
        if self.stopped {
            return;
        }
        if code != Some(0) {
            self.punish(id, "exited with an error");
        }
        if self.active(id).is_some() {
            let attempt = self.restarts.get(id).map_or(0, |r| r.1);
            let wait = Duration::from_secs(BACKOFF[attempt.min(BACKOFF.len() - 1)]);
            self.restarts
                .insert(id.to_owned(), (Instant::now() + wait, attempt + 1));
        }
    }
    fn stop(&mut self) {
        self.stopped = true;
        self.restarts.clear();
        self.focus = None;
        for id in self.daemons.keys().cloned().collect::<Vec<_>>() {
            self.next += 1;
            let request = self.next;
            self.pending
                .insert(request, (id.clone(), Waiting::Shutdown, Instant::now()));
            self.send(
                &id,
                &json!({"jsonrpc": "2.0", "id": request, "method": "shutdown", "params": {}}),
            );
        }
        let deadline = Instant::now() + SHUTDOWN;
        while Instant::now() < deadline
            && self
                .daemons
                .values_mut()
                .any(|d| matches!(d.child.try_wait(), Ok(None)))
        {
            thread::sleep(Duration::from_millis(10));
        }
        // Units still there (and every one-shot) are killed.
        self.config.executor.kill(None);
        for (_, mut daemon) in std::mem::take(&mut self.daemons) {
            let _ = daemon.child.kill();
            let _ = daemon.child.wait();
        }
        self.pending.clear();
        self.view.busy.clear();
    }

    // ---- protocol ----

    /// One line from a module; `false` when it broke the protocol.
    fn line(&mut self, id: &str, line: Result<String, ()>, oneshot: bool) -> bool {
        let message = line
            .ok()
            .and_then(|text| serde_json::from_str::<Json>(&text).ok())
            .filter(|m| m["jsonrpc"] == "2.0");
        let Some(message) = message else {
            log::warn!("Module {id}: invalid JSON-RPC line");
            if oneshot {
                return false;
            }
            self.punish(id, "invalid JSON");
            return true;
        };
        let params = &message["params"];
        match (message["method"].as_str(), message.get("id")) {
            (Some(method), None) => self.notification(id, method, params),
            (Some(method), Some(request)) if !oneshot => {
                let reply = match self.call(id, method, params) {
                    Ok(result) => json!({"jsonrpc": "2.0", "id": request, "result": result}),
                    Err((code, text)) => json!({"jsonrpc": "2.0", "id": request,
                        "error": {"code": code, "message": text}}),
                };
                self.send(id, &reply);
            }
            (Some(_), Some(_)) => {}
            (None, Some(request)) => self.response(id, request, &message),
            (None, None) => {}
        }
        true
    }
    fn response(&mut self, id: &str, request: &Json, message: &Json) {
        let Some(key) = request.as_u64() else { return };
        if self.pending.get(&key).is_none_or(|p| p.0 != id) {
            return;
        }
        let (_, waiting, _) = self.pending.remove(&key).expect("pending");
        match waiting {
            Waiting::Initialize if message["result"]["ready"] == true => {
                if let Some(daemon) = self.daemons.get_mut(id) {
                    daemon.ready = true;
                }
                self.restarts.remove(id);
                self.commit(id);
                if self.greeted.insert(id.to_owned()) {
                    let subscribed = self.subscribed(id);
                    for (name, data) in self.boot.clone() {
                        if subscribed.contains(name) {
                            self.send(id, &event(name, &data));
                        }
                    }
                }
            }
            Waiting::Initialize => self.end_daemon(id, true),
            Waiting::Invoke(action) => {
                self.view.busy.remove(&(id.to_owned(), action));
                self.touch();
            }
            Waiting::Shutdown => {}
            Waiting::Before => {
                self.late.remove(id);
                self.decision(id, &message["result"]);
            }
        }
    }
    fn notification(&mut self, id: &str, method: &str, params: &Json) {
        let Some(installed) = self.find(id) else {
            return;
        };
        match method {
            "ui.update" => {
                let Some(target) = params["target"]
                    .as_str()
                    .filter(|t| t.len() <= 64 && t.contains(':'))
                else {
                    return;
                };
                let key = (id.to_owned(), target.to_owned());
                let message = |field: &str| params.get(field).and_then(|v| installed.message(v));
                let (value, detail) = (message("value"), message("detail"));
                let items: Option<Vec<archr_ui::modules::Item>> =
                    params["items"].as_array().map(|items| {
                        let text = |item: &Json, field: &str| {
                            item.get(field)
                                .and_then(|v| installed.message(v))
                                .unwrap_or_default()
                        };
                        items
                            .iter()
                            .take(archr_ui::modules::ITEMS)
                            .map(|item| archr_ui::modules::Item {
                                title: text(item, "title"),
                                subtitle: text(item, "subtitle"),
                                value: text(item, "value"),
                                action: item["action"]
                                    .as_str()
                                    .filter(|a| a.len() <= 40)
                                    .map(str::to_owned),
                            })
                            .collect()
                    });
                if params["hidden"] == true {
                    self.view.hidden.insert(key.clone());
                } else {
                    self.view.hidden.remove(&key);
                }
                if let Some(value) = value {
                    self.view.values.insert(key.clone(), value);
                }
                if let Some(detail) = detail {
                    self.view.details.insert(key.clone(), detail);
                }
                if let Some(progress) = params["progress"].as_f64() {
                    let progress = progress.clamp(0., 100.).round() as u8;
                    self.view.progress.insert(key.clone(), progress);
                }
                if let Some(items) = items {
                    self.view.items.insert(key, items);
                }
                self.touch();
            }
            "ui.notify" => {
                let text = params
                    .get("text")
                    .and_then(|t| installed.message(t))
                    .or_else(|| installed.message(params));
                let quiet = self
                    .notified
                    .get(id)
                    .is_some_and(|at| at.elapsed() < NOTIFY);
                if let (Some(text), false) = (text, quiet) {
                    self.notified.insert(id.to_owned(), Instant::now());
                    let _ = self.out.send(Event::Notice(Notice::Text(text)));
                }
            }
            "log" => {
                let text: String = params["message"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(200)
                    .collect();
                if params["level"] == "error" {
                    log::warn!("Module {id}: {text}");
                } else {
                    log::info!("Module {id}: {text}");
                }
            }
            _ => {}
        }
    }
    /// A module's request: only the brokered operations of API 1.
    fn call(&mut self, id: &str, method: &str, params: &Json) -> Result<Json, (i64, &'static str)> {
        let manifest = self
            .find(id)
            .and_then(|i| i.manifest.clone())
            .ok_or((-32004, "not found"))?;
        match method {
            "settings.set" => {
                let key = params["key"].as_str().ok_or((-32602, "key"))?;
                let value = match &params["value"] {
                    Json::Bool(on) => if *on { "1" } else { "0" }.to_owned(),
                    Json::String(text) => text.clone(),
                    Json::Number(n) => n.to_string(),
                    _ => return Err((-32602, "value")),
                };
                let declared = manifest
                    .pages
                    .iter()
                    .flat_map(|p| &p.rows)
                    .any(|r| r.kind.key() == Some(key));
                if !declared {
                    return Err((-32001, "key not declared"));
                }
                self.set(id, key, &value);
                Ok(json!({}))
            }
            "config.get" => {
                let key = params["key"].as_str().ok_or((-32602, "key"))?;
                if !manifest.permissions.config_read.iter().any(|k| k == key) {
                    return Err((-32001, "permission denied"));
                }
                let value = archr_config::SystemCfg::load(&self.config.system_cfg)
                    .ok()
                    .and_then(|cfg| cfg.get_setting(key, None, None));
                Ok(json!({ "value": value }))
            }
            "config.set" => {
                let key = params["key"].as_str().ok_or((-32602, "key"))?;
                if !manifest.permissions.config_write.iter().any(|k| k == key) {
                    return Err((-32001, "permission denied"));
                }
                let value = params["value"].as_str().ok_or((-32602, "value"))?;
                if !manifest::writable(key, value) {
                    return Err((-32602, "value"));
                }
                let (path, lock) = (&self.config.system_cfg, &self.config.lock);
                let current = archr_config::SystemCfg::load(path)
                    .ok()
                    .and_then(|cfg| cfg.get_setting(key, None, None));
                if current.as_deref() == Some(value) {
                    // Nothing to write; it does not count either.
                    return Ok(json!({}));
                }
                let times = self.written.entry(id.to_owned()).or_default();
                times.retain(|at| at.elapsed() < HOUR);
                let busy = times.last().is_some_and(|at| at.elapsed() < CONFIG_SET);
                if busy || times.len() >= CONFIG_HOURLY {
                    return Err((-32003, "too many requests"));
                }
                times.push(Instant::now());
                if let Err(error) = archr_config::SystemCfg::set_and_save(path, lock, key, value) {
                    log::warn!("Module {id}: cannot save {key}: {error}");
                    return Err((-32000, "cannot save"));
                }
                log::info!("Module {id} set {key} to {value}");
                let _ = self.out.send(Event::Config);
                Ok(json!({}))
            }
            "game.launch" => {
                if !manifest.permissions.launch {
                    return Err((-32001, "permission denied"));
                }
                let text = |key: &str, max: usize| {
                    params[key]
                        .as_str()
                        .filter(|t| {
                            !t.is_empty() && t.len() <= max && !t.chars().any(char::is_control)
                        })
                        .map(str::to_owned)
                        .ok_or((-32602, "system and game_key"))
                };
                let (system, game_key) = (text("system", 64)?, text("game_key", 1024)?);
                // Only right after the user chose one of its actions, once.
                match self.gestures.remove(id) {
                    Some(at) if at.elapsed() < GESTURE => {}
                    _ => return Err((-32001, "only after the user's action")),
                }
                log::info!("Module {id} launches {system}/{game_key}");
                let _ = self.out.send(Event::Launch { system, game_key });
                Ok(json!({}))
            }
            _ => Err((-32601, "method not found")),
        }
    }
    fn subscribed(&self, id: &str) -> BTreeSet<&'static str> {
        let hooks = self
            .find(id)
            .and_then(|i| i.manifest.as_ref())
            .map(|m| m.hooks.clone())
            .unwrap_or_default();
        manifest::EVENTS
            .iter()
            .copied()
            .filter(|e| hooks.iter().any(|h| h == e))
            .collect()
    }
    /// An event to every active subscriber; one-shots never get focus events.
    fn deliver(&mut self, name: &'static str, data: &Json) {
        if self.stopped {
            return;
        }
        let ids: Vec<String> = self.view.modules.iter().map(|m| m.id.clone()).collect();
        for id in ids {
            let Some((_, manifest)) = self.active(&id) else {
                continue;
            };
            if !self.subscribed(&id).contains(name) {
                continue;
            }
            match manifest.mode {
                Mode::Daemon if self.daemons.get(&id).is_some_and(|d| d.ready) => {
                    self.send(&id, &event(name, data));
                }
                Mode::Oneshot if !name.starts_with("focus.") => {
                    self.oneshot(&id, event(name, data), None);
                }
                _ => {}
            }
        }
    }

    /// Asks the store for automatic updates when the user switched them on
    /// and a day passed since the last look. Never during a game: `tick`
    /// does nothing while stopped.
    fn auto_update(&mut self) {
        let on = archr_config::SystemCfg::load(&self.config.system_cfg)
            .ok()
            .and_then(|cfg| cfg.get_setting(AUTO_KEY, None, None))
            .is_some_and(|v| v == "1");
        let (now, store) = (now_secs(), &self.store);
        if !on || now.saturating_sub(self.state.checked) < AUTO_EVERY || store.is_none() {
            return;
        }
        self.state.checked = now;
        self.save_state();
        self.auto_queue = self
            .state
            .modules
            .iter()
            .filter(|(_, r)| !r.source.is_empty() && r.enabled && r.consent && !r.failed)
            .map(|(id, _)| id.clone())
            .collect();
        self.next_auto();
    }
    /// The next module to the store, one at a time and never while a game
    /// has the screen: the batch waits for the launcher to come back.
    fn next_auto(&mut self) {
        if self.stopped || self.auto_busy {
            return;
        }
        let (Some(store), Some(id)) = (&self.store, self.auto_queue.pop_front()) else {
            return;
        };
        if store.send(StoreAction::Auto(id)).is_ok() {
            self.auto_busy = true;
        }
    }

    // ---- launch hook ----

    /// `launch.before` to every ready daemon granted the launch hook.
    fn before(&mut self, game: &Json, reply: Governor) {
        let ids: Vec<String> = self
            .daemons
            .iter()
            .filter(|(_, d)| d.ready)
            .map(|(id, _)| id.clone())
            .filter(|id| {
                self.active(id)
                    .is_some_and(|(_, m)| m.permissions.hooks.iter().any(|h| h == "launch"))
            })
            .collect();
        self.before = Some((reply, BTreeMap::new()));
        for id in ids {
            self.next += 1;
            let request = self.next;
            self.pending.insert(
                request,
                (id.clone(), Waiting::Before, Instant::now() + BEFORE),
            );
            self.send(
                &id,
                &json!({"jsonrpc": "2.0", "id": request, "method": "launch.before",
                    "params": {"game": game}}),
            );
        }
        self.settle();
    }
    /// `{"decision": "allow"}` or `{"decision": "modify", "changes": {...}}`:
    /// only the changes the module was granted count.
    fn decision(&mut self, id: &str, result: &Json) {
        if result["decision"] != "modify" {
            return;
        }
        let granted = self
            .active(id)
            .is_some_and(|(_, m)| m.permissions.system.iter().any(|s| s == "cpu-profile"));
        let governor = result["changes"]["governor"].as_str();
        match (governor, &mut self.before) {
            (Some(g), Some((_, chosen))) if granted && manifest::GOVERNORS.contains(&g) => {
                chosen.insert(id.to_owned(), g.to_owned());
            }
            (Some(_), _) => log::warn!("Module {id}: launch change refused"),
            _ => {}
        }
    }
    /// Every answer is in, or late: the first module (by id) decides.
    fn settle(&mut self) {
        let waiting = self
            .pending
            .values()
            .any(|p| matches!(p.1, Waiting::Before));
        if waiting {
            return;
        }
        if let Some((reply, chosen)) = self.before.take() {
            let mut chosen = chosen.into_iter();
            let first = chosen.next();
            for (id, _) in chosen {
                log::info!("Module {id}: another module already chose the governor");
            }
            if let Some((id, governor)) = &first {
                log::info!("Module {id} runs this game with the {governor} governor");
            }
            let _ = reply.send(first.map(|(_, g)| g));
        }
    }

    // ---- failures ----

    fn punish(&mut self, id: &str, why: &str) {
        log::warn!("Module {id} {why}");
        let now = Instant::now();
        let list = self.failures.entry(id.to_owned()).or_default();
        list.retain(|at| now.duration_since(*at) < PUNISH_WINDOW);
        list.push(now);
        let count = list.len();
        if let Some(module) = self.view.modules.iter_mut().find(|m| m.id == id) {
            module.failures = count;
            self.touch();
        }
        if count < PUNISHMENTS {
            return;
        }
        self.failures.remove(id);
        self.end_daemon(id, false);
        self.restarts.remove(id);
        let name = self.name(id);
        if self.rollback(id) {
            let _ = self.out.send(Event::Notice(Notice::RolledBack(name)));
            return;
        }
        registry::record(&mut self.state, id).failed = true;
        self.save_state();
        self.reload();
        let _ = self.out.send(Event::Notice(Notice::Disabled(name)));
    }
    /// The new version works: its predecessor can go.
    fn commit(&mut self, id: &str) {
        let previous = self.config.dirs.previous(id);
        if previous.exists() {
            let _ = fs::remove_dir_all(previous);
        }
    }
    /// A fresh update keeps failing: the previous version comes back.
    fn rollback(&mut self, id: &str) -> bool {
        let dirs = &self.config.dirs;
        let (current, previous) = (dirs.module(id), dirs.previous(id));
        if !previous.is_dir() {
            return false;
        }
        let trash = dirs.modules.join(format!(".trash-{id}"));
        let _ = fs::remove_dir_all(&trash);
        if fs::rename(&current, &trash).is_err() || fs::rename(&previous, &current).is_err() {
            return false;
        }
        let _ = fs::remove_dir_all(&trash);
        let version = Installed::read(&current, "en")
            .manifest
            .map(|m| m.version)
            .unwrap_or_default();
        // The previous package is back, and the one that failed is never
        // taken again on its own.
        let record = registry::record(&mut self.state, id);
        record.version = version;
        record.bad_digest = std::mem::take(&mut record.digest);
        record.digest = std::mem::take(&mut record.previous_digest);
        self.save_state();
        self.reload();
        true
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if self.stopped {
            self.settle();
            return;
        }
        if !self.started && now >= self.start_at {
            self.started = true;
            let boot = self.boot.clone();
            let ids: Vec<String> = self.view.modules.iter().map(|m| m.id.clone()).collect();
            for id in ids {
                // One-shots get the session's start events once.
                if self
                    .active(&id)
                    .is_some_and(|(_, m)| m.mode == Mode::Oneshot)
                    && self.greeted.insert(id.clone())
                {
                    let subscribed = self.subscribed(&id);
                    for (name, data) in &boot {
                        if subscribed.contains(name) {
                            self.oneshot(&id, event(name, data), None);
                        }
                    }
                }
            }
        }
        // Daemons start one at a time, each after the previous answered.
        let starting = self
            .pending
            .values()
            .any(|p| matches!(p.1, Waiting::Initialize));
        if self.started && !starting {
            let next = self.view.modules.iter().map(|m| m.id.clone()).find(|id| {
                !self.daemons.contains_key(id)
                    && !self.restarts.contains_key(id)
                    && self.active(id).is_some_and(|(_, m)| m.mode == Mode::Daemon)
                    // A recent failure waits out the longest back-off; an
                    // old one never keeps it down (e.g. after a game).
                    && self
                        .failures
                        .get(id)
                        .and_then(|f| f.last())
                        .is_none_or(|at| at.elapsed() >= Duration::from_secs(BACKOFF[2]))
            });
            if let Some(id) = next {
                self.spawn_daemon(&id);
            }
        }
        let due: Vec<String> = self
            .restarts
            .iter()
            .filter(|(id, r)| r.0 <= now && !self.daemons.contains_key(*id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            if self.active(&id).is_some() {
                self.spawn_daemon(&id);
            } else {
                self.restarts.remove(&id);
            }
        }
        let late: Vec<(u64, String)> = self
            .pending
            .iter()
            .filter(|(_, p)| p.2 <= now)
            .map(|(k, p)| (*k, p.0.clone()))
            .collect();
        for (key, id) in late {
            match self.pending.remove(&key).map(|p| p.1) {
                Some(Waiting::Initialize) => {
                    log::warn!("Module {id} did not initialize in time");
                    self.end_daemon(&id, true);
                    self.restarts
                        .insert(id, (now + Duration::from_secs(BACKOFF[1]), 1));
                }
                Some(Waiting::Invoke(action)) => {
                    self.view.busy.remove(&(id.clone(), action));
                    self.touch();
                    self.punish(&id, "timed out");
                }
                // Late is "allow": the game never waits. Only a habit of it
                // counts against the module.
                Some(Waiting::Before) => {
                    let count = self.late.entry(id.clone()).or_default();
                    *count += 1;
                    log::warn!(
                        "Module {id} was late for launch.before ({count} in a row): allowed"
                    );
                    if *count >= LATE {
                        self.late.remove(&id);
                        self.punish(&id, "was late for launch.before 3 times in a row");
                    }
                }
                _ => {}
            }
        }
        self.settle();
        if now >= self.auto_at {
            self.auto_at = now + Duration::from_secs(3600);
            self.auto_update();
        }
        if self.focus.as_ref().is_some_and(|f| f.0 <= now) {
            let (_, name, data) = self.focus.take().expect("focus");
            self.deliver(name, &data);
        }
        if self.dirty.is_some_and(|at| at <= now) {
            self.publish();
        }
    }
}

fn event(name: &str, data: &Json) -> Json {
    json!({"jsonrpc": "2.0", "method": "event", "params": {"name": name, "data": data}})
}
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `--check-module <dir>`: the problems, one per line; `Ok` when none.
pub fn check_module(dir: &std::path::Path) -> Result<Vec<String>, Vec<String>> {
    manifest::check(dir).map(|(_, warnings)| warnings)
}
