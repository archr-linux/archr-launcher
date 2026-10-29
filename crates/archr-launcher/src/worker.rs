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
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    time::Duration,
};

/// automount publishes the second card's ROM folder here (a symlink in /run).
pub const SECOND_CARD: &str = "/run/archr/roms2";
/// One readlink on tmpfs: cheap enough to notice a card within seconds.
const CARD_POLL: Duration = Duration::from_secs(2);

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
    /// The second card came or went: new folders, loaded lists are stale.
    Systems(Vec<SystemDef>),
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
    attach_card(&mut systems, second_card(paths).as_deref(), paths);
    Ok(systems)
}

/// What the library follows: the second card's folder, and whether
/// unified storage joins both cards under /storage/roms (then there is no
/// second folder, and a pulled card changes /storage/roms itself).
fn cards(paths: &Paths) -> (Option<PathBuf>, bool) {
    let unified = fs::read_to_string(paths.at("/proc/mounts")).is_ok_and(|mounts| {
        mounts.lines().any(|line| {
            let mut fields = line.split_whitespace().skip(1);
            fields.next() == Some("/storage/roms") && fields.next() == Some("fuse.mergerfs")
        })
    });
    (second_card(paths), unified)
}

/// The second card's ROM folder, resolved to its real path for launching.
pub fn second_card(paths: &Paths) -> Option<PathBuf> {
    fs::canonicalize(paths.at(SECOND_CARD))
        .ok()
        .filter(|path| path.is_dir())
}

/// `/storage/roms/<system>` gets `<card>/<system>` as its second folder.
fn attach_card(systems: &mut [SystemDef], card: Option<&Path>, paths: &Paths) {
    let roms = paths.at("/storage/roms");
    for system in systems {
        system.external = card
            .zip(system.path.strip_prefix(&roms).ok())
            .map(|(card, relative)| card.join(relative));
    }
}

/// Counts for every system, off the selected-system lane.
fn refresh_overview(paths: &Paths, systems: &[SystemDef], done: &Sender<Update>) {
    let (paths, systems, done) = (paths.clone(), systems.to_vec(), done.clone());
    std::thread::spawn(move || {
        let old = cache::load_overview(&paths.cache).unwrap_or_default();
        let (overview, changed) =
            cache::refresh_overview_with_mapper(&old, &systems, &|path| paths.remap(path));
        if changed {
            if let Err(error) = cache::save_overview(&paths.cache, &overview) {
                eprintln!("overview cache: {error}");
            }
        }
        let _ = done.send(Update::Overview(overview));
    });
}

pub fn start(paths: Paths, initial: Vec<SystemDef>) -> Worker {
    let (send, jobs) = mpsc::channel();
    let (done, ready) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut systems, mut banner) = if initial.is_empty() {
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
        let mut card = cards(&paths);
        attach_card(&mut systems, card.0.as_deref(), &paths);
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
        refresh_overview(&paths, &systems, &done);
        let mut pending = VecDeque::new();
        let mut loaded = HashSet::new();
        loop {
            let job = match next_work(&jobs, &mut pending) {
                Ok(job) => job,
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    let now = cards(&paths);
                    if now == card {
                        continue;
                    }
                    log::info!("Cards (second folder, unified): {now:?}");
                    card = now;
                    attach_card(&mut systems, card.0.as_deref(), &paths);
                    loaded.clear();
                    refresh_overview(&paths, &systems, &done);
                    if done.send(Update::Systems(systems.clone())).is_err() {
                        break;
                    }
                    continue;
                }
            };
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
/// Idle for CARD_POLL: a timeout, to look at the second card.
fn next_work(
    jobs: &Receiver<Work>,
    pending: &mut VecDeque<Work>,
) -> Result<Work, RecvTimeoutError> {
    if pending.is_empty() {
        pending.push_back(jobs.recv_timeout(CARD_POLL)?);
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
    pending.remove(next).ok_or(RecvTimeoutError::Disconnected)
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
            matches!(next_work(&jobs,&mut pending),Ok(Work::Games {name,background:false}) if name=="selected")
        );
        assert!(
            matches!(next_work(&jobs,&mut pending),Ok(Work::Games {name,..}) if name=="background-one")
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
    #[test]
    fn a_second_card_adds_its_games_and_takes_them_away() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let sd1 = paths.at("/storage/roms/snes");
        let card = root.path().join("card");
        fs::create_dir_all(&sd1).unwrap();
        fs::create_dir_all(card.join("snes")).unwrap();
        fs::write(sd1.join("A.sfc"), b"rom").unwrap();
        fs::write(card.join("snes/B.sfc"), b"rom").unwrap();
        let system = SystemDef {
            name: "snes".into(),
            path: sd1.clone(),
            extensions: vec![".sfc".into()],
            ..Default::default()
        };
        let worker = start(paths.clone(), vec![system]);
        let next = |worker: &Worker| loop {
            match worker.ready.recv_timeout(Duration::from_secs(10)).unwrap() {
                Update::Games(_, games) => return Err(games),
                Update::Systems(systems) => return Ok(systems),
                _ => {}
            }
        };
        let load = |worker: &Worker| {
            worker.send.send(Work::Reload("snes".into())).unwrap();
            loop {
                if let Err(games) = next(worker) {
                    return games.into_iter().map(|g| g.path).collect::<Vec<_>>();
                }
            }
        };
        assert_eq!(load(&worker), [sd1.join("A.sfc")]);
        // automount publishes the card: the worker notices on its own.
        let link = paths.at(SECOND_CARD);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&card, &link).unwrap();
        let systems = next(&worker).unwrap();
        assert_eq!(systems[0].external, Some(card.join("snes")));
        assert_eq!(load(&worker), [sd1.join("A.sfc"), card.join("snes/B.sfc")]);
        // Unified: automount drops the link and joins the cards under
        // /storage/roms; the library reads that folder alone.
        fs::remove_file(&link).unwrap();
        fs::create_dir_all(paths.at("/proc")).unwrap();
        let mounts = paths.at("/proc/mounts");
        fs::write(&mounts, "archr-roms /storage/roms fuse.mergerfs rw 0 0\n").unwrap();
        assert_eq!(next(&worker).unwrap()[0].external, None);
        assert_eq!(load(&worker), [sd1.join("A.sfc")]);
        // The card pulled while joined: only the union goes, and the
        // library reloads although no second folder came or went.
        fs::write(&mounts, "").unwrap();
        assert_eq!(next(&worker).unwrap()[0].external, None);
        assert_eq!(load(&worker), [sd1.join("A.sfc")]);
    }
}
