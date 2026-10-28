//! Wi-Fi list (Config-wifi), text entry (Wifi-senha) and the scraper panel.
use super::*;
use crate::keyboard::Mode;

impl View<'_, '_, '_> {
    pub(super) fn wifi(&mut self, state: &Settings) -> Result<(), String> {
        let mut positions = vec![Placement {
            top: VIEW_TOP,
            height: 40.,
            section: None,
        }];
        // Divider under the switch: 4 px margins around a 1 px rule.
        let mut top = VIEW_TOP + 40. + ROW_GAP + 9.;
        for n in 1..state.wifi_rows() {
            let connected = state.networks.get(n - 1).is_some_and(|w| w.connected);
            let height = match state.networks.get(n - 1) {
                Some(_) if connected => 44.,
                Some(_) => 38.,
                None => 40.,
            };
            positions.push(Placement {
                top,
                height,
                section: None,
            });
            top += height + ROW_GAP;
        }
        self.list(state, &positions, RIGHT, |view, n, y, place, selected| {
            view.wifi_row(state, n, (y, place.height), selected)
        })
    }
    fn wifi_row(
        &mut self,
        state: &Settings,
        n: usize,
        (y, h): (f32, f32),
        selected: bool,
    ) -> Result<(), String> {
        let ink = self.ink(selected);
        let face = selection(selected, Face::Regular, Face::Semibold);
        if n == 0 {
            self.text_face(
                (28., y, 300., h),
                &self.tr("settings.wifi"),
                (15., ink, Align::Left),
                face,
            )?;
            let control = (300., y, 312., h);
            return if state.pending == Some(Id::WifiSwitch) {
                self.text(
                    control,
                    &self.tr("settings.applying"),
                    (14., ink, Align::Right),
                )
            } else {
                self.control(Id::WifiSwitch, Kind::Toggle, state, selected, control)?;
                self.ui.divider(
                    self.ui.px(28.),
                    self.ui.px(y + h + ROW_GAP + 4.),
                    self.ui.px(584.) as u32,
                    self.theme.border,
                )
            };
        }
        let Some(network) = state.networks.get(n - 1) else {
            let services = n == state.wifi_rows() - 1;
            let key = if services {
                "settings.services"
            } else {
                "settings.hidden"
            };
            let ink = selection(selected, self.theme.muted, self.theme.highlight_text);
            self.text_face(
                (28., y, 540., h),
                &self.tr(key),
                (15., ink, Align::Left),
                face,
            )?;
            if services {
                self.ui.icon(
                    Glyph::ChevronRight,
                    self.rect((596., y + (h - 14.) / 2., 14., 14.)),
                    ink,
                )?;
            }
            return Ok(());
        };
        let title = if network.connected { h - 18. } else { h };
        self.ui.label_aligned(
            &network.ssid,
            (face, 15., ink),
            self.rect((
                28.,
                y + if network.connected { 3. } else { 0. },
                520.,
                title,
            )),
            Align::Left,
        )?;
        if network.connected {
            let detail = if state.access_ip.is_empty() {
                self.tr("settings.connected")
            } else {
                self.ui
                    .catalog
                    .format("settings.connected_ip", &[("ip", &state.access_ip)])
            };
            self.text(
                (28., y + title - 1., 520., 18.),
                &detail,
                (
                    12.,
                    selection(selected, self.theme.muted, self.theme.accent_text_secondary),
                    Align::Left,
                ),
            )?;
        }
        let bars = selection(
            selected,
            self.theme.text_secondary,
            self.theme.highlight_text,
        );
        let icon_y = y + (h - 18.) / 2.;
        if !selected {
            self.ui.icon(
                Glyph::Signal,
                self.rect((592., icon_y, 18., 18.)),
                self.theme.badge,
            )?;
        }
        let level = match network.signal {
            75.. => Glyph::Signal,
            50..=74 => Glyph::SignalHigh,
            25..=49 => Glyph::SignalMedium,
            _ => Glyph::SignalLow,
        };
        self.ui
            .icon(level, self.rect((592., icon_y, 18., 18.)), bars)?;
        if network.security != "open" {
            let lock = selection(selected, self.theme.muted, self.theme.highlight_text);
            self.ui.icon(
                Glyph::Lock,
                self.rect((570., y + (h - 14.) / 2., 14., 14.)),
                lock,
            )?;
        }
        Ok(())
    }
    /// Breadcrumb name of the field being edited.
    pub(super) fn keyboard_name(&self, state: &Settings) -> String {
        match state.keyboard_target {
            Some(Id::WifiSwitch) => state.wifi_ssid.clone(),
            Some(Id::Hidden) => self.tr("settings.hidden"),
            Some(Id::ScraperAccount | Id::ScraperPassword) => self.tr("settings.account"),
            Some(id) => Page::Netplay
                .rows()
                .iter()
                .find(|row| row.id == id)
                .map_or_else(String::new, |row| self.tr(row.label)),
            None => String::new(),
        }
    }
    pub(super) fn keyboard(&mut self, state: &Settings) -> Result<(), String> {
        let Some(keyboard) = state.keyboard.as_ref() else {
            return Ok(());
        };
        let (caption, rule) = match state.keyboard_target {
            Some(Id::WifiSwitch) => (
                self.ui
                    .catalog
                    .format("settings.password_of", &[("name", &state.wifi_ssid)]),
                Some(if keyboard.optional {
                    "settings.open_allowed"
                } else {
                    "settings.wpa_length"
                }),
            ),
            Some(Id::Hidden) => (self.tr("settings.network_name"), None),
            Some(Id::ScraperAccount) => (self.tr("settings.user"), None),
            Some(Id::ScraperPassword) => (self.tr("settings.password"), None),
            _ => (self.keyboard_name(state), None),
        };
        let rule = rule.map(|key| self.tr(key));
        let rule_width = match &rule {
            Some(rule) => self.ui.measure(rule, Face::Regular, 12.)? as f32 + 8.,
            None => 0.,
        };
        self.note((24., 48., 592. - rule_width, 20.), &caption, 12.)?;
        if let Some(rule) = rule {
            self.text(
                (616. - rule_width, 48., rule_width, 20.),
                &rule,
                (12., self.theme.muted, Align::Right),
            )?;
        }
        let field = self.rect((24., 71., 592., 44.));
        self.ui.rounded(
            field,
            self.ui.px(10.) as f32,
            self.theme.surface,
            Some(self.theme.highlight_bg),
        )?;
        let text = keyboard.display();
        let mut size = 18.;
        while size > 12. && self.ui.measure(&text, Face::Mono, size)? as f32 > 530. {
            size -= 1.;
        }
        let width = self.ui.measure(&text, Face::Mono, size)? as f32 / self.ui.frame.scale;
        self.ui.label_aligned(
            &text,
            (Face::Mono, size, self.theme.text),
            self.rect((38., 71., 540., 44.)),
            Align::Left,
        )?;
        self.ui
            .canvas
            .set_draw_color(color(self.theme.highlight_bg));
        self.ui
            .canvas
            .fill_rect(self.rect((38. + width.min(540.) + 3., 83., 2., 20.)))?;
        if keyboard.mode == Mode::Secret {
            let eye = if keyboard.reveal {
                Glyph::EyeOff
            } else {
                Glyph::Eye
            };
            self.ui
                .icon(eye, self.rect((584., 84., 18., 18.)), self.theme.muted)?;
        }
        let submit = match state.keyboard_target {
            Some(Id::WifiSwitch) => "settings.connect",
            Some(Id::ScraperPassword) => "settings.sign_in_short",
            _ => "hint.confirm",
        };
        let labels = (self.tr("settings.space"), self.tr(submit));
        self.ui
            .keyboard_keys(keyboard, 129., (&labels.0, &labels.1), self.theme)
    }
    /// Config-scraper side panel: last or running collection.
    pub(super) fn scraper_panel(&mut self, state: &Settings) -> Result<(), String> {
        let scrape = &state.scrape;
        let (x, w) = (404., 220.);
        self.ui.rounded(
            self.rect((x, VIEW_TOP, w, 374.)),
            self.ui.px(12.) as f32,
            self.theme.surface,
            Some(self.theme.border),
        )?;
        let inner = (x + 14., w - 28.);
        let heading = if scrape.running {
            "settings.scraping_now"
        } else {
            "settings.last_scrape"
        };
        self.note(
            (inner.0, 62., inner.1, 18.),
            &self.tr(heading).to_uppercase(),
            12.,
        )?;
        let headline = if scrape.running || scrape.outcome.is_some() {
            let count = self.ui.catalog.format(
                "settings.scrape_count",
                &[
                    ("done", &scrape.done.to_string()),
                    ("total", &scrape.total.to_string()),
                ],
            );
            match scrape.outcome {
                Some(key) if !scrape.running => format!("{}\n{count}", self.tr(key)),
                _ => count,
            }
        } else {
            self.tr("settings.no_scrape_yet")
        };
        self.text_face(
            (inner.0, 84., inner.1, 44.),
            &headline,
            (15., self.theme.text, Align::Left),
            Face::Semibold,
        )?;
        let mut y = 134.;
        if scrape.running || scrape.outcome.is_some() {
            let percent = (scrape.done * 100).checked_div(scrape.total).unwrap_or(0);
            self.bar(
                (inner.0, y, inner.1, 6.),
                percent as u8,
                self.theme.highlight_bg,
            )?;
            let found = self.ui.catalog.format(
                "settings.scrape_found",
                &[
                    ("found", &scrape.scraped.to_string()),
                    ("missing", &scrape.missing.to_string()),
                ],
            );
            self.note((inner.0, y + 12., inner.1, 34.), &found, 12.)?;
            if scrape.running && !scrape.system.is_empty() {
                self.note((inner.0, y + 46., inner.1, 18.), &scrape.system, 12.)?;
            }
            y += 70.;
        }
        self.ui.divider(
            self.ui.px(inner.0),
            self.ui.px(y),
            self.ui.px(inner.1) as u32,
            self.theme.border,
        )?;
        let note = if scrape.available {
            "settings.scraper_note"
        } else {
            "settings.scraper_unavailable"
        };
        self.note((inner.0, y + 10., inner.1, 410. - y), &self.tr(note), 12.)
    }
}
