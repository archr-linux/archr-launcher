//! Declarative settings pages and pure navigation. Effects belong to the host.
use crate::state::Button;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Root,
    Sound,
    System,
    Region,
    Info,
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
            Self::Soon(key) => key,
        }
    }
    pub fn rows(self) -> &'static [Row] {
        match self {
            Self::Sound => SOUND,
            Self::System => SYSTEM,
            Self::Region => REGION,
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
            Self::Turbo => &["0", "1"],
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
];
pub const CATEGORIES: &[(Page, &str)] = &[
    (Page::Soon("settings.network"), "settings.network_detail"),
    (Page::Sound, "settings.sound_detail"),
    (Page::Soon("settings.controls"), "settings.controls_detail"),
    (Page::Soon("settings.games"), "settings.games_detail"),
    (Page::Region, "settings.region_detail"),
    (
        Page::Soon("settings.appearance"),
        "settings.appearance_detail",
    ),
    (
        Page::Soon("settings.achievements"),
        "settings.achievements_detail",
    ),
    (Page::System, "settings.system_detail"),
    (Page::Soon("settings.storage"), "settings.storage_detail"),
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Close,
    Set(Id, String),
    Reboot,
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
            (_, "") => "settings.unavailable",
            _ => return value.into(),
        };
        catalog.text(key)
    }
    pub fn hints(&self) -> &'static [(char, &'static str)] {
        match self.page() {
            Page::Root => &[
                ('A', "hint.open"),
                ('B', "hint.back"),
                ('Y', "settings.info_short"),
            ],
            page => match page.rows().get(self.cursor).map(|r| r.kind) {
                Some(Kind::Action) => &[('A', "hint.choose"), ('B', "hint.back")],
                Some(Kind::Toggle) => &[
                    ('L', "settings.change"),
                    ('R', "settings.change"),
                    ('A', "hint.choose"),
                    ('B', "hint.back"),
                ],
                Some(_) => &[
                    ('L', "settings.change"),
                    ('R', "settings.change"),
                    ('B', "hint.back"),
                ],
                None => &[('B', "hint.back")],
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
        let len = if page == Page::Root {
            CATEGORIES.len()
        } else {
            page.rows().len()
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
                let Some(row) = page.rows().get(self.cursor) else {
                    return Effect::None;
                };
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
