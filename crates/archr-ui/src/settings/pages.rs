//! Declarative rows for network, games and regional preferences.
use super::{described, row, Id, Kind, Row};

pub const WIFI: &[Row] = &[
    row(Id::WifiSwitch, "settings.wifi", Kind::Toggle),
    row(Id::Hidden, "settings.hidden", Kind::Action),
    row(Id::Bluetooth, "settings.bluetooth", Kind::Action),
    row(Id::ServicesLink, "settings.services", Kind::Action),
];
pub const SERVICES: &[Row] = &[
    Row {
        kind: Kind::Toggle,
        ..described(Id::Ssh, "settings.ssh", "settings.ssh_detail")
    },
    Row {
        kind: Kind::Toggle,
        ..described(Id::Samba, "settings.samba", "settings.samba_detail")
    },
    Row {
        kind: Kind::Toggle,
        ..described(Id::Http, "settings.http", "settings.http_detail")
    },
    Row {
        kind: Kind::Action,
        ..described(
            Id::NetplayLink,
            "settings.netplay",
            "settings.netplay_detail",
        )
    },
    Row {
        kind: Kind::Toggle,
        ..described(Id::Adhoc, "settings.adhoc", "settings.adhoc_detail")
    },
    described(Id::Player, "settings.player", "settings.player_detail"),
    described(Id::Channel, "settings.channel", "settings.channel_detail"),
    described(Id::UsbMode, "settings.usb_mode", "settings.usb_detail"),
];
pub const GAMES: &[Row] = &[
    row(Id::NetplayLink, "settings.netplay", Kind::Action),
    row(Id::ScraperLink, "settings.scraper", Kind::Action),
];
pub const NETPLAY: &[Row] = &[
    row(Id::NetplayEnabled, "settings.netplay_enabled", Kind::Toggle),
    row(Id::Nickname, "settings.nickname", Kind::Value),
    row(Id::NetplayHost, "settings.host", Kind::Value),
    row(Id::NetplayPort, "settings.port", Kind::Value),
    row(Id::Relay, "settings.relay", Kind::Value),
    row(Id::CustomServer, "settings.custom_server", Kind::Value),
    row(Id::AutoLobby, "settings.auto_lobby", Kind::Toggle),
    row(Id::OnlyRelay, "settings.only_relay", Kind::Toggle),
    row(Id::MissingGames, "settings.missing_games", Kind::Toggle),
    row(Id::CheckIndexes, "settings.check_indexes", Kind::Toggle),
    row(Id::IndexNow, "settings.index_now", Kind::Action),
];
pub const SCRAPER: &[Row] = &[
    row(Id::ScraperSource, "settings.source", Kind::Value),
    row(Id::ScraperAccount, "settings.account", Kind::Action),
    row(Id::ScraperSystems, "settings.scraper_systems", Kind::Value),
    row(Id::ScraperMedia, "settings.media", Kind::Action),
    row(Id::ScraperMissing, "settings.only_missing", Kind::Toggle),
    row(Id::ScraperStart, "settings.start_scraping", Kind::Action),
];

use super::{network::Keyboard, Effect, Page, Settings};
use crate::state::Button;
impl Settings {
    pub(super) fn text_input(id: Id) -> bool {
        matches!(
            id,
            Id::Nickname | Id::NetplayHost | Id::NetplayPort | Id::CustomServer
        )
    }
    pub(super) fn open_keyboard(&mut self, id: Id, title: String, secret: bool, value: String) {
        self.keyboard = Some(Keyboard::new(title, secret, value));
        self.keyboard_target = Some(id);
        self.open(Page::Keyboard);
    }
    pub(super) fn handle_keyboard(&mut self, button: Button) -> Effect {
        if button == Button::B {
            self.keyboard = None;
            self.keyboard_target = None;
            self.stack.pop();
            self.cursor = self.stack.last().expect("parent page").1;
            return Effect::None;
        }
        if button == Button::Select
            && self.keyboard_target == Some(Id::WifiSwitch)
            && self.wifi_hidden
        {
            // A hidden network may be open; Select connects without a password.
            self.keyboard = None;
            self.keyboard_target = None;
            self.stack.pop();
            self.cursor = self.stack.last().expect("parent page").1;
            return Effect::Connect(self.wifi_ssid.clone(), String::new());
        }
        let Some(value) = self
            .keyboard
            .as_mut()
            .and_then(|keyboard| keyboard.handle(button))
        else {
            return Effect::None;
        };
        let target = self.keyboard_target.take().expect("keyboard target");
        self.keyboard = None;
        self.stack.pop();
        self.cursor = self.stack.last().expect("parent page").1;
        match target {
            Id::Hidden => {
                self.wifi_ssid = value;
                self.wifi_hidden = true;
                self.open_keyboard(Id::WifiSwitch, self.wifi_ssid.clone(), true, String::new());
                Effect::None
            }
            Id::WifiSwitch => Effect::Connect(self.wifi_ssid.clone(), value),
            id => {
                self.pending = Some(id);
                Effect::Set(id, value)
            }
        }
    }
    pub(super) fn handle_wifi(&mut self, button: Button) -> Effect {
        let last = self.networks.len() + 3;
        match button {
            Button::B => {
                self.stack.pop();
                self.cursor = self.stack.last().expect("parent page").1;
            }
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(last),
            Button::X => return Effect::Scan,
            Button::Y if (1..=self.networks.len()).contains(&self.cursor) => {
                let network = &self.networks[self.cursor - 1];
                if network.known || network.connected {
                    return Effect::Forget(network.ssid.clone());
                }
            }
            Button::A | Button::L | Button::R if self.pending.is_none() && !self.loading => {
                if self.cursor == 0 {
                    let next = if self.value(Id::WifiSwitch) == "1" {
                        "0"
                    } else {
                        "1"
                    };
                    self.pending = Some(Id::WifiSwitch);
                    return Effect::Set(Id::WifiSwitch, next.into());
                }
                if let Some(network) = self.networks.get(self.cursor - 1).cloned() {
                    if button != Button::A || network.connected {
                        return Effect::None;
                    }
                    self.wifi_ssid = network.ssid.clone();
                    self.wifi_hidden = false;
                    if network.security == "open" {
                        return Effect::Connect(network.ssid, String::new());
                    }
                    if matches!(network.security.as_str(), "8021x" | "wep") {
                        return Effect::None;
                    } // EAP and WEP need different credential entry.
                    self.open_keyboard(Id::WifiSwitch, network.ssid, true, String::new());
                } else if button == Button::A {
                    match self.cursor - self.networks.len() {
                        1 => self.open_keyboard(
                            Id::Hidden,
                            "settings.hidden".into(),
                            false,
                            String::new(),
                        ),
                        2 => self.open(Page::Soon("settings.bluetooth")),
                        3 => self.open(Page::Services),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Effect::None
    }
    pub(super) fn handle_special(&mut self, id: Id) -> Option<Effect> {
        match id {
            Id::NetplayLink => self.open(Page::Netplay),
            Id::ScraperLink => self.open(Page::Scraper),
            Id::ServicesLink => self.open(Page::Services),
            Id::Bluetooth => self.open(Page::Soon("settings.bluetooth")),
            Id::Hidden => {
                self.open_keyboard(Id::Hidden, "settings.hidden".into(), false, String::new())
            }
            Id::IndexNow
            | Id::ScraperSource
            | Id::ScraperAccount
            | Id::ScraperSystems
            | Id::ScraperMedia
            | Id::ScraperMissing
            | Id::ScraperStart => {}
            id if Self::text_input(id) => {
                self.open_keyboard(id, id.key().into(), false, self.value(id).to_owned());
            }
            _ => return None,
        }
        Some(Effect::None)
    }
}

/// Real compact-page geometry: sections consume space but never enter navigation.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub id: Id,
    pub top: f32,
    pub height: f32,
    pub section: Option<&'static str>,
}
impl Settings {
    pub fn compact_rows(&self) -> Vec<Placement> {
        let page = self.page();
        let mut top = 56.;
        self.visible_rows()
            .into_iter()
            .map(|row| {
                let section = match (page, row.id) {
                    (Page::Services, Id::Ssh) => Some("settings.access"),
                    (Page::Services, Id::NetplayLink) => Some("settings.game_usb"),
                    (Page::Services, Id::Adhoc) => Some("settings.local_play"),
                    (Page::Netplay, Id::NetplayHost) => Some("settings.connection"),
                    (Page::Netplay, Id::AutoLobby) => Some("settings.rooms"),
                    (Page::Netplay, Id::CheckIndexes) => Some("settings.games_index"),
                    (Page::Region, Id::KeyboardLayout) => Some("settings.keyboard_section"),
                    _ => None,
                };
                if section.is_some() {
                    top += 21.;
                }
                let height = match (page, row.id) {
                    (Page::Netplay, _) => 34.,
                    (Page::Services, _) => 38.,
                    (Page::Region, Id::KeyboardLayout) => 46.,
                    (Page::Region, _) => 40.,
                    (Page::Scraper, Id::ScraperAccount | Id::ScraperMedia) => 46.,
                    (Page::Scraper, _) => 40.,
                    _ => 38.,
                };
                let placement = Placement {
                    id: row.id,
                    top,
                    height,
                    section,
                };
                top += height + if page == Page::Netplay { 2. } else { 3. };
                placement
            })
            .collect()
    }
}
