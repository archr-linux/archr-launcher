//! Background index/configuration work. Construct only after the first present.
use crate::{handoff::read_last_exit, paths::Paths};
use archr_config::{rom_key, SystemCfg};
use archr_index::{
    cache::{self, Overview},
    load_systems, Game, SystemDef,
};
use archr_ui::i18n::Message;
use std::{
    collections::{HashSet, VecDeque},
    fs,
    sync::mpsc::{self, Receiver, Sender},
};

pub enum Work {
    Games {
        name: String,
        background: bool,
    },
    Core {
        system: String,
        rom: String,
        emulator: String,
        core: String,
    },
    /// Rescan a loaded system whose files changed (new scraped art).
    Reload(String),
    /// Re-read system.cfg after the settings changed launch choices.
    Config,
}
pub enum Update {
    Boot {
        systems: Vec<SystemDef>,
        config: Option<SystemCfg>,
        banner: Option<Message>,
    },
    Overview(Overview),
    Games(String, Vec<Game>),
    CoreSaved(String, String, Result<Option<SystemCfg>, String>),
    Config(Option<SystemCfg>),
}
pub struct Worker {
    pub send: Sender<Work>,
    pub ready: Receiver<Update>,
}

pub fn definitions(paths: &Paths) -> Result<Vec<SystemDef>, String> {
    let mut systems = load_systems(&paths.systems)
        .map_err(|error| format!("Cannot read {}: {error}", paths.systems.display()))?;
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
                Err(error) => {
                    log::warn!("Cannot load systems: {error}");
                    (Vec::new(), Some(Message::new("error.systems")))
                }
            }
        } else {
            (initial, None)
        };
        if let Some(code) = read_last_exit(&paths.run).filter(|code| *code != 0) {
            log::warn!("Game exited with code {code}; log: /var/log/exec.log");
            banner = Some(Message::new("error.game_exit").with("code", code));
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
            let old = cache::load_overview(&overview_paths.cache).unwrap_or_default();
            let (overview, changed) =
                cache::refresh_overview_with_mapper(&old, &overview_systems, &|path| {
                    overview_paths.remap(path)
                });
            if changed {
                if let Err(error) = cache::save_overview(&overview_paths.cache, &overview) {
                    eprintln!("overview cache: {error}");
                }
            }
            let _ = overview_done.send(Update::Overview(overview));
        });
        let mut pending = VecDeque::new();
        let mut loaded = HashSet::new();
        while let Some(job) = next_work(&jobs, &mut pending) {
            let update = match job {
                Work::Config => Update::Config(SystemCfg::load(&paths.config).ok()),
                Work::Reload(name) => {
                    loaded.remove(&name);
                    pending.push_front(Work::Games {
                        name,
                        background: false,
                    });
                    continue;
                }
                Work::Games { name, .. } => {
                    if !loaded.insert(name.clone()) {
                        continue;
                    }
                    let mut games = systems
                        .iter()
                        .find(|system| system.name == name)
                        .map(|system| {
                            cache::load_games_with_mapper(&paths.cache, system, &|path| {
                                paths.remap(path)
                            })
                        })
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
/// Urgent selected-system/core work overtakes queued collection resolution.
/// A scan already in flight is allowed to finish; duplicate promotions are skipped.
fn next_work(jobs: &Receiver<Work>, pending: &mut VecDeque<Work>) -> Option<Work> {
    if pending.is_empty() {
        pending.push_back(jobs.recv().ok()?);
    }
    pending.extend(jobs.try_iter());
    let next = pending
        .iter()
        .position(|job| {
            !matches!(
                job,
                Work::Games {
                    background: true,
                    ..
                }
            )
        })
        .unwrap_or(0);
    pending.remove(next)
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
    SystemCfg::set_many_and_save(
        &paths.config,
        &paths.lock,
        &[
            (&rom_key(system, rom, "emulator"), emulator),
            (&rom_key(system, rom, "core"), core),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_system_overtakes_queued_collection_work() {
        let (send, jobs) = mpsc::channel();
        send.send(Work::Games {
            name: "background-one".into(),
            background: true,
        })
        .unwrap();
        send.send(Work::Games {
            name: "background-two".into(),
            background: true,
        })
        .unwrap();
        send.send(Work::Games {
            name: "selected".into(),
            background: false,
        })
        .unwrap();
        let mut pending = std::collections::VecDeque::new();
        assert!(
            matches!(next_work(&jobs,&mut pending),Some(Work::Games {name,background:false}) if name=="selected")
        );
        assert!(
            matches!(next_work(&jobs,&mut pending),Some(Work::Games {name,..}) if name=="background-one")
        );
    }
    #[test]
    fn reload_rescans_a_loaded_system_and_sees_new_art() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let dir = root.path().join("roms/snes");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Game.sfc"), b"rom").unwrap();
        let system = SystemDef {
            name: "snes".into(),
            path: dir.clone(),
            extensions: vec![".sfc".into()],
            ..Default::default()
        };
        let worker = start(paths, vec![system]);
        let games = |worker: &Worker| loop {
            if let Update::Games(_, games) = worker
                .ready
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
            {
                return games;
            }
        };
        let load = || Work::Games {
            name: "snes".into(),
            background: false,
        };
        worker.send.send(load()).unwrap();
        assert_eq!(games(&worker)[0].image, None);
        fs::create_dir_all(dir.join("images")).unwrap();
        fs::write(dir.join("images/Game.png"), b"png").unwrap();
        worker.send.send(Work::Reload("snes".into())).unwrap();
        assert!(games(&worker)[0].image.is_some());
    }
}
