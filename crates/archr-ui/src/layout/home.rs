//! The shelves (`[view.<id>.home]`, docs/temas.md, Prateleiras): a view
//! that declares `home` opens on tabs (Home, Library, Favorites) instead of
//! the systems list. Every key is optional and starts from the values
//! below, which draw the Ellegance layout on a 640×480 screen.
use super::{decor, element_rect, h, known, sub, Decor, Fit, Reader};
use crate::theme::Rect01;
use toml::{Table, Value};

/// The tabs at the top of the shelves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Home,
    Library,
    Favorites,
}
impl Tab {
    pub const ALL: [(&'static str, Tab); 3] = [
        ("home", Tab::Home),
        ("library", Tab::Library),
        ("favorites", Tab::Favorites),
    ];
    /// Its name in the catalog (`home.tab.<id>`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Home => "home.tab.home",
            Self::Library => "home.tab.library",
            Self::Favorites => "home.tab.favorites",
        }
    }
}

/// What a shelf of the Home tab holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShelfKind {
    /// The recent games, with the picture of where they were left.
    Continue,
    Systems,
    Favorites,
    /// The modules' apps (API 4).
    Apps,
    /// Favorites, Recent and the other collections.
    Collections,
}
impl ShelfKind {
    pub const ALL: [(&'static str, ShelfKind); 5] = [
        ("continue", ShelfKind::Continue),
        ("systems", ShelfKind::Systems),
        ("favorites", ShelfKind::Favorites),
        ("apps", ShelfKind::Apps),
        ("collections", ShelfKind::Collections),
    ];
    pub fn id(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, kind)| *kind == self)
            .map_or("", |(id, _)| id)
    }
}

/// One shelf's items: their size (`w` of the width, `h` of the height) and
/// corner radius (of 480), and whether a game shows how far it has gone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShelfStyle {
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub progress: bool,
}

/// The focused item's showcase above the shelves.
#[derive(Clone, Debug, PartialEq)]
pub struct Hero {
    /// Kicker, title, details and buttons.
    pub rect: Rect01,
    /// The item's picture beside them.
    pub art: Rect01,
    pub fit: Fit,
    /// The picture again behind the screen, blurred by `blur` (a size; 0:
    /// none) at `opacity`.
    pub blur: f32,
    pub opacity: f32,
    /// Buttons under the title: "play", "favorite", "more".
    pub actions: Vec<String>,
    /// The title's size (of 480).
    pub title: f32,
}

/// The Library tab: every system in a grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub rect: Rect01,
    pub columns: usize,
    /// Each card's height (of 480), the gap between cards and their radius.
    pub item: f32,
    pub gap: f32,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Home {
    pub tabs: Vec<Tab>,
    pub tabs_rect: Rect01,
    /// The tabs on the theme's glass instead of its surface.
    pub tabs_glass: bool,
    pub hero: Hero,
    pub order: Vec<ShelfKind>,
    /// The first shelf's top (of the height), the space between shelves and
    /// their titles' size (of 480).
    pub y: f32,
    pub gap: f32,
    pub title: f32,
    pub shelves: Vec<(ShelfKind, ShelfStyle)>,
    pub library: Grid,
    /// The Library and Favorites tabs' large title, and its size (of 480).
    pub heading: Rect01,
    pub heading_size: f32,
    pub decor: Vec<Decor>,
}

impl Default for Home {
    fn default() -> Self {
        let (x, y) = (|px: f32| px / 640., |px: f32| px / 480.);
        let shelf = |w, hh, radius| ShelfStyle {
            w: x(w),
            h: y(hh),
            radius: h(radius),
            progress: false,
        };
        Self {
            tabs: Tab::ALL.iter().map(|(_, tab)| *tab).collect(),
            tabs_rect: Rect01 {
                x: x(196.),
                y: y(14.),
                w: x(248.),
                h: y(34.),
            },
            tabs_glass: true,
            hero: Hero {
                rect: Rect01 {
                    x: x(32.),
                    y: y(74.),
                    w: x(300.),
                    h: y(150.),
                },
                art: Rect01 {
                    x: x(352.),
                    y: y(70.),
                    w: x(262.),
                    h: y(150.),
                },
                fit: Fit::Contain,
                blur: h(52.),
                opacity: 0.6,
                actions: ["play", "favorite", "more"].map(String::from).to_vec(),
                title: h(34.),
            },
            order: vec![ShelfKind::Continue, ShelfKind::Systems, ShelfKind::Apps],
            y: y(246.),
            gap: h(16.),
            title: h(13.),
            shelves: vec![
                (
                    ShelfKind::Continue,
                    ShelfStyle {
                        progress: true,
                        ..shelf(168., 94., 16.)
                    },
                ),
                (ShelfKind::Systems, shelf(92., 58., 14.)),
                (ShelfKind::Favorites, shelf(136., 84., 14.)),
                (ShelfKind::Apps, shelf(64., 64., 18.)),
                (ShelfKind::Collections, shelf(92., 58., 14.)),
            ],
            library: Grid {
                rect: Rect01 {
                    x: x(32.),
                    y: y(112.),
                    w: x(576.),
                    h: y(352.),
                },
                columns: 3,
                item: h(132.),
                gap: h(14.),
                radius: h(20.),
            },
            heading: Rect01 {
                x: x(32.),
                y: y(56.),
                w: x(576.),
                h: y(44.),
            },
            heading_size: h(30.),
            decor: Vec::new(),
        }
    }
}

impl Home {
    /// The style of the shelves of `kind`.
    pub fn shelf(&self, kind: ShelfKind) -> ShelfStyle {
        self.shelves
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(self.shelves[0].1, |(_, style)| *style)
    }
    /// `home = true` (the defaults), `false` (none) or a table over them.
    pub(super) fn overlay(slot: &mut Option<Self>, value: &Value, at: &str) {
        match value {
            Value::Boolean(false) => *slot = None,
            Value::Boolean(true) => {
                slot.get_or_insert_with(Self::default);
            }
            Value::Table(t) => slot.get_or_insert_with(Self::default).read(t, at),
            _ => log::warn!("Theme {at} must be true, false or a table; ignored"),
        }
    }
    fn read(&mut self, t: &Table, at: &str) {
        known(
            t,
            at,
            &["tabs", "hero", "shelves", "library", "heading", "decor"],
        );
        if let Some(tabs) = sub(t, at, "tabs") {
            let at = format!("{at}.tabs");
            let r = Reader {
                table: tabs,
                at: &at,
            };
            r.known(&["items", "rect", "style"]);
            if let Some(items) = names(tabs, &at, "items", &Tab::ALL) {
                self.tabs = items;
            }
            self.tabs_rect = element_rect(tabs, &at, "rect", self.tabs_rect);
            self.tabs_glass = r.pick(
                "style",
                self.tabs_glass,
                &[("glass", true), ("plain", false)],
            );
        }
        if let Some(hero) = sub(t, at, "hero") {
            let at = format!("{at}.hero");
            let r = Reader {
                table: hero,
                at: &at,
            };
            r.known(&["rect", "art", "backdrop", "actions", "title"]);
            self.hero.rect = element_rect(hero, &at, "rect", self.hero.rect);
            self.hero.art = element_rect(hero, &at, "art", self.hero.art);
            if let Some(art) = sub(hero, &at, "art") {
                let at = format!("{at}.art");
                let r = Reader {
                    table: art,
                    at: &at,
                };
                r.known(&["x", "y", "w", "h", "fit"]);
                self.hero.fit = r.pick(
                    "fit",
                    self.hero.fit,
                    &[("contain", Fit::Contain), ("cover", Fit::Cover)],
                );
            }
            match hero.get("backdrop") {
                None => {}
                Some(Value::Boolean(false)) => self.hero.opacity = 0.,
                Some(Value::Table(b)) => {
                    let at = format!("{at}.backdrop");
                    let r = Reader { table: b, at: &at };
                    r.known(&["blur", "opacity"]);
                    self.hero.blur = r.number("blur", self.hero.blur, 0., 0.3);
                    self.hero.opacity = r.opacity(self.hero.opacity);
                }
                Some(_) => log::warn!("Theme {at}.backdrop must be a table or false; ignored"),
            }
            match hero.get("actions") {
                None => {}
                Some(Value::Array(list)) => {
                    self.hero.actions = list
                        .iter()
                        .filter_map(|v| match v.as_str() {
                            Some(a @ ("play" | "favorite" | "more")) => Some(a.to_owned()),
                            _ => {
                                log::warn!(
                                    "Theme {at}.actions takes \"play\", \"favorite\" and \"more\"; {v} ignored"
                                );
                                None
                            }
                        })
                        .collect();
                }
                Some(_) => log::warn!("Theme {at}.actions must be a list; ignored"),
            }
            self.hero.title = r.number("title", self.hero.title, 0.02, 0.2);
        }
        if let Some(shelves) = sub(t, at, "shelves") {
            let at = format!("{at}.shelves");
            let mut keys = vec!["order", "y", "gap", "title"];
            keys.extend(ShelfKind::ALL.iter().map(|(id, _)| *id));
            known(shelves, &at, &keys);
            let r = Reader {
                table: shelves,
                at: &at,
            };
            if let Some(order) = names(shelves, &at, "order", &ShelfKind::ALL) {
                self.order = order;
            }
            self.y = r.number("y", self.y, 0., 1.);
            self.gap = r.number("gap", self.gap, 0., 0.3);
            self.title = r.number("title", self.title, 0.01, 0.1);
            for (id, kind) in ShelfKind::ALL {
                let Some(own) = sub(shelves, &at, id) else {
                    continue;
                };
                let at = format!("{at}.{id}");
                known(own, &at, &["item", "progress"]);
                let mut style = self.shelf(kind);
                if let Some(item) = sub(own, &at, "item") {
                    let at = format!("{at}.item");
                    let r = Reader {
                        table: item,
                        at: &at,
                    };
                    r.known(&["w", "h", "radius"]);
                    style.w = r.number("w", style.w, 0.02, 1.);
                    style.h = r.number("h", style.h, 0.02, 1.);
                    style.radius = r.number("radius", style.radius, 0., 0.5);
                }
                style.progress = Reader {
                    table: own,
                    at: &at,
                }
                .flag("progress", style.progress);
                if let Some(slot) = self.shelves.iter_mut().find(|(k, _)| *k == kind) {
                    slot.1 = style;
                }
            }
        }
        if let Some(library) = sub(t, at, "library") {
            let at = format!("{at}.library");
            let r = Reader {
                table: library,
                at: &at,
            };
            r.known(&["rect", "columns", "item", "gap", "radius"]);
            let grid = &mut self.library;
            grid.rect = element_rect(library, &at, "rect", grid.rect);
            grid.columns = r.number("columns", grid.columns as f32, 1., 8.) as usize;
            grid.item = r.number("item", grid.item, 0.04, 1.);
            grid.gap = r.number("gap", grid.gap, 0., 0.2);
            grid.radius = r.number("radius", grid.radius, 0., 0.5);
        }
        if let Some(heading) = sub(t, at, "heading") {
            let at = format!("{at}.heading");
            let r = Reader {
                table: heading,
                at: &at,
            };
            r.known(&["x", "y", "w", "h", "size"]);
            self.heading = r.rect(self.heading);
            self.heading_size = r.number("size", self.heading_size, 0.02, 0.2);
        }
        match t.get("decor") {
            None => {}
            Some(Value::Array(items)) => {
                self.decor = items
                    .iter()
                    .enumerate()
                    .filter_map(|(n, item)| decor(item, &format!("{at}.decor[{n}]")))
                    .collect();
            }
            Some(_) => log::warn!("Theme {at}.decor must be a list of tables; ignored"),
        }
    }
}

/// A list of names among `options`, each wrong one warned about and left
/// out; None when the key is absent or not a list.
fn names<T: Copy + PartialEq>(
    table: &Table,
    at: &str,
    key: &str,
    options: &[(&str, T)],
) -> Option<Vec<T>> {
    let value = table.get(key)?;
    let Some(list) = value.as_array() else {
        log::warn!("Theme {at}.{key} must be a list; ignored");
        return None;
    };
    let mut found = Vec::new();
    for item in list {
        match item
            .as_str()
            .and_then(|name| options.iter().find(|(n, _)| *n == name))
        {
            Some((_, choice)) if !found.contains(choice) => found.push(*choice),
            Some(_) => log::warn!("Theme {at}.{key} repeats {item}; ignored"),
            None => {
                let names: Vec<_> = options.iter().map(|(n, _)| *n).collect();
                log::warn!("Theme {at}.{key}: {item} is not one of {names:?}; ignored");
            }
        }
    }
    (!found.is_empty()).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(source: &str) -> Option<Home> {
        let mut slot = None;
        Home::overlay(
            &mut slot,
            &toml::from_str::<Value>(source).unwrap()["home"],
            "test",
        );
        slot
    }
    #[test]
    fn home_is_a_switch_or_a_table_over_the_defaults() {
        assert_eq!(home("home = true"), Some(Home::default()));
        assert_eq!(home("home = false"), None);
        assert_eq!(home("home = 3"), None, "warned and left as it was");
        let own = home(
            "[home.tabs]\n\
             items = ['home', 'favorites', 'home', 'shop']\n\
             rect = { x = 0.3, y = 0.02, w = 0.4, h = 0.08 }\n\
             style = 'plain'\n\
             [home.hero]\n\
             backdrop = { blur = 0.05, opacity = 0.4 }\n\
             actions = ['play', 'dance']\n\
             art = { x = 0.5, y = 0.1, w = 0.4, h = 0.3, fit = 'cover' }\n\
             [home.shelves]\n\
             order = ['systems', 'continue']\n\
             y = 0.6\n\
             [home.shelves.systems]\n\
             item = { w = 0.2, h = 0.15 }\n\
             [home.library]\n\
             columns = 4",
        )
        .unwrap();
        assert_eq!(
            own.tabs,
            [Tab::Home, Tab::Favorites],
            "repeats and unknown left out"
        );
        assert_eq!((own.tabs_rect.x, own.tabs_glass), (0.3, false));
        assert_eq!((own.hero.blur, own.hero.opacity), (0.05, 0.4));
        assert_eq!(own.hero.actions, ["play"]);
        assert_eq!((own.hero.art.w, own.hero.fit), (0.4, Fit::Cover));
        assert_eq!(own.order, [ShelfKind::Systems, ShelfKind::Continue]);
        assert_eq!(own.y, 0.6);
        let systems = own.shelf(ShelfKind::Systems);
        assert_eq!((systems.w, systems.h), (0.2, 0.15));
        assert_eq!(
            systems.radius,
            Home::default().shelf(ShelfKind::Systems).radius
        );
        assert!(own.shelf(ShelfKind::Continue).progress);
        assert_eq!(own.library.columns, 4);
    }
}
