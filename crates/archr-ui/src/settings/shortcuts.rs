//! Appearance › Start shortcuts (Hub-atalhos-editar): A shows or hides,
//! X moves (↑/↓, A or X keeps, B cancels), Y adds a built-in the list
//! lacks, SELECT restores the default row. Every change is saved at once.
use super::{row, Effect, Id, Kind, Page, Row, Settings, CATEGORIES};
use crate::{
    shortcuts::{self, Builtin, Item, Saved, MAX},
    state::Button,
    HintButton,
};

#[derive(Clone, Debug, Default)]
pub struct Shortcuts {
    /// What store.toml holds; the host fills it and saves each change.
    pub saved: Saved,
    /// A change for the host to save (it takes it).
    pub changed: bool,
    /// Move mode: the list being reordered and where the item started.
    moving: Option<(Vec<Item>, usize)>,
    /// The add chooser is open on this built-in.
    pub adding: Option<usize>,
}

impl Settings {
    /// A page a START shortcut opens: the way there is walked as if chosen,
    /// so B goes back along it, as from the Update shortcut.
    pub fn open_page(&mut self, page: Page) -> Effect {
        let category = match page {
            Page::Store => Page::Appearance,
            Page::ModuleStore => Page::System,
            Page::Bluetooth => Page::Wifi,
            page => page,
        };
        self.home();
        self.cursor = CATEGORIES.iter().position(|c| c.0 == category).unwrap_or(0);
        self.open(category);
        let link = |state: &Self, id| {
            let rows = state.visible_rows();
            rows.iter().position(|r| r.id == id).unwrap_or(0)
        };
        match page {
            Page::Store => {
                self.cursor = link(self, Id::ThemeStore);
                self.open_store()
            }
            Page::ModuleStore => {
                self.cursor = link(self, Id::ModulesLink);
                self.open(Page::Modules);
                self.cursor = link(self, Id::ModuleStoreLink);
                self.open(Page::ModuleStore);
                self.open_store_list(self.modules.tab, false)
            }
            Page::Bluetooth => {
                // The Bluetooth line sits above the USB adapters, last.
                self.cursor = self.wifi_rows().saturating_sub(2);
                self.open(Page::Bluetooth);
                Effect::None
            }
            _ => Effect::None,
        }
    }
    /// The list as the page shows it, a move in progress included.
    pub fn shortcut_items(&self) -> Vec<Item> {
        match &self.shortcuts.moving {
            Some((items, _)) => items.clone(),
            None => shortcuts::list(&self.shortcuts.saved, &self.modules.view),
        }
    }
    /// The page's rows: one switch per shortcut.
    pub(super) fn shortcut_rows(&self) -> Vec<Row> {
        (0..self.shortcut_items().len().min(256))
            .map(|n| row(Id::Shortcut(n as u8), "", Kind::Toggle))
            .collect()
    }
    pub fn shortcut_moving(&self) -> bool {
        self.shortcuts.moving.is_some()
    }
    /// The built-ins the chooser offers.
    pub fn shortcut_addable(&self) -> Vec<Builtin> {
        shortcuts::addable(&self.shortcut_items())
    }
    fn save_shortcuts(&mut self, items: &[Item]) {
        self.shortcuts.saved = shortcuts::save(items, &self.modules.view);
        self.shortcuts.changed = true;
    }
    /// The list the host should save, once.
    pub fn take_shortcuts(&mut self) -> Option<Saved> {
        std::mem::take(&mut self.shortcuts.changed).then(|| self.shortcuts.saved.clone())
    }
    pub(super) fn shortcuts_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::Key;
        if self.shortcuts.adding.is_some() {
            &[(Key('A'), "shortcuts.add"), (Key('B'), "hint.cancel")]
        } else if self.shortcut_moving() {
            &[(Key('A'), "hint.confirm"), (Key('B'), "hint.cancel")]
        } else {
            &[
                (Key('A'), "shortcuts.toggle"),
                (Key('X'), "shortcuts.move"),
                (Key('Y'), "shortcuts.add"),
                (Key('B'), "hint.back"),
            ]
        }
    }
    pub(super) fn shortcuts_input(&mut self, button: Button) -> Option<Effect> {
        let mut items = self.shortcut_items();
        let last = items.len().saturating_sub(1);
        if let Some(at) = self.shortcuts.adding {
            let addable = shortcuts::addable(&items);
            match button {
                Button::Up => self.shortcuts.adding = Some(at.saturating_sub(1)),
                Button::Down => {
                    self.shortcuts.adding = Some((at + 1).min(addable.len().saturating_sub(1)))
                }
                Button::A => {
                    self.shortcuts.adding = None;
                    let builtin = addable.get(at)?;
                    // Under the selected line, shown while the row has room.
                    let shown = items.iter().filter(|i| i.shown).count() < MAX;
                    let to = (self.cursor + 1).min(items.len());
                    let id = builtin.id().to_owned();
                    items.insert(to, Item { id, shown });
                    self.cursor = to;
                    self.save_shortcuts(&items);
                    if !shown {
                        self.fail(None, format!("shortcuts.full|count={MAX}"));
                    }
                }
                Button::B | Button::Y => self.shortcuts.adding = None,
                _ => {}
            }
            return Some(Effect::None);
        }
        if self.shortcuts.moving.is_some() {
            match button {
                Button::Up | Button::Down => {
                    let to = if button == Button::Up {
                        self.cursor.saturating_sub(1)
                    } else {
                        (self.cursor + 1).min(last)
                    };
                    if let Some((moving, _)) = &mut self.shortcuts.moving {
                        moving.swap(self.cursor, to);
                    }
                    self.cursor = to;
                }
                Button::A | Button::X => {
                    let (items, _) = self.shortcuts.moving.take().expect("moving");
                    self.save_shortcuts(&items);
                }
                Button::B => {
                    let (_, from) = self.shortcuts.moving.take().expect("moving");
                    self.cursor = from;
                }
                _ => {}
            }
            return Some(Effect::None);
        }
        match button {
            Button::A => {
                let shown = items.iter().filter(|i| i.shown).count();
                let item = items.get_mut(self.cursor)?;
                if !item.shown && shown >= MAX {
                    self.fail(None, format!("shortcuts.full|count={MAX}"));
                    return Some(Effect::None);
                }
                item.shown = !item.shown;
                self.save_shortcuts(&items);
            }
            Button::X if !items.is_empty() => {
                self.shortcuts.moving = Some((items, self.cursor.min(last)));
            }
            Button::Y if shortcuts::addable(&items).is_empty() => {
                self.fail(None, "shortcuts.nothing_to_add".into());
            }
            Button::Y => self.shortcuts.adding = Some(0),
            Button::Select => {
                self.cursor = 0;
                if self.shortcuts.saved != Saved::default() {
                    self.shortcuts.saved = Saved::default();
                    self.shortcuts.changed = true;
                }
                self.note("shortcuts.restored".into());
            }
            Button::L | Button::R | Button::X => {}
            // Up, Down and B as on every page.
            _ => return None,
        }
        Some(Effect::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state.open(Page::Appearance);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::ShortcutsLink)
            .unwrap();
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Shortcuts);
        state
    }
    fn shown(state: &Settings) -> Vec<String> {
        state
            .shortcut_items()
            .into_iter()
            .filter(|i| i.shown)
            .map(|i| i.id)
            .collect()
    }

    #[test]
    fn a_hides_and_shows_and_a_ninth_is_refused() {
        let mut state = page();
        assert_eq!(state.visible_rows().len(), 8);
        state.cursor = 6;
        state.handle(Button::A);
        assert_eq!(state.take_shortcuts().unwrap().hidden, ["reboot"]);
        assert_eq!(state.take_shortcuts(), None, "taken once");
        assert_eq!(shown(&state).len(), 7);
        // Bluetooth comes in under the cursor, shown: the row had room.
        state.handle(Button::Y);
        assert_eq!(state.shortcuts.adding, Some(0));
        state.handle(Button::A);
        assert_eq!(state.shortcut_items()[7].id, "bluetooth");
        assert_eq!(state.cursor, 7);
        assert_eq!(shown(&state).len(), 8);
        // Restart again would be the ninth.
        state.cursor = 6;
        state.handle(Button::A);
        assert_eq!(state.error.as_ref().unwrap().1, "shortcuts.full|count=8");
        assert_eq!(shown(&state).len(), 8);
    }

    #[test]
    fn x_moves_with_up_and_down_and_b_cancels() {
        let mut state = page();
        state.cursor = 3;
        state.handle(Button::X);
        assert!(state.shortcut_moving());
        state.handle(Button::Up);
        state.handle(Button::Up);
        assert_eq!(state.cursor, 1);
        assert_eq!(state.shortcut_items()[1].id, "modules");
        assert_eq!(state.take_shortcuts(), None, "nothing saved while moving");
        state.handle(Button::B);
        assert_eq!(
            (state.cursor, state.shortcut_items()[3].id.as_str()),
            (3, "modules")
        );
        state.handle(Button::X);
        state.handle(Button::Up);
        state.handle(Button::X);
        let saved = state.take_shortcuts().unwrap();
        assert_eq!(
            saved.order[..4],
            ["settings", "update", "modules", "themes"]
        );
        assert_eq!(state.page(), Page::Shortcuts);
    }

    #[test]
    fn start_shortcuts_open_their_pages_and_b_walks_back() {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        assert_eq!(
            state.open_page(Page::Store),
            Effect::Store(crate::settings::store::Action::List(
                Default::default(),
                false
            ))
        );
        assert_eq!(state.page(), Page::Store);
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Appearance);
        let Effect::Module(_) = state.open_page(Page::ModuleStore) else {
            panic!("the module store lists its tab");
        };
        assert_eq!(
            (state.page(), state.parent_page()),
            (Page::ModuleStore, Page::Modules)
        );
        assert_eq!(state.open_page(Page::Bluetooth), Effect::None);
        assert_eq!(
            (state.page(), state.parent_page()),
            (Page::Bluetooth, Page::Wifi)
        );
        state.open_page(Page::Sound);
        assert_eq!(
            (state.page(), state.parent_page()),
            (Page::Sound, Page::Root)
        );
    }

    #[test]
    fn select_restores_the_default_and_b_leaves() {
        let mut state = page();
        state.handle(Button::A);
        state.take_shortcuts();
        state.cursor = 4;
        state.handle(Button::Select);
        assert_eq!(state.take_shortcuts(), Some(Saved::default()));
        assert_eq!(state.cursor, 0);
        assert_eq!(shown(&state).len(), 8);
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Appearance);
    }
}
