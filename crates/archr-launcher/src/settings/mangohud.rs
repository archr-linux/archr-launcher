//! MangoHud page effects. The launcher saves the layout keys and asks
//! mangohud_set to rebuild MangoHud.conf from them; mangohud_set itself owns
//! `archr.mangohud.enabled` and `archr.mangohud.state`.
use super::*;
use archr_ui::settings::mangohud::ITEMS;

/// Written at boot by the RK3326 quirk from the GPU driver in use.
const SUPPORT: &str = "/storage/.config/profile.d/075-mangohud-supported";
const TIMEOUT: Duration = Duration::from_secs(10);

/// A comma list of known items, each once.
fn items_valid(value: &str) -> bool {
    let words: Vec<&str> = value.split(',').collect();
    !value.is_empty()
        && words
            .iter()
            .all(|w| ITEMS.iter().any(|(code, _)| code == w))
        && words
            .iter()
            .enumerate()
            .all(|(n, w)| !words[..n].contains(w))
}

/// The quirk found a GPU driver MangoHud runs on.
pub fn supported(paths: &Paths) -> bool {
    archr_io::read_utf8(&paths.at(SUPPORT), 64 * 1024, archr_io::Symlinks::Follow)
        .unwrap_or_default()
        .lines()
        .any(|line| line.trim() == "DEVICE_MANGOHUD_SUPPORT=\"true\"")
}

impl Backend {
    pub(super) fn load_mangohud(&self, cfg: &SystemCfg, state: &mut Settings) {
        state.mangohud_supported = supported(&self.paths);
        for row in Page::Mangohud.rows() {
            if let Some(value) = cfg.get_setting(row.id.key(), None, None) {
                let value = match row.id {
                    Id::MangohudShow => u8::from(value == "on").to_string(),
                    _ => value,
                };
                state.values.insert(row.id, value);
            }
        }
    }
    /// mangohud_set with a timeout, then the key it should have written.
    fn mangohud_set(&self, argument: &str, key: &str, stored: &str) -> Result<(), String> {
        self.command_with_timeout("mangohud_set", &[argument], TIMEOUT)?;
        if self.cfg()?.get_setting(key, None, None).as_deref() == Some(stored) {
            Ok(())
        } else {
            Err(format!("mangohud_set {argument}: {key} was not saved"))
        }
    }
    pub(super) fn apply_mangohud(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        let on = value == "1";
        let entries = match id {
            Id::Mangohud | Id::MangohudShow if !id.choices().contains(&value) => {
                return Some(Err("Invalid option".into()))
            }
            Id::Mangohud => {
                let argument = if on { "enable" } else { "disable" };
                return Some(
                    self.mangohud_set(argument, id.key(), value)
                        .map(|()| value.into()),
                );
            }
            Id::MangohudShow => {
                let state = if on { "on" } else { "off" };
                return Some(
                    self.mangohud_set(state, id.key(), state)
                        .map(|()| value.into()),
                );
            }
            Id::MangohudItems if items_valid(value) => vec![
                (Id::MangohudLayout.key(), "custom"),
                (Id::MangohudItems.key(), value),
            ],
            Id::MangohudLayout
            | Id::MangohudPosition
            | Id::MangohudSize
            | Id::MangohudAlpha
            | Id::MangohudFps
                if id.choices().contains(&value) =>
            {
                vec![(id.key(), value)]
            }
            Id::MangohudItems
            | Id::MangohudLayout
            | Id::MangohudPosition
            | Id::MangohudSize
            | Id::MangohudAlpha
            | Id::MangohudFps => return Some(Err("Invalid option".into())),
            _ => return None,
        };
        Some(
            SystemCfg::set_many_and_save(&self.paths.config, &self.paths.lock, &entries)
                .map_err(|e| e.to_string())
                .and_then(|()| self.command_with_timeout("mangohud_set", &["apply"], TIMEOUT))
                .map(|_| value.into()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    /// A root whose mangohud_set records its calls and saves like the real one.
    fn fixture(config: &str, support: Option<&str>) -> (tempfile::TempDir, Backend) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.at("/usr/bin")).unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, config).unwrap();
        if let Some(text) = support {
            fs::create_dir_all(paths.at("/storage/.config/profile.d")).unwrap();
            fs::write(paths.at(SUPPORT), text).unwrap();
        }
        let script = paths.at("/usr/bin/mangohud_set");
        let cfg = paths.config.display();
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$*\" >> {calls}\ncase $1 in\n\
                 enable) echo archr.mangohud.enabled=1 >> {cfg};;\n\
                 disable) echo archr.mangohud.enabled=0 >> {cfg};;\n\
                 on|off) echo archr.mangohud.state=$1 >> {cfg};;\nesac\n",
                calls = root.path().join("calls").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        (root, Backend::new(paths))
    }
    fn calls(root: &tempfile::TempDir) -> String {
        fs::read_to_string(root.path().join("calls")).unwrap_or_default()
    }
    #[test]
    fn support_and_saved_choices_load_with_the_state_as_a_switch() {
        let (_root, backend) = fixture(
            "archr.mangohud.state=on\narchr.mangohud.layout=detailed\n",
            Some("DEVICE_MANGOHUD_SUPPORT=\"true\"\n"),
        );
        let mut state = Settings::default();
        backend.load_mangohud(&backend.cfg().unwrap(), &mut state);
        assert!(state.mangohud_supported);
        assert_eq!(state.value(Id::MangohudShow), "1");
        assert_eq!(state.value(Id::MangohudLayout), "detailed");
        for support in [None, Some("DEVICE_MANGOHUD_SUPPORT=\"false\"\n")] {
            let (_root, backend) = fixture("", support);
            let mut state = Settings::default();
            backend.load_mangohud(&backend.cfg().unwrap(), &mut state);
            assert!(!state.mangohud_supported);
        }
    }
    #[test]
    fn choices_are_saved_then_applied_and_switches_go_through_mangohud_set() {
        let (root, backend) = fixture("", None);
        assert_eq!(
            backend.apply(Id::MangohudPosition, "bottom-right"),
            Ok("bottom-right".into())
        );
        assert_eq!(
            backend.apply(Id::MangohudItems, "fps,time"),
            Ok("fps,time".into())
        );
        assert_eq!(backend.apply(Id::MangohudShow, "1"), Ok("1".into()));
        assert_eq!(backend.apply(Id::Mangohud, "1"), Ok("1".into()));
        assert_eq!(calls(&root), "apply\napply\non\nenable\n");
        let cfg = backend.cfg().unwrap();
        for (key, value) in [
            ("archr.mangohud.position", "bottom-right"),
            ("archr.mangohud.layout", "custom"),
            ("archr.mangohud.items", "fps,time"),
        ] {
            assert_eq!(cfg.get_setting(key, None, None).as_deref(), Some(value));
        }
    }
    #[test]
    fn invalid_values_write_nothing() {
        let (root, backend) = fixture("", None);
        for (id, value) in [
            (Id::MangohudLayout, "wide"),
            (Id::MangohudSize, "99"),
            (Id::MangohudItems, ""),
            (Id::MangohudItems, "fps,fps"),
            (Id::MangohudItems, "fps,\nglobal.x=1"),
            (Id::MangohudShow, "on"),
        ] {
            assert!(backend.apply(id, value).is_err(), "{id:?} {value:?}");
        }
        assert_eq!(calls(&root), "");
        assert_eq!(fs::read_to_string(&backend.paths.config).unwrap(), "");
    }
    #[test]
    fn a_script_that_does_not_persist_fails() {
        let (root, backend) = fixture("", None);
        fs::write(
            backend.paths.at("/usr/bin/mangohud_set"),
            "#!/bin/sh\nexit 0\n",
        )
        .unwrap();
        assert!(backend.apply(Id::Mangohud, "1").is_err());
        drop(root);
    }
}
