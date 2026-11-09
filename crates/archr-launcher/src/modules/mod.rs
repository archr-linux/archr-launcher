//! Modules: programs the launcher starts in a sandbox and talks to with
//! JSON-RPC lines (docs/modulos.md). Processes, files and the network live on
//! the host thread and its helpers; the screens only read the `View` it
//! publishes, and the event loop only calls `poll`.
pub mod app;
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
/// A system script gets this long.
const SYSTEM_TIMEOUT: Duration = Duration::from_secs(15);
/// One `config.set` per module in this time, and at most `CONFIG_HOURLY`
/// writes in the last hour.
const CONFIG_SET: Duration = Duration::from_secs(1);
const CONFIG_HOURLY: usize = 10;
const HOUR: Duration = Duration::from_secs(3600);
const LAST_LAUNCH: &str = "last-launch.json";
/// An app's process (API 4): its resources, apart from the daemons'.
const APP_MEMORY: u32 = 96;
const APP_TASKS: u32 = 32;
/// Screen messages waiting for the next publish, at most.
const SCREENS: usize = 256;
/// One `rom.install` per module in this time.
const INSTALL_EVERY: Duration = Duration::from_secs(1);
/// `rom.remove` and `port.remove` only this soon after the user said yes
/// to one of the app's `screen.confirm`s, once.
const CONFIRMED: Duration = Duration::from_secs(30);

/// Where the pictures an app's screens show are copied, beside the data
/// folders and out of every module's reach (no id starts with a dot).
fn pictures(dirs: &Dirs, id: &str) -> PathBuf {
    dirs.data.join(".pictures").join(id)
}
/// The daemons map knows an app's process by this key; `_` is never in a
/// module id.
fn app_key(id: &str) -> String {
    format!("{id}_app")
}
/// The module a process key belongs to.
fn module_id(key: &str) -> &str {
    key.strip_suffix("_app").unwrap_or(key)
}

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
    /// The system's scripts (`/usr/bin`): brightness, volume, wifictl,
    /// archr-screenshot and archr-osd, for `system.call`.
    pub bin: PathBuf,
    pub shop: Option<store::Shop>,
    /// The CPU's cpufreq policy (`scaling_available_frequencies`...): the
    /// ceilings a `cpu-tune` module may choose.
    pub cpufreq: PathBuf,
    /// What archr-perf-probe measured in the last game (runemu.sh --measure).
    pub perf: PathBuf,
    /// The library's paths: the systems `rom.install` writes to (API 4).
    pub library: Option<crate::paths::Paths>,
}
impl Config {
    /// The device's sandbox, or the plain runner for a fixture root.
    pub fn production(paths: &crate::paths::Paths, language: &str) -> Self {
        let dirs = Dirs::new(&paths.store);
        let shop = store::Shop::production(paths.cache.join("store/modules"), dirs.clone())
            .with_systems(paths.systems.clone());
        let (executor, direct, userdb): (Arc<dyn Executor>, _, _) = if paths.root.is_some() {
            log::warn!("Modules run without a sandbox (fixture root)");
            let direct = Arc::new(Direct::default());
            (direct.clone(), Some(direct), None)
        } else {
            let systemd =
                process::Systemd::new("/usr/bin/systemd-run".into(), "/usr/bin/systemctl".into());
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
            bin: paths.at("/usr/bin"),
            shop: Some(shop),
            cpufreq: paths.at("/sys/devices/system/cpu/cpufreq/policy0"),
            perf: paths.at("/run/archr/perf-last.json"),
            library: Some(paths.clone()),
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
    /// A module asks to open this game, as if the user chose it; `owned`:
    /// a port it installed itself (API 4), which may open too.
    Launch {
        system: String,
        game_key: String,
        owned: bool,
    },
    /// What the open app sent since the last publish (API 4).
    Screens(String, Vec<archr_ui::apps::Message>),
    /// The app's process is gone: the screens close it.
    AppClosed(String),
    /// A module installed or removed a game here: the library reads it again.
    Installed(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The module with this name was switched off after failing.
    Disabled(String),
    /// A module turned the Wi-Fi on or off, or took a screenshot: its name
    /// and the locale key that says which.
    System(String, &'static str),
    /// Its update failed and the previous version is back.
    RolledBack(String),
    /// Another daemon would pass the limit.
    TooMany(String),
    /// Updated on its own: name and version.
    Updated(String, String),
    /// `ui.notify` text.
    Text(String),
}

/// What the modules chose for one launch, and whether runemu.sh measures it
/// for them (API 3). Only this launch: the user's settings stay.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tune {
    /// One of `manifest::GOVERNORS`.
    pub governor: Option<String>,
    /// One of the CPU's own frequencies, in kHz.
    pub cpu_max_khz: Option<u32>,
    pub measure: bool,
}
/// Where the choice for one launch goes.
type Governor = mpsc::Sender<Tune>;

enum Input {
    Request(Request),
    Language(String),
    Focus(&'static str, Json),
    GameStarted(Json),
    /// Settings are open: something private may be on the screen.
    Private(bool),
    /// A `system.call` ended: module, its process then, request id, success.
    System {
        id: String,
        generation: u64,
        request: Json,
        name: &'static str,
        ok: bool,
    },
    /// `launch.before` to the modules with the hook; their choice comes back.
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
    /// A `rom.*` or `port.*` job ended: the process that asked, its request,
    /// and what to record.
    Job {
        key: String,
        generation: u64,
        request: Json,
        done: Done,
    },
    /// A PortMaster script said how far it is.
    PortProgress {
        key: String,
        generation: u64,
        name: String,
        progress: u8,
    },
    /// A reconciliation's result: the modules whose badge holds, by id with
    /// the package digest and content it held for.
    /// The store kept a new reviewed list: every badge is checked again.
    ReviewedList,
    /// The last carries when the list they hold by stops counting.
    Reviewed(u64, BTreeSet<Badge>, u64),
}
/// Module id, package digest, content hash: the badge holds for exactly
/// these.
type Badge = (String, String, String);
/// What an install job did.
enum Done {
    Rom(Result<app::Game, (i64, &'static str)>),
    Port {
        name: String,
        install: bool,
        /// The game key of its launch script, when one came.
        result: Result<Option<String>, String>,
    },
}

/// The event loop's handle on the host thread.
pub struct Modules {
    tx: mpsc::Sender<Input>,
    private: bool,
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
            private: false,
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
    /// Whether Settings are open (passwords, keys, the network): no module
    /// screenshot then. Sent only when it changes.
    pub fn private(&mut self, private: bool) {
        if self.private != private {
            self.private = private;
            let _ = self.tx.send(Input::Private(private));
        }
    }
    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
    /// A game is about to take the screen: ask the modules with the launch
    /// hook (`BEFORE` at most), tell every module, then stop them all.
    /// Returns once nothing runs (500 ms for daemons, then killed), with
    /// what the modules chose for this launch. `game` holds `system`,
    /// `game_key` and `name`, and for an emulated game `system_name`,
    /// `emulator` and `core`; netplay and Moonlight have no system.
    pub fn hand_off(&self, game: Json) -> Tune {
        let mut tune = Tune::default();
        if game["system"].as_str().is_some_and(|s| !s.is_empty()) {
            let (tx, rx) = mpsc::channel();
            if self.tx.send(Input::Before(game.clone(), tx)).is_ok() {
                tune = rx
                    .recv_timeout(BEFORE + Duration::from_millis(200))
                    .unwrap_or_default();
            }
        }
        let _ = self.tx.send(Input::GameStarted(game));
        let (ack, done) = mpsc::channel();
        if self.tx.send(Input::Stop(ack)).is_ok() {
            let _ = done.recv_timeout(Duration::from_secs(5));
        }
        tune
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
    /// The store's language, shared with its lane.
    store_language: Option<Arc<std::sync::RwLock<String>>>,
    /// The key the reviewed list must be signed with.
    review_key: [u8; 32],
    /// The last reconciliation, and which of its answers counts.
    reviewed: BTreeSet<Badge>,
    reconciling: u64,
    /// When the list behind `reviewed` expires (seconds since 1970): the
    /// badges go then, with the launcher open or not.
    reviewed_until: u64,
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
    /// Each module's last `config.set`, written or not.
    config_asked: BTreeMap<String, Instant>,
    /// Each module's last `system.call` of each action.
    system_at: BTreeMap<(String, &'static str), Instant>,
    /// The same, for all modules together.
    system_all: BTreeMap<&'static str, Instant>,
    /// Actions running now: one at a time each.
    system_running: BTreeSet<&'static str>,
    /// Settings are open: no screenshot.
    private: bool,
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
    /// A launch waiting for `launch.before`: where the answer goes and what
    /// each module chose.
    before: Option<(Governor, BTreeMap<String, Tune>)>,
    /// The open app's module (API 4), what it sent for the screens since
    /// the last publish, and the icons its screens named.
    app: Option<String>,
    screens: Vec<archr_ui::apps::Message>,
    icons: BTreeMap<(String, String), Option<Arc<str>>>,
    /// When the user last said yes to each app's question.
    answered: BTreeMap<String, Instant>,
    /// Each module's last `rom.install`.
    installed_at: BTreeMap<String, Instant>,
    /// A game is being copied, or a port installed (its stop switch).
    copying: bool,
    porting: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// The systems' full names, for the permission lines.
    names: BTreeMap<String, String>,
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
        let review_key = config
            .shop
            .as_ref()
            .map_or(store::REVIEW_KEY, store::Shop::review_key);
        let language = config.shop.as_ref().map(|shop| {
            let language = shop.language();
            *language.write().unwrap_or_else(|e| e.into_inner()) = config.language.clone();
            language
        });
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
            store_language: language,
            review_key,
            reviewed: BTreeSet::new(),
            reconciling: 0,
            reviewed_until: 0,
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
            config_asked: BTreeMap::new(),
            system_at: BTreeMap::new(),
            system_all: BTreeMap::new(),
            system_running: BTreeSet::new(),
            private: false,
            gestures: BTreeMap::new(),
            late: BTreeMap::new(),
            auto_at: Instant::now() + config.delay.max(config.auto_delay),
            auto_queue: std::collections::VecDeque::new(),
            auto_busy: false,
            before: None,
            app: None,
            screens: Vec::new(),
            icons: BTreeMap::new(),
            answered: BTreeMap::new(),
            installed_at: BTreeMap::new(),
            copying: false,
            porting: None,
            names: BTreeMap::new(),
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
        // The probe's summary, only of this very launch.
        let perf = archr_io::read_bytes(&self.config.perf, 4096, archr_io::Symlinks::Reject)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Json>(&bytes).ok());
        let _ = fs::remove_file(&self.config.perf);
        let mut events = vec![("launcher.started", json!({"cold": ended.is_none()}))];
        if let Some(last) = ended {
            let started = last["started"].as_u64().unwrap_or(0);
            let seconds = now_secs().saturating_sub(started);
            let mut data = json!({
                "game": last["game"],
                "exit_code": crate::handoff::read_last_exit(&self.config.run),
                "seconds": seconds,
            });
            if let Some(perf) = perf.filter(|p| {
                p["started"].as_u64().is_some_and(|at| at + 5 >= started) && p["seconds"].is_u64()
            }) {
                data["perf"] = json!({
                    "cpu_max_khz": perf["cpu_max_khz"].as_u64().unwrap_or(0),
                    "seconds": perf["seconds"],
                    "busy_p95": perf["busy_p95"].as_u64().unwrap_or(0).min(100),
                    "limit_seconds": perf["limit_seconds"].as_u64().unwrap_or(0),
                    "idle_seconds": perf["idle_seconds"].as_u64().unwrap_or(0),
                    "current_ma": perf["current_ma"].as_u64(),
                });
            }
            events.push(("game.ended", data));
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
        let catalog = self.config.library.as_ref().map(|p| p.systems.clone());
        self.names = registry::system_names(catalog.as_deref());
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
                    (&self.config.language, &self.names),
                );
                module.reviewed = self.badge(&module.id);
                if let Some(seen) = old.iter().find(|m| m.id == module.id) {
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
        self.reconcile();
    }
    /// The badge, always derived: the last reconciliation found the kept
    /// reviewed list valid and naming this package, with its files on disk
    /// unchanged, for the package and content recorded now.
    fn badge(&self, id: &str) -> bool {
        self.state.modules.get(id).is_some_and(|r| {
            self.reviewed
                .contains(&(id.to_owned(), r.digest.clone(), r.content.clone()))
        })
    }
    /// New badges: the view follows.
    fn set_reviewed(&mut self, held: BTreeSet<Badge>) {
        if held == self.reviewed {
            return;
        }
        self.reviewed = held;
        let badges: Vec<bool> = self
            .view
            .modules
            .iter()
            .map(|m| self.badge(&m.id))
            .collect();
        for (module, badge) in self.view.modules.iter_mut().zip(badges) {
            module.reviewed = badge;
        }
        self.touch();
    }
    /// Checks every store module against the kept reviewed list and its
    /// files on disk, in a worker: hashing folders is no work for this
    /// thread. The answer comes back as `Input::Reviewed`.
    fn reconcile(&mut self) {
        self.reconciling += 1;
        let generation = self.reconciling;
        let dirs = self.config.dirs.clone();
        let key = self.review_key;
        let jobs: Vec<(String, String, String, String)> = self
            .state
            .modules
            .iter()
            .filter(|(_, r)| !r.source.is_empty() && !r.digest.is_empty() && !r.content.is_empty())
            .map(|(id, r)| {
                (
                    id.clone(),
                    r.source.clone(),
                    r.digest.clone(),
                    r.content.clone(),
                )
            })
            .collect();
        let inbox = self.inbox.clone();
        let spawned = thread::Builder::new()
            .name("launcher-module-review".into())
            .spawn(move || {
                let list = if jobs.is_empty() {
                    store::Reviewed::default()
                } else {
                    store::saved_reviewed(&dirs, &key).unwrap_or_default()
                };
                let held = jobs
                    .into_iter()
                    .filter(|(id, source, digest, content)| {
                        list.modules
                            .iter()
                            .any(|(repo, d)| repo == source && store::same_digest(d, digest))
                            && registry::tree(&dirs.module(id)).as_deref() == Some(content.as_str())
                    })
                    .map(|(id, _, digest, content)| (id, digest, content))
                    .collect();
                let _ = inbox.send(Input::Reviewed(generation, held, list.expires));
            });
        if let Err(error) = spawned {
            log::warn!("Cannot check the reviewed modules: {error}");
        }
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
        if let (Some(id), false) = (&self.app, self.screens.is_empty()) {
            let screens = std::mem::take(&mut self.screens);
            let _ = self.out.send(Event::Screens(id.clone(), screens));
        }
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
            Input::ReviewedList => self.reconcile(),
            Input::Reviewed(generation, held, until) => {
                if generation == self.reconciling {
                    self.reviewed_until = until;
                    self.set_reviewed(held);
                }
            }
            Input::Language(language) => {
                // The store speaks it too, from its next answer on.
                if let Some(shop) = &self.store_language {
                    *shop.write().unwrap_or_else(|e| e.into_inner()) = language.clone();
                }
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
            Input::Private(private) => self.private = private,
            Input::System {
                id,
                generation,
                request,
                name,
                ok,
            } => {
                self.system_running.remove(name);
                let reply = if ok {
                    json!({"jsonrpc": "2.0", "id": request, "result": {}})
                } else {
                    log::warn!("Module {id}: the system could not do {name}");
                    json!({"jsonrpc": "2.0", "id": request,
                        "error": {"code": -32000, "message": "failed"}})
                };
                // Only to the process that asked: a restarted one never
                // gets its predecessor's answer.
                if self
                    .daemons
                    .get(&id)
                    .is_some_and(|d| d.generation == generation)
                {
                    self.send(&id, &reply);
                }
                // The user sees who cut the Wi-Fi or took the picture.
                let said = match name {
                    "wifi" => Some("modules.did_wifi"),
                    "screenshot" => Some("modules.did_screenshot"),
                    _ => None,
                };
                if let (true, Some(key)) = (ok, said) {
                    let name = self.name(module_id(&id));
                    let _ = self.out.send(Event::Notice(Notice::System(name, key)));
                }
            }
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
            Input::Job {
                key,
                generation,
                request,
                done,
            } => self.job_done(&key, generation, &request, done),
            Input::PortProgress {
                key,
                generation,
                name,
                progress,
            } => {
                if self
                    .daemons
                    .get(&key)
                    .is_some_and(|d| d.generation == generation)
                {
                    let data = json!({"name": name, "progress": progress});
                    self.send(&key, &notice("port.progress", &data));
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
            Request::OpenApp(id) => self.open_app(&id),
            Request::CloseApp(id) => {
                if self.app.as_deref() == Some(id.as_str()) {
                    self.close_app();
                }
            }
            Request::AppEvent { module, name, data } => {
                if name == "screen.answer" && data["yes"] == true {
                    self.answered.insert(module.clone(), Instant::now());
                }
                let key = app_key(&module);
                if self.daemons.get(&key).is_some_and(|d| d.ready) {
                    self.send(&key, &notice(name, &data));
                }
            }
        }
    }
    fn invoke(&mut self, id: &str, action: &str, call: Option<Call>) {
        let Some(mode) = self.active(id).map(|(_, m)| m.mode) else {
            return;
        };
        // From an app screen, or from its pages while only the app runs.
        let app = app_key(id);
        let main = self.daemons.get(id).is_some_and(|d| d.ready);
        let to_app = call.as_ref().is_some_and(|c| c.screen.is_some())
            || (!main && self.daemons.get(&app).is_some_and(|d| d.ready));
        let key = if to_app { app } else { id.to_owned() };
        let context = call.map_or(json!({}), |c| match (c.screen, c.item) {
            (Some(screen), item) => {
                let mut context = json!({"screen": screen});
                if let Some(item) = item {
                    context["item"] = item.into();
                }
                if let Some(button) = c.button {
                    context["button"] = button.into();
                }
                context
            }
            (None, Some(item)) => json!({"item": item}),
            (None, None) => json!({"system": c.system, "game_key": c.game_key, "rom": c.rom}),
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
            Mode::Daemon if self.daemons.get(&key).is_some_and(|d| d.ready) => {
                self.pending.insert(
                    request_id,
                    (
                        key.clone(),
                        Waiting::Invoke(action.to_owned()),
                        Instant::now() + INVOKE,
                    ),
                );
                self.send(&key, &message);
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
        let data = json!({"key": key, "value": json[key]});
        for process in [id.to_owned(), app_key(id)] {
            if self.daemons.get(&process).is_some_and(|d| d.ready) {
                self.send(&process, &self.event_for(id, "settings.changed", &data));
            }
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
            .is_some_and(permanent);
        let running = self
            .view
            .modules
            .iter()
            .filter(|m| m.active() && m.id != id)
            .filter(|m| {
                self.find(&m.id)
                    .and_then(|i| i.manifest.as_ref())
                    .is_some_and(permanent)
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
            let _ = fs::remove_dir_all(pictures(dirs, id));
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
        record.previous_content = std::mem::replace(&mut record.content, installed.content);
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
            .is_some_and(permanent);
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
        record.previous_content = std::mem::take(&mut record.content);
        record.version = installed.version.clone();
        record.digest = installed.digest;
        record.content = installed.content;
        record.auto_updated = now_secs();
        self.save_state();
        self.end_daemon(&id, false);
        self.failures.remove(&id);
        self.reload();
        let daemon = self
            .find(&id)
            .and_then(|i| i.manifest.as_ref())
            .is_some_and(permanent);
        if daemon && self.started && !self.stopped && self.active(&id).is_some() {
            self.spawn_daemon(&id);
        }
        let notice = Notice::Updated(self.name(&id), installed.version);
        let _ = self.out.send(Event::Notice(notice));
    }
    /// Store cards tell which installed modules are outdated; a listing
    /// checks every badge again. (A new reviewed list also comes as
    /// `Input::ReviewedList`, from any answer.)
    fn store_reply(&mut self, reply: &StoreReply) {
        let cards = match reply {
            // The Installed tab comes from the disk: it knows nothing new.
            StoreReply::Listed { tab, .. } if *tab == archr_ui::modules::Tab::Installed => return,
            StoreReply::Listed { cards, .. } => {
                // Refreshing a list checks the files on disk again too,
                // even when the reviewed list did not change.
                self.reconcile();
                cards.as_slice()
            }
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
                module.update = archr_ui::settings::store::newer(&card.version, &module.version)
                    .then(|| card.version.clone());
            }
        }
        self.touch();
    }

    // ---- processes ----

    /// How process `key` of a module runs: its daemon or one-shot, or its
    /// app (`<id>_app`).
    fn plan(&self, key: &str, unit: String) -> Option<Plan> {
        let id = module_id(key);
        let app = key != id;
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
            (
                "ARCHR_MODULE_ROLE",
                if app { "app" } else { "daemon" }.into(),
            ),
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
            memory_mb: if app { APP_MEMORY } else { manifest.memory_mb },
            tasks: if app { APP_TASKS } else { 16 },
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
    /// Starts a module's daemon, or its app with `key` = `app_key(id)`:
    /// one app at a time, outside the daemons' limit.
    fn spawn_daemon(&mut self, key: &str) {
        let id = module_id(key).to_owned();
        let app = key != id;
        let daemons = self.daemons.keys().filter(|k| module_id(k) == *k).count();
        if self.daemons.contains_key(key) || (!app && daemons >= DAEMONS) || !self.prepare(&id) {
            return;
        }
        let id = id.as_str();
        self.ran(id);
        let Some(plan) = self.plan(key, format!("archr-mod-{key}")) else {
            return;
        };
        self.next += 1;
        let generation = self.next;
        let inbox = self.inbox.clone();
        let owner = key.to_owned();
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
                log::warn!("Module {key} cannot start: {error}");
                self.punish(id, "cannot start");
                return;
            }
        };
        self.daemons.insert(
            key.to_owned(),
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
                key.to_owned(),
                Waiting::Initialize,
                Instant::now() + INITIALIZE,
            ),
        );
        let mut capabilities = vec![
            "ui.update",
            "ui.notify",
            "settings.set",
            "config.get",
            "config.set",
            "game.launch",
            "launch.before",
            "system.call",
            "log",
        ];
        if app {
            capabilities.extend([
                "screen",
                "rom.install",
                "rom.list",
                "rom.remove",
                "port.install",
                "port.list",
                "port.remove",
            ]);
        }
        let mut params = json!({
            "api_version": manifest::API_VERSION,
            "launcher_version": env!("CARGO_PKG_VERSION"),
            "language": self.config.language,
            "device": self.config.device,
            "settings": settings,
            "permissions": permissions,
            "capabilities": capabilities,
            "role": if app { "app" } else { "daemon" },
        });
        if permissions.iter().any(|p| p == "system:cpu-tune") {
            let opps = self.cpu_opps();
            params["cpu"] = json!({"opps": opps, "max_khz": opps.last()});
        }
        self.send(
            key,
            &json!({"jsonrpc": "2.0", "id": request, "method": "initialize", "params": params}),
        );
    }
    /// The user opened an app's row: its process starts and is told to
    /// show its first screen once it answered `initialize`.
    fn open_app(&mut self, id: &str) {
        let has_app = self.active(id).is_some_and(|(_, m)| m.app.is_some());
        if self.stopped || !has_app {
            let _ = self.out.send(Event::AppClosed(id.to_owned()));
            return;
        }
        if self.app.as_deref().is_some_and(|open| open != id) {
            self.close_app();
        }
        self.app = Some(id.to_owned());
        self.screens.clear();
        self.answered.remove(id);
        // The last session's pictures go; this one copies what it shows.
        let _ = fs::remove_dir_all(pictures(&self.config.dirs, id));
        self.spawn_daemon(&app_key(id));
        if !self.daemons.contains_key(&app_key(id)) {
            self.app = None;
            let _ = self.out.send(Event::AppClosed(id.to_owned()));
        }
    }
    /// Leaving the app: `shutdown`, 500 ms, then it is killed. Its unsent
    /// screens go with it.
    fn close_app(&mut self) {
        let Some(id) = self.app.take() else { return };
        self.screens.clear();
        let key = app_key(&id);
        self.next += 1;
        let request = self.next;
        self.pending
            .insert(request, (key.clone(), Waiting::Shutdown, Instant::now()));
        self.send(
            &key,
            &json!({"jsonrpc": "2.0", "id": request, "method": "shutdown", "params": {}}),
        );
        let deadline = Instant::now() + SHUTDOWN;
        while Instant::now() < deadline
            && self
                .daemons
                .get_mut(&key)
                .is_some_and(|d| matches!(d.child.try_wait(), Ok(None)))
        {
            thread::sleep(Duration::from_millis(10));
        }
        self.end_daemon(&key, false);
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
    /// Ends a process now; `failed` counts it against the module. A
    /// module's daemon goes with its app.
    fn end_daemon(&mut self, key: &str, failed: bool) {
        self.pending.retain(|_, p| p.0 != key);
        if let Some(mut daemon) = self.daemons.remove(key) {
            self.config.executor.kill(Some(&format!("archr-mod-{key}")));
            let _ = daemon.child.kill();
            let _ = daemon.child.wait();
        }
        let id = module_id(key);
        if key == id && self.daemons.contains_key(&app_key(id)) {
            self.end_daemon(&app_key(id), false);
        }
        if key != id {
            self.app_gone(id);
        }
        if failed {
            self.punish(key, "broke the protocol");
        }
    }
    /// The app's process is over: the screens close it.
    fn app_gone(&mut self, id: &str) {
        if self.app.as_deref() == Some(id) {
            self.app = None;
            self.screens.clear();
            let _ = self.out.send(Event::AppClosed(id.to_owned()));
        }
    }
    fn daemon_exited(&mut self, key: &str) {
        let Some(mut daemon) = self.daemons.remove(key) else {
            return;
        };
        self.pending.retain(|_, p| p.0 != key);
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
        let id = module_id(key);
        if key != id {
            // Apps are never started again on their own.
            self.app_gone(id);
            if code != Some(0) {
                self.punish(id, "app exited with an error");
            }
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
        if let Some(id) = self.app.clone() {
            self.app_gone(&id);
        }
        // A PortMaster install never runs under a game.
        if let Some(stop) = self.porting.take() {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
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
            (Some(method @ ("rom.install" | "port.install" | "port.remove")), Some(request))
                if !oneshot && module_id(id) != id =>
            {
                if let Err((code, text)) = self.job(id, method, request, params) {
                    let reply = json!({"jsonrpc": "2.0", "id": request,
                        "error": {"code": code, "message": text}});
                    self.send(id, &reply);
                }
            }
            (Some("system.call"), Some(request)) if !oneshot => {
                if let Err((code, text)) = self.system_call(id, request, params) {
                    let reply = json!({"jsonrpc": "2.0", "id": request,
                        "error": {"code": code, "message": text}});
                    self.send(id, &reply);
                }
            }
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
                self.commit(module_id(id));
                if module_id(id) != id {
                    // The app shows its first screen now.
                    let home = self.active(module_id(id)).and_then(|(_, m)| m.app.clone());
                    if let Some(app) = home {
                        self.send(id, &notice("screen.opened", &json!({"screen": app.home})));
                    }
                    return;
                }
                if self.greeted.insert(id.to_owned()) {
                    let subscribed = self.subscribed(id);
                    for (name, data) in self.boot.clone() {
                        if subscribed.contains(name) {
                            self.send(id, &self.event_for(id, name, &data));
                        }
                    }
                }
            }
            Waiting::Initialize => self.end_daemon(id, true),
            Waiting::Invoke(action) => {
                self.view.busy.remove(&(module_id(id).to_owned(), action));
                self.touch();
            }
            Waiting::Shutdown => {}
            Waiting::Before => {
                self.late.remove(id);
                self.decision(id, &message["result"]);
            }
        }
    }
    fn notification(&mut self, key: &str, method: &str, params: &Json) {
        let id = module_id(key);
        let Some(installed) = self.installed.iter().find(|i| i.id == id) else {
            return;
        };
        match method {
            method if method.starts_with("screen.") => {
                if key == id || self.app.as_deref() != Some(id) {
                    return;
                }
                if self.screens.len() >= SCREENS {
                    log::warn!("Module {id}: too many screen messages, some dropped");
                    return;
                }
                let home = self.config.dirs.data_dir(id);
                let icons = &mut self.icons;
                let mut icon = |name: &str| -> Option<Arc<str>> {
                    let found = icons
                        .entry((id.to_owned(), name.to_owned()))
                        .or_insert_with(|| {
                            let path = installed.dir.join("icons").join(format!("{name}.svg"));
                            archr_io::read_utf8(&path, 16 << 10, archr_io::Symlinks::Reject)
                                .ok()
                                .filter(|svg| manifest::lucide(svg.as_bytes()))
                                .map(Into::into)
                        });
                    found.clone()
                };
                let store = pictures(&self.config.dirs, id);
                let mut reader = app::Reader {
                    installed,
                    home: &home,
                    store: &store,
                    icon: &mut icon,
                };
                match reader.message(method, params) {
                    Some(message) => {
                        self.screens.push(message);
                        self.touch();
                    }
                    None => log::warn!("Module {id}: {method} refused"),
                }
            }
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
                            .map(|item| {
                                let short = |field: &str| {
                                    item[field]
                                        .as_str()
                                        .filter(|t| {
                                            (1..=40).contains(&t.len())
                                                && !t.chars().any(char::is_control)
                                        })
                                        .map(str::to_owned)
                                };
                                let id = short("id").unwrap_or_default();
                                let game = item.get("game").and_then(|g| {
                                    let field = |f: &str| {
                                        g[f].as_str()
                                            .filter(|t| (1..=256).contains(&t.len()))
                                            .map(str::to_owned)
                                    };
                                    Some((field("system")?, field("game_key")?))
                                });
                                archr_ui::modules::Item {
                                    title: text(item, "title"),
                                    subtitle: text(item, "subtitle"),
                                    value: text(item, "value"),
                                    action: short("action").filter(|_| !id.is_empty()),
                                    id,
                                    detail: text(item, "detail"),
                                    badge: text(item, "badge"),
                                    tone: archr_ui::modules::Tone::parse(
                                        item["tone"].as_str().unwrap_or(""),
                                    ),
                                    progress: item["progress"]
                                        .as_f64()
                                        .map(|p| p.clamp(0., 100.).round() as u8),
                                    game,
                                    stay: item["open"] == false,
                                }
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
    fn call(
        &mut self,
        process: &str,
        method: &str,
        params: &Json,
    ) -> Result<Json, (i64, &'static str)> {
        let id = module_id(process);
        let app = process != id;
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
                // One request a second, before system.cfg is even read.
                let now = Instant::now();
                let last = self.config_asked.insert(id.to_owned(), now);
                if last.is_some_and(|at| now.duration_since(at) < CONFIG_SET) {
                    return Err((-32003, "too many requests"));
                }
                let (path, lock) = (&self.config.system_cfg, &self.config.lock);
                let current = archr_config::SystemCfg::load(path)
                    .ok()
                    .and_then(|cfg| cfg.get_setting(key, None, None));
                if current.as_deref() == Some(value) {
                    // Nothing to write; it does not count against the hour.
                    return Ok(json!({}));
                }
                let times = self.written.entry(id.to_owned()).or_default();
                times.retain(|at| at.elapsed() < HOUR);
                if times.len() >= CONFIG_HOURLY {
                    return Err((-32003, "too many requests"));
                }
                times.push(now);
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
                let ledger = app::Ledger::load(&self.config.dirs.data.join(app::LEDGER));
                let owned = system == app::PORTS
                    && ledger
                        .modules
                        .get(id)
                        .is_some_and(|o| o.ports.iter().any(|p| p.game_key == game_key));
                let _ = self.out.send(Event::Launch {
                    system,
                    game_key,
                    owned,
                });
                Ok(json!({}))
            }
            "rom.list" => {
                let ledger = app::Ledger::load(&self.config.dirs.data.join(app::LEDGER));
                let owned = ledger.modules.get(id).cloned().unwrap_or_default();
                let (games, used) = owned.present();
                let games: Vec<Json> = games
                    .iter()
                    .map(|g| {
                        json!({"game_key": g.game_key, "system": g.system,
                            "size": g.size, "date": g.installed})
                    })
                    .collect();
                Ok(json!({"games": games, "used": used, "limit": app::MODULE_BYTES}))
            }
            "port.list" => {
                let ledger = app::Ledger::load(&self.config.dirs.data.join(app::LEDGER));
                let ports: Vec<Json> = ledger
                    .modules
                    .get(id)
                    .into_iter()
                    .flat_map(|o| &o.ports)
                    .map(|p| json!({"name": p.name, "game_key": p.game_key, "date": p.installed}))
                    .collect();
                Ok(json!({"ports": ports}))
            }
            "rom.install" | "rom.remove" | "port.install" | "port.remove" if !app => {
                Err((-32001, "only from the app"))
            }
            "rom.remove" => {
                let game_key = params["game_key"].as_str().ok_or((-32602, "game_key"))?;
                let path = self.config.dirs.data.join(app::LEDGER);
                let mut ledger = app::Ledger::load(&path);
                let owned = ledger.modules.entry(id.to_owned()).or_default();
                let at = owned
                    .games
                    .iter()
                    .position(|g| g.game_key == game_key)
                    .ok_or((-32001, "not installed by this module"))?;
                self.confirmed(id)?;
                let game = owned.games.remove(at);
                app::remove(&game, &self.systems());
                ledger.save(&path).map_err(|_| (-32000, "cannot save"))?;
                log::info!("Module {id} removed {game_key}");
                let _ = self.out.send(Event::Installed(game.system));
                Ok(json!({}))
            }
            _ => Err((-32601, "method not found")),
        }
    }
    /// The systems as the library reads them: where games go.
    fn systems(&self) -> Vec<archr_index::SystemDef> {
        self.config
            .library
            .as_ref()
            .and_then(|paths| crate::worker::definitions(paths).ok())
            .unwrap_or_default()
    }
    /// A removal only right after the user said yes to the app, once.
    fn confirmed(&mut self, id: &str) -> Result<(), (i64, &'static str)> {
        match self.answered.remove(id) {
            Some(at) if at.elapsed() < CONFIRMED => Ok(()),
            _ => Err((-32001, "only after the user confirmed")),
        }
    }
    /// `rom.install`, `port.install` and `port.remove`, from the app:
    /// checked here, done in a worker, answered when they end. One copy and
    /// one port at a time.
    fn job(
        &mut self,
        key: &str,
        method: &str,
        request: &Json,
        params: &Json,
    ) -> Result<(), (i64, &'static str)> {
        let id = module_id(key).to_owned();
        let allowed = self
            .active(&id)
            .map(|(_, m)| m.permissions.install.clone())
            .ok_or((-32004, "not found"))?;
        let ledger = app::Ledger::load(&self.config.dirs.data.join(app::LEDGER));
        let owned = ledger.modules.get(&id).cloned().unwrap_or_default();
        let generation = self.daemons.get(key).map_or(0, |d| d.generation);
        let (inbox, request, process) = (self.inbox.clone(), request.clone(), key.to_owned());
        let finish = {
            let inbox = inbox.clone();
            let process = process.clone();
            move |done: Done| {
                let _ = inbox.send(Input::Job {
                    key: process,
                    generation,
                    request,
                    done,
                });
            }
        };
        if method == "rom.install" {
            // What is asked is checked first; only a sound request counts.
            let (systems, home) = (self.systems(), self.config.dirs.data_dir(&id));
            let copy = app::check(&home, params, &allowed, &systems, owned.present().1)?;
            if self.copying {
                return Err((-32003, "another install is running"));
            }
            let now = Instant::now();
            let last = self.installed_at.insert(id.clone(), now);
            if last.is_some_and(|at| now.duration_since(at) < INSTALL_EVERY) {
                return Err((-32003, "too many requests"));
            }
            self.copying = true;
            thread::spawn(move || finish(Done::Rom(copy.run())));
            return Ok(());
        }
        let install = method == "port.install";
        if !allowed.iter().any(|s| s == app::PORTS) {
            return Err((-32001, "permission denied"));
        }
        let name = params["name"]
            .as_str()
            .filter(|n| app::port_ok(n))
            .ok_or((-32602, "name"))?
            .to_owned();
        if !install {
            if !owned.ports.iter().any(|p| p.name == name) {
                return Err((-32001, "not installed by this module"));
            }
            self.confirmed(&id)?;
        }
        if self.porting.is_some() {
            return Err((-32003, "another port is being installed"));
        }
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.porting = Some(stop.clone());
        let bin = self.config.bin.clone();
        let ports = self.systems().into_iter().find(|s| s.name == app::PORTS);
        log::info!("Module {id}: archr-port-install {method} {name}");
        thread::spawn(move || {
            let before = ports.as_ref().map(app::scripts).unwrap_or_default();
            let progress = {
                let (name, process) = (name.clone(), process.clone());
                move |progress: u8| {
                    let _ = inbox.send(Input::PortProgress {
                        key: process.clone(),
                        generation,
                        name: name.clone(),
                        progress,
                    });
                }
            };
            let result = app::run_port(&bin, install, &name, &stop, progress).map(|()| {
                // The launch script the install brought, for "Play".
                let def = ports.as_ref()?;
                let new = app::scripts(def)
                    .into_iter()
                    .find(|s| !before.contains(s))?;
                crate::store::game_key(def, &new)
            });
            finish(Done::Port {
                name,
                install,
                result,
            });
        });
        Ok(())
    }
    /// A job ended: what it did is recorded, the library reads the system
    /// again, and the process that asked hears how it went.
    fn job_done(&mut self, key: &str, generation: u64, request: &Json, done: Done) {
        let id = module_id(key).to_owned();
        let path = self.config.dirs.data.join(app::LEDGER);
        let mut ledger = app::Ledger::load(&path);
        let owned = ledger.modules.entry(id.clone()).or_default();
        let (result, system) = match done {
            Done::Rom(result) => {
                self.copying = false;
                match result {
                    Ok(game) => {
                        log::info!("Module {id} installed {}", game.game_key);
                        let answer = json!({"game_key": game.game_key});
                        let system = game.system.clone();
                        owned.games.push(game);
                        (Ok(answer), Some(system))
                    }
                    Err((code, text)) => (Err((code, text.to_owned())), None),
                }
            }
            Done::Port {
                name,
                install,
                result,
            } => {
                self.porting = None;
                match result {
                    Ok(game_key) => {
                        let before = owned.ports.iter().find(|p| p.name == name).cloned();
                        owned.ports.retain(|p| p.name != name);
                        // A reinstall keeps the script it found the first time.
                        let game_key = game_key.or(before.map(|p| p.game_key)).unwrap_or_default();
                        let answer = if install {
                            owned.ports.push(app::Port {
                                name,
                                game_key: game_key.clone(),
                                installed: now_secs(),
                            });
                            json!({"game_key": game_key})
                        } else {
                            json!({})
                        };
                        (Ok(answer), Some(app::PORTS.to_owned()))
                    }
                    Err(text) => {
                        log::warn!("Module {id}: archr-port-install {name}: {text}");
                        (Err((-32000, text)), None)
                    }
                }
            }
        };
        if system.is_some() {
            if let Err(error) = ledger.save(&path) {
                log::warn!("Cannot save what modules installed: {error}");
            }
        }
        if let Some(system) = system {
            let _ = self.out.send(Event::Installed(system));
        }
        if self
            .daemons
            .get(key)
            .is_some_and(|d| d.generation == generation)
        {
            let reply = match result {
                Ok(answer) => json!({"jsonrpc": "2.0", "id": request, "result": answer}),
                Err((code, text)) => json!({"jsonrpc": "2.0", "id": request,
                    "error": {"code": code, "message": text}}),
            };
            self.send(key, &reply);
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
            let message = self.event_for(&id, name, data);
            match manifest.mode {
                Mode::Daemon if self.daemons.get(&id).is_some_and(|d| d.ready) => {
                    self.send(&id, &message);
                }
                Mode::Oneshot if !name.starts_with("focus.") => {
                    self.oneshot(&id, message, None);
                }
                _ => {}
            }
        }
    }
    /// The event as this module may see it: the game's measurement only
    /// for `cpu-tune`. Every event to a module goes through here.
    fn event_for(&self, id: &str, name: &str, data: &Json) -> Json {
        let mut data = data.clone();
        if !self.has_system(id, "cpu-tune") {
            if let Some(data) = data.as_object_mut() {
                data.remove("perf");
            }
        }
        event(name, &data)
    }
    fn has_system(&self, id: &str, system: &str) -> bool {
        self.active(id)
            .is_some_and(|(_, m)| m.permissions.system.iter().any(|s| s == system))
    }
    /// The ceilings a `cpu-tune` module may choose, in kHz, lowest first:
    /// the regular ones and, with boost on, the boost ones.
    fn cpu_opps(&self) -> Vec<u32> {
        let read = |name: &str| fs::read_to_string(self.config.cpufreq.join(name));
        let boost = fs::read_to_string(self.config.cpufreq.with_file_name("boost"))
            .is_ok_and(|b| b.trim() == "1");
        let mut opps: Vec<u32> = [
            read("scaling_available_frequencies").ok(),
            boost
                .then(|| read("scaling_boost_frequencies").ok())
                .flatten(),
        ]
        .into_iter()
        .flatten()
        .flat_map(|list| {
            list.split_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect::<Vec<u32>>()
        })
        .collect();
        opps.sort_unstable();
        opps.dedup();
        opps
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

    // ---- system actions ----

    /// `system.call {name, value}`: one of the module's `system` actions,
    /// run through the code that owns it (the scripts, and the launcher's
    /// own Wi-Fi switch), in a worker, with a time limit; the answer comes
    /// when it ends. Values are checked here; the rate is bounded per module
    /// and for all modules together; one call per action at a time; nothing
    /// runs while a game has the screen (no module runs then) and no
    /// screenshot while Settings are open.
    fn system_call(
        &mut self,
        process: &str,
        request: &Json,
        params: &Json,
    ) -> Result<(), (i64, &'static str)> {
        let id = module_id(process);
        let manifest = self
            .find(id)
            .and_then(|i| i.manifest.clone())
            .ok_or((-32004, "not found"))?;
        let name = match params["name"].as_str() {
            Some("brightness") => "brightness",
            Some("volume") => "volume",
            Some("wifi") => "wifi",
            Some("screenshot") => "screenshot",
            _ => return Err((-32602, "name")),
        };
        if !manifest.permissions.system.iter().any(|s| s == name) {
            return Err((-32001, "permission denied"));
        }
        if self.stopped {
            return Err((-32001, "not while a game runs"));
        }
        let percent = |min: u64| {
            params["value"]
                .as_u64()
                .filter(|v| (min..=100).contains(v))
                .ok_or((-32602, "value"))
        };
        // Seconds between two calls of one module, and of all modules.
        let (each, all, work) = match name {
            // Never dark enough to look like the device is off.
            "brightness" => {
                let v = percent(10)?.to_string();
                let run = vec![
                    s(&["brightness", "set", &v]),
                    s(&["archr-osd", "send", "brightness", &v]),
                ];
                (1, 1, Work::Run(run))
            }
            "volume" => {
                let v = percent(0)?.to_string();
                let run = vec![s(&["volume", &v]), s(&["archr-osd", "send", "volume", &v])];
                (1, 1, Work::Run(run))
            }
            "wifi" => {
                let on = params["value"].as_bool().ok_or((-32602, "value"))?;
                (60, 60, Work::Wifi(on))
            }
            _ => {
                if self.private {
                    return Err((-32001, "not while Settings are open"));
                }
                // The user's switch for screenshots, read before the script.
                let enabled = archr_config::SystemCfg::load(&self.config.system_cfg)
                    .ok()
                    .and_then(|cfg| cfg.get_setting("archr.screenshot.enabled", None, None))
                    .is_some_and(|v| v == "1");
                if !enabled {
                    return Err((-32005, "screenshots are off in the user's settings"));
                }
                (10, 10, Work::Run(vec![s(&["archr-screenshot"])]))
            }
        };
        if self.system_running.contains(name) {
            return Err((-32003, "already running"));
        }
        let now = Instant::now();
        let soon = |at: Option<&Instant>, every: u64| {
            at.is_some_and(|at| now.duration_since(*at) < Duration::from_secs(every))
        };
        let key = (id.to_owned(), name);
        if soon(self.system_at.get(&key), each) || soon(self.system_all.get(name), all) {
            return Err((-32003, "too many requests"));
        }
        let generation = self.daemons.get(process).map_or(0, |d| d.generation);
        let (owner, request) = (process.to_owned(), request.clone());
        let inbox = self.inbox.clone();
        let done = {
            let (id, inbox) = (owner, inbox.clone());
            move |ok: bool| {
                let _ = inbox.send(Input::System {
                    id,
                    generation,
                    request,
                    name,
                    ok,
                });
            }
        };
        match work {
            // The launcher's own switch: wifi.enabled, the lock against the
            // user's press, the connection after enabling.
            Work::Wifi(on) => {
                let tool = (
                    self.config.bin.join("wifictl"),
                    self.config.system_cfg.clone(),
                    self.config.lock.clone(),
                );
                match crate::wifi::switch(tool, on, done) {
                    Ok(true) => {}
                    Ok(false) => return Err((-32003, "the Wi-Fi is switching already")),
                    Err(_) => return Err((-32000, "failed")),
                }
            }
            Work::Run(commands) => {
                let bin = self.config.bin.clone();
                thread::spawn(move || {
                    let run = |argv: &[String]| {
                        let mut command = std::process::Command::new(bin.join(&argv[0]));
                        command.args(&argv[1..]).stdin(std::process::Stdio::null());
                        std::os::unix::process::CommandExt::process_group(&mut command, 0);
                        crate::process::capture(
                            &mut command,
                            SYSTEM_TIMEOUT,
                            &std::sync::atomic::AtomicBool::new(false),
                            4096,
                        )
                        .is_ok_and(|out| out.status.success())
                    };
                    // The first command does the work; the OSD only shows it.
                    let ok = run(&commands[0]);
                    if ok {
                        for extra in &commands[1..] {
                            run(extra);
                        }
                    }
                    done(ok);
                });
            }
        }
        log::info!("Module {id} asks the system for {name}");
        self.system_at.insert((id.to_owned(), name), now);
        self.system_all.insert(name, now);
        self.system_running.insert(name);
        Ok(())
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
        let changes = &result["changes"];
        let mut tune = Tune::default();
        if let Some(g) = changes["governor"].as_str() {
            if self.has_system(id, "cpu-profile") && manifest::GOVERNORS.contains(&g) {
                tune.governor = Some(g.to_owned());
            } else {
                log::warn!("Module {id}: launch change refused (governor)");
            }
        }
        if let Some(khz) = changes.get("cpu_max_khz").filter(|v| !v.is_null()) {
            match khz.as_u64().and_then(|k| u32::try_from(k).ok()) {
                Some(k) if self.has_system(id, "cpu-tune") && self.cpu_opps().contains(&k) => {
                    tune.cpu_max_khz = Some(k);
                }
                _ => log::warn!("Module {id}: launch change refused (cpu_max_khz)"),
            }
        }
        if let Some((_, chosen)) = &mut self.before {
            chosen.insert(id.to_owned(), tune);
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
            // Each choice is the first module's (by id) that made one.
            let mut tune = Tune::default();
            for (id, choice) in chosen {
                if let Some(governor) = choice.governor {
                    if tune.governor.is_none() {
                        log::info!("Module {id} runs this game with the {governor} governor");
                        tune.governor = Some(governor);
                    } else {
                        log::info!("Module {id}: another module already chose the governor");
                    }
                }
                if let Some(khz) = choice.cpu_max_khz {
                    if tune.cpu_max_khz.is_none() {
                        log::info!("Module {id} runs this game under {khz} kHz");
                        tune.cpu_max_khz = Some(khz);
                    } else {
                        log::info!("Module {id}: another module already chose the ceiling");
                    }
                }
            }
            let ids: Vec<String> = self.view.modules.iter().map(|m| m.id.clone()).collect();
            tune.measure = ids.iter().any(|id| self.has_system(id, "cpu-tune"));
            let _ = reply.send(tune);
        }
    }

    // ---- failures ----

    fn punish(&mut self, key: &str, why: &str) {
        let id = module_id(key);
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
        record.content = std::mem::take(&mut record.previous_content);
        self.save_state();
        self.reload();
        true
    }

    fn tick(&mut self) {
        let now = Instant::now();
        // The list behind the badges ran out while the launcher is open
        // (ticks come at least every second).
        if !self.reviewed.is_empty() && now_secs() >= self.reviewed_until {
            log::warn!("The reviewed list has expired: no module is reviewed");
            self.set_reviewed(BTreeSet::new());
        }
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
                            self.oneshot(&id, self.event_for(&id, name, data), None);
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
                    && self.active(id).is_some_and(|(_, m)| permanent(m))
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
                    if module_id(&id) == id {
                        self.restarts
                            .insert(id, (now + Duration::from_secs(BACKOFF[1]), 1));
                    }
                }
                Some(Waiting::Invoke(action)) => {
                    self.view.busy.remove(&(module_id(&id).to_owned(), action));
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

/// What a `system.call` does.
enum Work {
    Run(Vec<Vec<String>>),
    Wifi(bool),
}
fn s(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}
fn event(name: &str, data: &Json) -> Json {
    json!({"jsonrpc": "2.0", "method": "event", "params": {"name": name, "data": data}})
}
/// What an app hears about its screens: a notification of its own.
fn notice(method: &str, params: &Json) -> Json {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A daemon that runs while the launcher is on screen. An app-only module
/// (API 4) runs just while its app is open, unless it also has something
/// live elsewhere: Start entries, Y items or hooks.
pub(crate) fn permanent(m: &manifest::Manifest) -> bool {
    m.mode == Mode::Daemon
        && (m.app.is_none()
            || !(m.tiles.is_empty()
                && m.shortcuts.is_empty()
                && m.widgets.is_empty()
                && m.quick.is_empty()
                && m.hooks.is_empty()
                && m.permissions.hooks.is_empty()))
}

/// `--check-module <dir>`: the problems, one per line; `Ok` when none.
pub fn check_module(dir: &std::path::Path) -> Result<Vec<String>, Vec<String>> {
    manifest::check(dir).map(|(_, warnings)| warnings)
}
