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
    ) -> Result<(), String> {
        let Some(search) = &app.search else {
            return Ok(());
        };
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
