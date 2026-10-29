//! Rows and input for the network, games and keyboard pages.
use super::{
    bluetooth::Action, described, network::wpa_valid, row, Effect, Id, Kind, Page, Row, Settings,
};
use crate::{
    appearance::{parse_hex, Appearance},
    keyboard::{Keyboard, Mode},
    state::Button,
};

pub const WIFI: &[Row] = &[row(Id::WifiSwitch, "settings.wifi", Kind::Toggle)];
pub const BLUETOOTH: &[Row] = &[row(Id::BtSwitch, "settings.bluetooth", Kind::Toggle)];
pub const SERVICES: &[Row] = &[
    described(Id::Ssh, "settings.ssh", "settings.ssh_detail")
        .kind(Kind::Toggle)
        .under("settings.access"),
    described(Id::Samba, "settings.samba", "settings.samba_detail").kind(Kind::Toggle),
    described(Id::Http, "settings.http", "settings.http_detail").kind(Kind::Toggle),
    described(
        Id::NetplayEnabled,
        "settings.netplay",
        "settings.netplay_detail",
    )
    .under("settings.game_usb"),
    described(Id::UsbMode, "settings.usb_mode", "settings.usb_detail"),
    described(Id::Adhoc, "settings.adhoc", "settings.adhoc_detail").under("settings.local_play"),
    described(Id::Player, "settings.player", "settings.player_detail"),
    described(Id::Channel, "settings.channel", "settings.channel_detail"),
];
pub const GAMES: &[Row] = &[
    row(Id::Emulator, "settings.emulator", Kind::Value).under(super::games::SYSTEM_SECTION),
    row(Id::Core, "settings.core", Kind::Value),
    row(Id::Ratio, "settings.ratio", Kind::Value),
    row(Id::Filter, "settings.filter", Kind::Value),
    described(
        Id::Autosave,
        "settings.autosave",
        "settings.autosave_detail",
    )
    .kind(Kind::Toggle),
    row(Id::StateSlot, "settings.state_slot", Kind::Value),
    row(Id::MangohudGame, "settings.mangohud", Kind::Value),
    row(Id::GameSystem, "settings.other_system", Kind::Action).under("settings.other"),
    row(Id::NetplayLink, "settings.netplay", Kind::Action),
    row(Id::ScraperLink, "settings.scraper", Kind::Action),
    row(Id::MangohudLink, "settings.mangohud_link", Kind::Action),
];
pub const APPEARANCE: &[Row] = &[
    row(Id::Theme, "settings.theme", Kind::Value),
    row(Id::Accent, "settings.accent", Kind::Value),
    row(Id::Background, "settings.background", Kind::Value),
    row(Id::Font, "settings.font", Kind::Value),
    row(Id::TextSize, "settings.text_size", Kind::Value),
    row(Id::Animations, "settings.animations", Kind::Value),
    row(Id::Restore, "settings.restore", Kind::Action),
];
pub const NETPLAY: &[Row] = &[
    row(Id::NetplayEnabled, "settings.netplay_enabled", Kind::Toggle),
    row(Id::Nickname, "settings.nickname", Kind::Value),
    row(Id::NetplayHost, "settings.host", Kind::Value).under("settings.connection"),
    row(Id::NetplayPort, "settings.port", Kind::Value),
    row(Id::Relay, "settings.relay", Kind::Value),
    row(Id::CustomServer, "settings.custom_server", Kind::Value),
    row(Id::OnlyRelay, "settings.only_relay", Kind::Toggle).under("settings.rooms"),
    row(Id::MissingGames, "settings.missing_games", Kind::Toggle),
    row(Id::IndexNow, "settings.index_now", Kind::Action).under("settings.games_index"),
];
pub const SCRAPER: &[Row] = &[
    row(Id::ScraperSource, "settings.source", Kind::Value),
    described(
        Id::ScraperAccount,
        "settings.account",
        "settings.not_signed_in",
    )
    .kind(Kind::Action),
    row(Id::ScraperSystems, "settings.scraper_systems", Kind::Value),
    row(Id::ScraperMedia, "settings.media", Kind::Value),
    row(Id::ScraperMissing, "settings.only_missing", Kind::Toggle),
    row(Id::ScraperStart, "settings.start_scraping", Kind::Action),
];

impl Settings {
    pub(crate) fn text_input(id: Id) -> bool {
        matches!(
            id,
            Id::Nickname | Id::NetplayHost | Id::NetplayPort | Id::CustomServer
        )
    }
    pub(super) fn open_keyboard(&mut self, id: Id, mode: Mode, value: String) {
        let mut keyboard = Keyboard::new(mode, value);
        keyboard.max = match id {
            Id::Hidden => 32,     // IEEE 802.11 SSID
            Id::WifiSwitch => 64, // WPA passphrase or hex key
            Id::Nickname => 32,   // RetroArch nickname
            Id::NetplayHost | Id::CustomServer => 253,
            _ => keyboard.max,
        };
        // Clearing a Netplay field restores its default; an open hidden network
        // has no password. Ports, addresses and SSIDs stay mandatory.
        keyboard.optional = matches!(id, Id::Nickname | Id::NetplayHost | Id::CustomServer)
            || (id == Id::WifiSwitch && self.wifi_hidden);
        self.keyboard = Some(keyboard);
        self.keyboard_target = Some(id);
        self.open(Page::Keyboard);
    }
    /// The network under the cursor on the Wi-Fi page.
    pub fn wifi_network(&self) -> Option<&super::Network> {
        (self.page() == Page::Wifi)
            .then(|| self.cursor.checked_sub(1))
            .flatten()
            .and_then(|n| self.networks.get(n))
    }
    /// Rows after the networks: hidden network, network services, Bluetooth.
    pub fn wifi_rows(&self) -> usize {
        self.networks.len() + 4
    }
    pub(super) fn page_input(&mut self, button: Button) -> Option<Effect> {
        if self.connecting.is_some() {
            return Some(Effect::None);
        }
        if let Some(effect) = self.storage_input(button) {
            return Some(effect);
        }
        if let Some(ssid) = self.confirm_forget.clone() {
            self.confirm_forget = None;
            return Some(if button != Button::A {
                Effect::None
            } else if self.page() == Page::Bluetooth {
                Effect::Bluetooth(Action::Forget, ssid)
            } else {
                Effect::Forget(ssid)
            });
        }
        match self.page() {
            Page::Keyboard => Some(self.keyboard_input(button)),
            Page::AccentColor => Some(self.custom_input(button)),
            Page::Wifi => self.wifi_input(button),
            Page::Bluetooth => self.bt_input(button),
            Page::Games => self.games_input(button),
            Page::MangohudItems => Some(self.mangohud_items_input(button)),
            _ => None,
        }
    }
    fn keyboard_input(&mut self, button: Button) -> Effect {
        if button == Button::B {
            self.keyboard = None;
            self.keyboard_target = None;
            self.error = None;
            return self.back();
        }
        let target = self.keyboard_target.expect("keyboard target");
        let Some(value) = self.keyboard.as_mut().and_then(|k| k.handle(button)) else {
            return Effect::None;
        };
        if target == Id::WifiSwitch && !value.is_empty() && !wpa_valid(&value) {
            self.fail(None, "settings.wpa_length".into());
            return Effect::None;
        }
        if target == Id::RootPassword && !super::storage::root_password_valid(&value) {
            self.fail(None, "settings.root_invalid".into());
            return Effect::None;
        }
        self.keyboard = None;
        self.keyboard_target = None;
        self.error = None;
        self.back();
        match target {
            Id::Accent => {
                if let Some(rgb) = parse_hex(&value) {
                    self.custom = super::Custom::from_rgb(rgb);
                }
                Effect::None
            }
            Id::Hidden => {
                self.wifi_ssid = value;
                self.wifi_hidden = true;
                self.open_keyboard(Id::WifiSwitch, Mode::Secret, String::new());
                Effect::None
            }
            Id::WifiSwitch => {
                self.connecting = Some(self.wifi_ssid.clone());
                Effect::Connect(self.wifi_ssid.clone(), value)
            }
            Id::ScraperAccount => {
                // A draft until the service accepts it and the login is saved.
                self.scrape.draft = value;
                self.open_keyboard(Id::ScraperPassword, Mode::Secret, String::new());
                Effect::None
            }
            Id::ScraperPassword => {
                self.scrape.verifying = true;
                Effect::Login(std::mem::take(&mut self.scrape.draft), value)
            }
            Id::RootPassword => {
                self.root_password_typed(value);
                Effect::None
            }
            id => {
                self.pending = Some(id);
                Effect::Set(id, value)
            }
        }
    }
    fn wifi_input(&mut self, button: Button) -> Option<Effect> {
        let last = self.wifi_rows() - 1;
        let (bluetooth, services, hidden) = (last, last - 1, last - 2);
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(last),
            // X forgets a saved network (Config-wifi); Y refreshes anywhere.
            Button::X => {
                let network = self.wifi_network()?;
                if network.known || network.connected {
                    self.confirm_forget = Some(network.ssid.clone());
                }
            }
            Button::Y if self.value(Id::WifiSwitch) == "1" => return Some(Effect::Scan),
            Button::A if self.cursor == bluetooth => self.open(Page::Bluetooth),
            Button::A if self.cursor == services => self.open(Page::Services),
            Button::A if self.cursor == hidden => {
                self.wifi_hidden = true;
                self.open_keyboard(Id::Hidden, Mode::Text, String::new());
            }
            Button::A if self.cursor > 0 => {
                let network = self.wifi_network()?.clone();
                if network.connected {
                    return Some(Effect::None);
                }
                self.wifi_ssid = network.ssid.clone();
                self.wifi_hidden = false;
                match network.security.as_str() {
                    "open" => {
                        self.connecting = Some(network.ssid.clone());
                        return Some(Effect::Connect(network.ssid, String::new()));
                    }
                    // EAP and WEP need credentials this screen cannot collect.
                    "8021x" | "wep" => {
                        self.fail(None, "settings.wifi_unsupported".into());
                    }
                    _ => self.open_keyboard(Id::WifiSwitch, Mode::Secret, String::new()),
                }
            }
            // The toggle row uses the shared row handling.
            Button::A | Button::L | Button::R if self.cursor == 0 => return None,
            Button::B => return None,
            _ => {}
        }
        Some(Effect::None)
    }
    /// The choices on the Appearance page, drafts included.
    pub fn appearance(&self) -> Appearance {
        let text = |id| self.value(id).to_owned();
        Appearance {
            theme: text(Id::Theme),
            accent: text(Id::Accent),
            background: text(Id::Background),
            font: text(Id::Font),
            text_size: text(Id::TextSize),
            animations: self.value(Id::Animations) == "1",
        }
    }
    /// B saves the draft only when something changed.
    pub(super) fn leave_appearance(&mut self) -> Effect {
        let changed = self.appearance() != self.appearance_saved;
        self.back();
        if changed {
            self.pending = Some(Id::Theme);
            Effect::Appearance
        } else {
            Effect::None
        }
    }
    /// A on rows that open something instead of changing a value.
    pub(super) fn activate(&mut self, id: Id) -> Option<Effect> {
        match id {
            Id::Accent if self.open_custom() => {}
            Id::NetplayLink => self.open(Page::Netplay),
            Id::ScraperLink => self.open(Page::Scraper),
            Id::MangohudLink => self.open(Page::Mangohud),
            Id::MangohudItems => self.open(Page::MangohudItems),
            Id::ScraperAccount => {
                let user = self.scrape.user.clone();
                self.open_keyboard(Id::ScraperAccount, Mode::Text, user);
            }
            Id::Restore => {
                let theme = self.value(Id::Theme) != Id::Theme.default_value();
                for id in [
                    Id::Theme,
                    Id::Accent,
                    Id::Background,
                    Id::Font,
                    Id::TextSize,
                    Id::Animations,
                ] {
                    self.values.insert(id, id.default_value().into());
                }
                return Some(if theme {
                    Effect::Theme(Id::Theme.default_value().into())
                } else {
                    Effect::None
                });
            }
            Id::ScraperStart if self.scrape.running => return Some(Effect::CancelScrape),
            // Pending from the first press: a second A never starts a second job.
            Id::ScraperStart if self.scrape.starting => {}
            Id::ScraperStart => {
                self.scrape.outcome = None;
                self.scrape.starting = true;
                return Some(Effect::Scrape);
            }
            id if Self::text_input(id) => {
                let mode = if id == Id::NetplayPort {
                    Mode::Digits
                } else {
                    Mode::Text
                };
                self.open_keyboard(id, mode, self.value(id).to_owned());
            }
            id => return self.storage_activate(id),
        }
        Some(Effect::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{settings::network::Network, HintButton};
    fn network(ssid: &str, security: &str, known: bool) -> Network {
        Network {
            ssid: ssid.into(),
            security: security.into(),
            signal: 60,
            connected: false,
            known,
        }
    }
    fn wifi() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Wifi);
        state
    }
    #[test]
    fn password_entry_connects_and_blocks_input_until_the_reply() {
        let mut state = wifi();
        state.networks.push(network("Casa", "psk", false));
        state.cursor = 1;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Keyboard);
        for _ in 0..7 {
            state.handle(Button::A);
        }
        assert_eq!(state.handle(Button::Start), Effect::None);
        assert!(state.error.is_some());
        state.handle(Button::A);
        assert_eq!(
            state.handle(Button::Start),
            Effect::Connect("Casa".into(), "qqqqqqqq".into())
        );
        assert_eq!(state.page(), Page::Wifi);
        assert_eq!(state.connecting.as_deref(), Some("Casa"));
        assert_eq!(state.handle(Button::B), Effect::None);
        assert_eq!(state.page(), Page::Wifi);
    }
    #[test]
    fn open_hidden_network_confirms_an_empty_password_with_start() {
        let mut state = wifi();
        state.cursor = state.wifi_rows() - 3;
        state.handle(Button::A);
        state.handle(Button::A);
        assert_eq!(state.handle(Button::Start), Effect::None);
        assert_eq!(state.keyboard_target, Some(Id::WifiSwitch));
        assert_eq!(
            state.handle(Button::Start),
            Effect::Connect("q".into(), String::new())
        );
    }
    #[test]
    fn forget_asks_first_and_unsupported_security_explains() {
        let mut state = wifi();
        state.networks.push(network("Old", "wep", true));
        state.cursor = 1;
        assert_eq!(state.handle(Button::X), Effect::None);
        assert_eq!(state.confirm_forget.as_deref(), Some("Old"));
        assert_eq!(state.handle(Button::A), Effect::Forget("Old".into()));
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Wifi);
        assert_eq!(
            state.error.as_ref().map(|e| e.1.as_str()),
            Some("settings.wifi_unsupported")
        );
    }
    #[test]
    fn wifi_routes_to_services_and_bluetooth_and_back() {
        let mut state = wifi();
        for (offset, page) in [(2, Page::Services), (1, Page::Bluetooth)] {
            state.cursor = state.wifi_rows() - offset;
            state.handle(Button::A);
            assert_eq!(state.page(), page);
            assert_eq!(state.parent_page(), Page::Wifi);
            state.handle(Button::B);
            assert_eq!(state.page(), Page::Wifi);
        }
    }
    #[test]
    fn netplay_fields_open_the_right_keyboard_mode() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Netplay);
        state.cursor = 1;
        state.handle(Button::A);
        assert_eq!(state.keyboard.as_ref().unwrap().mode, Mode::Text);
        assert_eq!(state.keyboard.as_ref().unwrap().max, 32);
        state.handle(Button::B);
        state.cursor = 3;
        state.handle(Button::A);
        assert_eq!(state.keyboard.as_ref().unwrap().mode, Mode::Digits);
    }
    #[test]
    fn scraper_login_asks_user_then_secret_password() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.scrape.available = true;
        state.open(Page::Scraper);
        state.cursor = 1;
        state.handle(Button::A);
        state.handle(Button::A);
        state.handle(Button::Start);
        assert_eq!(state.keyboard.as_ref().unwrap().mode, Mode::Secret);
        state.handle(Button::A);
        assert_eq!(
            state.handle(Button::Start),
            Effect::Login("q".into(), "q".into())
        );
        assert!(state.scrape.verifying);
        state.cursor = 5;
        assert_eq!(state.handle(Button::A), Effect::None);
    }
    #[test]
    fn unverified_login_never_replaces_the_shown_account() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.scrape.available = true;
        state.scrape.user = "saved".into();
        state.open(Page::Scraper);
        state.cursor = 1;
        state.handle(Button::A);
        state.handle(Button::A);
        state.handle(Button::Start);
        assert_eq!(state.scrape.user, "saved");
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Scraper);
        assert_eq!(state.scrape.user, "saved");
    }
    #[test]
    fn a_second_start_press_before_the_reply_starts_nothing() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.scrape.available = true;
        state.open(Page::Scraper);
        state.cursor = 5;
        assert_eq!(state.handle(Button::A), Effect::Scrape);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.scrape.starting);
    }
    #[test]
    fn netplay_text_fields_can_be_cleared_but_the_port_cannot() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.values.insert(Id::NetplayHost, "1".into());
        state.open(Page::Netplay);
        state.cursor = 2;
        state.handle(Button::A);
        state.handle(Button::Y);
        assert_eq!(
            state.handle(Button::Start),
            Effect::Set(Id::NetplayHost, String::new())
        );
        state.pending = None;
        state.cursor = 3;
        state.handle(Button::A);
        for _ in 0..5 {
            state.handle(Button::Y);
        }
        assert_eq!(state.handle(Button::Start), Effect::None);
    }
    #[test]
    fn y_refreshes_everywhere_and_x_forgets_only_saved_networks() {
        let mut state = wifi();
        state.values.insert(Id::WifiSwitch, "1".into());
        state.networks.push(network("Guest", "open", false));
        state.networks.push(network("Home", "psk", true));
        for cursor in 0..state.wifi_rows() {
            state.cursor = cursor;
            assert_eq!(state.handle(Button::Y), Effect::Scan);
            let forget = state
                .hints()
                .contains(&(HintButton::Key('X'), "settings.forget"));
            assert_eq!(forget, cursor == 2);
            assert!(state
                .hints()
                .contains(&(HintButton::Key('Y'), "settings.refresh")));
        }
        state.cursor = 1;
        assert_eq!(state.handle(Button::X), Effect::None);
        assert_eq!(state.confirm_forget, None);
        state.cursor = 2;
        assert_eq!(state.handle(Button::X), Effect::None);
        assert_eq!(state.confirm_forget.as_deref(), Some("Home"));
    }
    #[test]
    fn effect_debug_never_shows_passwords() {
        const SENTINEL: &str = "sentinel-p4ssw0rd";
        for effect in [
            Effect::Connect("Casa".into(), SENTINEL.into()),
            Effect::Login("user".into(), SENTINEL.into()),
        ] {
            let text = format!("{effect:?}");
            assert!(!text.contains(SENTINEL), "{text}");
            assert!(text.contains("<hidden>"));
        }
    }
}
