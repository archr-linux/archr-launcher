//! Lazy library adapter for the navigation model.
use crate::{
    handoff::{launch_command_with, write_next, NextCmd},
    paths::Paths,
    state::{DataSource, Outcome},
    store::Store,
    worker::{self, Update, Work, Worker},
};
use archr_config::SystemCfg;
use archr_index::{
    cache::{self, Overview, SystemEntry},
    Game, SystemDef,
};
use std::collections::{HashMap, HashSet};

pub struct Library {
    pub paths: Paths,
    pub ready: bool,
    pub counts_known: bool,
    pub banner: Option<String>,
    pub pending_writes: usize,
    pub write_failed: bool,
    overview: Overview,
    definitions: Vec<SystemDef>,
    games: HashMap<String, Vec<Game>>,
    origins: HashMap<String, Vec<(String, usize)>>,
    requested: HashSet<String>,
    config: Option<SystemCfg>,
    overrides: HashMap<(String, String), (String, String)>,
    store: Store,
    worker: Option<Worker>,
}
impl Library {
    /// Only the overview and (on a cold start) system names are read here.
    pub fn initial(paths: Paths) -> Self {
        let cached = cache::load_overview(&paths.cache);
        let counts_known = cached.is_some();
        let (definitions, banner) = if counts_known {
            (Vec::new(), None)
        } else {
            match worker::definitions(&paths) {
                Ok(defs) => (defs, None),
                Err(error) => (Vec::new(), Some(error)),
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
            config: None,
            overrides: HashMap::new(),
            store: Store::default(),
            worker: None,
        }
    }
    pub fn start(&mut self) {
        self.worker = Some(worker::start(self.paths.clone(), self.definitions.clone()));
    }
    pub fn ensure(&mut self, system: &str) {
        if !self.ready || system.starts_with('@') || self.requested.contains(system) {
            return;
        }
        if let Some(worker) = &self.worker {
            if worker.send.send(Work::Games(system.to_owned())).is_ok() {
                self.requested.insert(system.to_owned());
            }
        }
    }
    pub fn loading(&self, system: &str) -> bool {
        !system.starts_with('@') && !self.games.contains_key(system)
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
                        if let Err(error) = self.store.save(&self.paths.store) {
                            self.banner =
                                Some(format!("Não foi possível importar favoritos: {error}"));
                        }
                    }
                    self.games.insert(system, games);
                }
                Update::CoreSaved(system, rom, result) => {
                    self.pending_writes = self.pending_writes.saturating_sub(1);
                    match result {
                        Ok(config) => self.config = config,
                        Err(error) => {
                            self.overrides.remove(&(system, rom));
                            self.banner =
                                Some(format!("Não foi possível salvar o núcleo:\n{error}"));
                            self.write_failed = true;
                        }
                    }
                }
            }
            changed = true;
        }
        if changed {
            let names: Vec<_> = self
                .store
                .favorites()
                .iter()
                .chain(self.store.recent())
                .filter_map(|key| key.split_once('/').map(|(system, _)| system.to_owned()))
                .collect();
            for name in names {
                self.ensure(&name);
            }
            self.refresh_collections();
        }
        changed
    }
    pub fn handle(&mut self, outcome: Outcome, controllers: &str) -> Result<bool, String> {
        match outcome {
            Outcome::None => {}
            Outcome::ToggleFavorite { system, game } => {
                let key = self.game_key(&system, game).ok_or("Jogo não encontrado")?;
                self.store.toggle_favorite(&key);
                self.store
                    .save(&self.paths.store)
                    .map_err(|error| error.to_string())?;
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
                    .ok_or("Nome da ROM inválido")?
                    .to_owned();
                self.overrides.insert(
                    (system.clone(), rom.clone()),
                    (emulator.clone(), core.clone()),
                );
                self.worker
                    .as_ref()
                    .ok_or("Índice não inicializado")?
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
                    .ok_or("Jogo não encontrado")?
                    .path;
                let (emulator, core) = self
                    .current_core(&system, game)
                    .ok_or("Nenhum emulador/núcleo configurado")?;
                let command = launch_command_with(
                    &self.paths.at("/usr/bin/runemu.sh"),
                    rom,
                    &system,
                    &core,
                    &emulator,
                    controllers,
                )
                .map_err(|error| error.to_string())?;
                let key = self.game_key(&system, game).ok_or("Jogo não encontrado")?;
                self.store.push_recent(&key);
                self.store
                    .save(&self.paths.store)
                    .map_err(|error| error.to_string())?;
                write_next(&self.paths.run, &NextCmd::Game(command))
                    .map_err(|error| error.to_string())?;
                return Ok(true);
            }
            Outcome::Action(action) => {
                write_next(&self.paths.run, &NextCmd::Action(action))
                    .map_err(|error| error.to_string())?;
                return Ok(true);
            }
        }
        Ok(false)
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
