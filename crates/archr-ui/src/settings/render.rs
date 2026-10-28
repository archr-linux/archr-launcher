//! Shared, bounded settings layout. The common text cache only sees fitted lines.
use super::*;
use crate::settings::{Id, InfoCard, Kind, Page, Settings, CATEGORIES};
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
    pub(super) fn settings_page(&mut self, state: &Settings, theme: &Theme) -> Result<(), String> {
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
    fn ink(&self, selected: bool) -> Rgba {
        selection(selected, self.theme.text, self.theme.highlight_text)
    }
    fn tr(&self, text: &str) -> String {
        if let Some((key, value)) = text
            .split_once('|')
            .filter(|(k, _)| k.starts_with("settings."))
        {
            self.ui.catalog.format(key, &[("value", value)])
        } else if text.starts_with("settings.") {
            self.ui.catalog.text(text)
        } else {
            text.into()
        }
    }
    /// Wrap at measured character boundaries, preserve explicit newlines, never
    /// send an oversized line to the ellipsizing game-text API. Minimum: 12 px.
    fn lines(&mut self, text: &str, size: f32, width: u32) -> Result<Vec<String>, String> {
        let mut lines = vec![String::new()];
        for ch in text.chars() {
            let line = lines.last_mut().expect("one line");
            if ch == '\n' {
                lines.push(String::new());
                continue;
            }
            if !line.is_empty()
                && self
                    .ui
                    .measure(&format!("{line}{ch}"), Face::Regular, size)?
                    > width
            {
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
        let rect = self.rect(bounds);
        let (mut size, ink, align) = style;
        let (lines, size) = loop {
            let lines = self.lines(text, size, rect.width())?;
            let limit = (bounds.3 / (size + 3.)).floor().max(1.) as usize;
            if lines.len() <= limit {
                break (lines, size);
            }
            if size <= 12. {
                log::warn!("Settings text unavailable in this layout: {text}");
                break (
                    self.lines(&self.tr("settings.unavailable"), style.0, rect.width())?,
                    style.0,
                );
            }
            size = (size - 1.).max(12.);
        };
        let step = size + 3.;
        let top = bounds.1 + (bounds.3 - lines.len() as f32 * step) / 2.;
        for (n, line) in lines.iter().enumerate() {
            self.ui.label_aligned(
                line,
                (Face::Regular, size, ink),
                self.rect((bounds.0, top + n as f32 * step, bounds.2, step)),
                align,
            )?;
        }
        Ok(())
    }
    fn label(&mut self, bounds: Bounds, text: &str, size: f32) -> Result<(), String> {
        self.text(bounds, text, (size, self.theme.text, Align::Left))
    }
    fn note(&mut self, bounds: Bounds, text: &str, size: f32) -> Result<(), String> {
        self.text(bounds, text, (size, self.theme.muted, Align::Left))
    }
    fn draw(&mut self, state: &Settings) -> Result<(), String> {
        let right = self.ui.status_header(self.theme)? as f32 / self.ui.frame.scale;
        let title = if state.page() == Page::Root {
            self.tr("settings.title")
        } else {
            format!(
                "{} / {}",
                self.tr("settings.title"),
                self.tr(state.page().title())
            )
        };
        self.label((24., 0., right - 24., 48.), &title, 15.)?;
        match state.page() {
            Page::Root => self.root(state)?,
            Page::Info => self.info(state)?,
            Page::Soon(_) => {
                self.panel((24., 90., 592., 100.), false)?;
                self.text(
                    (40., 100., 560., 80.),
                    &self.tr("settings.soon"),
                    (20., self.theme.text, Align::Center),
                )?;
            }
            page => {
                let step = if page == Page::Region { 80. } else { 64. };
                for (n, row) in page.rows().iter().enumerate() {
                    let y = 68. + n as f32 * step;
                    let selected = n == state.cursor;
                    let ink = self.ink(selected);
                    let label = self.tr(row.label);
                    let expand = row.subtitle.is_none()
                        && row.kind == Kind::Value
                        && self.ui.measure(
                            &state.display_value(row.id, &self.ui.catalog),
                            Face::Regular,
                            14.,
                        )? > 228;
                    let label_width = if expand {
                        self.ui.measure(&label, Face::Regular, 15.)?.clamp(120, 278) as f32
                    } else {
                        278.
                    };
                    let control_x = 38. + label_width + 16.;
                    let control = (control_x, y, 600. - control_x, step - 6.);
                    self.panel((24., y, 592., step - 6.), selected)?;
                    self.text(
                        (
                            38.,
                            y,
                            label_width,
                            if row.subtitle.is_some() {
                                28.
                            } else {
                                step - 6.
                            },
                        ),
                        &label,
                        (15., ink, Align::Left),
                    )?;
                    if let Some(key) = row.subtitle {
                        self.text(
                            (38., y + 28., 278., 30.),
                            &self.tr(key),
                            (
                                12.,
                                selection(
                                    selected,
                                    self.theme.muted,
                                    self.theme.accent_text_secondary,
                                ),
                                Align::Left,
                            ),
                        )?;
                    }
                    if state.pending == Some(row.id) {
                        self.text(
                            control,
                            &self.tr("settings.applying"),
                            (14., ink, Align::Right),
                        )?;
                    } else {
                        self.control(row.id, row.kind, state, selected, control)?;
                    }
                }
            }
        }
        if state.error.as_ref().is_some_and(|(id, _)| {
            state.page() != Page::Info
                && (id.is_none()
                    || state.page() == Page::Root
                    || state.page().rows().iter().any(|r| Some(r.id) == *id))
        }) {
            let bounds = if state.page() == Page::Root {
                (386., 318., 230., 110.)
            } else {
                (24., 394., 592., 40.)
            };
            self.panel(bounds, false)?;
            let (x, y, w, h) = bounds;
            self.label(
                (x + 12., y + 8., w - 24., h - 16.),
                &self.tr("settings.failed"),
                12.,
            )?;
        }
        self.ui
            .divider(0, self.ui.px(440.), self.ui.frame.width, self.theme.border)?;
        self.ui.hints(
            state.hints(),
            self.theme,
            self.ui.frame.width,
            self.ui.px(440.),
            self.ui.px(40.) as u32,
        )?;
        if state.loading {
            self.note((24., 440., 160., 40.), &self.tr("loading"), 12.)?;
        }
        if state.reboot_prompt {
            self.ui.scrim(self.theme, 1.)?;
            self.panel((80., 158., 480., 164.), false)?;
            self.label(
                (104., 174., 432., 80.),
                &self.tr("settings.reboot_question"),
                18.,
            )?;
            self.ui.hints(
                &[('A', "menu.reboot"), ('B', "settings.later")],
                self.theme,
                self.ui.px(550.) as u32,
                self.ui.px(264.),
                self.ui.px(34.) as u32,
            )?;
        }
        Ok(())
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
        let ink = self.ink(selected);
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
                if kind == Kind::Value {
                    self.ui.icon(
                        Glyph::ChevronLeft,
                        self.rect((x, y + (h - 14.) / 2., 14., 14.)),
                        ink,
                    )?;
                    self.ui.icon(
                        Glyph::ChevronRight,
                        self.rect((x + w - 14., y + (h - 14.) / 2., 14., 14.)),
                        ink,
                    )?;
                }
                self.text((x + 20., y, w - 40., h), &v, (14., ink, Align::Right))?;
            }
        }
        Ok(())
    }
    fn root(&mut self, state: &Settings) -> Result<(), String> {
        for (n, (page, _, glyph)) in CATEGORIES.iter().enumerate() {
            let key = page.title();
            let y = 66. + n as f32 * 40.;
            let selected = n == state.cursor;
            let ink = self.ink(selected);
            self.panel((24., y, 346., 38.), selected)?;
            self.ui
                .icon(*glyph, self.rect((38., y + 10., 18., 18.)), ink)?;
            self.text((68., y, 282., 38.), &self.tr(key), (15., ink, Align::Left))?;
        }
        let (page, detail, _) = CATEGORIES[state.cursor.min(CATEGORIES.len() - 1)];
        let key = page.title();
        self.panel((386., 66., 230., 240.), false)?;
        self.label((402., 82., 198., 64.), &self.tr(key), 18.)?;
        self.note((402., 146., 198., 112.), &self.tr(detail), 15.)?;
        self.text(
            (402., 272., 198., 28.),
            &self.tr("settings.instant"),
            (12., self.theme.dim_text, Align::Left),
        )
    }
    fn card_height(pair: &[(&InfoCard, Vec<String>)]) -> f32 {
        let count = pair.iter().map(|(_, lines)| lines.len()).max().unwrap_or(1);
        36. + count as f32 * 17.
            + if pair.iter().any(|(card, _)| card.percent.is_some()) {
                12.
            } else {
                0.
            }
    }
    fn info(&mut self, state: &Settings) -> Result<(), String> {
        let info = &state.info;
        self.label((24., 50., 94., 38.), "ArchR", 24.)?;
        self.text(
            (128., 50., 488., 38.),
            &info.version,
            (18., self.theme.highlight_bg, Align::Left),
        )?;
        self.note(
            (24., 88., 592., 30.),
            &self.ui.catalog.format(
                "settings.device_line",
                &[("device", &info.device), ("kernel", &info.kernel)],
            ),
            13.,
        )?;
        self.note(
            (24., 118., 592., 20.),
            &self
                .ui
                .catalog
                .format("settings.uptime", &[("value", &info.uptime)]),
            12.,
        )?;
        let mut content = Vec::new();
        for card in &info.cards {
            let mut lines = Vec::new();
            for line in card.lines.iter().filter(|s| !s.is_empty()) {
                lines.extend(self.lines(&self.tr(line), 12., self.ui.px(260.) as u32)?);
            }
            if lines.is_empty() {
                lines.push(self.tr("settings.unavailable"));
            }
            content.push((card, lines));
        }
        // Try the complete content first; only unavailable cards replace an
        // external value when the combined layout cannot clear the footer.
        while content
            .chunks(2)
            .map(|pair| Self::card_height(pair) + 8.)
            .sum::<f32>()
            > 440. - 144.
        {
            let Some((card, lines)) = content
                .iter_mut()
                .filter(|(_, lines)| lines.len() > 1)
                .max_by_key(|(_, lines)| lines.len())
            else {
                break;
            };
            log::warn!(
                "Settings information unavailable in this layout: {}",
                card.lines.join("\n")
            );
            *lines = vec![self.tr("settings.unavailable")];
        }
        let mut y = 144.;
        for pair in content.chunks(2) {
            let h = Self::card_height(pair);
            for (n, (card, lines)) in pair.iter().enumerate() {
                let x = 24. + n as f32 * 304.;
                self.panel((x, y, 288., h), false)?;
                self.label((x + 14., y + 6., 260., 22.), &self.tr(card.title), 14.)?;
                for (i, line) in lines.iter().enumerate() {
                    self.note((x + 14., y + 31. + i as f32 * 17., 260., 17.), line, 12.)?;
                }
                if let Some(v) = card.percent {
                    self.bar((x + 14., y + h - 12., 260., 5.), v, self.theme.highlight_bg)?;
                }
            }
            y += h + 8.;
        }
        Ok(())
    }
}
