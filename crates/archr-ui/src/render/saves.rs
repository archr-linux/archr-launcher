//! A game's save states (Saves) and screenshots (Screenshots, and one over
//! the whole screen), with the question before playing or deleting. The
//! list, the card and the grid sit in theme rectangles (`saves_*`,
//! `shots_grid`), like the details.
use super::details::fit;
use super::*;
use crate::{
    primitives::crop,
    saves::{Action, Blocked, Kind, Save},
};

/// Saves rows, and the screenshot cells (4:3 near 138×104) with their gap,
/// at the 640×480 reference.
const ROW: f32 = 62.;
const ROW_GAP: f32 = 4.;
const CELL: f32 = 138.;
const CELL_GAP: f32 = 12.;

impl Ui<'_, '_> {
    /// A date in the locale's `key` pattern: {day} {d} {month} {month_name}
    /// {year} {time}.
    fn when(&self, unix: u64, key: &str) -> String {
        let Some(tm) = crate::status::local_time(unix) else {
            return String::new();
        };
        let clock_12 = self.status.as_ref().is_some_and(Status::clock_12);
        let months = self.catalog.text("saves.months");
        let month = usize::try_from(tm.tm_mon).unwrap_or(0);
        self.catalog.format(
            key,
            &[
                ("day", &format!("{:02}", tm.tm_mday)),
                ("d", &tm.tm_mday.to_string()),
                ("month", &format!("{:02}", month + 1)),
                ("month_name", months.split(' ').nth(month).unwrap_or("")),
                ("year", &(tm.tm_year + 1900).to_string()),
                ("time", &crate::status::local_clock(unix, clock_12)),
            ],
        )
    }
    fn slot_name(&self, save: &Save) -> String {
        match save.slot {
            Some(n) => self.catalog.format("saves.slot", &[("n", &n.to_string())]),
            None => self.catalog.text("saves.auto"),
        }
    }
    /// A picture filling `rect` (cropped), or the image icon on a plain box.
    fn filled(
        &mut self,
        texture: Option<&Texture<'_>>,
        rect: Rect,
        radius: f32,
        theme: &Theme,
    ) -> Result<(), String> {
        let radius = self.px(radius) as f32;
        if let Some(texture) = texture {
            let size = texture.query();
            let part = crop((size.width, size.height), (rect.width(), rect.height()));
            return self
                .geometry
                .cover(&mut self.canvas, texture, rect, radius, 255, part);
        }
        self.rounded(rect, radius, theme.surface, Some(theme.border))?;
        let side = (rect.height() as f32 * 0.4).round().max(1.) as u32;
        let icon = Rect::new(
            rect.x() + (rect.width() - side) as i32 / 2,
            rect.y() + (rect.height() - side) as i32 / 2,
            side,
            side,
        );
        self.icon(Glyph::Image, icon, theme.dim_text)
    }
    /// `rect` shaded from `top` to `bottom`, a line at a time.
    fn veil(&mut self, rect: Rect, top: Rgba, bottom: Rgba) -> Result<(), String> {
        let h = rect.height().max(1);
        for line in 0..h {
            let t = line as f32 / (h - 1).max(1) as f32;
            let a = (f32::from(top.a) + (f32::from(bottom.a) - f32::from(top.a)) * t).round();
            self.canvas.set_draw_color(color(Rgba {
                a: a as u8,
                ..bottom
            }));
            self.canvas
                .fill_rect(Rect::new(rect.x(), rect.y() + line as i32, rect.width(), 1))?;
        }
        Ok(())
    }
    /// The bottom bar: a summary at the left, the buttons at the right.
    fn progress_footer(
        &mut self,
        summary: &str,
        hints: &[(HintButton, &str)],
        theme: &Theme,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let h = (theme.footer_height * height as f32).round().max(1.) as u32;
        let y = height as i32 - h as i32;
        if !self.has_frame("footer") {
            self.divider(0, y, width, theme.border)?;
        }
        // Outlined like the other footers, over a theme's picture.
        self.look.outlining = true;
        let drawn = self.hints(hints, theme, width, y, h).and_then(|left| {
            self.label(
                summary,
                (Face::Regular, 13., theme.muted),
                Rect::new(self.px(24.), y, (left - self.px(40.)).max(1) as u32, h),
            )
        });
        self.look.outlining = false;
        drawn
    }
    fn progress_crumbs(
        &mut self,
        (system, game): (&str, usize),
        key: &str,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let name = data.games(system).get(game).map_or("", |g| g.name.as_str());
        let trail = format!("{} / {name}", system_title(system, data, &self.catalog));
        let current = self.catalog.text(key);
        self.look.outlining = true;
        let drawn = self.crumbs(&trail, &current, theme);
        self.look.outlining = false;
        drawn
    }

    /// The game menu's value for Saves ("5 · last 2 h ago") and Screenshots.
    pub(super) fn progress_value(&self, app: &App, item: QuickItem) -> Option<(String, String)> {
        let progress = &app.progress;
        match item {
            QuickItem::Saves => {
                let count = progress.saves.len().to_string();
                let ago = progress
                    .latest()
                    .map(|at| self.catalog.relative_time(at, self.frame.unix))
                    .unwrap_or_default();
                let full = self
                    .catalog
                    .format("menu.saves_value", &[("count", &count), ("ago", &ago)]);
                Some((full, count))
            }
            QuickItem::Shots => {
                let count = progress.shots.len().to_string();
                Some((count.clone(), count))
            }
            _ => None,
        }
    }

    pub(super) fn saves(
        &mut self,
        app: &App,
        game: (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        self.progress_crumbs(game, "menu.saves", data, theme)?;
        let Frame { width, height, .. } = self.frame;
        let saves = &app.progress.saves;
        let list = pixels(theme.saves_list, width, height);
        let card = pixels(theme.saves_card, width, height);
        let (row, gap) = (self.px(ROW), self.px(ROW_GAP));
        let visible = ((list.height() as i32 + gap) / (row + gap)).max(1) as usize;
        self.saves_page = visible;
        let first = app.cursor / visible * visible;
        for (n, save) in saves.iter().enumerate().skip(first).take(visible) {
            let y = list.y() + (n - first) as i32 * (row + gap);
            let rect = Rect::new(list.x(), y, list.width(), row as u32);
            let selected = n == app.cursor;
            if selected && !self.framed("selected", rect)? {
                let radius = theme.radius_item * height as f32;
                self.rounded(rect, radius, theme.highlight_bg, None)?;
            }
            let thumb = Rect::new(
                rect.x() + self.px(6.),
                y + (row - self.px(48.)) / 2,
                self.px(64.) as u32,
                self.px(48.) as u32,
            );
            let texture = save
                .picture
                .as_ref()
                .and_then(|path| art.thumbs.and_then(|cache| cache.peek(path)));
            self.filled(texture, thumb, 5., theme)?;
            let soft = selection(selected, theme.dim_text, theme.accent_text_secondary);
            let ago = self.catalog.relative_time(save.saved, self.frame.unix);
            let x = thumb.right() + self.px(12.);
            let end = rect.right() - self.px(10.);
            // The date never loses characters: it shrinks, and when it
            // still does not fit beside the age the age gives way.
            let date = self.when(save.saved, "saves.date");
            let floor = self.frame.min_size();
            let age_w = self.measure(&ago, Face::Regular, 12.)? as i32;
            let beside = (end - age_w - self.px(8.) - x).max(1) as u32;
            let (room, date_size) = match self.shrink(&date, Face::Mono, 12., floor, beside)? {
                Some(size) => {
                    let style = (Face::Regular, 12., soft);
                    self.packed_label(&ago, style, (end, y, row as u32), u32::MAX, Align::Left)?;
                    (beside, size)
                }
                None => {
                    let room = (end - x).max(1) as u32;
                    let size = self.shrink(&date, Face::Mono, 12., floor, room)?;
                    (room, size.unwrap_or(floor))
                }
            };
            let ink = selection(selected, theme.text, theme.highlight_text);
            let face = selection(selected, Face::Regular, Face::Semibold);
            let title = self.slot_name(save);
            let line = Rect::new(x, y + self.px(11.), room, self.px(20.) as u32);
            let title_w = self.measure(&title, face, 15.)?.min(room);
            self.label(&title, (face, 15., ink), line)?;
            let note = self.catalog.text("saves.on_exit");
            let note_w = self.measure(&note, Face::Regular, 11.)? as i32;
            // The automatic state says when it is written, where that fits.
            if save.slot.is_none() && title_w as i32 + self.px(23.) + note_w <= room as i32 {
                let icon_x = x + title_w as i32 + self.px(8.);
                let icon = self.frame.square(icon_x, line.y() + self.px(4.), 11.);
                self.icon(Glyph::Recent, icon, soft)?;
                let note_x = icon.right() + self.px(4.);
                let note_w = (x + room as i32 - note_x).max(1) as u32;
                self.label(
                    &note,
                    (Face::Regular, 11., soft),
                    Rect::new(note_x, line.y(), note_w, line.height()),
                )?;
            }
            let muted = selection(selected, theme.muted, theme.accent_text_secondary);
            self.label(
                &date,
                (Face::Mono, date_size, muted),
                Rect::new(x, y + self.px(34.), room, self.px(16.) as u32),
            )?;
        }

        // The selected state: its picture, facts and what playing it does.
        let selected = saves.get(app.cursor);
        let shot = Rect::new(
            card.x(),
            card.y(),
            card.width(),
            (card.width() * 3 / 4).min(card.height()),
        );
        self.rounded(shot, self.px(10.) as f32, theme.surface, Some(theme.border))?;
        let inner = Rect::new(
            shot.x() + 1,
            shot.y() + 1,
            shot.width().saturating_sub(2).max(1),
            shot.height().saturating_sub(2).max(1),
        );
        self.filled(art.picture, inner, 9., theme)?;
        let mut y = shot.bottom() + self.px(10.);
        if let Some(save) = selected {
            let facts = [
                (
                    "saves.saved_at",
                    self.when(save.saved, "saves.date_long"),
                    Face::Regular,
                ),
                ("saves.emulator", save.emulator.clone(), Face::Regular),
                ("saves.size", self.catalog.size(save.bytes), Face::Mono),
            ];
            let line = self.px(27.);
            for (key, value, face) in facts {
                if y + line > card.bottom() {
                    break;
                }
                let label = self.catalog.text(key);
                let lw = self.measure(&label, Face::Regular, 13.)?;
                let room = card.width().saturating_sub(lw + self.px(12.) as u32).max(1);
                let floor = self.frame.min_size();
                let size = self
                    .shrink(&value, face, 13., floor, room)?
                    .unwrap_or(floor);
                let band = Rect::new(card.x(), y, card.width(), (line - 1) as u32);
                self.label(&label, (Face::Regular, 13., theme.muted), band)?;
                self.label_aligned(&value, (face, size, theme.text), band, Align::Right)?;
                self.divider(card.x(), y + line - 1, card.width(), theme.border)?;
                y += line;
            }
            let note = match (save.kind, save.slot) {
                _ if !save.playable => self.catalog.text("saves.note_other"),
                (Kind::RetroArch, _) if app.progress.blocked == Some(Blocked::Hardcore) => {
                    self.catalog.text("saves.note_hardcore")
                }
                (Kind::RetroArch, _) if app.progress.blocked == Some(Blocked::Netplay) => {
                    self.catalog.text("saves.note_netplay")
                }
                (Kind::RetroArch, Some(n)) => self
                    .catalog
                    .format("saves.note_slot", &[("n", &n.to_string())]),
                (Kind::RetroArch, None) => self.catalog.text("saves.note_auto"),
                (Kind::Standalone, _) => self.catalog.text("saves.note_state"),
            };
            let lines = self.wrap(&note, Face::Regular, 12., card.width())?;
            let step = self.px(17.);
            y += self.px(8.);
            for text in lines {
                if y + step > card.bottom() {
                    break;
                }
                let band = Rect::new(card.x(), y, card.width(), step as u32);
                self.label(&text, (Face::Regular, 12., theme.muted), band)?;
                y += step;
            }
        }

        let total: u64 = saves.iter().map(|s| s.bytes).sum();
        let summary = self.catalog.plural(
            "saves.total",
            saves.len() as f64,
            &[
                ("count", &saves.len().to_string()),
                ("size", &self.catalog.size(total)),
            ],
        );
        let mut hints: Vec<(HintButton, &str)> = Vec::new();
        if selected.is_some_and(|save| app.can_play(save)) {
            hints.push(('A'.into(), "hint.play_here"));
        }
        hints.extend([('X'.into(), "hint.delete"), ('B'.into(), "hint.back")]);
        self.progress_footer(&summary, &hints, theme)
    }

    pub(super) fn shots(
        &mut self,
        app: &App,
        game: (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        self.progress_crumbs(game, "menu.shots", data, theme)?;
        let Frame { width, height, .. } = self.frame;
        let shots = &app.progress.shots;
        let grid = pixels(theme.shots_grid, width, height);
        let gap = self.px(CELL_GAP);
        // As many cells near the reference width as fit, stretched to fill.
        let columns = ((grid.width() as i32 + gap) / (self.px(CELL) + gap)).max(1);
        let cell_w = ((grid.width() as i32 - gap * (columns - 1)) / columns).max(1);
        let cell_h = (cell_w * 104 / 138).max(1);
        let rows = ((grid.height() as i32 + gap) / (cell_h + gap)).max(1);
        let page = (columns * rows) as usize;
        self.shots_grid = (columns as usize, page);
        let first = app.cursor / page * page;
        let radius = self.px(8.) as f32;
        for (n, shot) in shots.iter().enumerate().skip(first).take(page) {
            let k = (n - first) as i32;
            let cell = Rect::new(
                grid.x() + k % columns * (cell_w + gap),
                grid.y() + k / columns * (cell_h + gap),
                cell_w as u32,
                cell_h as u32,
            );
            let grow = |r: Rect, by: i32| {
                Rect::new(
                    r.x() - by,
                    r.y() - by,
                    (r.width() as i32 + 2 * by) as u32,
                    (r.height() as i32 + 2 * by) as u32,
                )
            };
            let selected = n == app.cursor;
            if selected {
                let glow = Rgba {
                    a: 64,
                    ..theme.highlight_bg
                };
                let ring = self.px(5.);
                self.rounded(grow(cell, ring), radius + ring as f32, glow, None)?;
                let edge = self.px(2.);
                self.rounded(
                    grow(cell, edge),
                    radius + edge as f32,
                    theme.highlight_bg,
                    None,
                )?;
            } else {
                self.rounded(grow(cell, 1), radius + 1., theme.border, None)?;
            }
            let texture = art.thumbs.and_then(|cache| cache.peek(&shot.path));
            self.filled(texture, cell, 8., theme)?;
            let strip = self.px(26.);
            let band = Rect::new(cell.x(), cell.bottom() - strip, cell.width(), strip as u32);
            let shade = Rgba {
                a: 217,
                ..theme.background
            };
            self.veil(band, Rgba { a: 0, ..shade }, shade)?;
            let ink = selection(selected, theme.text_secondary, theme.text);
            let room = cell.width().saturating_sub(self.px(16.) as u32).max(1);
            let floor = self.frame.min_size();
            let mut date = self.when(shot.taken, "saves.date_short");
            let mut size = self.shrink(&date, Face::Mono, 11., floor, room)?;
            if size.is_none() {
                // The day alone rather than a cut time.
                date = self.when(shot.taken, "saves.day_short");
                size = self.shrink(&date, Face::Mono, 11., floor, room)?;
            }
            let h = self
                .px(14.)
                .max(f32::from(self.frame.font_size(size.unwrap_or(floor))) as i32);
            self.label(
                &date,
                (Face::Mono, size.unwrap_or(floor), ink),
                Rect::new(
                    cell.x() + self.px(8.),
                    cell.bottom() - self.px(5.) - h,
                    room,
                    h as u32,
                ),
            )?;
            if shot.details {
                let at = (cell.x() + self.px(6.), cell.y() + self.px(6.));
                self.details_chip(at, "shots.in_details", 11., theme)?;
            }
        }
        let total: u64 = shots.iter().map(|s| s.bytes).sum();
        let summary = self.catalog.format(
            "shots.position",
            &[
                ("n", &(app.cursor + 1).min(shots.len()).to_string()),
                ("total", &shots.len().to_string()),
                ("size", &self.catalog.size(total)),
            ],
        );
        let hints = [
            ('A'.into(), "hint.open"),
            ('X'.into(), "hint.delete"),
            ('B'.into(), "hint.back"),
        ];
        self.progress_footer(&summary, &hints, theme)
    }

    /// The "used on the details screen" tag: image icon and text on a pill.
    fn details_chip(
        &mut self,
        (x, y): (i32, i32),
        key: &str,
        size: f32,
        theme: &Theme,
    ) -> Result<Rect, String> {
        let text = self.catalog.text(key);
        let (icon, gap, pad) = (self.px(12.), self.px(4.), self.px(7.));
        let tw = self.measure(&text, Face::Regular, size)? as i32;
        let h = self.px(if size > 11. { 24. } else { 20. });
        let chip = Rect::new(x, y, (pad * 2 + icon + gap + tw) as u32, h as u32);
        let veil = Rgba {
            a: 209,
            ..theme.background
        };
        self.rounded(chip, h as f32 / 2., veil, Some(theme.border))?;
        let glyph = Rect::new(
            chip.x() + pad,
            chip.y() + (h - icon) / 2,
            icon as u32,
            icon as u32,
        );
        self.icon(Glyph::Image, glyph, theme.highlight_bg)?;
        self.label(
            &text,
            (Face::Regular, size, theme.text),
            Rect::new(
                glyph.right() + gap,
                chip.y(),
                tw.max(1) as u32,
                chip.height(),
            ),
        )?;
        Ok(chip)
    }

    /// One screenshot over the whole screen, its date and place, the buttons.
    pub(super) fn shot(
        &mut self,
        app: &App,
        (system, game): (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let screen = Rect::new(0, 0, width, height);
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.fill_rect(screen)?;
        if let Some(texture) = art.picture {
            self.canvas
                .copy(texture, None, fit(texture, screen, false))?;
        }
        let shots = &app.progress.shots;
        let Some(shot) = shots.get(app.cursor) else {
            return Ok(());
        };
        // A plate under the title and date: readable over any picture.
        let plate = Rgba {
            a: (f32::from(theme.scrim.a) * 1.2).round().min(255.) as u8,
            ..theme.scrim
        };
        self.canvas.set_draw_color(color(plate));
        self.canvas
            .fill_rect(Rect::new(0, 0, width, self.px(58.) as u32))?;
        let name = data.games(system).get(game).map_or("", |g| g.name.as_str());
        let counter = format!("{} / {}", app.cursor + 1, shots.len());
        let cw = self.measure(&counter, Face::Mono, 12.)? + self.px(20.) as u32;
        let pill = Rect::new(
            width as i32 - self.px(20.) - cw as i32,
            self.px(12.),
            cw,
            self.px(24.) as u32,
        );
        self.rounded(
            pill,
            pill.height() as f32 / 2.,
            Rgba {
                a: 204,
                ..theme.background
            },
            Some(theme.border),
        )?;
        self.label_aligned(&counter, (Face::Mono, 12., theme.text), pill, Align::Center)?;
        let room = (pill.x() - self.px(32.)).max(1) as u32;
        self.label(
            name,
            (Face::Semibold, 15., theme.text),
            Rect::new(self.px(20.), self.px(12.), room, self.px(20.) as u32),
        )?;
        let mut line = self.when(shot.taken, "saves.date_long");
        if let Some((w, h)) = shot.size {
            line = format!("{line} · {w}×{h}");
        }
        self.label(
            &line,
            (Face::Mono, 12., theme.text_secondary),
            Rect::new(self.px(20.), self.px(34.), room, self.px(16.) as u32),
        )?;
        let footer = (theme.footer_height * height as f32).round().max(1.) as u32;
        let y = height as i32 - footer as i32;
        if shot.details {
            let at = (self.px(20.), y - self.px(36.));
            self.details_chip(at, "shots.details_chip", 12., theme)?;
        }
        self.canvas.set_draw_color(color(Rgba {
            a: 224,
            ..theme.background
        }));
        self.canvas.fill_rect(Rect::new(0, y, width, footer))?;
        self.divider(0, y, width, theme.border)?;
        let mut hints: Vec<(HintButton, &str)> = Vec::new();
        if !shot.details {
            hints.push(('Y'.into(), "hint.use_details"));
        }
        hints.extend([('X'.into(), "hint.delete"), ('B'.into(), "hint.back")]);
        let left = self.hints(&hints, theme, width, y, footer)?;
        // ‹ › at the left edge: the pair is two 20px badges and a slash.
        let browse = [(HintButton::Pair('‹', '›'), "hint.browse")];
        let label = self.catalog.text("hint.browse");
        let w = self.measure(&label, Face::Regular, 13.)? as i32 + self.px(56.);
        if self.px(24.) + w < left - self.px(16.) {
            let end = (self.px(48.) + w) as u32;
            self.hints(&browse, theme, end, y, footer)?;
        }
        Ok(())
    }

    /// Play from a state, or delete a state or a screenshot: the item, what
    /// happens, Cancel and the action (Saves-apagar).
    pub(super) fn progress_question(
        &mut self,
        app: &App,
        (system, game): (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Some(confirm) = &app.confirm else {
            return Ok(());
        };
        let progress = &app.progress;
        let save = progress.saves.iter().find(|s| s.path == confirm.path);
        let shot = progress.shots.iter().find(|s| s.path == confirm.path);
        let (question, detail, picture, taken) = match (confirm.action, save, shot) {
            (Action::Play, Some(save), _) => {
                let question = match save.slot {
                    Some(n) => self
                        .catalog
                        .format("saves.play_question", &[("n", &n.to_string())]),
                    None => self.catalog.text("saves.play_auto_question"),
                };
                let detail = match (!save.backup.is_empty(), save.kind) {
                    (false, _) => "saves.play_detail",
                    (true, Kind::RetroArch) => "saves.play_detail_srm",
                    (true, Kind::Standalone) => "saves.play_detail_backup",
                };
                (
                    question,
                    self.catalog.text(detail),
                    save.picture.as_ref(),
                    save.saved,
                )
            }
            (Action::DeleteSave, Some(save), _) => {
                let question = match save.slot {
                    Some(n) => self
                        .catalog
                        .format("saves.delete_question", &[("n", &n.to_string())]),
                    None => self.catalog.text("saves.delete_auto_question"),
                };
                let detail = self.catalog.text("saves.delete_detail");
                (question, detail, save.picture.as_ref(), save.saved)
            }
            (Action::DeleteShot, _, Some(shot)) => (
                self.catalog.text("shots.delete_question"),
                self.catalog.text("shots.delete_detail"),
                Some(&shot.path),
                shot.taken,
            ),
            _ => return Ok(()),
        };
        let play = confirm.action == Action::Play;
        let Frame { width, height, .. } = self.frame;
        let footer = (theme.footer_height * height as f32).round().max(1.) as u32;
        self.scrim(theme, 1.2)?;
        let w = self.px(360.).min(width as i32 - self.px(32.)).max(1);
        let pad = self.px(20.);
        let inner = (w - 2 * pad).max(1) as u32;
        let badge = self.px(36.);
        let title_room = (inner as i32 - badge - self.px(12.)).max(1) as u32;
        // The question shrinks a little before it wraps.
        let title_size = self
            .shrink(&question, Face::Semibold, 17., 15., title_room)?
            .unwrap_or(17.);
        let titles = self.wrap(&question, Face::Semibold, title_size, title_room)?;
        let title_step = self.px(22.);
        let title_h = (title_step * titles.len() as i32).max(badge);
        let body = self.wrap(&detail, Face::Regular, 13., inner)?;
        let body_step = self.px(19.);
        let gap = self.px(14.);
        let item_h = self.px(70.);
        let button_h = self.px(40.);
        let h = self.px(22.)
            + title_h
            + gap
            + item_h
            + gap
            + body_step * body.len() as i32
            + gap
            + self.px(2.)
            + button_h
            + self.px(18.);
        let room = height as i32 - footer as i32;
        let panel = Rect::new(
            (width as i32 - w) / 2,
            ((room - h) / 2).max(0),
            w as u32,
            h as u32,
        );
        self.panel(
            panel,
            self.px(16.) as f32,
            theme.surface_elevated,
            theme.border,
        )?;
        let x = panel.x() + pad;
        let mut y = panel.y() + self.px(22.);
        let accent = if play {
            theme.highlight_bg
        } else {
            theme.danger
        };
        let circle = Rect::new(x, y + (title_h - badge) / 2, badge as u32, badge as u32);
        self.rounded(circle, badge as f32 / 2., Rgba { a: 36, ..accent }, None)?;
        let glyph = if play { Glyph::Play } else { Glyph::Trash };
        let side = self.px(18.);
        let icon = Rect::new(
            circle.x() + (badge - side) / 2,
            circle.y() + (badge - side) / 2,
            side as u32,
            side as u32,
        );
        self.icon(glyph, icon, accent)?;
        let text_x = circle.right() + self.px(12.);
        let text_top = y + (title_h - title_step * titles.len() as i32) / 2;
        for (k, line) in titles.iter().enumerate() {
            self.label(
                line,
                (Face::Semibold, title_size, theme.text),
                Rect::new(
                    text_x,
                    text_top + k as i32 * title_step,
                    title_room,
                    title_step as u32,
                ),
            )?;
        }
        y += title_h + gap;

        // The item: its picture, the game and when.
        let item = Rect::new(x, y, inner, item_h as u32);
        self.rounded(
            item,
            self.px(10.) as f32,
            theme.background,
            Some(theme.border),
        )?;
        let thumb = Rect::new(
            item.x() + self.px(10.),
            item.y() + (item_h - self.px(48.)) / 2,
            self.px(64.) as u32,
            self.px(48.) as u32,
        );
        let texture = picture.and_then(|path| art.thumbs.and_then(|cache| cache.peek(path)));
        self.filled(texture, thumb, 5., theme)?;
        let tx = thumb.right() + self.px(12.);
        let tw = (item.right() - self.px(10.) - tx).max(1) as u32;
        let name = data.games(system).get(game).map_or("", |g| g.name.as_str());
        let middle = item.y() + item_h / 2;
        self.label(
            name,
            (Face::Regular, 14., theme.text),
            Rect::new(tx, middle - self.px(19.), tw, self.px(19.) as u32),
        )?;
        self.label(
            &self.when(taken, "saves.date"),
            (Face::Mono, 12., theme.muted),
            Rect::new(tx, middle + self.px(2.), tw, self.px(16.) as u32),
        )?;
        y += item_h + gap;
        for line in &body {
            self.label(
                line,
                (Face::Regular, 13., theme.muted),
                Rect::new(x, y, inner, body_step as u32),
            )?;
            y += body_step;
        }
        y += gap + self.px(2.);

        // Cancel, then the action; the focused one is filled.
        let half = ((inner as i32 - self.px(10.)) / 2).max(1) as u32;
        let radius = self.px(10.) as f32;
        let action_key = if play { "hint.play" } else { "hint.delete" };
        for (n, key) in ["hint.cancel", action_key].into_iter().enumerate() {
            let rect = Rect::new(
                x + n as i32 * (half as i32 + self.px(10.)),
                y,
                half,
                button_h as u32,
            );
            let focused = (n == 1) == confirm.on_action;
            let (fill, border, ink, face) = match (focused, n) {
                (true, 0) => (
                    theme.highlight_bg,
                    None,
                    theme.highlight_text,
                    Face::Semibold,
                ),
                (true, _) if play => (
                    theme.highlight_bg,
                    None,
                    theme.highlight_text,
                    Face::Semibold,
                ),
                (true, _) => (theme.danger, None, theme.highlight_text, Face::Semibold),
                (false, 0) => (theme.badge, Some(theme.border), theme.text, Face::Regular),
                (false, _) => (theme.badge, Some(theme.border), accent, Face::Regular),
            };
            let frame = if focused { "button_selected" } else { "button" };
            if !self.framed(frame, rect)? {
                self.rounded(rect, radius, fill, border)?;
            }
            let label = self.catalog.text(key);
            let floor = self.frame.min_size();
            let lw_room = rect.width().saturating_sub(self.px(40.) as u32).max(1);
            let size = self
                .shrink(&label, face, 15., floor, lw_room)?
                .unwrap_or(floor);
            if n == 0 {
                self.label_aligned(&label, (face, size, ink), rect, Align::Center)?;
                continue;
            }
            // The action carries its icon before the word.
            let lw = self.measure(&label, face, size)? as i32;
            let side = self.px(16.);
            let start = rect.x() + (rect.width() as i32 - side - self.px(8.) - lw) / 2;
            let glyph_rect = Rect::new(
                start,
                rect.y() + (button_h - side) / 2,
                side as u32,
                side as u32,
            );
            self.icon(glyph, glyph_rect, ink)?;
            self.label(
                &label,
                (face, size, ink),
                Rect::new(
                    glyph_rect.right() + self.px(8.),
                    rect.y(),
                    lw.max(1) as u32,
                    rect.height(),
                ),
            )?;
        }

        // The footer answers the question.
        let fy = height as i32 - footer as i32;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.fill_rect(Rect::new(0, fy, width, footer))?;
        self.divider(0, fy, width, theme.border)?;
        self.hints(
            &[
                (HintButton::Pair('‹', '›'), "hint.choose"),
                ('A'.into(), "hint.confirm"),
                ('B'.into(), "hint.cancel"),
            ],
            theme,
            width,
            fy,
            footer,
        )?;
        Ok(())
    }
}
