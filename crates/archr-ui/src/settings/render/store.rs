//! Theme store: the grid with its tabs (Loja-temas) and a theme's details
//! (Loja-tema). Fixed layout like every settings page; theme names and authors
//! come from GitHub and are clipped like game names.
use super::*;
use crate::settings::store::{Card, Tab, COLUMNS, TOPIC};
use std::path::Path;

const TABS: (f32, f32) = (48., 22.);
const GRID_TOP: f32 = 78.;
const CARD: (f32, f32) = (188., 120.);
const GAP: f32 = 10.;
/// The details page: picture column and text column (flex 1.25 : 1, gap 16).
const PICTURE: (f32, f32, f32, f32) = (20., 50., 324., 200.);
const TEXT: (f32, f32) = (360., 260.);
const BUTTONS: f32 = 388.;

impl View<'_, '_, '_> {
    /// `path`'s picture inside `bounds`: cropped to fill, or whole and centred.
    /// False until the loader has it.
    pub(super) fn picture(
        &mut self,
        path: Option<&Path>,
        bounds: Bounds,
        fill: bool,
    ) -> Result<bool, String> {
        let Some(path) = path else { return Ok(false) };
        self.ui.store_wanted.push(path.to_owned());
        let dst = self.rect(bounds);
        let ui = &mut *self.ui;
        let Some(texture) = ui.store_art.as_ref().and_then(|art| art.peek(path)) else {
            return Ok(false);
        };
        let query = texture.query();
        let (tw, th) = (query.width as f32, query.height as f32);
        let (dw, dh) = (dst.width() as f32, dst.height() as f32);
        if fill {
            let scale = (dw / tw).max(dh / th);
            let (sw, sh) = (dw / scale, dh / scale);
            let src = Rect::new(
                ((tw - sw) / 2.) as i32,
                ((th - sh) / 2.) as i32,
                sw as u32,
                sh as u32,
            );
            ui.canvas.copy(texture, src, dst)?;
        } else {
            let scale = (dw / tw).min(dh / th);
            let (w, h) = (tw * scale, th * scale);
            let at = Rect::new(
                dst.x() + ((dw - w) / 2.) as i32,
                dst.y() + ((dh - h) / 2.) as i32,
                w as u32,
                h as u32,
            );
            ui.canvas.copy(texture, None, at)?;
        }
        Ok(true)
    }
    /// A picture box: background, thin border and the image or a palette.
    fn frame_picture(
        &mut self,
        path: Option<&Path>,
        bounds: Bounds,
        (radius, edge): (f32, Rgba),
        fill: bool,
    ) -> Result<(), String> {
        let theme = self.theme;
        self.ui.rounded(
            self.rect(bounds),
            self.ui.px(radius) as f32,
            theme.background,
            Some(edge),
        )?;
        let (x, y, w, h) = bounds;
        if !self.picture(path, (x + 1., y + 1., w - 2., h - 2.), fill)? {
            let side = (h * 0.4).clamp(12., 40.);
            let icon = self.rect((x + (w - side) / 2., y + (h - side) / 2., side, side));
            self.ui.icon(Glyph::Palette, icon, theme.border)?;
        }
        Ok(())
    }
    /// One line of GitHub text, clipped like a game name.
    pub(super) fn plain(
        &mut self,
        text: &str,
        bounds: Bounds,
        style: (Face, f32, Rgba),
    ) -> Result<(), String> {
        let rect = self.rect(bounds);
        self.ui.label(text, style, rect)
    }
    /// "★ 214": the star and the count, right-aligned at `right`; returns the left edge.
    pub(super) fn stars(
        &mut self,
        count: u64,
        right: f32,
        (y, h): (f32, f32),
        (size, ink): (f32, Rgba),
    ) -> Result<f32, String> {
        let text = count.to_string();
        let width = self.ui.measure(&text, Face::Regular, size)? as f32;
        let left = right - width - size - 3.;
        let icon = self.rect((left, y + (h - size) / 2., size, size));
        self.ui.icon(Glyph::Star, icon, ink)?;
        self.text(
            (right - width, y, width + 1., h),
            &text,
            (size, ink, Align::Right),
        )?;
        Ok(left)
    }
    /// The store tabs (Loja-temas, Loja-modulos) and the source note.
    pub(super) fn tabs(&mut self, keys: &[&str], chosen: usize, note: &str) -> Result<(), String> {
        let theme = self.theme;
        let (top, height) = TABS;
        let mut x = 20.;
        for (n, key) in keys.iter().enumerate() {
            let label = self.tr(key);
            let face = if n == chosen {
                Face::Semibold
            } else {
                Face::Regular
            };
            let w = self.ui.measure(&label, face, 12.)? as f32 + 20.;
            let (fill, ink) = if n == chosen {
                (theme.highlight_bg, theme.highlight_text)
            } else {
                (theme.surface, theme.muted)
            };
            self.ui.rounded(
                self.rect((x, top, w, height)),
                self.ui.px(11.) as f32,
                fill,
                None,
            )?;
            self.text_face((x, top, w, height), &label, (12., ink, Align::Center), face)?;
            x += w + 8.;
        }
        let note = self.tr(note);
        if self.ui.measure(&note, Face::Regular, 12.)? as f32 <= 620. - x {
            self.text(
                (x, top, 620. - x, height),
                &note,
                (12., theme.muted, Align::Right),
            )?;
        }
        Ok(())
    }
    pub(super) fn store_grid(&mut self, state: &Settings) -> Result<(), String> {
        let store = &state.store;
        let keys = Tab::ALL.map(Tab::key);
        self.tabs(
            &keys,
            store.tab as usize,
            &format!("store.source|topic={TOPIC}"),
        )?;
        let cards = store.cards();
        if cards.is_empty() {
            return self.grid_message(state);
        }
        let row = state.cursor / COLUMNS;
        let first = row.saturating_sub(1) * COLUMNS;
        // A third row peeks under the second, clipped above the footer.
        let previous = self.ui.canvas.clip_rect();
        let clip = self.rect((0., GRID_TOP - 2., 640., FOOTER - GRID_TOP + 2.));
        self.ui.canvas.set_clip_rect(clip);
        let drawn = (|| {
            for (n, card) in cards.iter().enumerate().skip(first).take(COLUMNS * 3) {
                let (column, line) = ((n - first) % COLUMNS, (n - first) / COLUMNS);
                let at = (
                    20. + column as f32 * (CARD.0 + GAP),
                    GRID_TOP + line as f32 * (CARD.1 + GAP),
                );
                self.card(state, card, at, n == state.cursor)?;
            }
            Ok::<_, String>(())
        })();
        self.ui.canvas.set_clip_rect(previous);
        drawn
    }
    fn card(
        &mut self,
        state: &Settings,
        card: &Card,
        (x, y): (f32, f32),
        selected: bool,
    ) -> Result<(), String> {
        let theme = self.theme;
        let (w, h) = CARD;
        let radius = self.ui.px(12.) as f32;
        if selected {
            self.ui
                .rounded(self.rect((x, y, w, h)), radius, theme.highlight_bg, None)?;
            let inner = self.rect((x + 2., y + 2., w - 4., h - 4.));
            self.ui
                .rounded(inner, self.ui.px(10.) as f32, theme.surface, None)?;
        } else {
            self.ui
                .rounded(self.rect((x, y, w, h)), radius, theme.surface, None)?;
        }
        let inner = w - 16.;
        self.frame_picture(
            card.preview.as_deref(),
            (x + 8., y + 8., inner, 66.),
            (8., theme.border),
            true,
        )?;
        let line = (y + 80., 18.);
        let right = x + w - 8.;
        let left = if state.store.is_installed(card) {
            let badge = self.tr("store.installed_badge");
            let width = self.ui.measure(&badge, Face::Regular, 11.)? as f32;
            self.text(
                (right - width, line.0, width + 1., line.1),
                &badge,
                (11., theme.success, Align::Right),
            )?;
            right - width
        } else {
            self.stars(card.stars, right, line, (11., self.theme.muted))?
        };
        let title = card.title(self.ui.catalog.language()).to_owned();
        self.plain(
            &title,
            (x + 8., line.0, left - x - 14., line.1),
            (Face::Semibold, 14., theme.text),
        )?;
        self.plain(
            &card.owner,
            (x + 8., y + 97., inner, 14.),
            (Face::Regular, 11., theme.muted),
        )
    }
    /// Searching, empty and failure states of a tab.
    fn grid_message(&mut self, state: &Settings) -> Result<(), String> {
        let store = &state.store;
        let theme = self.theme;
        if store.loading {
            for n in 0..COLUMNS * 2 {
                let (column, line) = (n % COLUMNS, n / COLUMNS);
                let bounds = (
                    20. + column as f32 * (CARD.0 + GAP),
                    GRID_TOP + line as f32 * (CARD.1 + GAP),
                    CARD.0,
                    CARD.1,
                );
                self.ui.rounded(
                    self.rect(bounds),
                    self.ui.px(12.) as f32,
                    theme.surface,
                    None,
                )?;
            }
            return Ok(());
        }
        let (glyph, title, detail) = match store.problem {
            Some("store.offline") => (
                Glyph::WifiOff,
                "store.offline",
                Some("store.offline_detail"),
            ),
            Some("store.rate_limited") => (
                Glyph::Clock,
                "store.rate_limited",
                Some("store.rate_limited_detail"),
            ),
            Some(key) => (Glyph::WifiOff, key, None),
            None if store.tab == Tab::Installed => (Glyph::Palette, "store.none_installed", None),
            None => (Glyph::Palette, "store.empty", None),
        };
        self.panel((20., GRID_TOP, 600., 150.), false)?;
        let icon = self.rect((302., GRID_TOP + 22., 36., 36.));
        self.ui.icon(glyph, icon, theme.muted)?;
        self.text(
            (40., GRID_TOP + 66., 560., 26.),
            &self.tr(title),
            (15., theme.text, Align::Center),
        )?;
        if let Some(detail) = detail {
            self.text(
                (40., GRID_TOP + 94., 560., 40.),
                &self.tr(detail),
                (12., theme.muted, Align::Center),
            )?;
        }
        Ok(())
    }
    /// Footer note of the store pages; the repository address only when it
    /// fits `room` on one line.
    pub(super) fn store_status(&mut self, state: &Settings, room: f32) -> Option<String> {
        let store = &state.store;
        if state.page() == Page::StoreTheme {
            let card = store.detail.as_ref().map(|d| &d.card)?;
            let address = format!("github.com/{}/{}", card.owner, card.repo);
            let width = self.ui.measure(&address, Face::Regular, 13.).ok()? as f32;
            return (width <= room).then_some(address);
        }
        if let Some(problem) = store.problem.filter(|_| !store.cards().is_empty()) {
            return Some(self.tr(problem));
        }
        if store.loading {
            return Some(self.tr("store.searching"));
        }
        let fetched = store.fetched.filter(|_| store.tab != Tab::Installed)?;
        let minutes = fetched.elapsed().map_or(0, |age| age.as_secs() / 60);
        Some(if minutes == 0 {
            self.tr("settings.updated_now")
        } else {
            self.tr(&format!("store.updated_ago|count={minutes}"))
        })
    }
    /// Top-aligned wrapped text that shrinks to 12 px to fit `max` lines;
    /// returns the bottom.
    pub(super) fn flow(
        &mut self,
        (x, y, w): (f32, f32, f32),
        text: &str,
        (size, ink, face): (f32, Rgba, Face),
        max: usize,
    ) -> Result<f32, String> {
        let width = self.rect((x, y, w, 1.)).width();
        let mut size = size;
        let mut lines = self.lines_face(text, size, width, face)?;
        while lines.len() > max && size > 12. {
            size = (size - 1.).max(12.);
            lines = self.lines_face(text, size, width, face)?;
        }
        let step = (size * 1.4).round();
        for (n, line) in lines.iter().take(max).enumerate() {
            let rect = self.rect((x, y + n as f32 * step, w, step));
            self.ui
                .label_aligned(line, (face, size, ink), rect, Align::Left)?;
        }
        Ok(y + lines.len().min(max) as f32 * step)
    }
    pub(super) fn store_detail(&mut self, state: &Settings) -> Result<(), String> {
        let store = &state.store;
        let theme = self.theme;
        let language = self.ui.catalog.language().to_owned();
        let detail = store.detail.as_ref();
        let card = detail
            .map(|d| &d.card)
            .or_else(|| store.cards().get(state.cursor));
        let images: Vec<&Path> = match detail {
            Some(d) => d.images.iter().map(PathBuf::as_path).collect(),
            None => card
                .and_then(|c| c.preview.as_deref())
                .into_iter()
                .collect(),
        };
        let shown = images.get(store.image).or(images.first()).copied();
        self.frame_picture(shown, PICTURE, (12., theme.border), false)?;
        if images.len() > 1 {
            let (x, w) = (PICTURE.0, PICTURE.2);
            let count = images.len() as f32;
            let each = (w - 6. * (count - 1.)) / count;
            for (n, path) in images.iter().enumerate() {
                let edge = if n == store.image {
                    theme.highlight_bg
                } else {
                    theme.border
                };
                let bounds = (x + n as f32 * (each + 6.), 258., each, 34.);
                self.frame_picture(Some(path), bounds, (6., edge), true)?;
            }
        }
        let (x, w) = TEXT;
        let title = card.map_or(String::new(), |c| c.title(&language).to_owned());
        let mut y = self.flow(
            (x, VIEW_TOP, w),
            &title,
            (20., theme.text, Face::Semibold),
            2,
        )?;
        let Some(detail) = detail else {
            let key = store.problem.unwrap_or("store.loading");
            self.flow(
                (x, y + 8., w),
                &self.tr(key),
                (12., theme.muted, Face::Regular),
                4,
            )?;
            return Ok(());
        };
        if let Some(text) = detail.description.get(&language) {
            y = self.flow((x, y + 4., w), text, (12., theme.muted, Face::Regular), 5)?;
        }
        y += 8.;
        for (label, value) in self.facts(detail, &language) {
            let label = self.tr(label);
            let label_w = self.ui.measure(&label, Face::Regular, 12.)? as f32 + 12.;
            let width = self.rect((0., 0., w - label_w, 1.)).width();
            let lines = self.lines(&value, 12., width)?;
            let height = lines.len() as f32 * 15. + 6.;
            if y + height > BUTTONS - 6. {
                break;
            }
            self.text(
                (x, y, label_w, 21.),
                &label,
                (12., theme.muted, Align::Left),
            )?;
            for (n, line) in lines.iter().enumerate() {
                let rect = self.rect((x + label_w, y + 3. + n as f32 * 15., w - label_w, 15.));
                self.ui.label_aligned(
                    line,
                    (Face::Regular, 12., theme.text),
                    rect,
                    Align::Right,
                )?;
            }
            y += height;
            self.ui.divider(
                self.ui.px(x),
                self.ui.px(y - 1.),
                self.ui.px(w) as u32,
                theme.border,
            )?;
        }
        // Main action and the star count (Instalar, ★ 214).
        let count = detail.card.stars.to_string();
        let pill = self.ui.measure(&count, Face::Regular, 14.)? as f32 + 14. + 4. + 24.;
        let pill_x = x + w - pill;
        self.ui.rounded(
            self.rect((pill_x, BUTTONS, pill, 36.)),
            self.ui.px(10.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        self.stars(
            detail.card.stars,
            x + w - 12.,
            (BUTTONS, 36.),
            (14., theme.muted),
        )?;
        if let Some(action) = store.action() {
            let button = (x, BUTTONS, pill_x - 8. - x, 36.);
            self.ui.rounded(
                self.rect(button),
                self.ui.px(10.) as f32,
                theme.highlight_bg,
                None,
            )?;
            self.text_face(
                button,
                &self.tr(action),
                (14., theme.highlight_text, Align::Center),
                Face::Semibold,
            )?;
        }
        Ok(())
    }
    /// The details table: label key and value.
    fn facts(
        &self,
        detail: &crate::settings::store::Detail,
        language: &str,
    ) -> Vec<(&'static str, String)> {
        let join = |names: Vec<String>| names.join(", ");
        let version = [detail.version.as_str(), detail.reference.as_str()]
            .into_iter()
            .find(|v| !v.is_empty() && *v != detail.card.branch)
            .unwrap_or("");
        let date = match detail.date.split('-').collect::<Vec<_>>()[..] {
            [y, m, d] => self.tr(&format!("store.date|y={y}|m={m}|d={d}")),
            _ => String::new(),
        };
        let size = if detail.size >= 1 << 20 {
            let tenths = (detail.size * 10 + (1 << 19)) >> 20;
            let mut text = format!("{}.{}", tenths / 10, tenths % 10);
            if !language.starts_with("en") {
                text = text.replace('.', ",");
            }
            self.tr(&format!("store.megabytes|count={text}"))
        } else {
            let kilobytes = detail.size.div_ceil(1024).max(1);
            self.tr(&format!("store.kilobytes|count={kilobytes}"))
        };
        let author = if detail.author.is_empty() {
            &detail.card.owner
        } else {
            &detail.author
        };
        vec![
            ("store.author", author.clone()),
            (
                "store.version",
                [version, &date]
                    .into_iter()
                    .filter(|v| !v.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
            (
                "store.views",
                join(
                    detail
                        .views
                        .iter()
                        .filter_map(|n| n.get(language))
                        .map(str::to_owned)
                        .collect(),
                ),
            ),
            (
                "store.includes",
                join(detail.includes.iter().map(|k| self.tr(k)).collect()),
            ),
            ("store.size", size),
            (
                "store.license",
                if detail.license.is_empty() {
                    self.tr("store.no_license")
                } else {
                    detail.license.clone()
                },
            ),
        ]
    }
}
