//! Background index/configuration work. Construct only after the first present.
use crate::{handoff::read_last_exit, paths::Paths, store::Store};
use archr_config::{rom_key, SystemCfg};
use archr_index::{
    cache::{self, Overview},
    load_systems, Game, SystemDef,
};
use std::{
    fs,
    sync::mpsc::{self, Receiver, Sender},
};

pub enum Work {
    Games(String),
    Core {
        system: String,
        rom: String,
        emulator: String,
        core: String,
    },
}
pub enum Update {
    Boot {
        systems: Vec<SystemDef>,
        config: Option<SystemCfg>,
        store: Store,
        banner: Option<String>,
    },
    Overview(Overview),
    Games(String, Vec<Game>),
    CoreSaved(String, String, Result<Option<SystemCfg>, String>),
}
pub struct Worker {
    pub send: Sender<Work>,
    pub ready: Receiver<Update>,
}

pub fn definitions(paths: &Paths) -> Result<Vec<SystemDef>, String> {
    let mut systems = load_systems(&paths.systems)
        .map_err(|error| format!("Não foi possível ler es_systems.cfg: {error}"))?;
    for system in &mut systems {
        system.path = paths.remap(&system.path);
    }
    Ok(systems)
}

pub fn start(paths: Paths, initial: Vec<SystemDef>) -> Worker {
    let (send, jobs) = mpsc::channel();
    let (done, ready) = mpsc::channel();
    std::thread::spawn(move || {
        let (systems, mut banner) = if initial.is_empty() {
            match definitions(&paths) {
                Ok(systems) => (systems, None),
                Err(error) => (Vec::new(), Some(error)),
            }
        } else {
            (initial, None)
        };
        if let Some(code) = read_last_exit(&paths.run).filter(|code| *code != 0) {
            banner = Some(format!(
                "O jogo terminou com erro (código {code}).\nLog: /var/log/exec.log"
            ));
        }
        match fs::remove_file(paths.run.join("last-exit")) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("last-exit: {error}")
            }
            _ => {}
        }
        let boot = Update::Boot {
            systems: systems.clone(),
            config: SystemCfg::load(&paths.config).ok(),
            store: Store::load(&paths.store),
            banner,
        };
        if done.send(boot).is_err() {
            return;
        }
        // Overview rebuilding must not delay selected-system loads.
        let overview_paths = paths.clone();
        let overview_systems = systems.clone();
        let overview_done = done.clone();
        std::thread::spawn(move || {
            let overview = cache::build_overview(&overview_systems);
            if let Err(error) = cache::save_overview(&overview_paths.cache, &overview) {
                eprintln!("overview cache: {error}");
            }
            let _ = overview_done.send(Update::Overview(overview));
        });
        while let Ok(job) = jobs.recv() {
            let update = match job {
                Work::Games(name) => {
                    let mut games = systems
                        .iter()
                        .find(|system| system.name == name)
                        .map(|system| cache::load_games(&paths.cache, system))
                        .unwrap_or_default();
                    for game in &mut games {
                        game.path = paths.remap(&game.path);
                        game.image = game.image.as_deref().map(|path| paths.remap(path));
                        game.video = game.video.as_deref().map(|path| paths.remap(path));
                    }
                    Update::Games(name, games)
                }
                Work::Core {
                    system,
                    rom,
                    emulator,
                    core,
                } => {
                    let result = save_core(&paths, &system, &rom, &emulator, &core)
                        .map(|()| SystemCfg::load(&paths.config).ok())
                        .map_err(|error| error.to_string());
                    Update::CoreSaved(system, rom, result)
                }
            };
            if done.send(update).is_err() {
                break;
            }
        }
    });
    Worker { send, ready }
}
fn save_core(
    paths: &Paths,
    system: &str,
    rom: &str,
    emulator: &str,
    core: &str,
) -> std::io::Result<()> {
    if let Some(parent) = paths.config.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = paths.lock.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&paths.config)
    {
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    SystemCfg::set_and_save(
        &paths.config,
        &paths.lock,
        &rom_key(system, rom, "emulator"),
        emulator,
    )?;
    SystemCfg::set_and_save(
        &paths.config,
        &paths.lock,
        &rom_key(system, rom, "core"),
        core,
    )
}
