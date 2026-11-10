//! START menu shortcuts (Hub-atalhos): the built-in ones, the modules' and
//! the row the user arranges in Settings › Appearance › Start shortcuts,
//! kept in store.toml.
use crate::{
    icons::Glyph,
    modules::{Entry, View},
    settings::Page,
    state::{Outcome, SessionAction},
};
use serde::{Deserialize, Serialize};

/// The row shows at most this many.
pub const MAX: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Settings,
    Update,
    Themes,
    Modules,
    Wifi,
    Bluetooth,
    Levels,
    Suspend,
    Reboot,
    Shutdown,
}
/// Every built-in in default order: its saved id, label and icon, and
/// whether the default row has it (the others are added by the user).
const BUILTINS: [(Builtin, &str, &str, Glyph, bool); 10] = [
    (
        Builtin::Settings,
        "settings",
        "start.settings",
        Glyph::Gear,
        true,
    ),
    (
        Builtin::Update,
        "update",
        "start.update",
        Glyph::Download,
        true,
    ),
    (
        Builtin::Themes,
        "themes",
        "settings.theme_store",
        Glyph::Palette,
        true,
    ),
    (
        Builtin::Modules,
        "modules",
        "modules.store",
        Glyph::Puzzle,
        true,
    ),
    (Builtin::Wifi, "wifi", "settings.wifi", Glyph::Wifi, true),
    (
        Builtin::Bluetooth,
        "bluetooth",
        "settings.bluetooth",
        Glyph::Bluetooth,
        false,
    ),
    (
        Builtin::Levels,
        "levels",
        "shortcuts.levels",
        Glyph::Sound,
        false,
    ),
    (
        Builtin::Suspend,
        "suspend",
        "menu.suspend",
        Glyph::Moon,
        true,
    ),
    (
        Builtin::Reboot,
        "reboot",
        "menu.reboot",
        Glyph::Restart,
        true,
    ),
    (
        Builtin::Shutdown,
        "shutdown",
        "menu.shutdown",
        Glyph::Power,
        true,
    ),
];
/// Module shortcuts take the default row's place after this one.
const MODULES_AFTER: Builtin = Builtin::Wifi;

impl Builtin {
    fn entry(self) -> (Builtin, &'static str, &'static str, Glyph, bool) {
        BUILTINS[self as usize]
    }
    pub fn id(self) -> &'static str {
        self.entry().1
    }
    pub fn label(self) -> &'static str {
        self.entry().2
    }
    pub(crate) fn glyph(self) -> Glyph {
        self.entry().3
    }
    fn of(id: &str) -> Option<Self> {
        BUILTINS.iter().find(|b| b.1 == id).map(|b| b.0)
    }
    /// What choosing it on the START menu does.
    pub fn outcome(self) -> Outcome {
        match self {
            Self::Settings => Outcome::Settings,
            Self::Update => Outcome::Updates,
            Self::Themes => Outcome::Open(Page::Store),
            Self::Modules => Outcome::Open(Page::ModuleStore),
            Self::Wifi => Outcome::ToggleWifi,
            Self::Bluetooth => Outcome::Open(Page::Bluetooth),
            Self::Levels => Outcome::Open(Page::Sound),
            Self::Suspend => Outcome::Action(SessionAction::Suspend),
            Self::Reboot => Outcome::Action(SessionAction::Reboot),
            Self::Shutdown => Outcome::Action(SessionAction::Shutdown),
        }
    }
}

/// A shortcut there is now: a built-in, or a module's entry (with the
/// module's index in the view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shortcut<'a> {
    Builtin(Builtin),
    Module(usize, &'a Entry),
}
impl<'a> Shortcut<'a> {
    /// The shortcut behind a saved id, if there is one now.
    pub fn find(id: &str, view: &'a View) -> Option<Self> {
        if let Some(builtin) = Builtin::of(id) {
            return Some(Self::Builtin(builtin));
        }
        let (module, entry) = id.strip_prefix("module:")?.split_once(':')?;
        view.shortcuts()
            .into_iter()
            .find(|(n, e)| view.modules[*n].id == module && e.id == entry)
            .map(|(n, e)| Self::Module(n, e))
    }
}

/// What store.toml keeps: every shortcut of the list in order, and those
/// hidden from the row. No order is the default row, which follows new
/// built-ins.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub order: Vec<String>,
    pub hidden: Vec<String>,
}

/// One line of the list: on the row or hidden from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub shown: bool,
}

/// The default row: the built-ins with the modules' after Wi-Fi, and
/// whether each is a module's.
fn defaults(view: &View) -> Vec<(String, bool)> {
    let modules = view
        .shortcuts()
        .into_iter()
        .map(|(n, e)| (format!("module:{}:{}", view.modules[n].id, e.id), true));
    let mut ids: Vec<(String, bool)> = BUILTINS
        .iter()
        .filter(|b| b.4)
        .map(|b| (b.1.to_owned(), false))
        .collect();
    let after = ids.iter().position(|(id, _)| id == MODULES_AFTER.id());
    let at = after.map_or(ids.len(), |n| n + 1);
    ids.splice(at..at, modules);
    ids
}

/// The user's list: the saved order without the ids that are gone (an old
/// config, a module removed), and the shortcuts it never had (a new
/// built-in, a new module's) at their default place, shown while the row
/// has room, built-ins first. At most `MAX` are shown.
pub fn list(saved: &Saved, view: &View) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for id in &saved.order {
        if Shortcut::find(id, view).is_some() && !items.iter().any(|i| i.id == *id) {
            let shown = !saved.hidden.contains(id);
            items.push(Item {
                id: id.clone(),
                shown,
            });
        }
    }
    let defaults = defaults(view);
    for modules in [false, true] {
        for (n, (id, _)) in defaults.iter().enumerate() {
            if defaults[n].1 != modules || items.iter().any(|i| i.id == *id) {
                continue;
            }
            let at = defaults[..n]
                .iter()
                .rev()
                .find_map(|(before, _)| items.iter().position(|i| i.id == *before))
                .map_or(0, |at| at + 1);
            let shown = items.iter().filter(|i| i.shown).count() < MAX;
            let id = id.clone();
            items.insert(at, Item { id, shown });
        }
    }
    let mut room = MAX;
    for item in items.iter_mut().filter(|i| i.shown) {
        item.shown = room > 0;
        room = room.saturating_sub(1);
    }
    items
}

/// Built-ins the list lacks, which the user may add.
pub fn addable(items: &[Item]) -> Vec<Builtin> {
    BUILTINS
        .iter()
        .map(|b| b.0)
        .filter(|b| !items.iter().any(|i| i.id == b.id()))
        .collect()
}

/// The list as store.toml keeps it: nothing when it is the default row.
pub fn save(items: &[Item], view: &View) -> Saved {
    if items == list(&Saved::default(), view) {
        return Saved::default();
    }
    Saved {
        order: items.iter().map(|i| i.id.clone()).collect(),
        hidden: items
            .iter()
            .filter(|i| !i.shown)
            .map(|i| i.id.clone())
            .collect(),
    }
}

/// The shortcuts the row shows, in order.
pub fn row<'a>(saved: &Saved, view: &'a View) -> Vec<Shortcut<'a>> {
    list(saved, view)
        .iter()
        .filter(|i| i.shown)
        .filter_map(|i| Shortcut::find(&i.id, view))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{Module, Target};

    fn view(shortcuts: &[&str]) -> View {
        View {
            modules: vec![Module {
                id: "bat".into(),
                enabled: true,
                granted: true,
                shortcuts: shortcuts
                    .iter()
                    .map(|id| Entry {
                        id: (*id).into(),
                        label: (*id).into(),
                        icon: None,
                        target: Target::Page(0),
                    })
                    .collect(),
                ..Module::default()
            }],
            ..View::default()
        }
    }
    fn ids(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|i| format!("{}{}", if i.shown { "" } else { "-" }, i.id))
            .collect()
    }
    fn saved(order: &[&str], hidden: &[&str]) -> Saved {
        Saved {
            order: order.iter().map(|s| (*s).into()).collect(),
            hidden: hidden.iter().map(|s| (*s).into()).collect(),
        }
    }
    const ROW: [&str; 8] = [
        "settings", "update", "themes", "modules", "wifi", "suspend", "reboot", "shutdown",
    ];

    #[test]
    fn the_default_row_has_the_eight_built_ins_and_hides_module_shortcuts() {
        for (n, builtin) in BUILTINS.iter().enumerate() {
            assert_eq!(builtin.0 as usize, n, "the table follows the enum");
        }
        let none = View::default();
        assert_eq!(ids(&list(&Saved::default(), &none)), ROW);
        assert_eq!(
            addable(&list(&Saved::default(), &none)),
            [Builtin::Bluetooth, Builtin::Levels]
        );
        // A module's shortcut is listed after Wi-Fi, hidden: the row is full.
        let one = view(&["s"]);
        let items = list(&Saved::default(), &one);
        assert_eq!(items[5].id, "module:bat:s");
        assert!(!items[5].shown);
        assert_eq!(row(&Saved::default(), &one).len(), MAX);
        assert_eq!(
            row(&Saved::default(), &none)[4],
            Shortcut::Builtin(Builtin::Wifi)
        );
    }

    #[test]
    fn unknown_ids_are_dropped_and_new_ones_take_their_default_place() {
        let one = view(&["s"]);
        // An old list: no stores, an id that is gone, Restart hidden.
        let old = saved(
            &[
                "update", "gone", "settings", "wifi", "suspend", "reboot", "shutdown",
            ],
            &["reboot", "gone"],
        );
        assert_eq!(
            ids(&list(&old, &one)),
            [
                "update",
                "themes",
                "modules",
                "settings",
                "wifi",
                "module:bat:s",
                "suspend",
                "-reboot",
                "shutdown"
            ]
        );
    }

    #[test]
    fn at_most_eight_are_shown_and_the_list_saves_round_trip() {
        let one = view(&["s"]);
        let mut items = list(&Saved::default(), &one);
        // Shown in a hand-edited file beyond the limit: the last ones wait.
        let all: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            list(&saved(&all, &[]), &one)
                .iter()
                .filter(|i| i.shown)
                .count(),
            MAX
        );
        // The default list saves as nothing, a changed one as itself.
        assert_eq!(save(&items, &one), Saved::default());
        items.swap(0, 1);
        items[5].shown = true;
        items[7].shown = false;
        let kept = save(&items, &one);
        assert_eq!(kept.hidden, ["reboot"]);
        assert_eq!(list(&kept, &one), items);
        let text = toml::to_string(&kept).unwrap();
        assert_eq!(toml::from_str::<Saved>(&text).unwrap(), kept);
    }

    #[test]
    fn added_built_ins_stay_listed_and_resolve_to_their_page() {
        let none = View::default();
        let mut order = ROW.to_vec();
        order.insert(5, "bluetooth");
        let items = list(&saved(&order, &["reboot"]), &none);
        assert_eq!(addable(&items), [Builtin::Levels]);
        let shown = row(&saved(&order, &["reboot"]), &none);
        assert_eq!(shown[5], Shortcut::Builtin(Builtin::Bluetooth));
        assert_eq!(Builtin::Bluetooth.outcome(), Outcome::Open(Page::Bluetooth));
        assert_eq!(Builtin::Themes.outcome(), Outcome::Open(Page::Store));
    }
}
