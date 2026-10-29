//! Declarative settings pages and pure navigation. Effects belong to the host.
use crate::{
    appearance, appearance::Appearance, icons::Glyph, keyboard::Keyboard, state::Button,
    HintButton, Theme,
};
use std::collections::BTreeMap;
use std::time::Instant;
pub mod bluetooth;
pub mod cheevos;
mod controls;
pub use controls::Capture;
mod custom;
pub(crate) mod games;
pub(crate) mod information;
pub use custom::Custom;
pub mod hotkeys;
pub mod mangohud;
pub use games::{GameCore, GameSystem, Games};
pub mod network;
pub(crate) mod pages;
use bluetooth::{Action, Device};
pub use pages::MEDIA;
pub mod modules;
pub mod raimport;
pub mod storage;
pub mod store;
pub mod updates;
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
    ScraperMedia,
    Appearance,
    AccentColor,
    Keyboard,
    Storage,
    Mangohud,
    MangohudItems,
    Achievements,
    Controls,
    LauncherControls,
    Hotkeys,
    Store,
    StoreTheme,
    Updates,
    UpdatePackages,
    /// System › Modules (Modulos).
    Modules,
    /// The open module's details and permissions.
    ModuleInfo,
    /// A module's own page: module and page index.
    Module(u8, u8),
    ModuleStore,
    ModuleOffer,
    /// System › Import from RetroArch, and its list of what applies.
    RaImport,
    RaImportList,
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
            Self::ScraperMedia => "settings.media",
            Self::Appearance => "settings.appearance",
            Self::AccentColor => "appearance.custom",
            Self::Keyboard => "settings.keyboard",
            Self::Storage => "settings.storage",
            Self::Mangohud => "settings.mangohud_page",
            Self::MangohudItems => "settings.mangohud_items",
            Self::Achievements => "settings.achievements",
            Self::Controls => "settings.controls",
            Self::LauncherControls => "controls.launcher",
            Self::Hotkeys => "controls.hotkeys",
            Self::Store | Self::StoreTheme => "settings.theme_store",
            Self::Updates => "updates.title",
            Self::UpdatePackages => "updates.packages",
            Self::Modules | Self::ModuleInfo | Self::Module(..) => "settings.modules",
            Self::ModuleStore | Self::ModuleOffer => "modules.store",
            Self::RaImport => "raimport.title",
            Self::RaImportList => "raimport.list_title",
            Self::Soon(key) => key,
        }
    }
    /// Breadcrumb parents between "Settings" and the page itself.
    pub fn parent(self) -> &'static [&'static str] {
        match self {
            Self::Wifi | Self::Bluetooth | Self::Services => &["settings.network"],
            Self::Netplay | Self::Scraper | Self::Mangohud => &["settings.games"],
            Self::MangohudItems => &["settings.games", "settings.mangohud_page"],
            Self::AccentColor | Self::Store => &["settings.appearance"],
            Self::StoreTheme => &["settings.appearance", "settings.theme_store"],
            Self::Modules | Self::ModuleStore | Self::RaImport => &["settings.system"],
            Self::RaImportList => &["settings.system", "raimport.title"],
            Self::ModuleInfo | Self::Module(..) => &["settings.system", "settings.modules"],
            Self::ModuleOffer => &["settings.system", "modules.store"],
            Self::LauncherControls | Self::Hotkeys => &["settings.controls"],
            Self::ScraperMedia => &["settings.games", "settings.scraper_short"],
            Self::Updates => &["settings.system"],
            Self::UpdatePackages => &["settings.system", "updates.title"],
            _ => &[],
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
            Self::ScraperMedia => pages::SCRAPER_MEDIA,
            Self::Appearance => pages::APPEARANCE,
            Self::Storage => storage::STORAGE,
            Self::Mangohud => mangohud::MANGOHUD,
            Self::Achievements => cheevos::CHEEVOS,
            Self::Controls => controls::CONTROLS,
            Self::LauncherControls => controls::LAUNCHER,
            Self::Hotkeys => hotkeys::HOTKEYS_PAGE,
            Self::Updates => updates::UPDATES,
            Self::RaImport => raimport::ROWS,
            _ => &[],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Id {
    Volume,
    Brightness,
    Audio,
    Sounds,
    SoundsVolume,
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
    /// One check box of the Media page: an index into `MEDIA`.
    MediaKind(u8),
    ScraperMissing,
    ScraperStart,
    Theme,
    /// Appearance link to the theme store.
    ThemeStore,
    View,
    Accent,
    Background,
    Font,
    TextSize,
    Animations,
    Restore,
    UnifiedCard,
    StorageCard,
    SecondCard,
    Automount,
    MergedStorage,
    Backup,
    RestoreBackup,
    RootPassword,
    FactoryReset,
    MangohudLink,
    /// Per-system override on the Games page.
    MangohudGame,
    Mangohud,
    MangohudShow,
    MangohudLayout,
    MangohudItems,
    MangohudPosition,
    MangohudSize,
    MangohudAlpha,
    MangohudFps,
    Cheevos,
    CheevosAccount,
    /// Keyboard target only: the password typed for a sign-in.
    CheevosPassword,
    /// The user's own web API key (keyboard target too), and its removal.
    CheevosWebKey,
    CheevosWebKeyRemove,
    CheevosHardcore,
    CheevosVerbose,
    CheevosEncore,
    CheevosRichPresence,
    CheevosScreenshot,
    CheevosSound,
    CheevosLogout,
    LauncherLink,
    /// `launcher.buttons`: the control of every launcher action.
    Buttons,
    /// One action of the launcher Controls page: an index into `pad::ACTIONS`.
    Bind(u8),
    ButtonsRestore,
    HotkeysLink,
    /// `key.hotkey.a`, held for exit, MangoHud and screenshot.
    HotkeyModifier,
    /// `key.function.a`, held for volume and brightness; shown, not edited.
    FunctionModifier,
    /// One in-game hotkey: an index into `hotkeys::HOTKEYS`.
    Hotkey(u8),
    /// Whether that hotkey works now.
    HotkeyOn(u8),
    UpdateInstall,
    UpdateCheck,
    /// `updates.branch`, read by archr-update.
    UpdateChannel,
    UpdateBoot,
    UpdateNotify,
    /// System link to the modules list.
    ModulesLink,
    /// One module's switch on the modules list.
    ModuleEntry(u8),
    ModuleStoreLink,
    /// One row of the open module page.
    ModuleRow(u8),
    /// System link to Import from RetroArch, and that page's rows.
    RaImportLink,
    RaConfig,
    RaOverrides,
    RaShaders,
    RaPreview,
    /// Back up before importing: this page only, never saved.
    RaBackup,
    RaRun,
}
impl Id {
    fn metadata(self) -> (&'static str, &'static str) {
        match self {
            Self::Volume => ("audio.volume", "60"),
            Self::Brightness => ("display.brightness", "50"),
            Self::Audio => ("audio.device", ""),
            Self::Sounds => ("launcher.sounds", "1"),
            Self::SoundsVolume => ("launcher.sounds_volume", "60"),
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
            // "" leaves the option to RetroArch: no key, nothing imposed.
            Self::Ratio => ("ratio", ""),
            Self::Filter => ("smooth", ""),
            Self::Autosave => ("autosave", "0"),
            Self::StateSlot => ("incrementalsavestates", ""),
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
            // Comma-separated `MEDIA` tokens; "none" when nothing is checked.
            Self::ScraperMedia => ("launcher.scraper.media", "box,screenshot,wheel"),
            Self::ScraperMissing => ("launcher.scraper.only_missing", "1"),
            Self::Theme => ("launcher.theme", "default"),
            Self::View => ("launcher.view", crate::layout::LISTA),
            Self::Accent => ("launcher.accent", ""),
            Self::Background => ("launcher.background", ""),
            Self::Font => ("launcher.font", ""),
            Self::TextSize => ("launcher.text_size", "normal"),
            Self::Animations => ("launcher.animations", "on"),
            // Read by the automount script at boot.
            Self::Automount => ("system.automount", "1"),
            // automount owns it ("automount unify"), applied at once.
            Self::MergedStorage => ("system.merged.storage", "0"),
            // mangohud_set owns enabled and state, and builds MangoHud.conf
            // from the others. The Games page writes `<system>.<suffix>`.
            Self::MangohudGame => ("archr.mangohud.enabled", ""),
            Self::Mangohud => ("archr.mangohud.enabled", "0"),
            Self::MangohudShow => ("archr.mangohud.state", "0"),
            Self::MangohudLayout => ("archr.mangohud.layout", "compact"),
            Self::MangohudItems => ("archr.mangohud.items", ""),
            Self::MangohudPosition => ("archr.mangohud.position", "top-left"),
            Self::MangohudSize => ("archr.mangohud.font_size", "24"),
            Self::MangohudAlpha => ("archr.mangohud.alpha", "50"),
            Self::MangohudFps => ("archr.mangohud.fps_limit", "0"),
            // setsettings.sh hands these to RetroArch at every launch.
            Self::Cheevos => ("global.retroachievements", "0"),
            Self::CheevosHardcore => ("global.retroachievements.hardcore", "0"),
            Self::CheevosVerbose => ("global.retroachievements.verbose", "0"),
            Self::CheevosEncore => ("global.retroachievements.encore", "0"),
            Self::CheevosRichPresence => ("global.retroachievements.richpresence", "0"),
            Self::CheevosScreenshot => ("global.retroachievements.screenshot", "0"),
            // An unlock sound's name, or "none".
            Self::CheevosSound => ("global.retroachievements.sound", "none"),
            Self::Buttons => ("launcher.buttons", "A,B,X,Y,L1,R1,START"),
            // input_sense reads these at start; the launcher restarts it.
            Self::HotkeyModifier => ("key.hotkey.a", "BTN_TL"),
            Self::FunctionModifier => ("key.function.a", "BTN_SELECT"),
            Self::Hotkey(n) => {
                let hotkey = &hotkeys::HOTKEYS[usize::from(n)];
                (hotkey.key, hotkey.default)
            }
            Self::HotkeyOn(n) => (hotkeys::HOTKEYS[usize::from(n)].enabled, "1"),
            Self::UpdateChannel => ("updates.branch", "stable"),
            // The launcher's own: check in the background after starting, and
            // mark the START menu when something is pending.
            Self::UpdateBoot => ("updates.check_on_boot", "1"),
            Self::UpdateNotify => ("updates.notify_start", "1"),
            Self::Update
            | Self::Restore
            | Self::ThemeStore
            | Self::Hidden
            | Self::GameSystem
            | Self::NetplayLink
            | Self::IndexNow
            | Self::ScraperLink
            | Self::ScraperAccount
            | Self::ScraperPassword
            | Self::ScraperStart
            | Self::MediaKind(_)
            | Self::UnifiedCard
            | Self::StorageCard
            | Self::SecondCard
            | Self::Backup
            | Self::RestoreBackup
            | Self::RootPassword
            | Self::FactoryReset
            | Self::MangohudLink
            | Self::CheevosAccount
            | Self::CheevosPassword
            | Self::CheevosWebKey
            | Self::CheevosWebKeyRemove
            | Self::CheevosLogout
            | Self::UpdateInstall
            | Self::UpdateCheck
            | Self::ModulesLink
            | Self::ModuleEntry(_)
            | Self::ModuleStoreLink
            | Self::ModuleRow(_) => ("", ""),
            Self::LauncherLink | Self::Bind(_) | Self::ButtonsRestore | Self::HotkeysLink => {
                ("", "")
            }
            Self::RaBackup => ("", "1"),
            Self::RaImportLink
            | Self::RaConfig
            | Self::RaOverrides
            | Self::RaShaders
            | Self::RaPreview
            | Self::RaRun => ("", ""),
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
            Self::HotkeyModifier => &hotkeys::MODIFIERS,
            Self::Language => &["en_US", "pt_BR", "es_ES", "ru_RU"],
            Self::Idle | Self::IdleGame => &["0", "1", "2", "5", "10", "15", "30"],
            Self::Governor => &["ondemand", "performance", "schedutil", "powersave"],
            Self::Sounds
            | Self::Turbo
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
            | Self::Autosave
            | Self::Automount
            | Self::MergedStorage
            | Self::Mangohud
            | Self::MangohudShow
            | Self::Cheevos
            | Self::CheevosHardcore
            | Self::CheevosVerbose
            | Self::CheevosEncore
            | Self::CheevosRichPresence
            | Self::CheevosScreenshot
            | Self::UpdateBoot
            | Self::UpdateNotify => &["0", "1"],
            Self::UpdateChannel => &["stable", "next", "dev"],
            Self::MangohudGame => &["", "1", "0"],
            Self::Filter | Self::StateSlot => &["", "0", "1"],
            Self::MangohudLayout => &mangohud::LAYOUTS,
            Self::MangohudPosition => &mangohud::POSITIONS,
            Self::MangohudSize => &["18", "24", "30"],
            Self::MangohudAlpha => &["0", "25", "50", "75", "100"],
            Self::MangohudFps => &["0", "30", "60"],
            // setsettings.sh CORE_RATIOS names; "core" lets the core decide.
            Self::Ratio => &[
                "",
                "core",
                "4/3",
                "3/2",
                "16/9",
                "8/7",
                "squarepixel",
                "full",
            ],
            Self::ClockFormat => &["24", "12"],
            Self::Player => &["1", "2", "3", "4"],
            Self::Channel => &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"],
            Self::UsbMode => &["disabled", "network", "file_transfer"],
            Self::Relay => &["none", "nyc", "madrid", "montreal", "saopaulo", "custom"],
            Self::ScraperSource => &["screenscraper"],
            Self::Accent => &appearance::ACCENTS,
            Self::Background => &appearance::BACKGROUNDS,
            Self::Font => &appearance::FONTS,
            Self::TextSize => &appearance::TEXT_SIZES,
            Self::Animations => &crate::motion::Motion::CODES,
            _ => &[],
        }
    }
    pub fn normalize(self, value: &str) -> String {
        if self == Self::Animations {
            return crate::motion::Motion::parse(value).code().into();
        }
        if !matches!(self, Self::Volume | Self::Brightness | Self::SoundsVolume) {
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
    Slider {
        min: u8,
        max: u8,
        step: u8,
    },
    Action,
    /// A check box toggled with A.
    Check,
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
    Row {
        subtitle: Some("settings.sounds_subtitle"),
        ..row(Id::Sounds, "settings.sounds", Kind::Toggle)
    },
    row(
        Id::SoundsVolume,
        "settings.sounds_volume",
        Kind::Slider {
            min: 0,
            max: 100,
            step: 10,
        },
    ),
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
    row(Id::ModulesLink, "settings.modules", Kind::Action),
    row(Id::RaImportLink, "raimport.title", Kind::Action),
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
    (Page::Controls, "settings.controls_detail", Glyph::Controls),
    (Page::Games, "settings.games_detail", Glyph::Play),
    (Page::Region, "settings.region_detail", Glyph::Globe),
    (
        Page::Appearance,
        "settings.appearance_detail",
        Glyph::Palette,
    ),
    (
        Page::Achievements,
        "settings.achievements_detail",
        Glyph::Trophy,
    ),
    (Page::System, "settings.system_detail", Glyph::Gear),
    (Page::Storage, "settings.storage_detail", Glyph::Storage),
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
    /// RetroAchievements sign-in: user and password.
    CheevosLogin(String, String),
    CheevosLogout,
    /// Check and save the web API key; empty removes it.
    CheevosKey(String),
    Scrape,
    CancelScrape,
    CheckUpdates,
    /// Download and install the pending updates.
    Update,
    /// Stop the download (the install cannot stop).
    CancelUpdate,
    /// Preview another theme: load it off the draw thread.
    Theme(String),
    /// Leaving Appearance with changes: persist the choices and apply them.
    Appearance,
    Storage(storage::Action),
    Store(store::Action),
    Module(crate::modules::Request),
    /// Import what roms/retroarch-import/ holds, after a backup if asked.
    RaImport {
        backup: bool,
    },
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
            Self::CheevosLogin(user, _) => write!(f, "CheevosLogin({user:?}, <hidden>)"),
            Self::CheevosLogout => write!(f, "CheevosLogout"),
            Self::CheevosKey(_) => write!(f, "CheevosKey(<hidden>)"),
            Self::Scrape => write!(f, "Scrape"),
            Self::CancelScrape => write!(f, "CancelScrape"),
            Self::CheckUpdates => write!(f, "CheckUpdates"),
            Self::Update => write!(f, "Update"),
            Self::CancelUpdate => write!(f, "CancelUpdate"),
            Self::Theme(name) => write!(f, "Theme({name:?})"),
            Self::Appearance => write!(f, "Appearance"),
            Self::Storage(action) => write!(f, "Storage({action:?})"),
            Self::Store(action) => write!(f, "Store({action:?})"),
            // A keyboard row's value may be secret.
            Self::Module(crate::modules::Request::Set { module, key, .. }) => {
                write!(f, "Module(Set({module:?}, {key:?}, <hidden>))")
            }
            Self::Module(request) => write!(f, "Module({request:?})"),
            Self::RaImport { backup } => write!(f, "RaImport {{ backup: {backup} }}"),
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
    /// Names the installed themes give themselves, by folder.
    pub theme_names: BTreeMap<String, crate::layout::Names>,
    pub pending: Option<Id>,
    pub loading: bool,
    pub error: Option<(Option<Id>, String)>,
    /// Bumped by every `fail`, so a repeated identical message restarts its timer.
    pub error_seq: u64,
    /// Locale key of the question asking to reboot now.
    pub reboot_prompt: Option<&'static str>,
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
    pub cheevos: cheevos::Cheevos,
    pub games: Games,
    pub storage: storage::Storage,
    pub store: store::Store,
    pub modules: modules::Modules,
    /// Sliders in flight: last confirmed value and the newest unsent one.
    pub slides: BTreeMap<Id, (String, Option<String>)>,
    /// Appearance page: the choices when it opened, the theme being previewed
    /// and the choices the host just saved (taken by the host to apply them).
    pub appearance_saved: Appearance,
    pub base_theme: Theme,
    pub base_dir: Option<std::path::PathBuf>,
    pub applied: Option<Appearance>,
    /// The custom accent colour: remembered while other dots are chosen and
    /// edited on its own page.
    pub custom: Custom,
    /// The GPU driver in use can run the overlay (075-mangohud-supported).
    pub mangohud_supported: bool,
    /// Layout and items when the items page opened, to save or restore.
    pub mangohud_saved: (String, String),
    /// A "press a button" prompt is open.
    pub capture: Option<Capture>,
    /// The remembered custom colour while the editor shows another one.
    custom_kept: Custom,
    pub updates: updates::Updates,
    pub raimport: raimport::RaImport,
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
            theme_names: BTreeMap::new(),
            pending: None,
            loading: true,
            error: None,
            error_seq: 0,
            reboot_prompt: None,
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
            cheevos: cheevos::Cheevos::default(),
            games: Games::default(),
            storage: storage::Storage::default(),
            store: store::Store::default(),
            modules: modules::Modules::default(),
            slides: BTreeMap::new(),
            appearance_saved: Appearance::default(),
            base_theme: Theme::builtin(),
            base_dir: None,
            applied: None,
            custom: Custom::default(),
            mangohud_supported: false,
            mangohud_saved: Default::default(),
            capture: None,
            custom_kept: Custom::default(),
            updates: updates::Updates::default(),
            raimport: raimport::RaImport::default(),
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
            (Id::Update, version) => {
                let count = self.updates.count();
                return match &self.updates.report {
                    Some(_) if count > 0 => information::value(
                        catalog,
                        &format!("updates.row_pending|version={version}|count={count}"),
                    ),
                    Some(_) => catalog.format("updates.row_current", &[("version", version)]),
                    None => version.into(),
                };
            }
            (Id::UpdateInstall | Id::UpdateCheck, _) => return "›".into(),
            (Id::UpdateChannel, "next") => "updates.next",
            (Id::UpdateChannel, "dev") => "updates.dev",
            (Id::UpdateChannel, _) => "updates.stable",
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
            (Id::Theme, folder) => {
                let named = self.theme_names.get(folder).cloned().or_else(|| {
                    (folder == Id::Theme.default_value()).then(|| Theme::builtin().name)
                });
                return named
                    .as_ref()
                    .and_then(|n| n.get(catalog.language()))
                    .unwrap_or(folder)
                    .to_owned();
            }
            (Id::View, view) => {
                let views = self.base_theme.views();
                let (id, names) = views.iter().find(|(id, _)| id == view).unwrap_or(&views[0]);
                return names.get(catalog.language()).unwrap_or(id).to_owned();
            }
            (Id::Background, "") => "appearance.bg_theme",
            (Id::Background, "black") => "appearance.bg_black",
            (Id::Background, "slate") => "appearance.bg_slate",
            (Id::Font, "") if self.value(Id::Theme) == "default" => "appearance.font_inter",
            (Id::Font, "") => "appearance.font_theme",
            (Id::Font, "mono") => return "JetBrains Mono".into(),
            (Id::Font, "system") => "appearance.font_system",
            (Id::TextSize, "small") => "appearance.text_small",
            (Id::TextSize, "normal") => "appearance.text_normal",
            (Id::TextSize, "large") => "appearance.text_large",
            (Id::Animations, v) => match crate::motion::Motion::parse(v) {
                crate::motion::Motion::On => "appearance.anim_on",
                crate::motion::Motion::Reduced => "appearance.anim_reduced",
                crate::motion::Motion::Off => "appearance.anim_off",
            },
            (Id::Restore | Id::Accent | Id::ButtonsRestore, _) => return String::new(),
            (Id::Bind(n), _) => return self.bindings()[usize::from(n)].name().into(),
            (Id::HotkeyModifier, key) => return hotkeys::key_name(key).into(),
            (Id::Hotkey(n), _) if self.value(Id::HotkeyOn(n)) == "0" => "settings.off",
            (Id::Hotkey(n), _) => return self.hotkey_pads(usize::from(n)).join(" + "),
            (Id::IndexNow, _) => "settings.soon",
            (Id::ScraperAccount | Id::CheevosAccount, _) => "settings.sign_in",
            (Id::CheevosSound, "none") => "settings.cheevos_no_sound",
            (Id::CheevosLogout | Id::CheevosWebKey | Id::CheevosWebKeyRemove, _) => {
                return String::new()
            }
            (Id::ScraperStart, _) if self.scrape.running => "settings.cancel_scraping",
            (Id::ScraperStart, _) => return String::new(),
            (Id::GameSystem, _) => return self.games.fullname().to_owned(),
            (Id::RaConfig | Id::RaOverrides | Id::RaShaders, _) => {
                return raimport::joined(catalog, &self.raimport_refs(id))
            }
            (Id::Ratio | Id::Filter | Id::StateSlot, "") => "settings.retroarch_default",
            (Id::Ratio, "core") => "settings.ratio_auto",
            (Id::Ratio, "squarepixel") => "settings.ratio_square",
            (Id::Ratio, "full") => "settings.ratio_full",
            (Id::Ratio, v) => return v.replace('/', ":"),
            (Id::Filter, "0") => "settings.filter_none",
            (Id::Filter, _) => "settings.filter_smooth",
            (Id::StateSlot, "1") => "settings.slot_auto",
            (Id::StateSlot, _) => "settings.slot_fixed",
            (
                Id::NetplayLink
                | Id::ScraperLink
                | Id::Backup
                | Id::RestoreBackup
                | Id::FactoryReset
                | Id::MangohudLink
                | Id::LauncherLink
                | Id::HotkeysLink
                | Id::ThemeStore
                | Id::ModulesLink
                | Id::ModuleStoreLink
                | Id::RaImportLink
                | Id::RaPreview
                | Id::RaRun,
                _,
            ) => return "›".into(),
            (Id::SecondCard, _) => "settings.eject_link",
            (Id::ModuleEntry(_), _) => return String::new(),
            (Id::ModuleRow(_), _) => return self.module_value(id, catalog).unwrap_or_default(),
            (Id::StorageCard | Id::UnifiedCard, _) => return String::new(),
            (Id::RootPassword, _) => "settings.change_link",
            (Id::KeyboardVariant | Id::MangohudGame, "") => "menu.default",
            (Id::MangohudGame, "1") => "settings.on",
            (Id::MangohudGame, _) => "settings.off",
            (
                Id::MangohudLayout
                | Id::MangohudItems
                | Id::MangohudPosition
                | Id::MangohudSize
                | Id::MangohudAlpha
                | Id::MangohudFps,
                _,
            ) => return self.mangohud_value(id, catalog),
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
            (Id::ScraperMedia, _) => "settings.choose_link",
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
            Id::CheevosAccount | Id::CheevosWebKey if self.cheevos.verifying => {
                "settings.verifying".into()
            }
            Id::CheevosAccount if self.cheevos.expired => "settings.cheevos_expired".into(),
            Id::CheevosWebKey if !self.cheevos.web_key => "settings.cheevos_web_key_missing".into(),
            Id::CheevosWebKey if self.cheevos.key_refused => {
                "settings.cheevos_web_key_refused".into()
            }
            Id::CheevosWebKey => "settings.cheevos_web_key_saved".into(),
            Id::ScraperAccount if self.scrape.user.is_empty() => "settings.not_signed_in".into(),
            Id::ScraperAccount if self.scrape.threads > 0 => format!(
                "settings.account_threads|user={}|count={}",
                self.scrape.user, self.scrape.threads
            ),
            Id::ScraperAccount => self.scrape.user.clone(),
            Id::UnifiedCard => self.storage.unified.as_ref()?.detail.clone(),
            Id::StorageCard => self.storage.card.as_ref()?.detail.clone(),
            Id::SecondCard => self.storage.second.as_ref()?.detail.clone(),
            Id::UpdateInstall => self.updates.install_note()?.into(),
            Id::Mangohud if !self.mangohud_supported => "settings.mangohud_unsupported".into(),
            Id::RestoreBackup => self
                .storage
                .backup
                .clone()
                .unwrap_or_else(|| "settings.restore_none".into()),
            // Catalog arguments are split on '|', which a password cannot hold.
            Id::RootPassword => format!(
                "settings.root_shown|password={}",
                self.storage.password.replace('|', "")
            ),
            Id::ModuleRow(_) => self.module_subtitle(row.id)?,
            _ => row.subtitle?.into(),
        })
    }
    /// Row title text, or a catalog reference.
    pub fn label(&self, row: &Row) -> String {
        let card = match row.id {
            Id::UnifiedCard => &self.storage.unified,
            Id::StorageCard => &self.storage.card,
            Id::SecondCard => &self.storage.second,
            Id::ModuleEntry(_) | Id::ModuleRow(_) => {
                return self.module_label(row.id).unwrap_or_default()
            }
            _ => &None,
        };
        card.as_ref()
            .map_or_else(|| row.label.into(), |card| card.name.clone())
    }
    pub fn visible_rows(&self) -> Vec<Row> {
        if matches!(self.page(), Page::Modules | Page::Module(..)) {
            return self.module_rows();
        }
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
                Id::SecondCard => self.storage.second.is_some(),
                Id::UnifiedCard => self.storage.unified.is_some(),
                id if Games::ROWS.contains(&id) => self.games.shows(id, self),
                id @ (Id::CheevosAccount
                | Id::CheevosWebKey
                | Id::CheevosWebKeyRemove
                | Id::CheevosLogout) => self.cheevos_visible(id),
                _ => true,
            })
            .copied()
            .collect()
    }
    /// Rows with at most one choice show their value and accept no input
    /// (one audio sink: the jack switches speaker and headphones itself).
    pub fn fixed(&self, id: Id) -> bool {
        match id {
            Id::Audio | Id::Emulator | Id::Core | Id::CheevosSound => self
                .options
                .get(&id)
                .is_none_or(|choices| choices.len() <= 1),
            Id::StorageCard | Id::UnifiedCard => true,
            Id::ModuleRow(_) => self.module_fixed(id),
            _ => false,
        }
    }
    /// Disabled rows are drawn muted and accept no input.
    pub fn disabled(&self, id: Id) -> bool {
        match id {
            Id::IndexNow => true,
            Id::UpdateInstall => self.updates.count() == 0 || self.updates.checking,
            Id::UpdateCheck => self.updates.checking,
            Id::RestoreBackup => self.storage.backup.is_none(),
            Id::ButtonsRestore => self.default_bindings(),
            Id::MangohudGame
            | Id::Mangohud
            | Id::MangohudShow
            | Id::MangohudLayout
            | Id::MangohudItems
            | Id::MangohudPosition
            | Id::MangohudSize
            | Id::MangohudAlpha
            | Id::MangohudFps => !self.mangohud_supported,
            Id::Cheevos
            | Id::CheevosAccount
            | Id::CheevosHardcore
            | Id::CheevosVerbose
            | Id::CheevosEncore
            | Id::CheevosRichPresence
            | Id::CheevosScreenshot
            | Id::CheevosSound
            | Id::CheevosWebKey
            | Id::CheevosWebKeyRemove
            | Id::CheevosLogout => self.cheevos.verifying,
            // Choices are kept for when the service is reachable; only what
            // talks to ScreenScraper needs the build's developer key.
            Id::ScraperSource
            | Id::ScraperSystems
            | Id::ScraperMedia
            | Id::MediaKind(_)
            | Id::ScraperMissing => {
                self.scrape.verifying || self.scrape.starting || self.scrape.running
            }
            Id::ScraperAccount | Id::ScraperStart => {
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
        if self.connecting.is_some() || self.storage.working.is_some() {
            return &[];
        }
        if let Some(confirm) = &self.storage.confirm {
            return confirm.dialog().2;
        }
        if self.confirm_forget.is_some() {
            return &[(Key('A'), "settings.forget"), (Key('B'), "hint.cancel")];
        }
        if let Some(hints) = self.modules_hints() {
            return hints;
        }
        match self.page() {
            Page::Updates | Page::UpdatePackages => self.updates_hints(),
            Page::RaImport | Page::RaImportList => self.raimport_hints(),
            Page::Root => &[
                (Key('A'), "hint.open"),
                (Key('B'), "hint.back"),
                (Key('Y'), "settings.info_short"),
            ],
            Page::Info => &[(Key('A'), "settings.details"), (Key('B'), "hint.back")],
            Page::MangohudItems => &[(Key('A'), "mangohud.mark"), (Key('B'), "hint.back")],
            Page::Store | Page::StoreTheme => self.store_hints(),
            Page::Controls | Page::LauncherControls | Page::Hotkeys => self.controls_hints(),
            Page::ScraperMedia if !self.disabled(Id::ScraperMedia) => {
                &[(Key('A'), "settings.mark"), (Key('B'), "hint.back")]
            }
            Page::InfoDetail(_) => &[
                (Pair('L', 'R'), "settings.previous_next"),
                (Key('B'), "hint.back"),
            ],
            Page::AccentColor if self.cursor == custom::ROWS - 1 => {
                &[(Key('A'), "settings.edit"), (Key('B'), "hint.cancel")]
            }
            Page::AccentColor => &[
                (Pair('‹', '›'), "hint.adjust"),
                (Pair('X', 'Y'), "hint.fast"),
                (Key('A'), "hint.use"),
                (Key('B'), "hint.cancel"),
            ],
            Page::Appearance
                if self.accent_is_custom()
                    && self.visible_rows().get(self.cursor).map(|r| r.id) == Some(Id::Accent) =>
            {
                &[
                    (Pair('L', 'R'), "settings.change"),
                    (Key('A'), "settings.edit"),
                    (Key('B'), "hint.back"),
                ]
            }
            Page::Keyboard
                if self
                    .keyboard
                    .as_ref()
                    .is_some_and(|k| k.mode == crate::keyboard::Mode::Secret) =>
            {
                // Network and account passwords connect; the root password is
                // confirmed; the RetroAchievements password signs in.
                if self.keyboard_target == Some(Id::CheevosPassword) {
                    &[
                        (Key('A'), "settings.type"),
                        (Key('Y'), "settings.delete"),
                        (Key('X'), "settings.show"),
                        (HintButton::Start, "settings.sign_in_short"),
                        (Key('B'), "hint.back"),
                    ]
                } else if matches!(
                    self.keyboard_target,
                    Some(Id::RootPassword | Id::CheevosWebKey)
                ) {
                    &[
                        (Key('A'), "settings.type"),
                        (Key('Y'), "settings.delete"),
                        (Key('X'), "settings.show"),
                        (HintButton::Start, "hint.confirm"),
                        (Key('B'), "hint.back"),
                    ]
                } else {
                    &[
                        (Key('A'), "settings.type"),
                        (Key('Y'), "settings.delete"),
                        (Key('X'), "settings.show"),
                        (HintButton::Start, "settings.connect"),
                        (Key('B'), "hint.back"),
                    ]
                }
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
        if page == Page::Appearance {
            self.appearance_saved = self.appearance();
            if let Some(rgb) = appearance::parse_hex(self.value(Id::Accent)) {
                self.custom = Custom::from_rgb(rgb);
            }
        }
        if page == Page::MangohudItems {
            self.mangohud_saved = self.mangohud_draft();
        }
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
                self.reboot_prompt = match id {
                    Id::Gpu => Some("settings.reboot_question"),
                    Id::Automount => Some("settings.storage_reboot_question"),
                    _ => None,
                };
                // Another channel offers other packages.
                if id == Id::UpdateChannel {
                    self.updates.report = None;
                    self.updates.failed = None;
                }
            }
            Err(error) => {
                if id == Id::MangohudItems {
                    self.restore_mangohud();
                }
                self.fail(Some(id), error)
            }
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
        if self.reboot_prompt.is_some() {
            return match button {
                Button::A => Effect::Reboot,
                Button::B => {
                    self.reboot_prompt = None;
                    Effect::None
                }
                _ => Effect::None,
            };
        }
        // Presses go to an open prompt (`captured`), never here.
        if self.capturing() {
            return Effect::None;
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
            Button::B if page == Page::Appearance => return self.leave_appearance(),
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
                if let Some(effect) = self.storage_switch(row.id) {
                    return effect;
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
                            None if row.id == Id::Accent => self.accent_choices(),
                            None if row.id == Id::View => {
                                self.base_theme.views().into_iter().map(|v| v.0).collect()
                            }
                            None => row.id.choices().iter().map(|v| (*v).into()).collect(),
                        };
                        if options.is_empty() {
                            return Effect::None;
                        }
                        let index = if row.id == Id::Accent {
                            self.accent_index()
                        } else {
                            options.iter().position(|v| v == old).unwrap_or(0)
                        };
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
                if page == Page::Appearance {
                    // A draft: shown in the preview now, saved on leaving.
                    self.values.insert(row.id, next.clone());
                    return if row.id == Id::Theme {
                        Effect::Theme(next)
                    } else {
                        Effect::None
                    };
                }
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
        state.cursor = 1;
        assert_eq!(state.visible_rows()[1].id, Id::ScraperAccount);
        assert_eq!(state.hints().len(), 1, "no developer key: no sign-in");
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
mod appearance_tests {
    use super::*;
    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state
            .options
            .insert(Id::Theme, vec!["default".into(), "night".into()]);
        state.open(Page::Appearance);
        state
    }
    #[test]
    fn choices_are_a_draft_until_the_page_is_left() {
        let mut state = page();
        state.cursor = 3;
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(state.value(Id::Accent), "teal");
        assert_eq!(state.pending, None);
        assert_eq!(state.handle(Button::B), Effect::Appearance);
        assert_eq!(state.appearance().accent, "teal");
        assert_eq!(state.pending, Some(Id::Theme));
    }
    #[test]
    fn leaving_unchanged_or_restored_saves_nothing() {
        let mut state = page();
        state.cursor = 6;
        state.handle(Button::R);
        state.handle(Button::L);
        assert_eq!(state.handle(Button::B), Effect::None);
        assert_eq!(state.pending, None);
    }
    #[test]
    fn theme_changes_ask_the_host_to_load_the_preview() {
        let mut state = page();
        assert_eq!(state.handle(Button::R), Effect::Theme("night".into()));
        // Restoring keeps the theme and returns the rest to it.
        state.values.insert(Id::Accent, "pink".into());
        state.cursor = 8;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.value(Id::Theme), "night");
        assert_eq!(state.value(Id::Accent), "");
    }
    fn custom_dot() -> Settings {
        let mut state = page();
        state.cursor = 3;
        state.handle(Button::L);
        assert!(state.accent_is_custom());
        state
    }
    #[test]
    fn the_custom_dot_follows_the_presets_and_opens_its_page_on_a() {
        let mut state = custom_dot();
        let remembered = state.value(Id::Accent).to_owned();
        assert_eq!(remembered, appearance::hex(Custom::default().rgb()));
        state.handle(Button::R);
        assert_eq!(
            state.value(Id::Accent),
            "",
            "the theme's own accent comes first"
        );
        // A anywhere on the row edits the colour in effect.
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::AccentColor);
        assert_eq!(state.custom, Custom::from_rgb(0x1E88FF));
        state.handle(Button::B);
        state.handle(Button::L);
        assert_eq!(state.value(Id::Accent), remembered);
        state.handle(Button::A);
        assert_eq!(state.page(), Page::AccentColor);
        assert_eq!(
            state.custom.rgb(),
            appearance::parse_hex(&remembered).unwrap()
        );
    }
    #[test]
    fn custom_sliders_step_small_and_large_and_a_keeps_the_draft() {
        let mut state = custom_dot();
        state.handle(Button::A);
        let start = state.custom;
        state.handle(Button::R);
        assert_eq!(state.custom.hue, (start.hue + 3) % 360);
        state.handle(Button::X);
        assert_eq!(state.custom.hue, (start.hue + 3 + 330) % 360);
        state.handle(Button::Down);
        for _ in 0..20 {
            state.handle(Button::Y);
        }
        assert_eq!(state.custom.sat, 100);
        let wanted = appearance::hex(state.custom.rgb());
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Appearance);
        assert_eq!(state.value(Id::Accent), wanted);
        assert_eq!(state.handle(Button::B), Effect::Appearance);
    }
    #[test]
    fn b_on_the_custom_page_discards_and_hex_input_sets_the_colour() {
        let mut state = custom_dot();
        let before = state.value(Id::Accent).to_owned();
        state.handle(Button::A);
        state.handle(Button::R);
        state.handle(Button::B);
        assert_eq!(state.value(Id::Accent), before);
        state.handle(Button::A);
        state.cursor = 3;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Keyboard);
        let keyboard = state.keyboard.as_mut().unwrap();
        assert_eq!(keyboard.mode, crate::keyboard::Mode::Hex);
        *keyboard = crate::keyboard::Keyboard::new(crate::keyboard::Mode::Hex, "FFE14D".into());
        assert_eq!(state.handle(Button::Start), Effect::None);
        assert_eq!(state.page(), Page::AccentColor);
        assert_eq!(state.custom.rgb(), 0xFFE14D);
        assert_eq!(state.value(Id::Accent), before);
    }
    #[test]
    fn restore_returns_every_choice_to_its_default() {
        let mut state = page();
        for (id, value) in [
            (Id::Accent, "pink"),
            (Id::TextSize, "large"),
            (Id::Animations, "0"),
        ] {
            state.values.insert(id, value.into());
        }
        state.cursor = 8;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(
            state.appearance(),
            Appearance {
                text_size: "normal".into(),
                theme: "default".into(),
                animations: crate::motion::Motion::On,
                view: "lista".into(),
                ..Appearance::default()
            }
        );
    }
    #[test]
    fn the_view_row_offers_the_previewed_theme_views_by_name() {
        let mut state = page();
        state.cursor = 2;
        let catalog = crate::i18n::I18n::load(std::path::Path::new("/none"), Some("pt_BR"), None);
        assert_eq!(state.display_value(Id::View, &catalog), "Lista");
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(state.value(Id::View), "vitrine");
        assert_eq!(state.display_value(Id::View, &catalog), "Vitrine");
        state.handle(Button::R);
        assert_eq!(state.value(Id::View), "lista");
        state.handle(Button::L);
        assert_eq!(state.handle(Button::B), Effect::Appearance);
        let look = state.appearance();
        assert_eq!(look.view, "vitrine");
        let theme = look.apply(&state.base_theme);
        assert_eq!(theme.active_view, "vitrine");
        assert_eq!(
            theme.view(&theme.active_view).games.list.mark,
            crate::layout::Mark::Bar
        );
        // A theme without that view keeps the value and draws the classic one.
        let mut gone = state.clone();
        gone.values.insert(Id::View, "grade".into());
        assert_eq!(gone.display_value(Id::View, &catalog), "Lista");
        let theme = gone.appearance().apply(&gone.base_theme);
        assert_eq!(theme.view(&theme.active_view).id, "lista");
    }
    #[test]
    fn the_first_dot_is_the_theme_accent_and_presets_like_it_are_hidden() {
        let mut state = page();
        let grafite = state.accent_choices();
        assert_eq!(grafite.first().map(String::as_str), Some(""));
        assert!(
            !grafite.iter().any(|c| c == "blue"),
            "the theme's blue is the first dot"
        );
        assert_eq!(grafite.len(), 1 + 5 + 1);
        // A theme in yellow and blue shows all six presets after its own.
        state.base_theme.highlight_bg = appearance::parse_hex("#FFC93C")
            .map(appearance::rgb)
            .unwrap();
        state.base_theme.highlight_text = appearance::rgb(0x0A2540);
        assert_eq!(state.accent_choices().len(), 1 + 6 + 1);
        // A saved preset equal to the theme's accent rings the theme's dot.
        let mut matching = page();
        matching.values.insert(Id::Accent, "blue".into());
        assert_eq!(matching.accent_index(), 0);
        // The theme's accent keeps the theme's own ink.
        let look = state.appearance().apply(&state.base_theme);
        assert_eq!(look.highlight_text, appearance::rgb(0x0A2540));
        state.values.insert(Id::Accent, "violet".into());
        assert_eq!(
            state.appearance().apply(&state.base_theme).highlight_bg,
            appearance::rgb(0x8B5CF6)
        );
        let catalog = crate::i18n::I18n::load(std::path::Path::new("/none"), Some("pt_BR"), None);
        assert_eq!(state.display_value(Id::Background, &catalog), "Do tema");
        assert_eq!(state.display_value(Id::Theme, &catalog), "Grafite");
        state
            .theme_names
            .insert("night".into(), crate::layout::Names::plain("Noite"));
        state.values.insert(Id::Theme, "night".into());
        assert_eq!(state.display_value(Id::Theme, &catalog), "Noite");
        state.values.insert(Id::Theme, "sol".into());
        assert_eq!(state.display_value(Id::Theme, &catalog), "sol");
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
