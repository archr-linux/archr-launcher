//! Shared, bounded settings layout. The common text cache only sees fitted lines.
use super::*;
#[path = "render/bluetooth.rs"]
mod bluetooth;
use crate::settings::{
    games::SYSTEM_SECTION,
    pages::{MEDIA, MEDIA_SECTION},
    Id, Kind, Page, Row, Settings, CATEGORIES,
};
#[path = "render/appearance.rs"]
mod appearance;
#[path = "render/custom.rs"]
mod custom;
#[path = "render/information.rs"]
mod information;
#[path = "render/mangohud.rs"]
mod mangohud;
#[path = "render/network.rs"]
mod network;
const VIEW_TOP: f32 = 50.;
const VIEW_BOTTOM: f32 = 428.;
const FOOTER: f32 = 432.;
const ROW_GAP: f32 = 3.;
const SECTION_HEIGHT: f32 = 21.;
/// Usage bar and percentage under the games card row (Config-armazenamento).
const CARD_METER: f32 = 34.;
/// Right edge of full-width rows; pages with a side panel stop earlier.
const RIGHT: f32 = 624.;
#[derive(Clone, Copy)]
struct Placement {
    top: f32,
    height: f32,
    section: Option<&'static str>,
}
impl Placement {
    fn selected_top(self) -> f32 {
        self.top
            - if self.section.is_some() {
                SECTION_HEIGHT
            } else {
                0.
            }
    }
    fn bottom(self) -> f32 {
        self.top + self.height
    }
}
/// Mockup row heights: dense Netplay and Services lists, 40/46 elsewhere.
fn placements(page: Page, rows: &[Row]) -> Vec<Placement> {
    let mut top = VIEW_TOP;
    rows.iter()
        .map(|row| {
            if row.section.is_some() {
                top += SECTION_HEIGHT;
            }
            let height = match page {
                Page::Games if row.subtitle.is_some() => 40.,
                Page::Netplay | Page::Games => 34.,
                Page::Services => 38.,
                Page::Storage | Page::ScraperMedia => 40.,
                Page::Mangohud if row.subtitle.is_some() => 44.,
                _ if row.subtitle.is_some() => 46.,
                _ => 40.,
            };
            let place = Placement {
                top,
                height,
                section: row.section,
            };
            top += height + ROW_GAP;
            if row.id == Id::StorageCard {
                top += CARD_METER;
            }
            place
        })
        .collect()
}
pub(super) struct Scroll {
    page: Option<Page>,
    pub(super) tween: Tween,
}
impl Default for Scroll {
    fn default() -> Self {
        Self {
            page: None,
            tween: Tween::new(0., Duration::from_millis(90)),
        }
    }
}
impl Scroll {
    fn offset(
        &mut self,
        page: Page,
        selected_top: f32,
        selected_bottom: f32,
        content_bottom: f32,
        now: Instant,
    ) -> f32 {
        let changed = self.page != Some(page);
        let previous = if changed { 0. } else { self.tween.target };
        let target = previous
            .min(selected_top - VIEW_TOP)
            .max(selected_bottom - VIEW_BOTTOM)
            .clamp(0., (content_bottom - VIEW_BOTTOM).max(0.));
        self.tween.retarget(target, now);
        if changed {
            self.tween.snap();
            self.page = Some(page);
        }
        self.tween.value(now)
    }
}
/// How long a message stays up, and the fade that removes it.
const BANNER_TIME: Duration = Duration::from_secs(4);
const BANNER_FADE: Duration = Duration::from_millis(150);
/// The visible message: fades out on a timer or when the state drops it, and
/// a different message shows at once and restarts the timer.
pub(super) struct Banner {
    shown: Option<(Option<Id>, String)>,
    seq: u64,
    since: Instant,
    pub(super) fade: Tween,
}
impl Default for Banner {
    fn default() -> Self {
        Self {
            shown: None,
            seq: 0,
            since: Instant::now(),
            fade: Tween::new(0., BANNER_FADE),
        }
    }
}
impl Banner {
    /// Opacity (0..=1) to draw `error` with this frame.
    fn alpha(&mut self, error: Option<&(Option<Id>, String)>, seq: u64, now: Instant) -> f32 {
        match error {
            Some(error) if self.shown.as_ref() != Some(error) || self.seq != seq => {
                self.shown = Some(error.clone());
                self.seq = seq;
                self.since = now;
                self.fade.retarget(1., now);
                self.fade.snap();
            }
            Some(_) if now.saturating_duration_since(self.since) >= BANNER_TIME => {
                self.fade.retarget(0., now)
            }
            Some(_) => {}
            None => {
                self.fade.retarget(0., now);
                if !self.fade.active(now) {
                    self.shown = None;
                }
            }
        }
        self.fade.value(now)
    }
    /// A frame is due while fading or while the timer is still running.
    pub(super) fn busy(&self, now: Instant) -> bool {
        self.fade.active(now)
            || (self.shown.is_some()
                && self.fade.target > 0.
                && now.saturating_duration_since(self.since) < BANNER_TIME + BANNER_FADE)
    }
}
type Bounds = (f32, f32, f32, f32);
struct View<'a, 'ttf, 'tex> {
    ui: &'a mut Ui<'ttf, 'tex>,
    theme: &'a Theme,
}
impl Ui<'_, '_> {
    pub fn invalidate_settings_clock(&mut self) {
        if let Some(status) = &mut self.status {
            status.invalidate_clock();
        }
    }
    pub fn set_settings_clock_format(&mut self, format: &str) {
        if let Some(status) = &mut self.status {
            status.set_clock_format(format);
        }
    }
    pub(super) fn settings_page(&mut self, state: &Settings, theme: &Theme) -> Result<(), String> {
        if state.page().rows().is_empty() {
            self.settings_scroll.page = None;
            self.settings_scroll.tween.retarget(0., self.frame.now);
            self.settings_scroll.tween.snap();
        }
        View { ui: self, theme }.draw(state)
    }
}
impl View<'_, '_, '_> {
    fn rect(&self, (x, y, w, h): Bounds) -> Rect {
        Rect::new(
            self.ui.px(x),
            self.ui.px(y),
            self.ui.px(w) as u32,
            self.ui.px(h) as u32,
        )
    }
    fn panel(&mut self, bounds: Bounds, selected: bool) -> Result<(), String> {
        self.ui.rounded(
            self.rect(bounds),
            self.ui.px(10.) as f32,
            selection(selected, self.theme.surface, self.theme.highlight_bg),
            (!selected).then_some(self.theme.border),
        )
    }
    fn row_panel(&mut self, bounds: Bounds, selected: bool) -> Result<(), String> {
        if !selected {
            return Ok(());
        }
        // Clip the mesh's antialias fringe to the placement's exact height.
        let previous = self.ui.canvas.clip_rect();
        self.ui
            .canvas
            .set_clip_rect(previous.intersect_rect(self.rect(bounds)));
        let result = self.panel(bounds, true);
        self.ui.canvas.set_clip_rect(previous);
        result
    }
    fn ink(&self, selected: bool) -> Rgba {
        selection(selected, self.theme.text, self.theme.highlight_text)
    }
    fn tr(&self, text: &str) -> String {
        crate::settings::information::value(&self.ui.catalog, text)
    }
    /// Wrap at measured character boundaries, preserve explicit newlines, never
    /// send an oversized line to the ellipsizing game-text API. Minimum: 12 px.
    fn lines(&mut self, text: &str, size: f32, width: u32) -> Result<Vec<String>, String> {
        self.lines_face(text, size, width, Face::Regular)
    }
    fn lines_face(
        &mut self,
        text: &str,
        size: f32,
        width: u32,
        face: Face,
    ) -> Result<Vec<String>, String> {
        let mut lines = vec![String::new()];
        for ch in text.chars() {
            let line = lines.last_mut().expect("one line");
            if ch == '\n' {
                lines.push(String::new());
                continue;
            }
            if !line.is_empty() && self.ui.measure(&format!("{line}{ch}"), face, size)? > width {
                if let Some(at) = line.rfind(' ') {
                    let rest = line.split_off(at + 1);
                    line.truncate(at);
                    lines.push(format!("{rest}{ch}"));
                } else {
                    lines.push(ch.to_string());
                }
            } else {
                line.push(ch);
            }
        }
        Ok(lines)
    }
    fn text(
        &mut self,
        bounds: Bounds,
        text: &str,
        style: (f32, Rgba, Align),
    ) -> Result<(), String> {
        self.text_face(bounds, text, style, Face::Regular)
    }
    fn text_face(
        &mut self,
        bounds: Bounds,
        text: &str,
        style: (f32, Rgba, Align),
        face: Face,
    ) -> Result<(), String> {
        let (_, ink, align) = style;
        let (lines, size) = self.fit(bounds, text, style.0, face)?;
        let step = size + 3.;
        let top = bounds.1 + (bounds.3 - lines.len() as f32 * step) / 2.;
        for (n, line) in lines.iter().enumerate() {
            self.ui.label_aligned(
                line,
                (face, size, ink),
                self.rect((bounds.0, top + n as f32 * step, bounds.2, step)),
                align,
            )?;
        }
        Ok(())
    }
    /// Lines and size that fit the bounds, or the localized "unavailable".
    fn fit(
        &mut self,
        bounds: Bounds,
        text: &str,
        start: f32,
        face: Face,
    ) -> Result<(Vec<String>, f32), String> {
        let rect = self.rect(bounds);
        let mut size = start;
        Ok(loop {
            let lines = self.lines_face(text, size, rect.width(), face)?;
            let limit = (bounds.3 / (size + 3.)).floor().max(1.) as usize;
            if lines.len() <= limit {
                break (lines, size);
            }
            if size <= 12. {
                log::warn!("Settings text unavailable in this layout: {text}");
                break (
                    self.lines(&self.tr("settings.unavailable"), start, rect.width())?,
                    start,
                );
            }
            size = (size - 1.).max(12.);
        })
    }
    fn label(&mut self, bounds: Bounds, text: &str, size: f32) -> Result<(), String> {
        self.text(bounds, text, (size, self.theme.text, Align::Left))
    }
    fn note(&mut self, bounds: Bounds, text: &str, size: f32) -> Result<(), String> {
        self.text(bounds, text, (size, self.theme.muted, Align::Left))
    }
    /// "Settings / Parent /" in muted 13 px, the current page in 15 px semibold.
    fn breadcrumbs(&mut self, state: &Settings, right: f32) -> Result<(), String> {
        let page = state.page();
        let mut trail = Vec::new();
        let current = match page {
            Page::Root => self.tr("settings.title"),
            Page::Keyboard => {
                trail.push(self.tr("settings.title"));
                trail.push(self.tr(state.parent_page().title()));
                self.keyboard_name(state)
            }
            _ => {
                trail.push(self.tr("settings.title"));
                trail.extend(page.parent().iter().map(|key| self.tr(key)));
                self.tr(page.title())
            }
        };
        let trail: Vec<String> = trail.into_iter().map(|t| format!("{t} /")).collect();
        let (mut small, mut large) = (13., 15.);
        let widths = loop {
            let mut widths = Vec::new();
            for part in &trail {
                widths.push(self.ui.measure(part, Face::Regular, small)? as f32 + 8.);
            }
            widths.push(self.ui.measure(&current, Face::Semibold, large)? as f32);
            if widths.iter().sum::<f32>() <= right - 24. || small <= 12. {
                break widths;
            }
            small -= 0.5;
            large -= 1.;
        };
        let mut x = 24.;
        for (part, width) in trail.iter().zip(&widths) {
            self.text(
                (x, 1., *width, 44.),
                part,
                (small, self.theme.muted, Align::Left),
            )?;
            x += width;
        }
        self.text_face(
            (x, 0., (right - x).max(40.), 44.),
            &current,
            (large, self.theme.text, Align::Left),
            Face::Semibold,
        )
    }
    fn draw(&mut self, state: &Settings) -> Result<(), String> {
        let right = self.ui.status_header(self.theme)? as f32 / self.ui.frame.scale;
        if let Page::InfoDetail(index) = state.page() {
            let base = self.tr("settings.info");
            let item = self.tr(crate::settings::information::ITEMS[index.min(5)].0);
            let title = format!("{base} › {item}");
            let mut size = 15.;
            while size > 12. && self.ui.measure(&title, Face::Semibold, size)? as f32 > right - 24.
            {
                size -= 1.;
            }
            let base_width = self.ui.measure(&base, Face::Semibold, size)? as f32;
            let arrow_width = self.ui.measure(" › ", Face::Semibold, size)? as f32;
            self.text_face(
                (24., 0., base_width, 44.),
                &base,
                (size, self.theme.text, Align::Left),
                Face::Semibold,
            )?;
            self.text_face(
                (24. + base_width, 0., arrow_width, 44.),
                " › ",
                (size, self.theme.muted, Align::Left),
                Face::Semibold,
            )?;
            self.text_face(
                (
                    24. + base_width + arrow_width,
                    0.,
                    right - 24. - base_width - arrow_width,
                    44.,
                ),
                &item,
                (size, self.theme.text, Align::Left),
                Face::Semibold,
            )?;
        } else if state.page() == Page::Info {
            self.text_face(
                (24., 0., right - 24., 44.),
                &self.tr("settings.info"),
                (15., self.theme.text, Align::Left),
                Face::Semibold,
            )?;
        } else {
            self.breadcrumbs(state, right)?;
        }
        let rows: Vec<Row> = state.visible_rows().into_iter().copied().collect();
        match state.page() {
            Page::Root => self.root(state)?,
            Page::Info => self.info(state)?,
            Page::InfoDetail(index) => self.info_detail(state, index)?,
            Page::Soon(_) => {
                self.panel((24., 90., 592., 100.), false)?;
                self.text(
                    (40., 100., 560., 80.),
                    &self.tr("settings.soon"),
                    (20., self.theme.text, Align::Center),
                )?;
            }
            Page::Appearance => self.appearance_page(state, &rows)?,
            Page::AccentColor => self.custom_page(state)?,
            Page::Mangohud => self.mangohud_page(state, &rows)?,
            Page::MangohudItems => self.mangohud_items(state)?,
            Page::Wifi => self.wifi(state)?,
            Page::Bluetooth => self.bluetooth(state)?,
            Page::Keyboard => self.keyboard(state)?,
            Page::Scraper => {
                self.rows_at(state, &rows, &placements(Page::Scraper, &rows), 384.)?;
                self.scraper_panel(state)?;
            }
            Page::Storage => {
                let positions = placements(Page::Storage, &rows);
                self.list(
                    state,
                    &positions,
                    RIGHT,
                    |view, n, y, position, selected| {
                        view.row(state, &rows[n], (y, position.height), RIGHT, selected)?;
                        match (rows[n].id, &state.storage.card) {
                            (Id::StorageCard, Some(card)) => {
                                view.card_meter(card.percent, y + position.height)
                            }
                            _ => Ok(()),
                        }
                    },
                )?
            }
            page => self.rows_at(state, &rows, &placements(page, &rows), RIGHT)?,
        }
        let error = state.error.as_ref().filter(|(id, _)| {
            !matches!(state.page(), Page::Info | Page::InfoDetail(_))
                && (id.is_none() || rows.iter().any(|r| Some(r.id) == *id))
        });
        let alpha = self
            .ui
            .settings_banner
            .alpha(error, state.error_seq, self.ui.frame.now);
        if let Some((id, error)) = self.ui.settings_banner.shown.clone().filter(|_| alpha > 0.) {
            let fade = |c: Rgba| Rgba {
                a: (f32::from(c.a) * alpha) as u8,
                ..c
            };
            let bounds = if state.page() == Page::Root {
                (386., 318., 230., 110.)
            } else if state.page() == Page::Keyboard {
                (24., 296., 592., 40.)
            } else {
                (24., 384., 592., 40.)
            };
            self.ui.rounded(
                self.rect(bounds),
                self.ui.px(10.) as f32,
                fade(self.theme.surface),
                Some(fade(self.theme.border)),
            )?;
            let (x, y, w, h) = bounds;
            // Known failures arrive as locale keys; anything else stays generic.
            let message = if error.starts_with("settings.") {
                self.tr(&error)
            } else {
                log::debug!("Settings error on {id:?}: {error}");
                self.tr("settings.failed")
            };
            self.text(
                (x + 12., y + 4., w - 24., h - 8.),
                &message,
                (13., fade(self.theme.text), Align::Left),
            )?;
        }
        let footer = FOOTER;
        self.ui.divider(
            0,
            self.ui.px(footer),
            self.ui.frame.width,
            self.theme.border,
        )?;
        let left = self.ui.hints(
            state.hints(),
            self.theme,
            self.ui.frame.width,
            self.ui.px(footer),
            self.ui.px(480. - footer) as u32,
        )? as f32
            / self.ui.frame.scale;
        let status = if state.loading {
            Some(self.ui.catalog.text("loading"))
        } else if matches!(state.page(), Page::Info | Page::InfoDetail(_)) {
            Some(self.tr(if state.page() == Page::Info {
                "settings.updated_now"
            } else {
                "settings.refresh_interval"
            }))
        } else if let Some((busy, at)) = state.refreshing() {
            if busy {
                Some(self.tr("settings.refreshing"))
            } else {
                at.map(|at| {
                    let seconds = self.ui.frame.now.saturating_duration_since(at).as_secs();
                    self.ui
                        .catalog
                        .format("settings.updated_ago", &[("count", &seconds.to_string())])
                })
            }
        } else {
            None
        };
        if let Some(status) = status {
            self.note((24., footer, (left - 36.).max(60.), 48.), &status, 13.)?;
        }
        use crate::HintButton::Key;
        type Hints = &'static [(crate::HintButton, &'static str)];
        // Question, explanation, and the answers (none while the host works).
        let dialog: Option<(String, Option<String>, Option<Hints>)> =
            if let Some(key) = state.reboot_prompt {
                Some((
                    self.tr(key),
                    None,
                    Some(&[(Key('A'), "menu.reboot"), (Key('B'), "settings.later")]),
                ))
            } else if let Some(name) = &state.confirm_forget {
                let bluetooth = state.page() == Page::Bluetooth;
                let name = state
                    .bt
                    .iter()
                    .find(|d| bluetooth && d.addr == *name)
                    .map_or(name, |d| &d.name);
                let key = if bluetooth {
                    "settings.bt_forget_question"
                } else {
                    "settings.forget_question"
                };
                Some((
                    self.ui.catalog.format(key, &[("name", name)]),
                    None,
                    Some(&[(Key('A'), "settings.forget"), (Key('B'), "hint.cancel")]),
                ))
            } else if let Some(confirm) = &state.storage.confirm {
                let (question, detail, hints) = confirm.dialog();
                Some((self.tr(question), Some(self.tr(detail)), Some(hints)))
            } else if let Some(key) = state.storage.working {
                Some((self.tr(key), Some(self.tr("settings.wait_storage")), None))
            } else {
                state.connecting.as_ref().map(|ssid| {
                    (
                        self.ui
                            .catalog
                            .format("settings.connecting", &[("name", ssid)]),
                        Some(self.tr("settings.wait")),
                        None,
                    )
                })
            };
        if let Some((question, detail, hints)) = dialog {
            self.ui.scrim(self.theme, 1.)?;
            if let (Some(detail), Some(hints)) = (&detail, hints) {
                return self.question(&question, detail, hints);
            }
            self.panel((80., 158., 480., 164.), false)?;
            self.label((104., 174., 432., 80.), &question, 18.)?;
            if let Some(detail) = detail {
                self.note((104., 262., 432., 40.), &detail, 14.)?;
            }
            if let Some(hints) = hints {
                self.ui.hints(
                    hints,
                    self.theme,
                    self.ui.px(550.) as u32,
                    self.ui.px(264.),
                    self.ui.px(34.) as u32,
                )?;
            }
        }
        Ok(())
    }
    /// A question, its explanation and the answers, in a panel sized to the
    /// wrapped text and centered above the footer.
    fn question(
        &mut self,
        question: &str,
        detail: &str,
        hints: &[(crate::HintButton, &str)],
    ) -> Result<(), String> {
        let height = |view: &mut Self, text: &str, size: f32, most: f32| {
            view.fit((104., 0., 432., most), text, size, Face::Regular)
                .map(|(lines, size)| lines.len() as f32 * (size + 3.))
        };
        let title = height(self, question, 18., 63.)?;
        let body = height(self, detail, 14., 136.)?;
        let total = 20. + title + 8. + body + 18. + 34. + 16.;
        let top = ((432. - total) / 2.).round();
        self.panel((80., top, 480., total), false)?;
        self.label((104., top + 20., 432., title), question, 18.)?;
        self.note((104., top + 28. + title, 432., body), detail, 14.)?;
        self.ui
            .hints(
                hints,
                self.theme,
                self.ui.px(550.) as u32,
                self.ui.px(top + total - 50.),
                self.ui.px(34.) as u32,
            )
            .map(|_| ())
    }
    /// Shared scrolling list: sections, clipping and the eased offset.
    fn list(
        &mut self,
        state: &Settings,
        positions: &[Placement],
        right: f32,
        mut draw: impl FnMut(&mut Self, usize, f32, Placement, bool) -> Result<(), String>,
    ) -> Result<(), String> {
        let selected = positions.get(state.cursor).copied().unwrap_or(Placement {
            top: VIEW_TOP,
            height: 0.,
            section: None,
        });
        let bottom = positions.last().map_or(VIEW_TOP, |p| p.bottom());
        let offset = self.ui.settings_scroll.offset(
            state.page(),
            selected.selected_top(),
            selected.bottom(),
            bottom,
            self.ui.frame.now,
        );
        self.ui.canvas.set_clip_rect(self.rect((
            16.,
            VIEW_TOP,
            right - 16.,
            VIEW_BOTTOM - VIEW_TOP,
        )));
        let result = (|| {
            for (n, position) in positions.iter().enumerate() {
                let y = position.top - offset;
                if position.bottom() - offset < VIEW_TOP
                    || position.selected_top() - offset >= VIEW_BOTTOM
                {
                    continue;
                }
                if let Some(section) = position.section {
                    // The Games page names the system its rows configure.
                    let name = state.games.fullname();
                    let heading = if section == SYSTEM_SECTION && !name.is_empty() {
                        name.to_owned()
                    } else if section == MEDIA_SECTION {
                        let count = state.media_tokens().len();
                        self.tr(&format!("{section}|count={count}|total={}", MEDIA.len()))
                    } else {
                        self.tr(section)
                    }
                    .to_uppercase();
                    self.text(
                        (28., y - SECTION_HEIGHT, right - 40., SECTION_HEIGHT),
                        &heading,
                        (11., self.theme.muted, Align::Left),
                    )?;
                }
                let selected = n == state.cursor;
                self.row_panel((16., y, right - 16., position.height), selected)?;
                draw(self, n, y, *position, selected)?;
            }
            Ok(())
        })();
        self.ui.canvas.set_clip_rect(None);
        result
    }
    /// Sections and variable-height rows share one geometry with scrolling.
    fn rows_at(
        &mut self,
        state: &Settings,
        rows: &[Row],
        positions: &[Placement],
        right: f32,
    ) -> Result<(), String> {
        self.list(state, positions, right, |view, n, y, position, selected| {
            view.row(state, &rows[n], (y, position.height), right, selected)
        })
    }
    fn row(
        &mut self,
        state: &Settings,
        row: &Row,
        (y, height): (f32, f32),
        right: f32,
        selected: bool,
    ) -> Result<(), String> {
        let ink = if state.disabled(row.id) {
            selection(selected, self.theme.muted, self.theme.accent_text_secondary)
        } else {
            self.ink(selected)
        };
        let face = selection(selected, Face::Regular, Face::Semibold);
        let label = self.tr(&state.label(row));
        // A switch needs little room: its title and subtitle take the rest.
        let full = if row.kind == Kind::Toggle {
            right - 98.
        } else {
            (right - 46.) * 0.48
        };
        // Action rows only show a short link on the right: the label may use the rest.
        let link = match row.kind {
            Kind::Action => {
                self.ui
                    .measure(&state.display_value(row.id, &self.ui.catalog), face, 14.)?
                    as f32
                    / self.ui.frame.scale
            }
            _ => f32::MAX,
        };
        let full = full.max(right - 46. - link - 24.);
        let subtitle = match row.id {
            Id::ScraperMedia => Some(self.media_summary(state, full)?),
            _ => state.subtitle(row).map(|text| self.tr(&text)),
        };
        let expand = subtitle.is_none()
            && (row.kind == Kind::Value || row.id == Id::MangohudItems)
            && self
                .ui
                .measure(&state.display_value(row.id, &self.ui.catalog), face, 14.)?
                as f32
                > right - full - 66.;
        let label_width = if expand {
            // The items summary is the long value of a short title.
            let least = if row.id == Id::MangohudItems {
                60.
            } else {
                120.
            };
            (self.ui.measure(&label, face, 15.)? as f32).clamp(least, full)
        } else {
            full
        };
        let control_x = 30. + label_width + 16.;
        let control = (control_x, y, right - 14. - control_x, height);
        let sub_height = ((height - 4.) / 2.).floor();
        let title_height = if subtitle.is_some() {
            height - sub_height
        } else {
            height
        };
        self.text_face(
            (30., y, label_width, title_height),
            &label,
            (15., ink, Align::Left),
            face,
        )?;
        if let Some(subtitle) = subtitle {
            self.text(
                (30., y + title_height - 2., label_width, sub_height),
                &subtitle,
                (
                    12.,
                    selection(selected, self.theme.muted, self.theme.accent_text_secondary),
                    Align::Left,
                ),
            )?;
        }
        if state.pending == Some(row.id) {
            self.text_face(
                control,
                &self.tr("settings.applying"),
                (14., ink, Align::Right),
                face,
            )
        } else {
            self.control(row.id, row.kind, state, selected, control)
        }
    }
    /// "Cover · screen · logo", or "3 of 8" when the names do not fit.
    fn media_summary(&mut self, state: &Settings, width: f32) -> Result<String, String> {
        let chosen = state.media_tokens();
        if chosen.is_empty() {
            return Ok(self.tr("settings.media_none"));
        }
        let names: Vec<String> = chosen
            .iter()
            .map(|token| self.tr(&format!("media.{token}_short")))
            .collect();
        let mut summary = names.join(" · ");
        if let Some(first) = summary.chars().next() {
            summary = first
                .to_uppercase()
                .chain(summary.chars().skip(1))
                .collect();
        }
        Ok(
            if self.ui.measure(&summary, Face::Regular, 12.)? as f32 / self.ui.frame.scale <= width
            {
                summary
            } else {
                self.tr(&format!(
                    "settings.media_count|count={}|total={}",
                    chosen.len(),
                    MEDIA.len()
                ))
            },
        )
    }
    /// Thin usage bar and "35% in use" below the games card row.
    fn card_meter(&mut self, percent: u8, top: f32) -> Result<(), String> {
        let (x, width) = (30., RIGHT - 44.);
        self.capsule((x, top + 5., width, 6.), self.theme.border)?;
        if percent > 0 {
            let fill = (width * f32::from(percent.min(100)) / 100.).max(6.);
            self.capsule((x, top + 5., fill, 6.), self.theme.highlight_bg)?;
        }
        self.text_face(
            (x, top + 14., width, 16.),
            &self.tr(&format!("settings.percent_used|count={percent}")),
            (12., self.theme.muted, Align::Left),
            Face::Mono,
        )
    }
    fn capsule(&mut self, bounds: Bounds, ink: Rgba) -> Result<(), String> {
        let rect = self.rect(bounds);
        self.ui.rounded(rect, rect.height() as f32 / 2., ink, None)
    }
    fn bar(&mut self, (x, y, w, h): Bounds, value: u8, ink: Rgba) -> Result<(), String> {
        self.capsule((x, y, w, h), self.theme.badge)?;
        if value > 0 {
            self.capsule(
                (x, y, (w * f32::from(value.min(100)) / 100.).max(1.), h),
                ink,
            )?;
        }
        Ok(())
    }
    fn control(
        &mut self,
        id: Id,
        kind: Kind,
        state: &Settings,
        selected: bool,
        (x, y, w, h): Bounds,
    ) -> Result<(), String> {
        let ink = if state.disabled(id) {
            selection(selected, self.theme.muted, self.theme.accent_text_secondary)
        } else {
            self.ink(selected)
        };
        match kind {
            Kind::Slider { .. } => {
                let v = id
                    .normalize(state.value(id))
                    .parse::<u8>()
                    .expect("normalized");
                self.bar((x, y + h / 2. - 3., w - 80., 6.), v, ink)?;
                self.text(
                    (x + w - 70., y, 70., h),
                    &format!("{v}%"),
                    (14., ink, Align::Right),
                )?;
            }
            Kind::Value if id == Id::Accent => self.swatches(state, selected, (x, y, w, h))?,
            Kind::Check => {
                let Id::MediaKind(n) = id else {
                    return Ok(());
                };
                let (token, _, _, used) = MEDIA[usize::from(n)];
                let on = state.media_tokens().contains(&token);
                let (bx, by) = (x + w - 18., y + (h - 18.) / 2.);
                let rect = self.rect((bx, by, 18., 18.));
                let radius = self.ui.px(5.) as f32;
                if on {
                    // On the highlighted row the box inverts to stay visible.
                    let (fill, ink) = selection(
                        selected,
                        (self.theme.highlight_bg, self.theme.highlight_text),
                        (self.theme.highlight_text, self.theme.highlight_bg),
                    );
                    self.ui.rounded(rect, radius, fill, None)?;
                    let tick = self.rect((bx + 3., by + 3., 12., 12.));
                    self.ui.icon(Glyph::Check, tick, ink)?;
                } else {
                    // A 1.5px dim ring: the box face drawn over a dim square.
                    self.ui.rounded(rect, radius, self.theme.dim_text, None)?;
                    let face = self.rect((bx + 1.5, by + 1.5, 15., 15.));
                    let fill = selection(selected, self.theme.background, self.theme.highlight_bg);
                    self.ui.rounded(face, radius - 1., fill, None)?;
                }
                if let Some(used) = used {
                    self.text(
                        (x, y, w - 28., h),
                        &self.tr(used),
                        (
                            11.,
                            selection(
                                selected,
                                self.theme.dim_text,
                                self.theme.accent_text_secondary,
                            ),
                            Align::Right,
                        ),
                    )?;
                }
            }
            Kind::Toggle => {
                let on = state.value(id) == "1";
                let track = selection(
                    on,
                    self.theme.badge,
                    selection(selected, self.theme.highlight_bg, self.theme.highlight_text),
                );
                self.capsule((x + w - 38., y + h / 2. - 10., 38., 20.), track)?;
                self.capsule(
                    (
                        x + w - 36. + if on { 18. } else { 0. },
                        y + h / 2. - 8.,
                        16.,
                        16.,
                    ),
                    selection(selected && on, self.theme.text, self.theme.highlight_bg),
                )?;
            }
            _ => {
                let v = state.display_value(id, &self.ui.catalog);
                let chevrons = kind == Kind::Value
                    && !Settings::text_input(id)
                    && !state.disabled(id)
                    && !state.fixed(id)
                    && id != Id::ScraperSource;
                if chevrons {
                    // "‹ value ›" hugs the value, as in the mockups.
                    let face = selection(selected, Face::Regular, Face::Semibold);
                    let (lines, size) = self.fit((x + 20., y, w - 40., h), &v, 14., face)?;
                    let mut width = 0.;
                    for line in &lines {
                        width = f32::max(width, self.ui.measure(line, face, size)? as f32);
                    }
                    let width = width / self.ui.frame.scale;
                    let left = (x + w - 20. - width - 20.).max(x);
                    self.ui.icon(
                        Glyph::ChevronLeft,
                        self.rect((left, y + (h - 14.) / 2., 14., 14.)),
                        ink,
                    )?;
                    self.ui.icon(
                        Glyph::ChevronRight,
                        self.rect((x + w - 14., y + (h - 14.) / 2., 14., 14.)),
                        ink,
                    )?;
                }
                let inset = if chevrons { 20. } else { 0. };
                self.text_face(
                    (x + inset, y, w - 2. * inset, h),
                    &v,
                    (14., ink, Align::Right),
                    selection(selected, Face::Regular, Face::Semibold),
                )?;
            }
        }
        Ok(())
    }
    fn root(&mut self, state: &Settings) -> Result<(), String> {
        for (n, (page, _, glyph)) in CATEGORIES.iter().enumerate() {
            let key = page.title();
            let key = if *page == Page::Wifi {
                "settings.network"
            } else {
                key
            };
            let y = 50. + n as f32 * 39.;
            let selected = n == state.cursor;
            let ink = self.ink(selected);
            self.row_panel((16., y, 358., 36.), selected)?;
            self.ui
                .icon(*glyph, self.rect((28., y + 9., 18., 18.)), ink)?;
            self.text((58., y, 300., 36.), &self.tr(key), (15., ink, Align::Left))?;
        }
        let (page, detail, _) = CATEGORIES[state.cursor.min(CATEGORIES.len() - 1)];
        let key = if page == Page::Wifi {
            "settings.network"
        } else {
            page.title()
        };
        self.panel((386., 66., 230., 240.), false)?;
        self.label((402., 82., 198., 64.), &self.tr(key), 18.)?;
        self.note((402., 146., 198., 112.), &self.tr(detail), 15.)?;
        self.text(
            (402., 268., 198., 36.),
            &self.tr("settings.instant"),
            (12., self.theme.dim_text, Align::Left),
        )
    }
}

#[cfg(test)]
#[path = "../../tests/support/settings_capture.rs"]
mod capture_tests;
#[cfg(test)]
#[path = "../../tests/support/settings_scroll.rs"]
mod scroll_tests;
#[cfg(test)]
mod banner_tests {
    use super::*;
    fn error(text: &str) -> Option<(Option<Id>, String)> {
        Some((None, text.into()))
    }
    #[test]
    fn message_fades_after_four_seconds_and_a_new_one_restarts_the_timer() {
        let (mut banner, t0) = (Banner::default(), Instant::now());
        let first = error("settings.failed");
        assert_eq!(banner.alpha(first.as_ref(), 0, t0), 1.);
        let late = t0 + Duration::from_millis(3900);
        assert_eq!(banner.alpha(first.as_ref(), 0, late), 1.);
        // A different message replaces it at full opacity and restarts the clock.
        let second = error("settings.wifi_timeout");
        assert_eq!(banner.alpha(second.as_ref(), 1, late), 1.);
        let later = late + Duration::from_millis(3900);
        assert_eq!(banner.alpha(second.as_ref(), 1, later), 1.);
        // Past the timer the message fades, then stays gone while the state keeps it.
        let gone = later + Duration::from_millis(300);
        banner.alpha(second.as_ref(), 1, gone);
        let end = gone + BANNER_FADE + Duration::from_millis(10);
        assert_eq!(banner.alpha(second.as_ref(), 1, end), 0.);
        assert!(!banner.busy(end));
        assert_eq!(banner.alpha(second.as_ref(), 1, end), 0.);
    }
    #[test]
    fn an_identical_message_arriving_again_restarts_the_timer() {
        let (mut banner, t0) = (Banner::default(), Instant::now());
        let message = error("settings.failed");
        banner.alpha(message.as_ref(), 1, t0);
        let late = t0 + Duration::from_millis(3900);
        assert_eq!(banner.alpha(message.as_ref(), 2, late), 1.);
        let later = late + Duration::from_millis(3900);
        assert_eq!(banner.alpha(message.as_ref(), 2, later), 1.);
    }
    #[test]
    fn input_dropping_the_message_fades_it_out_early() {
        let (mut banner, t0) = (Banner::default(), Instant::now());
        banner.alpha(error("settings.failed").as_ref(), 0, t0);
        let now = t0 + Duration::from_millis(500);
        banner.alpha(None, 0, now);
        assert!(banner.busy(now));
        let end = now + BANNER_FADE + Duration::from_millis(10);
        assert_eq!(banner.alpha(None, 0, end), 0.);
        assert!(banner.shown.is_none());
    }
}
