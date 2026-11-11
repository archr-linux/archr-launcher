use archr_index::{cache::SystemEntry, Game};
use archr_ui::state::DataSource;
use std::collections::HashMap;

#[derive(Default)]
pub struct FakeData {
    pub systems: Vec<SystemEntry>,
    pub games: HashMap<String, Vec<Game>>,
    pub origins: HashMap<(String, usize), (String, usize)>,
    pub cores: HashMap<String, Vec<(String, String)>>,
    pub counts: HashMap<String, usize>,
    pub current: HashMap<(String, usize), (String, String)>,
    pub order: Vec<String>,
    pub hidden: Vec<String>,
    /// Each game's folder, by system; the systems listed by folder.
    pub dirs: HashMap<String, Vec<String>>,
    pub by_folder: Vec<String>,
}

impl DataSource for FakeData {
    fn system_order(&self) -> &[String] {
        &self.order
    }

    fn hidden_systems(&self) -> &[String] {
        &self.hidden
    }

    fn folders(&self, system: &str) -> Vec<&str> {
        let dirs = self.dirs.get(system);
        (0..self.games(system).len())
            .map(|n| dirs.and_then(|d| d.get(n)).map_or("", String::as_str))
            .collect()
    }

    fn shows_folders(&self, system: &str) -> bool {
        self.by_folder.iter().any(|s| s == system)
    }

    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }

    fn games(&self, system: &str) -> &[Game] {
        self.games.get(system).map(Vec::as_slice).unwrap_or(&[])
    }

    fn collection_len(&self, name: &str) -> usize {
        self.counts
            .get(name)
            .copied()
            .unwrap_or_else(|| self.games(name).len())
    }

    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        if system.starts_with('@') {
            self.origins.get(&(system.into(), game)).cloned()
        } else {
            self.games(system).get(game).map(|_| (system.into(), game))
        }
    }

    fn cores(&self, system: &str) -> Vec<(String, String)> {
        self.cores.get(system).cloned().unwrap_or_default()
    }

    fn current_core(&self, system: &str, game: usize) -> Option<(String, String)> {
        self.current.get(&(system.into(), game)).cloned()
    }
}

/// An open app (API 4) with a catalog, and its detail when `detail`.
#[allow(dead_code)]
pub fn app_session(detail: bool) -> archr_ui::apps::Session {
    use archr_ui::apps::{Action, Chip, Fact, Item, Layout, Message, Screen, Session};
    use archr_ui::modules::Tone;
    let mut session = Session::new("demo", "Demo catalog", true);
    let items = [
        "Pixel Hop",
        "Blue Lines",
        "Red Shift",
        "Violet Sky",
        "An extraordinarily long game title that has to fit",
    ]
    .iter()
    .enumerate()
    .map(|(n, title)| Item {
        id: format!("g{n}"),
        title: (*title).into(),
        subtitle: "Game Boy · ArchR · 2026".into(),
        badge: if n == 1 { "Installed" } else { "Free" }.into(),
        tone: if n == 1 { Tone::Good } else { Tone::Info },
        image: None,
    })
    .collect();
    let chips = ["All", "GB", "GBC", "GBA", "NES", "Mega Drive", "PICO-8"]
        .iter()
        .enumerate()
        .map(|(n, label)| Chip {
            id: format!("c{n}"),
            label: (*label).into(),
            selected: n == 0,
        })
        .collect();
    session.apply(Message::Show(Box::new(Screen {
        id: "catalog".into(),
        title: "Catalog".into(),
        chips,
        items,
        more: true,
        y: Some(Action {
            id: "y".into(),
            label: "Sort".into(),
            action: "sort".into(),
            ..Action::default()
        }),
        footer: "1.240 games".into(),
        ..Screen::default()
    })));
    session.top_mut().unwrap().cursor = 1;
    if detail {
        session.apply(Message::Show(Box::new(Screen {
            id: "game:g1".into(),
            title: "Blue Lines".into(),
            layout: Layout::Detail,
            subtitle: "Game Boy · ArchR · 2025".into(),
            facts: vec![
                Fact {
                    title: "Price".into(),
                    value: "Free".into(),
                    tone: Tone::Good,
                },
                Fact {
                    title: "Size".into(),
                    value: "128 KB".into(),
                    tone: Tone::Plain,
                },
            ],
            text: "A test cartridge. ".repeat(30),
            buttons: vec![
                Action {
                    id: "get".into(),
                    label: "Downloading".into(),
                    action: "get".into(),
                    progress: Some(42),
                    ..Action::default()
                },
                Action {
                    id: "page".into(),
                    label: "Page".into(),
                    action: "page".into(),
                    ..Action::default()
                },
            ],
            ..Screen::default()
        })));
    }
    session
}
