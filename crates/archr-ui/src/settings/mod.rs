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
    UiSound,
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
    pub fn key(self) -> &'static str {
        match self {
            Self::Volume => "audio.volume",
            Self::Brightness => "display.brightness",
            Self::Audio => "audio.device",
            Self::UiSound => "launcher.ui_sound",
            Self::Idle => "idle.timeout",
            Self::IdleGame => "idle.timeout.game",
            Self::Gpu => "gpu.driver",
            Self::Suspend => "system.suspendmode",
            Self::Governor => "system.cpugovernor",
            Self::Turbo => "enable.turbo-mode",
            Self::Update => "",
            Self::Language => "system.language",
            Self::Timezone => "system.timezone",
        }
    }
    pub fn default_value(self) -> &'static str {
        match self {
            Self::Volume => "60",
            Self::Brightness => "50",
            Self::UiSound | Self::Turbo | Self::Idle | Self::IdleGame => "0",
            Self::Gpu => "libmali",
            Self::Suspend => "off",
            Self::Governor => "ondemand",
            Self::Language => "en_US",
            Self::Timezone => "America/Sao_Paulo",
            _ => "",
        }
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
    row(Id::UiSound, "settings.ui_sound", Kind::Toggle),
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
pub const CATEGORIES: &[(Page, &str, &str)] = &[
    (
        Page::Soon("settings.network"),
        "settings.network",
        "settings.network_detail",
    ),
    (Page::Sound, "settings.sound", "settings.sound_detail"),
    (
        Page::Soon("settings.controls"),
        "settings.controls",
        "settings.controls_detail",
    ),
    (
        Page::Soon("settings.games"),
        "settings.games",
        "settings.games_detail",
    ),
    (Page::Region, "settings.region", "settings.region_detail"),
    (
        Page::Soon("settings.appearance"),
        "settings.appearance",
        "settings.appearance_detail",
    ),
    (
        Page::Soon("settings.achievements"),
        "settings.achievements",
        "settings.achievements_detail",
    ),
    (Page::System, "settings.system", "settings.system_detail"),
    (
        Page::Soon("settings.storage"),
        "settings.storage",
        "settings.storage_detail",
    ),
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
    pub version: String,
    pub device: String,
    pub kernel: String,
    pub cards: Vec<InfoCard>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            stack: vec![(Page::Root, 0)],
            cursor: 0,
            values: BTreeMap::new(),
            options: BTreeMap::from([
                (Id::Gpu, vec!["libmali".into(), "panfrost".into()]),
                (
                    Id::Language,
                    ["en_US", "pt_BR", "es_ES", "ru_RU"]
                        .map(str::to_owned)
                        .to_vec(),
                ),
                (
                    Id::Idle,
                    ["0", "1", "3", "5", "10", "15", "30"]
                        .map(str::to_owned)
                        .to_vec(),
                ),
                (
                    Id::IdleGame,
                    ["0", "1", "3", "5", "10", "15", "30"]
                        .map(str::to_owned)
                        .to_vec(),
                ),
            ]),
            pending: None,
            loading: true,
            error: None,
            reboot_prompt: false,
            version: String::new(),
            device: String::new(),
            kernel: String::new(),
            cards: Vec::new(),
        }
    }
}
impl Settings {
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
                        let old = old.parse::<i16>().unwrap_or(i16::from(min));
                        (old + if button == Button::L {
                            -i16::from(step)
                        } else {
                            i16::from(step)
                        })
                        .clamp(i16::from(min), i16::from(max))
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
