//! System › Port compatibility: archr-compat-debian owns the runtime,
//! runemu.sh the marks it leaves per port and the keys it reads. The page
//! reads the signed manifest, the tool's status and the marks, and asks the
//! tool to download the image or drop the writable layer.
use super::*;
use archr_ui::settings::compat::{Action, Facts, Mark, Port};

pub(super) const TOOL: &str = "/usr/bin/archr-compat-debian";
const MANIFEST: &str = "/usr/share/archr/compat-debian/manifest";
/// runemu.sh's marks: `wanted/<script>` and `no/<script>`.
pub const STATE: &str = "/storage/.cache/compat-debian";
/// An image of ~65 MB over a slow Wi-Fi.
const DOWNLOAD_TIME: Duration = Duration::from_secs(900);

/// The tool is in the image: the page and the game menu's switch show.
pub fn supported(paths: &Paths) -> bool {
    paths.at(TOOL).is_file()
}

impl Backend {
    /// `ports.compat` reads as on unless it says off; `ports.terminal` as is.
    pub(super) fn load_compat(&self, cfg: &SystemCfg, state: &mut Settings) {
        state.compat_supported = supported(&self.paths);
        let off = matches!(
            cfg.get_setting(Id::Compat.key(), None, None).as_deref(),
            Some("off" | "0" | "native")
        );
        state
            .values
            .insert(Id::Compat, if off { "0" } else { "1" }.into());
        if let Some(value) = cfg.get_setting(Id::CompatTerminal.key(), None, None) {
            state.values.insert(Id::CompatTerminal, value);
        }
    }
    /// "1" is saved as "auto": runemu.sh takes "1" as Debian for every port.
    pub(super) fn apply_compat(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        let stored = match (id, value) {
            (Id::Compat, "1") => "auto",
            (Id::Compat, "0") => "off",
            (Id::CompatTerminal, "0" | "1") => value,
            (Id::Compat | Id::CompatTerminal, _) => return Some(Err("Invalid option".into())),
            _ => return None,
        };
        Some(
            SystemCfg::set_and_save(&self.paths.config, &self.paths.lock, id.key(), stored)
                .map(|()| value.into())
                .map_err(|e| e.to_string()),
        )
    }
    /// The action, then the page's readings, and what the banner says.
    pub(super) fn compat(&self, action: Action) -> Reply {
        let said = match action {
            Action::Look => Ok(None),
            Action::Download => match self.compat_code("ensure", DOWNLOAD_TIME) {
                Ok(0) => Ok(Some("compat.downloaded")),
                Ok(2) => Err("compat.download_offline"),
                _ => Err("compat.download_failed"),
            },
            Action::Reset => match self.compat_code("reset", Duration::from_secs(120)) {
                Ok(0) => Ok(Some("compat.reset_done")),
                _ => Err("compat.reset_failed"),
            },
        };
        Reply::Compat(said, Box::new(self.compat_facts()))
    }
    fn compat_code(&self, argument: &str, timeout: Duration) -> Result<i32, String> {
        let mut command = self.tool("archr-compat-debian");
        command.arg(argument);
        let output = crate::process::capture_with_input(
            &mut command,
            None,
            timeout,
            &AtomicBool::new(false),
            64 * 1024,
        )
        .map_err(|e| format!("archr-compat-debian {argument}: {e}"))?;
        if !output.status.success() {
            log::warn!(
                "archr-compat-debian {argument}: {}; {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        output
            .status
            .code()
            .ok_or_else(|| output.status.to_string())
    }
    fn compat_facts(&self) -> Facts {
        let manifest = fs::read_to_string(self.paths.at(MANIFEST)).unwrap_or_default();
        let status = self
            .command("archr-compat-debian", &["status"])
            .map_err(|e| log::warn!("{e}"))
            .unwrap_or_default();
        Facts::parse(&manifest, &status, self.compat_ports())
    }
    /// Ports chosen as Debian in system.cfg, then runemu.sh's marks.
    fn compat_ports(&self) -> Vec<Port> {
        let mut ports: Vec<Port> = self
            .cfg()
            .map(|cfg| {
                cfg.entries()
                    .filter_map(|(key, value)| {
                        let script = key.strip_prefix("ports[\"")?.strip_suffix("\"].compat")?;
                        matches!(value, "debian" | "on" | "1").then(|| Port {
                            script: script.into(),
                            mark: Mark::Chosen,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (dir, mark) in [("wanted", Mark::Learned), ("no", Mark::Refused)] {
            let Ok(entries) = fs::read_dir(self.paths.at(&format!("{STATE}/{dir}"))) else {
                continue;
            };
            let mut found: Vec<String> = entries
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|script| !ports.iter().any(|p| p.script == *script))
                .collect();
            found.sort();
            ports.extend(found.into_iter().map(|script| Port { script, mark }));
        }
        ports
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(cfg: &str) -> (tempfile::TempDir, Backend) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, cfg).unwrap();
        (root, Backend::new(paths))
    }

    #[test]
    fn the_switch_never_saves_forced_debian() {
        let (_root, backend) = backend("");
        assert_eq!(backend.apply_compat(Id::Compat, "1"), Some(Ok("1".into())));
        let cfg = backend.cfg().unwrap();
        assert_eq!(
            cfg.get_setting("ports.compat", None, None).as_deref(),
            Some("auto")
        );
        assert_eq!(backend.apply_compat(Id::Compat, "0"), Some(Ok("0".into())));
        let mut state = Settings::default();
        backend.load_compat(&backend.cfg().unwrap(), &mut state);
        assert_eq!(state.value(Id::Compat), "0");
        assert!(backend.apply_compat(Id::Compat, "debian").unwrap().is_err());
        assert_eq!(
            backend.apply_compat(Id::CompatTerminal, "0"),
            Some(Ok("0".into()))
        );
        backend.load_compat(&backend.cfg().unwrap(), &mut state);
        assert_eq!(state.value(Id::CompatTerminal), "0");
        assert_eq!(backend.apply_compat(Id::Volume, "1"), None);
    }

    #[test]
    fn gating_follows_the_tool() {
        let (root, backend) = backend("");
        let mut state = Settings::default();
        backend.load_compat(&backend.cfg().unwrap(), &mut state);
        assert!(!state.compat_supported);
        let tool = root.path().join(TOOL.trim_start_matches('/'));
        fs::create_dir_all(tool.parent().unwrap()).unwrap();
        fs::write(&tool, "#!/bin/sh\n").unwrap();
        backend.load_compat(&backend.cfg().unwrap(), &mut state);
        assert!(state.compat_supported);
    }

    #[test]
    fn ports_come_from_the_config_and_the_marks() {
        let (root, backend) =
            backend("ports[\"Halo.sh\"].compat=debian\nports[\"Bully.sh\"].compat=native\n");
        for (dir, script) in [
            ("wanted", "Terminal-Launcher.sh"),
            ("wanted", "Halo.sh"),
            ("no", "Cuphead.sh"),
        ] {
            let dir = root.path().join(STATE.trim_start_matches('/')).join(dir);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(script), "").unwrap();
        }
        let ports = backend.compat_ports();
        let found: Vec<_> = ports.iter().map(|p| (p.script.as_str(), p.mark)).collect();
        assert_eq!(
            found,
            [
                ("Halo.sh", Mark::Chosen),
                ("Terminal-Launcher.sh", Mark::Learned),
                ("Cuphead.sh", Mark::Refused)
            ]
        );
    }
}
