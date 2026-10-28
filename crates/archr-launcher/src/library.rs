//! Lazy library adapter for the navigation model.
use crate::{
    handoff::{launch_command_with, write_next, NextCmd},
    paths::Paths,
    persistence::StoreWriter,
    state::{DataSource, Outcome},
    store::Store,
    worker::{self, Update, Work, Worker},
};
use archr_config::SystemCfg;
use archr_index::{
    cache::{self, Overview, SystemEntry},
    Game, SystemDef,
};
use archr_ui::{i18n::Message, startup::StartupTrace, startup_phase};
use std::collections::{HashMap, HashSet};

pub struct Library {
    pub paths: Paths,
    pub ready: bool,
    pub counts_known: bool,
    pub banner: Option<Message>,
    pub pending_writes: usize,
    pub write_failed: bool,
    overview: Overview,
    definitions: Vec<SystemDef>,
    games: HashMap<String, Vec<Game>>,
    origins: HashMap<String, Vec<(String, usize)>>,
    requested: HashSet<String>,
    urgent: HashSet<String>,
    collections: HashSet<String>,
    config: Option<SystemCfg>,
    overrides: HashMap<(String, String), (String, String)>,
    store: Store,
    store_writer: Option<StoreWriter>,
    worker: Option<Worker>,
}
impl Library {
    /// Only the overview and (on a cold start) system names are read here.
    pub fn initial(paths: Paths) -> Self {
        Self::initial_traced(paths, StartupTrace::default())
    }
    pub fn initial_traced(paths: Paths, trace: StartupTrace) -> Self {
        let cached = cache::load_overview(&paths.cache);
        startup_phase!(trace, "overview cache read");
        let counts_known = cached.is_some();
        let (definitions, banner) = if counts_known {
            (Vec::new(), None)
        } else {
            let definitions = worker::definitions(&paths);
            startup_phase!(trace, "system definitions read");
            match definitions {
                Ok(defs) => (defs, None),
                Err(error) => {
                    log::warn!("Cannot load systems: {error}");
                    (Vec::new(), Some(Message::new("error.systems")))
                }
            }
        };
        let overview = cached.unwrap_or_else(|| Overview {
            systems: definitions
                .iter()
                .map(|system| SystemEntry {
                    name: system.name.clone(),
                    fullname: system.fullname.clone(),
                    game_count: 1,
                    dir_mtime: 0,
                    source_fingerprint: 0,
                })
                .collect(),
        });
        Self {
            paths,
            ready: false,
            counts_known,
            banner,
            pending_writes: 0,
            write_failed: false,
            overview,
            definitions,
            games: HashMap::new(),
            origins: HashMap::new(),
            requested: HashSet::new(),
            urgent: HashSet::new(),
            collections: HashSet::new(),
            config: None,
            overrides: HashMap::new(),
            store: Store::default(),
            store_writer: None,
            worker: None,
        }
    }
    pub fn start(&mut self) {
        self.store_writer = Some(StoreWriter::new(self.paths.store.clone()));
        self.worker = Some(worker::start(self.paths.clone(), self.definitions.clone()));
    }
    pub fn ensure(&mut self, system: &str) {
        if !self.ready {
            return;
        }
        if system.starts_with('@') {
            let Some(keys) = self.collection_keys(system) else {
                return;
            };
            let names: Vec<_> = keys
                .iter()
                .filter_map(|key| key.split_once('/').map(|(name, _)| name.to_owned()))
                .collect();
            let entered = self.collections.insert(system.to_owned());
            for name in names {
                self.ensure_system(&name, true);
            }
            if entered {
                self.refresh_collections();
            }
        } else {
            self.ensure_system(system, false);
        }
    }
    fn ensure_system(&mut self, system: &str, background: bool) {
        if self.games.contains_key(system)
            || (if background {
                &self.requested
            } else {
                &self.urgent
            })
            .contains(system)
        {
            return;
        }
        if let Some(worker) = &self.worker {
            if worker
                .send
                .send(Work::Games {
                    name: system.to_owned(),
                    background,
                })
                .is_ok()
            {
                self.requested.insert(system.to_owned());
                if !background {
                    self.urgent.insert(system.to_owned());
                }
            }
        }
    }
    fn collection_keys(&self, name: &str) -> Option<&[String]> {
        match name {
            "@favorites" => Some(self.store.favorites()),
            "@recent" => Some(self.store.recent()),
            _ => None,
        }
    }
    pub fn loading(&self, system: &str) -> bool {
        if let Some(keys) = self.collection_keys(system) {
            keys.iter()
                .filter_map(|key| key.split_once('/'))
                .any(|(name, _)| !self.games.contains_key(name))
        } else {
            !self.games.contains_key(system)
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Some(update) = self
            .worker
            .as_ref()
            .and_then(|worker| worker.ready.try_recv().ok())
        {
            match update {
                Update::Boot {
                    systems,
                    config,
                    store,
                    banner,
                } => {
                    self.definitions = systems;
                    self.config = config;
                    self.store = store;
                    if banner.is_some() {
                        self.banner = banner;
                    }
                    self.ready = true;
                }
                Update::Overview(overview) => {
                    self.overview = overview;
                    self.counts_known = true;
                }
                Update::Games(system, games) => {
                    if self.store.import_games(&system, &games) {
                        self.queue_store();
                    }
                    self.games.insert(system, games);
                }
                Update::CoreSaved(system, rom, result) => {
                    self.pending_writes = self.pending_writes.saturating_sub(1);
                    match result {
                        Ok(config) => self.config = config,
                        Err(error) => {
                            self.overrides.remove(&(system, rom));
                            log::warn!("Cannot save core: {error}");
                            self.banner = Some(Message::new("error.core_save"));
                            self.write_failed = true;
                        }
                    }
                }
            }
            changed = true;
        }
        while let Some(error) = self.store_writer.as_ref().and_then(StoreWriter::error) {
            log::warn!("Cannot save favorites/history: {error}");
            self.banner = Some(Message::new("error.store_save"));
            changed = true;
        }
        if changed {
            self.refresh_collections();
        }
        changed
    }
    pub fn handle(&mut self, outcome: Outcome, controllers: &str) -> Result<bool, String> {
        match outcome {
            Outcome::None => {}
            Outcome::ToggleFavorite { system, game } => {
                let key = self.game_key(&system, game).ok_or("error.game_missing")?;
                self.store.toggle_favorite(&key);
                self.queue_store();
                self.refresh_collections();
            }
            Outcome::SetCore {
                system,
                game,
                emulator,
                core,
            } => {
                let rom = self
                    .games(&system)
                    .get(game)
                    .and_then(|game| game.path.file_name())
                    .and_then(|name| name.to_str())
                    .ok_or("error.rom_name")?
                    .to_owned();
                self.overrides.insert(
                    (system.clone(), rom.clone()),
                    (emulator.clone(), core.clone()),
                );
                self.worker
                    .as_ref()
                    .ok_or("error.index")?
                    .send
                    .send(Work::Core {
                        system,
                        rom,
                        emulator,
                        core,
                    })
                    .map_err(|error| error.to_string())?;
                self.pending_writes += 1;
                self.write_failed = false;
            }
            Outcome::Launch { system, game } => {
                let rom = &self
                    .games(&system)
                    .get(game)
                    .ok_or("error.game_missing")?
                    .path;
                let (emulator, core) = self.current_core(&system, game).ok_or("error.no_core")?;
                let command = launch_command_with(
                    &self.paths.at("/usr/bin/runemu.sh"),
                    rom,
                    &system,
                    &core,
                    &emulator,
                    controllers,
                )
                .map_err(|error| error.to_string())?;
                let key = self.game_key(&system, game).ok_or("error.game_missing")?;
                self.store.push_recent(&key);
                self.flush_store()?;
                write_next(&self.paths.run, &NextCmd::Game(command))
                    .map_err(|error| error.to_string())?;
                return Ok(true);
            }
            Outcome::Action(action) => {
                self.flush_store()?;
                write_next(&self.paths.run, &NextCmd::Action(action))
                    .map_err(|error| error.to_string())?;
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn queue_store(&self) {
        if let Some(writer) = &self.store_writer {
            writer.queue(self.store.clone());
        }
    }
    /// Windowed close also waits for queued favorite/import snapshots.
    pub fn finish(&self) -> Result<(), String> {
        if self.ready {
            self.flush_store()
        } else {
            Ok(())
        }
    }
    fn flush_store(&self) -> Result<(), String> {
        if !self.ready {
            return Err("error.loading".to_owned());
        }
        self.store_writer
            .as_ref()
            .ok_or("error.persistence")?
            .flush(self.store.clone())
    }
    fn game_key(&self, system: &str, index: usize) -> Option<String> {
        self.games(system)
            .get(index)?
            .path
            .to_str()
            .map(|path| format!("{system}/{path}"))
    }
    fn refresh_collections(&mut self) {
        for (system, games) in &mut self.games {
            if system.starts_with('@') {
                continue;
            }
            for game in games {
                game.favorite = self
                    .store
                    .is_favorite(&format!("{system}/{}", game.path.display()));
            }
        }
        for (name, keys) in [
            ("@favorites", self.store.favorites()),
            ("@recent", self.store.recent()),
        ] {
            if !self.collections.contains(name) {
                continue;
            }
            let mut collection = Vec::new();
            let mut origins = Vec::new();
            for key in keys {
                let Some((system, path)) = key.split_once('/') else {
                    continue;
                };
                if let Some((index, game)) = self.games.get(system).and_then(|games| {
                    games
                        .iter()
                        .enumerate()
                        .find(|(_, game)| game.path.to_str() == Some(path))
                }) {
                    collection.push(game.clone());
                    origins.push((system.to_owned(), index));
                }
            }
            self.games.insert(name.to_owned(), collection);
            self.origins.insert(name.to_owned(), origins);
        }
    }
}
impl DataSource for Library {
    fn systems(&self) -> &[SystemEntry] {
        &self.overview.systems
    }
    fn collection_len(&self, collection: &str) -> usize {
        self.collection_keys(collection).map_or(0, <[String]>::len)
    }
    fn games(&self, system: &str) -> &[Game] {
        self.games
            .get(system)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        if system.starts_with('@') {
            self.origins.get(system)?.get(game).cloned()
        } else {
            self.games(system)
                .get(game)
                .map(|_| (system.to_owned(), game))
        }
    }
    fn cores(&self, system: &str) -> Vec<(String, String)> {
        self.definitions
            .iter()
            .find(|definition| definition.name == system)
            .map(|definition| {
                definition
                    .emulators
                    .iter()
                    .flat_map(|emulator| {
                        emulator
                            .cores
                            .iter()
                            .map(|core| (emulator.name.clone(), core.name.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)> {
        let game = self.games(system).get(game)?;
        let rom = game.path.file_name()?.to_str()?;
        if let Some(pair) = self.overrides.get(&(system.to_owned(), rom.to_owned())) {
            return Some(pair.clone());
        }
        let default = self
            .definitions
            .iter()
            .find(|definition| definition.name == system)
            .and_then(SystemDef::default_emulator_core);
        let get = |key| {
            self.config
                .as_ref()
                .and_then(|cfg| cfg.get_local(system, Some(rom), key))
        };
        Some((
            get("emulator").or_else(|| default.map(|pair| pair.0.to_owned()))?,
            get("core").or_else(|| default.map(|pair| pair.1.to_owned()))?,
        ))
    }
}
