//! Game details (Jogo-detalhes): scraped art, facts, description and actions.
//! Every block sits in a theme rectangle, so a theme can rearrange the screen.
use super::*;
use crate::state::DETAILS_ACTIONS;

/// Decoded art for the details screen; textures stay with their caches.
#[derive(Clone, Copy, Default)]
pub struct Art<'a, 't> {
    pub cover: Option<&'a Texture<'t>>,
    pub shot: Option<&'a Texture<'t>>,
    pub logo: Option<&'a Texture<'t>>,
    /// Achievement badges decoded so far, by file.
    pub badges: Option<&'a crate::covers::CoverCache<'t>>,
}

/// Wrapped description lines, kept until the text, width or size changes.
#[derive(Default)]
pub(super) struct Wrapped {
    key: (String, u32, u16),
    lines: Vec<String>,
}

/// `texture` scaled to fit inside `rect`, centered (or left-aligned).
fn fit(texture: &Texture<'_>, rect: Rect, left: bool) -> Rect {
    let size = texture.query();
    let scale = (rect.width() as f32 / size.width.max(1) as f32)
        .min(rect.height() as f32 / size.height.max(1) as f32);
    let w = (size.width as f32 * scale).round().max(1.) as u32;
    let h = (size.height as f32 * scale).round().max(1.) as u32;
    let x = if left {
        rect.x()
    } else {
        rect.x() + (rect.width() - w) as i32 / 2
    };
    Rect::new(x, rect.y() + (rect.height() - h) as i32 / 2, w, h)
}

impl Ui<'_, '_> {
    /// Word wrap at measured boundaries; a word wider than the line breaks by character.
    pub(super) fn wrap(
        &mut self,
        text: &str,
        face: Face,
        size: f32,
        width: u32,
    ) -> Result<Vec<String>, String> {
        let mut lines = Vec::new();
        for paragraph in text.lines() {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let joined = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if self.measure(&joined, face, size)? <= width {
                    line = joined;
                    continue;
                }
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                for ch in word.chars() {
                    if !line.is_empty() && self.measure(&format!("{line}{ch}"), face, size)? > width
                    {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push(ch);
                }
            }
            lines.push(line);
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        Ok(lines)
    }

    /// Largest size from `start` down to `min` at which `text` fits `width`.
    pub(super) fn shrink(
        &mut self,
        text: &str,
        face: Face,
        start: f32,
        min: f32,
        width: u32,
    ) -> Result<Option<f32>, String> {
        let mut size = start;
        while size >= min {
            if self.measure(text, face, size)? <= width {
                return Ok(Some(size));
            }
            size -= 1.;
        }
        Ok(None)
    }

    pub(super) fn details_header(
        &mut self,
        system: &str,
        game: &str,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let trail = system_title(system, data, &self.catalog);
        self.look.outlining = true;
        let drawn = self.crumbs(&trail, game, theme);
        self.look.outlining = false;
        drawn
    }

    /// "Trail /" in muted 13 px, then the current place in 15 px semibold.
    pub(super) fn crumbs(&mut self, trail: &str, game: &str, theme: &Theme) -> Result<(), String> {
        let right = self.status_header(theme)?;
        let h = (theme.header_height * self.frame.height as f32).round() as u32;
        let pad = self.px(24.);
        let trail = format!("{trail} /");
        let tw = self
            .measure(&trail, Face::Regular, 13.)?
            .min((right - pad).max(1) as u32 / 2);
        self.label(
            &trail,
            (Face::Regular, 13., theme.muted),
            Rect::new(pad, 0, tw, h),
        )?;
        let x = pad + tw as i32 + self.px(8.);
        self.label(
            game,
            (Face::Semibold, 15., theme.text),
            Rect::new(x, 0, (right - self.px(16.) - x).max(1) as u32, h),
        )
    }

    pub(super) fn details(
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
        let rect = |r| pixels(r, width, height);

        // Screen shot (a still even when a preview video exists), else the cover.
        let shot = rect(theme.details_shot);
        let radius = self.px(12.) as f32;
        self.panel_named("art", shot, radius, (theme.surface, theme.border))?;
        // A themed frame dresses the facts, and the column from the title to
        // the buttons; without one they sit on the background.
        self.framed("panel", rect(theme.details_info))?;
        let column = [
            theme.details_title,
            theme.details_subtitle,
            theme.details_text,
            theme.details_actions,
        ]
        .map(rect);
        let union = column.iter().skip(1).fold(column[0], |a, b| a.union(*b));
        self.framed("panel", union)?;
        if let Some(texture) = art.shot.or(art.cover) {
            let inner = Rect::new(
                shot.x() + 1,
                shot.y() + 1,
                shot.width().saturating_sub(2).max(1),
                shot.height().saturating_sub(2).max(1),
            );
            let dst = fit(texture, inner, false);
            self.geometry.cover(
                &mut self.canvas,
                texture,
                dst,
                (radius - 1.).max(0.),
                255,
                WHOLE,
            )?;
        } else {
            self.label_aligned(
                &system_title(system, data, &self.catalog),
                (Face::Regular, 13., theme.dim_text),
                shot,
                Align::Center,
            )?;
        }

        // Facts table: label left, value right; values shrink, then wrap.
        let info = rect(theme.details_info);
        let details = &game.details;
        let mut facts: Vec<(&str, String)> = Vec::new();
        let mut add = |key, value: &str| {
            if !value.trim().is_empty() {
                facts.push((key, value.trim().to_owned()));
            }
        };
        add("details.genre", &details.genre);
        add("details.players", game.players.as_deref().unwrap_or(""));
        add(
            "details.released",
            &game.year.map(|y| y.to_string()).unwrap_or_default(),
        );
        add("details.developer", &details.developer);
        if details.publisher != details.developer {
            add("details.publisher", &details.publisher);
        }
        let mut y = info.y();
        // Lines follow the text size, which a theme may scale.
        let line = (f32::from(self.frame.font_size(12.)) * 1.25)
            .round()
            .max(1.) as i32;
        let pad = self.px(3.);
        for (key, value) in facts {
            let label = self.catalog.text(key);
            let lw = self.measure(&label, Face::Regular, 12.)?;
            let beside = info.width().saturating_sub(lw + self.px(12.) as u32).max(1);
            // A value that does not fit beside its label goes whole to the
            // lines below, across the table; words are never split.
            let (size, lines, below, room) =
                match self.shrink(&value, Face::Regular, 12., 10., beside)? {
                    Some(size) => (size, vec![value], 0, beside),
                    None => {
                        let lines = self.wrap(&value, Face::Regular, 12., info.width())?;
                        (12., lines, 1, info.width())
                    }
                };
            let row = line * (lines.len() + below) as i32 + pad * 2;
            if y + row > info.bottom() {
                break;
            }
            self.label(
                &label,
                (Face::Regular, 12., theme.muted),
                Rect::new(info.x(), y + pad, lw.max(1), line as u32),
            )?;
            for (n, text) in lines.iter().enumerate() {
                self.label_aligned(
                    text,
                    (Face::Regular, size, theme.text),
                    Rect::new(
                        info.right() - room as i32,
                        y + pad + line * (n + below) as i32,
                        room,
                        line as u32,
                    ),
                    Align::Right,
                )?;
            }
            y += row;
            self.divider(info.x(), y - 1, info.width(), theme.border)?;
        }
        if let Some(rating) = details
            .rating
            .filter(|_| y + line + pad * 2 <= info.bottom())
        {
            let label = self.catalog.text("details.rating");
            let lw = self.measure(&label, Face::Regular, 12.)?;
            self.label(
                &label,
                (Face::Regular, 12., theme.muted),
                Rect::new(info.x(), y + pad, lw.max(1), line as u32),
            )?;
            let score = format!("{:.1}", f32::from(rating) / 20.)
                .replace('.', &self.catalog.text("details.decimal"));
            let sw = self.measure(&score, Face::Regular, 12.)?;
            self.label(
                &score,
                (Face::Regular, 12., theme.text),
                Rect::new(info.right() - sw as i32, y + pad, sw.max(1), line as u32),
            )?;
            let filled = (u32::from(rating) + 10) / 20;
            let star = self.px(11.);
            for n in 0..5 {
                let x = info.right() - sw as i32 - self.px(6.) - (5 - n) * (star + self.px(2.));
                let ink = if (n as u32) < filled {
                    theme.warning
                } else {
                    theme.border
                };
                self.icon(
                    Glyph::Star,
                    Rect::new(x, y + pad + (line - star) / 2, star as u32, star as u32),
                    ink,
                )?;
            }
            y += line + pad * 2;
            self.divider(info.x(), y - 1, info.width(), theme.border)?;
        }

        // Logo as the title when there is one, else the name in text.
        let title = rect(theme.details_title);
        if let Some(logo) = art.logo {
            let dst = fit(logo, title, true);
            self.canvas.copy(logo, None, dst)?;
        } else {
            let size = self
                .shrink(&game.name, Face::Semibold, 26., 18., title.width())?
                .unwrap_or(18.);
            self.label(&game.name, (Face::Semibold, size, theme.text), title)?;
        }

        // System and last played; the date goes before the text ever shrinks away.
        let sub = rect(theme.details_subtitle);
        let mut parts = vec![system_title(system, data, &self.catalog)];
        if let Some(played) = game.last_played {
            parts.push(self.catalog.relative_time(played, self.frame.unix));
        }
        loop {
            let text = parts.join(" · ");
            if let Some(size) = self.shrink(&text, Face::Regular, 12., 10., sub.width())? {
                self.label(&text, (Face::Regular, size, theme.muted), sub)?;
                break;
            }
            if parts.len() == 1 {
                let lines = self.wrap(&text, Face::Regular, 10., sub.width())?;
                self.label(&lines[0], (Face::Regular, 10., theme.muted), sub)?;
                break;
            }
            parts.pop();
        }

        self.details_text(app, game, rect(theme.details_text), theme)?;
        self.details_actions(app, game.favorite, rect(theme.details_actions), theme)
    }

    fn details_text(
        &mut self,
        app: &App,
        game: &archr_index::Game,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let text = game.details.description.trim();
        if text.is_empty() {
            self.details_overflow = 0;
            let lines = self.wrap(
                &self.catalog.text("details.no_description"),
                Face::Regular,
                13.,
                area.width(),
            )?;
            for (n, line) in lines.iter().enumerate() {
                let y = area.y() + self.px(19.5 * n as f32);
                self.label(
                    line,
                    (Face::Regular, 13., theme.muted),
                    Rect::new(area.x(), y, area.width(), self.px(19.5) as u32),
                )?;
            }
            return Ok(());
        }
        let key = (text.to_owned(), area.width(), self.frame.font_size(13.));
        if self.details_wrap.key != key {
            let lines = self.wrap(text, Face::Regular, 13., area.width())?;
            self.details_wrap = Wrapped { key, lines };
        }
        let step = self.frame.scale * 19.5 * self.frame.text;
        let visible = ((area.height() as f32 / step).floor() as usize).max(1);
        let total = self.details_wrap.lines.len();
        self.details_overflow = total.saturating_sub(visible);
        let first = app.details_scroll.min(self.details_overflow);
        for n in 0..visible.min(total - first) {
            let line = self.details_wrap.lines[first + n].clone();
            let y = area.y() + (step * n as f32).round() as i32;
            self.label(
                &line,
                (Face::Regular, 13., theme.text_secondary),
                Rect::new(area.x(), y, area.width(), step.round() as u32),
            )?;
        }
        // More below: fade the last fifth into the background.
        if first < self.details_overflow {
            let band = (area.height() as f32 * 0.22).round() as i32;
            let steps = 12;
            for n in 0..steps {
                let alpha = (255. * (n + 1) as f32 / steps as f32) as u8;
                let top = area.bottom() - band + band * n / steps;
                self.canvas.set_draw_color(color(Rgba {
                    a: alpha,
                    ..theme.background
                }));
                self.canvas.fill_rect(Rect::new(
                    area.x(),
                    top,
                    area.width(),
                    (band / steps).max(1) as u32 + 1,
                ))?;
            }
        }
        Ok(())
    }

    fn details_actions(
        &mut self,
        app: &App,
        favorite: bool,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let labels: Vec<String> = DETAILS_ACTIONS
            .iter()
            .enumerate()
            .map(|(n, key)| {
                self.catalog.text(match n {
                    2 if app.rescraping => "details.rescraping",
                    _ => key,
                })
            })
            .collect();
        // Shrink text and padding together until the three buttons fit.
        let icon = |n: usize| n == 1;
        let mut size = 14.;
        let floor = self.frame.min_size().max(10. / self.frame.font.max(1.));
        let gap = self.px(8.);
        let (mut widths, mut pad) = loop {
            let pad = self.px(size + 1.);
            let mut widths = Vec::new();
            for (n, label) in labels.iter().enumerate() {
                let extra = if icon(n) { self.px(size + 6.) } else { 0 };
                widths.push(self.measure(label, Face::Semibold, size)? as i32 + pad * 2 + extra);
            }
            let total: i32 = widths.iter().sum::<i32>() + gap * 2;
            // The floor: 10 px on screen and the theme's smallest text.
            if total <= area.width() as i32 || size <= floor {
                break (widths, pad);
            }
            size -= 1.;
        };
        // Still too wide at the floor: labels break between words. Each
        // button holds its longest word, and the room left is shared.
        let total: i32 = widths.iter().sum::<i32>() + gap * 2;
        if total > area.width() as i32 {
            pad = self.px(6.);
            let mut least = Vec::new();
            for (n, label) in labels.iter().enumerate() {
                let mut word = 0;
                for w in label.split_whitespace() {
                    word = word.max(self.measure(w, Face::Semibold, size)? as i32);
                }
                least.push(word + pad * 2 + if icon(n) { self.px(size + 6.) } else { 0 });
            }
            let spare = (area.width() as i32 - gap * 2 - least.iter().sum::<i32>()) / 3;
            widths = least.iter().map(|w| (w + spare).max(1)).collect();
        }
        let mut x = area.x();
        for (n, label) in labels.iter().enumerate() {
            let focused = n == app.details_focus;
            let button = Rect::new(x, area.y(), widths[n] as u32, area.height());
            if focused {
                if !self.framed("button_selected", button)? {
                    self.rounded(button, self.px(10.) as f32, theme.highlight_bg, None)?;
                }
            } else {
                let colors = (theme.surface, theme.border);
                self.panel_named("button", button, self.px(10.) as f32, colors)?;
            }
            let disabled = n == 2 && (!app.can_rescrape || app.rescraping);
            let ink = match (focused, disabled) {
                (true, _) => theme.highlight_text,
                (false, true) => theme.dim_text,
                (false, false) => theme.text,
            };
            let mut text_x = button.x() + pad;
            if icon(n) {
                let side = self.px(size);
                let star = if favorite { theme.warning } else { ink };
                self.icon(
                    Glyph::Star,
                    Rect::new(
                        text_x,
                        button.y() + (button.height() as i32 - side) / 2,
                        side as u32,
                        side as u32,
                    ),
                    star,
                )?;
                text_x += side + self.px(6.);
            }
            let face = if focused {
                Face::Semibold
            } else {
                Face::Regular
            };
            let room = (button.right() - text_x - pad.min(text_x - button.x())).max(1) as u32;
            let lines = if self.measure(label, face, size)? > room {
                self.wrap(label, face, size, room)?
            } else {
                vec![label.clone()]
            };
            let step = button.height() as i32 / lines.len() as i32;
            for (k, line) in lines.iter().enumerate() {
                self.label(
                    line,
                    (face, size, ink),
                    Rect::new(
                        text_x,
                        button.y() + step * k as i32,
                        room,
                        step.max(1) as u32,
                    ),
                )?;
            }
            x += widths[n] + gap;
        }
        Ok(())
    }
}
