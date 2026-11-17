//! The list's search (Pastas-busca) and a folder's art (Pastas-lista).
use super::*;

/// Result rows, in reference pixels.
const ROW: f32 = 34.;

impl Ui<'_, '_> {
    /// `<System> / Search` and "N of TOTAL", the field, the results, and
    /// the keyboard over the hints.
    pub(super) fn search(
        &mut self,
        app: &App,
        (system, cursor): (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Some(search) = &app.search else {
            return Ok(());
        };
        if theme.screens.search.field {
            self.search_field(app, (system, cursor), data, theme, art)?;
            return self.search_hints(app, !search.results, theme);
        }
        let at = |ui: &Self, x: f32, y: f32, w: f32, h: f32| {
            Rect::new(ui.px(x), ui.px(y), ui.px(w) as u32, ui.px(h) as u32)
        };
        let (_, height) = self.frame.reference();
        let found = app.search_results(data, system);
        let games = data.games(system);
        let dirs = data.folders(system);
        // Header: the title, the count, then the status at the right.
        let header = self.frame.size(theme.header_height).round() as u32;
        let right = if self.view(theme).games.status {
            self.status_header(theme)?
        } else {
            self.frame.width as i32 - self.px(24.)
        };
        let count = self.catalog.format(
            "search.count",
            &[
                ("n", &found.len().to_string()),
                ("total", &games.len().to_string()),
            ],
        );
        let right = self.packed_label(
            &count,
            (Face::Mono, 13., theme.muted),
            (right - self.px(10.), 0, header),
            u32::MAX,
            Align::Right,
        )?;
        let title = self.catalog.format(
            "search.title",
            &[("system", &system_title(system, data, &self.catalog))],
        );
        let pad = self.px(24.);
        let width = (right - pad - self.px(10.)).max(1) as u32;
        self.label(
            &title,
            (Face::Semibold, 15., theme.text),
            Rect::new(pad, 0, width, header),
        )?;
        // The field, its border lit while typing.
        let field = at(self, 24., 50., 592., 38.);
        let typing = !search.results;
        let border = selection(typing, theme.border, theme.highlight_bg);
        self.rounded(field, self.px(10.) as f32, theme.surface, Some(border))?;
        self.icon(Glyph::Search, at(self, 38., 60., 18., 18.), theme.muted)?;
        let text = search.keyboard.text();
        let (shown, ink) = if text.is_empty() && !typing {
            (self.catalog.text("search.placeholder"), theme.muted)
        } else if typing && self.blink_on(self.frame.now) {
            (format!("{text}_"), theme.text)
        } else {
            (text.to_owned(), theme.text)
        };
        self.label(
            &shown,
            (Face::Regular, 16., ink),
            at(self, 66., 50., 536., 38.),
        )?;
        // As many results as fit above the keyboard; the selected one in sight.
        let keys = height - 48. - 4. - 40. * search.keyboard.rows().len() as f32;
        let top = 96.;
        let fits = (((keys - 6. - top) / ROW).floor() as usize).max(1);
        let first = (cursor + 1)
            .saturating_sub(fits)
            .min(found.len().saturating_sub(fits));
        if found.is_empty() {
            self.label(
                &self.catalog.text("search.empty"),
                (Face::Regular, 15., theme.muted),
                at(self, 38., top, 560., ROW),
            )?;
        }
        for (n, &game) in found.iter().enumerate().skip(first).take(fits) {
            let y = top + (n - first) as f32 * ROW;
            let selected = search.results && n == cursor;
            if selected {
                let pill = at(self, 24., y, 592., ROW - 2.);
                self.rounded(
                    pill,
                    self.frame.size(theme.radius_item),
                    theme.highlight_bg,
                    None,
                )?;
            }
            let ink = selection(selected, theme.text, theme.highlight_text);
            let dim = selection(selected, theme.muted, theme.accent_text_secondary);
            // The folder the game lies in, by its last name; the system at the root.
            let dir = dirs.get(game).copied().unwrap_or_default();
            let place = dir.rsplit('/').next().filter(|d| !d.is_empty());
            let place =
                place.map_or_else(|| system_title(system, data, &self.catalog), str::to_owned);
            let row = (self.px(604.), self.px(y), self.px(ROW - 2.) as u32);
            let end = self.packed_label(
                &place,
                (Face::Regular, 13., dim),
                row,
                self.px(200.) as u32,
                Align::Right,
            )?;
            let name = games.get(game).map_or("", |g| g.name.as_str());
            let face = selection(selected, Face::Regular, Face::Semibold);
            let w = (end - self.px(52.)).max(1) as u32;
            self.label(
                name,
                (face, 15., ink),
                Rect::new(self.px(38.), self.px(y), w, row.2),
            )?;
        }
        let labels = (
            self.catalog.text("settings.space"),
            self.catalog.text("search.list"),
        );
        self.keyboard_keys(&search.keyboard, keys, (&labels.0, &labels.1), theme)?;
        self.search_hints(app, typing, theme)
    }
    /// The search's buttons: typing, or choosing among the results.
    fn search_hints(&mut self, app: &App, typing: bool, theme: &Theme) -> Result<(), String> {
        let (_, height) = self.frame.reference();
        // Under the game menu, its own hints take the footer.
        if !matches!(app.screen, Screen::Search { .. }) {
            return Ok(());
        }
        let hints: &[(HintButton, &str)] = if typing {
            &[
                (HintButton::Key('A'), "search.type"),
                (HintButton::Key('Y'), "hint.delete"),
                (HintButton::Start, "search.list"),
                (HintButton::Key('B'), "hint.close"),
            ]
        } else {
            &[
                (HintButton::Key('A'), "hint.play"),
                (HintButton::Key('X'), "hint.favorite"),
                (HintButton::Key('Y'), "hint.menu"),
                (HintButton::Key('B'), "search.keyboard"),
            ]
        };
        self.hints(
            hints,
            theme,
            self.frame.width,
            self.px(height - 48.),
            self.px(48.) as u32,
        )?;
        Ok(())
    }

    /// The search as a field of glass with the count inside, the results in
    /// a group of glass with their covers and the term in bold, and the
    /// keyboard docked at the bottom on a sheet.
    fn search_field(
        &mut self,
        app: &App,
        (system, cursor): (&str, usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Some(search) = &app.search else {
            return Ok(());
        };
        let style = theme.screens.search;
        let at = |ui: &Self, x: f32, y: f32, w: f32, h: f32| {
            Rect::new(ui.px(x), ui.px(y), ui.px(w) as u32, ui.px(h) as u32)
        };
        let (_, height) = self.frame.reference();
        let found = app.search_results(data, system);
        let games = data.games(system);
        let dirs = data.folders(system);
        let header = self.frame.size(theme.header_height).round() as u32;
        let right = self.status_header(theme)?;
        let title = self.catalog.format(
            "search.title",
            &[("system", &system_title(system, data, &self.catalog))],
        );
        let pad = self.px(24.);
        self.label(
            &title,
            (Face::Semibold, 15., theme.text),
            Rect::new(pad, 0, (right - pad - self.px(10.)).max(1) as u32, header),
        )?;
        // The field: a capsule of glass, lit while typing; where the theme
        // puts it, else under the header.
        let fh = style.height * 480.;
        let field = match style.field_at {
            Some(r) => self.area(r),
            None => at(self, 24., 50., 592., fh),
        };
        let typing = !search.results;
        let round = field.height() as f32 / 2.;
        self.glass(field, round, &theme.glass_or_default())?;
        if typing {
            let width = 2. * self.frame.scale;
            self.geometry
                .ring(&mut self.canvas, field, round, width, theme.highlight_bg)?;
        }
        let side = self.px(18.);
        let icon = Rect::new(
            field.x() + self.px(16.),
            field.y() + (field.height() as i32 - side) / 2,
            side as u32,
            side as u32,
        );
        self.icon(Glyph::Search, icon, theme.text_secondary)?;
        let count = self.catalog.format(
            "search.count",
            &[
                ("n", &found.len().to_string()),
                ("total", &games.len().to_string()),
            ],
        );
        let end = self.packed_label(
            &count,
            (Face::Regular, 12., theme.text_secondary),
            (field.right() - self.px(18.), field.y(), field.height()),
            u32::MAX,
            Align::Right,
        )?;
        let text = search.keyboard.text();
        let tx = icon.right() + self.px(10.);
        let room = (end - self.px(12.) - tx).max(1) as u32;
        if text.is_empty() && !typing {
            let hint = self.catalog.text("search.placeholder");
            self.label(
                &hint,
                (Face::Regular, 16., theme.muted),
                Rect::new(tx, field.y(), room, field.height()),
            )?;
        } else {
            self.label(
                text,
                (Face::Regular, 16., theme.text),
                Rect::new(tx, field.y(), room, field.height()),
            )?;
            if typing && self.blink_on(self.frame.now) {
                let w = self.measure(text, Face::Regular, 16.)?.min(room) as i32;
                let bar = Rect::new(
                    tx + w + self.px(2.),
                    field.y() + field.height() as i32 / 4,
                    self.px(2.).max(1) as u32,
                    (field.height() / 2).max(1),
                );
                self.canvas.set_draw_color(color(theme.highlight_bg));
                self.canvas.fill_rect(bar)?;
            }
        }
        // In reference pixels from here: the results' box and the keyboard.
        let scale = self.frame.scale;
        let rows = search.keyboard.rows().len() as f32;
        let k = theme.screens.keyboard;
        let (kh, kg) = if k == crate::screens::KeyboardStyle::default() {
            (34., 6.)
        } else {
            (k.key_h * 480., k.key_gap * 480.)
        };
        let keys_h = (kh + kg) * rows;
        let base = style.row * 480.;
        let below_field = field.bottom() as f32 / scale + 10.;
        let boxed = style.results.map(|r| {
            let r = self.area(r);
            (
                r.x() as f32 / scale,
                r.y() as f32 / scale,
                r.width() as f32 / scale,
                r.height() as f32 / scale,
            )
        });
        // Docked at the bottom edge, or right under the results' box (five
        // rows when the theme gives none): it never moves while typing.
        let keys = if style.dock_results {
            let bottom = boxed.map_or(below_field + 5. * base, |(_, y, _, h)| y + h);
            bottom + 22.
        } else {
            height - 48. - 6. - keys_h
        };
        let (gx, top, gw, box_h) =
            boxed.unwrap_or((24., below_field, 592., keys - 22. - below_field));
        // A taller box gives the rows more room, up to two fifths more, so
        // five of them fill it; a short one keeps the theme's.
        let row = if boxed.is_none() {
            (box_h / 5.).clamp(base, base * 1.4).floor()
        } else {
            base
        };
        if style.dock_results {
            let card = at(self, 8., keys - 12., 624., keys_h + 18.);
            self.cast_shadow(card, self.px(22.) as f32)?;
            self.glass(card, self.px(22.) as f32, &theme.glass_or_default())?;
        } else {
            let sheet = at(self, 8., keys - 12., 624., height - keys + 60.);
            self.glass(sheet, self.px(22.) as f32, &theme.glass_or_default())?;
        }
        let fits = ((box_h / row).floor() as usize).max(1);
        let first = (cursor + 1)
            .saturating_sub(fits)
            .min(found.len().saturating_sub(fits));
        if found.is_empty() {
            self.label(
                &self.catalog.text("search.empty"),
                (Face::Regular, 15., theme.muted),
                at(self, gx + 14., top, gw - 28., row),
            )?;
        } else {
            let shown = found.len().saturating_sub(first).min(fits) as f32;
            let group = at(self, gx, top, gw, shown * row);
            self.glass(group, self.px(14.) as f32, &theme.glass_or_default())?;
        }
        let term = text.trim().to_lowercase();
        for (n, &game) in found.iter().enumerate().skip(first).take(fits) {
            let y = top + (n - first) as f32 * row;
            let selected = search.results && n == cursor;
            let band = at(self, gx + 4., y + 2., gw - 8., row - 4.);
            if selected {
                self.rounded(
                    band,
                    self.px(10.) as f32,
                    Rgba {
                        a: 0x24,
                        ..theme.text
                    },
                    None,
                )?;
            }
            let entry = games.get(game);
            let thumb_h = row - 12.;
            let thumb = at(self, gx + 12., y + 6., thumb_h * 4. / 3., thumb_h);
            let cover = entry
                .and_then(|g| g.image.as_ref())
                .and_then(|path| art.covers.and_then(|c| c.peek(path)));
            match cover {
                Some(texture) => {
                    let size = texture.query();
                    let part = crate::primitives::crop(
                        (size.width, size.height),
                        (thumb.width(), thumb.height()),
                    );
                    let radius = self.px(5.) as f32;
                    self.geometry
                        .cover(&mut self.canvas, texture, thumb, radius, 255, part)?;
                }
                None => {
                    self.rounded(thumb, self.px(5.) as f32, theme.surface, Some(theme.border))?
                }
            }
            let dir = dirs.get(game).copied().unwrap_or_default();
            let place = dir.rsplit('/').next().filter(|d| !d.is_empty());
            let place =
                place.map_or_else(|| system_title(system, data, &self.catalog), str::to_owned);
            let line = (self.px(gx + gw - 16.), self.px(y), self.px(row) as u32);
            let end = self.packed_label(
                &place,
                (Face::Regular, 12., theme.text_secondary),
                line,
                self.px(180.) as u32,
                Align::Right,
            )?;
            let name = entry.map_or("", |g| g.name.as_str());
            let x = thumb.right() + self.px(12.);
            let room = (end - self.px(12.) - x).max(1) as u32;
            self.term_label(name, &term, (x, line.1, room, line.2), theme)?;
        }
        let labels = (
            self.catalog.text("settings.space"),
            self.catalog.text("search.list"),
        );
        self.keyboard_keys(&search.keyboard, keys, (&labels.0, &labels.1), theme)
    }
    /// `name` with the searched `term` in bold where it is found.
    fn term_label(
        &mut self,
        name: &str,
        term: &str,
        (x, y, room, h): (i32, i32, u32, u32),
        theme: &Theme,
    ) -> Result<(), String> {
        let lower = name.to_lowercase();
        // Only where lowering kept every byte in place.
        let found = (!term.is_empty() && lower.len() == name.len())
            .then(|| lower.find(term))
            .flatten()
            .filter(|&i| name.is_char_boundary(i) && name.is_char_boundary(i + term.len()));
        let Some(start) = found else {
            return self.label(
                name,
                (Face::Regular, 15., theme.text),
                Rect::new(x, y, room, h),
            );
        };
        let parts = [
            (&name[..start], Face::Regular),
            (&name[start..start + term.len()], Face::Bold),
            (&name[start + term.len()..], Face::Regular),
        ];
        let mut left = x;
        for (text, face) in parts {
            if text.is_empty() {
                continue;
            }
            let w = self.measure(text, face, 15.)? as i32;
            let room = (x + room as i32 - left).max(0) as u32;
            if room == 0 {
                break;
            }
            self.label(text, (face, 15., theme.text), Rect::new(left, y, room, h))?;
            left += w;
        }
        Ok(())
    }

    /// A folder in the art panel: the covers of its first four games in a
    /// 2x2 mosaic (a folder icon when none has one), and "Folder · N games".
    pub(super) fn folder_art(
        &mut self,
        (system, folder): (&str, &str),
        count: usize,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        layout: &Layout,
    ) -> Result<(), String> {
        // A game selected next fades in again.
        self.cover_key = None;
        self.cover_visible = false;
        if let Some(box_) = layout.art {
            let rect = self.area(box_.rect);
            let gap = self.px(6.);
            let inner = Rect::new(
                rect.x() + gap,
                rect.y() + gap,
                rect.width().saturating_sub(2 * gap as u32).max(1),
                rect.height().saturating_sub(2 * gap as u32).max(1),
            );
            let covers: Vec<_> = App::folder_covers(data, system, folder)
                .iter()
                .filter_map(|path| art.covers?.peek(path))
                .collect();
            if covers.is_empty() {
                let side = self.px(48.);
                let icon = Rect::new(
                    rect.x() + (rect.width() as i32 - side) / 2,
                    rect.y() + (rect.height() as i32 - side) / 2,
                    side as u32,
                    side as u32,
                );
                self.icon(Glyph::Folder, icon, theme.dim_text)?;
            }
            let (w, h) = (
                (inner.width().saturating_sub(gap as u32) / 2).max(1),
                (inner.height().saturating_sub(gap as u32) / 2).max(1),
            );
            let radius = self.frame.size(box_.radius) / 2.;
            for (n, cover) in covers.iter().enumerate() {
                let cell = Rect::new(
                    inner.x() + (n % 2) as i32 * (w as i32 + gap),
                    inner.y() + (n / 2) as i32 * (h as i32 + gap),
                    w,
                    h,
                );
                let size = cover.query();
                let part = crop((size.width, size.height), (w, h));
                self.geometry
                    .cover(&mut self.canvas, cover, cell, radius, 255, part)?;
            }
        }
        if let Some(meta) = layout.meta {
            let games = self.catalog.plural(
                "games.count",
                count as f64,
                &[("count", &count.to_string())],
            );
            let text = self.catalog.format("folder.summary", &[("games", &games)]);
            let ink = (Face::Regular, meta.size * 480., theme.muted);
            self.label(&text, ink, self.area(meta.rect))?;
        }
        Ok(())
    }
}
