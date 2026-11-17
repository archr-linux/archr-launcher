//! The shelves (docs/temas.md, Prateleiras): the tabs, the focused item's
//! showcase over its blurred picture, the rows of items, the Library grid
//! and the Favorites tab; and the games grid with its strip of systems.
//! Everything is placed by the view's `home` and drawn with the style
//! primitives (glass, lift, the cached blur, gradients, the bold face).
use super::{details::Art, system_art::short_name, system_title, Frame, Ui};
use crate::{
    appearance::mix,
    icons::Glyph,
    layout::{Home, Layout, ShelfKind, ShelfStyle, Tab},
    primitives::{crop, shade},
    state::{App, DataSource, Item, Screen, Shelf},
    text::{Align, Face},
    theme::{FocusStyle, Gradient, Rgba, Theme},
};
use sdl2::{rect::Rect, render::Texture};
use std::path::PathBuf;

/// Left and right margin of the rows, and the space between items, in
/// reference pixels.
const GAP: f32 = 12.;
/// A name under a card.
const LABEL: f32 = 18.;

fn alpha(c: Rgba, a: u8) -> Rgba {
    Rgba { a, ..c }
}
fn vertical(from: Rgba, to: Rgba) -> Gradient {
    Gradient {
        from,
        to,
        angle: 90.,
    }
}
/// The catalog key of a shelf's title.
fn shelf_title(kind: ShelfKind) -> &'static str {
    match kind {
        ShelfKind::Continue => "home.shelf.continue",
        ShelfKind::Systems => "home.shelf.systems",
        ShelfKind::Favorites => "home.shelf.favorites",
        ShelfKind::Apps => "home.shelf.apps",
        ShelfKind::Collections => "home.shelf.collections",
    }
}

impl<'t> Ui<'_, 't> {
    /// The shelves screen: `tab`, with `shelf` and `item` focused.
    pub(super) fn home(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        (home, at): (&Home, (Tab, usize, usize)),
    ) -> Result<(), String> {
        let (tab, shelf, item) = at;
        let shelves = app.shelves(tab, data);
        let shelf = shelf.min(shelves.len().saturating_sub(1));
        let item = shelves
            .get(shelf)
            .map_or(0, |s| item.min(s.items.len().saturating_sub(1)));
        let focused = shelves.get(shelf).and_then(|s| s.items.get(item));
        if let Some(art) = &mut self.art {
            let names: Vec<&str> = shelves
                .iter()
                .flat_map(|s| s.items.iter().take(12))
                .filter_map(|i| match i {
                    Item::System(name) | Item::App(name) => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            art.want(&names);
        }
        if tab == Tab::Home {
            if let Some(focused) = focused {
                self.home_backdrop(app, data, theme, art, home, focused)?;
            }
        }
        self.home_tabs(home, tab, theme)?;
        self.status_header(theme)?;
        match tab {
            Tab::Home => {
                if let Some(focused) = focused {
                    self.hero(app, data, theme, art, home, focused)?;
                }
                let top = (home.y * self.frame.height as f32).round() as i32;
                self.shelf_rows(app, data, theme, art, home, (&shelves, shelf, item), top)?;
            }
            Tab::Library => {
                let count = shelves.first().map_or(0, |s| s.items.len());
                let note = self.catalog.plural(
                    "home.systems_count",
                    count as f64,
                    &[("count", &count.to_string())],
                );
                self.heading(home, Tab::Library, &note, theme)?;
                if let Some(systems) = shelves.first() {
                    self.library(data, theme, home, (systems, item))?;
                }
            }
            Tab::Favorites => {
                let count: usize = shelves.iter().map(|s| s.items.len()).sum();
                let note = self.catalog.plural(
                    "games.count",
                    count as f64,
                    &[("count", &count.to_string())],
                );
                self.heading(home, Tab::Favorites, &note, theme)?;
                let top = self.area(home.heading).bottom() + self.px(12.);
                self.shelf_rows(app, data, theme, art, home, (&shelves, shelf, item), top)?;
            }
        }
        if shelves.is_empty() {
            let text = self.catalog.text("home.empty");
            let rect = self.area(home.library.rect);
            self.label_aligned(
                &text,
                (Face::Regular, 15., theme.muted),
                rect,
                Align::Center,
            )?;
        }
        Ok(())
    }

    /// The tabs in their capsule: the theme's glass (or surface), the
    /// current one lit.
    fn home_tabs(&mut self, home: &Home, tab: Tab, theme: &Theme) -> Result<(), String> {
        let rect = self.area(home.tabs_rect);
        let radius = rect.height() as f32 / 2.;
        if home.tabs_glass {
            self.glass(rect, radius, &theme.glass_or_default())?;
        } else {
            self.rounded(rect, radius, theme.surface, Some(theme.border))?;
        }
        let count = home.tabs.len().max(1) as i32;
        let pad = self.px(3.);
        let inner_w = rect.width() as i32 - 2 * pad;
        let h = (rect.height() as i32 - 2 * pad).max(1) as u32;
        for (n, own) in home.tabs.iter().enumerate() {
            let n = n as i32;
            let x = rect.x() + pad + inner_w * n / count;
            let w = (inner_w * (n + 1) / count - inner_w * n / count).max(1) as u32;
            let cell = Rect::new(x, rect.y() + pad, w, h);
            let on = *own == tab;
            if on {
                self.rounded(cell, h as f32 / 2., alpha(theme.text, 0x33), None)?;
            }
            let text = self.catalog.text(own.key());
            let style = if on {
                (Face::Semibold, 12., theme.text)
            } else {
                (Face::Regular, 12., theme.text_secondary)
            };
            self.label_aligned(&text, style, cell, Align::Center)?;
        }
        Ok(())
    }

    /// The Library's and the Favorites' large title, a note at its right.
    fn heading(&mut self, home: &Home, tab: Tab, note: &str, theme: &Theme) -> Result<(), String> {
        let rect = self.area(home.heading);
        let text = self.catalog.text(tab.key());
        let size = home.heading_size * 480.;
        self.label(&text, (Face::Bold, size, theme.text), rect)?;
        self.label_aligned(
            note,
            (Face::Regular, 13., theme.text_secondary),
            rect,
            Align::Right,
        )
    }

    /// The focused item's picture, blurred once and kept, over the top of
    /// the screen, then veils so the text stays legible.
    fn home_backdrop(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        home: &Home,
        focused: &Item,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        if home.hero.opacity > 0. && home.hero.blur > 0. {
            if let Some((path, texture)) = picture_of(app, data, art, focused) {
                let steps = super::material::halvings(self.frame.size(home.hero.blur));
                let size = texture.query();
                let part = crop((size.width, size.height), (width, height));
                let src = Rect::new(
                    (part[0] * size.width as f32) as i32,
                    (part[1] * size.height as f32) as i32,
                    (part[2] * size.width as f32).round().max(1.) as u32,
                    (part[3] * size.height as f32).round().max(1.) as u32,
                );
                let made = self.blurred(&path, steps, |ui| {
                    ui.canvas.copy(texture, src, None).map(|()| true)
                });
                if let Some(index) = made {
                    let a = (home.hero.opacity * 255.).round() as u8;
                    self.draw_blurred(index, Rect::new(0, 0, width, height), a)?;
                }
            }
        }
        // Dark at the text's side, clear at the picture's; then the shelves
        // fade into the background.
        let bg = theme.background;
        let whole = Rect::new(0, 0, width, height);
        shade(
            &mut self.canvas,
            whole,
            &Gradient {
                from: alpha(bg, 240),
                to: alpha(bg, 77),
                angle: 0.,
            },
        )?;
        let top = (height as f32 * 0.42) as i32;
        shade(
            &mut self.canvas,
            Rect::new(0, top, width, height - top as u32),
            &vertical(alpha(bg, 0), bg),
        )
    }

    /// Kicker, title, details and buttons of the focused item, and its
    /// picture beside them.
    fn hero(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        home: &Home,
        focused: &Item,
    ) -> Result<(), String> {
        let rect = self.area(home.hero.rect);
        let catalog = &self.catalog;
        let game_line = |system: &str, game: usize| {
            let Some(g) = data.games(system).get(game) else {
                return String::new();
            };
            let mut parts = Vec::new();
            if let Some(year) = g.year {
                parts.push(year.to_string());
            }
            if let Some(players) = &g.players {
                parts.push(catalog.players(players));
            }
            if let Some(at) = g.last_played {
                parts.push(catalog.relative_time(at, self.frame.unix));
            }
            parts.join(" · ")
        };
        let (kicker, title, line, button) = match focused {
            Item::Recent(n) => {
                let Some(recent) = app.recent.get(*n) else {
                    return Ok(());
                };
                let system = system_title(&recent.system, data, catalog);
                (
                    catalog.format("home.kicker_continue", &[("system", &system)]),
                    recent.name.clone(),
                    game_line(&recent.system, recent.game),
                    catalog.text("home.continue"),
                )
            }
            Item::Game { system, game } => (
                system_title(system, data, catalog),
                data.games(system)
                    .get(*game)
                    .map_or_else(String::new, |g| g.name.clone()),
                game_line(system, *game),
                catalog.text("hint.play"),
            ),
            Item::System(name) => {
                let count = data
                    .systems()
                    .iter()
                    .find(|s| s.name == *name)
                    .map_or_else(|| data.collection_len(name), |s| s.game_count as usize);
                (
                    catalog.text("home.kicker_system"),
                    system_title(name, data, catalog),
                    catalog.plural(
                        "games.count",
                        count as f64,
                        &[("count", &count.to_string())],
                    ),
                    catalog.text("home.open"),
                )
            }
            Item::App(row) => {
                let found = data.apps().iter().find(|a| a.key == *row);
                (
                    catalog.text("home.kicker_app"),
                    found.map_or_else(|| row.clone(), |a| a.name.clone()),
                    found.map_or_else(String::new, |a| a.summary.clone()),
                    catalog.text("home.open"),
                )
            }
        };
        let mut y = rect.y();
        let line_h = |ui: &Self, size: f32| (ui.frame.size(size / 480.) * 1.3).round() as i32;
        self.label(
            &kicker.to_uppercase(),
            (Face::Semibold, 10., theme.text_secondary),
            Rect::new(rect.x(), y, rect.width(), line_h(self, 10.) as u32),
        )?;
        y += line_h(self, 10.) + self.px(4.);
        let big = home.hero.title * 480.;
        let floor = self.frame.min_size().max(18.);
        let size = self
            .shrink(&title, Face::Bold, big, floor, rect.width())?
            .unwrap_or(floor);
        let lines = self.wrap(&title, Face::Bold, size, rect.width())?;
        let step = (self.frame.size(size / 480.) * 1.05).round() as i32;
        for text in lines.iter().take(2) {
            self.label(
                text,
                (Face::Bold, size, theme.text),
                Rect::new(rect.x(), y, rect.width(), step as u32),
            )?;
            y += step;
        }
        y += self.px(6.);
        for text in self
            .wrap(&line, Face::Regular, 13., rect.width())?
            .iter()
            .take(2)
        {
            self.label(
                text,
                (Face::Regular, 13., theme.text_secondary),
                Rect::new(rect.x(), y, rect.width(), line_h(self, 13.) as u32),
            )?;
            y += line_h(self, 13.);
        }
        y += self.px(12.);
        // Favorite and More are for games; a system or an app only opens.
        let game = matches!(focused, Item::Recent(_) | Item::Game { .. });
        self.hero_buttons(home, &button, game, (rect.x(), y), theme)?;
        // The picture beside the text.
        let picture = self.area(home.hero.art);
        match focused {
            Item::System(name) => {
                let path = self.art.as_ref().and_then(|a| a.key(name));
                if !self.console_image(picture, path.as_ref(), 255) {
                    self.generated_card(picture, name, theme)?;
                }
            }
            Item::App(row) => {
                let side = picture.width().min(picture.height());
                let tile = Rect::new(
                    picture.center().x() - side as i32 / 2,
                    picture.y(),
                    side,
                    side,
                );
                self.app_tile(tile, row, theme)?;
            }
            _ => {
                if let Some((_, texture)) = picture_of(app, data, art, focused) {
                    let size = texture.query();
                    let scale = (picture.width() as f32 / size.width as f32)
                        .min(picture.height() as f32 / size.height as f32);
                    let (w, h) = (
                        (size.width as f32 * scale).round().max(1.) as u32,
                        (size.height as f32 * scale).round().max(1.) as u32,
                    );
                    let dst = Rect::new(
                        picture.center().x() - w as i32 / 2,
                        picture.center().y() - h as i32 / 2,
                        w,
                        h,
                    );
                    let radius = self.px(12.) as f32;
                    self.cast_shadow(dst, radius)?;
                    self.geometry.cover(
                        &mut self.canvas,
                        texture,
                        dst,
                        radius,
                        255,
                        [0., 0., 1., 1.],
                    )?;
                }
            }
        }
        Ok(())
    }

    /// The first action as a light pill with its text, the others round.
    fn hero_buttons(
        &mut self,
        home: &Home,
        first: &str,
        game: bool,
        (x, y): (i32, i32),
        theme: &Theme,
    ) -> Result<(), String> {
        let h = self.px(34.);
        let mut x = x;
        for action in home.hero.actions.iter().filter(|a| game || *a == "play") {
            if action == "play" {
                let text_w = self.measure(first, Face::Semibold, 14.)? as i32;
                let icon = self.px(12.);
                let w = self.px(14.) + icon + self.px(7.) + text_w + self.px(18.);
                let pill = Rect::new(x, y, w as u32, h as u32);
                self.cast_shadow(pill, h as f32 / 2.)?;
                self.rounded(pill, h as f32 / 2., theme.text, None)?;
                let glyph = Rect::new(
                    x + self.px(14.),
                    y + (h - icon) / 2,
                    icon as u32,
                    icon as u32,
                );
                self.icon(Glyph::Play, glyph, theme.background)?;
                self.label(
                    first,
                    (Face::Semibold, 14., theme.background),
                    Rect::new(
                        glyph.right() + self.px(7.),
                        y,
                        text_w.max(1) as u32,
                        h as u32,
                    ),
                )?;
                x += w + self.px(8.);
                continue;
            }
            let round = Rect::new(x, y, h as u32, h as u32);
            self.rounded(
                round,
                h as f32 / 2.,
                alpha(theme.text, 0x1A),
                Some(alpha(theme.text, 0x24)),
            )?;
            if action == "favorite" {
                let side = self.px(15.);
                let glyph = Rect::new(
                    x + (h - side) / 2,
                    y + (h - side) / 2,
                    side as u32,
                    side as u32,
                );
                self.icon(Glyph::Star, glyph, theme.text)?;
            } else {
                let dot = self.px(3.2).max(2);
                for k in -1..=1 {
                    let cx = x + h / 2 + k * self.px(6.);
                    self.rounded(
                        Rect::new(cx - dot / 2, y + h / 2 - dot / 2, dot as u32, dot as u32),
                        dot as f32 / 2.,
                        theme.text,
                        None,
                    )?;
                }
            }
            x += h + self.px(8.);
        }
        Ok(())
    }

    /// The shelves from `top` down, scrolled so the focused one shows whole,
    /// each scrolled so its focused item shows.
    #[allow(clippy::too_many_arguments)]
    fn shelf_rows(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        home: &Home,
        (shelves, focus_shelf, focus_item): (&[Shelf], usize, usize),
        top: i32,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let left = self.area(home.hero.rect).x();
        let title_h = (self.frame.size(home.title) * 1.8).round() as i32;
        let gap = self.frame.size(home.gap).round() as i32;
        let heights: Vec<i32> = shelves
            .iter()
            .map(|s| {
                let style = home.shelf(s.kind);
                let label = if labelled(s) { self.px(LABEL) } else { 0 };
                title_h + (style.h * height as f32).round() as i32 + label
            })
            .collect();
        // The first shelf drawn: as far down as the focused one still fits.
        let mut first = 0;
        let bottom = |first: usize| {
            top + heights[first..=focus_shelf.min(heights.len().saturating_sub(1))]
                .iter()
                .map(|h| h + gap)
                .sum::<i32>()
        };
        while first < focus_shelf && bottom(first) > height as i32 {
            first += 1;
        }
        let mut y = top;
        for (n, shelf) in shelves.iter().enumerate().skip(first) {
            if y >= height as i32 {
                break;
            }
            let title = match &shelf.system {
                Some(system) => system_title(system, data, &self.catalog),
                None => self.catalog.text(shelf_title(shelf.kind)),
            };
            let ink = if n == focus_shelf {
                theme.text
            } else {
                theme.text_secondary
            };
            self.label(
                &title,
                (Face::Semibold, home.title * 480., ink),
                Rect::new(
                    left,
                    y,
                    width.saturating_sub(left as u32 * 2).max(1),
                    title_h as u32,
                ),
            )?;
            let style = home.shelf(shelf.kind);
            let (w, h) = (
                (style.w * width as f32).round() as i32,
                (style.h * height as f32).round() as i32,
            );
            let step = w + self.px(GAP);
            let focused = (n == focus_shelf).then_some(focus_item);
            // Scrolled so the focused item ends inside the right margin.
            let at = focused.unwrap_or(0) as i32;
            let shift = (left + at * step + w - (width as i32 - left)).max(0);
            let row_y = y + title_h;
            for (k, item) in shelf.items.iter().enumerate() {
                let x = left + k as i32 * step - shift;
                if x + w < 0 {
                    continue;
                }
                if x > width as i32 {
                    break;
                }
                let rect = Rect::new(x, row_y, w.max(1) as u32, h.max(1) as u32);
                let on = focused == Some(k);
                self.shelf_item(
                    app,
                    data,
                    theme,
                    art,
                    (item, style, labelled(shelf)),
                    rect,
                    on,
                )?;
            }
            y += heights[n] + gap;
        }
        Ok(())
    }

    /// One item of a shelf in `rect`, lifted and ringed when `on`.
    #[allow(clippy::too_many_arguments)]
    fn shelf_item(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        (item, style, label): (&Item, ShelfStyle, bool),
        rect: Rect,
        on: bool,
    ) -> Result<(), String> {
        let radius = self.frame.size(style.radius);
        let (rect, radius) = self.focus_box(rect, radius, on, theme)?;
        let name = match item {
            Item::Recent(n) => app.recent.get(*n).map(|r| r.name.clone()),
            Item::Game { system, game } => data.games(system).get(*game).map(|g| g.name.clone()),
            _ => None,
        };
        match item {
            Item::System(system) => {
                self.system_card(rect, radius, system, theme, None)?;
            }
            Item::App(row) => self.app_tile(rect, row, theme)?,
            Item::Recent(_) | Item::Game { .. } => {
                let a = if on { 255 } else { 210 };
                match picture_of(app, data, art, item) {
                    Some((_, texture)) => {
                        let size = texture.query();
                        let part = crop((size.width, size.height), (rect.width(), rect.height()));
                        self.geometry
                            .cover(&mut self.canvas, texture, rect, radius, a, part)?;
                    }
                    None => {
                        self.geometry.shaded(
                            &mut self.canvas,
                            rect,
                            radius,
                            &vertical(theme.surface_elevated, theme.surface),
                            None,
                        )?;
                        let side = (rect.height() as f32 * 0.3) as u32;
                        let icon = Rect::new(
                            rect.center().x() - side as i32 / 2,
                            rect.center().y() - side as i32 / 2,
                            side,
                            side,
                        );
                        self.icon(Glyph::Games, icon, theme.dim_text)?;
                    }
                }
                if !label {
                    // The name over the picture's darkened foot.
                    self.geometry.shaded(
                        &mut self.canvas,
                        rect,
                        radius,
                        &vertical(
                            Rgba {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 0,
                            },
                            Rgba {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 180,
                            },
                        ),
                        None,
                    )?;
                    if let Some(name) = &name {
                        let pad = self.px(10.);
                        let text = Rect::new(
                            rect.x() + pad,
                            rect.bottom() - self.px(30.),
                            rect.width().saturating_sub(2 * pad as u32).max(1),
                            self.px(18.) as u32,
                        );
                        self.label(name, (Face::Semibold, 12., theme.text), text)?;
                    }
                }
            }
        }
        if on {
            self.focus_ring(rect, radius, theme)?;
        }
        if let (true, Some(name)) = (label, &name) {
            let style = if on {
                (Face::Semibold, 11., theme.text)
            } else {
                (Face::Regular, 11., theme.text_secondary)
            };
            let below = Rect::new(
                rect.x(),
                rect.bottom() + self.px(4.),
                rect.width(),
                self.px(16.) as u32,
            );
            self.label(name, style, below)?;
        }
        Ok(())
    }

    /// `rect` for a focused item: grown and raised with `focus = "lift"`;
    /// the box and radius to draw it in.
    fn focus_box(
        &mut self,
        rect: Rect,
        radius: f32,
        on: bool,
        theme: &Theme,
    ) -> Result<(Rect, f32), String> {
        if !on || theme.focus != FocusStyle::Lift {
            return Ok((rect, radius));
        }
        let grown = self.lift_focus(rect, radius, 1., theme)?;
        Ok((
            grown,
            radius * grown.width() as f32 / rect.width().max(1) as f32,
        ))
    }

    /// A system as a card: the theme's light gradient, the console picture
    /// and, with `caption` (count), its name and count at the foot.
    fn system_card(
        &mut self,
        rect: Rect,
        radius: f32,
        system: &str,
        theme: &Theme,
        caption: Option<(String, String)>,
    ) -> Result<(), String> {
        self.geometry.shaded(
            &mut self.canvas,
            rect,
            radius,
            &vertical(alpha(theme.text, 0x1C), alpha(theme.text, 0x09)),
            Some(alpha(theme.text, 0x1A)),
        )?;
        let foot = if caption.is_some() { self.px(30.) } else { 0 };
        let pad = (rect.width().min(rect.height()) as f32 * 0.12).round() as i32;
        let inner = Rect::new(
            rect.x() + pad,
            rect.y() + pad,
            rect.width().saturating_sub(2 * pad as u32).max(1),
            (rect.height() as i32 - 2 * pad - foot).max(1) as u32,
        );
        let path = self.art.as_ref().and_then(|a| a.key(system));
        if !self.console_image(inner, path.as_ref(), 255) {
            let text = short_name(system);
            self.label_aligned(
                &text,
                (Face::Semibold, 13., theme.text_secondary),
                inner,
                Align::Center,
            )?;
        }
        if let Some((name, count)) = caption {
            let pad = self.px(14.);
            let line = Rect::new(
                rect.x() + pad,
                rect.bottom() - self.px(28.),
                rect.width().saturating_sub(2 * pad as u32).max(1),
                self.px(18.) as u32,
            );
            let count_w = self.measure(&count, Face::Regular, 11.)?;
            self.label_aligned(
                &count,
                (Face::Regular, 11., theme.text_secondary),
                line,
                Align::Right,
            )?;
            let name_box = Rect::new(
                line.x(),
                line.y(),
                line.width()
                    .saturating_sub(count_w + self.px(8.) as u32)
                    .max(1),
                line.height(),
            );
            self.label(&name, (Face::Semibold, 13., theme.text), name_box)?;
        }
        Ok(())
    }

    /// A module's app: its icon in white on the accent.
    fn app_tile(&mut self, rect: Rect, row: &str, theme: &Theme) -> Result<(), String> {
        let radius = rect.height() as f32 * 0.28;
        self.geometry.shaded(
            &mut self.canvas,
            rect,
            radius,
            &Gradient {
                from: theme.highlight_bg,
                to: mix(theme.highlight_bg, theme.background, 0.3),
                angle: 55.,
            },
            None,
        )?;
        let side = (rect.width().min(rect.height()) as f32 * 0.45) as u32;
        let icon = Rect::new(
            rect.center().x() - side as i32 / 2,
            rect.center().y() - side as i32 / 2,
            side,
            side,
        );
        let svg = self.art.as_ref().and_then(|a| a.icon(row));
        self.module_icon((svg.as_deref(), ""), icon, theme.highlight_text)
    }

    /// Every system in the Library's grid, scrolled to the focused row.
    fn library(
        &mut self,
        data: &dyn DataSource,
        theme: &Theme,
        home: &Home,
        (systems, focus): (&Shelf, usize),
    ) -> Result<(), String> {
        let area = self.area(home.library.rect);
        let columns = home.library.columns.max(1) as i32;
        let gap = self.frame.size(home.library.gap).round() as i32;
        let w = (area.width() as i32 - gap * (columns - 1)) / columns;
        let h = self.frame.size(home.library.item).round() as i32;
        let radius = self.frame.size(home.library.radius);
        let pitch = h + gap;
        let rows_shown = ((area.height() as i32 + gap) / pitch).max(1);
        let focus_row = focus as i32 / columns;
        let first = (focus_row - rows_shown + 1).max(0);
        self.canvas.set_clip_rect(Rect::new(
            0,
            area.y() - self.px(10.),
            self.frame.width,
            area.height() + self.px(20.) as u32,
        ));
        for (n, item) in systems.items.iter().enumerate() {
            let (row, col) = (n as i32 / columns, n as i32 % columns);
            if row < first || row > first + rows_shown {
                continue;
            }
            let Item::System(system) = item else { continue };
            let rect = Rect::new(
                area.x() + col * (w + gap),
                area.y() + (row - first) * pitch,
                w.max(1) as u32,
                h.max(1) as u32,
            );
            let on = n == focus;
            let (rect, r) = self.focus_box(rect, radius, on, theme)?;
            let count = data
                .systems()
                .iter()
                .find(|s| s.name == *system)
                .map_or(0, |s| s.game_count);
            let name = system_title(system, data, &self.catalog);
            self.system_card(rect, r, system, theme, Some((name, count.to_string())))?;
            if on {
                self.focus_ring(rect, r, theme)?;
            }
        }
        self.canvas.set_clip_rect(None);
        Ok(())
    }

    /// A games list laid out as a grid (`mark = "grid"`): covers with their
    /// names under them, and the strip of systems when the layout has one.
    pub(super) fn games_grid(
        &mut self,
        (system, folder): (&str, &str),
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        layout: &Layout,
    ) -> Result<(), String> {
        if let Some(strip) = layout.strip {
            self.strip(self.area(strip), system, data, theme)?;
        }
        let list = &layout.list;
        let area = self.area(list.rect);
        let columns = list.columns.max(1) as i32;
        let gap = self.frame.size(list.gap).round().max(self.px(8.) as f32) as i32;
        let w = (area.width() as i32 - gap * (columns - 1)) / columns;
        let h = self.frame.size(list.row).round() as i32;
        let label = (self.frame.size(list.font) * 1.6).round() as i32;
        let pitch = h + label + gap;
        let radius = self.frame.size(list.radius);
        let len = App::list_len(data, system, folder);
        let rows_shown = ((area.height() as i32 + gap) / pitch).max(1);
        let focus_row = cursor as i32 / columns;
        let first = (focus_row - rows_shown + 1).max(0);
        let level = App::level(data, system, folder);
        let games = data.games(system);
        // Room for the lift and the ring, nothing below the list.
        let reach = self.px(8.);
        self.canvas.set_clip_rect(Rect::new(
            area.x() - reach,
            area.y() - reach,
            area.width() + 2 * reach as u32,
            area.height() + reach as u32,
        ));
        for n in (first * columns) as usize..len.min(((first + rows_shown + 1) * columns) as usize)
        {
            let (row, col) = (n as i32 / columns - first, n as i32 % columns);
            let rect = Rect::new(
                area.x() + col * (w + gap),
                area.y() + row * pitch,
                w.max(1) as u32,
                h.max(1) as u32,
            );
            if rect.y() > self.frame.height as i32 {
                break;
            }
            let on = n == cursor;
            let (rect, r) = self.focus_box(rect, radius, on, theme)?;
            let folder_name = level.as_ref().and_then(|l| l.folder(n)).map(str::to_owned);
            let game = App::list_game(data, system, folder, n).and_then(|g| games.get(g));
            let cover = game
                .and_then(|g| g.image.as_ref())
                .and_then(|path| art.covers.and_then(|c| c.peek(path)));
            match cover {
                Some(texture) => {
                    let size = texture.query();
                    let part = crop((size.width, size.height), (rect.width(), rect.height()));
                    self.geometry
                        .cover(&mut self.canvas, texture, rect, r, 255, part)?;
                }
                None => {
                    self.geometry.shaded(
                        &mut self.canvas,
                        rect,
                        r,
                        &vertical(theme.surface_elevated, theme.surface),
                        None,
                    )?;
                    let side = (rect.height() as f32 * 0.3) as u32;
                    let icon = Rect::new(
                        rect.center().x() - side as i32 / 2,
                        rect.center().y() - side as i32 / 2,
                        side,
                        side,
                    );
                    let glyph = if folder_name.is_some() {
                        Glyph::Folder
                    } else {
                        Glyph::Games
                    };
                    self.icon(glyph, icon, theme.dim_text)?;
                }
            }
            if on {
                self.focus_ring(rect, r, theme)?;
            }
            let name = folder_name.or_else(|| game.map(|g| g.name.clone()));
            if let Some(name) = name {
                let size = list.font * 480.;
                let style = if on {
                    (Face::Semibold, size, theme.text)
                } else {
                    (Face::Regular, size, theme.text_secondary)
                };
                let below = Rect::new(
                    rect.x(),
                    rect.bottom() + self.px(4.),
                    rect.width(),
                    label.max(1) as u32,
                );
                self.label(&name, style, below)?;
            }
        }
        self.canvas.set_clip_rect(None);
        Ok(())
    }

    /// The systems as chips in a row, the current one lit, with the L1 and
    /// R1 marks at its ends.
    fn strip(
        &mut self,
        area: Rect,
        current: &str,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let systems: Vec<String> = App::system_rows(data)
            .into_iter()
            .filter(|row| !row.starts_with('@'))
            .collect();
        let h = area.height().min(self.px(26.) as u32);
        let y = area.y() + (area.height() as i32 - h as i32) / 2;
        let pad = self.px(12.);
        let gap = self.px(6.);
        let mark = |ui: &mut Self, text: &str, x: i32| -> Result<i32, String> {
            let w = ui.measure(text, Face::Semibold, 10.)? as i32 + ui.px(10.);
            let rect = Rect::new(
                x,
                y + (h as i32 - ui.px(16.)) / 2,
                w as u32,
                ui.px(16.) as u32,
            );
            ui.rounded(
                rect,
                ui.px(5.) as f32,
                alpha(theme.text, 0),
                Some(alpha(theme.text, 0x59)),
            )?;
            ui.label_aligned(
                text,
                (Face::Semibold, 10., theme.text_secondary),
                rect,
                Align::Center,
            )?;
            Ok(w)
        };
        let mut x = area.x();
        x += mark(self, "L1", x)? + gap;
        let widths: Vec<i32> = systems
            .iter()
            .map(|s| {
                let name = system_title(s, data, &self.catalog);
                self.measure(&name, Face::Semibold, 12.)
                    .map(|w| w as i32 + 2 * pad)
            })
            .collect::<Result<_, _>>()?;
        let at = systems.iter().position(|s| s == current).unwrap_or(0);
        let start: i32 = widths[..at].iter().map(|w| w + gap).sum();
        let room = area.right() - x - self.px(40.);
        let shift = (start + widths.get(at).copied().unwrap_or(0) - room).max(0);
        self.canvas
            .set_clip_rect(Rect::new(x, area.y(), (room.max(1)) as u32, area.height()));
        let mut cx = x - shift;
        for (n, system) in systems.iter().enumerate() {
            let w = widths[n];
            if cx + w > x - gap && cx < x + room {
                let chip = Rect::new(cx, y, w as u32, h);
                let name = system_title(system, data, &self.catalog);
                if n == at {
                    self.rounded(chip, h as f32 / 2., theme.text, None)?;
                    self.label_aligned(
                        &name,
                        (Face::Semibold, 12., theme.background),
                        chip,
                        Align::Center,
                    )?;
                } else {
                    self.rounded(
                        chip,
                        h as f32 / 2.,
                        alpha(theme.text, 0x14),
                        Some(alpha(theme.text, 0x1A)),
                    )?;
                    self.label_aligned(
                        &name,
                        (Face::Regular, 12., theme.text_secondary),
                        chip,
                        Align::Center,
                    )?;
                }
            }
            cx += w + gap;
        }
        self.canvas.set_clip_rect(None);
        mark(self, "R1", (x + room + gap).min(cx))?;
        Ok(())
    }
}

/// Whether a shelf names its items under them (games) or on them.
fn labelled(shelf: &Shelf) -> bool {
    shelf.kind == ShelfKind::Favorites
}

/// The picture of a recent game or a game, when it has loaded: its path
/// (the blur's key) and texture.
fn picture_of<'a, 't>(
    app: &App,
    data: &dyn DataSource,
    art: Art<'a, 't>,
    item: &Item,
) -> Option<(PathBuf, &'a Texture<'t>)> {
    match item {
        Item::Recent(n) => {
            let recent = app.recent.get(*n)?;
            let picture = recent.picture.as_ref().and_then(|path| {
                art.pictures
                    .and_then(|c| c.peek(path))
                    .map(|t| (path.clone(), t))
            });
            picture.or_else(|| {
                let path = data.games(&recent.system).get(recent.game)?.image.clone()?;
                art.covers.and_then(|c| c.peek(&path)).map(|t| (path, t))
            })
        }
        Item::Game { system, game } => {
            let path = data.games(system).get(*game)?.image.clone()?;
            art.covers.and_then(|c| c.peek(&path)).map(|t| (path, t))
        }
        _ => None,
    }
}

/// The shelves' screen as `Screen::Home` gives it, if that is the screen.
pub(super) fn home_at(screen: &Screen) -> Option<(Tab, usize, usize)> {
    match *screen {
        Screen::Home { tab, shelf, item } => Some((tab, shelf, item)),
        _ => None,
    }
}
