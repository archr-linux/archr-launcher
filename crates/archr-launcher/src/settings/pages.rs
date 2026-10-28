//! Settings effects owned by the network and games pages.
use super::*;
use archr_ui::settings::network::{parse_scanlist, Network};

impl Backend {
    pub(super) fn scan(&self) -> Result<Vec<Network>, String> {
        self.command("wifictl", &["scanlist", "--detail"])
            .map(|output| parse_scanlist(&output))
    }
    pub(super) fn connect(&self, ssid: &str, password: &str) -> Result<(), String> {
        if ssid.is_empty() || ssid.len() > 128 || ssid.contains(['\n', '\r', '\0']) {
            return Err("Invalid network name".into());
        }
        if !password.is_empty()
            && !((8..=63).contains(&password.len())
                || (password.len() == 64 && password.bytes().all(|b| b.is_ascii_hexdigit())))
        {
            return Err("Invalid WPA password".into());
        }
        // Current wifictl treats an empty third argument as "use wifi.key".
        // Refuse open-network attempts while a saved PSK exists; wifictl must
        // distinguish an explicit empty argument from an omitted one.
        if password.is_empty()
            && self
                .cfg()?
                .get_setting("wifi.key", None, None)
                .is_some_and(|key| !key.is_empty())
        {
            return Err("Cannot connect to open network while a Wi-Fi password is saved".into());
        }
        self.command_with_timeout(
            "wifictl",
            &["connect", ssid, password],
            Duration::from_secs(95),
        )?;
        SystemCfg::set_many_and_save(
            &self.paths.config,
            &self.paths.lock,
            &[("wifi.ssid", ssid), ("wifi.key", password)],
        )
        .map_err(|e| e.to_string())
    }
    pub(super) fn forget(&self, ssid: &str) -> Result<(), String> {
        if ssid.is_empty() || ssid.contains(['\n', '\r', '\0']) {
            return Err("Invalid network name".into());
        }
        // wifictl owns profile deletion and clears wifi.ssid/key when applicable.
        self.command("wifictl", &["forget", ssid]).map(|_| ())
    }
    pub(super) fn load_pages(&self, cfg: &SystemCfg, state: &mut Settings) {
        for page in [
            Page::Wifi,
            Page::Services,
            Page::Netplay,
            Page::Games,
            Page::Scraper,
        ] {
            for row in page.rows() {
                if !row.id.key().is_empty() {
                    if let Some(value) = cfg.get_setting(row.id.key(), None, None) {
                        state.values.insert(row.id, value);
                    }
                }
            }
        }
        if let Ok(mode) = self.command("usbgadget", &[]) {
            state.values.insert(Id::UsbMode, mode);
        }
        if let Ok(ip) = self.command("hostname", &["-I"]) {
            state.access_ip = ip.split_whitespace().next().unwrap_or_default().to_owned();
        }
        let mut layouts = self
            .command("archr-keyboard", &["list-layouts"])
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        layouts.push(state.value(Id::KeyboardLayout).into());
        layouts.sort();
        layouts.dedup();
        state.options.insert(Id::KeyboardLayout, layouts);
        self.load_variants(state);
        state.options.insert(
            Id::ClockFormat,
            Id::ClockFormat
                .choices()
                .iter()
                .map(|v| (*v).into())
                .collect(),
        );
        for id in [Id::Player, Id::Channel, Id::UsbMode, Id::Relay] {
            state
                .options
                .insert(id, id.choices().iter().map(|v| (*v).into()).collect());
        }
    }
    pub(super) fn load_variants(&self, state: &mut Settings) {
        let layout = state.value(Id::KeyboardLayout).to_owned();
        let mut variants = self.keyboard_variants(&layout);
        variants.push(String::new());
        variants.push(state.value(Id::KeyboardVariant).into());
        variants.sort();
        variants.dedup();
        state.options.insert(Id::KeyboardVariant, variants);
    }
    pub(super) fn keyboard_variants(&self, layout: &str) -> Vec<String> {
        self.command("archr-keyboard", &["list-variants", layout])
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .map(str::to_owned)
            .collect()
    }
    pub(super) fn apply_page(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        if !matches!(
            id,
            Id::WifiSwitch
                | Id::Ssh
                | Id::Samba
                | Id::Http
                | Id::Adhoc
                | Id::Player
                | Id::Channel
                | Id::UsbMode
                | Id::KeyboardLayout
                | Id::KeyboardVariant
                | Id::ClockFormat
                | Id::NetplayEnabled
                | Id::Nickname
                | Id::NetplayHost
                | Id::NetplayPort
                | Id::Relay
                | Id::CustomServer
                | Id::AutoLobby
                | Id::OnlyRelay
                | Id::MissingGames
                | Id::CheckIndexes
        ) {
            return None;
        }
        let result = (|| -> Result<String, String> {
            match id {
                Id::WifiSwitch
                | Id::Ssh
                | Id::Samba
                | Id::Http
                | Id::Adhoc
                | Id::NetplayEnabled
                | Id::AutoLobby
                | Id::OnlyRelay
                | Id::MissingGames
                | Id::CheckIndexes
                    if !matches!(value, "0" | "1") =>
                {
                    Err("Invalid switch value".into())
                }
                Id::Ssh | Id::Samba | Id::Http => self.service(id, value),
                Id::WifiSwitch => {
                    self.command(
                        "wifictl",
                        &[if value == "1" { "enable" } else { "disable" }],
                    )?;
                    self.save(id, value)?;
                    if value == "1" && self.cfg()?.get_setting("wifi.ssid", None, None).is_some() {
                        if let Err(error) = self.command_with_timeout(
                            "wifictl",
                            &["connect"],
                            Duration::from_secs(95),
                        ) {
                            log::warn!("Wi-Fi enabled, saved network reconnect failed: {error}");
                        }
                    }
                    Ok(value.into())
                }
                Id::Adhoc | Id::Player | Id::Channel => {
                    if !id.choices().contains(&value) {
                        Err("Invalid local play option".into())
                    } else {
                        // wifictl reads this key directly; reconnect after publishing.
                        self.save(id, value)?;
                        if self
                            .cfg()?
                            .get_setting("wifi.enabled", None, None)
                            .as_deref()
                            == Some("1")
                        {
                            self.command_with_timeout(
                                "wifictl",
                                &["reconnect"],
                                Duration::from_secs(95),
                            )?;
                        }
                        Ok(value.into())
                    }
                }
                Id::UsbMode => {
                    if !id.choices().contains(&value) {
                        Err("Invalid USB mode".into())
                    } else {
                        self.command("usbgadget", &[value]).map(|_| value.into())
                    }
                }
                Id::KeyboardLayout | Id::KeyboardVariant => {
                    let cfg = self.cfg()?;
                    let layout = if id == Id::KeyboardLayout {
                        value.to_owned()
                    } else {
                        cfg.get_setting(Id::KeyboardLayout.key(), None, None)
                            .unwrap_or_else(|| "us".into())
                    };
                    let variant = if id == Id::KeyboardVariant { value } else { "" };
                    if !valid_xkb(&layout) || !valid_xkb(variant) {
                        Err("Invalid keyboard layout".into())
                    } else {
                        self.command("swaymsg", &["input", "*", "xkb_layout", &layout])?;
                        self.command("swaymsg", &["input", "*", "xkb_variant", variant])?;
                        SystemCfg::set_many_and_save(
                            &self.paths.config,
                            &self.paths.lock,
                            &[
                                (Id::KeyboardLayout.key(), layout.as_str()),
                                (Id::KeyboardVariant.key(), variant),
                            ],
                        )
                        .map_err(|e| e.to_string())?;
                        Ok(value.into())
                    }
                }
                Id::NetplayPort => {
                    if value.parse::<u16>().is_ok_and(|port| port > 0) {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("Invalid port".into())
                    }
                }
                Id::Nickname | Id::NetplayHost | Id::CustomServer => {
                    if value.len() > 128 || value.contains(['\n', '\r', '\0']) {
                        Err("Invalid text".into())
                    } else {
                        self.save(id, value).map(|_| value.into())
                    }
                }
                Id::Relay | Id::ClockFormat => {
                    if id.choices().contains(&value) {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("Invalid option".into())
                    }
                }
                Id::NetplayEnabled
                | Id::AutoLobby
                | Id::OnlyRelay
                | Id::MissingGames
                | Id::CheckIndexes => self.save(id, value).map(|_| value.into()),
                _ => Err("Unsupported setting".into()),
            }
        })();
        Some(result)
    }
    fn service(&self, id: Id, value: &str) -> Result<String, String> {
        let (marker, units): (&str, &[&str]) = match id {
            Id::Ssh => ("sshd.conf", &["sshd.service"]),
            Id::Samba => (
                "smb.conf",
                &["nmbd.service", "smbd.service", "wsdd2.service"],
            ),
            Id::Http => ("simplehttp.conf", &["simple-http-server.service"]),
            _ => unreachable!(),
        };
        let marker = self.paths.at(&format!("/var/lib/archr/services/{marker}"));
        if value == "1" {
            fs::create_dir_all(marker.parent().expect("marker parent"))
                .map_err(|e| e.to_string())?;
            fs::write(&marker, "").map_err(|e| e.to_string())?;
        }
        for unit in if value == "1" {
            units.to_vec()
        } else {
            units.iter().rev().copied().collect::<Vec<_>>()
        } {
            if let Err(error) = self.command(
                "systemctl",
                &[if value == "1" { "start" } else { "stop" }, unit],
            ) {
                if value == "1" {
                    let _ = fs::remove_file(&marker);
                }
                return Err(error);
            }
        }
        if value == "0" && marker.exists() {
            fs::remove_file(&marker).map_err(|e| e.to_string())?;
        }
        self.save(id, value)?;
        Ok(value.into())
    }
}
fn valid_xkb(value: &str) -> bool {
    value.len() <= 32
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_+-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn ssh_toggle_prepares_marker_starts_unit_then_persists() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.at("/usr/bin")).unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "ssh.enabled=0\n").unwrap();
        let script = paths.at("/usr/bin/systemctl");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$1 $2\" >> {}\n",
                root.path().join("calls").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let backend = Backend::new(paths.clone());
        assert_eq!(backend.apply_page(Id::Ssh, "1"), Some(Ok("1".into())));
        assert_eq!(
            fs::read_to_string(root.path().join("calls")).unwrap(),
            "start sshd.service\n"
        );
        assert!(paths.at("/var/lib/archr/services/sshd.conf").exists());
        assert_eq!(
            SystemCfg::load(&paths.config)
                .unwrap()
                .get_setting("ssh.enabled", None, None)
                .as_deref(),
            Some("1")
        );
    }
    #[test]
    fn open_network_never_inherits_previous_password() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.at("/usr/bin")).unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::write(&paths.config, "wifi.ssid=Old\nwifi.key=oldpassword\n").unwrap();
        let command = paths.at("/usr/bin/wifictl");
        fs::write(
            &command,
            format!(
                "#!/bin/sh\ntouch {}\n",
                root.path().join("called").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
        let backend = Backend::new(paths.clone());
        assert!(backend
            .connect("Guest", "")
            .unwrap_err()
            .contains("open network"));
        assert!(!root.path().join("called").exists());
    }

    #[test]
    fn open_network_passes_explicit_empty_password_argument() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.at("/usr/bin")).unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "wifi.ssid=\nwifi.key=\n").unwrap();
        let command = paths.at("/usr/bin/wifictl");
        fs::write(
            &command,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$#\" \"$1\" \"$2\" \"$3\" > {}\n",
                root.path().join("called").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
        let backend = Backend::new(paths);
        backend.connect("Guest", "").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("called")).unwrap(),
            "3\nconnect\nGuest\n\n"
        );
    }

    #[test]
    fn invalid_port_is_not_published() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "global.netplay.port=55435\n").unwrap();
        let backend = Backend::new(paths.clone());
        assert!(backend
            .apply_page(Id::NetplayPort, "65536")
            .unwrap()
            .is_err());
        assert_eq!(
            SystemCfg::load(&paths.config)
                .unwrap()
                .get_setting("global.netplay.port", None, None)
                .as_deref(),
            Some("55435")
        );
    }
}
