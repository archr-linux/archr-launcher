//! The list of a value row with many choices (Lista-media, Lista-longa): a
//! panel over the page up to `PANEL_MAX` choices, a grouped page beyond.
//! Choosing writes exactly what stepping with L/R to that value would.
use super::{Effect, Id, Kind, Row, Settings};
use crate::{state::Button, HintButton};

/// Rows with fewer choices keep only L/R.
pub const MIN: usize = 7;
/// Longer lists take a page of their own, in groups.
pub const PANEL_MAX: usize = 15;
/// Headings of what no group takes: systems, then time zones.
const OTHER: &str = "picker.other";
const OTHER_ZONES: &str = "picker.tz_other";
/// Time zone regions with a heading of their own (`picker.tz_<region>`).
const REGIONS: [&str; 9] = [
    "Africa",
    "America",
    "Antarctica",
    "Asia",
    "Atlantic",
    "Australia",
    "Europe",
    "Indian",
    "Pacific",
];

#[derive(Clone)]
pub struct Picker {
    pub row: Row,
    /// Option codes in list order: by group, then in the row's own order.
    pub choices: Vec<String>,
    /// Group headings (catalog keys or plain text) and the first choice of
    /// each; empty for a list in one piece.
    pub groups: Vec<(String, usize)>,
    pub cursor: usize,
}
impl Picker {
    /// More choices than a panel holds: the list takes the page.
    pub fn full(&self) -> bool {
        self.choices.len() > PANEL_MAX
    }
    /// The group under the cursor.
    pub fn group(&self) -> usize {
        self.groups
            .iter()
            .rposition(|(_, start)| *start <= self.cursor)
            .unwrap_or(0)
    }
    /// First and one-past-last choice of group `n`, or of the whole list.
    pub fn span(&self, n: usize) -> (usize, usize) {
        let start = self.groups.get(n).map_or(0, |g| g.1);
        let end = self.groups.get(n + 1).map_or(self.choices.len(), |g| g.1);
        (start, end)
    }
    pub(super) fn hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::{Key, Pair};
        const CHOOSE: (HintButton, &str) = (Key('A'), "hint.choose");
        const CANCEL: (HintButton, &str) = (Key('B'), "hint.cancel");
        match (self.groups.is_empty(), self.row.id) {
            (true, _) => &[CHOOSE, CANCEL],
            (false, Id::Timezone) => &[(Pair('L', 'R'), "picker.region"), CHOOSE, CANCEL],
            (false, Id::GameSystem) => &[(Pair('L', 'R'), "picker.maker"), CHOOSE, CANCEL],
            (false, _) => &[(Pair('L', 'R'), "picker.letter"), CHOOSE, CANCEL],
        }
    }
}
impl Settings {
    /// The value a row shows: the System row's is the system on the page.
    pub fn chosen(&self, id: Id) -> &str {
        match id {
            Id::GameSystem => self.games.name(),
            _ => self.value(id),
        }
    }
    /// A value row whose choices are many enough for a list on A.
    pub fn listable(&self, id: Id, kind: Kind) -> bool {
        let count = match id {
            Id::GameSystem => self.games.systems.len(),
            _ => self
                .options
                .get(&id)
                .filter(|o| !o.is_empty())
                .map_or_else(|| self.cycle_choices(id).len(), Vec::len),
        };
        kind == Kind::Value
            && count >= MIN
            && id != Id::Accent
            && id != Id::LedsColor
            // Picture: A keeps the value, it opens no list.
            && self.page() != super::Page::Image
            && !Self::is_colour(id)
            && !Self::text_input(id)
            && !self.disabled(id)
            && !self.fixed(id)
    }
    /// Opens the row's list on its value; false when the row has none.
    pub(super) fn open_picker(&mut self, row: Row) -> bool {
        if !self.listable(row.id, row.kind) {
            return false;
        }
        let choices = match row.id {
            Id::GameSystem => self.games.systems.iter().map(|s| s.name.clone()).collect(),
            id => self.cycle_choices(id),
        };
        let current = self.chosen(row.id).to_owned();
        let full = choices.len() > PANEL_MAX;
        let mut keyed: Vec<(String, String)> = choices
            .into_iter()
            .map(|code| {
                let heading = if full {
                    self.heading(row.id, &code)
                } else {
                    String::new()
                };
                (heading, code)
            })
            .collect();
        // A maker with a single system joins the others.
        if row.id == Id::GameSystem {
            let lone: Vec<String> = keyed
                .iter()
                .map(|(h, _)| h)
                .filter(|h| keyed.iter().filter(|(other, _)| other == *h).count() == 1)
                .cloned()
                .collect();
            for (heading, _) in &mut keyed {
                if lone.contains(heading) {
                    *heading = OTHER.into();
                }
            }
        }
        // Stable: inside a group the row's own order stays, except layouts,
        // which read by name.
        keyed.sort_by_cached_key(|(heading, code)| {
            let name = if row.id == Id::KeyboardLayout {
                self.layout_name(code).to_lowercase()
            } else {
                String::new()
            };
            (
                matches!(heading.as_str(), OTHER | OTHER_ZONES),
                heading.clone(),
                name,
            )
        });
        let mut groups: Vec<(String, usize)> = Vec::new();
        for (n, (heading, _)) in keyed.iter().enumerate() {
            if groups.last().is_none_or(|(last, _)| last != heading) {
                groups.push((heading.clone(), n));
            }
        }
        if groups.len() < 2 {
            groups.clear();
        }
        let choices: Vec<String> = keyed.into_iter().map(|(_, code)| code).collect();
        let cursor = choices.iter().position(|c| *c == current).unwrap_or(0);
        self.picker = Some(Picker {
            row,
            choices,
            groups,
            cursor,
        });
        true
    }
    /// Heading of the group `code` belongs to, in a long list.
    fn heading(&self, id: Id, code: &str) -> String {
        match id {
            Id::Timezone => match code.split_once('/') {
                Some((region, _)) if REGIONS.contains(&region) => {
                    format!("picker.tz_{}", region.to_lowercase())
                }
                _ => OTHER_ZONES.into(),
            },
            Id::GameSystem => self
                .games
                .systems
                .iter()
                .find(|s| s.name == code)
                .map(|s| s.manufacturer.trim().to_owned())
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| OTHER.into()),
            Id::KeyboardLayout => self
                .layout_name(code)
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect())
                .unwrap_or_else(|| OTHER.into()),
            _ => String::new(),
        }
    }
    fn layout_name<'a>(&'a self, code: &'a str) -> &'a str {
        self.labels
            .get(&(Id::KeyboardLayout, code.to_owned()))
            .map_or(code, String::as_str)
    }
    /// Up/Down move and wrap, L/R jump between groups, A chooses, B leaves.
    pub(super) fn picker_input(&mut self, button: Button) -> Effect {
        let Some(picker) = self.picker.as_mut() else {
            return Effect::None;
        };
        let len = picker.choices.len();
        match button {
            Button::Up => picker.cursor = (picker.cursor + len - 1) % len,
            Button::Down => picker.cursor = (picker.cursor + 1) % len,
            Button::L | Button::R if !picker.groups.is_empty() => {
                let count = picker.groups.len();
                let step = if button == Button::L { count - 1 } else { 1 };
                picker.cursor = picker.groups[(picker.group() + step) % count].1;
            }
            Button::B => self.picker = None,
            Button::A => {
                let mut picker = self.picker.take().expect("open list");
                let code = picker.choices.swap_remove(picker.cursor);
                if picker.row.id != Id::GameSystem {
                    return self.write(picker.row, code);
                }
                if let Some(n) = self.games.systems.iter().position(|s| s.name == code) {
                    self.show_system(n);
                }
            }
            _ => {}
        }
        Effect::None
    }
    /// The text of `code` in the open list, and its note on the right.
    pub fn picker_entry(
        &self,
        code: &str,
        catalog: &crate::i18n::I18n,
    ) -> (String, Option<String>) {
        let Some(picker) = &self.picker else {
            return (code.into(), None);
        };
        let id = picker.row.id;
        let text = match id {
            // The region is the group's heading; the city reads with spaces.
            Id::Timezone if !picker.groups.is_empty() => code
                .split_once('/')
                .filter(|(region, _)| REGIONS.contains(region))
                .map_or(code, |(_, city)| city)
                .replace('_', " "),
            _ => self.choice_text(id, code, catalog),
        };
        let note = match id {
            Id::Core => self
                .games
                .current()
                .filter(|s| {
                    s.cores.iter().any(|p| {
                        p.default && p.core == code && p.emulator == self.value(Id::Emulator)
                    })
                })
                .map(|_| "menu.default".to_owned()),
            Id::GameSystem => self
                .games
                .systems
                .iter()
                .find(|s| s.name == code)
                .map(|s| format!("games.count|count={}", s.games)),
            _ => self.notes.get(&(id, code.to_owned())).cloned(),
        };
        (text, note.map(|n| super::information::value(catalog, &n)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{GameCore, GameSystem, Page};
    fn ready(page: Page) -> Settings {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state.open(Page::Region);
        if page != Page::Region {
            state.open(page);
        }
        state
    }
    fn focus(state: &mut Settings, id: Id) {
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == id)
            .unwrap();
    }
    const ZONES: [&str; 18] = [
        "Africa/Cairo",
        "America/Argentina/Buenos_Aires",
        "America/New_York",
        "America/Sao_Paulo",
        "Asia/Tokyo",
        "Atlantic/Azores",
        "Australia/Sydney",
        "Europe/Lisbon",
        "Europe/London",
        "Europe/Moscow",
        "Indian/Maldives",
        "Pacific/Auckland",
        "UTC",
        "Etc/GMT+3",
        "Antarctica/Troll",
        "Asia/Kolkata",
        "Europe/Madrid",
        "Asia/Dubai",
    ];
    fn zones() -> Settings {
        let mut state = ready(Page::Region);
        let mut zones: Vec<String> = ZONES.iter().map(|z| (*z).to_owned()).collect();
        zones.sort();
        state.options.insert(Id::Timezone, zones);
        state.values.insert(Id::Timezone, "Europe/Lisbon".into());
        focus(&mut state, Id::Timezone);
        state
    }
    fn snes() -> Settings {
        let mut state = ready(Page::Games);
        let cores = [
            "snes9x",
            "snes9x2010",
            "snes9x2005",
            "snes9x2005_plus",
            "snes9x2002",
            "bsnes",
            "bsnes_hd_beta",
            "mesen-s",
            "beetle_supafaust",
        ];
        let system = |name: &str, maker: &str, cores: Vec<GameCore>| GameSystem {
            name: name.into(),
            fullname: name.to_uppercase(),
            manufacturer: maker.into(),
            games: 3,
            cores,
            ..GameSystem::default()
        };
        let mut systems = vec![system(
            "snes",
            "Nintendo",
            cores
                .iter()
                .map(|core| GameCore {
                    emulator: "retroarch".into(),
                    core: (*core).into(),
                    default: *core == "snes9x",
                    ..GameCore::default()
                })
                .chain([GameCore {
                    emulator: "snes9x-sa".into(),
                    core: "snes9x-sa".into(),
                    ..GameCore::default()
                }])
                .collect(),
        )];
        for (name, maker) in [
            ("nes", "Nintendo"),
            ("gba", "Nintendo"),
            ("n64", "Nintendo"),
            ("gb", "Nintendo"),
            ("gbc", "Nintendo"),
            ("nds", "Nintendo"),
            ("psx", "Sony"),
            ("psp", "Sony"),
            ("megadrive", "Sega"),
            ("mastersystem", "Sega"),
            ("gamegear", "Sega"),
            ("saturn", "Sega"),
            ("dreamcast", "Sega"),
            ("pce", "NEC"),
            ("3do", "Panasonic"),
            ("ports", ""),
        ] {
            let pair = GameCore {
                emulator: "retroarch".into(),
                core: format!("{name}-core"),
                ..GameCore::default()
            };
            systems.push(system(name, maker, vec![pair]));
        }
        state.games.replace(systems);
        state.sync_games();
        state
    }
    #[test]
    fn only_rows_of_seven_choices_or_more_open_a_list() {
        let mut state = ready(Page::Region);
        focus(&mut state, Id::ClockFormat);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.picker.is_none());
        assert!(!state.hints().iter().any(|h| h.1 == "settings.list"));
        // Seven idle times are the shortest list.
        state.open(Page::System);
        state.open(Page::Power);
        assert_eq!(Id::Idle.choices().len(), MIN);
        assert!(state.listable(Id::Idle, Kind::Value));
        assert!(!state.listable(Id::Governor, Kind::Value));
        assert!(!state.listable(Id::Idle, Kind::Toggle));
    }
    #[test]
    fn the_panel_opens_on_the_value_moves_wraps_and_chooses_like_l_r() {
        let mut state = snes();
        focus(&mut state, Id::Core);
        assert!(state.hints().iter().any(|h| h.1 == "settings.list"));
        assert_eq!(state.value(Id::Core), "snes9x");
        let mut stepped = state.clone();
        assert_eq!(state.handle(Button::A), Effect::None);
        let picker = state.picker.as_ref().unwrap();
        assert!(!picker.full() && picker.groups.is_empty());
        assert_eq!(picker.choices.len(), 9);
        assert_eq!(picker.choices[picker.cursor], "snes9x");
        assert_eq!(picker.hints().len(), 2);
        let catalog = crate::i18n::I18n::load(std::path::Path::new("/none"), Some("pt_BR"), None);
        assert_eq!(
            state.picker_entry("snes9x", &catalog),
            ("snes9x".into(), Some("Padrão".into()))
        );
        assert_eq!(state.picker_entry("bsnes", &catalog).1, None);
        // Up from the first wraps to the last, Down comes back.
        state.handle(Button::Up);
        assert_eq!(state.picker.as_ref().unwrap().cursor, 8);
        state.handle(Button::Down);
        state.handle(Button::Down);
        assert_eq!(state.picker.as_ref().unwrap().cursor, 1);
        // The same write, and the same busy row, as R to that value.
        let chosen = state.handle(Button::A);
        assert_eq!(chosen, stepped.handle(Button::R));
        assert_eq!(chosen, Effect::Set(Id::Core, "snes9x2010".into()));
        assert!(state.picker.is_none());
        assert_eq!(state.pending, Some(Id::Core));
        assert_eq!(state.hints(), stepped.hints());
    }
    #[test]
    fn choosing_an_emulator_also_takes_its_first_core() {
        let mut state = snes();
        // Seven emulators make a list.
        let mut emulators: Vec<String> = (1..=5).map(|n| format!("emu{n}")).collect();
        emulators.splice(0..0, ["retroarch".into(), "snes9x-sa".into()]);
        state.options.insert(Id::Emulator, emulators);
        focus(&mut state, Id::Emulator);
        state.handle(Button::A);
        state.handle(Button::Down);
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::Emulator, "snes9x-sa".into())
        );
        state.complete(Id::Emulator, Ok("snes9x-sa".into()));
        assert_eq!(state.value(Id::Core), "snes9x-sa");
    }
    #[test]
    fn b_closes_the_list_and_changes_nothing() {
        let mut state = snes();
        focus(&mut state, Id::Core);
        state.handle(Button::A);
        state.handle(Button::Down);
        assert_eq!(state.handle(Button::B), Effect::None);
        assert!(state.picker.is_none());
        assert_eq!(state.page(), Page::Games);
        assert_eq!(state.value(Id::Core), "snes9x");
        assert_eq!(state.pending, None);
    }
    #[test]
    fn time_zones_take_a_page_grouped_by_region() {
        let mut state = zones();
        assert!(state.hints().iter().any(|h| h.1 == "settings.list"));
        state.handle(Button::A);
        let picker = state.picker.clone().unwrap();
        assert!(picker.full());
        let headings: Vec<&str> = picker.groups.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(
            headings,
            [
                "picker.tz_africa",
                "picker.tz_america",
                "picker.tz_antarctica",
                "picker.tz_asia",
                "picker.tz_atlantic",
                "picker.tz_australia",
                "picker.tz_europe",
                "picker.tz_indian",
                "picker.tz_pacific",
                "picker.tz_other",
            ]
        );
        assert_eq!(picker.choices[picker.cursor], "Europe/Lisbon");
        assert_eq!(
            picker.choices[picker.choices.len() - 2..],
            ["Etc/GMT+3", "UTC"]
        );
        assert_eq!(picker.hints()[0].1, "picker.region");
        let europe = picker.group();
        assert_eq!(picker.span(europe), (10, 14));
        let catalog = crate::i18n::I18n::default();
        state
            .notes
            .insert((Id::Timezone, "America/Sao_Paulo".into()), "UTC-3".into());
        assert_eq!(
            state.picker_entry("America/Sao_Paulo", &catalog),
            ("Sao Paulo".into(), Some("UTC-3".into()))
        );
        assert_eq!(
            state
                .picker_entry("America/Argentina/Buenos_Aires", &catalog)
                .0,
            "Argentina/Buenos Aires"
        );
        assert_eq!(state.picker_entry("UTC", &catalog).0, "UTC");
        // R and L jump a group and wrap at the ends; Down crosses into the next.
        state.handle(Button::R);
        assert_eq!(state.picker.as_ref().unwrap().cursor, 14);
        for _ in 0..3 {
            state.handle(Button::R);
        }
        assert_eq!(state.picker.as_ref().unwrap().cursor, 0, "back to Africa");
        state.handle(Button::L);
        assert_eq!(state.picker.as_ref().unwrap().group(), 9);
        state.handle(Button::Up);
        assert_eq!(
            state.picker.as_ref().unwrap().group(),
            8,
            "into the Pacific"
        );
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::Timezone, "Pacific/Auckland".into())
        );
    }
    #[test]
    fn systems_group_by_maker_and_choosing_switches_locally() {
        let mut state = snes();
        focus(&mut state, Id::GameSystem);
        assert_eq!(state.handle(Button::A), Effect::None);
        let picker = state.picker.clone().unwrap();
        let headings: Vec<&str> = picker.groups.iter().map(|g| g.0.as_str()).collect();
        // NEC, Panasonic and the unknown maker have one system each.
        assert_eq!(headings, ["Nintendo", "Sega", "Sony", "picker.other"]);
        assert_eq!(picker.choices[picker.cursor], "snes");
        assert_eq!(&picker.choices[..2], ["snes", "nes"]);
        assert_eq!(picker.hints()[0].1, "picker.maker");
        let catalog = crate::i18n::I18n::load(std::path::Path::new("/none"), Some("pt_BR"), None);
        assert_eq!(
            state.picker_entry("psx", &catalog),
            ("PSX".into(), Some("3 jogos".into()))
        );
        state.handle(Button::R);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.games.name(), "megadrive");
        assert_eq!(state.visible_rows()[state.cursor].id, Id::GameSystem);
        assert_eq!(state.value(Id::Core), "megadrive-core");
    }
    #[test]
    fn many_layouts_group_by_the_first_letter_of_their_name() {
        let mut state = ready(Page::Region);
        let names = [
            ("us", "English (US)"),
            ("gb", "English (UK)"),
            ("br", "Portuguese (Brazil)"),
            ("pt", "Portuguese"),
            ("es", "Spanish"),
            ("de", "German"),
            ("fr", "French"),
            ("it", "Italian"),
            ("ru", "Russian"),
            ("ua", "Ukrainian"),
            ("pl", "Polish"),
            ("cz", "Czech"),
            ("se", "Swedish"),
            ("fi", "Finnish"),
            ("tr", "Turkish"),
            ("jp", "Japanese"),
        ];
        let mut codes = Vec::new();
        for (code, name) in names {
            state
                .labels
                .insert((Id::KeyboardLayout, code.into()), name.into());
            codes.push(code.to_owned());
        }
        codes.sort();
        state.options.insert(Id::KeyboardLayout, codes);
        focus(&mut state, Id::KeyboardLayout);
        state.handle(Button::A);
        let picker = state.picker.as_ref().unwrap();
        assert_eq!(picker.groups[0], ("C".into(), 0));
        assert_eq!(&picker.choices[1..3], ["gb", "us"], "English (UK), (US)");
        assert_eq!(picker.choices[picker.cursor], "us");
        assert_eq!(picker.hints()[0].1, "picker.letter");
    }
}
