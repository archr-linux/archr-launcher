//! Declarative settings pages and pure navigation. Effects belong to the host.
use crate::{icons::Glyph, state::Button, HintButton};
pub mod network;
mod pages;
use network::{Keyboard, Network};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Root,
    Sound,
    System,
    Region,
    Info,
    Wifi,
    Services,
    Games,
    Netplay,
    Scraper,
    Keyboard,
    Soon(&'static str),
}
impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Self::Root => "settings.title",
            Self::Sound => "settings.sound",
            Self::System => "settings.system",
            Self::Region => "settings.region",
            Self::Info => "settings.info",
            Self::Wifi => "settings.network",
            Self::Services => "settings.services",
            Self::Games => "settings.games",
            Self::Netplay => "settings.netplay",
            Self::Scraper => "settings.scraper",
            Self::Keyboard => "settings.keyboard",
            Self::Soon(key) => key,
        }
    }
    pub fn rows(self) -> &'static [Row] {
        match self {
            Self::Sound => SOUND,
            Self::System => SYSTEM,
            Self::Region => REGION,
            Self::Wifi => pages::WIFI,
            Self::Services => pages::SERVICES,
            Self::Games => pages::GAMES,
            Self::Netplay => pages::NETPLAY,
            Self::Scraper => pages::SCRAPER,
            _ => &[],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Id {
    Volume,
    Brightness,
    Audio,
    Idle,
    IdleGame,
    Gpu,
    Suspend,
    Governor,
    Turbo,
    Update,
    Language,
    Timezone,
    ClockFormat,
    KeyboardLayout,
    KeyboardVariant,
    WifiSwitch,
    Hidden,
    Bluetooth,
    ServicesLink,
    Ssh,
    Samba,
    Http,
    NetplayLink,
    Adhoc,
    Player,
    Channel,
    UsbMode,
    NetplayEnabled,
    Nickname,
    NetplayHost,
    NetplayPort,
    Relay,
    CustomServer,
    AutoLobby,
    OnlyRelay,
    MissingGames,
    CheckIndexes,
    IndexNow,
    ScraperLink,
    ScraperSource,
    ScraperAccount,
    ScraperSystems,
    ScraperMedia,
    ScraperMissing,
    ScraperStart,
}
impl Id {
    fn metadata(self) -> (&'static str, &'static str) {
        match self {
            Self::Volume => ("audio.volume", "60"),
            Self::Brightness => ("display.brightness", "50"),
            Self::Audio => ("audio.device", ""),
            Self::Idle => ("idle.timeout", "0"),
            Self::IdleGame => ("idle.timeout.game", "0"),
            Self::Gpu => ("gpu.driver", "libmali"),
            Self::Suspend => ("system.suspendmode", "off"),
            Self::Governor => ("system.cpugovernor", "ondemand"),
            Self::Turbo => ("enable.turbo-mode", "0"),
            Self::Update => ("", ""),
            Self::Language => ("system.language", "en_US"),
            Self::Timezone => ("system.timezone", "America/Sao_Paulo"),
            Self::ClockFormat => ("launcher.clock.format", "24"),
            Self::KeyboardLayout => ("system.kblayout", "us"),
            Self::KeyboardVariant => ("system.kbvariant", ""),
            Self::WifiSwitch => ("wifi.enabled", "0"),
            Self::Ssh => ("ssh.enabled", "0"),
            Self::Samba => ("samba.enabled", "0"),
            Self::Http => ("simplehttp.enabled", "0"),
            Self::Adhoc => ("wifi.adhoc.enabled", "0"),
            Self::Player => ("wifi.adhoc.id", "1"),
            Self::Channel => ("wifi.adhoc.channel", "6"),
            Self::UsbMode => ("", "disabled"), // usbgadget owns its state file
            Self::NetplayEnabled => ("global.netplay", "0"),
            Self::Nickname => ("global.netplay.nickname", ""),
            Self::NetplayHost => ("global.netplay.host", ""),
            Self::NetplayPort => ("global.netplay.port", "55435"),
            Self::Relay => ("global.netplay.relay", "none"),
            Self::CustomServer => ("global.netplay.customserver", ""),
            Self::AutoLobby => ("netplay.auto_lobby", "0"),
            Self::OnlyRelay => ("netplay.relay_only", "0"),
            Self::MissingGames => ("netplay.show_missing", "0"),
            Self::CheckIndexes => ("netplay.index_at_start", "0"),
            Self::ScraperSource
            | Self::ScraperAccount
            | Self::ScraperSystems
            | Self::ScraperMedia
            | Self::ScraperMissing
            | Self::ScraperStart
            | Self::ScraperLink
            | Self::IndexNow
            | Self::Hidden
            | Self::Bluetooth
            | Self::ServicesLink
            | Self::NetplayLink => ("", ""),
        }
    }
    pub fn key(self) -> &'static str {
        self.metadata().0
    }
    pub fn default_value(self) -> &'static str {
        self.metadata().1
    }
    pub fn choices(self) -> &'static [&'static str] {
        match self {
            Self::Gpu => &["libmali", "panfrost"],
            Self::Language => &["en_US", "pt_BR", "es_ES", "ru_RU"],
            Self::Idle | Self::IdleGame => &["0", "1", "3", "5", "10", "15", "30"],
            Self::Governor => &["ondemand", "performance", "schedutil", "powersave"],
            Self::Turbo
            | Self::WifiSwitch
            | Self::Ssh
            | Self::Samba
            | Self::Http
            | Self::Adhoc
            | Self::NetplayEnabled
            | Self::AutoLobby
            | Self::OnlyRelay
            | Self::MissingGames
            | Self::CheckIndexes => &["0", "1"],
            Self::ClockFormat => &["24", "12"],
            Self::Player => &["1", "2", "3", "4"],
            Self::Channel => &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"],
            Self::UsbMode => &["disabled", "network", "file_transfer"],
            Self::Relay => &["none", "nyc", "madrid", "montreal", "saopaulo", "custom"],
            _ => &[],
        }
    }
    pub fn normalize(self, value: &str) -> String {
        if !matches!(self, Self::Volume | Self::Brightness) {
            return value.into();
        }
        let min = if self == Self::Brightness { 5 } else { 0 };
        let value = value.trim();
        let digits = value.strip_prefix(['-', '+']).unwrap_or(value);
        let number = if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            if value.starts_with('-') {
                0
            } else {
                digits
                    .bytes()
                    .fold(0u8, |v, b| v.saturating_mul(10).saturating_add(b - b'0'))
            }
        } else {
            self.default_value().parse().expect("numeric default")
        };
        number.clamp(min, 100).to_string()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Toggle,
    Value,
    Slider { min: u8, max: u8, step: u8 },
    Action,
}
#[derive(Clone, Copy)]
pub struct Row {
    pub id: Id,
    pub label: &'static str,
    pub subtitle: Option<&'static str>,
    pub kind: Kind,
}
const fn row(id: Id, label: &'static str, kind: Kind) -> Row {
    Row {
        id,
        label,
        subtitle: None,
        kind,
    }
}
const fn slider(id: Id, label: &'static str, min: u8) -> Row {
    row(
        id,
        label,
        Kind::Slider {
            min,
            max: 100,
            step: 5,
        },
    )
}
const fn described(id: Id, label: &'static str, subtitle: &'static str) -> Row {
    Row {
        subtitle: Some(subtitle),
        ..row(id, label, Kind::Value)
    }
}
const SOUND: &[Row] = &[
    slider(Id::Volume, "settings.volume", 0),
    slider(Id::Brightness, "settings.brightness", 5),
    row(Id::Audio, "settings.audio", Kind::Value),
    described(Id::Idle, "settings.idle", "settings.idle_subtitle"),
    described(
        Id::IdleGame,
        "settings.idle_game",
        "settings.idle_game_subtitle",
    ),
];
const SYSTEM: &[Row] = &[
    described(Id::Gpu, "settings.gpu", "settings.reboot_required"),
    row(Id::Suspend, "settings.suspend", Kind::Value),
    row(Id::Governor, "settings.governor", Kind::Value),
    row(Id::Turbo, "settings.turbo", Kind::Toggle),
    row(Id::Update, "settings.update", Kind::Action),
];
const REGION: &[Row] = &[
    row(Id::Language, "settings.language", Kind::Value),
    row(Id::Timezone, "settings.timezone", Kind::Value),
    row(Id::ClockFormat, "settings.clock_format", Kind::Value),
    described(
        Id::KeyboardLayout,
        "settings.keyboard_layout",
        "settings.keyboard_detail",
    ),
    row(
        Id::KeyboardVariant,
        "settings.keyboard_variant",
        Kind::Value,
    ),
];
/// Navigation, detail copy and shared icon identity have one source of truth.
pub(crate) const CATEGORIES: &[(Page, &str, Glyph)] = &[
    (Page::Wifi, "settings.network_detail", Glyph::Wifi),
    (Page::Sound, "settings.sound_detail", Glyph::Sound),
    (
        Page::Soon("settings.controls"),
        "settings.controls_detail",
        Glyph::Controls,
    ),
    (Page::Games, "settings.games_detail", Glyph::Play),
    (Page::Region, "settings.region_detail", Glyph::Globe),
    (
        Page::Soon("settings.appearance"),
        "settings.appearance_detail",
        Glyph::Palette,
    ),
    (
        Page::Soon("settings.achievements"),
        "settings.achievements_detail",
        Glyph::Trophy,
    ),
    (Page::System, "settings.system_detail", Glyph::Gear),
    (
        Page::Soon("settings.storage"),
        "settings.storage_detail",
        Glyph::Storage,
    ),
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Close,
    Set(Id, String),
    Reboot,
    Scan,
    Connect(String, String),
    Forget(String),
}
#[derive(Clone, Default)]
pub struct InfoCard {
    pub title: &'static str,
    pub lines: Vec<String>,
    pub percent: Option<u8>,
}
#[derive(Clone, Default)]
pub struct Information {
    pub version: String,
    pub device: String,
    pub kernel: String,
    pub uptime: String,
    pub cards: Vec<InfoCard>,
}
#[derive(Clone)]
pub struct Settings {
    stack: Vec<(Page, usize)>,
    pub cursor: usize,
    pub values: BTreeMap<Id, String>,
    pub options: BTreeMap<Id, Vec<String>>,
    pub pending: Option<Id>,
    pub loading: bool,
    pub error: Option<(Option<Id>, String)>,
    pub reboot_prompt: bool,
    pub info: Information,
    pub networks: Vec<Network>,
    pub keyboard: Option<Keyboard>,
    pub keyboard_target: Option<Id>,
    pub wifi_ssid: String,
    pub wifi_hidden: bool,
    pub access_ip: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            stack: vec![(Page::Root, 0)],
            cursor: 0,
            values: BTreeMap::new(),
            options: [Id::Gpu, Id::Language, Id::Idle, Id::IdleGame]
                .into_iter()
                .map(|id| (id, id.choices().iter().map(|v| (*v).into()).collect()))
                .collect(),
            pending: None,
            loading: true,
            error: None,
            reboot_prompt: false,
            info: Information::default(),
            networks: Vec::new(),
            keyboard: None,
            keyboard_target: None,
            wifi_ssid: String::new(),
            wifi_hidden: false,
            access_ip: String::new(),
        }
    }
}
impl Settings {
    pub fn display_value(&self, id: Id, catalog: &crate::i18n::I18n) -> String {
        let value = self.value(id);
        let key = match (id, value) {
            (Id::Language, "en_US") => "settings.lang_en",
            (Id::Language, "pt_BR") => "settings.lang_pt",
            (Id::Language, "es_ES") => "settings.lang_es",
            (Id::Language, "ru_RU") => "settings.lang_ru",
            (Id::Idle | Id::IdleGame, "0") => "settings.never",
            (Id::Idle | Id::IdleGame, v) => {
                return catalog.format("settings.minutes", &[("count", v)])
            }
            (Id::Suspend, "off") => "settings.off",
            (Id::Suspend, "default") => "menu.default",
            (Id::Suspend, "mem") => "settings.suspend_mem",
            (Id::Suspend, "freeze") => "settings.suspend_freeze",
            (Id::Suspend, "standby") => "settings.suspend_standby",
            (
                Id::Bluetooth
                | Id::IndexNow
                | Id::ScraperAccount
                | Id::ScraperMedia
                | Id::ScraperStart,
                _,
            ) => "settings.soon",
            (Id::Hidden | Id::ServicesLink | Id::NetplayLink | Id::ScraperLink, _) => {
                return "›".into()
            }
            (Id::KeyboardVariant, "") => "menu.default",
            (Id::ClockFormat, "24") => "settings.clock_24",
            (Id::ClockFormat, "12") => "settings.clock_12",
            (Id::UsbMode, "disabled") => "settings.off",
            (Id::UsbMode, "network") => "settings.usb_network",
            (Id::UsbMode, "file_transfer") => "settings.usb_files",
            (Id::Relay, "none") => "settings.off",
            (Id::Nickname | Id::NetplayHost | Id::CustomServer, "") => "settings.define",
            (_, "") => "settings.unavailable",
            _ => return value.into(),
        };
        catalog.text(key)
    }
    pub fn visible_rows(&self) -> Vec<&'static Row> {
        self.page()
            .rows()
            .iter()
            .filter(|row| {
                self.page() != Page::Netplay
                    || (!matches!(
                        row.id,
                        Id::NetplayHost
                            | Id::NetplayPort
                            | Id::Relay
                            | Id::CustomServer
                            | Id::AutoLobby
                            | Id::OnlyRelay
                            | Id::MissingGames
                    ) || self.value(Id::Adhoc) != "1")
                        && (row.id != Id::CustomServer || self.value(Id::Relay) == "custom")
            })
            .collect()
    }
    pub fn hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::{Key, Pair};
        match self.page() {
            Page::Wifi => &[
                (Key('A'), "hint.choose"),
                (Key('X'), "settings.refresh"),
                (Key('Y'), "settings.forget"),
                (Key('B'), "hint.back"),
            ],
            Page::Keyboard if self.keyboard_target == Some(Id::WifiSwitch) => &[
                (Key('A'), "hint.choose"),
                (Key('X'), "settings.delete"),
                (Key('Y'), "settings.shift"),
                (HintButton::Start, "settings.connect"),
                (Key('B'), "hint.back"),
            ],
            Page::Keyboard => &[
                (Key('A'), "hint.choose"),
                (Key('X'), "settings.delete"),
                (Key('Y'), "settings.shift"),
                (HintButton::Start, "hint.confirm"),
                (Key('B'), "hint.back"),
            ],
            Page::Root => &[
                (Key('A'), "hint.open"),
                (Key('B'), "hint.back"),
                (Key('Y'), "settings.info_short"),
            ],
            Page::Scraper => &[(Key('B'), "hint.back")],
            Page::Netplay
                if self
                    .visible_rows()
                    .get(self.cursor)
                    .is_some_and(|row| row.id == Id::IndexNow) =>
            {
                &[(Key('B'), "hint.back")]
            }
            _ => match self.visible_rows().get(self.cursor).map(|r| r.kind) {
                Some(Kind::Action) => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
                Some(Kind::Toggle) => &[
                    (Pair('L', 'R'), "settings.change"),
                    (Key('A'), "hint.choose"),
                    (Key('B'), "hint.back"),
                ],
                Some(_) => &[(Pair('L', 'R'), "settings.change"), (Key('B'), "hint.back")],
                None => &[(Key('B'), "hint.back")],
            },
        }
    }
    pub fn page(&self) -> Page {
        self.stack.last().expect("root page").0
    }
    pub fn open(&mut self, page: Page) {
        self.stack.last_mut().expect("root page").1 = self.cursor;
        self.stack.push((page, 0));
        self.cursor = 0;
    }
    pub fn value(&self, id: Id) -> &str {
        self.values
            .get(&id)
            .map_or(id.default_value(), String::as_str)
    }
    pub fn complete(&mut self, id: Id, result: Result<String, String>) {
        self.pending = None;
        match result {
            Ok(value) => {
                self.values.insert(id, value);
                self.error = None;
                self.reboot_prompt = id == Id::Gpu;
            }
            Err(error) => self.error = Some((Some(id), error)),
        }
    }
    pub fn handle(&mut self, button: Button) -> Effect {
        if self.reboot_prompt {
            return match button {
                Button::A => Effect::Reboot,
                Button::B => {
                    self.reboot_prompt = false;
                    Effect::None
                }
                _ => Effect::None,
            };
        }
        let page = self.page();
        if page == Page::Keyboard {
            return self.handle_keyboard(button);
        }
        if page == Page::Wifi {
            return self.handle_wifi(button);
        }
        if page == Page::Scraper {
            match button {
                Button::B => {
                    self.stack.pop();
                    self.cursor = self.stack.last().expect("parent page").1;
                }
                Button::Up => self.cursor = self.cursor.saturating_sub(1),
                Button::Down => self.cursor = (self.cursor + 1).min(page.rows().len() - 1),
                _ => {}
            }
            return Effect::None;
        }
        let rows = self.visible_rows();
        let len = if page == Page::Root {
            CATEGORIES.len()
        } else {
            rows.len()
        };
        match button {
            Button::B => {
                if self.stack.len() == 1 {
                    return Effect::Close;
                }
                self.stack.pop();
                self.cursor = self.stack.last().expect("root page").1;
            }
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(len.saturating_sub(1)),
            Button::Y if page == Page::Root => self.open(Page::Info),
            Button::A if page == Page::Root => {
                self.open(CATEGORIES[self.cursor.min(CATEGORIES.len() - 1)].0)
            }
            Button::A | Button::L | Button::R if self.pending.is_none() && !self.loading => {
                let Some(row) = rows.get(self.cursor) else {
                    return Effect::None;
                };
                if button == Button::A {
                    if let Some(effect) = self.handle_special(row.id) {
                        return effect;
                    }
                }
                let old = self.value(row.id);
                let next = match row.kind {
                    Kind::Toggle => {
                        if old == "1" {
                            "0".into()
                        } else {
                            "1".into()
                        }
                    }
                    Kind::Slider { min, max, step } if button != Button::A => {
                        let old = row
                            .id
                            .normalize(old)
                            .parse::<u8>()
                            .expect("normalized slider");
                        if button == Button::L {
                            old.saturating_sub(step)
                        } else {
                            old.saturating_add(step)
                        }
                        .clamp(min, max)
                        .to_string()
                    }
                    Kind::Value if button != Button::A => {
                        let Some(options) = self.options.get(&row.id).filter(|v| !v.is_empty())
                        else {
                            return Effect::None;
                        };
                        let index = options.iter().position(|v| v == old).unwrap_or(0);
                        options[(index
                            + if button == Button::L {
                                options.len() - 1
                            } else {
                                1
                            })
                            % options.len()]
                        .clone()
                    }
                    Kind::Action if button == Button::A => {
                        self.open(Page::Soon("settings.update"));
                        return Effect::None;
                    }
                    _ => return Effect::None,
                };
                if next != old {
                    self.pending = Some(row.id);
                    self.error = None;
                    return Effect::Set(row.id, next);
                }
            }
            _ => {}
        }
        Effect::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_and_games_use_distinct_shared_glyphs() {
        assert_eq!(CATEGORIES[2].2, Glyph::Controls);
        assert_eq!(CATEGORIES[3].2, Glyph::Play);
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;
    #[test]
    fn lobby_keys_match_the_independent_netplay_consumer() {
        assert_eq!(Id::AutoLobby.key(), "netplay.auto_lobby");
        assert_eq!(Id::OnlyRelay.key(), "netplay.relay_only");
        assert_eq!(Id::MissingGames.key(), "netplay.show_missing");
        assert_eq!(Id::CheckIndexes.key(), "netplay.index_at_start");
    }

    #[test]
    fn empty_netplay_text_fields_prompt_to_define_instead_of_unavailable() {
        let state = Settings::default();
        let catalog = crate::i18n::I18n::default();
        for id in [Id::Nickname, Id::NetplayHost, Id::CustomServer] {
            assert_eq!(
                state.display_value(id, &catalog),
                catalog.text("settings.define")
            );
        }
    }

    #[test]
    fn netplay_text_fields_open_localized_keyboard_and_host_starts_numeric() {
        let mut state = Settings::default();
        state.open(Page::Netplay);
        state.loading = false;
        state.cursor = 1;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.keyboard.as_ref().unwrap().title, "settings.nickname");
        assert!(!state.keyboard.as_ref().unwrap().numeric);
        state.handle(Button::B);
        state.cursor = 2;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.keyboard.as_ref().unwrap().title, "settings.host");
        assert!(state.keyboard.as_ref().unwrap().numeric);
    }

    #[test]
    fn keyboard_footer_has_a_start_connect_badge() {
        let mut state = Settings::default();
        state.open_keyboard(Id::WifiSwitch, "AP".into(), true, String::new());
        assert!(state
            .hints()
            .contains(&(HintButton::Start, "settings.connect")));
        state.open_keyboard(
            Id::NetplayHost,
            "settings.host".into(),
            false,
            String::new(),
        );
        assert!(state.hints().contains(&(HintButton::Start, "hint.confirm")));
    }

    #[test]
    fn compact_rows_keep_non_selectable_sections_and_real_heights() {
        let mut state = Settings::default();
        state.open(Page::Services);
        let rows = state.compact_rows();
        assert_eq!(rows[0].section, Some("settings.access"));
        assert_eq!(rows[0].height, 38.0);
        assert!(rows[4].top > rows[3].top + rows[3].height);
        state.open(Page::Netplay);
        let rows = state.compact_rows();
        assert_eq!(rows[2].section, Some("settings.connection"));
        assert_eq!(rows[2].height, 34.0);
    }

    #[test]
    fn service_netplay_link_is_an_action_and_wifi_hint_has_no_duplicate_button() {
        assert_eq!(pages::SERVICES[3].kind, Kind::Action);
        let mut state = Settings::default();
        state.open(Page::Wifi);
        assert_eq!(state.hints()[1].1, "settings.refresh");
    }

    #[test]
    fn disabled_scraping_and_index_actions_have_only_back_hint() {
        let mut state = Settings::default();
        state.open(Page::Scraper);
        assert_eq!(state.hints().len(), 1);
        state.open(Page::Netplay);
        state.cursor = state.visible_rows().len() - 1;
        assert_eq!(state.visible_rows()[state.cursor].id, Id::IndexNow);
        assert_eq!(state.hints().len(), 1);
        state.cursor = 5;
        assert_eq!(state.visible_rows()[state.cursor].id, Id::AutoLobby);
        assert_eq!(state.hints().len(), 3);
    }

    #[test]
    fn legacy_wep_network_does_not_offer_a_wpa_password_prompt() {
        let mut state = Settings::default();
        state.open(Page::Wifi);
        state.loading = false;
        state.networks.push(network::Network {
            ssid: "Old AP".into(),
            security: "wep".into(),
            signal: "33".into(),
            connected: false,
            known: false,
        });
        state.cursor = 1;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Wifi);
    }

    #[test]
    fn network_routes_to_services_and_netplay() {
        let mut state = Settings::default();
        state.open(Page::Wifi);
        state.loading = false;
        state.cursor = 3;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Services);
        state.cursor = 3;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Netplay);
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Services);
    }
    #[test]
    fn netplay_hides_host_and_port_for_local_play() {
        let mut state = Settings::default();
        state.values.insert(Id::Adhoc, "1".into());
        assert!(!state
            .visible_rows()
            .iter()
            .any(|r| matches!(r.id, Id::NetplayHost | Id::NetplayPort)));
    }
}
