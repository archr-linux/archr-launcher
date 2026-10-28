//! Declarative settings pages and pure navigation. Effects belong to the host.
use crate::{icons::Glyph, keyboard::Keyboard, state::Button, HintButton};
use std::collections::BTreeMap;
use std::time::Instant;
pub mod bluetooth;
pub(crate) mod games;
pub(crate) mod information;
pub use games::{GameCore, GameSystem, Games};
pub mod network;
mod pages;
use bluetooth::{Action, Device};
pub use information::{InfoCard, InfoDetail, InfoMeter, Information};
use network::Network;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Root,
    Sound,
    System,
    Region,
    Info,
    InfoDetail(usize),
    Wifi,
    Bluetooth,
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
            Self::Info | Self::InfoDetail(_) => "settings.info",
            Self::Wifi => "settings.wifi",
            Self::Bluetooth => "settings.bluetooth",
            Self::Services => "settings.services",
            Self::Games => "settings.games",
            Self::Netplay => "settings.netplay",
            Self::Scraper => "settings.scraper",
            Self::Keyboard => "settings.keyboard",
            Self::Soon(key) => key,
        }
    }
    /// Breadcrumb parents between "Settings" and the page itself.
    pub fn parent(self) -> Option<&'static str> {
        match self {
            Self::Wifi | Self::Bluetooth | Self::Services => Some("settings.network"),
            Self::Netplay | Self::Scraper => Some("settings.games"),
            _ => None,
        }
    }
    pub fn rows(self) -> &'static [Row] {
        match self {
            Self::Sound => SOUND,
            Self::System => SYSTEM,
            Self::Region => REGION,
            Self::Wifi => pages::WIFI,
            Self::Bluetooth => pages::BLUETOOTH,
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
    BtSwitch,
    Hidden,
    Ssh,
    Samba,
    Http,
    Adhoc,
    Player,
    Channel,
    UsbMode,
    Emulator,
    Core,
    Ratio,
    Filter,
    Autosave,
    StateSlot,
    GameSystem,
    NetplayLink,
    NetplayEnabled,
    Nickname,
    NetplayHost,
    NetplayPort,
    Relay,
    CustomServer,
    OnlyRelay,
    MissingGames,
    IndexNow,
    ScraperLink,
    ScraperSource,
    ScraperAccount,
    ScraperPassword,
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
            Self::Language => ("system.language", "en_US"),
            Self::Timezone => ("system.timezone", "America/Sao_Paulo"),
            Self::ClockFormat => ("launcher.clock.format", "24"),
            // archr-keyboard owns both keyboard keys.
            Self::KeyboardLayout => ("system.kblayout", "us"),
            Self::KeyboardVariant => ("system.kbvariant", ""),
            Self::WifiSwitch => ("wifi.enabled", "0"),
            // archr-bluetooth owns this key.
            Self::BtSwitch => ("controllers.bluetooth.enabled", "0"),
            Self::Ssh => ("ssh.enabled", "0"),
            Self::Samba => ("samba.enabled", "0"),
            Self::Http => ("simplehttp.enabled", "0"),
            Self::Adhoc => ("wifi.adhoc.enabled", "0"),
            Self::Player => ("wifi.adhoc.id", "1"),
            Self::Channel => ("wifi.adhoc.channel", "6"),
            Self::UsbMode => ("", "disabled"), // usbgadget owns its state file
            // Per-system suffixes: the Games page writes `<system>.<suffix>`,
            // the keys runemu.sh passes on and setsettings.sh reads.
            Self::Emulator => ("emulator", ""),
            Self::Core => ("core", ""),
            Self::Ratio => ("ratio", "core"),
            Self::Filter => ("smooth", "0"),
            Self::Autosave => ("autosave", "0"),
            Self::StateSlot => ("incrementalsavestates", "0"),
            Self::NetplayEnabled => ("global.netplay", "0"),
            Self::Nickname => ("global.netplay.nickname", ""),
            Self::NetplayHost => ("global.netplay.host", ""),
            Self::NetplayPort => ("global.netplay.port", "55435"),
            Self::Relay => ("global.netplay.relay", "none"),
            Self::CustomServer => ("global.netplay.customserver", ""),
            Self::OnlyRelay => ("netplay.relay_only", "0"),
            Self::MissingGames => ("netplay.show_missing", "1"),
            Self::ScraperSource => ("", "screenscraper"),
            Self::ScraperSystems => ("launcher.scraper.systems", ""),
            Self::ScraperMedia => ("launcher.scraper.media", "box"),
            Self::ScraperMissing => ("launcher.scraper.only_missing", "1"),
            Self::Update
            | Self::Hidden
            | Self::GameSystem
            | Self::NetplayLink
            | Self::IndexNow
            | Self::ScraperLink
            | Self::ScraperAccount
            | Self::ScraperPassword
            | Self::ScraperStart => ("", ""),
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
            | Self::BtSwitch
            | Self::Ssh
            | Self::Samba
            | Self::Http
            | Self::Adhoc
            | Self::NetplayEnabled
            | Self::OnlyRelay
            | Self::MissingGames
            | Self::ScraperMissing
            | Self::Filter
            | Self::Autosave
            | Self::StateSlot => &["0", "1"],
            // setsettings.sh CORE_RATIOS names; "core" lets the core decide.
            Self::Ratio => &["core", "4/3", "3/2", "16/9", "8/7", "squarepixel", "full"],
            Self::ClockFormat => &["24", "12"],
            Self::Player => &["1", "2", "3", "4"],
            Self::Channel => &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"],
            Self::UsbMode => &["disabled", "network", "file_transfer"],
            Self::Relay => &["none", "nyc", "madrid", "montreal", "saopaulo", "custom"],
            Self::ScraperSource => &["screenscraper"],
            Self::ScraperMedia => &["box", "box,screenshot", "box,screenshot,wheel"],
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
    /// Non-selectable heading drawn above this row.
    pub section: Option<&'static str>,
}
impl Row {
    const fn under(self, section: &'static str) -> Self {
        Self {
            section: Some(section),
            ..self
        }
    }
    const fn kind(self, kind: Kind) -> Self {
        Self { kind, ..self }
    }
}
const fn row(id: Id, label: &'static str, kind: Kind) -> Row {
    Row {
        id,
        label,
        subtitle: None,
        kind,
        section: None,
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
    )
    .under("settings.keyboard_section"),
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
#[derive(Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Close,
    Set(Id, String),
    Reboot,
    Scan,
    Connect(String, String),
    Forget(String),
    Bluetooth(Action, String),
    Login(String, String),
    Scrape,
    CancelScrape,
}
/// Passwords never reach a log through `{:?}`.
impl std::fmt::Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Close => write!(f, "Close"),
            Self::Set(id, value) => write!(f, "Set({id:?}, {value:?})"),
            Self::Reboot => write!(f, "Reboot"),
            Self::Scan => write!(f, "Scan"),
            Self::Connect(ssid, _) => write!(f, "Connect({ssid:?}, <hidden>)"),
            Self::Forget(ssid) => write!(f, "Forget({ssid:?})"),
            Self::Bluetooth(action, addr) => write!(f, "Bluetooth({action:?}, {addr:?})"),
            Self::Login(user, _) => write!(f, "Login({user:?}, <hidden>)"),
            Self::Scrape => write!(f, "Scrape"),
            Self::CancelScrape => write!(f, "CancelScrape"),
        }
    }
}
/// ScreenScraper state mirrored from the host's engine job.
#[derive(Clone, Debug, Default)]
pub struct Scrape {
    /// This build carries the developer credential.
    pub available: bool,
    pub user: String,
    /// Allowed threads after a verified login, 0 when unknown.
    pub threads: usize,
    pub verifying: bool,
    /// User name typed for a login that is not verified yet.
    pub draft: String,
    /// A start was requested and the host has not answered.
    pub starting: bool,
    pub running: bool,
    pub system: String,
    pub done: usize,
    pub total: usize,
    pub scraped: usize,
    pub missing: usize,
    /// Locale key describing how the last run ended.
    pub outcome: Option<&'static str>,
}
#[derive(Clone)]
pub struct Settings {
    stack: Vec<(Page, usize)>,
    pub cursor: usize,
    pub values: BTreeMap<Id, String>,
    pub options: BTreeMap<Id, Vec<String>>,
    /// Display names for option codes (keyboard layouts, systems).
    pub labels: BTreeMap<(Id, String), String>,
    pub pending: Option<Id>,
    pub loading: bool,
    pub error: Option<(Option<Id>, String)>,
    /// Bumped by every `fail`, so a repeated identical message restarts its timer.
    pub error_seq: u64,
    pub reboot_prompt: bool,
    pub info: Information,
    pub networks: Vec<Network>,
    pub bt: Vec<Device>,
    pub bt_scanned: Option<Instant>,
    pub bt_scanning: bool,
    pub scanned: Option<Instant>,
    /// A radio scan is running; the list shown meanwhile is the previous one.
    pub scanning: bool,
    /// SSID being joined; input waits for the host's reply.
    pub connecting: Option<String>,
    pub confirm_forget: Option<String>,
    pub keyboard: Option<Keyboard>,
    pub keyboard_target: Option<Id>,
    pub wifi_ssid: String,
    pub wifi_hidden: bool,
    pub access_ip: String,
    /// The single USB port hosts a device, so gadget mode is refused.
    pub usb_busy: bool,
    pub scrape: Scrape,
    pub games: Games,
    /// Sliders in flight: last confirmed value and the newest unsent one.
    pub slides: BTreeMap<Id, (String, Option<String>)>,
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
            labels: BTreeMap::new(),
            pending: None,
            loading: true,
            error: None,
            error_seq: 0,
            reboot_prompt: false,
            info: Information::default(),
            networks: Vec::new(),
            bt: Vec::new(),
            bt_scanned: None,
            bt_scanning: false,
            scanned: None,
            scanning: false,
            connecting: None,
            confirm_forget: None,
            keyboard: None,
            keyboard_target: None,
            wifi_ssid: String::new(),
            wifi_hidden: false,
            access_ip: String::new(),
            usb_busy: false,
            scrape: Scrape::default(),
            games: Games::default(),
            slides: BTreeMap::new(),
        }
    }
}
impl Settings {
    pub fn display_value(&self, id: Id, catalog: &crate::i18n::I18n) -> String {
        let value = self.value(id);
        if let Some(label) = self.labels.get(&(id, value.to_owned())) {
            return label.clone();
        }
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
            (Id::IndexNow, _) => "settings.soon",
            (Id::ScraperAccount, _) => "settings.sign_in",
            (Id::ScraperStart, _) if self.scrape.running => "settings.cancel_scraping",
            (Id::ScraperStart, _) => return String::new(),
            (Id::NetplayLink | Id::ScraperLink, _) => return "›".into(),
            (Id::GameSystem, _) => return format!("{} ›", self.games.fullname()),
            (Id::Ratio, "core") => "settings.ratio_auto",
            (Id::Ratio, "squarepixel") => "settings.ratio_square",
            (Id::Ratio, "full") => "settings.ratio_full",
            (Id::Ratio, v) => return v.replace('/', ":"),
            (Id::Filter, "0") => "settings.filter_none",
            (Id::Filter, _) => "settings.filter_smooth",
            (Id::StateSlot, "1") => "settings.slot_auto",
            (Id::StateSlot, _) => "settings.slot_fixed",
            (Id::KeyboardVariant, "") => "menu.default",
            (Id::ScraperSystems, "") => "settings.all",
            (Id::ClockFormat, "24") => "settings.clock_24",
            (Id::ClockFormat, "12") => "settings.clock_12",
            (Id::UsbMode, "disabled") => "settings.off",
            (Id::UsbMode, "network") => "settings.usb_network",
            (Id::UsbMode, "file_transfer") => "settings.usb_files",
            (Id::Relay, "none") => "settings.off",
            (Id::NetplayEnabled | Id::Adhoc, "0") => "settings.off",
            (Id::NetplayEnabled | Id::Adhoc, "1") => "settings.on",
            (Id::Relay, "nyc") => "settings.relay_nyc",
            (Id::Relay, "madrid") => "settings.relay_madrid",
            (Id::Relay, "montreal") => "settings.relay_montreal",
            (Id::Relay, "saopaulo") => "settings.relay_saopaulo",
            (Id::Relay, "custom") => "settings.relay_custom",
            (Id::ScraperSource, _) => return "ScreenScraper".into(),
            (Id::ScraperMedia, "box") => "settings.media_box",
            (Id::ScraperMedia, "box,screenshot") => "settings.media_screen",
            (Id::ScraperMedia, _) => "settings.media_all",
            (Id::Nickname | Id::NetplayHost | Id::CustomServer, "") => "settings.define",
            (_, "") => "settings.unavailable",
            _ => return value.into(),
        };
        catalog.text(key)
    }
    /// Subtitle text, or a `key|name=value` catalog reference.
    pub fn subtitle(&self, row: &Row) -> Option<String> {
        let ip = &self.access_ip;
        Some(match row.id {
            Id::Ssh if !ip.is_empty() => format!("settings.ssh_at|ip={ip}"),
            Id::Http if !ip.is_empty() => format!("settings.http_at|ip={ip}"),
            Id::UsbMode if self.usb_busy => "settings.usb_in_use".into(),
            Id::ScraperAccount if self.scrape.verifying => "settings.verifying".into(),
            Id::ScraperAccount if self.scrape.user.is_empty() => "settings.not_signed_in".into(),
            Id::ScraperAccount if self.scrape.threads > 0 => format!(
                "settings.account_threads|user={}|count={}",
                self.scrape.user, self.scrape.threads
            ),
            Id::ScraperAccount => self.scrape.user.clone(),
            _ => row.subtitle?.into(),
        })
    }
    pub fn visible_rows(&self) -> Vec<&'static Row> {
        self.page()
            .rows()
            .iter()
            .filter(|row| match row.id {
                Id::NetplayHost
                | Id::NetplayPort
                | Id::Relay
                | Id::CustomServer
                | Id::OnlyRelay
                | Id::MissingGames
                    if self.value(Id::Adhoc) == "1" =>
                {
                    false
                }
                Id::CustomServer => self.value(Id::Relay) == "custom",
                id if Games::ROWS.contains(&id) => self.games.shows(id, self),
                _ => true,
            })
            .collect()
    }
    /// Rows with at most one choice show their value and accept no input
    /// (one audio sink: the jack switches speaker and headphones itself).
    pub fn fixed(&self, id: Id) -> bool {
        matches!(id, Id::Audio | Id::Emulator | Id::Core)
            && self
                .options
                .get(&id)
                .is_none_or(|choices| choices.len() <= 1)
    }
    /// Disabled rows are drawn muted and accept no input.
    pub fn disabled(&self, id: Id) -> bool {
        match id {
            Id::IndexNow => true,
            Id::ScraperSource
            | Id::ScraperAccount
            | Id::ScraperSystems
            | Id::ScraperMedia
            | Id::ScraperMissing
            | Id::ScraperStart => {
                !self.scrape.available
                    || self.scrape.verifying
                    || self.scrape.starting
                    || (self.scrape.running && id != Id::ScraperStart)
            }
            _ => false,
        }
    }
    pub fn hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::{Key, Pair};
        const BACK: &[(HintButton, &str)] = &[(Key('B'), "hint.back")];
        if self.connecting.is_some() {
            return &[];
        }
        if self.confirm_forget.is_some() {
            return &[(Key('A'), "settings.forget"), (Key('B'), "hint.cancel")];
        }
        match self.page() {
            Page::Root => &[
                (Key('A'), "hint.open"),
                (Key('B'), "hint.back"),
                (Key('Y'), "settings.info_short"),
            ],
            Page::Info => &[(Key('A'), "settings.details"), (Key('B'), "hint.back")],
            Page::InfoDetail(_) => &[
                (Pair('L', 'R'), "settings.previous_next"),
                (Key('B'), "hint.back"),
            ],
            Page::Keyboard
                if self
                    .keyboard
                    .as_ref()
                    .is_some_and(|k| k.mode == crate::keyboard::Mode::Secret) =>
            {
                &[
                    (Key('A'), "settings.type"),
                    (Key('Y'), "settings.delete"),
                    (Key('X'), "settings.show"),
                    (HintButton::Start, "settings.connect"),
                    (Key('B'), "hint.back"),
                ]
            }
            Page::Keyboard => &[
                (Key('A'), "settings.type"),
                (Key('Y'), "settings.delete"),
                (HintButton::Start, "hint.confirm"),
                (Key('B'), "hint.back"),
            ],
            Page::Bluetooth => match self.bt_device() {
                Some(device) => match (device.paired, device.connected) {
                    (true, true) => &[
                        (Key('A'), "settings.disconnect"),
                        (Key('X'), "settings.forget"),
                        (Key('Y'), "settings.refresh"),
                        (Key('B'), "hint.back"),
                    ],
                    (true, false) => &[
                        (Key('A'), "settings.connect"),
                        (Key('X'), "settings.forget"),
                        (Key('Y'), "settings.refresh"),
                        (Key('B'), "hint.back"),
                    ],
                    _ => &[
                        (Key('A'), "settings.bt_pair"),
                        (Key('Y'), "settings.refresh"),
                        (Key('B'), "hint.back"),
                    ],
                },
                None if self.value(Id::BtSwitch) == "1" => &[
                    (Pair('L', 'R'), "settings.change"),
                    (Key('Y'), "settings.refresh"),
                    (Key('B'), "hint.back"),
                ],
                None => &[(Pair('L', 'R'), "settings.change"), (Key('B'), "hint.back")],
            },
            Page::Wifi => match self.wifi_network() {
                Some(network) if network.known || network.connected => &[
                    (Key('A'), "settings.connect"),
                    (Key('X'), "settings.forget"),
                    (Key('Y'), "settings.refresh"),
                    (Key('B'), "hint.back"),
                ],
                Some(_) => &[
                    (Key('A'), "settings.connect"),
                    (Key('Y'), "settings.refresh"),
                    (Key('B'), "hint.back"),
                ],
                None if self.cursor > 0 => &[
                    (Key('A'), "hint.open"),
                    (Key('Y'), "settings.refresh"),
                    (Key('B'), "hint.back"),
                ],
                None => &[
                    (Pair('L', 'R'), "settings.change"),
                    (Key('Y'), "settings.refresh"),
                    (Key('B'), "hint.back"),
                ],
            },
            _ => match self.visible_rows().get(self.cursor) {
                Some(row) if self.disabled(row.id) || self.fixed(row.id) => BACK,
                Some(row) => match row.kind {
                    Kind::Action => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
                    Kind::Toggle => &[
                        (Pair('L', 'R'), "settings.change"),
                        (Key('A'), "hint.choose"),
                        (Key('B'), "hint.back"),
                    ],
                    _ if Self::text_input(row.id) => {
                        &[(Key('A'), "settings.edit"), (Key('B'), "hint.back")]
                    }
                    _ => &[(Pair('L', 'R'), "settings.change"), (Key('B'), "hint.back")],
                },
                None => BACK,
            },
        }
    }
    pub fn page(&self) -> Page {
        self.stack.last().expect("root page").0
    }
    /// The page below the current one, for breadcrumbs.
    pub fn parent_page(&self) -> Page {
        self.stack
            .len()
            .checked_sub(2)
            .map_or(Page::Root, |n| self.stack[n].0)
    }
    pub fn open(&mut self, page: Page) {
        self.stack.last_mut().expect("root page").1 = self.cursor;
        self.stack.push((page, 0));
        self.cursor = 0;
    }
    fn back(&mut self) -> Effect {
        if self.stack.len() == 1 {
            return Effect::Close;
        }
        self.stack.pop();
        self.cursor = self.stack.last().expect("root page").1;
        Effect::None
    }
    pub fn value(&self, id: Id) -> &str {
        self.values
            .get(&id)
            .map_or(id.default_value(), String::as_str)
    }
    pub fn fail(&mut self, id: Option<Id>, error: String) {
        self.error = Some((id, error));
        self.error_seq += 1;
    }
    pub fn complete(&mut self, id: Id, result: Result<String, String>) {
        self.pending = None;
        match result {
            Ok(value) => {
                if Games::ROWS.contains(&id) {
                    self.games.record(id, &value);
                }
                self.values.insert(id, value);
                self.sync_games();
                self.error = None;
                self.reboot_prompt = id == Id::Gpu;
            }
            Err(error) => self.fail(Some(id), error),
        }
    }
    /// A slider reply: returns the newest value still to apply, if any.
    /// Sliders never show "applying"; a failure reverts to the real value.
    pub fn settle(&mut self, id: Id, result: Result<String, String>) -> Option<String> {
        let Some((confirmed, queued)) = self.slides.get_mut(&id) else {
            self.complete(id, result);
            return None;
        };
        match result {
            Ok(value) => {
                *confirmed = value;
                if let Some(next) = queued.take() {
                    return Some(next);
                }
                let (confirmed, _) = self.slides.remove(&id).expect("slide");
                self.values.insert(id, confirmed);
            }
            Err(error) => {
                let (confirmed, _) = self.slides.remove(&id).expect("slide");
                self.values.insert(id, confirmed);
                self.fail(Some(id), error);
            }
        }
        None
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
        // Any navigation input dismisses the last message.
        self.error = None;
        if let Some(effect) = self.information_input(button) {
            return effect;
        }
        if let Some(effect) = self.page_input(button) {
            return effect;
        }
        let page = self.page();
        let rows = self.visible_rows();
        let len = if page == Page::Root {
            CATEGORIES.len()
        } else {
            rows.len()
        };
        match button {
            Button::B => return self.back(),
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
                if self.disabled(row.id) || self.fixed(row.id) {
                    return Effect::None;
                }
                if button == Button::A {
                    if let Some(effect) = self.activate(row.id) {
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
                        let options = match self.options.get(&row.id).filter(|v| !v.is_empty()) {
                            Some(options) => options.clone(),
                            None => row.id.choices().iter().map(|v| (*v).into()).collect(),
                        };
                        if options.is_empty() {
                            return Effect::None;
                        }
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
                if next != old && matches!(row.kind, Kind::Slider { .. }) {
                    // Optimistic: show it now, apply in the background and
                    // coalesce presses made while a value is being applied.
                    let old = old.to_owned();
                    self.values.insert(row.id, next.clone());
                    self.error = None;
                    if let Some((_, queued)) = self.slides.get_mut(&row.id) {
                        *queued = Some(next);
                        return Effect::None;
                    }
                    self.slides.insert(row.id, (old, None));
                    return Effect::Set(row.id, next);
                }
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
    #[test]
    fn netplay_keys_match_the_lobby_and_launch_consumers() {
        assert_eq!(Id::Nickname.key(), "global.netplay.nickname");
        assert_eq!(Id::OnlyRelay.key(), "netplay.relay_only");
        assert_eq!(Id::MissingGames.key(), "netplay.show_missing");
        assert_eq!(Id::MissingGames.default_value(), "1");
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
    fn netplay_hides_server_rows_for_local_play() {
        let mut state = Settings::default();
        state.open(Page::Netplay);
        state.values.insert(Id::Adhoc, "1".into());
        assert!(!state
            .visible_rows()
            .iter()
            .any(|r| matches!(r.id, Id::NetplayHost | Id::NetplayPort)));
    }
    #[test]
    fn disabled_rows_offer_only_back() {
        let mut state = Settings::default();
        state.open(Page::Scraper);
        assert_eq!(state.hints().len(), 1);
        state.scrape.available = true;
        assert_eq!(state.hints().len(), 2);
        state.open(Page::Netplay);
        state.cursor = state.visible_rows().len() - 1;
        assert_eq!(state.visible_rows()[state.cursor].id, Id::IndexNow);
        assert_eq!(state.hints().len(), 1);
    }
    #[test]
    fn value_rows_fall_back_to_their_static_choices() {
        let mut state = Settings::default();
        state.open(Page::Region);
        state.loading = false;
        state.cursor = 2;
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::ClockFormat, "12".into())
        );
    }
    #[test]
    fn sliders_apply_optimistically_coalesce_and_revert_on_failure() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Sound);
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::Volume, "65".into())
        );
        assert_eq!(state.pending, None);
        assert_eq!(state.value(Id::Volume), "65");
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(state.value(Id::Volume), "75");
        assert_eq!(state.settle(Id::Volume, Ok("65".into())), Some("75".into()));
        assert_eq!(state.value(Id::Volume), "75");
        assert_eq!(state.settle(Id::Volume, Ok("75".into())), None);
        assert!(state.slides.is_empty());
        state.handle(Button::R);
        assert_eq!(state.settle(Id::Volume, Err("pactl".into())), None);
        assert_eq!(state.value(Id::Volume), "75");
        assert_eq!(state.error, Some((Some(Id::Volume), "pactl".into())));
    }
}
#[cfg(test)]
mod dismiss_tests {
    use super::*;
    #[test]
    fn any_input_dismisses_the_message() {
        let mut state = Settings {
            loading: false,
            error: Some((None, "settings.failed".into())),
            ..Settings::default()
        };
        state.handle(Button::Down);
        assert!(state.error.is_none());
    }
    #[test]
    fn busy_usb_port_explains_itself_in_the_row_subtitle() {
        let row = pages::SERVICES
            .iter()
            .find(|r| r.id == Id::UsbMode)
            .unwrap();
        let mut state = Settings::default();
        assert_eq!(state.subtitle(row).as_deref(), Some("settings.usb_detail"));
        state.usb_busy = true;
        assert_eq!(state.subtitle(row).as_deref(), Some("settings.usb_in_use"));
    }
}
