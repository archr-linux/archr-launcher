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
    /// The playing preview video's frame, already the shot box's size, and
    /// how far through the clip it is (0..1).
    pub video: Option<(&'a Texture<'t>, f32)>,
    /// Achievement badges decoded so far, by file.
    pub badges: Option<&'a crate::covers::CoverCache<'t>>,
    /// Save-state pictures and screenshots, small, by file.
    pub thumbs: Option<&'a crate::covers::CoverCache<'t>>,
    /// The selected state's picture, or the screenshot on the whole screen.
    pub picture: Option<&'a Texture<'t>>,
}

/// Wrapped description lines, kept until the text, width or size changes.
#[derive(Default)]
pub(super) struct Wrapped {
    key: (String, u32, u16),
    lines: Vec<String>,
}

/// `texture` scaled to fit inside `rect`, centered (or left-aligned).
pub(super) fn fit(texture: &Texture<'_>, rect: Rect, left: bool) -> Rect {
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
    /// Over a playing preview: a "Preview" tag and a thin progress line.
    fn preview_marks(&mut self, inner: Rect, progress: f32, theme: &Theme) -> Result<(), String> {
        let edge = self.px(10.);
        let text = self.catalog.text("details.preview");
        let (icon, gap, pad) = (self.px(12.), self.px(6.), self.px(8.));
        let tw = self.measure(&text, Face::Regular, 11.)? as i32;
        let tag = Rect::new(
            inner.x() + edge,
            inner.y() + edge,
            (pad * 2 + icon + gap + tw) as u32,
            self.px(20.) as u32,
        );
        let veil = Rgba {
            a: 184,
            ..theme.background
        };
        self.rounded(tag, tag.height() as f32 / 2., veil, None)?;
        let middle = tag.y() + (tag.height() as i32 - icon) / 2;
        let glyph = Rect::new(tag.x() + pad, middle, icon as u32, icon as u32);
        self.icon(Glyph::Play, glyph, theme.text)?;
        self.label(
            &text,
            (Face::Regular, 11., theme.text),
            Rect::new(glyph.right() + gap, tag.y(), tw.max(1) as u32, tag.height()),
        )?;
        let bar = self.px(3.);
        let track = Rect::new(
            inner.x() + edge,
            inner.bottom() - edge - bar,
            inner.width().saturating_sub(2 * edge as u32).max(1),
            bar as u32,
        );
        let white = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 64,
        };
        self.rounded(track, 2., white, None)?;
        let done = (track.width() as f32 * progress.clamp(0., 1.)).round() as u32;
        if done > 0 {
            let fill = Rect::new(track.x(), track.y(), done.max(bar as u32), track.height());
            self.rounded(fill, 2., theme.highlight_bg, None)?;
        }
        Ok(())
    }
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
        let gap = self.px(8.);
        let available = (right - self.px(16.) - pad).max(1);
        let trail = format!("{trail} /");
        // Both shrink together, down to the theme's smallest text.
        let floor = self.frame.min_size();
        let (mut small, mut big) = (13., 15.);
        let tw = loop {
            let tw = self.measure(&trail, Face::Regular, small)? as i32;
            let gw = self.measure(game, Face::Semibold, big)? as i32;
            if tw + gap + gw <= available || small <= floor {
                break tw;
            }
            (small, big) = ((small - 1.).max(floor), (big - 1.).max(floor));
        };
        // The trail is never cut: past two thirds of the room it wraps. The
        // game name, a title, takes what is left.
        let cap = available * 2 / 3;
        let lines = if tw > cap {
            self.wrap(&trail, Face::Regular, small, cap as u32)?
        } else {
            vec![trail.clone()]
        };
        let tw = tw.min(cap);
        let step = h as i32 / lines.len() as i32;
        for (n, line) in lines.iter().enumerate() {
            self.label(
                line,
                (Face::Regular, small, theme.muted),
                Rect::new(pad, step * n as i32, tw.max(1) as u32, step.max(1) as u32),
            )?;
        }
        let x = pad + tw + gap;
        self.label(
            game,
            (Face::Semibold, big, theme.text),
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

        // The preview video while it plays, else the screen shot, else the cover.
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
        let inner = Rect::new(
            shot.x() + 1,
            shot.y() + 1,
            shot.width().saturating_sub(2).max(1),
            shot.height().saturating_sub(2).max(1),
        );
        if let Some((frame, progress)) = art.video {
            self.geometry.cover(
                &mut self.canvas,
                frame,
                inner,
                (radius - 1.).max(0.),
                255,
                WHOLE,
            )?;
            self.preview_marks(inner, progress, theme)?;
        } else if let Some(texture) = art.shot.or(art.cover) {
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
            // Stars that do not fit beside the label and score go below them,
            // or stay out when the table has no room left.
            let stars = 5 * (star + self.px(2.)) + self.px(6.);
            let below = lw as i32 + self.px(12.) + stars + sw as i32 > info.width() as i32;
            let shown = !below || y + line * 2 + pad * 2 <= info.bottom();
            let (end, top) = if below {
                (info.right() + self.px(6.), y + pad + line)
            } else {
                (info.right() - sw as i32, y + pad)
            };
            for n in (0..5).filter(|_| shown) {
                let x = end - self.px(6.) - (5 - n) * (star + self.px(2.));
                let ink = if (n as u32) < filled {
                    theme.warning
                } else {
                    theme.border
                };
                self.icon(
                    Glyph::Star,
                    Rect::new(x, top + (line - star) / 2, star as u32, star as u32),
                    ink,
                )?;
            }
            y += line * (1 + i32::from(below && shown)) + pad * 2;
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
        // Sized for both of the last button's states (resting, and dots
        // with "Scraping…"), so a running job never moves the row.
        let labels: Vec<String> = DETAILS_ACTIONS
            .iter()
            .map(|key| self.catalog.text(key))
            .collect();
        let scraping = self.catalog.text("details.rescraping");
        // Shrink text and padding together until the three buttons fit.
        let icon = |n: usize| n == 1;
        let mut size = 14.;
        let floor = self.frame.min_size().max(10. / self.frame.font.max(1.));
        let mut gap = self.px(8.);
        let between = labels.len().saturating_sub(1) as i32;
        let (mut widths, mut pad) = loop {
            let pad = self.px(size + 1.);
            let mut widths = Vec::new();
            for (n, label) in labels.iter().enumerate() {
                let extra = if icon(n) { self.px(size + 6.) } else { 0 };
                let mut text = self.measure(label, Face::Semibold, size)? as i32;
                if n == 2 {
                    let busy = self.measure(&scraping, Face::Semibold, size)? as i32;
                    text = text.max(busy + self.px(size * 1.8 + 6.));
                }
                widths.push(text + pad * 2 + extra);
            }
            let total: i32 = widths.iter().sum::<i32>() + gap * between;
            // The floor: 10 px on screen and the theme's smallest text.
            if total <= area.width() as i32 || size <= floor {
                break (widths, pad);
            }
            size -= 1.;
        };
        // Still too wide at the floor: labels break between words. Each
        // button holds its longest word, and the room left is shared; when
        // even that is too wide, the buttons share the row equally and words
        // break. A button never leaves its row.
        let total: i32 = widths.iter().sum::<i32>() + gap * between;
        if total > area.width() as i32 {
            (pad, gap) = (self.px(6.), self.px(4.));
            let mut least = Vec::new();
            for (n, label) in labels.iter().enumerate() {
                // In the face it is drawn with: semibold only when focused.
                let face = if n == app.details_focus {
                    Face::Semibold
                } else {
                    Face::Regular
                };
                let mut word = 0;
                for w in label.split_whitespace() {
                    word = word.max(self.measure(w, face, size)? as i32);
                }
                least.push(word + pad * 2 + if icon(n) { self.px(size + 6.) } else { 0 });
            }
            let count = labels.len().max(1) as i32;
            let room = area.width() as i32 - gap * between;
            let spare = (room - least.iter().sum::<i32>()) / count;
            widths = if spare >= 0 {
                least.iter().map(|w| w + spare).collect()
            } else {
                vec![(room / count).max(1); labels.len()]
            };
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
            let face = if focused {
                Face::Semibold
            } else {
                Face::Regular
            };
            if n == 2 && app.rescraping {
                self.scraping(button, (face, size, ink), pad, focused, theme)?;
                x += widths[n] + gap;
                continue;
            }
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

    /// "Scrape again" while its job runs: the Wi-Fi search's pulsing dots,
    /// then "Scraping…" when it fits on one line; the dots alone, centered,
    /// when it does not. Looping state comes from the clock, never stored.
    fn scraping(
        &mut self,
        button: Rect,
        (face, size, ink): (Face, f32, Rgba),
        pad: i32,
        focused: bool,
        theme: &Theme,
    ) -> Result<(), String> {
        // The dots take 1.8 × the text size, as measured for the row.
        let (dot, step) = (self.px(size * 0.4).max(2), self.px(size * 0.7));
        let dots = step * 2 + dot;
        let text = self.catalog.text("details.rescraping");
        let lead = self.px(size * 1.8 + 6.);
        let room = button.width() as i32 - pad * 2 - lead;
        let shown = self.measure(&text, face, size)? as i32 <= room;
        let mut left = if shown {
            button.x() + pad
        } else {
            button.x() + (button.width() as i32 - dots) / 2
        };
        let mid = button.y() + button.height() as i32 / 2;
        let secs = self.frame.now.saturating_duration_since(self.born);
        let fill = if focused {
            theme.highlight_text
        } else {
            theme.highlight_bg
        };
        for k in 0..3 {
            let (alpha, lift) = self.motion.search_dot(secs.as_secs_f32(), k);
            let at = Rect::new(
                left + step * k as i32,
                mid - dot / 2 - self.px(lift),
                dot as u32,
                dot as u32,
            );
            let fill = Rgba {
                a: (f32::from(fill.a) * alpha) as u8,
                ..fill
            };
            self.rounded(at, dot as f32 / 2., fill, None)?;
        }
        if shown {
            left += lead;
            let area = Rect::new(left, button.y(), room.max(1) as u32, button.height());
            self.label(&text, (face, size, ink), area)?;
        }
        Ok(())
    }
}
