//! Lazy library adapter for the navigation model.
use crate::{
    handoff::{catalog_command, launch_command_with, netplay_command, write_next},
    paths::Paths,
    persistence::StoreWriter,
    state::{App, DataSource, Outcome},
    store::{game_key, Store},
    worker::{self, Update, Work, Worker},
};
use archr_config::SystemCfg;
use archr_index::{
    cache::{self, Overview, SystemEntry},
    Game, SystemDef,
};
use archr_ui::{i18n::Message, startup::StartupTrace, startup_phase};
use std::collections::HashMap;

pub struct Library {
    pub paths: Paths,
    pub ready: bool,
    pub counts_known: bool,
    pub banner: Option<Message>,
    pub pending_writes: usize,
    pub write_failed: bool,
    moonlight_available: bool,
    overview: Overview,
    definitions: Vec<SystemDef>,
    games: HashMap<String, Vec<Game>>,
    origins: HashMap<String, Vec<(String, usize)>>,
    members: HashMap<String, (String, usize)>,
    requested: HashMap<String, bool>,
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
        // Tiny file, read in the startup worker alongside video setup: the first
        // frame already shows the user's order, and the Store is complete before
        // any input can change it.
        let store = Store::load(&paths.store);
        startup_phase!(trace, "store read");
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
        let banner = banner.or_else(|| {
            store
                .load_error
                .is_some()
                .then(|| Message::new("error.store_load"))
        });
        let mut overview = cached.unwrap_or_else(|| Overview {
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
        let moonlight_available = paths.at("/usr/bin/moonlight").is_file();
        crate::moonlight::collection(&mut overview, moonlight_available);
        Self {
            moonlight_available,
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
            requested: HashMap::new(),
            members: HashMap::new(),
            config: None,
            overrides: HashMap::new(),
            store,
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
        if let Some(keys) = self.collection_keys(system) {
            let names: Vec<_> = keys
                .iter()
                .filter_map(|key| key.split_once('/').map(|(name, _)| name.to_owned()))
                .collect();
            for name in names {
                self.ensure_system(&name, true);
            }
            if !self.origins.contains_key(system) {
                self.origins.insert(system.to_owned(), Vec::new());
                self.refresh_collection(system);
            }
        } else if !system.starts_with('@') {
            self.ensure_system(system, false);
        }
    }
    fn ensure_system(&mut self, system: &str, background: bool) {
        if self.games.contains_key(system)
            || self
                .requested
                .get(system)
                .is_some_and(|urgent| background || *urgent)
        {
            return;
        }
        let work = Work::Games {
            name: system.to_owned(),
            background,
        };
        if self.send(work).is_ok() {
            self.requested.insert(system.to_owned(), !background);
        }
    }
    /// After scraping: rescan the systems already loaded, off-thread. The old
    /// list stays on screen until the new one arrives; the media fingerprint
    /// changed, so the worker rebuilds the cache instead of reusing it.
    pub fn reload(&mut self, systems: &[String]) {
        for name in systems {
            if (self.games.contains_key(name) || self.requested.contains_key(name))
                && self.send(Work::Reload(name.clone())).is_ok()
            {
                self.requested.insert(name.clone(), true);
            }
        }
    }
    fn send(&self, work: Work) -> Result<(), String> {
        let worker = self.worker.as_ref().ok_or("error.index")?;
        worker.send.send(work).map_err(|error| error.to_string())
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
                    banner,
                } => {
                    self.definitions = systems;
                    self.config = config;
                    if self.store.migrate(&self.definitions) {
                        self.queue_store();
                    }
                    if banner.is_some() {
                        self.banner = banner;
                    }
                    self.ready = true;
                }
                Update::Overview(overview) => {
                    self.overview = overview;
                    crate::moonlight::collection(&mut self.overview, self.moonlight_available);
                    self.counts_known = true;
                }
                Update::Games(system, mut games) => {
                    if let Some(definition) = self.definitions.iter().find(|sys| sys.name == system)
                    {
                        if self.store.import_games(definition, &games) {
                            self.queue_store();
                        }
                        for (index, game) in games.iter_mut().enumerate() {
                            if let Some(key) = game_key(definition, &game.path) {
                                game.favorite = self.store.is_favorite(&key);
                                self.members.insert(key, (system.clone(), index));
                            }
                        }
                    }
                    self.games.insert(system, games);
                    for name in ["@favorites", "@recent"] {
                        self.refresh_collection(name);
                    }
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
        changed
    }
    pub fn handle(&mut self, outcome: Outcome, controllers: &str) -> Result<bool, String> {
        match outcome {
            Outcome::None | Outcome::Settings | Outcome::OpenNetplay | Outcome::Moonlight => {}
            Outcome::ToggleFavorite { system, game } => {
                let key = self.game_key(&system, game).ok_or("error.game_missing")?;
                self.store.toggle_favorite(&key);
                let favorite = self.store.is_favorite(&key);
                self.games.get_mut(&system).unwrap()[game].favorite = favorite;
                if let Some(index) = self.origins.get("@recent").and_then(|origins| {
                    origins
                        .iter()
                        .position(|origin| origin.0 == system && origin.1 == game)
                }) {
                    self.games.get_mut("@recent").unwrap()[index].favorite = favorite;
                }
                self.queue_store();
                self.refresh_collection("@favorites");
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
                self.send(Work::Core {
                    system,
                    rom,
                    emulator,
                    core,
                })?;
                self.pending_writes += 1;
                self.write_failed = false;
            }
            Outcome::MoonlightStream(stream) => {
                let command = crate::moonlight::stream_command(&self.paths, &stream)?;
                self.flush_store()?;
                write_next(&self.paths.run, &command).map_err(|error| error.to_string())?;
                return Ok(true);
            }
            Outcome::Launch { system, game } => {
                let command = self.launch_command(&system, game, controllers)?;
                let key = self.game_key(&system, game).ok_or("error.game_missing")?;
                if !self.is_tools(&system) {
                    self.store.push_recent(&key);
                    self.refresh_collection("@recent");
                }
                self.flush_store()?;
                write_next(&self.paths.run, &command).map_err(|error| error.to_string())?;
                return Ok(true);
            }
            Outcome::Netplay(join) => {
                let nickname = self
                    .config
                    .as_ref()
                    .and_then(|config| config.get_setting("global.netplay.nickname", None, None))
                    .unwrap_or_else(|| "ArchR Player".to_owned());
                let runemu = self.paths.at("/usr/bin/runemu.sh");
                let game = (&*join.system, &*join.core, &*join.emulator, controllers);
                let peer = (&*join.ip, join.port, &*nickname, &*join.session);
                let command = netplay_command(&runemu, &join.rom, game, peer, join.spectate)
                    .map_err(|error| error.to_string())?;
                self.flush_store()?;
                write_next(&self.paths.run, &command).map_err(|error| error.to_string())?;
                return Ok(true);
            }
            Outcome::SetOrder(order) => {
                // The default arrangement is stored as no custom order.
                if order.is_empty() || order == App::default_rows(self) {
                    self.store.clear_order();
                } else {
                    let overview = &self.overview;
                    let exist = |id: &str| {
                        id.starts_with('@') || overview.systems.iter().any(|s| s.name == id)
                    };
                    self.store.set_order(order, exist);
                }
                self.queue_store();
            }
            Outcome::Action(_) | Outcome::ToggleWifi => return Err("error.operation".to_owned()),
        }
        Ok(false)
    }
    /// Emulated systems use the runemu contract; systems without emulators
    /// (tools, moonlight) run their catalog `<command>`.
    fn launch_command(
        &self,
        system: &str,
        game: usize,
        controllers: &str,
    ) -> Result<String, String> {
        let rom = &self
            .games(system)
            .get(game)
            .ok_or("error.game_missing")?
            .path;
        if let Some((emulator, core)) = self.current_core(system, game) {
            let runemu = self.paths.at("/usr/bin/runemu.sh");
            return launch_command_with(&runemu, rom, system, &core, &emulator, controllers)
                .map_err(|error| error.to_string());
        }
        let template = self
            .definition(system)
            .map(|definition| definition.command.as_str())
            .filter(|command| !command.is_empty())
            .ok_or("error.no_core")?;
        catalog_command(template, rom, system).map_err(|error| {
            log::warn!("Cannot launch {system}: {error}");
            "error.no_core".to_owned()
        })
    }
    fn queue_store(&self) {
        if let Some(writer) = &self.store_writer {
            writer.queue(self.store.clone());
        }
    }
    /// Closing also waits for queued favorite/import snapshots.
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
    fn definition(&self, name: &str) -> Option<&SystemDef> {
        self.definitions.iter().find(|system| system.name == name)
    }
    fn game_key(&self, system: &str, index: usize) -> Option<String> {
        game_key(
            self.definition(system)?,
            &self.games(system).get(index)?.path,
        )
    }
    fn refresh_collection(&mut self, name: &str) {
        if !self.origins.contains_key(name) {
            return;
        }
        let keys = self.collection_keys(name).unwrap();
        let desired: Vec<_> = keys
            .iter()
            .filter_map(|key| self.members.get(key).cloned())
            .collect();
        let mut games = self.games.remove(name).unwrap_or_default();
        let origins = self.origins.get_mut(name).unwrap();
        for (index, origin) in desired.iter().enumerate() {
            if origins.get(index) == Some(origin) {
                continue;
            }
            if let Some(offset) = origins[index..].iter().position(|entry| entry == origin) {
                // Keep payloads in place; only a newly visible member needs a clone.
                origins.swap(index, index + offset);
                games.swap(index, index + offset);
            } else {
                origins.insert(index, origin.clone());
                games.insert(index, self.games[&origin.0][origin.1].clone());
            }
        }
        origins.truncate(desired.len());
        games.truncate(desired.len());
        self.games.insert(name.to_owned(), games);
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
        self.games.get(system).map_or(&[], Vec::as_slice)
    }
    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        if system.starts_with('@') {
            self.origins.get(system)?.get(game).cloned()
        } else {
            self.games(system).get(game)?;
            Some((system.to_owned(), game))
        }
    }
    fn manufacturer(&self, system: &str) -> Option<(&str, &str)> {
        let def = self.definition(system)?;
        Some((&def.manufacturer, &def.release))
    }
    fn system_order(&self) -> &[String] {
        self.store.order()
    }
    fn is_tools(&self, system: &str) -> bool {
        self.definition(system)
            .is_some_and(|def| def.emulators.is_empty() && !def.command.is_empty())
    }
    fn favorite_count(&self, system: &str) -> usize {
        self.store
            .favorites()
            .iter()
            .filter(|key| key.split_once('/').is_some_and(|(name, _)| name == system))
            .count()
    }
    fn cores(&self, system: &str) -> Vec<(String, String)> {
        self.definition(system)
            .into_iter()
            .flat_map(|definition| &definition.emulators)
            .flat_map(|emulator| {
                emulator
                    .cores
                    .iter()
                    .map(|core| (emulator.name.clone(), core.name.clone()))
            })
            .collect()
    }
    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)> {
        let game = self.games(system).get(game)?;
        let rom = game.path.file_name()?.to_str()?;
        if let Some(pair) = self.overrides.get(&(system.to_owned(), rom.to_owned())) {
            return Some(pair.clone());
        }
        let default = self
            .definition(system)
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
