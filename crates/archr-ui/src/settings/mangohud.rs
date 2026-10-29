//! MangoHud page (MangoHud, MangoHud-itens): overlay choices for every game
//! and the items grid. Codes are the words `mangohud_set apply` reads.
use super::{described, row, Effect, Id, Kind, Row, Settings};
use crate::state::Button;

pub const LAYOUTS: [&str; 5] = ["fps", "horizontal", "compact", "detailed", "custom"];
pub const POSITIONS: [&str; 6] = [
    "top-left",
    "top-center",
    "top-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];
/// Grid order, with each item's label.
pub const ITEMS: [(&str, &str); 12] = [
    ("fps", "mangohud.item_fps"),
    ("frametime", "mangohud.item_frametime"),
    ("cpu_load", "mangohud.item_cpu_load"),
    ("cpu_temp", "mangohud.item_cpu_temp"),
    ("cpu_freq", "mangohud.item_cpu_freq"),
    ("gpu_load", "mangohud.item_gpu_load"),
    ("gpu_temp", "mangohud.item_gpu_temp"),
    ("gpu_freq", "mangohud.item_gpu_freq"),
    ("ram", "mangohud.item_ram"),
    ("battery", "mangohud.item_battery"),
    ("time", "mangohud.item_time"),
    ("game", "mangohud.item_game"),
];
/// The items of each fixed layout, as mangohud_set maps them.
pub fn layout_items(layout: &str) -> &'static [&'static str] {
    match layout {
        "fps" => &["fps"],
        "horizontal" => &["fps", "cpu_load", "gpu_load", "ram", "battery"],
        "detailed" => &[
            "fps",
            "frametime",
            "cpu_load",
            "cpu_temp",
            "cpu_freq",
            "gpu_load",
            "gpu_temp",
            "gpu_freq",
            "ram",
            "battery",
        ],
        _ => &[
            "fps", "cpu_load", "cpu_temp", "gpu_load", "gpu_temp", "battery",
        ],
    }
}
/// Item groups named in the Items row summary.
const GROUPS: [(&str, &str); 8] = [
    ("fps", "mangohud.sum_fps"),
    ("frametime", "mangohud.sum_graph"),
    ("cpu", "mangohud.sum_cpu"),
    ("gpu", "mangohud.sum_gpu"),
    ("ram", "mangohud.sum_ram"),
    ("battery", "mangohud.sum_battery"),
    ("time", "mangohud.sum_time"),
    ("game", "mangohud.sum_game"),
];
pub const MANGOHUD: &[Row] = &[
    described(
        Id::Mangohud,
        "settings.mangohud",
        "settings.mangohud_panfrost",
    )
    .kind(Kind::Toggle),
    described(
        Id::MangohudShow,
        "settings.mangohud_show",
        "settings.mangohud_show_detail",
    )
    .kind(Kind::Toggle),
    row(Id::MangohudLayout, "settings.mangohud_layout", Kind::Value),
    row(Id::MangohudItems, "settings.mangohud_items", Kind::Action),
    row(
        Id::MangohudPosition,
        "settings.mangohud_position",
        Kind::Value,
    ),
    row(Id::MangohudSize, "settings.mangohud_size", Kind::Value),
    row(Id::MangohudAlpha, "settings.mangohud_alpha", Kind::Value),
    row(Id::MangohudFps, "settings.mangohud_fps", Kind::Value),
];

/// Items the overlay shows: the layout's own, or the custom list.
fn shown_items(layout: &str, saved: &str) -> Vec<&'static str> {
    let custom: Vec<_> = ITEMS
        .iter()
        .map(|(code, _)| *code)
        .filter(|code| saved.split(',').any(|v| v == *code))
        .collect();
    if layout == "custom" && !custom.is_empty() {
        custom
    } else {
        layout_items(layout).to_vec()
    }
}

impl Settings {
    pub fn mangohud_items(&self) -> Vec<&'static str> {
        shown_items(
            self.value(Id::MangohudLayout),
            self.value(Id::MangohudItems),
        )
    }
    pub(super) fn mangohud_draft(&self) -> (String, String) {
        (
            self.value(Id::MangohudLayout).into(),
            self.value(Id::MangohudItems).into(),
        )
    }
    pub(super) fn restore_mangohud(&mut self) {
        let (layout, items) = self.mangohud_saved.clone();
        self.values.insert(Id::MangohudLayout, layout);
        self.values.insert(Id::MangohudItems, items);
    }
    pub(super) fn mangohud_value(&self, id: Id, catalog: &crate::i18n::I18n) -> String {
        let value = self.value(id);
        let key = match (id, value) {
            (Id::MangohudLayout, v) => format!("mangohud.layout_{v}"),
            (Id::MangohudPosition, v) => format!("mangohud.{}", v.replace('-', "_")),
            (Id::MangohudSize, "18") => "mangohud.size_small".into(),
            (Id::MangohudSize, "24") => "mangohud.size_normal".into(),
            (Id::MangohudSize, "30") => "mangohud.size_large".into(),
            (Id::MangohudFps, "0") => "mangohud.no_limit".into(),
            (Id::MangohudFps, v) => return format!("{v} FPS"),
            (Id::MangohudItems, _) => return format!("{} ›", self.mangohud_summary(catalog)),
            (_, v) => return format!("{v}{}", if id == Id::MangohudAlpha { "%" } else { " px" }),
        };
        if catalog.has(&key) {
            catalog.text(&key)
        } else {
            value.into()
        }
    }
    /// "FPS, CPU, GPU, battery": up to four groups, then "+N".
    fn mangohud_summary(&self, catalog: &crate::i18n::I18n) -> String {
        let items = self.mangohud_items();
        let names: Vec<String> = GROUPS
            .iter()
            .filter(|(group, _)| {
                items
                    .iter()
                    .any(|item| item.split('_').next() == Some(group))
            })
            .map(|(_, key)| catalog.text(key))
            .collect();
        let mut text = names[..names.len().min(4)].join(", ");
        if names.len() > 4 {
            text += &format!(" +{}", names.len() - 4);
        }
        text
    }
    /// Items page: a two-column grid; A marks, B saves the draft once.
    pub(super) fn mangohud_items_input(&mut self, button: Button) -> Effect {
        let last = ITEMS.len() - 1;
        match button {
            Button::Up if self.cursor >= 2 => self.cursor -= 2,
            Button::Down if self.cursor + 2 <= last => self.cursor += 2,
            Button::L => self.cursor &= !1,
            Button::R => self.cursor = (self.cursor | 1).min(last),
            Button::A if self.pending.is_none() && !self.loading => self.mark(ITEMS[self.cursor].0),
            Button::B => {
                self.back();
                let (layout, items) = &self.mangohud_saved;
                if self.mangohud_items() == shown_items(layout, items) {
                    self.restore_mangohud();
                } else {
                    self.pending = Some(Id::MangohudItems);
                    return Effect::Set(Id::MangohudItems, self.value(Id::MangohudItems).into());
                }
            }
            _ => {}
        }
        Effect::None
    }
    /// Toggling an item makes the layout custom; the last item stays.
    fn mark(&mut self, code: &'static str) {
        let mut items = self.mangohud_items();
        match items.iter().position(|item| *item == code) {
            Some(_) if items.len() == 1 => return,
            Some(n) => drop(items.remove(n)),
            None => items.push(code),
        }
        let list: Vec<&str> = ITEMS
            .iter()
            .map(|(code, _)| *code)
            .filter(|code| items.contains(code))
            .collect();
        self.values.insert(Id::MangohudItems, list.join(","));
        self.values.insert(Id::MangohudLayout, "custom".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Page;
    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            mangohud_supported: true,
            ..Settings::default()
        };
        state.open(Page::Games);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::MangohudLink)
            .unwrap();
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Mangohud);
        state
    }
    #[test]
    fn defaults_show_the_compact_layout_and_its_summary() {
        let state = page();
        let catalog = crate::i18n::I18n::default();
        assert_eq!(state.mangohud_items(), layout_items("compact"));
        let summary = state.display_value(Id::MangohudItems, &catalog);
        assert_eq!(summary, "FPS, CPU, GPU, battery ›");
        assert_eq!(state.display_value(Id::MangohudFps, &catalog), "No limit");
        assert_eq!(state.display_value(Id::MangohudAlpha, &catalog), "50%");
    }
    #[test]
    fn value_rows_write_their_keys_and_toggles_too() {
        let mut state = page();
        state.cursor = 2;
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::MangohudLayout, "detailed".into())
        );
        state.complete(Id::MangohudLayout, Ok("detailed".into()));
        assert_eq!(state.mangohud_items().len(), 10);
        state.cursor = 1;
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::MangohudShow, "1".into())
        );
    }
    #[test]
    fn marking_items_turns_the_layout_custom_and_saves_once_on_leaving() {
        let mut state = page();
        state.cursor = 3;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::MangohudItems);
        assert_eq!(state.hints().len(), 2);
        // Uncheck FPS, then check the clock (row 6, left column).
        assert_eq!(state.handle(Button::A), Effect::None);
        for _ in 0..5 {
            state.handle(Button::Down);
        }
        state.handle(Button::A);
        assert_eq!(state.value(Id::MangohudLayout), "custom");
        assert_eq!(
            state.handle(Button::B),
            Effect::Set(
                Id::MangohudItems,
                "cpu_load,cpu_temp,gpu_load,gpu_temp,battery,time".into()
            )
        );
        assert_eq!(state.page(), Page::Mangohud);
        assert_eq!(state.pending, Some(Id::MangohudItems));
    }
    #[test]
    fn a_failed_save_restores_and_an_unchanged_grid_saves_nothing() {
        let mut state = page();
        state.cursor = 3;
        state.handle(Button::A);
        state.handle(Button::R);
        state.handle(Button::A);
        state.handle(Button::A);
        assert_eq!(state.handle(Button::B), Effect::None);
        state.cursor = 3;
        state.handle(Button::A);
        state.handle(Button::A);
        state.handle(Button::B);
        state.complete(Id::MangohudItems, Err("mangohud_set".into()));
        assert_eq!(state.value(Id::MangohudLayout), "compact");
        assert_eq!(state.mangohud_items(), layout_items("compact"));
    }
    #[test]
    fn the_last_item_cannot_be_removed() {
        let mut state = page();
        state.values.insert(Id::MangohudLayout, "fps".into());
        state.open(Page::MangohudItems);
        state.handle(Button::A);
        assert_eq!(state.mangohud_items(), ["fps"]);
        assert_eq!(state.handle(Button::B), Effect::None);
    }
    #[test]
    fn without_support_every_row_is_disabled_and_explains_why() {
        let mut state = page();
        state.mangohud_supported = false;
        for row in MANGOHUD {
            assert!(state.disabled(row.id), "{:?}", row.id);
        }
        assert_eq!(
            state.subtitle(&MANGOHUD[0]).as_deref(),
            Some("settings.mangohud_unsupported")
        );
        state.cursor = 3;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Mangohud);
        assert_eq!(state.hints().len(), 1);
    }
}
