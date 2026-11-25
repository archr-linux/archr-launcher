//! Per-system emulator, core and RetroArch options on the Games page.
//! Catalog pairs and saved values come from the host; switching systems is local.
use super::{Effect, Id, Settings};
use crate::state::Button;
use std::collections::BTreeMap;

/// Heading key replaced by the shown system's name, when it has one.
pub(crate) const SYSTEM_SECTION: &str = "settings.system_options";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameCore {
    pub emulator: String,
    pub core: String,
    /// Marked default in the systems catalog.
    pub default: bool,
    /// Option rows this pair accepts, from `es_features.cfg`.
    pub options: Vec<Id>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameSystem {
    pub name: String,
    pub fullname: String,
    /// Who made it, as the systems catalog says; empty when unknown.
    pub manufacturer: String,
    /// Games the library holds for it.
    pub games: u32,
    /// Catalog pairs in order; never empty.
    pub cores: Vec<GameCore>,
    /// Values found in system.cfg for this system, by row.
    pub saved: BTreeMap<Id, String>,
}
impl GameSystem {
    /// The catalog's launch default, as the library resolves it.
    fn default_pair(&self) -> Option<&GameCore> {
        self.cores
            .iter()
            .find(|pair| pair.default)
            .or(self.cores.first())
    }
    /// The core an emulator starts with when it is chosen.
    pub fn first_core(&self, emulator: &str) -> Option<&str> {
        let mut cores = self.cores.iter().filter(|pair| pair.emulator == emulator);
        cores
            .clone()
            .find(|pair| pair.default)
            .or_else(|| cores.next())
            .map(|pair| pair.core.as_str())
    }
}
#[derive(Clone, Debug, Default)]
pub struct Games {
    pub systems: Vec<GameSystem>,
    current: usize,
    /// System the launcher was browsing when the settings opened.
    wanted: Option<String>,
}
impl Games {
    /// Rows bound to the shown system, in page order.
    pub const ROWS: [Id; 10] = [
        Id::Emulator,
        Id::Core,
        Id::Ratio,
        Id::Filter,
        Id::Autosave,
        Id::StateSlot,
        Id::MangohudGame,
        Id::GraphicsBackend,
        Id::NpuUpscale,
        Id::GameSystem,
    ];
    pub fn current(&self) -> Option<&GameSystem> {
        self.systems.get(self.current)
    }
    pub fn name(&self) -> &str {
        self.current().map_or("", |s| s.name.as_str())
    }
    pub fn fullname(&self) -> &str {
        self.current().map_or("", |s| s.fullname.as_str())
    }
    /// Show this system now, or once the list arrives.
    pub fn focus(&mut self, name: &str) {
        self.wanted = Some(name.into());
        if let Some(n) = self.systems.iter().position(|s| s.name == name) {
            self.current = n;
        }
    }
    /// A fresh list keeps the shown system, or the one asked for.
    pub fn replace(&mut self, systems: Vec<GameSystem>) {
        let keep = self
            .wanted
            .clone()
            .unwrap_or_else(|| self.name().to_owned());
        self.systems = systems;
        self.current = self
            .systems
            .iter()
            .position(|s| s.name == keep)
            .unwrap_or(0);
    }
    /// A confirmed write; choosing an emulator also chose its first core.
    pub(super) fn record(&mut self, id: Id, value: &str) {
        let Some(system) = self.systems.get_mut(self.current) else {
            return;
        };
        if id == Id::Emulator {
            if let Some(core) = system.first_core(value).map(str::to_owned) {
                system.saved.insert(Id::Core, core);
            }
        }
        system.saved.insert(id, value.to_owned());
    }
    pub(super) fn shows(&self, id: Id, state: &Settings) -> bool {
        let Some(system) = self.current() else {
            return false;
        };
        if id == Id::NpuUpscale {
            return state.npu_systems.contains(&system.name);
        }
        matches!(
            id,
            Id::Emulator | Id::Core | Id::MangohudGame | Id::GraphicsBackend | Id::GameSystem
        ) || system.cores.iter().any(|pair| {
            pair.emulator == state.value(Id::Emulator)
                && pair.core == state.value(Id::Core)
                && pair.options.contains(&id)
        })
    }
}
impl Settings {
    pub fn focus_system(&mut self, name: &str) {
        self.games.focus(name);
        self.sync_games();
    }
    /// Shown values and choices follow the current system.
    pub fn sync_games(&mut self) {
        let Some((system, default)) = self
            .games
            .current()
            .and_then(|system| Some((system, system.default_pair()?)))
        else {
            return;
        };
        let saved = |id: Id| system.saved.get(&id).cloned();
        // The same precedence as the library's launch: saved, else catalog.
        let emulator = saved(Id::Emulator).unwrap_or_else(|| default.emulator.clone());
        let core = saved(Id::Core).unwrap_or_else(|| default.core.clone());
        let mut emulators: Vec<String> = Vec::new();
        for pair in &system.cores {
            if !emulators.contains(&pair.emulator) {
                emulators.push(pair.emulator.clone());
            }
        }
        let cores = system
            .cores
            .iter()
            .filter(|pair| pair.emulator == emulator)
            .map(|pair| pair.core.clone())
            .collect();
        let mut values = vec![(Id::Emulator, emulator), (Id::Core, core)];
        for id in [
            Id::Ratio,
            Id::Filter,
            Id::Autosave,
            Id::StateSlot,
            Id::MangohudGame,
            Id::GraphicsBackend,
            Id::NpuUpscale,
        ] {
            values.push((id, saved(id).unwrap_or_else(|| id.default_value().into())));
        }
        self.values.extend(values);
        self.options.insert(Id::Emulator, emulators);
        self.options.insert(Id::Core, cores);
        self.labels
            .insert((Id::Emulator, "retroarch".into()), "RetroArch".into());
    }
    /// L/R or A on the System row switch locally, never mid-write; A opens
    /// the list instead when it is long.
    pub(super) fn games_input(&mut self, button: Button) -> Option<Effect> {
        let row = *self.visible_rows().get(self.cursor)?;
        if row.id != Id::GameSystem || !matches!(button, Button::A | Button::L | Button::R) {
            return None;
        }
        let count = self.games.systems.len();
        if self.pending.is_some() || self.loading || count == 0 {
            return Some(Effect::None);
        }
        if button == Button::A && self.open_picker(row) {
            return Some(Effect::None);
        }
        let step = if button == Button::L { count - 1 } else { 1 };
        self.show_system((self.games.current + step) % count);
        Some(Effect::None)
    }
    /// The page shows the system at `index` of the list.
    pub(super) fn show_system(&mut self, index: usize) {
        self.games.current = index.min(self.games.systems.len().saturating_sub(1));
        self.games.wanted = None;
        self.sync_games();
        // The option rows above may come and go: stay on this row.
        self.cursor = self
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::GameSystem)
            .unwrap_or(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Page;
    fn pair(emulator: &str, core: &str, default: bool, options: &[Id]) -> GameCore {
        GameCore {
            emulator: emulator.into(),
            core: core.into(),
            default,
            options: options.to_vec(),
        }
    }
    const ALL: &[Id] = &[Id::Ratio, Id::Filter, Id::Autosave, Id::StateSlot];
    fn state() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.games.replace(vec![
            GameSystem {
                name: "snes".into(),
                fullname: "Super Nintendo".into(),
                cores: vec![
                    pair("retroarch", "snes9x", false, ALL),
                    pair("retroarch", "bsnes", true, ALL),
                    pair("standalone", "snes-sa", false, &[]),
                ],
                saved: BTreeMap::from([(Id::Ratio, "4/3".into())]),
                ..GameSystem::default()
            },
            GameSystem {
                name: "n64".into(),
                fullname: "Nintendo 64".into(),
                cores: vec![pair("mupen64plus", "glide64mk2", false, &[])],
                ..GameSystem::default()
            },
        ]);
        state.sync_games();
        state.open(Page::Games);
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }
    #[test]
    fn unsaved_systems_show_the_catalog_default_and_saved_options() {
        let state = state();
        assert_eq!(state.value(Id::Emulator), "retroarch");
        assert_eq!(state.value(Id::Core), "bsnes");
        assert_eq!(state.value(Id::Ratio), "4/3");
        // Unsaved RetroArch options stay RetroArch's.
        assert_eq!(state.value(Id::Filter), "");
        assert_eq!(state.options[&Id::Core], ["snes9x", "bsnes"]);
        assert_eq!(state.options[&Id::Emulator], ["retroarch", "standalone"]);
    }
    #[test]
    fn rows_follow_the_features_of_the_chosen_pair() {
        let mut state = state();
        assert!(ids(&state).contains(&Id::Ratio));
        state.complete(Id::Emulator, Ok("standalone".into()));
        assert_eq!(state.value(Id::Core), "snes-sa");
        assert!(state.fixed(Id::Core));
        assert!(!ids(&state).contains(&Id::Ratio));
        assert!(!ids(&state).contains(&Id::Autosave));
        assert!(ids(&state).contains(&Id::GameSystem));
    }
    #[test]
    fn npu_upscaling_shows_only_for_the_systems_the_npu_lists() {
        let mut state = state();
        // No NPU: no row on any system.
        assert!(!ids(&state).contains(&Id::NpuUpscale));
        state.npu_systems = vec!["n64".into()];
        assert!(!ids(&state).contains(&Id::NpuUpscale), "snes is not listed");
        state.show_system(1);
        assert!(ids(&state).contains(&Id::NpuUpscale));
        assert_eq!(state.value(Id::NpuUpscale), "auto");
        state.cursor = ids(&state)
            .iter()
            .position(|id| *id == Id::NpuUpscale)
            .unwrap();
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::NpuUpscale, "x2".into())
        );
        state.complete(Id::NpuUpscale, Ok("x2".into()));
        assert_eq!(state.games.current().unwrap().saved[&Id::NpuUpscale], "x2");
        state.show_system(0);
        state.show_system(1);
        assert_eq!(state.value(Id::NpuUpscale), "x2");
    }
    #[test]
    fn switching_systems_is_local_and_waits_for_a_pending_write() {
        let mut state = state();
        state.cursor = ids(&state)
            .iter()
            .position(|id| *id == Id::GameSystem)
            .unwrap();
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(state.games.name(), "n64");
        assert_eq!(state.value(Id::Ratio), "");
        assert_eq!(state.visible_rows()[state.cursor].id, Id::GameSystem);
        state.pending = Some(Id::Core);
        state.handle(Button::L);
        assert_eq!(state.games.name(), "n64");
        state.pending = None;
        state.handle(Button::L);
        assert_eq!(state.games.name(), "snes");
        assert_eq!(state.value(Id::Ratio), "4/3");
    }
    #[test]
    fn option_rows_produce_writes_for_the_host() {
        let mut state = state();
        state.cursor = ids(&state).iter().position(|id| *id == Id::Ratio).unwrap();
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::Ratio, "3/2".into())
        );
        state.complete(Id::Ratio, Ok("3/2".into()));
        state.handle(Button::Down);
        state.games.focus("n64");
        state.games.focus("snes");
        state.sync_games();
        assert_eq!(state.value(Id::Ratio), "3/2");
    }
    #[test]
    fn a_fresh_list_keeps_the_focused_system() {
        let mut state = state();
        state.focus_system("n64");
        let systems = state.games.systems.clone();
        state.games.replace(systems);
        assert_eq!(state.games.name(), "n64");
        assert_eq!(state.games.fullname(), "Nintendo 64");
    }
    #[test]
    fn without_a_catalog_only_the_launcher_wide_rows_remain() {
        let mut state = Settings::default();
        state.open(Page::Games);
        assert_eq!(
            ids(&state),
            [
                Id::PreviewVideo,
                Id::ResumeLink,
                Id::NetplayLink,
                Id::ScraperLink,
                Id::MangohudLink,
                Id::BiosLink
            ]
        );
    }
    #[test]
    fn games_page_cycles_the_system_override() {
        let mut state = state();
        state.mangohud_supported = true;
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::MangohudGame)
            .unwrap();
        assert_eq!(state.value(Id::MangohudGame), "");
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::MangohudGame, "1".into())
        );
        state.complete(Id::MangohudGame, Ok("1".into()));
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::MangohudGame, "0".into())
        );
    }
    #[test]
    fn the_renderer_offers_vulkan_only_on_panfrost() {
        let mut state = state();
        state.cursor = ids(&state)
            .iter()
            .position(|id| *id == Id::GraphicsBackend)
            .unwrap();
        let row = state.visible_rows()[state.cursor];
        assert!(state.disabled(Id::GraphicsBackend), "libmali has no Vulkan");
        assert_eq!(
            state.subtitle(&row).as_deref(),
            Some("settings.renderer_unsupported")
        );
        assert_eq!(state.handle(Button::R), Effect::None);
        state.vulkan_supported = true;
        assert!(!state.disabled(Id::GraphicsBackend));
        assert_eq!(state.subtitle(&row), None);
        assert_eq!(state.value(Id::GraphicsBackend), "");
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::GraphicsBackend, "opengl".into())
        );
        state.complete(Id::GraphicsBackend, Ok("opengl".into()));
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::GraphicsBackend, "vulkan".into())
        );
        state.complete(Id::GraphicsBackend, Ok("vulkan".into()));
        // The choice is the system's own: another system keeps its default.
        state.focus_system("n64");
        assert_eq!(state.value(Id::GraphicsBackend), "");
        state.focus_system("snes");
        assert_eq!(state.value(Id::GraphicsBackend), "vulkan");
    }
}
