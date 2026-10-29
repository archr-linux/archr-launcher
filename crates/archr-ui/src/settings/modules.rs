//! System › Modules (Modulos), a module's details and its own declarative
//! pages, and the module store (Loja-modulos, Loja-modulo). Pure state and
//! input; the host runs modules and fetches the store.
use super::{Effect, Id, Kind, Page, Row, Settings};
use crate::{
    keyboard::Mode,
    modules::{Card, Module, Offer, Request, RowKind, Status, StoreAction, StoreReply, Tab, View},
    state::Button,
    HintButton,
};
use std::sync::Arc;

/// A module row's heading: its text comes from the module (`section_text`).
pub const SECTION: &str = "@module-section";

#[derive(Clone, Debug, Default)]
pub struct Modules {
    pub view: Arc<View>,
    /// The module whose details or pages are open.
    pub opened: usize,
    pub confirm_remove: bool,
    /// An action row's question, and the action A sends.
    pub confirm: Option<(String, String)>,
    pub tab: Tab,
    pub cards: [Vec<Card>; 3],
    pub loading: bool,
    /// Locale key of the last store failure.
    pub problem: Option<&'static str>,
    pub offer: Option<Offer>,
    /// The repository whose offer is awaited.
    pub wanted: String,
    /// An install runs.
    pub working: bool,
}
impl Modules {
    pub fn module(&self) -> Option<&Module> {
        self.view.modules.get(self.opened)
    }
    pub fn cards(&self) -> &[Card] {
        &self.cards[self.tab as usize]
    }
    /// The open page's rows, as the module declared them.
    fn page_rows(&self, page: Page) -> &[crate::modules::Row] {
        match page {
            Page::Module(m, p) => self
                .view
                .modules
                .get(usize::from(m))
                .and_then(|module| module.pages.get(usize::from(p)))
                .map_or(&[], |page| &page.rows),
            _ => &[],
        }
    }
}

const fn empty_row(id: Id, kind: Kind) -> Row {
    Row {
        id,
        label: "",
        subtitle: None,
        kind,
        section: None,
    }
}

impl Settings {
    /// The host published modules: switches and open rows follow it.
    pub fn set_modules(&mut self, view: Arc<View>) {
        self.modules.view = view;
        self.sync_modules();
    }
    fn sync_modules(&mut self) {
        let view = self.modules.view.clone();
        for (n, module) in view.modules.iter().enumerate().take(256) {
            let on = if module.active() { "1" } else { "0" };
            self.values.insert(Id::ModuleEntry(n as u8), on.into());
        }
        if let Page::Module(m, _) = self.page() {
            let settings = view.modules.get(usize::from(m)).map(|m| &m.settings);
            for (n, row) in self.modules.page_rows(self.page()).iter().enumerate() {
                if let (Some(key), Some(settings)) = (row.kind.key(), settings) {
                    let value = settings.get(key).unwrap_or(&row.default).clone();
                    self.values.insert(Id::ModuleRow(n as u8), value);
                }
            }
        }
        if matches!(self.page(), Page::Modules) {
            self.cursor = self.cursor.min(view.modules.len());
        }
    }
    /// Rows of the module pages, built from what the modules declared.
    pub(super) fn module_rows(&self) -> Vec<Row> {
        match self.page() {
            Page::Modules => {
                let mut rows: Vec<Row> = (0..self.modules.view.modules.len().min(256))
                    .map(|n| Row {
                        subtitle: Some(""),
                        ..empty_row(Id::ModuleEntry(n as u8), Kind::Toggle)
                    })
                    .collect();
                rows.push(Row {
                    label: "modules.store",
                    ..empty_row(Id::ModuleStoreLink, Kind::Action)
                });
                rows
            }
            page @ Page::Module(..) => self
                .modules
                .page_rows(page)
                .iter()
                .enumerate()
                .map(|(n, row)| Row {
                    subtitle: row.subtitle.as_ref().map(|_| ""),
                    section: row.section.as_ref().map(|_| SECTION),
                    ..empty_row(
                        Id::ModuleRow(n as u8),
                        match row.kind {
                            RowKind::Toggle { .. } => Kind::Toggle,
                            RowKind::Check { .. } => Kind::Check,
                            RowKind::Action { .. } | RowKind::Keyboard { .. } => Kind::Action,
                            RowKind::Value { .. } | RowKind::Text { .. } => Kind::Value,
                        },
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }
    fn module_row(&self, id: Id) -> Option<&crate::modules::Row> {
        match id {
            Id::ModuleRow(n) => self.modules.page_rows(self.page()).get(usize::from(n)),
            _ => None,
        }
    }
    /// Title of a module row or switch.
    pub(super) fn module_label(&self, id: Id) -> Option<String> {
        match id {
            Id::ModuleEntry(n) => self
                .modules
                .view
                .modules
                .get(usize::from(n))
                .map(|m| m.name.clone()),
            Id::ModuleRow(_) => self.module_row(id).map(|r| r.label.clone()),
            _ => None,
        }
    }
    pub(super) fn module_subtitle(&self, id: Id) -> Option<String> {
        self.module_row(id)?.subtitle.clone()
    }
    /// The heading above row `n` of a module page.
    pub fn section_text(&self, n: usize) -> String {
        self.modules
            .page_rows(self.page())
            .get(n)
            .and_then(|r| r.section.clone())
            .unwrap_or_default()
    }
    /// The value text of a module row.
    pub(super) fn module_value(&self, id: Id, catalog: &crate::i18n::I18n) -> Option<String> {
        let row = self.module_row(id)?;
        let module = self.modules.view.modules.get(match self.page() {
            Page::Module(m, _) => usize::from(m),
            _ => return None,
        })?;
        let value = self.value(id);
        Some(match &row.kind {
            RowKind::Value {
                choices, labels, ..
            } => choices
                .iter()
                .position(|c| c == value)
                .and_then(|n| labels.get(n))
                .cloned()
                .unwrap_or_else(|| value.to_owned()),
            RowKind::Text { id } => self
                .modules
                .view
                .value(&module.id, &format!("row:{id}"))
                .unwrap_or(&row.default)
                .to_owned(),
            RowKind::Action { action, .. }
                if self
                    .modules
                    .view
                    .busy
                    .contains(&(module.id.clone(), action.clone())) =>
            {
                "…".into()
            }
            RowKind::Keyboard { secret: true, .. } if !value.is_empty() => "•".repeat(8),
            RowKind::Keyboard { .. } if value.is_empty() => catalog.text("settings.define"),
            RowKind::Keyboard { .. } => value.to_owned(),
            _ => String::new(),
        })
    }
    /// Text rows only show; disabled modules cannot be switched.
    pub(super) fn module_fixed(&self, id: Id) -> bool {
        matches!(
            self.module_row(id).map(|r| &r.kind),
            Some(RowKind::Text { .. })
        )
    }
    fn open_store_list(&mut self, tab: Tab, refresh: bool) -> Effect {
        self.modules.tab = tab;
        self.modules.loading = true;
        self.modules.problem = None;
        Effect::Module(Request::Store(StoreAction::List(tab, refresh)))
    }
    /// From a Start tile: Settings opens on a module's page.
    pub fn open_module_page(&mut self, module: &str, page: u8) {
        let Some(index) = self
            .modules
            .view
            .modules
            .iter()
            .position(|m| m.id == module)
        else {
            return;
        };
        self.modules.opened = index;
        self.open(Page::Modules);
        self.cursor = index;
        self.open(Page::Module(index as u8, page));
        self.sync_modules();
    }
    pub(super) fn modules_input(&mut self, button: Button) -> Option<Effect> {
        if self.modules.working {
            return Some(Effect::None);
        }
        if let Some((action, _)) = self.modules.confirm.take() {
            return Some(match (button, self.modules.module()) {
                (Button::A, Some(module)) => self.invoke(module.id.clone(), action),
                _ => Effect::None,
            });
        }
        if self.modules.confirm_remove {
            self.modules.confirm_remove = false;
            let module = self.modules.module()?.id.clone();
            return Some(match button {
                Button::A | Button::X => {
                    self.back();
                    Effect::Module(Request::Remove {
                        module,
                        data: button == Button::X,
                    })
                }
                _ => Effect::None,
            });
        }
        match self.page() {
            Page::Modules => self.list_input(button),
            Page::ModuleInfo => Some(self.info_input(button)),
            Page::Module(..) => self.page_rows_input(button),
            Page::ModuleStore => Some(self.store_list_input(button)),
            Page::ModuleOffer => Some(self.offer_input(button)),
            _ => None,
        }
    }
    fn invoke(&mut self, module: String, action: String) -> Effect {
        Effect::Module(Request::Invoke {
            module,
            action,
            call: None,
        })
    }
    fn list_input(&mut self, button: Button) -> Option<Effect> {
        let count = self.modules.view.modules.len();
        if self.cursor == count && button == Button::A {
            self.open(Page::ModuleStore);
            return Some(self.open_store_list(self.modules.tab, false));
        }
        let module = self.modules.view.modules.get(self.cursor)?.clone();
        match button {
            Button::Y => {
                self.modules.opened = self.cursor;
                self.open(Page::ModuleInfo);
            }
            Button::A | Button::L | Button::R => {
                if matches!(module.status, Status::Invalid | Status::Incompatible) {
                    self.fail(None, "modules.cannot_run".into());
                } else if !module.granted {
                    // Its permissions come first (local folders, new grants).
                    self.modules.opened = self.cursor;
                    self.open(Page::ModuleInfo);
                } else {
                    let on = !module.active();
                    self.values.insert(
                        Id::ModuleEntry(self.cursor as u8),
                        if on { "1" } else { "0" }.into(),
                    );
                    return Some(Effect::Module(Request::Enable {
                        module: module.id,
                        on,
                        grant: false,
                    }));
                }
            }
            _ => return None,
        }
        Some(Effect::None)
    }
    fn info_input(&mut self, button: Button) -> Effect {
        let Some(module) = self.modules.module().cloned() else {
            return self.back();
        };
        match button {
            Button::A if !module.granted && module.status == Status::Ready => {
                self.back();
                return Effect::Module(Request::Enable {
                    module: module.id,
                    on: true,
                    grant: true,
                });
            }
            Button::A if !module.pages.is_empty() && module.active() => {
                self.open(Page::Module(self.modules.opened as u8, 0));
                self.sync_modules();
            }
            Button::X => self.modules.confirm_remove = true,
            Button::B => return self.back(),
            _ => {}
        }
        Effect::None
    }
    fn page_rows_input(&mut self, button: Button) -> Option<Effect> {
        if !matches!(button, Button::A | Button::L | Button::R) {
            return None;
        }
        let Page::Module(m, _) = self.page() else {
            return None;
        };
        let id = Id::ModuleRow(self.cursor as u8);
        let row = self.module_row(id)?.clone();
        let module = self.modules.view.modules.get(usize::from(m))?.id.clone();
        let old = self.value(id).to_owned();
        let next = match &row.kind {
            RowKind::Toggle { .. } | RowKind::Check { .. }
                if button == Button::A || matches!(row.kind, RowKind::Toggle { .. }) =>
            {
                if old == "1" { "0" } else { "1" }.to_owned()
            }
            RowKind::Value { choices, .. } if button != Button::A && !choices.is_empty() => {
                let at = choices.iter().position(|c| *c == old).unwrap_or(0);
                let step = if button == Button::L {
                    choices.len() - 1
                } else {
                    1
                };
                choices[(at + step) % choices.len()].clone()
            }
            RowKind::Action { action, confirm } if button == Button::A => {
                return Some(match confirm {
                    Some(question) => {
                        self.modules.confirm = Some((action.clone(), question.clone()));
                        Effect::None
                    }
                    None => self.invoke(module, action.clone()),
                });
            }
            RowKind::Keyboard { max, secret, .. } if button == Button::A => {
                let mode = if *secret { Mode::Secret } else { Mode::Text };
                self.open_keyboard(id, mode, if *secret { String::new() } else { old });
                if let Some(keyboard) = &mut self.keyboard {
                    keyboard.max = usize::from(*max);
                    keyboard.optional = true;
                }
                return Some(Effect::None);
            }
            _ => return Some(Effect::None),
        };
        Some(self.module_set(id, next))
    }
    /// A module setting changes: shown at once, saved by the host.
    pub(super) fn module_set(&mut self, id: Id, value: String) -> Effect {
        let (Some(row), Page::Module(m, _)) = (self.module_row(id), self.page()) else {
            return Effect::None;
        };
        let (Some(key), Some(module)) = (
            row.kind.key().map(str::to_owned),
            self.modules.view.modules.get(usize::from(m)),
        ) else {
            return Effect::None;
        };
        let module = module.id.clone();
        self.values.insert(id, value.clone());
        Effect::Module(Request::Set { module, key, value })
    }
    fn store_list_input(&mut self, button: Button) -> Effect {
        let count = self.modules.cards().len();
        let tab = self.modules.tab as usize;
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(count.saturating_sub(1)),
            Button::L | Button::R => {
                let step = if button == Button::L { 2 } else { 1 };
                self.cursor = 0;
                return self.open_store_list(Tab::ALL[(tab + step) % 3], false);
            }
            Button::Y => return self.open_store_list(self.modules.tab, true),
            Button::A => {
                let Some(card) = self.modules.cards().get(self.cursor).cloned() else {
                    return Effect::None;
                };
                self.modules.offer = None;
                self.modules.wanted = card.source();
                self.modules.loading = true;
                self.modules.problem = None;
                self.open(Page::ModuleOffer);
                return Effect::Module(Request::Store(StoreAction::Open(Box::new(card))));
            }
            Button::B => return self.back(),
            _ => {}
        }
        Effect::None
    }
    fn offer_input(&mut self, button: Button) -> Effect {
        match (button, self.modules.offer.clone()) {
            (Button::A, Some(offer)) => {
                if let Some(blocked) = offer.blocked {
                    self.fail(None, blocked.into());
                } else if offer.installed.as_deref() != Some(offer.version.as_str()) {
                    self.modules.working = true;
                    return Effect::Module(Request::Store(StoreAction::Install(Box::new(offer))));
                }
                Effect::None
            }
            (Button::B, _) => self.back(),
            _ => Effect::None,
        }
    }
    /// The main button of the permission page, as a locale key.
    pub fn offer_action(&self) -> Option<&'static str> {
        if self.page() == Page::ModuleInfo {
            let module = self.modules.module()?;
            return if !module.granted && module.status == Status::Ready {
                Some("modules.allow_enable")
            } else if module.active() && !module.pages.is_empty() {
                Some("modules.open")
            } else {
                None
            };
        }
        let offer = self.modules.offer.as_ref()?;
        Some(match &offer.installed {
            _ if offer.blocked.is_some() => return None,
            Some(version) if *version == offer.version => "modules.installed",
            Some(_) if offer.added.is_empty() => "modules.update",
            Some(_) => "modules.allow_update",
            None => "modules.allow_install",
        })
    }
    pub(super) fn modules_hints(&self) -> Option<&'static [(HintButton, &'static str)]> {
        use HintButton::{Key, Pair};
        let modules = &self.modules;
        if modules.working {
            return Some(&[]);
        }
        if modules.confirm.is_some() {
            return Some(&[(Key('A'), "hint.confirm"), (Key('B'), "hint.cancel")]);
        }
        if modules.confirm_remove {
            return Some(&[
                (Key('A'), "modules.remove"),
                (Key('X'), "modules.remove_all"),
                (Key('B'), "hint.cancel"),
            ]);
        }
        Some(match self.page() {
            Page::Modules if self.cursor < modules.view.modules.len() => &[
                (Key('A'), "modules.toggle"),
                (Key('Y'), "modules.details"),
                (Key('B'), "hint.back"),
            ],
            Page::Modules => &[(Key('A'), "hint.open"), (Key('B'), "hint.back")],
            Page::ModuleStore => &[
                (Pair('L', 'R'), "store.tab"),
                (Key('A'), "store.view"),
                (Key('Y'), "store.refresh"),
                (Key('B'), "hint.back"),
            ],
            Page::ModuleInfo => match self.offer_action() {
                Some("modules.allow_enable") => &[
                    (Key('A'), "modules.allow"),
                    (Key('X'), "modules.remove"),
                    (Key('B'), "hint.back"),
                ],
                Some(_) => &[
                    (Key('A'), "hint.open"),
                    (Key('X'), "modules.remove"),
                    (Key('B'), "hint.back"),
                ],
                None => &[(Key('X'), "modules.remove"), (Key('B'), "hint.back")],
            },
            Page::ModuleOffer => match self.offer_action() {
                Some("modules.installed") | None => &[(Key('B'), "hint.back")],
                Some("modules.update") => &[(Key('A'), "store.update"), (Key('B'), "hint.back")],
                Some(_) => &[(Key('A'), "store.install"), (Key('B'), "hint.back")],
            },
            _ => return None,
        })
    }
    /// A store answer; late ones for another tab or module are dropped.
    pub fn module_store_reply(&mut self, reply: StoreReply) {
        let page = self.page();
        let modules = &mut self.modules;
        match reply {
            StoreReply::Listed {
                tab,
                cards,
                problem,
            } => {
                if tab == modules.tab {
                    modules.loading = false;
                    modules.problem = problem;
                }
                modules.cards[tab as usize] = cards;
                if page == Page::ModuleStore {
                    self.cursor = self
                        .cursor
                        .min(self.modules.cards().len().saturating_sub(1));
                }
            }
            StoreReply::Card(card) => {
                for found in modules.cards.iter_mut().flatten() {
                    if found.source() == card.source() {
                        *found = Card {
                            reviewed: found.reviewed,
                            stars: found.stars,
                            ..card.clone()
                        };
                    }
                }
            }
            StoreReply::Offer(source, result) => {
                if page != Page::ModuleOffer || modules.offer.is_some() || source != modules.wanted
                {
                    return;
                }
                modules.loading = false;
                match result {
                    Ok(offer) => modules.offer = Some(*offer),
                    Err(key) => modules.problem = Some(key),
                }
            }
            StoreReply::Done(result) => {
                modules.working = false;
                let key = match result {
                    Ok(key) => {
                        if let Some(offer) = &mut modules.offer {
                            offer.installed = Some(offer.version.clone());
                        }
                        key
                    }
                    Err(key) => key,
                };
                self.fail(None, key.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{Permission, Row as ModuleRow};

    fn view() -> Arc<View> {
        let module = Module {
            id: "bateria".into(),
            name: "Bateria detalhada".into(),
            version: "1.0.0".into(),
            enabled: true,
            granted: true,
            pages: vec![crate::modules::Page {
                id: "main".into(),
                title: "Bateria".into(),
                rows: vec![
                    ModuleRow {
                        label: "Mostrar potência".into(),
                        subtitle: None,
                        section: Some("Opções".into()),
                        kind: RowKind::Toggle {
                            key: "watts".into(),
                        },
                        default: "1".into(),
                    },
                    ModuleRow {
                        label: "Atualizar a cada".into(),
                        subtitle: None,
                        section: None,
                        kind: RowKind::Value {
                            key: "intervalo".into(),
                            choices: vec!["5".into(), "15".into()],
                            labels: vec!["5 s".into(), "15 s".into()],
                        },
                        default: "15".into(),
                    },
                    ModuleRow {
                        label: "Agora".into(),
                        subtitle: None,
                        section: None,
                        kind: RowKind::Text { id: "agora".into() },
                        default: String::new(),
                    },
                    ModuleRow {
                        label: "Atualizar agora".into(),
                        subtitle: None,
                        section: None,
                        kind: RowKind::Action {
                            action: "atualizar".into(),
                            confirm: Some("Certeza?".into()),
                        },
                        default: String::new(),
                    },
                ],
            }],
            permissions: vec![Permission {
                key: "modules.perm_none",
                detail: String::new(),
                sensitive: false,
            }],
            ..Module::default()
        };
        let local = Module {
            id: "copiado".into(),
            name: "Copiado".into(),
            ..Module::default()
        };
        let mut view = View {
            modules: vec![module, local],
            ..View::default()
        };
        view.values
            .insert(("bateria".into(), "row:agora".into()), "87 %".into());
        Arc::new(view)
    }
    fn settings() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state.set_modules(view());
        state.open(Page::System);
        state.open(Page::Modules);
        state
    }

    #[test]
    fn the_list_switches_modules_and_asks_before_a_first_run() {
        let mut state = settings();
        let rows = state.visible_rows();
        assert_eq!(rows.len(), 3, "two modules and the store");
        assert_eq!(state.value(Id::ModuleEntry(0)), "1");
        assert_eq!(
            state.handle(Button::A),
            Effect::Module(Request::Enable {
                module: "bateria".into(),
                on: false,
                grant: false
            })
        );
        state.handle(Button::Down);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::ModuleInfo, "permissions first");
        assert_eq!(state.offer_action(), Some("modules.allow_enable"));
        assert_eq!(
            state.handle(Button::A),
            Effect::Module(Request::Enable {
                module: "copiado".into(),
                on: true,
                grant: true
            })
        );
        state.handle(Button::Down);
        assert_eq!(state.cursor, 2);
        let Effect::Module(Request::Store(StoreAction::List(Tab::Reviewed, false))) =
            state.handle(Button::A)
        else {
            panic!("no store");
        };
        assert_eq!(state.page(), Page::ModuleStore);
    }

    #[test]
    fn module_pages_edit_declared_rows_only() {
        let mut state = settings();
        state.open_module_page("bateria", 0);
        assert_eq!(state.page(), Page::Module(0, 0));
        assert_eq!(state.section_text(0), "Opções");
        assert_eq!(state.value(Id::ModuleRow(1)), "15");
        let catalog = crate::i18n::I18n::default();
        assert_eq!(state.display_value(Id::ModuleRow(1), &catalog), "15 s");
        assert_eq!(state.display_value(Id::ModuleRow(2), &catalog), "87 %");
        assert_eq!(
            state.handle(Button::R),
            Effect::Module(Request::Set {
                module: "bateria".into(),
                key: "watts".into(),
                value: "0".into()
            })
        );
        state.handle(Button::Down);
        assert_eq!(
            state.handle(Button::L),
            Effect::Module(Request::Set {
                module: "bateria".into(),
                key: "intervalo".into(),
                value: "5".into()
            })
        );
        state.handle(Button::Down);
        assert_eq!(state.handle(Button::A), Effect::None, "text rows only show");
        state.handle(Button::Down);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.modules.confirm.is_some(), "asks first");
        assert_eq!(
            state.handle(Button::A),
            Effect::Module(Request::Invoke {
                module: "bateria".into(),
                action: "atualizar".into(),
                call: None
            })
        );
        assert_eq!(state.handle(Button::B), Effect::None);
        assert_eq!(state.page(), Page::Modules);
    }

    #[test]
    fn removing_asks_and_can_take_the_data_too() {
        let mut state = settings();
        state.handle(Button::Y);
        assert_eq!(state.page(), Page::ModuleInfo);
        state.handle(Button::X);
        assert!(state.modules.confirm_remove);
        assert_eq!(
            state.handle(Button::X),
            Effect::Module(Request::Remove {
                module: "bateria".into(),
                data: true
            })
        );
        assert_eq!(state.page(), Page::Modules);
    }

    #[test]
    fn the_store_offers_install_update_and_drops_late_answers() {
        let mut state = settings();
        state.open(Page::ModuleStore);
        state.module_store_reply(StoreReply::Listed {
            tab: Tab::Reviewed,
            cards: vec![Card {
                owner: "kaio".into(),
                repo: "archr-bateria".into(),
                reviewed: true,
                ..Card::default()
            }],
            problem: None,
        });
        let Effect::Module(Request::Store(StoreAction::Open(card))) = state.handle(Button::A)
        else {
            panic!("no open");
        };
        state.module_store_reply(StoreReply::Offer("x/y".into(), Err("store.failed")));
        assert!(state.modules.problem.is_none(), "another module's answer");
        let offer = Offer {
            card: *card,
            version: "1.1.0".into(),
            installed: Some("1.0.0".into()),
            added: vec![Permission {
                key: "modules.perm_net",
                detail: String::new(),
                sensitive: true,
            }],
            ..Offer::default()
        };
        state.module_store_reply(StoreReply::Offer(
            "kaio/archr-bateria".into(),
            Ok(Box::new(offer.clone())),
        ));
        assert_eq!(state.offer_action(), Some("modules.allow_update"));
        assert_eq!(
            state.handle(Button::A),
            Effect::Module(Request::Store(StoreAction::Install(Box::new(offer))))
        );
        assert_eq!(
            state.handle(Button::B),
            Effect::None,
            "busy while installing"
        );
        state.module_store_reply(StoreReply::Done(Ok("modules.installed_ok")));
        assert_eq!(state.offer_action(), Some("modules.installed"));
        assert_eq!(state.error.as_ref().unwrap().1, "modules.installed_ok");
    }
}
