//! Apps (API 4): a module's own screens, drawn by the launcher with the
//! games and details layouts (docs/modulos.md, section 10). The host checks
//! and resolves what the module sends (texts, pictures inside its folder);
//! this keeps the screen stack and turns buttons into what the module hears.
use crate::keyboard::{Keyboard, Mode};
use crate::modules::{Call, Tone};
use crate::state::{Button, DataSource};
use archr_index::{cache::SystemEntry, Game};
use serde_json::{json, Value as Json};
use std::{path::PathBuf, sync::Arc};

/// Carousel rows of apps: `@app:<module id>`.
pub const PREFIX: &str = "@app:";
/// Items one screen holds, and one `screen.show` or `append` brings.
pub const ITEMS: usize = 1000;
pub const PAGE: usize = 50;
pub const CHIPS: usize = 8;
pub const FACTS: usize = 8;
pub const BUTTONS: usize = 4;
pub const GALLERY: usize = 8;
/// Screens on the stack; a deeper `screen.show` takes the top's place.
pub const STACK: usize = 8;
/// `screen.more` goes this close to the end of the list.
const NEAR_END: usize = 5;
/// The search text, in bytes.
const SEARCH: usize = 64;

pub fn row(module: &str) -> String {
    format!("{PREFIX}{module}")
}
pub fn module_of(row: &str) -> Option<&str> {
    row.strip_prefix(PREFIX)
}

/// An app's carousel row, from its manifest and its `app:summary`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// `@app:<module id>`.
    pub key: String,
    pub name: String,
    pub subtitle: String,
    pub summary: String,
    /// The module's Lucide SVG.
    pub icon: Option<Arc<str>>,
    /// The module's own carousel picture.
    pub art: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub tone: Tone,
    pub image: Option<PathBuf>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chip {
    pub id: String,
    pub label: String,
    pub selected: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fact {
    pub title: String,
    pub value: String,
    pub tone: Tone,
}
/// A button of a detail screen, or X and Y of a collection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub icon: Option<Arc<str>>,
    pub action: String,
    /// 0–100 while the module works on it.
    pub progress: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Collection,
    Detail,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub layout: Layout,
    pub chips: Vec<Chip>,
    pub items: Vec<Item>,
    pub more: bool,
    pub x: Option<Action>,
    pub y: Option<Action>,
    pub footer: String,
    pub image: Option<PathBuf>,
    pub gallery: Vec<PathBuf>,
    pub subtitle: String,
    pub facts: Vec<Fact>,
    pub text: String,
    pub buttons: Vec<Action>,
    /// The selected item, the focused button, the text's first line and
    /// the picture shown (0 is `image`, then the gallery).
    pub cursor: usize,
    pub focus: usize,
    pub scroll: usize,
    pub picture: usize,
    /// Lines of text below the view, as last drawn.
    pub overflow: usize,
    /// `screen.more` was sent for the items there are now; kept by the session.
    pub asked: bool,
    /// The items as the games renderer reads them; made by the session.
    pub games: Vec<Game>,
}
impl Screen {
    /// The items as games: the id is the path, the key of covers and fades.
    fn mirror(&mut self) {
        self.games = self.items.iter().map(game).collect();
    }
    /// The picture the detail shows now.
    pub fn shown(&self) -> Option<&PathBuf> {
        self.image
            .iter()
            .chain(&self.gallery)
            .nth(self.picture)
            .or(self.image.as_ref())
    }
    pub fn pictures(&self) -> usize {
        usize::from(self.image.is_some()) + self.gallery.len()
    }
}
fn game(item: &Item) -> Game {
    Game {
        path: PathBuf::from(&item.id),
        name: item.title.clone(),
        image: item.image.clone(),
        video: None,
        favorite: false,
        last_played: None,
        year: None,
        players: None,
        details: Default::default(),
    }
}

/// What a `screen.patch` changes in an item or a button.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub label: Option<String>,
    pub subtitle: Option<String>,
    pub badge: Option<String>,
    pub tone: Option<Tone>,
    pub image: Option<PathBuf>,
    /// `Some(None)` takes the bar away.
    pub progress: Option<Option<u8>>,
    pub action: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Item(String),
    Button(String),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Patch {
    /// The screen it is for; the top one when absent.
    pub screen: Option<String>,
    pub append: Vec<Item>,
    pub more: Option<bool>,
    pub chips: Option<Vec<Chip>>,
    pub footer: Option<String>,
    pub target: Option<(Target, Change)>,
}

/// What the module sends, checked by the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Show(Box<Screen>),
    Patch(Box<Patch>),
    Toast(String, Tone),
    Confirm(Ask),
    Busy(Option<String>),
    Offline(Option<String>),
    /// `port.progress` of a PortMaster install: the name and 0–100.
    Port(String, u8),
}

/// `screen.confirm`: the launcher's question, answered with `screen.answer`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    pub id: String,
    pub text: String,
    pub yes: String,
    pub no: String,
    pub on_yes: bool,
    /// The launcher's own question (a removal): its words, its answer.
    pub launcher: bool,
}

/// What a button does for the module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// An event to the app: `screen.select`, `screen.chip`...
    Event(&'static str, Json),
    Invoke(String, Call),
    /// START: the module's pages in Settings.
    Settings,
    /// B on the first screen.
    Close,
}

/// One open app.
#[derive(Clone, Debug, Default)]
pub struct Session {
    pub module: String,
    pub name: String,
    /// The module has pages in Settings, which START opens.
    pub settings: bool,
    pub stack: Vec<Screen>,
    /// `screen.busy`, until the next show or patch.
    pub busy: Option<String>,
    pub offline: Option<String>,
    pub ask: Option<Ask>,
    pub keyboard: Option<Keyboard>,
}
impl Session {
    pub fn new(module: &str, name: &str, settings: bool) -> Self {
        Self {
            module: module.to_owned(),
            name: name.to_owned(),
            settings,
            ..Self::default()
        }
    }
    pub fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }
    pub fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }
    /// The key the renderer knows the top screen by.
    pub fn key(&self) -> String {
        let screen = self.top().map_or("", |s| s.id.as_str());
        format!("{}/{screen}", row(&self.module))
    }
    pub fn apply(&mut self, message: Message) {
        match message {
            Message::Show(screen) => self.show(*screen),
            Message::Patch(patch) => self.patch(*patch),
            Message::Confirm(ask) => self.ask = Some(ask),
            Message::Busy(text) => self.busy = text,
            Message::Offline(text) => self.offline = text,
            // The main loop shows these in its banner.
            Message::Toast(..) | Message::Port(..) => {}
        }
    }
    /// A screen already on the stack is replaced where it is, keeping the
    /// user's place and the screens above it; a new one goes on top.
    fn show(&mut self, mut screen: Screen) {
        self.busy = None;
        screen.mirror();
        if let Some(old) = self.stack.iter_mut().find(|s| s.id == screen.id) {
            screen.cursor = old.cursor.min(screen.items.len().saturating_sub(1));
            screen.focus = old.focus.min(screen.buttons.len().saturating_sub(1));
            screen.picture = old.picture.min(screen.pictures().saturating_sub(1));
            *old = screen;
            return;
        }
        if self.stack.len() >= STACK {
            self.stack.pop();
        }
        self.stack.push(screen);
    }
    fn patch(&mut self, patch: Patch) {
        self.busy = None;
        let screen = match &patch.screen {
            Some(id) => self.stack.iter_mut().find(|s| s.id == *id),
            None => self.stack.last_mut(),
        };
        let Some(screen) = screen else { return };
        if !patch.append.is_empty() {
            let room = ITEMS.saturating_sub(screen.items.len());
            let new: Vec<Item> = patch.append.into_iter().take(room).collect();
            screen.games.extend(new.iter().map(game));
            screen.items.extend(new);
            screen.asked = false;
        }
        if let Some(more) = patch.more {
            screen.more = more;
        }
        if let Some(chips) = patch.chips {
            screen.chips = chips;
        }
        if let Some(footer) = patch.footer {
            screen.footer = footer;
        }
        match patch.target {
            Some((Target::Item(id), change)) => {
                if let Some(at) = screen.items.iter().position(|i| i.id == id) {
                    let item = &mut screen.items[at];
                    let Change {
                        label,
                        subtitle,
                        badge,
                        tone,
                        image,
                        ..
                    } = change;
                    item.title = label.unwrap_or(std::mem::take(&mut item.title));
                    item.subtitle = subtitle.unwrap_or(std::mem::take(&mut item.subtitle));
                    item.badge = badge.unwrap_or(std::mem::take(&mut item.badge));
                    item.tone = tone.unwrap_or(item.tone);
                    if image.is_some() {
                        item.image = image;
                    }
                    // Only the changed item, never the whole list again.
                    screen.games[at] = game(item);
                }
            }
            Some((Target::Button(id), change)) => {
                let all = screen
                    .buttons
                    .iter_mut()
                    .chain(screen.x.as_mut())
                    .chain(screen.y.as_mut());
                for button in all.filter(|b| b.id == id) {
                    if let Some(label) = &change.label {
                        button.label.clone_from(label);
                    }
                    if let Some(action) = &change.action {
                        button.action.clone_from(action);
                    }
                    if let Some(progress) = change.progress {
                        button.progress = progress;
                    }
                }
            }
            None => {}
        }
    }

    /// A button press: what the module should hear, if anything. `page` is
    /// how many rows the list shows, for L and R without chips.
    pub fn handle(&mut self, button: Button, page: usize) -> Option<Effect> {
        if let Some(keyboard) = &mut self.keyboard {
            if button == Button::B {
                self.keyboard = None;
                return None;
            }
            let text = keyboard.handle(button)?;
            self.keyboard = None;
            let screen = self.top()?.id.clone();
            return Some(Effect::Event(
                "screen.search",
                json!({"screen": screen, "text": text}),
            ));
        }
        if let Some(ask) = &mut self.ask {
            let yes = match button {
                Button::L | Button::R | Button::Up | Button::Down => {
                    ask.on_yes = !ask.on_yes;
                    return None;
                }
                Button::A => ask.on_yes,
                Button::B => false,
                _ => return None,
            };
            let ask = self.ask.take().unwrap_or_default();
            if ask.launcher {
                return Some(Effect::Event("launcher.answer", json!({"yes": yes})));
            }
            let id = ask.id;
            return Some(Effect::Event(
                "screen.answer",
                json!({"id": id, "yes": yes}),
            ));
        }
        if button == Button::Start && self.settings {
            return Some(Effect::Settings);
        }
        let Some(screen) = self.stack.last_mut() else {
            return (button == Button::B).then_some(Effect::Close);
        };
        if button == Button::B {
            let closed = self.stack.pop().map(|s| s.id).unwrap_or_default();
            self.busy = None;
            return Some(if self.stack.is_empty() {
                Effect::Close
            } else {
                Effect::Event("screen.back", json!({"screen": closed}))
            });
        }
        let id = screen.id.clone();
        let call = |item: Option<String>, button: Option<String>| Call {
            screen: Some(id.clone()),
            item,
            button,
            ..Call::default()
        };
        match screen.layout {
            Layout::Collection => {
                let last = screen.items.len().saturating_sub(1);
                let item = screen.items.get(screen.cursor).map(|i| i.id.clone());
                match button {
                    Button::Up => screen.cursor = screen.cursor.saturating_sub(1),
                    Button::Down => screen.cursor = (screen.cursor + 1).min(last),
                    Button::L | Button::R if !screen.chips.is_empty() => {
                        let at = screen.chips.iter().position(|c| c.selected).unwrap_or(0);
                        let to = if button == Button::L {
                            at.saturating_sub(1)
                        } else {
                            (at + 1).min(screen.chips.len() - 1)
                        };
                        if to == at {
                            return None;
                        }
                        for (n, chip) in screen.chips.iter_mut().enumerate() {
                            chip.selected = n == to;
                        }
                        screen.cursor = 0;
                        let chip = screen.chips[to].id.clone();
                        return Some(Effect::Event(
                            "screen.chip",
                            json!({"screen": id, "chip": chip}),
                        ));
                    }
                    Button::L => screen.cursor = screen.cursor.saturating_sub(page.max(1)),
                    Button::R => screen.cursor = (screen.cursor + page.max(1)).min(last),
                    Button::A => {
                        return item.map(|item| {
                            Effect::Event("screen.select", json!({"screen": id, "item": item}))
                        })
                    }
                    Button::X | Button::Y => {
                        let key = if button == Button::X {
                            &screen.x
                        } else {
                            &screen.y
                        };
                        let action = key.as_ref()?.action.clone();
                        return Some(Effect::Invoke(action, call(item, None)));
                    }
                    Button::Select => {
                        let mut keyboard = Keyboard::new(Mode::Text, String::new());
                        keyboard.max = SEARCH;
                        self.keyboard = Some(keyboard);
                        return None;
                    }
                    _ => return None,
                }
                let near = screen.cursor + NEAR_END >= screen.items.len();
                if near && screen.more && !screen.asked {
                    screen.asked = true;
                    let count = screen.items.len();
                    return Some(Effect::Event(
                        "screen.more",
                        json!({"screen": id, "count": count}),
                    ));
                }
                None
            }
            Layout::Detail => {
                let last = screen.buttons.len().saturating_sub(1);
                match button {
                    Button::Up => screen.scroll = screen.scroll.saturating_sub(1),
                    Button::Down => screen.scroll = (screen.scroll + 1).min(screen.overflow),
                    Button::L => screen.focus = screen.focus.saturating_sub(1),
                    Button::R => screen.focus = (screen.focus + 1).min(last),
                    Button::X if screen.pictures() > 1 => {
                        screen.picture = (screen.picture + 1) % screen.pictures();
                    }
                    Button::A => {
                        let action = screen.buttons.get(screen.focus)?;
                        let (name, button) = (action.action.clone(), action.id.clone());
                        return Some(Effect::Invoke(name, call(None, Some(button))));
                    }
                    _ => {}
                }
                None
            }
        }
    }
}

/// The library seen through an app screen: its items are the games of the
/// session's key, so the games list, cover and details draw them.
pub struct Screens<'a> {
    pub data: &'a dyn DataSource,
    pub screen: &'a Screen,
    systems: Vec<SystemEntry>,
}
impl<'a> Screens<'a> {
    pub fn new(data: &'a dyn DataSource, session: &'a Session, screen: &'a Screen) -> Self {
        Self {
            data,
            screen,
            systems: vec![SystemEntry {
                name: session.key(),
                // The header names the screen; the app's name is the fallback.
                fullname: if screen.title.is_empty() {
                    session.name.clone()
                } else {
                    screen.title.clone()
                },
                game_count: screen.items.len() as u32,
                dir_mtime: 0,
                source_fingerprint: 0,
            }],
        }
    }
}
impl DataSource for Screens<'_> {
    fn systems(&self) -> &[SystemEntry] {
        &self.systems
    }
    fn games(&self, system: &str) -> &[Game] {
        if system == self.systems[0].name {
            &self.screen.games
        } else {
            self.data.games(system)
        }
    }
    fn origin(&self, _system: &str, _game: usize) -> Option<(String, usize)> {
        None
    }
    fn cores(&self, _system: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    fn current_core(&self, _system: &str, _game: usize) -> Option<(String, String)> {
        None
    }
    fn badge(&self, _system: &str, game: usize) -> Option<(&str, Tone)> {
        let item = self.screen.items.get(game)?;
        (!item.badge.is_empty()).then_some((item.badge.as_str(), item.tone))
    }
    fn subtitle(&self, _system: &str, game: usize) -> Option<&str> {
        let item = self.screen.items.get(game)?;
        (!item.subtitle.is_empty()).then_some(item.subtitle.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            title: id.to_uppercase(),
            ..Item::default()
        }
    }
    fn catalog(items: usize) -> Screen {
        Screen {
            id: "catalog".into(),
            title: "Catalog".into(),
            chips: ["all", "gb"]
                .iter()
                .enumerate()
                .map(|(n, c)| Chip {
                    id: (*c).into(),
                    label: c.to_uppercase(),
                    selected: n == 0,
                })
                .collect(),
            items: (0..items).map(|n| item(&format!("g{n}"))).collect(),
            more: true,
            y: Some(Action {
                id: "y".into(),
                label: "Sort".into(),
                action: "sort".into(),
                ..Action::default()
            }),
            ..Screen::default()
        }
    }
    fn detail() -> Screen {
        Screen {
            id: "game:g1".into(),
            layout: Layout::Detail,
            image: Some("/home/a.png".into()),
            gallery: vec!["/home/b.png".into()],
            buttons: ["get", "page"]
                .iter()
                .map(|id| Action {
                    id: (*id).into(),
                    label: id.to_uppercase(),
                    action: format!("do-{id}"),
                    ..Action::default()
                })
                .collect(),
            ..Screen::default()
        }
    }
    fn event(effect: Option<Effect>) -> (&'static str, Json) {
        match effect {
            Some(Effect::Event(name, data)) => (name, data),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_stack_opens_replaces_and_closes_with_b() {
        let mut session = Session::new("demo", "Demo", true);
        assert_eq!(
            session.handle(Button::B, 8),
            Some(Effect::Close),
            "nothing shown yet"
        );
        session.apply(Message::Show(Box::new(catalog(10))));
        session.handle(Button::Down, 8);
        session.handle(Button::Down, 8);
        let (name, data) = event(session.handle(Button::A, 8));
        assert_eq!((name, data["item"].as_str()), ("screen.select", Some("g2")));
        session.apply(Message::Show(Box::new(detail())));
        assert_eq!(session.stack.len(), 2);
        assert_eq!(session.key(), "@app:demo/game:g1");
        // The catalog shown again (its settings changed) keeps the user's
        // place, and the detail stays on top.
        let mut again = catalog(4);
        again.title = "Again".into();
        session.apply(Message::Show(Box::new(again)));
        assert_eq!(session.stack.len(), 2);
        assert_eq!(session.stack[0].cursor, 2);
        assert_eq!(session.stack[0].title, "Again");
        assert_eq!(session.top().unwrap().id, "game:g1");
        let (name, data) = event(session.handle(Button::B, 8));
        assert_eq!(
            (name, data["screen"].as_str()),
            ("screen.back", Some("game:g1"))
        );
        assert_eq!(session.handle(Button::Start, 8), Some(Effect::Settings));
        assert_eq!(session.handle(Button::B, 8), Some(Effect::Close));
        assert!(session.stack.is_empty());
        for n in 0..STACK + 2 {
            let mut screen = detail();
            screen.id = format!("s{n}");
            session.apply(Message::Show(Box::new(screen)));
        }
        assert_eq!(
            session.stack.len(),
            STACK,
            "a deeper show takes the top's place"
        );
    }

    #[test]
    fn collections_ask_for_more_switch_chips_and_search() {
        let mut session = Session::new("demo", "Demo", false);
        session.apply(Message::Show(Box::new(catalog(8))));
        assert_eq!(
            session.handle(Button::Start, 8),
            None,
            "no pages in Settings"
        );
        assert_eq!(session.handle(Button::Down, 8), None);
        assert_eq!(session.handle(Button::Down, 8), None);
        let (name, data) = event(session.handle(Button::Down, 8));
        assert_eq!((name, data["count"].as_u64()), ("screen.more", Some(8)));
        assert_eq!(session.handle(Button::Down, 8), None, "asked once");
        session.apply(Message::Patch(Box::new(Patch {
            append: (8..12).map(|n| item(&format!("g{n}"))).collect(),
            more: Some(false),
            ..Patch::default()
        })));
        assert_eq!(
            session.top().unwrap().games.len(),
            12,
            "mirrored for the list"
        );
        assert_eq!(session.handle(Button::Down, 8), None, "no more");
        let (name, data) = event(session.handle(Button::R, 8));
        assert_eq!((name, data["chip"].as_str()), ("screen.chip", Some("gb")));
        assert_eq!(session.top().unwrap().cursor, 0);
        assert_eq!(session.handle(Button::R, 8), None, "the last chip");
        match session.handle(Button::Y, 8) {
            Some(Effect::Invoke(action, call)) => {
                assert_eq!(action, "sort");
                assert_eq!(call.screen.as_deref(), Some("catalog"));
                assert_eq!(call.item.as_deref(), Some("g0"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(session.handle(Button::X, 8), None, "no X action");
        session.handle(Button::Select, 8);
        assert!(session.keyboard.is_some());
        session.handle(Button::A, 8);
        let (name, data) = event(session.handle(Button::Start, 8));
        assert_eq!((name, data["text"].as_str()), ("screen.search", Some("q")));
        assert!(session.keyboard.is_none());
    }

    #[test]
    fn details_move_between_buttons_pictures_and_patches() {
        let mut session = Session::new("demo", "Demo", false);
        session.apply(Message::Show(Box::new(detail())));
        session.apply(Message::Busy(Some("…".into())));
        session.handle(Button::R, 8);
        session.handle(Button::R, 8);
        match session.handle(Button::A, 8) {
            Some(Effect::Invoke(action, call)) => {
                assert_eq!(action, "do-page");
                assert_eq!(call.button.as_deref(), Some("page"));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            session.top().unwrap().shown().unwrap().to_str(),
            Some("/home/a.png")
        );
        session.handle(Button::X, 8);
        assert_eq!(
            session.top().unwrap().shown().unwrap().to_str(),
            Some("/home/b.png")
        );
        session.handle(Button::X, 8);
        assert_eq!(session.top().unwrap().picture, 0, "round again");
        session.apply(Message::Patch(Box::new(Patch {
            target: Some((
                Target::Button("get".into()),
                Change {
                    label: Some("Downloading".into()),
                    progress: Some(Some(42)),
                    ..Change::default()
                },
            )),
            ..Patch::default()
        })));
        assert!(session.busy.is_none(), "a patch ends the wait");
        let get = &session.top().unwrap().buttons[0];
        assert_eq!(
            (get.label.as_str(), get.progress),
            ("Downloading", Some(42))
        );
        session.apply(Message::Patch(Box::new(Patch {
            screen: Some("elsewhere".into()),
            footer: Some("x".into()),
            ..Patch::default()
        })));
        assert!(
            session.top().unwrap().footer.is_empty(),
            "another screen's patch"
        );
    }

    #[test]
    fn a_question_answers_once() {
        let mut session = Session::new("demo", "Demo", false);
        session.apply(Message::Show(Box::new(detail())));
        session.apply(Message::Confirm(Ask {
            id: "remove".into(),
            text: "Remove?".into(),
            ..Ask::default()
        }));
        assert_eq!(session.handle(Button::Down, 8), None);
        let (name, data) = event(session.handle(Button::A, 8));
        assert_eq!(name, "screen.answer");
        assert_eq!(data, json!({"id": "remove", "yes": true}));
        assert!(session.ask.is_none());
        session.apply(Message::Confirm(Ask {
            id: "again".into(),
            ..Ask::default()
        }));
        let (_, data) = event(session.handle(Button::B, 8));
        assert_eq!(data["yes"], false);
        assert_eq!(session.stack.len(), 1, "B answered, it did not go back");
    }
}
