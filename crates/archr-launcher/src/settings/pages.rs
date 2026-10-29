//! Settings effects owned by the network, regional and games pages.
use super::*;
use archr_scraper::{Credentials, Engine, Job, Media, Options, Stop};
use archr_ui::settings::network::{parse_scanlist, wpa_valid, Network};
use archr_ui::settings::MEDIA;
use std::os::unix::process::CommandExt;

/// wifictl's worst case: scan, connect, hidden-network retry and DHCP wait.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(150);

/// Known wifictl failures become locale keys; the raw text goes to the log.
fn wifi_error(error: String) -> String {
    log::warn!("Wi-Fi: {error}");
    if error.contains("no WiFi interface") {
        "settings.wifi_no_adapter".into()
    } else if error.contains("timed out") {
        "settings.wifi_timeout".into()
    } else if error.contains("passphrase") {
        "settings.wpa_length".into()
    } else {
        "settings.wifi_connect_failed".into()
    }
}
/// usbgadget exits 3 when the single OTG port hosts a device (Wi-Fi dongle or
/// gamepad) and when the gadget was never prepared; both leave the value as is.
pub(super) fn usb_error(error: &str) -> String {
    if !error.contains("exit status: 3") {
        error.into()
    } else if error.contains("not configured") {
        "settings.usb_unprepared".into()
    } else {
        "settings.usb_busy".into()
    }
}
/// A scraping job from the saved options. `only` names one system and ROM:
/// a forced "scrape again" of that game. Blocking file reads: worker only.
pub(crate) fn start_scrape(
    paths: &crate::paths::Paths,
    only: Option<(&str, std::path::PathBuf)>,
) -> Result<(Job, Vec<String>), String> {
    let cfg = SystemCfg::load(&paths.config).map_err(|e| e.to_string())?;
    let get = |id: Id| {
        cfg.get_setting(id.key(), None, None)
            .unwrap_or_else(|| id.default_value().into())
    };
    let engine = Engine::production(paths.cache.clone(), Credentials::load(&paths.store))
        .ok_or("settings.scraper_unavailable")?;
    let mut options = Options::new(&get(Id::Language));
    options.only_missing = get(Id::ScraperMissing) == "1";
    options.media = Media::parse_list(&get(Id::ScraperMedia));
    options.systems = Some(get(Id::ScraperSystems))
        .filter(|s| !s.is_empty())
        .into_iter()
        .collect();
    if let Some((system, rom)) = only {
        options.systems = vec![system.to_owned()];
        options.game = Some(rom);
        options.force = true;
    }
    fs::create_dir_all(&paths.cache).map_err(|e| e.to_string())?;
    let systems = crate::worker::definitions(paths)?;
    let touched = systems
        .iter()
        .filter(|s| options.systems.is_empty() || options.systems.contains(&s.name))
        .filter(|s| archr_scraper::system_id(&s.name).is_some())
        .map(|s| s.name.clone())
        .collect();
    Ok((engine.start(systems, options), touched))
}
pub(crate) fn scrape_outcome(stop: Stop) -> &'static str {
    match stop {
        Stop::Completed => "settings.scrape_done",
        Stop::Cancelled => "settings.scrape_cancelled",
        Stop::Quota => "settings.scrape_quota",
        Stop::Auth => "settings.scrape_auth",
        Stop::Restricted => "settings.scrape_restricted",
        Stop::Outdated => "settings.scrape_outdated",
        Stop::Closed => "settings.scrape_closed",
        Stop::Network => "settings.scrape_network",
    }
}

impl Backend {
    /// Networks and the current address, refreshed together.
    pub(super) fn scan(&self) -> Result<(Vec<Network>, String), String> {
        let networks = self
            .command_with_timeout("wifictl", &["scanlist", "--detail"], super::SCAN_TIMEOUT)
            .map(|output| parse_scanlist(&output))
            .map_err(wifi_error)?;
        Ok((networks, self.address()))
    }
    pub(super) fn address(&self) -> String {
        info::ipv4_interface("wlan0").map_or_else(String::new, |(_, ip)| ip)
    }
    pub(super) fn connect(&self, ssid: &str, password: &str) -> Result<(), String> {
        if ssid.is_empty() || ssid.len() > 32 || ssid.contains(['\n', '\r', '\0']) {
            return Err("settings.wifi_connect_failed".into());
        }
        if !password.is_empty() && !wpa_valid(password) {
            return Err("settings.wpa_length".into());
        }
        // An explicit empty third argument is an open network; wifictl then
        // clears the saved key itself.
        self.command_with_timeout("wifictl", &["connect", ssid, password], CONNECT_TIMEOUT)
            .map_err(wifi_error)?;
        SystemCfg::set_many_and_save(
            &self.paths.config,
            &self.paths.lock,
            &[("wifi.ssid", ssid), ("wifi.key", password)],
        )
        .map_err(|e| e.to_string())
    }
    pub(super) fn forget(&self, ssid: &str) -> Result<(), String> {
        if ssid.is_empty() || ssid.contains(['\n', '\r', '\0']) {
            return Err("settings.wifi_connect_failed".into());
        }
        // wifictl owns profile deletion and clears wifi.ssid/key when applicable.
        self.command("wifictl", &["forget", ssid])
            .map(|_| ())
            .map_err(wifi_error)
    }
    /// Like boot's background reconnect: the reply and the scan never wait for it.
    fn reconnect_detached(&self) {
        let mut command = Command::new(self.paths.at("/usr/bin/wifictl"));
        command
            .arg("connect")
            .process_group(0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if let Some(root) = &self.paths.root {
            command.env("ARCHR_LAUNCHER_ROOT", root);
        }
        match crate::process::spawn(&mut command) {
            Ok(mut child) => {
                thread::spawn(move || child.wait());
            }
            Err(error) => log::warn!("Wi-Fi reconnect not started: {error}"),
        }
    }
    /// The OTG port already serves a downstream device: the same sysfs state
    /// usbgadget checks before it refuses to leave host mode.
    fn usb_busy(&self) -> bool {
        let Some(udc) = fs::read_dir(self.paths.at("/sys/class/udc"))
            .ok()
            .and_then(|dir| dir.filter_map(Result::ok).map(|e| e.file_name()).min())
        else {
            return false;
        };
        let state = format!(
            "/sys/class/udc/{}/device/usb1/1-0:1.0/usb1-port1/state",
            udc.to_string_lossy()
        );
        self.read(&state).contains("configured")
    }
    /// `archr-keyboard list-*` prints "code Name" lines.
    pub(super) fn keyboard_names(&self, args: &[&str]) -> Vec<(String, String)> {
        self.command("archr-keyboard", args)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().split_once(' '))
            .map(|(code, name)| (code.into(), name.trim().into()))
            .collect()
    }
    pub(super) fn load_pages(&self, cfg: &SystemCfg, state: &mut Settings) {
        for page in [
            Page::Wifi,
            Page::Bluetooth,
            Page::Services,
            Page::Netplay,
            Page::Scraper,
            Page::Appearance,
            Page::Achievements,
        ] {
            for row in page.rows() {
                if !row.id.key().is_empty() {
                    if let Some(value) = cfg.get_setting(row.id.key(), None, None) {
                        state.values.insert(row.id, row.id.normalize(&value));
                    }
                }
            }
        }
        if let Ok(mode) = self.command("usbgadget", &[]) {
            state.values.insert(Id::UsbMode, mode);
        }
        state.usb_busy = self.usb_busy();
        state.access_ip = self.address();
        keyboard_choices(
            state,
            Id::KeyboardLayout,
            self.keyboard_names(&["list-layouts"]),
        );
        let layout = state.value(Id::KeyboardLayout).to_owned();
        let variants = self.keyboard_names(&["list-variants", &layout]);
        keyboard_choices(state, Id::KeyboardVariant, variants);
        state.scrape.available = archr_scraper::available();
        state.scrape.user = Credentials::load(&self.paths.store)
            .map(|c| c.user)
            .unwrap_or_default();
        let definitions = crate::worker::definitions(&self.paths).unwrap_or_default();
        state.games.systems = self.load_games(cfg, &definitions);
        let mut systems = vec![String::new()];
        for system in definitions {
            if archr_scraper::system_id(&system.name).is_some() {
                state
                    .labels
                    .insert((Id::ScraperSystems, system.name.clone()), system.fullname);
                systems.push(system.name);
            }
        }
        state.options.insert(Id::ScraperSystems, systems);
        let mut themes = crate::appearance::theme_names(&self.paths);
        themes.push(state.value(Id::Theme).to_owned());
        themes.sort();
        themes.dedup();
        for theme in &themes {
            let (loaded, dir) = crate::appearance::load_theme(&self.paths, theme);
            if dir.is_some() || theme == "default" {
                state.theme_names.insert(theme.clone(), loaded.name);
            }
        }
        state.options.insert(Id::Theme, themes);
    }
    /// One locked, atomic publish for all the page's choices.
    pub(super) fn save_appearance(&self, choices: &[(Id, String)]) -> Result<(), String> {
        let entries: Vec<_> = choices
            .iter()
            .map(|(id, v)| (id.key(), v.as_str()))
            .collect();
        SystemCfg::set_many_and_save(&self.paths.config, &self.paths.lock, &entries)
            .map_err(|e| e.to_string())
    }
    /// Blocking login check; the login is stored only once the service accepts it.
    pub(super) fn login(&self, user: &str, password: &str) -> Result<(String, usize), String> {
        let creds = Credentials {
            user: user.trim().into(),
            password: password.into(),
        };
        let engine = Engine::production(self.paths.cache.clone(), Some(creds.clone()))
            .ok_or("settings.scraper_unavailable")?;
        let info = engine
            .verify()
            .map_err(|stop| scrape_outcome(stop).to_owned())?;
        fs::create_dir_all(&self.paths.store).map_err(|e| e.to_string())?;
        creds.save(&self.paths.store).map_err(|e| e.to_string())?;
        Ok((creds.user, info.threads))
    }
    /// Starts the engine's own background job from the saved options; also
    /// names the systems whose art may change.
    pub(super) fn scrape(&self) -> Result<(Job, Vec<String>), String> {
        start_scrape(&self.paths, None)
    }
    pub(super) fn apply_page(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        if !matches!(
            id,
            Id::WifiSwitch
                | Id::BtSwitch
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
                | Id::OnlyRelay
                | Id::MissingGames
                | Id::ScraperSystems
                | Id::ScraperMedia
                | Id::ScraperMissing
                | Id::Automount
                | Id::MergedStorage
        ) {
            return None;
        }
        let result = (|| -> Result<String, String> {
            let choices = id.choices();
            match id {
                _ if !choices.is_empty() && !choices.contains(&value) => {
                    Err("Invalid option".into())
                }
                Id::Ssh | Id::Samba | Id::Http => self.service(id, value),
                // archr-bluetooth persists its own key.
                Id::BtSwitch => self.bt_switch(value == "1").map(|()| value.into()),
                Id::WifiSwitch => {
                    self.command(
                        "wifictl",
                        &[if value == "1" { "enable" } else { "disable" }],
                    )
                    .map_err(wifi_error)?;
                    self.save(id, value)?;
                    if value == "1" && self.cfg()?.get_setting("wifi.ssid", None, None).is_some() {
                        self.reconnect_detached();
                    }
                    Ok(value.into())
                }
                Id::Adhoc | Id::Player | Id::Channel => {
                    // wifictl reads this key directly; reconnect after publishing.
                    self.save(id, value)?;
                    if self
                        .cfg()?
                        .get_setting("wifi.enabled", None, None)
                        .as_deref()
                        == Some("1")
                    {
                        self.command_with_timeout("wifictl", &["reconnect"], CONNECT_TIMEOUT)
                            .map_err(wifi_error)?;
                    }
                    Ok(value.into())
                }
                Id::UsbMode => self
                    .command("usbgadget", &[value])
                    .map(|_| value.into())
                    .map_err(|e| usb_error(&e)),
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
                        return Err("Invalid keyboard layout".into());
                    }
                    // One sway transaction; clear the variant first so the old
                    // one never compiles with the new layout. Sway strips the
                    // quotes of "" and receives an empty variant.
                    let mut sway = format!("input * xkb_variant \"\"; input * xkb_layout {layout}");
                    if !variant.is_empty() {
                        sway += &format!("; input * xkb_variant {variant}");
                    }
                    self.command("swaymsg", &[&sway])?;
                    // archr-keyboard owns system.kblayout and system.kbvariant.
                    self.command("archr-keyboard", &["set", &layout, variant])?;
                    Ok(value.into())
                }
                Id::NetplayPort => {
                    if value.parse::<u16>().is_ok_and(|port| port > 0) {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("Invalid port".into())
                    }
                }
                Id::Nickname => {
                    use crate::handoff::{nick_char, NICK_MAX};
                    if value.len() <= NICK_MAX && value.chars().all(nick_char) {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("settings.invalid_nickname".into())
                    }
                }
                Id::NetplayHost | Id::CustomServer => {
                    if value.len() <= 253
                        && value
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b))
                    {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("settings.invalid_address".into())
                    }
                }
                Id::ScraperMedia => {
                    let tokens: Vec<_> = value.split(',').collect();
                    if value == "none"
                        || Media::parse_list(value).len() == tokens.len()
                            && tokens.iter().all(|t| MEDIA.iter().any(|m| m.0 == *t))
                    {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("Invalid option".into())
                    }
                }
                Id::MergedStorage => self.unify(value),
                Id::ScraperSystems => {
                    if value.is_empty() || archr_scraper::system_id(value).is_some() {
                        self.save(id, value).map(|_| value.into())
                    } else {
                        Err("Invalid option".into())
                    }
                }
                _ => self.save(id, value).map(|_| value.into()),
            }
        })();
        Some(result)
    }
    /// Marker names and unit lists mirror the distribution's `daemons/*` scripts.
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
            for (n, unit) in units.iter().enumerate() {
                if let Err(error) = self.command("systemctl", &["start", unit]) {
                    // Never leave half a service running.
                    for started in units[..n].iter().rev() {
                        let _ = self.command("systemctl", &["stop", started]);
                    }
                    let _ = fs::remove_file(&marker);
                    return Err(error);
                }
            }
        } else {
            for unit in units.iter().rev() {
                self.command("systemctl", &["stop", unit])?;
            }
            if marker.exists() {
                fs::remove_file(&marker).map_err(|e| e.to_string())?;
            }
        }
        self.save(id, value)?;
        Ok(value.into())
    }
}
/// Names label the codes; the current value and "default variant" stay choosable.
pub(super) fn keyboard_choices(state: &mut Settings, id: Id, names: Vec<(String, String)>) {
    let mut codes = vec![state.value(id).to_owned()];
    if id == Id::KeyboardVariant {
        codes.push(String::new());
    }
    for (code, name) in names {
        state.labels.insert((id, code.clone()), name);
        codes.push(code);
    }
    codes.sort();
    codes.dedup();
    state.options.insert(id, codes);
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
    fn fixture(config: &str) -> (tempfile::TempDir, Paths) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.at("/usr/bin")).unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, config).unwrap();
        (root, paths)
    }
    /// A fixture command that appends its arguments, one call per line.
    fn recorder(root: &tempfile::TempDir, paths: &Paths, name: &str, body: &str) {
        let script = paths.at(&format!("/usr/bin/{name}"));
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"{name} $*\" >> {}\n{body}\n",
                root.path().join("calls").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn calls(root: &tempfile::TempDir) -> String {
        fs::read_to_string(root.path().join("calls")).unwrap_or_default()
    }
    #[test]
    fn ssh_toggle_prepares_marker_starts_unit_then_persists() {
        let (root, paths) = fixture("ssh.enabled=0\n");
        recorder(&root, &paths, "systemctl", "");
        let backend = Backend::new(paths.clone());
        assert_eq!(backend.apply_page(Id::Ssh, "1"), Some(Ok("1".into())));
        assert_eq!(calls(&root), "systemctl start sshd.service\n");
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
    fn failed_samba_start_stops_units_already_started() {
        let (root, paths) = fixture("samba.enabled=0\n");
        recorder(
            &root,
            &paths,
            "systemctl",
            "[ \"$2\" = smbd.service ] && exit 1\nexit 0",
        );
        let backend = Backend::new(paths.clone());
        assert!(backend.apply_page(Id::Samba, "1").unwrap().is_err());
        assert_eq!(
            calls(&root),
            "systemctl start nmbd.service\nsystemctl start smbd.service\nsystemctl stop nmbd.service\n"
        );
        assert!(!paths.at("/var/lib/archr/services/smb.conf").exists());
    }
    #[test]
    fn open_network_passes_explicit_empty_password_argument() {
        let (root, paths) = fixture("wifi.ssid=\nwifi.key=\n");
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
    fn wifictl_failures_become_locale_keys() {
        for (raw, key) in [
            (
                "wifictl: exit status: 1; wifictl: no WiFi interface",
                "settings.wifi_no_adapter",
            ),
            ("wifictl: command timed out", "settings.wifi_timeout"),
            (
                "wifictl: exit status: 2; wifictl: passphrase has 5 bytes",
                "settings.wpa_length",
            ),
            ("wifictl: exit status: 1; ", "settings.wifi_connect_failed"),
        ] {
            assert_eq!(wifi_error(raw.into()), key);
        }
        let (_root, paths) = fixture("");
        assert_eq!(
            Backend::new(paths).connect(&"x".repeat(33), ""),
            Err("settings.wifi_connect_failed".into())
        );
    }
    #[test]
    fn enabling_wifi_returns_before_the_background_reconnect_finishes() {
        let (root, paths) = fixture("wifi.ssid=Casa\n");
        recorder(
            &root,
            &paths,
            "wifictl",
            "[ \"$1\" = connect ] && sleep 3\nexit 0",
        );
        let backend = Backend::new(paths);
        let started = std::time::Instant::now();
        assert_eq!(
            backend.apply_page(Id::WifiSwitch, "1"),
            Some(Ok("1".into()))
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        thread::sleep(Duration::from_millis(300));
        assert_eq!(calls(&root), "wifictl enable\nwifictl connect\n");
    }
    #[test]
    fn keyboard_layout_clears_variant_in_one_sway_command_then_persists_through_script() {
        let (root, paths) = fixture("system.kblayout=br\nsystem.kbvariant=abnt2\n");
        recorder(&root, &paths, "swaymsg", "");
        recorder(&root, &paths, "archr-keyboard", "");
        let backend = Backend::new(paths);
        assert_eq!(
            backend.apply_page(Id::KeyboardLayout, "us"),
            Some(Ok("us".into()))
        );
        assert_eq!(
            calls(&root),
            "swaymsg input * xkb_variant \"\"; input * xkb_layout us\narchr-keyboard set us \n"
        );
    }
    #[test]
    fn scraper_stays_disabled_without_the_build_credential() {
        let (_root, paths) = fixture("");
        let backend = Backend::new(paths.clone());
        let mut state = Settings::default();
        backend.load_pages(&SystemCfg::load(&paths.config).unwrap(), &mut state);
        assert_eq!(state.scrape.available, archr_scraper::available());
        if !archr_scraper::available() {
            assert_eq!(
                backend.login("user", "secret"),
                Err("settings.scraper_unavailable".into())
            );
            assert!(backend.scrape().is_err());
            assert!(!paths.store.join("scraper.toml").exists());
        }
    }
    #[test]
    fn media_choices_are_scraper_kinds_and_only_known_lists_are_saved() {
        let tokens: Vec<_> = MEDIA.iter().map(|m| m.0).collect();
        assert_eq!(Media::parse_list(&tokens.join(",")), Media::ALL);
        let (_root, paths) = fixture("");
        let backend = Backend::new(paths.clone());
        for bad in ["box,bogus", "box,box", "Box", ""] {
            assert!(
                backend.apply_page(Id::ScraperMedia, bad).unwrap().is_err(),
                "{bad}"
            );
        }
        for good in ["none", "box,video,manual"] {
            assert_eq!(
                backend.apply_page(Id::ScraperMedia, good),
                Some(Ok(good.into()))
            );
        }
        let saved =
            SystemCfg::load(&paths.config)
                .unwrap()
                .get_setting(Id::ScraperMedia.key(), None, None);
        assert_eq!(saved.as_deref(), Some("box,video,manual"));
    }
    #[test]
    fn invalid_port_and_nickname_are_not_published() {
        let (_root, paths) = fixture("global.netplay.port=55435\n");
        let backend = Backend::new(paths.clone());
        assert!(backend
            .apply_page(Id::NetplayPort, "65536")
            .unwrap()
            .is_err());
        assert!(backend
            .apply_page(Id::Nickname, "bad-name")
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
    #[test]
    fn appearance_choices_publish_together_and_load_back_with_their_themes() {
        let (_root, paths) = fixture("launcher.accent=pink\n");
        let backend = Backend::new(paths.clone());
        let night = paths.at("/storage/.config/archr-launcher/themes/night");
        fs::create_dir_all(&night).unwrap();
        fs::write(night.join("theme.toml"), "accent = \"#FF0000\"\n").unwrap();
        backend
            .save_appearance(&[
                (Id::Theme, "night".into()),
                (Id::Accent, "teal".into()),
                (Id::Animations, "0".into()),
            ])
            .unwrap();
        let cfg = SystemCfg::load(&paths.config).unwrap();
        let look = crate::appearance::saved(Some(&cfg));
        assert_eq!(
            (look.theme.as_str(), look.accent.as_str()),
            ("night", "teal")
        );
        // "0", as earlier versions saved it, still reads as off.
        assert_eq!(look.animations, archr_ui::motion::Motion::Off);
        assert_eq!(look.text_size, "normal");
        assert_eq!(crate::appearance::theme_names(&paths), ["default", "night"]);
        let (theme, dir) = crate::appearance::load_theme(&paths, "night");
        assert_eq!(dir, Some(night));
        assert_eq!(theme.highlight_bg.r, 255);
        // A theme that vanished falls back to the builtin one.
        assert_eq!(crate::appearance::load_theme(&paths, "gone").1, None);
    }
}
