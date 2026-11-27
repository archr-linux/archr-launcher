//! Game details (Jogo-detalhes): scraped art, facts, description and actions.
//! Every block sits in a theme rectangle, so a theme can rearrange the screen.
use super::*;
use crate::state::DETAILS_ACTIONS;

/// The line a theme's capsule header gives a game screen's title, in
/// reference pixels: its content lowers by it.
const TITLE_LINE: f32 = 44.;

/// `rect` lowered by `by` pixels, its bottom where it was.
pub(super) fn lowered(rect: Rect, by: i32) -> Rect {
    Rect::new(
        rect.x(),
        rect.y() + by,
        rect.width(),
        (rect.height() as i32 - by).max(1) as u32,
    )
}

/// Decoded art for the details screen; textures stay with their caches.
#[derive(Clone, Copy, Default)]
pub struct Art<'a, 't> {
    pub cover: Option<&'a Texture<'t>>,
    pub shot: Option<&'a Texture<'t>>,
    pub logo: Option<&'a Texture<'t>>,
    /// The playing preview video's frame, already the shot box's size, and
    /// how far through the clip it is (0..1).
    pub video: Option<(&'a Texture<'t>, f32)>,
    /// The clip that frame is of.
    pub video_file: Option<&'a Path>,
    /// Achievement badges decoded so far, by file.
    pub badges: Option<&'a crate::covers::CoverCache<'t>>,
    /// The covers decoded so far, by file: a folder's mosaic.
    pub covers: Option<&'a crate::covers::CoverCache<'t>>,
    /// Save-state pictures and screenshots, small, by file.
    pub thumbs: Option<&'a crate::covers::CoverCache<'t>>,
    /// The selected state's picture, or the screenshot on the whole screen.
    pub picture: Option<&'a Texture<'t>>,
    /// Large pictures by file: the switcher's and the resume screen's.
    pub pictures: Option<&'a crate::covers::CoverCache<'t>>,
}

/// What the details layout shows (Jogo-detalhes), from a game or an app.
pub(super) struct Sheet<'s> {
    pub title: &'s str,
    /// Joined with " · "; the last parts go first when the line is tight.
    pub subtitle: Vec<String>,
    /// Label, value and the value's ink.
    pub facts: Vec<(String, String, Rgba)>,
    pub rating: Option<u8>,
    pub text: &'s str,
    /// In the picture's box while there is no picture.
    pub placeholder: String,
    pub buttons: Vec<Button<'s>>,
    pub focus: usize,
    pub scroll: usize,
    /// A wait shown by the buttons: its steps above them, its bytes below.
    pub transfer: Option<&'s crate::apps::Transfer>,
}
pub(super) struct Button<'s> {
    pub label: String,
    pub icon: Option<Icon<'s>>,
    /// The text of its working state, which its width also holds.
    pub busy: Option<String>,
    pub working: bool,
    pub disabled: bool,
    pub progress: Option<u8>,
}
pub(super) enum Icon<'s> {
    /// The favorite star, amber when the game is a favorite.
    Star(bool),
    /// A module's Lucide SVG.
    Svg(&'s str),
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

    /// "‹ <back>" in a capsule of glass at the top left (the details' hero
    /// and the theme's capsule header); returns the capsule.
    pub(super) fn back_capsule(&mut self, back: &str, theme: &Theme) -> Result<Rect, String> {
        let w = self.frame.width as i32;
        let cap_h = self.px(30.);
        let text_w = self.measure(back, Face::Semibold, 13.)? as i32;
        let chevron = self.px(14.);
        let cap_w = (self.px(12.) + chevron + self.px(4.) + text_w + self.px(14.))
            .min(w / 2)
            .max(cap_h);
        let capsule = Rect::new(self.px(20.), self.px(14.), cap_w as u32, cap_h as u32);
        self.glass(capsule, cap_h as f32 / 2., &theme.glass_or_default())?;
        let glyph = Rect::new(
            capsule.x() + self.px(10.),
            capsule.y() + (cap_h - chevron) / 2,
            chevron as u32,
            chevron as u32,
        );
        self.icon(Glyph::ChevronLeft, glyph, theme.text)?;
        self.label(
            back,
            (Face::Semibold, 13., theme.text),
            Rect::new(
                glyph.right() + self.px(4.),
                capsule.y(),
                (capsule.right() - glyph.right() - self.px(10.)).max(1) as u32,
                cap_h as u32,
            ),
        )?;
        Ok(capsule)
    }

    /// The top of a game's screen. As always, the path in text: "trail /"
    /// and `title`. With the theme's capsule header (`[header] style =
    /// "capsule"`), "‹ <game>" in a capsule, the status at the right and,
    /// when `line`, `title` large on a line of its own under them. Returns
    /// how far that line lowers the screen's content, in pixels.
    pub(super) fn game_header(
        &mut self,
        (trail, game): (&str, &str),
        title: &str,
        line: bool,
        theme: &Theme,
    ) -> Result<i32, String> {
        self.look.outlining = true;
        let drawn = (|| {
            if !theme.screens.header.capsule {
                return self.crumbs(trail, title, theme).map(|()| 0);
            }
            self.back_capsule(game, theme)?;
            self.status_header(theme)?;
            if !line {
                return Ok(0);
            }
            let x = self.px(32.);
            let room = (self.frame.width as i32 - 2 * x).max(1) as u32;
            let floor = self.frame.min_size().max(15.);
            let size = self
                .shrink(title, Face::Bold, 28., floor, room)?
                .unwrap_or(floor);
            self.label(
                title,
                (Face::Bold, size, theme.text),
                Rect::new(x, self.px(52.), room, self.px(36.) as u32),
            )?;
            Ok(self.px(TITLE_LINE))
        })();
        self.look.outlining = false;
        drawn
    }

    /// "Trail /" in muted 13 px, then the current place in 15 px semibold.
    pub(super) fn crumbs(&mut self, trail: &str, game: &str, theme: &Theme) -> Result<(), String> {
        let right = self.status_header(theme)?;
        let h = self.frame.size(theme.header_height).round() as u32;
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
        let details = &game.details;
        let mut facts = Vec::new();
        let mut add = |key, value: &str| {
            if !value.trim().is_empty() {
                facts.push((self.catalog.text(key), value.trim().to_owned(), theme.text));
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
        // System and last played; the date goes before the text ever shrinks away.
        let mut subtitle = vec![system_title(system, data, &self.catalog)];
        if let Some(played) = game.last_played {
            subtitle.push(self.catalog.relative_time(played, self.frame.unix));
        }
        let buttons = self.details_buttons(app, game);
        let sheet = Sheet {
            title: &game.name,
            subtitle,
            facts,
            rating: details.rating,
            text: details.description.trim(),
            placeholder: system_title(system, data, &self.catalog),
            buttons,
            focus: app.details_focus,
            scroll: app.details_scroll,
            transfer: None,
        };
        self.sheet(&sheet, theme, art)
    }

    /// The details' buttons for `game`: Play, Favorite and Scrape again.
    pub(super) fn details_buttons(
        &self,
        app: &App,
        game: &archr_index::Game,
    ) -> Vec<Button<'static>> {
        DETAILS_ACTIONS
            .iter()
            .enumerate()
            .map(|(n, key)| Button {
                label: self.catalog.text(key),
                icon: (n == 1).then_some(Icon::Star(game.favorite)),
                // Sized for both of its states (resting, and dots with
                // "Scraping…"), so a running job never moves the row.
                busy: (n == 2).then(|| self.catalog.text("details.rescraping")),
                working: n == 2 && app.rescraping,
                disabled: n == 2 && (!app.can_rescrape || app.rescraping),
                progress: None,
            })
            .collect()
    }

    /// The details layout over any content: a game's, or an app's (API 4).
    pub(super) fn sheet(
        &mut self,
        sheet: &Sheet<'_>,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
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
                &sheet.placeholder,
                (Face::Regular, 13., theme.dim_text),
                shot,
                Align::Center,
            )?;
        }

        // Facts table: label left, value right; values shrink, then wrap.
        let info = rect(theme.details_info);
        let mut y = info.y();
        // Lines follow the text size, which a theme may scale.
        let line = (f32::from(self.frame.font_size(12.)) * 1.25)
            .round()
            .max(1.) as i32;
        let pad = self.px(3.);
        for (label, value, ink) in &sheet.facts {
            let (value, ink) = (value.clone(), *ink);
            let lw = self.measure(label, Face::Regular, 12.)?;
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
                label,
                (Face::Regular, 12., theme.muted),
                Rect::new(info.x(), y + pad, lw.max(1), line as u32),
            )?;
            for (n, text) in lines.iter().enumerate() {
                self.label_aligned(
                    text,
                    (Face::Regular, size, ink),
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
        if let Some(rating) = sheet.rating.filter(|_| y + line + pad * 2 <= info.bottom()) {
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
                .shrink(sheet.title, Face::Bold, 26., 18., title.width())?
                .unwrap_or(18.);
            self.label(sheet.title, (Face::Bold, size, theme.text), title)?;
        }

        let sub = rect(theme.details_subtitle);
        let mut parts = sheet.subtitle.clone();
        while !parts.is_empty() {
            let text = parts.join(" · ");
            if let Some(size) = self.shrink(&text, Face::Regular, 12., 10., sub.width())? {
                self.label(&text, (Face::Regular, size, theme.muted), sub)?;
                break;
            }
            if parts.len() <= 1 {
                let lines = self.wrap(&text, Face::Regular, 10., sub.width())?;
                if let Some(line) = lines.first() {
                    self.label(line, (Face::Regular, 10., theme.muted), sub)?;
                }
                break;
            }
            parts.pop();
        }

        let (mut text, mut area) = (rect(theme.details_text), rect(theme.details_actions));
        // A wait (Progresso-itch-baixando): its steps, the buttons, then its
        // bytes, ending where the buttons did; the text gives up the room.
        let below = sheet.transfer.map(|transfer| {
            let (line, gap) = (self.px(18.), self.px(6.));
            let buttons = area.y() - line - gap;
            let steps = Rect::new(area.x(), buttons - gap - line, area.width(), line as u32);
            let bottom = (steps.y() - gap).max(text.y() + 1);
            text.set_height((bottom - text.y()) as u32);
            area.set_y(buttons);
            let bytes = Rect::new(area.x(), area.bottom() + gap, area.width(), line as u32);
            (transfer, steps, bytes)
        });
        self.details_text(sheet.text, sheet.scroll, text, theme)?;
        if let Some((transfer, steps, bytes)) = below {
            self.details_steps(transfer, steps, theme)?;
            self.details_bytes(transfer, bytes, theme)?;
        }
        self.details_actions((&sheet.buttons, sheet.focus), area, theme)
    }

    /// The wait's steps in a row: done and running lit, numbered.
    fn details_steps(
        &mut self,
        transfer: &crate::apps::Transfer,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let disc = self.px(16.);
        let mut x = area.x();
        for (n, step) in transfer.steps.iter().enumerate() {
            let lit = n <= transfer.step;
            let at = Rect::new(
                x,
                area.y() + (area.height() as i32 - disc) / 2,
                disc as u32,
                disc as u32,
            );
            let fill = if lit {
                theme.highlight_bg
            } else {
                theme.border
            };
            self.rounded(at, disc as f32 / 2., fill, None)?;
            let ink = if lit {
                theme.highlight_text
            } else {
                theme.muted
            };
            self.label_aligned(
                &(n + 1).to_string(),
                (Face::Semibold, 9., ink),
                at,
                Align::Center,
            )?;
            x += disc + self.px(6.);
            let ink = if n == transfer.step {
                theme.text
            } else {
                theme.muted
            };
            let width = self.measure(step, Face::Semibold, 12.)? as i32;
            let room = (area.right() - x).max(1) as u32;
            self.label(
                step,
                (Face::Semibold, 12., ink),
                Rect::new(x, area.y(), room, area.height()),
            )?;
            x += width + self.px(18.);
            if x >= area.right() {
                break;
            }
        }
        Ok(())
    }

    /// "15,7 de 27,0 MB · 1,8 MB/s" on the left, "faltam ~6 s" on the right.
    fn details_bytes(
        &mut self,
        transfer: &crate::apps::Transfer,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        if transfer.now.done == 0 && transfer.now.total == 0 {
            return Ok(());
        }
        let (text, left) = crate::progress::describe(&self.catalog, transfer.now);
        let mut room = area.width();
        if let Some(left) = left {
            let width = self.measure(&left, Face::Regular, 12.)?;
            let at = Rect::new(area.right() - width as i32, area.y(), width, area.height());
            self.label(&left, (Face::Regular, 12., theme.muted), at)?;
            room = room.saturating_sub(width + self.px(12.) as u32);
        }
        let at = Rect::new(area.x(), area.y(), room.max(1), area.height());
        self.label(&text, (Face::Regular, 12., theme.muted), at)
    }

    fn details_text(
        &mut self,
        text: &str,
        scroll: usize,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
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
        let first = scroll.min(self.details_overflow);
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

    /// A row of buttons in `area`, `focus` lit.
    pub(super) fn details_actions(
        &mut self,
        (buttons, focus): (&[Button<'_>], usize),
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let labels: Vec<&String> = buttons.iter().map(|b| &b.label).collect();
        // Shrink text and padding together until the buttons fit.
        // The icons go before a word is cut: a row of three buttons keeps
        // its words whole (Progresso-itch-1024 has none).
        let mut icons = true;
        let icon = |n: usize, icons: bool| icons && buttons[n].icon.is_some();
        let mut size = 14.;
        // Never under 12 px on screen: past that the labels are cut ("…").
        let floor = self.frame.min_size().max(12. / self.frame.font.max(1.));
        let mut gap = self.px(8.);
        let between = labels.len().saturating_sub(1) as i32;
        let (mut widths, mut pad) = loop {
            let pad = self.px(size + 1.);
            let mut widths = Vec::new();
            for (n, label) in labels.iter().enumerate() {
                let extra = if icon(n, icons) {
                    self.px(size + 6.)
                } else {
                    0
                };
                // Drawn semibold when focused, regular otherwise; small
                // hinted sizes can make either one the wider.
                let mut text = self.measure(label, Face::Semibold, size)?.max(self.measure(
                    label,
                    Face::Regular,
                    size,
                )?) as i32;
                if let Some(busy) = &buttons[n].busy {
                    let busy = self.measure(busy, Face::Semibold, size)? as i32;
                    text = text.max(busy + self.px(size * 1.8 + 6.));
                }
                widths.push(text + pad * 2 + extra);
            }
            let total: i32 = widths.iter().sum::<i32>() + gap * between;
            // The floor: 10 px on screen and the theme's smallest text.
            if total <= area.width() as i32 {
                break (widths, pad);
            }
            if size <= floor {
                // Cut at the smallest text even with its icons: without
                // them, from the largest again.
                if icons && buttons.iter().any(|b| b.icon.is_some()) {
                    (icons, size) = (false, 14.);
                    continue;
                }
                break (widths, pad);
            }
            size -= 1.;
        };
        // Still too wide at the floor: a button that fits its share keeps
        // its width, the room left is shared by the others, and a label
        // that still does not fit stays on one line, cut with "…"
        // (Progresso-itch-1024).
        let total: i32 = widths.iter().sum::<i32>() + gap * between;
        if total > area.width() as i32 {
            let tighter = self.px(8.);
            widths = widths.iter().map(|w| w - 2 * (pad - tighter)).collect();
            (pad, gap) = (tighter, self.px(6.));
            widths = share(&widths, area.width() as i32 - gap * between);
        }
        let mut x = area.x();
        for (n, (label, button)) in labels.iter().zip(buttons).enumerate() {
            let focused = n == focus;
            let rect = Rect::new(x, area.y(), widths[n] as u32, area.height());
            if focused {
                if !self.framed("button_selected", rect)? {
                    self.rounded(rect, self.px(10.) as f32, theme.highlight_bg, None)?;
                }
            } else {
                let colors = (theme.surface, theme.border);
                self.panel_named("button", rect, self.px(10.) as f32, colors)?;
            }
            // Work in progress fills the button from the left.
            if let Some(progress) = button.progress {
                let done = (rect.width() as f32 * f32::from(progress.min(100)) / 100.).round();
                if done >= 1. {
                    let fill = Rgba {
                        a: 72,
                        ..selection(focused, theme.highlight_bg, theme.highlight_text)
                    };
                    let bar = Rect::new(rect.x(), rect.y(), done as u32, rect.height());
                    self.rounded(bar, self.px(10.) as f32, fill, None)?;
                }
            }
            let ink = match (focused, button.disabled) {
                (true, _) => theme.highlight_text,
                (false, true) => theme.dim_text,
                (false, false) => theme.text,
            };
            let face = if focused {
                Face::Semibold
            } else {
                Face::Regular
            };
            if let (true, Some(busy)) = (button.working, &button.busy) {
                self.scraping(rect, (busy, (face, size, ink)), pad, focused, theme)?;
                x += widths[n] + gap;
                continue;
            }
            let mut text_x = rect.x() + pad;
            if let Some(glyph) = button.icon.as_ref().filter(|_| icons) {
                let side = self.px(size);
                let at = Rect::new(
                    text_x,
                    rect.y() + (rect.height() as i32 - side) / 2,
                    side as u32,
                    side as u32,
                );
                match glyph {
                    Icon::Star(favorite) => {
                        let star = if *favorite { theme.warning } else { ink };
                        self.icon(Glyph::Star, at, star)?;
                    }
                    Icon::Svg(svg) => self.module_icon((Some(svg), ""), at, ink)?,
                }
                text_x += side + self.px(6.);
            }
            let room = (rect.right() - text_x - pad.min(text_x - rect.x())).max(1) as u32;
            self.label(
                label,
                (face, size, ink),
                Rect::new(text_x, rect.y(), room, rect.height()),
            )?;
            x += widths[n] + gap;
        }
        Ok(())
    }

    /// "Scrape again" while its job runs: the Wi-Fi search's pulsing dots,
    /// then "Scraping…" when it fits on one line; the dots alone, centered,
    /// when it does not. Looping state comes from the clock, never stored.
    pub(super) fn scraping(
        &mut self,
        button: Rect,
        (text, (face, size, ink)): (&str, (Face, f32, Rgba)),
        pad: i32,
        focused: bool,
        theme: &Theme,
    ) -> Result<(), String> {
        // The dots take 1.8 × the text size, as measured for the row.
        let (dot, step) = (self.px(size * 0.4).max(2), self.px(size * 0.7));
        let dots = step * 2 + dot;
        let lead = self.px(size * 1.8 + 6.);
        let room = button.width() as i32 - pad * 2 - lead;
        let shown = self.measure(text, face, size)? as i32 <= room;
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
            self.label(text, (face, size, ink), area)?;
        }
        Ok(())
    }
}

/// Widths that fit `room`: the narrow ones keep theirs, the rest share
/// what is left equally.
fn share(wanted: &[i32], room: i32) -> Vec<i32> {
    let mut fixed = vec![false; wanted.len()];
    loop {
        let left = wanted.len() - fixed.iter().filter(|f| **f).count();
        let kept: i32 = wanted
            .iter()
            .zip(&fixed)
            .filter(|(_, f)| **f)
            .map(|(w, _)| w)
            .sum();
        let each = (room - kept) / left.max(1) as i32;
        let mut changed = false;
        for (n, w) in wanted.iter().enumerate() {
            if !fixed[n] && *w <= each {
                fixed[n] = true;
                changed = true;
            }
        }
        if !changed || left == 0 {
            return wanted
                .iter()
                .zip(&fixed)
                .map(|(w, f)| if *f { *w } else { each.max(1) })
                .collect();
        }
    }
}

#[cfg(test)]
mod share_tests {
    #[test]
    fn narrow_buttons_keep_their_width_and_the_rest_share() {
        assert_eq!(super::share(&[120, 160, 70], 330), [120, 140, 70]);
        assert_eq!(super::share(&[100, 100], 300), [100, 100]);
        assert_eq!(super::share(&[300, 300, 300], 300), [100, 100, 100]);
    }
}
