//! A game's details as a showcase (docs/temas.md, `[details]`,
//! `style = "hero"`): the game's picture blurred over the whole screen
//! under veils, a capsule back to its system, the title large, a line of
//! facts, the description, Play, Favorite and Scrape again, when it was
//! last played, its picture at the right and, below, more of its system.
use super::{
    details::{fit, Art},
    system_title, Align, App, DataSource, Face, Frame, Glyph, Rect, Rgba, Theme, Ui,
};
use crate::{primitives::shade, state::DETAILS_ACTIONS, theme::Gradient};

fn alpha(c: Rgba, a: u8) -> Rgba {
    Rgba { a, ..c }
}
/// Games beside this one on the shelf below, and its cards' tallest
/// height in reference pixels.
const SHELF: usize = 8;
const SHELF_CARD: f32 = 100.;

impl Ui<'_, '_> {
    pub(super) fn details_hero(
        &mut self,
        app: &App,
        (system, index): (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Some(game) = data.games(system).get(index) else {
            return Ok(());
        };
        let Frame { width, height, .. } = self.frame;
        let (w, h) = (width as i32, height as i32);
        let whole = Rect::new(0, 0, width, height);
        let picture = art.shot.or(art.cover);
        // The picture blurred once and kept, over the whole screen.
        let key = game
            .details
            .screenshot
            .clone()
            .or_else(|| game.image.clone());
        if let (Some(texture), Some(key)) = (picture, key) {
            let blur = theme.glass.map_or(0., |g| g.blur).max(24. / 480.);
            let steps = super::material::halvings(self.frame.size(blur));
            let made = self.blurred(&key, steps, |ui| {
                ui.canvas.copy(texture, None, None).map(|()| true)
            });
            if let Some(index) = made {
                self.draw_blurred(index, whole, 200)?;
            }
        }
        let bg = theme.background;
        shade(
            &mut self.canvas,
            whole,
            &Gradient {
                from: alpha(bg, 235),
                to: alpha(bg, 90),
                angle: 0.,
            },
        )?;
        let low = h * 45 / 100;
        shade(
            &mut self.canvas,
            Rect::new(0, low, width, (h - low) as u32),
            &Gradient {
                from: alpha(bg, 0),
                to: bg,
                angle: 90.,
            },
        )?;

        // Back to the system, in a capsule of glass; the status at the right.
        let name = system_title(system, data, &self.catalog);
        let margin = self.px(28.);
        let cap_h = self.px(30.);
        let text_w = self.measure(&name, Face::Semibold, 13.)? as i32;
        let chevron = self.px(14.);
        let cap_w = (self.px(12.) + chevron + self.px(4.) + text_w + self.px(14.))
            .min(w / 2)
            .max(cap_h);
        let capsule = Rect::new(
            margin - self.px(8.),
            self.px(14.),
            cap_w as u32,
            cap_h as u32,
        );
        self.glass(capsule, cap_h as f32 / 2., &theme.glass_or_default())?;
        let glyph = Rect::new(
            capsule.x() + self.px(10.),
            capsule.y() + (cap_h - chevron) / 2,
            chevron as u32,
            chevron as u32,
        );
        self.icon(Glyph::ChevronLeft, glyph, theme.text)?;
        self.label(
            &name,
            (Face::Semibold, 13., theme.text),
            Rect::new(
                glyph.right() + self.px(4.),
                capsule.y(),
                (capsule.right() - glyph.right() - self.px(10.)).max(1) as u32,
                cap_h as u32,
            ),
        )?;
        self.status_header(theme)?;

        // The shelf below takes cards up to a height; a taller screen gives
        // the rest to the showcase above it.
        let shelf = theme.screens.details.shelf && data.games(system).len() > 1;
        let shelf_top = if shelf {
            let tallest = self.px(SHELF_CARD);
            let over = self.px(56.) + self.px(28.);
            (h * 64 / 100).max(h - over - tallest)
        } else {
            h - margin
        };
        let top = capsule.bottom() + self.px(22.);
        let band_bottom = shelf_top - self.px(26.);
        // The picture at the right, in the middle of the band.
        let pic = Rect::new(
            w * 58 / 100,
            top,
            (w * 42 / 100 - margin).max(1) as u32,
            (band_bottom - top - self.px(2.)).max(1) as u32,
        );
        if let Some(texture) = picture {
            let dst = fit(texture, pic, false);
            let radius = self.px(14.) as f32;
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
        // The text beside it: the title, the facts, the description,
        // the buttons and when it was played, one block in the middle of
        // the band; the description takes what the band leaves.
        let column = (pic.x() - margin - self.px(20.)).max(1) as u32;
        let big = 38.;
        let floor = self.frame.min_size().max(20.);
        let size = self
            .shrink(&game.name, Face::Bold, big, floor, column)?
            .unwrap_or(floor);
        let step = (self.frame.size(size / 480.) * 1.08).round() as i32;
        let title: Vec<String> = self
            .wrap(&game.name, Face::Bold, size, column)?
            .into_iter()
            .take(2)
            .collect();
        let details = &game.details;
        let mut facts: Vec<String> = Vec::new();
        if let Some(year) = game.year {
            facts.push(year.to_string());
        }
        if !details.genre.trim().is_empty() {
            facts.push(details.genre.trim().to_owned());
        }
        if let Some(players) = &game.players {
            facts.push(self.catalog.players(players));
        }
        if !details.developer.trim().is_empty() {
            facts.push(details.developer.trim().to_owned());
        }
        let line_h = self.px(18.);
        let button_h = self.px(36.);
        let played_h = if game.last_played.is_some() {
            self.px(22.)
        } else {
            0
        };
        let text = details.description.trim();
        let text_step = self.px(18.);
        let fixed = step * title.len() as i32
            + self.px(6.)
            + line_h
            + self.px(10.)
            + self.px(12.)
            + button_h
            + played_h;
        let band = band_bottom - top;
        let room = ((band - fixed) / text_step).max(0) as usize;
        let lines = if text.is_empty() || room == 0 {
            Vec::new()
        } else {
            self.wrap(text, Face::Regular, 13., column)?
        };
        let shown = lines.len().min(room);
        let block = fixed + text_step * shown as i32;
        let mut y = top + ((band - block) / 2).max(0);
        for line in &title {
            self.label(
                line,
                (Face::Bold, size, theme.text),
                Rect::new(margin, y, column, step as u32),
            )?;
            y += step;
        }
        y += self.px(6.);
        // The last facts go first when the line is tight.
        while !facts.is_empty() {
            let text = facts.join(" · ");
            if self.measure(&text, Face::Regular, 13.)? <= column || facts.len() == 1 {
                self.label(
                    &text,
                    (Face::Regular, 13., theme.text_secondary),
                    Rect::new(margin, y, column, line_h as u32),
                )?;
                break;
            }
            facts.pop();
        }
        y += line_h + self.px(10.);
        if shown > 0 {
            let first = app.details_scroll.min(lines.len().saturating_sub(shown));
            self.details_overflow = lines.len().saturating_sub(shown);
            for line in lines.iter().skip(first).take(shown) {
                self.label(
                    line,
                    (Face::Regular, 13., theme.text_secondary),
                    Rect::new(margin, y, column, text_step as u32),
                )?;
                y += text_step;
            }
        } else {
            self.details_overflow = 0;
        }
        let buttons_y = y + self.px(12.);
        self.hero_actions(app, game.favorite, (margin, buttons_y, button_h), theme)?;
        if let Some(at) = game.last_played {
            let ago = self.catalog.relative_time(at, self.frame.unix);
            let played = self.catalog.format("details.played", &[("ago", &ago)]);
            self.label(
                &played,
                (Face::Regular, 12., theme.muted),
                Rect::new(
                    margin,
                    buttons_y + button_h + self.px(8.),
                    column,
                    self.px(16.) as u32,
                ),
            )?;
        }
        if shelf {
            self.more_from(system, index, data, theme, art, (margin, shelf_top))?;
        }
        Ok(())
    }

    /// Play as a light pill, Favorite and Scrape again round; the focused
    /// one ringed.
    fn hero_actions(
        &mut self,
        app: &App,
        favorite: bool,
        (x, y, h): (i32, i32, i32),
        theme: &Theme,
    ) -> Result<(), String> {
        let mut x = x;
        for (n, key) in DETAILS_ACTIONS.iter().enumerate() {
            let focused = app.details_focus == n;
            let rect = if n == 0 {
                let label = self.catalog.text(key);
                let icon = self.px(13.);
                let tw = self.measure(&label, Face::Semibold, 14.)? as i32;
                let pw = self.px(16.) + icon + self.px(8.) + tw + self.px(20.);
                let pill = Rect::new(x, y, pw as u32, h as u32);
                self.cast_shadow(pill, h as f32 / 2.)?;
                self.rounded(pill, h as f32 / 2., theme.text, None)?;
                let glyph = Rect::new(
                    x + self.px(16.),
                    y + (h - icon) / 2,
                    icon as u32,
                    icon as u32,
                );
                self.icon(Glyph::Play, glyph, theme.background)?;
                self.label(
                    &label,
                    (Face::Semibold, 14., theme.background),
                    Rect::new(glyph.right() + self.px(8.), y, tw.max(1) as u32, h as u32),
                )?;
                pill
            } else {
                let round = Rect::new(x, y, h as u32, h as u32);
                self.rounded(
                    round,
                    h as f32 / 2.,
                    alpha(theme.text, 0x1C),
                    Some(alpha(theme.text, 0x24)),
                )?;
                let side = self.px(16.);
                let glyph = Rect::new(
                    x + (h - side) / 2,
                    y + (h - side) / 2,
                    side as u32,
                    side as u32,
                );
                let (icon, ink) = match n {
                    1 if favorite => (Glyph::Star, theme.warning),
                    1 => (Glyph::Star, theme.text),
                    _ if app.rescraping => (Glyph::Loader, theme.muted),
                    _ if !app.can_rescrape => (Glyph::Restart, theme.dim_text),
                    _ => (Glyph::Restart, theme.text),
                };
                self.icon(icon, glyph, ink)?;
                round
            };
            if focused {
                // The game menu opens beside the button it was called from.
                self.focus_at = Some(rect);
                let out = self.px(3.);
                let ring = Rect::new(
                    rect.x() - out,
                    rect.y() - out,
                    rect.width() + 2 * out as u32,
                    rect.height() + 2 * out as u32,
                );
                self.focus_ring(ring, ring.height() as f32 / 2., theme)?;
            }
            x = rect.right() + self.px(10.);
        }
        Ok(())
    }

    /// "More from <system>": the covers of the games around this one.
    fn more_from(
        &mut self,
        system: &str,
        index: usize,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        (x, top): (i32, i32),
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let name = system_title(system, data, &self.catalog);
        let title = self
            .catalog
            .format("details.more_from", &[("system", &name)]);
        let title_h = self.px(22.);
        self.label(
            &title,
            (Face::Semibold, 14., theme.text),
            Rect::new(
                x,
                top,
                width.saturating_sub(2 * x as u32).max(1),
                title_h as u32,
            ),
        )?;
        let y = top + title_h + self.px(6.);
        let card_h = (height as i32 - self.px(56.) - y).max(self.px(24.));
        let card_w = card_h * 4 / 3;
        let gap = self.px(10.);
        let games = data.games(system);
        let others = (0..games.len())
            .filter(|&n| n != index)
            .skip(index.saturating_sub(2));
        let mut cx = x;
        for n in others.take(SHELF) {
            if cx >= width as i32 {
                break;
            }
            let rect = Rect::new(cx, y, card_w as u32, card_h as u32);
            let radius = self.px(10.) as f32;
            let cover = games[n]
                .image
                .as_ref()
                .and_then(|path| art.covers.and_then(|c| c.peek(path)));
            match cover {
                Some(texture) => self.geometry.cover(
                    &mut self.canvas,
                    texture,
                    rect,
                    radius,
                    255,
                    [0., 0., 1., 1.],
                )?,
                None => {
                    self.rounded(rect, radius, theme.surface, Some(theme.border))?;
                    let inset = Rect::new(
                        rect.x() + self.px(6.),
                        rect.y(),
                        rect.width().saturating_sub(self.px(12.) as u32).max(1),
                        rect.height(),
                    );
                    self.label_aligned(
                        &games[n].name,
                        (Face::Regular, 11., theme.muted),
                        inset,
                        Align::Center,
                    )?;
                }
            }
            cx += card_w + gap;
        }
        Ok(())
    }
}
