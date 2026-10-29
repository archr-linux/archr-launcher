//! Theme store (Loja-temas, Loja-tema): themes published on GitHub, browsed,
//! installed and removed. Pure state and input; the host fetches and installs.
use super::{Effect, Id, Page, Settings};
use crate::{layout::Names, state::Button, HintButton, Theme};
use std::{collections::BTreeMap, path::PathBuf, time::SystemTime};

/// The GitHub topic that lists a repository in the store.
pub const TOPIC: &str = "archr-launcher-theme";
/// Cards per grid row.
pub const COLUMNS: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Popular,
    Recent,
    Installed,
}
impl Tab {
    pub const ALL: [Self; 3] = [Self::Popular, Self::Recent, Self::Installed];
    pub fn key(self) -> &'static str {
        match self {
            Self::Popular => "store.popular",
            Self::Recent => "store.recent",
            Self::Installed => "store.installed",
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

/// One repository on the grid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub owner: String,
    pub repo: String,
    /// The theme's own name once its theme.toml is known.
    pub name: Names,
    pub stars: u64,
    pub branch: String,
    /// Last push, `YYYY-MM-DD`.
    pub pushed: String,
    /// preview.png on disk, once downloaded.
    pub preview: Option<PathBuf>,
}
impl Card {
    pub fn folder(&self) -> String {
        format!("{}-{}", self.owner, self.repo)
    }
    pub fn title(&self, language: &str) -> &str {
        self.name.get(language).unwrap_or(&self.repo)
    }
}

/// The details page (Loja-tema).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    pub card: Card,
    pub description: Names,
    pub author: String,
    pub version: String,
    /// Release or last push, `YYYY-MM-DD`.
    pub date: String,
    pub views: Vec<Names>,
    /// Locale keys of what the theme brings.
    pub includes: Vec<&'static str>,
    /// Bytes an install writes.
    pub size: u64,
    pub license: String,
    /// preview.png, then the screenshots, on disk.
    pub images: Vec<PathBuf>,
    /// What an install downloads (a release tag or the default branch) and
    /// the id of that content, which tells an update apart.
    pub reference: String,
    pub tree: String,
    /// `min_launcher` asks for a newer launcher than this one.
    pub too_new: bool,
}

/// Files an install keeps; everything else in the repository is ignored.
pub fn allowed(path: &str) -> bool {
    const EXTENSIONS: [&str; 12] = [
        "toml", "png", "jpg", "jpeg", "webp", "svg", "ttf", "otf", "ogg", "wav", "txt", "md",
    ];
    let name = path.rsplit('/').next().unwrap_or(path);
    let upper = name.to_ascii_uppercase();
    !path
        .split('/')
        .any(|part| part.is_empty() || part.starts_with('.'))
        && (matches!(upper.as_str(), "LICENSE" | "LICENCE" | "COPYING")
            || name.rsplit_once('.').is_some_and(|(stem, ext)| {
                !stem.is_empty() && EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
            }))
}

/// Short printable metadata text.
fn short(table: &toml::Table, key: &str, limit: usize) -> String {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// `wanted` (as `1.2` or `1.2.3`) is newer than `ours`.
pub fn newer(wanted: &str, ours: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse().unwrap_or(0))
            .collect()
    };
    let (mut a, mut b) = (parts(wanted), parts(ours));
    a.resize(3, 0);
    b.resize(3, 0);
    a > b
}

impl Detail {
    /// The details of `card` from its theme.toml and the files an install would
    /// keep (`path`, bytes). Fails when theme.toml is not a theme.
    pub fn describe(
        card: Card,
        source: &str,
        files: &[(String, u64)],
        launcher: &str,
    ) -> Result<Self, String> {
        let table: toml::Table = toml::from_str(source).map_err(|e| e.to_string())?;
        let theme = Theme::parse(&table);
        if theme.name.get("en").is_none() {
            return Err("theme.toml has no name".into());
        }
        let kept: Vec<&(String, u64)> = files.iter().filter(|(p, _)| allowed(p)).collect();
        let has = |test: &dyn Fn(&str) -> bool| kept.iter().any(|(p, _)| test(p));
        let lower = |p: &str| p.to_ascii_lowercase();
        let mut includes = vec!["store.colors"];
        for (key, found) in [
            (
                "store.fonts",
                has(&|p| lower(p).ends_with(".ttf") || lower(p).ends_with(".otf")),
            ),
            ("store.layout", table.contains_key("view")),
            ("store.art", has(&|p| p.starts_with("systems/"))),
            ("store.icons", has(&|p| p.starts_with("icons/"))),
            ("store.sounds", has(&|p| p.starts_with("sounds/"))),
        ] {
            if found {
                includes.push(key);
            }
        }
        let min = short(&table, "min_launcher", 16);
        Ok(Self {
            card: Card {
                name: theme.name.clone(),
                ..card
            },
            description: Names::parse(table.get("description"), "description").truncated(200),
            author: short(&table, "author", 40),
            version: short(&table, "version", 24),
            date: String::new(),
            views: theme.views().into_iter().map(|(_, names)| names).collect(),
            includes,
            size: kept.iter().map(|(_, size)| size).sum(),
            license: short(&table, "license", 40),
            images: Vec::new(),
            reference: String::new(),
            tree: String::new(),
            too_new: !min.is_empty() && newer(&min, launcher),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Show a tab; `true` asks GitHub again instead of the saved list.
    List(Tab, bool),
    Open(Box<Card>),
    Install(Box<Detail>),
    /// Remove the theme folder.
    Remove(String),
}

/// Host answers.
#[derive(Clone, Debug)]
pub enum Reply {
    Listed {
        tab: Tab,
        cards: Vec<Card>,
        /// Locale key of a failure; `cards` may still be a saved list.
        problem: Option<&'static str>,
        fetched: Option<SystemTime>,
    },
    /// A card's name and cover arrived.
    Card(Card),
    /// The details of the theme in that folder.
    Detail(String, Result<Box<Detail>, &'static str>),
    /// Themes installed by the store, by folder with their content id.
    Installed(BTreeMap<String, String>, Vec<Card>),
    /// An install or removal ended; the key names the outcome.
    Done(Result<&'static str, &'static str>),
}

#[derive(Clone, Debug, Default)]
pub struct Store {
    pub tab: Tab,
    pub cards: [Vec<Card>; 3],
    /// Store themes on this device: folder and content id.
    pub installed: BTreeMap<String, String>,
    /// A list or details request is in flight.
    pub loading: bool,
    /// Locale key of the last failure.
    pub problem: Option<&'static str>,
    pub fetched: Option<SystemTime>,
    pub detail: Option<Detail>,
    /// Folder of the theme whose details are open.
    pub opened: String,
    /// The image shown large on the details page.
    pub image: usize,
    /// Locale key while an install or removal runs.
    pub working: Option<&'static str>,
    pub confirm_remove: bool,
}
impl Store {
    pub fn cards(&self) -> &[Card] {
        &self.cards[self.tab.index()]
    }
    /// The details' main action: install, update or use.
    pub fn action(&self) -> Option<&'static str> {
        let detail = self.detail.as_ref()?;
        Some(match self.installed.get(&detail.card.folder()) {
            Some(tree) if *tree == detail.tree => "store.use",
            Some(_) => "store.update",
            None => "store.install",
        })
    }
    pub fn is_installed(&self, card: &Card) -> bool {
        self.installed.contains_key(&card.folder())
    }
}

impl Settings {
    fn list(&mut self, tab: Tab, refresh: bool) -> Effect {
        self.store.tab = tab;
        self.store.loading = true;
        self.store.problem = None;
        Effect::Store(Action::List(tab, refresh))
    }
    pub(super) fn open_store(&mut self) -> Effect {
        self.open(Page::Store);
        self.list(self.store.tab, false)
    }
    /// The store theme in use: on screen or saved.
    fn active_theme(&self, folder: &str) -> bool {
        self.value(Id::Theme) == folder || self.appearance_saved.theme == folder
    }
    pub(super) fn store_input(&mut self, button: Button) -> Option<Effect> {
        let store = &mut self.store;
        if store.working.is_some() {
            return Some(Effect::None);
        }
        if store.confirm_remove {
            store.confirm_remove = false;
            let folder = store.detail.as_ref().map(|d| d.card.folder());
            return Some(match (button, folder) {
                (Button::A, Some(folder)) => {
                    store.working = Some("store.removing");
                    Effect::Store(Action::Remove(folder))
                }
                _ => Effect::None,
            });
        }
        match self.page() {
            Page::Store => Some(self.grid_input(button)),
            Page::StoreTheme => Some(self.detail_input(button)),
            _ => None,
        }
    }
    fn grid_input(&mut self, button: Button) -> Effect {
        let count = self.store.cards().len();
        let tab = self.store.tab.index();
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(count.saturating_sub(1)),
            Button::L | Button::R => {
                let step = if button == Button::L { 2 } else { 1 };
                self.cursor = 0;
                return self.list(Tab::ALL[(tab + step) % 3], false);
            }
            Button::Y => return self.list(self.store.tab, true),
            Button::A => {
                let Some(card) = self.store.cards().get(self.cursor).cloned() else {
                    return Effect::None;
                };
                self.store.detail = None;
                self.store.opened = card.folder();
                self.store.image = 0;
                self.store.loading = true;
                self.store.problem = None;
                self.open(Page::StoreTheme);
                return Effect::Store(Action::Open(Box::new(card)));
            }
            Button::B => return self.back(),
            _ => {}
        }
        Effect::None
    }
    fn detail_input(&mut self, button: Button) -> Effect {
        let Some(detail) = self.store.detail.clone() else {
            return if button == Button::B {
                self.back()
            } else {
                Effect::None
            };
        };
        let folder = detail.card.folder();
        match button {
            Button::L | Button::R if !detail.images.is_empty() => {
                let len = detail.images.len();
                let step = if button == Button::L { len - 1 } else { 1 };
                self.store.image = (self.store.image + step) % len;
            }
            Button::A => match self.store.action() {
                Some("store.use") => {
                    // As choosing it on the Appearance page: a draft shown at once.
                    self.values.insert(Id::Theme, folder.clone());
                    self.back();
                    self.back();
                    return Effect::Theme(folder);
                }
                _ if detail.too_new => self.fail(None, "store.too_new".into()),
                _ => {
                    self.store.working = Some("store.installing");
                    return Effect::Store(Action::Install(Box::new(detail)));
                }
            },
            Button::X if self.store.installed.contains_key(&folder) => {
                if self.active_theme(&folder) {
                    self.fail(None, "store.active_theme".into());
                } else {
                    self.store.confirm_remove = true;
                }
            }
            Button::B => return self.back(),
            _ => {}
        }
        Effect::None
    }
    pub(super) fn store_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::{Key, Pair};
        let store = &self.store;
        if store.working.is_some() {
            return &[];
        }
        if store.confirm_remove {
            return &[(Key('A'), "store.remove"), (Key('B'), "hint.cancel")];
        }
        if self.page() == Page::Store {
            return &[
                (Pair('L', 'R'), "store.tab"),
                (Key('A'), "store.view"),
                (Key('Y'), "store.refresh"),
                (Key('B'), "hint.back"),
            ];
        }
        let removable = store
            .detail
            .as_ref()
            .is_some_and(|d| store.installed.contains_key(&d.card.folder()));
        match (store.action(), removable) {
            (Some("store.use"), true) => &[
                (Pair('L', 'R'), "store.images"),
                (Key('A'), "store.use"),
                (Key('X'), "store.remove"),
                (Key('B'), "hint.back"),
            ],
            (Some("store.update"), true) => &[
                (Pair('L', 'R'), "store.images"),
                (Key('A'), "store.update"),
                (Key('X'), "store.remove"),
                (Key('B'), "hint.back"),
            ],
            (Some(_), _) => &[
                (Pair('L', 'R'), "store.images"),
                (Key('A'), "store.install"),
                (Key('B'), "hint.back"),
            ],
            (None, _) => &[(Key('B'), "hint.back")],
        }
    }
    /// A host answer; stale ones (another tab, another theme) are dropped.
    pub fn store_reply(&mut self, reply: Reply) {
        let page = self.page();
        let store = &mut self.store;
        match reply {
            Reply::Listed {
                tab,
                cards,
                problem,
                fetched,
            } => {
                if tab == store.tab {
                    store.loading = false;
                    store.problem = problem;
                    store.fetched = fetched;
                }
                store.cards[tab.index()] = cards;
                if page == Page::Store {
                    self.cursor = self.cursor.min(self.store.cards().len().saturating_sub(1));
                }
            }
            Reply::Card(card) => {
                for found in store.cards.iter_mut().flatten() {
                    if found.owner == card.owner && found.repo == card.repo {
                        found.name = card.name.clone();
                        found.preview = card.preview.clone();
                    }
                }
            }
            Reply::Detail(folder, result) => {
                // A late answer for a theme left behind is dropped.
                if page != Page::StoreTheme || store.detail.is_some() || folder != store.opened {
                    return;
                }
                store.loading = false;
                match result {
                    Ok(detail) => store.detail = Some(*detail),
                    Err(key) => store.problem = Some(key),
                }
            }
            Reply::Installed(installed, cards) => {
                let themes = self.options.entry(Id::Theme).or_default();
                themes.retain(|t| installed.contains_key(t) || !store.installed.contains_key(t));
                for card in &cards {
                    let folder = card.folder();
                    self.theme_names.insert(folder.clone(), card.name.clone());
                    if !themes.contains(&folder) {
                        themes.push(folder);
                    }
                }
                themes.sort();
                store.installed = installed;
                store.cards[Tab::Installed.index()] = cards;
                if store.tab == Tab::Installed {
                    store.loading = false;
                    store.problem = None;
                }
            }
            Reply::Done(result) => {
                store.working = None;
                match result {
                    Ok(key) => {
                        self.fail(None, key.into());
                        if key == "store.removed" && page == Page::StoreTheme {
                            self.back();
                        }
                    }
                    Err(key) => self.fail(None, key.into()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(owner: &str, repo: &str) -> Card {
        Card {
            owner: owner.into(),
            repo: repo.into(),
            branch: "main".into(),
            ..Card::default()
        }
    }
    fn store() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        state.open(Page::Appearance);
        assert_eq!(
            state.open_store(),
            Effect::Store(Action::List(Tab::Popular, false))
        );
        state.store_reply(Reply::Listed {
            tab: Tab::Popular,
            cards: vec![card("kaio", "neon"), card("ana", "papel")],
            problem: None,
            fetched: None,
        });
        state
    }
    fn detail(state: &mut Settings) -> Detail {
        let Effect::Store(Action::Open(card)) = state.handle(Button::A) else {
            panic!("no open");
        };
        let mut detail = Detail::describe(*card, "name = \"Neon\"\n", &[], "0.1.0").unwrap();
        detail.tree = "t1".into();
        detail.images = vec!["/a.png".into(), "/b.png".into()];
        state.store_reply(Reply::Detail(
            "ana-papel".into(),
            Ok(Box::new(Detail::default())),
        ));
        assert!(state.store.detail.is_none(), "another theme's late answer");
        state.store_reply(Reply::Detail(
            "kaio-neon".into(),
            Ok(Box::new(detail.clone())),
        ));
        detail
    }

    #[test]
    fn tabs_cycle_with_l_and_r_and_y_refreshes() {
        let mut state = store();
        state.handle(Button::Down);
        assert_eq!(state.cursor, 1);
        assert_eq!(
            state.handle(Button::R),
            Effect::Store(Action::List(Tab::Recent, false))
        );
        assert_eq!(state.cursor, 0);
        assert_eq!(
            state.handle(Button::L),
            Effect::Store(Action::List(Tab::Popular, false))
        );
        assert_eq!(
            state.handle(Button::L),
            Effect::Store(Action::List(Tab::Installed, false))
        );
        assert_eq!(
            state.handle(Button::Y),
            Effect::Store(Action::List(Tab::Installed, true))
        );
        // A late answer for another tab never replaces what is shown.
        state.store_reply(Reply::Listed {
            tab: Tab::Popular,
            cards: Vec::new(),
            problem: Some("store.offline"),
            fetched: None,
        });
        assert!(state.store.loading && state.store.problem.is_none());
    }

    #[test]
    fn install_then_use_now_drafts_the_theme_on_the_appearance_page() {
        let mut state = store();
        let detail = detail(&mut state);
        assert_eq!(state.page(), Page::StoreTheme);
        assert_eq!(state.store.action(), Some("store.install"));
        state.handle(Button::R);
        assert_eq!(state.store.image, 1);
        assert_eq!(
            state.handle(Button::A),
            Effect::Store(Action::Install(Box::new(detail.clone())))
        );
        assert_eq!(
            state.handle(Button::B),
            Effect::None,
            "busy while installing"
        );
        let mut installed = BTreeMap::new();
        installed.insert("kaio-neon".to_owned(), "t1".to_owned());
        state.store_reply(Reply::Installed(installed, vec![detail.card.clone()]));
        state.store_reply(Reply::Done(Ok("store.installed_ok")));
        assert!(state.options[&Id::Theme].contains(&"kaio-neon".to_owned()));
        assert_eq!(state.store.action(), Some("store.use"));
        assert_eq!(state.handle(Button::A), Effect::Theme("kaio-neon".into()));
        assert_eq!(state.page(), Page::Appearance);
        assert_eq!(state.value(Id::Theme), "kaio-neon");
        assert_eq!(state.handle(Button::B), Effect::Appearance);
    }

    #[test]
    fn a_newer_content_offers_an_update_and_the_active_theme_stays() {
        let mut state = store();
        let detail = detail(&mut state);
        let mut installed = BTreeMap::new();
        installed.insert("kaio-neon".to_owned(), "t0".to_owned());
        state.store_reply(Reply::Installed(installed, vec![detail.card]));
        assert_eq!(state.store.action(), Some("store.update"));
        state.values.insert(Id::Theme, "kaio-neon".into());
        assert_eq!(state.handle(Button::X), Effect::None);
        assert!(!state.store.confirm_remove);
        assert_eq!(state.error.as_ref().unwrap().1, "store.active_theme");
        state.values.insert(Id::Theme, "default".into());
        state.handle(Button::X);
        assert!(state.store.confirm_remove);
        assert_eq!(
            state.handle(Button::A),
            Effect::Store(Action::Remove("kaio-neon".into()))
        );
        state.store_reply(Reply::Installed(BTreeMap::new(), Vec::new()));
        state.store_reply(Reply::Done(Ok("store.removed")));
        assert!(!state.options[&Id::Theme].contains(&"kaio-neon".to_owned()));
        assert_eq!(state.page(), Page::Store);
    }

    #[test]
    fn details_come_from_theme_toml_and_the_kept_files() {
        let toml = "name = { en = \"Neon\", pt_BR = \"Néon\" }\n\
                    description = \"Rosa neon\"\nauthor = \"kaio\"\nversion = \"1.2.0\"\n\
                    license = \"CC BY 4.0\"\nmin_launcher = \"0.2\"\n\
                    [view.grade]\nname = \"Grade\"\n";
        let files = [
            ("theme.toml".to_owned(), 100),
            ("fonts/A.ttf".to_owned(), 1000),
            ("sounds/move.ogg".to_owned(), 10),
            ("Install.sh".to_owned(), 5000),
            (".github/x.md".to_owned(), 5000),
        ];
        let detail = Detail::describe(card("kaio", "neon"), toml, &files, "0.1.0").unwrap();
        assert_eq!(detail.card.title("pt_BR"), "Néon");
        assert_eq!(detail.author, "kaio");
        assert_eq!(
            detail.includes,
            [
                "store.colors",
                "store.fonts",
                "store.layout",
                "store.sounds"
            ]
        );
        assert_eq!(detail.size, 1110);
        assert_eq!(detail.views.len(), 3, "lista, vitrine and grade");
        assert!(detail.too_new);
        assert!(Detail::describe(card("a", "b"), "accent = \"#fff\"", &[], "1").is_err());
        assert!(Detail::describe(card("a", "b"), "name = ", &[], "1").is_err());
    }

    #[test]
    fn only_theme_files_are_allowed() {
        for ok in [
            "theme.toml",
            "fonts/A.TTF",
            "LICENSE",
            "screenshots/1.png",
            "X.jpeg",
            "systems/snes.webp",
        ] {
            assert!(allowed(ok), "{ok}");
        }
        for no in [
            "Run.sh",
            ".hidden/a.png",
            "a/.b.png",
            "Lib.so",
            ".png",
            "a//b.png",
            "Makefile",
        ] {
            assert!(!allowed(no), "{no}");
        }
        assert!(newer("0.2", "0.1.9") && !newer("0.1", "0.1.0") && !newer("", "0.1.0"));
    }
}
