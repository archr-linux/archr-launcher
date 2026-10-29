//! Modules as the screens see them: what each one declared, already in the
//! launcher's language, and the live values it sent. Pure data: the host
//! (archr-launcher `modules`) loads, runs and talks to them (docs/modulos.md).
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Start tiles in all, and per module.
pub const TILES: (usize, usize) = (4, 2);
/// Start shortcuts in all, and per module.
pub const SHORTCUTS: (usize, usize) = (2, 1);
/// Items of one quick menu in all, and per module.
pub const QUICK: (usize, usize) = (3, 1);

/// What choosing a tile or shortcut does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Opens the module's settings page with this index.
    Page(u8),
    Action(String),
}

/// A Start tile or shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    /// The module's own Lucide SVG, checked by the host.
    pub icon: Option<Arc<str>>,
    pub target: Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Game,
    System,
}

/// A quick menu (Y) item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quick {
    pub id: String,
    pub label: String,
    pub context: Context,
    /// Only on these systems; empty means all.
    pub systems: Vec<String>,
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Toggle {
        key: String,
    },
    /// Sliders arrive here too, as their steps.
    Value {
        key: String,
        choices: Vec<String>,
        labels: Vec<String>,
    },
    Check {
        key: String,
    },
    /// `confirm` asks this question before sending.
    Action {
        action: String,
        confirm: Option<String>,
    },
    /// Read only; the value comes from `ui.update`.
    Text {
        id: String,
    },
    Keyboard {
        key: String,
        max: u8,
        secret: bool,
    },
}
impl RowKind {
    /// The setting this row edits.
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::Toggle { key }
            | Self::Value { key, .. }
            | Self::Check { key }
            | Self::Keyboard { key, .. } => Some(key),
            Self::Action { .. } | Self::Text { .. } => None,
        }
    }
}

/// One declarative row of a module page; the layout is the launcher's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub subtitle: Option<String>,
    pub section: Option<String>,
    pub kind: RowKind,
    pub default: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub rows: Vec<Row>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Ready,
    /// Switched off after failing three times in ten minutes.
    Failed,
    /// Needs another API or a newer launcher.
    Incompatible,
    /// The folder is not a valid module.
    Invalid,
}

/// One line of the permission list: a locale key with its argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permission {
    pub key: &'static str,
    pub detail: String,
    /// Shown in amber: the user should think twice.
    pub sensitive: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub license: String,
    pub icon: Option<Arc<str>>,
    /// The user's switch.
    pub enabled: bool,
    /// The user saw and accepted what it asks for.
    pub granted: bool,
    pub status: Status,
    /// `owner/repo` for store modules; empty for a local folder.
    pub source: String,
    pub reviewed: bool,
    /// A newer version the store offers.
    pub update: Option<String>,
    pub permissions: Vec<Permission>,
    /// Why it cannot run (Invalid or Incompatible).
    pub problem: String,
    pub tiles: Vec<Entry>,
    pub shortcuts: Vec<Entry>,
    pub quick: Vec<Quick>,
    pub pages: Vec<Page>,
    /// Saved values of its settings rows.
    pub settings: BTreeMap<String, String>,
}
impl Module {
    /// Running, or allowed to run.
    pub fn active(&self) -> bool {
        self.enabled && self.granted && self.status == Status::Ready
    }
}

/// Everything installed, and what the running modules last said.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub modules: Vec<Module>,
    /// `ui.update` values by module id and target (`tile:<id>`, `row:<id>`...).
    pub values: BTreeMap<(String, String), String>,
    pub hidden: BTreeSet<(String, String)>,
    /// Actions sent and not answered yet: module id and action.
    pub busy: BTreeSet<(String, String)>,
}

impl View {
    pub fn value(&self, module: &str, target: &str) -> Option<&str> {
        self.values
            .get(&(module.to_owned(), target.to_owned()))
            .map(String::as_str)
    }
    fn shown(&self, module: &str, target: String) -> bool {
        !self.hidden.contains(&(module.to_owned(), target))
    }
    /// Active modules' entries under the per-module and total caps.
    fn entries<'a, T>(
        &'a self,
        (total, each): (usize, usize),
        list: impl Fn(&'a Module) -> &'a [T],
        target: impl Fn(&T) -> String,
    ) -> Vec<(usize, &'a T)> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.active())
            .flat_map(|(n, m)| {
                list(m)
                    .iter()
                    .filter(|e| self.shown(&m.id, target(e)))
                    .take(each)
                    .map(move |e| (n, e))
            })
            .take(total)
            .collect()
    }
    /// Module tiles of the Start menu, after the built-in ones.
    pub fn tiles(&self) -> Vec<(usize, &Entry)> {
        self.entries(TILES, |m| &m.tiles, |e| format!("tile:{}", e.id))
    }
    pub fn shortcuts(&self) -> Vec<(usize, &Entry)> {
        self.entries(
            SHORTCUTS,
            |m| &m.shortcuts,
            |e| format!("shortcut:{}", e.id),
        )
    }
    /// Quick menu items for a game (with its system) or for a system.
    pub fn quick(&self, context: Context, system: &str) -> Vec<(usize, &Quick)> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.active())
            .flat_map(|(n, m)| {
                m.quick
                    .iter()
                    .filter(|q| q.context == context)
                    .filter(|q| q.systems.is_empty() || q.systems.iter().any(|s| s == system))
                    .filter(|q| self.shown(&m.id, format!("quick:{}", q.id)))
                    .take(QUICK.1)
                    .map(move |q| (n, q))
            })
            .take(QUICK.0)
            .collect()
    }
}

/// Where an action was chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Call {
    pub system: String,
    pub game_key: String,
    pub rom: String,
}

/// The store tabs (Loja-modulos).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Reviewed,
    All,
    Installed,
}
impl Tab {
    pub const ALL: [Self; 3] = [Self::Reviewed, Self::All, Self::Installed];
    pub fn key(self) -> &'static str {
        match self {
            Self::Reviewed => "modules.reviewed_tab",
            Self::All => "modules.all_tab",
            Self::Installed => "modules.installed_tab",
        }
    }
}

/// A repository in the module store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub owner: String,
    pub repo: String,
    /// The module id and texts, once its module.toml is known.
    pub id: String,
    pub name: String,
    pub description: String,
    /// The Lucide name the module chose.
    pub icon: String,
    /// The latest release's version.
    pub version: String,
    pub stars: u64,
    pub reviewed: bool,
    /// Package digests (`sha256:<hex>`) the project reviewed for this
    /// repository: an installed module is reviewed only if its own matches.
    pub digests: Vec<String>,
}
impl Card {
    pub fn source(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
    pub fn title(&self) -> &str {
        if self.name.is_empty() {
            &self.repo
        } else {
            &self.name
        }
    }
}

/// The permission page of a store module (Loja-modulo).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    pub card: Card,
    pub version: String,
    pub author: String,
    pub license: String,
    /// Bytes of the package.
    pub size: u64,
    pub permissions: Vec<Permission>,
    /// For an update: only the permissions it adds, empty when none.
    pub added: Vec<Permission>,
    pub preview: Option<std::path::PathBuf>,
    /// The installed version, when there is one.
    pub installed: Option<String>,
    /// The release matches the reviewed list (badge).
    pub reviewed: bool,
    /// Locale key when it cannot be installed here.
    pub blocked: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreAction {
    /// Show a tab; `true` asks GitHub again.
    List(Tab, bool),
    Open(Box<Card>),
    Install(Box<Offer>),
}

/// The host's store answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreReply {
    Listed {
        tab: Tab,
        cards: Vec<Card>,
        problem: Option<&'static str>,
    },
    Card(Card),
    Offer(String, Result<Box<Offer>, &'static str>),
    /// An install ended; the key names the outcome.
    Done(Result<&'static str, &'static str>),
}

/// What the screens ask the host to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Invoke {
        module: String,
        action: String,
        call: Option<Call>,
    },
    /// Open a module page in Settings (handled by the launcher itself).
    Open {
        module: String,
        page: u8,
    },
    Set {
        module: String,
        key: String,
        value: String,
    },
    /// Switch a module on or off; `grant` records the consent first.
    Enable {
        module: String,
        on: bool,
        grant: bool,
    },
    /// Remove the code, and the data and settings too when `data`.
    Remove {
        module: String,
        data: bool,
    },
    Store(StoreAction),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(id: &str, tiles: usize) -> Module {
        Module {
            id: id.into(),
            enabled: true,
            granted: true,
            tiles: (0..tiles)
                .map(|n| Entry {
                    id: format!("t{n}"),
                    label: format!("{id} {n}"),
                    icon: None,
                    target: Target::Action("a".into()),
                })
                .collect(),
            quick: vec![Quick {
                id: "q".into(),
                label: id.into(),
                context: Context::Game,
                systems: vec!["snes".into()],
                action: "a".into(),
            }],
            ..Module::default()
        }
    }

    #[test]
    fn entries_respect_the_caps_the_switch_and_hidden_targets() {
        let mut view = View {
            modules: vec![module("a", 3), module("b", 1), module("c", 2)],
            ..View::default()
        };
        let ids = |view: &View| -> Vec<String> {
            view.tiles()
                .iter()
                .map(|(n, e)| format!("{}{}", view.modules[*n].id, e.id))
                .collect()
        };
        assert_eq!(ids(&view), ["at0", "at1", "bt0", "ct0"]);
        view.modules[0].enabled = false;
        view.hidden.insert(("b".into(), "tile:t0".into()));
        assert_eq!(ids(&view), ["ct0", "ct1"]);
        view.modules[2].status = Status::Failed;
        assert!(view.tiles().is_empty());
        assert_eq!(view.quick(Context::Game, "snes").len(), 1);
        assert!(view.quick(Context::Game, "nes").is_empty());
        assert!(view.quick(Context::System, "snes").is_empty());
    }
}
