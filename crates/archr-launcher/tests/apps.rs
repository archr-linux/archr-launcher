//! Apps (API 4) against the host, without a sandbox: the example
//! catalogo-demo end to end (its screens, an install into the library and a
//! removal after the user's yes), and what the host refuses a hostile app.
use archr_launcher::{
    modules::{
        app::{Ledger, LEDGER},
        process::Direct,
        registry::{Dirs, Record, State},
        Config, Event, Modules,
    },
    paths::Paths,
};
use archr_ui::apps::{Layout, Message};
use archr_ui::modules::{Call, Request, View};
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

fn jq() -> bool {
    std::process::Command::new("jq")
        .arg("--version")
        .output()
        .is_ok()
}
/// A root with one system, Game Boy, at `/storage/roms/gb`.
fn root() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("storage/.config/archr");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("systems.cfg"),
        "<systemList><system><name>gb</name><fullname>Game Boy</fullname>\
         <path>/storage/roms/gb</path><extension>.gb .zip</extension>\
         <command>/usr/bin/runemu.sh %ROM% -P%SYSTEM% --core=%CORE%</command></system>\
         <system><name>ports</name><fullname>Ports</fullname>\
         <path>/storage/roms/ports</path><extension>.sh</extension></system></systemList>",
    )
    .unwrap();
    root
}
fn start(root: &Path, id: &str, granted: &[&str]) -> Modules {
    let base = root.join("storage/.config/archr-launcher");
    let mut state = State::default();
    state.modules.insert(
        id.into(),
        Record {
            consent: true,
            enabled: true,
            granted: granted.iter().map(|g| (*g).into()).collect(),
            ..Record::default()
        },
    );
    state.save(&base.join("modules.toml")).unwrap();
    let direct = Arc::new(Direct::default());
    Modules::start(Config {
        dirs: Dirs::new(&base),
        run: root.join("run"),
        system_cfg: root.join("system.cfg"),
        lock: root.join("system.cfg.lock"),
        executor: direct.clone(),
        direct: Some(direct),
        userdb: None,
        language: "en".into(),
        device: "test".into(),
        delay: Duration::ZERO,
        auto_delay: Duration::ZERO,
        firewall: None,
        bin: root.join("bin"),
        shop: None,
        cpufreq: root.join("cpufreq/policy0"),
        perf: root.join("run/perf-last.json"),
        library: Some(Paths::new(Some(root.to_owned()))),
    })
}
/// What the host published, kept until a test looks.
struct Probe {
    modules: Modules,
    view: Arc<View>,
    screens: Vec<Message>,
    installed: Vec<String>,
    closed: Vec<String>,
}
impl Probe {
    fn new(modules: Modules) -> Self {
        Self {
            modules,
            view: Arc::default(),
            screens: Vec::new(),
            installed: Vec::new(),
            closed: Vec::new(),
        }
    }
    fn wait(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !done(self) {
            for event in self.modules.poll() {
                match event {
                    Event::View(view) => self.view = view,
                    Event::Screens(_, messages) => self.screens.extend(messages),
                    Event::Installed(system) => self.installed.push(system),
                    Event::AppClosed(id) => self.closed.push(id),
                    _ => {}
                }
            }
            assert!(Instant::now() < deadline, "{what}: {:?}", self.screens);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    /// The last screen shown with this id.
    fn shown(&self, id: &str) -> Option<&archr_ui::apps::Screen> {
        self.screens.iter().rev().find_map(|m| match m {
            Message::Show(screen) if screen.id == id => Some(&**screen),
            _ => None,
        })
    }
    fn event(&self, module: &str, name: &'static str, data: serde_json::Value) {
        self.modules.request(Request::AppEvent {
            module: module.into(),
            name,
            data,
        });
    }
}
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn the_demo_catalog_installs_and_removes_a_game() {
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/modules/catalogo-demo");
    let warnings = archr_launcher::modules::check_module(&example).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    if !jq() {
        eprintln!("jq missing: the example was only checked");
        return;
    }
    let root = root();
    let base = root.path().join("storage/.config/archr-launcher");
    copy_dir(&example, &base.join("modules/catalogo-demo"));
    let id = "catalogo-demo";
    let mut probe = Probe::new(start(root.path(), id, &["install:gb", "launch"]));
    probe.wait("its row", |p| p.view.apps().len() == 1);
    assert_eq!(probe.view.apps()[0].key, "@app:catalogo-demo");
    assert_eq!(probe.view.apps()[0].name, "Demo catalog");
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        probe.view.values.is_empty(),
        "no process before the app opens: {:?}",
        probe.view.values
    );
    probe.modules.request(Request::OpenApp(id.into()));
    probe.wait("the catalog", |p| p.shown("catalog").is_some());
    let catalog = probe.shown("catalog").unwrap();
    assert_eq!(catalog.layout, Layout::Collection);
    assert_eq!((catalog.items.len(), catalog.more), (3, true));
    assert_eq!(catalog.title, "Catalog");
    assert_eq!(catalog.items[0].badge, "Free");
    let cover = catalog.items[0]
        .image
        .clone()
        .expect("its cover, from its folder");
    // The screens decode the launcher's copy, never the module's file.
    assert!(cover.starts_with(base.join("module-data/.pictures/catalogo-demo")));
    assert_eq!(
        fs::read(&cover).unwrap(),
        fs::read(base.join("module-data/catalogo-demo/cache/covers/g1.png")).unwrap()
    );
    probe.wait("its summary", |p| {
        p.view.apps()[0].summary == "4 test games"
    });
    probe.event(id, "screen.more", json!({"screen": "catalog", "count": 3}));
    probe.wait("the fourth", |p| {
        p.screens
            .iter()
            .any(|m| matches!(m, Message::Patch(patch) if patch.append.len() == 1))
    });
    probe.event(
        id,
        "screen.select",
        json!({"screen": "catalog", "item": "g1"}),
    );
    probe.wait("the detail", |p| p.shown("game:g1").is_some());
    let detail = probe.shown("game:g1").unwrap();
    assert_eq!(detail.layout, Layout::Detail);
    assert_eq!(detail.buttons[0].id, "get");
    assert!(
        detail.buttons[0].icon.is_some(),
        "the module's own Lucide icon"
    );
    assert!(detail.image.is_some());
    let invoke = |modules: &Modules, action: &str| {
        modules.request(Request::Invoke {
            module: id.into(),
            action: action.into(),
            call: Some(Call {
                screen: Some("game:g1".into()),
                button: Some(action.into()),
                ..Call::default()
            }),
        })
    };
    invoke(&probe.modules, "get");
    probe.wait("installed", |p| p.installed.iter().any(|s| s == "gb"));
    let rom = root.path().join("storage/roms/gb/Pixel Hop.gb");
    assert_eq!(fs::metadata(&rom).unwrap().len(), 32768);
    assert!(root
        .path()
        .join("storage/roms/gb/media/images/Pixel Hop.png")
        .is_file());
    let gamelist = fs::read_to_string(root.path().join("storage/roms/gb/gamelist.xml")).unwrap();
    assert!(gamelist.contains("<name>Pixel Hop</name>"), "{gamelist}");
    let ledger = Ledger::load(&base.join("module-data").join(LEDGER));
    assert_eq!(ledger.modules[id].games[0].game_key, "gb/Pixel Hop.gb");
    probe.wait("Play", |p| {
        p.shown("game:g1")
            .is_some_and(|s| s.buttons[0].id == "play")
    });
    probe.wait("the action answered", |p| p.view.busy.is_empty());
    // Removing: the launcher asks the user itself, and acts on the answer.
    let count = probe.installed.len();
    invoke(&probe.modules, "remove");
    probe.wait("the question", |p| {
        p.screens
            .iter()
            .any(|m| matches!(m, Message::Confirm(ask) if ask.launcher && ask.text == "Pixel Hop"))
    });
    probe.event(id, "launcher.answer", json!({"yes": true}));
    probe.wait("removed", |p| p.installed.len() > count);
    assert!(!rom.exists());
    let ledger = Ledger::load(&base.join("module-data").join(LEDGER));
    assert!(ledger.modules[id].games.is_empty());
    probe.wait("Get again", |p| {
        p.shown("game:g1").is_some_and(|s| s.buttons[0].id == "get")
    });
    probe.modules.request(Request::CloseApp(id.into()));
    std::thread::sleep(Duration::from_millis(800));
    probe.wait("quiet", |_| true);
    assert!(
        probe.closed.is_empty(),
        "the user closed it: {:?}",
        probe.closed
    );
}

/// An app that asks for what it may not: it logs every answer it gets.
const HOSTILE: &str = r#"#!/bin/bash
log="$HOME/answers.log"
send() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    id=$(jq -r '.id // "null"' <<<"$line")
    method=$(jq -r '.method // ""' <<<"$line")
    case "$method" in
        initialize) send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"ready\":true}}" ;;
        shutdown) send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"; exit 0 ;;
        screen.opened)
            mkdir -p "$HOME/d"; printf 'rom' >"$HOME/d/x.gb"; printf 'rom' >"$HOME/d/x.nes"
            ln -sf /etc/hostname "$HOME/d/link.gb"
            send '{"jsonrpc":"2.0","id":1,"method":"rom.install","params":{"system":"gb","file":"../../x.gb"}}'
            send '{"jsonrpc":"2.0","id":2,"method":"rom.install","params":{"system":"nes","file":"d/x.nes"}}'
            send '{"jsonrpc":"2.0","id":3,"method":"rom.install","params":{"system":"gb","file":"d/link.gb"}}'
            send '{"jsonrpc":"2.0","id":4,"method":"rom.remove","params":{"game_key":"gb/Other.gb"}}'
            send '{"jsonrpc":"2.0","id":5,"method":"port.install","params":{"name":"x;reboot.zip"}}'
            send '{"jsonrpc":"2.0","id":6,"method":"port.install","params":{"name":"2048.zip"}}'
            printf 'rom' >"$HOME/d/ok.gb"
            send '{"jsonrpc":"2.0","id":7,"method":"rom.install","params":{"system":"gb","file":"d/ok.gb"}}'
            send '{"jsonrpc":"2.0","method":"screen.show","params":{"id":"s","layout":"collection","items":[{"id":"a","title":"A","image":"../../../etc/x.png"}]}}'
            ;;
        "") printf '%s\n' "$line" >>"$log" ;;
    esac
done
"#;

#[test]
fn an_app_gets_nothing_it_may_not_have() {
    if !jq() {
        eprintln!("jq missing");
        return;
    }
    let root = root();
    let base = root.path().join("storage/.config/archr-launcher");
    let dir = base.join("modules/rogue");
    fs::create_dir_all(dir.join("locales")).unwrap();
    fs::write(dir.join("locales/en.toml"), "\"name\" = \"Rogue\"\n").unwrap();
    fs::write(dir.join("main.sh"), HOSTILE).unwrap();
    fs::write(
        dir.join("module.toml"),
        "id = \"rogue\"\nversion = \"1.0.0\"\napi_version = 4\nname = \"@name\"\n\
         [run]\nruntime = \"bash\"\nentry = \"main.sh\"\nmode = \"daemon\"\n\
         [app]\nhome = \"main\"\n[permissions]\ninstall = [\"gb\"]\n",
    )
    .unwrap();
    // Somebody else's game, which the module must not touch.
    let other = root.path().join("storage/roms/gb/Other.gb");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, b"mine").unwrap();
    let mut probe = Probe::new(start(root.path(), "rogue", &["install:gb"]));
    probe.wait("its row", |p| p.view.apps().len() == 1);
    probe.modules.request(Request::OpenApp("rogue".into()));
    let log = base.join("module-data/rogue/answers.log");
    probe.wait("seven answers", |_| {
        fs::read_to_string(&log).is_ok_and(|t| t.lines().count() >= 7)
    });
    let answers: Vec<serde_json::Value> = fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let code = |id: u64| {
        answers
            .iter()
            .find(|a| a["id"] == id)
            .map(|a| a["error"]["code"].as_i64().unwrap_or(0))
    };
    assert_eq!(code(1), Some(-32602), "a path out of its folder");
    assert_eq!(code(2), Some(-32001), "a system it did not ask for");
    assert_eq!(code(3), Some(-32602), "a link out of its folder");
    assert_eq!(code(4), Some(-32001), "a game it did not install");
    assert_eq!(code(5), Some(-32001), "no ports permission");
    assert_eq!(code(6), Some(-32001), "no ports permission");
    assert_eq!(
        code(7),
        Some(-32001),
        "no install without the user's action"
    );
    assert_eq!(fs::read(&other).unwrap(), b"mine");
    assert!(!root.path().join("storage/roms/nes").exists());
    probe.wait("its screen", |p| p.shown("s").is_some());
    assert_eq!(
        probe.shown("s").unwrap().items[0].image,
        None,
        "a picture out of its folder"
    );
}

/// An app with `install = ["ports"]`: the launcher runs its own script for
/// PortMaster, passes the progress on and records the port as the module's.
const PORTER: &str = r#"#!/bin/bash
log="$HOME/answers.log"
send() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    id=$(jq -r '.id // "null"' <<<"$line")
    method=$(jq -r '.method // ""' <<<"$line")
    action=$(jq -r '.params.action // ""' <<<"$line")
    case "$method" in
        initialize) send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"ready\":true}}" ;;
        shutdown) send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"; exit 0 ;;
        action.invoke)
            send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{}}"
            case "$action" in
                install) send '{"jsonrpc":"2.0","id":1,"method":"port.install","params":{"name":"2048.zip"}}' ;;
                theirs) send '{"jsonrpc":"2.0","id":2,"method":"port.install","params":{"name":"user.zip"}}' ;;
                remove) send '{"jsonrpc":"2.0","id":3,"method":"port.remove","params":{"name":"2048.zip"}}' ;;
            esac
            ;;
        port.progress) printf '%s\n' "$line" >>"$log" ;;
        "") printf '%s\n' "$line" >>"$log" ;;
    esac
done
"#;

#[test]
fn ports_go_through_the_system_script() {
    use std::os::unix::fs::PermissionsExt;
    if !jq() {
        eprintln!("jq missing");
        return;
    }
    let root = root();
    let base = root.path().join("storage/.config/archr-launcher");
    let dir = base.join("modules/porter");
    fs::create_dir_all(dir.join("locales")).unwrap();
    fs::write(dir.join("locales/en.toml"), "\"name\" = \"Porter\"\n").unwrap();
    fs::write(dir.join("main.sh"), PORTER).unwrap();
    fs::write(
        dir.join("module.toml"),
        "id = \"porter\"\nversion = \"1.0.0\"\napi_version = 4\nname = \"@name\"\n\
         [run]\nruntime = \"bash\"\nentry = \"main.sh\"\nmode = \"daemon\"\n\
         [app]\nhome = \"main\"\n[permissions]\ninstall = [\"ports\"]\n",
    )
    .unwrap();
    // The user's own port, which the module must not take over.
    let ports = root.path().join("storage/roms/ports");
    fs::create_dir_all(ports.join("user")).unwrap();
    fs::write(
        ports.join("user/port.json"),
        r#"{"name":"user.zip","items":["User.sh","user/"]}"#,
    )
    .unwrap();
    // The fake archr-port-install: what it was asked, a bar, PortMaster's files.
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let calls = root.path().join("calls");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >>{calls}\necho 'PROGRESS 50'\ncd '{ports}'\n\
         if [ \"$1\" = install ]; then mkdir -p 2048 && echo x >2048.sh && \
         echo '{{\"name\":\"2048.zip\",\"items\":[\"2048.sh\",\"2048/\"]}}' >2048/port.json; \
         else rm -rf 2048 2048.sh; fi\necho 'PROGRESS 100'\n",
        calls = calls.display(),
        ports = ports.display()
    );
    fs::write(bin.join("archr-port-install"), script).unwrap();
    fs::set_permissions(
        bin.join("archr-port-install"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let mut probe = Probe::new(start(root.path(), "porter", &["install:ports"]));
    probe.wait("its row", |p| p.view.apps().len() == 1);
    probe.modules.request(Request::OpenApp("porter".into()));
    probe.wait("open", |p| p.view.modules.iter().all(|m| m.failures == 0));
    std::thread::sleep(Duration::from_millis(500));
    let log = base.join("module-data/porter/answers.log");
    let answer = |id: u64| {
        fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find(|a| a["id"] == id)
    };
    let act = |modules: &Modules, action: &str| {
        modules.request(Request::Invoke {
            module: "porter".into(),
            action: action.into(),
            call: Some(Call {
                screen: Some("main".into()),
                ..Call::default()
            }),
        })
    };
    act(&probe.modules, "theirs");
    probe.wait("refused", |_| answer(2).is_some());
    assert_eq!(answer(2).unwrap()["error"]["code"], -32005);
    act(&probe.modules, "install");
    probe.wait("installed", |p| p.installed.iter().any(|s| s == "ports"));
    probe.wait("the answer", |_| answer(1).is_some());
    let text = fs::read_to_string(&log).unwrap();
    assert!(text.contains("\"progress\":50"), "{text}");
    assert_eq!(
        answer(1).unwrap()["result"]["game_key"],
        "ports/2048.sh",
        "for Play"
    );
    assert_eq!(fs::read_to_string(&calls).unwrap(), "install 2048.zip\n");
    let ledger = Ledger::load(&base.join("module-data").join(LEDGER));
    let port = &ledger.modules["porter"].ports[0];
    assert_eq!(
        (port.name.as_str(), port.game_key.as_str()),
        ("2048.zip", "ports/2048.sh")
    );
    assert!(port.size > 0, "counted against its quota");
    // Removing: the launcher's question, then the script; a no keeps it.
    act(&probe.modules, "remove");
    probe.wait("the question", |p| {
        p.screens
            .iter()
            .any(|m| matches!(m, Message::Confirm(ask) if ask.launcher))
    });
    probe.event("porter", "launcher.answer", json!({"yes": true}));
    probe.wait("removed", |_| answer(3).is_some());
    assert_eq!(answer(3).unwrap()["result"], json!({"removed": true}));
    assert_eq!(
        fs::read_to_string(&calls).unwrap(),
        "install 2048.zip\nuninstall 2048.zip\n"
    );
    let ledger = Ledger::load(&base.join("module-data").join(LEDGER));
    assert!(ledger.modules["porter"].ports.is_empty());
}
