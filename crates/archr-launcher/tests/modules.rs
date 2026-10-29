//! The module host against fake modules written in shell, run without a
//! sandbox by the `Direct` executor: protocol, time limits, repeated
//! failures, permissions and the stop before a game.
use archr_launcher::modules::{
    process::{self, Direct, Executor, Plan},
    registry::{Dirs, Record, State},
    Config, Event, Modules, Notice,
};
use archr_ui::modules::{Request, Status, View};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// A module folder: module.toml, an English catalog and main.sh.
fn module(base: &Path, id: &str, mode: &str, extra: &str, script: &str) {
    let dir = base.join("modules").join(id);
    fs::create_dir_all(dir.join("locales")).unwrap();
    fs::write(
        dir.join("module.toml"),
        format!(
            "id = \"{id}\"\nversion = \"1.0.0\"\napi_version = 1\nname = \"@name\"\n\
             [run]\nruntime = \"bash\"\nentry = \"main.sh\"\nmode = \"{mode}\"\n{extra}"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("locales/en.toml"),
        "\"name\" = \"Test {id}\"\n\"hello\" = \"Hello {who}\"\n".replace("{id}", id),
    )
    .unwrap();
    fs::write(dir.join("main.sh"), script).unwrap();
}
/// Consent given and switched on, as after the permission page.
fn allow(base: &Path, ids: &[(&str, &[&str])]) {
    let mut state = State::default();
    for (id, granted) in ids {
        state.modules.insert(
            (*id).into(),
            Record {
                consent: true,
                enabled: true,
                granted: granted.iter().map(|g| (*g).into()).collect(),
                ..Record::default()
            },
        );
    }
    state.save(&base.join("modules.toml")).unwrap();
}
fn start(base: &Path) -> Modules {
    let direct = Arc::new(Direct::default());
    start_with(base, direct.clone(), Some(direct), None)
}
fn start_with(
    base: &Path,
    executor: Arc<dyn Executor>,
    direct: Option<Arc<Direct>>,
    userdb: Option<std::path::PathBuf>,
) -> Modules {
    Modules::start(Config {
        dirs: Dirs::new(base),
        run: base.join("run"),
        system_cfg: base.join("system.cfg"),
        executor,
        direct,
        userdb,
        language: "en".into(),
        device: "test".into(),
        delay: Duration::ZERO,
        shop: None,
    })
}
/// The host's handle with what it published so far.
struct Probe {
    modules: Modules,
    view: Arc<View>,
    notices: Vec<Notice>,
}
impl Probe {
    fn new(base: &Path) -> Self {
        Self {
            modules: start(base),
            view: Arc::default(),
            notices: Vec::new(),
        }
    }
    /// Polls until `done` holds for the latest view.
    fn wait(&mut self, done: impl Fn(&View, &[Notice]) -> bool) -> Arc<View> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            for event in self.modules.poll() {
                match event {
                    Event::View(view) => self.view = view,
                    Event::Notice(notice) => self.notices.push(notice),
                    Event::Store(_) => {}
                }
            }
            if done(&self.view, &self.notices) {
                return self.view.clone();
            }
            assert!(
                Instant::now() < deadline,
                "timed out: {:?} {:?}",
                self.view,
                self.notices
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn request(&self, request: Request) {
        self.modules.request(request);
    }
    fn invoke(&self, module: &str, action: &str) {
        self.request(Request::Invoke {
            module: module.into(),
            action: action.into(),
            call: None,
        });
    }
}
fn value<'a>(view: &'a View, id: &str, target: &str) -> Option<&'a str> {
    view.value(id, target)
}

/// A bash daemon: answers initialize and shutdown, logs every line it gets
/// in its data folder and replies to actions with the protocol under test.
const DAEMON: &str = r#"#!/bin/bash
log="$HOME/seen.log"
echo $$ > "$HOME/pid"
send() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    printf '%s\n' "$line" >> "$log"
    id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    case "$line" in
    *'"method":"initialize"'*)
        send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"ready\":true}}"
        send '{"jsonrpc":"2.0","method":"ui.update","params":{"target":"tile:t","value":{"key":"@hello","args":{"who":"R36S"}}}}' ;;
    *'"action":"ask"'*)
        send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"
        send '{"jsonrpc":"2.0","id":"c1","method":"config.get","params":{"key":"system.language"}}'
        send '{"jsonrpc":"2.0","id":"c2","method":"config.get","params":{"key":"system.timezone"}}'
        send '{"jsonrpc":"2.0","id":"s1","method":"settings.set","params":{"key":"watts","value":false}}'
        send '{"jsonrpc":"2.0","id":"s2","method":"settings.set","params":{"key":"other","value":"x"}}' ;;
    *'"id":"c1"'*) case "$line" in *'"result"'*)
        send '{"jsonrpc":"2.0","method":"ui.update","params":{"target":"row:lang","value":"granted"}}' ;; esac ;;
    *'"id":"c2"'*) case "$line" in *-32001*)
        send '{"jsonrpc":"2.0","method":"ui.update","params":{"target":"row:tz","value":"denied"}}' ;; esac ;;
    *'"id":"s2"'*) case "$line" in *-32001*)
        send '{"jsonrpc":"2.0","method":"ui.notify","params":{"text":"undeclared refused","level":"info"}}' ;; esac ;;
    *'"action":"garbage"'*) send 'not json at all' ;;
    *'"method":"shutdown"'*) send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"; exit 0 ;;
    esac
done
"#;
const PAGE: &str = "[permissions]\nconfig_read = [\"system.language\"]\n\
    [[hooks]]\nevent = \"game.started\"\n[[hooks]]\nevent = \"game.ended\"\n\
    [[hooks]]\nevent = \"focus.system\"\n\
    [[start.tiles]]\nid = \"t\"\nlabel = \"@name\"\naction = \"ask\"\n\
    [[settings.pages]]\nid = \"main\"\n\
    [[settings.pages.rows]]\ntype = \"toggle\"\nkey = \"watts\"\ndefault = true\n";

#[test]
fn a_daemon_speaks_the_protocol_and_only_gets_what_it_was_granted() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    module(base, "bateria", "daemon", PAGE, DAEMON);
    allow(base, &[("bateria", &["config_read:system.language"])]);
    fs::write(
        base.join("system.cfg"),
        "system.language=pt_BR\nsystem.timezone=UTC\n",
    )
    .unwrap();
    let mut probe = Probe::new(base);
    let view = probe.wait(|v, _| value(v, "bateria", "tile:t").is_some());
    assert_eq!(value(&view, "bateria", "tile:t"), Some("Hello R36S"));
    assert_eq!(view.tiles().len(), 1);
    let settings = base.join("module-settings/bateria.json");
    assert_eq!(fs::read_to_string(&settings).unwrap(), r#"{"watts":true}"#);
    probe.invoke("bateria", "ask");
    let view = probe.wait(|v, _| {
        value(v, "bateria", "row:lang").is_some() && value(v, "bateria", "row:tz").is_some()
    });
    assert_eq!(value(&view, "bateria", "row:lang"), Some("granted"));
    assert_eq!(
        value(&view, "bateria", "row:tz"),
        Some("denied"),
        "not in config_read"
    );
    probe.wait(|v, n| {
        v.modules[0].settings["watts"] == "0"
            && n.contains(&Notice::Text("undeclared refused".into()))
    });
    assert_eq!(fs::read_to_string(&settings).unwrap(), r#"{"watts":false}"#);
    // The launcher's own change reaches the module as an event.
    probe.request(Request::Set {
        module: "bateria".into(),
        key: "watts".into(),
        value: "1".into(),
    });
    probe.modules.focus("snes", None);
    probe.modules.focus("nes", None);
    let seen = until(&base.join("module-data/bateria/seen.log"), "focus.system");
    assert!(seen.contains(r#""name":"settings.changed""#) && seen.contains(r#""value":true"#));
    assert_eq!(
        seen.matches("focus.system").count(),
        1,
        "the cursor rested once: {seen}"
    );
    assert!(seen.contains(r#""system":"nes""#));
}

/// Runs modules plainly and records the sandbox each one would get.
#[derive(Default)]
struct Sandboxes(Mutex<Vec<(String, Vec<String>)>>);
impl Executor for Sandboxes {
    fn command(&self, plan: &Plan) -> Command {
        let args = process::sandbox(plan);
        self.0.lock().unwrap().push((plan.id.clone(), args));
        Direct::default().command(plan)
    }
    fn kill(&self, _: Option<&str>) {}
}
impl Sandboxes {
    fn args(&self, id: &str) -> String {
        let runs = self.0.lock().unwrap();
        runs.iter()
            .rev()
            .find(|(m, _)| m == id)
            .unwrap()
            .1
            .join(" ")
    }
    fn user(&self, id: &str) -> u32 {
        let args = self.args(id);
        let user = args.split(" -p ").find_map(|a| a.strip_prefix("User="));
        user.unwrap().parse().unwrap()
    }
}
/// Whether `uid` (with gid = uid, no other groups) may list or read `path`.
fn readable(path: &Path, uid: u32) -> bool {
    let meta = fs::metadata(path).unwrap();
    let mode = meta.permissions().mode();
    uid == 0
        || if meta.uid() == uid {
            mode & 0o400 != 0
        } else if meta.gid() == uid {
            mode & 0o040 != 0
        } else {
            mode & 0o004 != 0
        }
}

#[test]
fn each_module_runs_as_its_own_user_and_keeps_its_secrets_apart() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    let page = "[[start.tiles]]\nid = \"t\"\nlabel = \"@name\"\naction = \"ask\"\n\
        [[settings.pages]]\nid = \"main\"\n\
        [[settings.pages.rows]]\ntype = \"keyboard\"\nkey = \"token\"\nsecret = true\n\
        [[settings.pages.rows]]\ntype = \"toggle\"\nkey = \"watts\"\ndefault = true\n";
    for id in ["alpha", "beta"] {
        module(base, id, "daemon", page, DAEMON);
    }
    allow(base, &[("alpha", &[]), ("beta", &[])]);
    // As root (a CI container) the files are really handed to each uid.
    // SAFETY: geteuid has no preconditions.
    let root_user = unsafe { libc::geteuid() } == 0;
    let both = |v: &View, _: &[Notice]| {
        ["alpha", "beta"]
            .iter()
            .all(|id| value(v, id, "tile:t").is_some())
    };
    let sandboxes = Arc::new(Sandboxes::default());
    let userdb = root_user.then(|| base.join("userdb"));
    let mut probe = Probe {
        modules: start_with(base, sandboxes.clone(), None, userdb.clone()),
        view: Arc::default(),
        notices: Vec::new(),
    };
    probe.wait(both);
    let (alpha, beta) = (sandboxes.user("alpha"), sandboxes.user("beta"));
    assert_ne!(alpha, beta);
    for (id, uid) in [("alpha", alpha), ("beta", beta)] {
        let args = sandboxes.args(id);
        assert!(args.contains(&format!("-p Group={uid}")), "{args}");
        assert!(args.contains("-p PrivateUsers=yes"), "{args}");
        let data = base.join("module-data").join(id);
        assert_eq!(
            fs::metadata(&data).unwrap().permissions().mode() & 0o777,
            0o700
        );
        if root_user {
            assert_eq!(fs::metadata(&data).unwrap().uid(), uid);
        }
    }
    // Neither module's uid can read the other's folder.
    assert!(!readable(&base.join("module-data/alpha"), beta));
    assert!(!readable(&base.join("module-data/beta"), alpha));

    probe.request(Request::Set {
        module: "alpha".into(),
        key: "token".into(),
        value: "s3cr3t-value".into(),
    });
    probe.wait(|v, _| v.modules[0].settings["token"] == "s3cr3t-value");
    let secrets = base.join("module-settings/alpha.secret.json");
    assert_eq!(
        fs::read_to_string(&secrets).unwrap(),
        r#"{"token":"s3cr3t-value"}"#
    );
    assert_eq!(
        fs::metadata(&secrets).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!readable(&secrets, beta));
    if root_user {
        assert_eq!(fs::metadata(&secrets).unwrap().uid(), alpha);
        assert!(userdb.unwrap().join(format!("{alpha}.user")).is_file());
    }
    let settings = fs::read_to_string(base.join("module-settings/alpha.json")).unwrap();
    assert_eq!(
        settings, r#"{"watts":true}"#,
        "no secret among the settings"
    );
    // The value reaches the module on its stdin, never in an argument.
    until(&base.join("module-data/alpha/seen.log"), "s3cr3t-value");
    assert!(sandboxes
        .0
        .lock()
        .unwrap()
        .iter()
        .all(|(_, args)| !args.join(" ").contains("s3cr3t")));
    assert!(sandboxes
        .args("alpha")
        .contains(&format!("BindReadOnlyPaths=-{}", secrets.display())));

    // A restart keeps every uid.
    drop(probe);
    let again = Arc::new(Sandboxes::default());
    let mut probe = Probe {
        modules: start_with(base, again.clone(), None, None),
        view: Arc::default(),
        notices: Vec::new(),
    };
    let view = probe.wait(both);
    assert_eq!((again.user("alpha"), again.user("beta")), (alpha, beta));
    assert_eq!(view.modules[0].settings["token"], "s3cr3t-value");
}

/// The file's text once it holds `wanted`.
fn until(path: &Path, wanted: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = fs::read_to_string(path).unwrap_or_default();
        if text.contains(wanted) {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "{wanted} never reached {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_stop_before_a_game_ends_every_module_and_the_return_reports_the_game() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    module(base, "bateria", "daemon", PAGE, DAEMON);
    // A daemon that ignores shutdown must still be gone.
    module(
        base,
        "teimoso",
        "daemon",
        "",
        "#!/bin/bash\necho $$ > \"$HOME/pid\"\ntrap '' TERM\nread -r line\nid=$(printf '%s' \"$line\" | sed -n 's/.*\"id\":\\([0-9]*\\).*/\\1/p')\n\
         printf '{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{\"ready\":true}}\\n' \"$id\"\nwhile :; do sleep 1; done\n",
    );
    allow(
        base,
        &[
            ("bateria", &["config_read:system.language"]),
            ("teimoso", &[]),
        ],
    );
    let mut probe = Probe::new(base);
    let pid = |id: &str| -> i32 {
        until(&base.join("module-data").join(id).join("pid"), "\n")
            .trim()
            .parse()
            .unwrap()
    };
    let pids = [pid("bateria"), pid("teimoso")];
    probe.wait(|v, _| value(v, "bateria", "tile:t").is_some());
    let began = Instant::now();
    probe
        .modules
        .hand_off("snes", "snes/Chrono.sfc", "Chrono Trigger");
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "{:?}",
        began.elapsed()
    );
    std::thread::sleep(Duration::from_millis(100));
    for pid in pids {
        // SAFETY: probing with signal 0.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        assert!(!alive, "process {pid} survived the hand-off");
    }
    let seen = fs::read_to_string(base.join("module-data/bateria/seen.log")).unwrap();
    assert!(seen.contains(r#""name":"game.started""#) && seen.contains("Chrono Trigger"));
    assert!(seen.contains(r#""method":"shutdown""#));
    assert!(base.join("run/last-launch.json").is_file());
    drop(probe);
    // The launcher comes back: the game's end is the first news.
    fs::remove_file(base.join("module-data/bateria/seen.log")).unwrap();
    let probe = Probe::new(base);
    let seen = until(&base.join("module-data/bateria/seen.log"), "game.ended");
    assert!(seen.contains("snes/Chrono.sfc"));
    assert!(!base.join("run/last-launch.json").exists());
    drop(probe);
}

#[test]
fn a_oneshot_that_hangs_three_times_is_switched_off_with_a_notice() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    module(
        base,
        "lento",
        "oneshot",
        "[[start.tiles]]\nid = \"t\"\naction = \"go\"\n",
        "#!/bin/bash\nread -r line\nsleep 30\n",
    );
    allow(base, &[("lento", &[])]);
    let mut probe = Probe::new(base);
    probe.wait(|v, _| v.modules.len() == 1);
    for _ in 0..3 {
        probe.invoke("lento", "go");
    }
    let view = probe.wait(|v, _| v.modules[0].status == Status::Failed);
    assert!(view.tiles().is_empty(), "its tile is gone");
    probe.wait(|v, n| v.busy.is_empty() && n.contains(&Notice::Disabled("Test lento".into())));
    assert!(fs::read_to_string(base.join("modules.toml"))
        .unwrap()
        .contains("failed = true"));
    // Switching it on again forgives it.
    probe.request(Request::Enable {
        module: "lento".into(),
        on: true,
        grant: false,
    });
    probe.wait(|v, _| v.modules[0].status == Status::Ready);
}

#[test]
fn a_daemon_that_breaks_the_protocol_is_switched_off() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    module(base, "bateria", "daemon", PAGE, DAEMON);
    allow(base, &[("bateria", &["config_read:system.language"])]);
    let mut probe = Probe::new(base);
    probe.wait(|v, _| value(v, "bateria", "tile:t").is_some());
    for _ in 0..3 {
        probe.invoke("bateria", "garbage");
    }
    probe.wait(|v, n| {
        v.modules[0].status == Status::Failed
            && n.contains(&Notice::Disabled("Test bateria".into()))
    });
}

#[test]
fn modules_without_consent_or_with_new_permissions_never_run() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    module(base, "bateria", "daemon", PAGE, DAEMON);
    module(base, "copiado", "daemon", PAGE, DAEMON);
    // Accepted before it asked to read the language.
    allow(base, &[("bateria", &[])]);
    let mut probe = Probe::new(base);
    let view = probe.wait(|v, _| v.modules.len() == 2);
    assert!(view.modules.iter().all(|m| !m.active()));
    std::thread::sleep(Duration::from_millis(500));
    assert!(!base.join("module-data/bateria/pid").exists());
    assert!(!base.join("module-data/copiado/pid").exists());
    // The permission page's "allow" records the grant and starts it.
    probe.request(Request::Enable {
        module: "copiado".into(),
        on: true,
        grant: true,
    });
    probe.wait(|v, _| value(v, "copiado", "tile:t").is_some());
    let state = State::load(&base.join("modules.toml"));
    assert_eq!(
        state.modules["copiado"].granted,
        ["config_read:system.language"]
    );
}

#[test]
fn at_most_three_daemons_run() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    let ids = ["uno", "dois", "tres", "quatro"];
    for id in ids {
        module(base, id, "daemon", PAGE, DAEMON);
    }
    let granted: &[&str] = &["config_read:system.language"];
    allow(
        base,
        &[("uno", granted), ("dois", granted), ("tres", granted)],
    );
    let mut probe = Probe::new(base);
    probe.wait(|v, _| ids[..3].iter().all(|id| value(v, id, "tile:t").is_some()));
    probe.request(Request::Enable {
        module: "quatro".into(),
        on: true,
        grant: true,
    });
    probe.wait(|_, n| n.contains(&Notice::TooMany("Test quatro".into())));
    assert!(!base.join("module-data/quatro/pid").exists());
}

#[test]
fn an_update_that_keeps_failing_gives_way_to_the_previous_version() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    let tile = "[[start.tiles]]\nid = \"t\"\naction = \"go\"\n";
    module(
        base,
        "lento",
        "oneshot",
        tile,
        "#!/bin/bash\nread -r line\nexit 0\n",
    );
    let modules = base.join("modules");
    fs::rename(modules.join("lento"), modules.join(".previous-lento")).unwrap();
    module(
        base,
        "lento",
        "oneshot",
        tile,
        "#!/bin/bash\nread -r line\nexit 3\n",
    );
    let toml = modules.join("lento/module.toml");
    fs::write(
        &toml,
        fs::read_to_string(&toml).unwrap().replace("1.0.0", "1.1.0"),
    )
    .unwrap();
    allow(base, &[("lento", &[])]);
    let mut probe = Probe::new(base);
    probe.wait(|v, _| v.modules.first().is_some_and(|m| m.version == "1.1.0"));
    for _ in 0..3 {
        probe.invoke("lento", "go");
    }
    probe.wait(|v, n| {
        v.modules[0].version == "1.0.0" && n.contains(&Notice::RolledBack("Test lento".into()))
    });
    assert!(!modules.join(".previous-lento").exists());
    assert_eq!(
        probe.view.modules[0].status,
        Status::Ready,
        "the old one runs on"
    );
    probe.invoke("lento", "go");
    probe.wait(|v, _| v.busy.is_empty());
}

/// The two examples of docs/modulos.md pass the checks and run (they need
/// jq, as on the device).
#[test]
fn the_documented_examples_check_and_run() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/modules");
    for id in ["bateria-detalhada", "perfil-cpu"] {
        let warnings = archr_launcher::modules::check_module(&examples.join(id)).unwrap();
        assert!(warnings.is_empty(), "{id}: {warnings:?}");
    }
    if std::process::Command::new("jq")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("jq missing: the examples were only checked");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    for id in ["bateria-detalhada", "perfil-cpu"] {
        let to = base.join("modules").join(id);
        for entry in walk(&examples.join(id)) {
            let target = to.join(entry.strip_prefix(examples.join(id)).unwrap());
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(&entry, target).unwrap();
        }
    }
    allow(base, &[("bateria-detalhada", &[]), ("perfil-cpu", &[])]);
    let mut probe = Probe::new(base);
    // A PC has no battery: the daemon hides its tile.
    probe.wait(|v, _| {
        v.hidden
            .contains(&("bateria-detalhada".into(), "tile:bateria".into()))
    });
    probe.request(Request::Invoke {
        module: "perfil-cpu".into(),
        action: "sugerir".into(),
        call: Some(archr_ui::modules::Call {
            system: "n64".into(),
            ..Default::default()
        }),
    });
    probe.wait(|_, n| n.contains(&Notice::Text("n64: maximum performance suggested".into())));
    probe.request(Request::Set {
        module: "perfil-cpu".into(),
        key: "pesados".into(),
        value: "schedutil".into(),
    });
    probe.wait(|v, _| v.modules[1].settings["pesados"] == "schedutil");
    std::thread::sleep(Duration::from_secs(10));
    probe.request(Request::Invoke {
        module: "perfil-cpu".into(),
        action: "sugerir".into(),
        call: Some(archr_ui::modules::Call {
            system: "psp".into(),
            ..Default::default()
        }),
    });
    probe.wait(|_, n| n.contains(&Notice::Text("psp: balanced profile suggested".into())));
}
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        if entry.file_type().unwrap().is_dir() {
            files.extend(walk(&entry.path()));
        } else {
            files.push(entry.path());
        }
    }
    files
}
