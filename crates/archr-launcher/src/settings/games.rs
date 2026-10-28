//! Games page: per-system launch choices in system.cfg, under the keys
//! runemu.sh receives (`<system>.emulator`, `<system>.core`) and
//! setsettings.sh reads (`ratio`, `smooth`, `autosave`, `incrementalsavestates`).
use super::*;
use crate::paths::FEATURES;
use archr_index::{load_features, SystemDef};
use archr_ui::settings::{GameCore, GameSystem};

/// Option rows and the es_features.cfg flag that offers each of them.
const OPTIONS: [(Id, &str); 4] = [
    (Id::Ratio, "ratio"),
    (Id::Filter, "smooth"),
    (Id::Autosave, "autosave"),
    (Id::StateSlot, "autosave"),
];

impl Backend {
    /// Systems that launch through emulators, with their saved choices.
    /// Without the features file every option row stays available.
    pub(super) fn load_games(&self, cfg: &SystemCfg, definitions: &[SystemDef]) -> Vec<GameSystem> {
        let features = load_features(&self.paths.at(FEATURES))
            .map_err(|error| log::warn!("Cannot read {FEATURES}: {error}"))
            .ok();
        definitions
            .iter()
            .filter_map(|system| {
                let cores: Vec<GameCore> = system
                    .emulators
                    .iter()
                    .flat_map(|emulator| emulator.cores.iter().map(move |core| (emulator, core)))
                    .map(|(emulator, core)| GameCore {
                        emulator: emulator.name.clone(),
                        core: core.name.clone(),
                        default: core.default,
                        options: OPTIONS
                            .iter()
                            .filter(|(_, flag)| {
                                features
                                    .as_ref()
                                    .is_none_or(|f| f.has(&emulator.name, &core.name, flag))
                            })
                            .map(|(id, _)| *id)
                            .collect(),
                    })
                    .collect();
                if cores.is_empty() {
                    return None;
                }
                let name = system.name.as_str();
                let mut saved = BTreeMap::new();
                for id in [Id::Emulator, Id::Core] {
                    if let Some(value) = cfg.get_local(name, None, id.key()) {
                        saved.insert(id, value);
                    }
                }
                for (id, _) in OPTIONS {
                    if let Some(value) = cfg.get_setting(id.key(), Some(name), None) {
                        saved.insert(id, value);
                    }
                }
                Some(GameSystem {
                    name: system.name.clone(),
                    fullname: system.fullname.clone(),
                    cores,
                    saved,
                })
            })
            .collect()
    }
    /// One validated write for the system; an emulator brings its first core.
    pub(super) fn apply_game(&self, system: &str, id: Id, value: &str) -> Result<String, String> {
        let definitions = crate::worker::definitions(&self.paths)?;
        let definition = definitions
            .iter()
            .find(|d| d.name == system)
            .ok_or("Unknown system")?;
        let cores = |emulator: &str| -> Vec<&archr_index::Core> {
            definition
                .emulators
                .iter()
                .filter(|e| e.name == emulator)
                .flat_map(|e| &e.cores)
                .collect()
        };
        let key = |id: Id| format!("{system}.{}", id.key());
        let core;
        let entries: Vec<(String, &str)> = match id {
            Id::Emulator => {
                let choices = cores(value);
                core = choices
                    .iter()
                    .find(|c| c.default)
                    .or(choices.first())
                    .ok_or("Invalid option")?
                    .name
                    .clone();
                vec![(key(Id::Emulator), value), (key(Id::Core), core.as_str())]
            }
            Id::Core => {
                let emulator = self
                    .cfg()?
                    .get_local(system, None, Id::Emulator.key())
                    .or_else(|| definition.default_emulator_core().map(|p| p.0.to_owned()))
                    .unwrap_or_default();
                if !cores(&emulator).iter().any(|c| c.name == value) {
                    return Err("Invalid option".into());
                }
                vec![(key(id), value)]
            }
            _ if OPTIONS.iter().any(|(o, _)| *o == id) && id.choices().contains(&value) => {
                vec![(key(id), value)]
            }
            _ => return Err("Invalid option".into()),
        };
        let entries: Vec<(&str, &str)> = entries.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        SystemCfg::set_many_and_save(&self.paths.config, &self.paths.lock, &entries)
            .map_err(|e| e.to_string())?;
        Ok(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CATALOG: &str = r#"<systemList>
  <system><name>snes</name><fullname>Super Nintendo</fullname><path>/roms/snes</path>
    <emulators>
      <emulator name="retroarch"><cores><core>snes9x</core><core default="true">bsnes</core></cores></emulator>
      <emulator name="standalone"><cores><core>snes-sa</core></cores></emulator>
    </emulators></system>
  <system><name>tools</name><fullname>Tools</fullname><path>/roms/tools</path><command>%ROM%</command></system>
</systemList>"#;
    fn fixture(config: &str, features: Option<&str>) -> (tempfile::TempDir, Backend) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        for (path, text) in [
            (paths.at("/usr/share/archr/systems.cfg"), Some(CATALOG)),
            (paths.at(FEATURES), features),
            (paths.config.clone(), Some(config)),
        ] {
            if let Some(text) = text {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text).unwrap();
            }
        }
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        (root, Backend::new(paths))
    }
    fn games(backend: &Backend) -> Vec<GameSystem> {
        let definitions = crate::worker::definitions(&backend.paths).unwrap();
        backend.load_games(&backend.cfg().unwrap(), &definitions)
    }
    fn saved(backend: &Backend) -> String {
        fs::read_to_string(&backend.paths.config).unwrap()
    }
    #[test]
    fn loads_emulator_systems_with_saved_values_and_shell_fallbacks() {
        let (_root, backend) = fixture(
            "global.ratio=core\nsnes.ratio=4/3\nglobal.autosave=1\nsnes.emulator=standalone\n",
            None,
        );
        let systems = games(&backend);
        assert_eq!(systems.len(), 1, "tools has no emulator");
        let snes = &systems[0];
        assert_eq!(snes.fullname, "Super Nintendo");
        assert_eq!(snes.cores.len(), 3);
        assert!(snes.cores[1].default);
        assert_eq!(snes.saved[&Id::Ratio], "4/3");
        assert_eq!(snes.saved[&Id::Autosave], "1");
        assert_eq!(snes.saved[&Id::Emulator], "standalone");
        assert!(!snes.saved.contains_key(&Id::Core));
        // No features file: nothing is hidden.
        assert_eq!(snes.cores[2].options.len(), 4);
    }
    #[test]
    fn features_decide_the_option_rows_of_each_pair() {
        let features = r#"<features>
  <emulator name="retroarch" features="ratio, smooth"><cores>
    <core name="bsnes" features="autosave"/></cores></emulator>
</features>"#;
        let (_root, backend) = fixture("", Some(features));
        let snes = &games(&backend)[0];
        assert_eq!(snes.cores[0].options, [Id::Ratio, Id::Filter]);
        assert_eq!(snes.cores[1].options.len(), 4);
        assert!(snes.cores[2].options.is_empty());
    }
    #[test]
    fn emulator_writes_its_default_core_in_one_publish() {
        let (_root, backend) = fixture("snes.core=snes-sa\n", None);
        assert_eq!(
            backend.apply_game("snes", Id::Emulator, "retroarch"),
            Ok("retroarch".into())
        );
        assert_eq!(
            saved(&backend),
            "snes.core=bsnes\nsnes.emulator=retroarch\n"
        );
        assert_eq!(
            backend.apply_game("snes", Id::Core, "snes9x"),
            Ok("snes9x".into())
        );
        assert_eq!(
            backend.apply_game("snes", Id::Ratio, "16/9"),
            Ok("16/9".into())
        );
        assert_eq!(backend.apply_game("snes", Id::Filter, "1"), Ok("1".into()));
        assert_eq!(
            backend.apply_game("snes", Id::StateSlot, "1"),
            Ok("1".into())
        );
        assert_eq!(
            saved(&backend),
            "snes.core=snes9x\nsnes.emulator=retroarch\nsnes.ratio=16/9\nsnes.smooth=1\n\
             snes.incrementalsavestates=1\n"
        );
    }
    #[test]
    fn invalid_systems_pairs_and_values_are_not_written() {
        let (_root, backend) = fixture("snes.emulator=standalone\n", None);
        for (system, id, value) in [
            ("tools", Id::Core, "x"),
            ("../snes", Id::Ratio, "4/3"),
            ("snes", Id::Emulator, "mame"),
            ("snes", Id::Core, "snes9x"),
            ("snes", Id::Ratio, "5/4\nglobal.x=1"),
            ("snes", Id::Autosave, "2"),
            ("snes", Id::GameSystem, ""),
            ("snes", Id::Volume, "50"),
        ] {
            assert!(
                backend.apply_game(system, id, value).is_err(),
                "{system} {id:?} {value}"
            );
        }
        assert_eq!(saved(&backend), "snes.emulator=standalone\n");
    }
}
