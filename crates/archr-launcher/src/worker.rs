//! Background index/configuration work. Construct only after the first present.
use crate::{handoff::read_last_exit, paths::Paths, store::Store};
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
}
pub enum Update {
    Boot {
        systems: Vec<SystemDef>,
        config: Option<SystemCfg>,
        store: Box<Store>,
        banner: Option<Message>,
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
        .map_err(|error| format!("Cannot read es_systems.cfg: {error}"))?;
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
        let store = Store::load(&paths.store);
        if store.load_error.is_some() {
            banner = Some(Message::new("error.store_load"));
        }
        let boot = Update::Boot {
            systems: systems.clone(),
            config: SystemCfg::load(&paths.config).ok(),
            store: Box::new(store),
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
}
