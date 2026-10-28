//! Grafite settings presentation. All pages share rows, controls and navigation.
use super::*;
use crate::settings::{Id, Kind, Page, Settings, CATEGORIES};
impl Ui<'_, '_> {
    pub fn invalidate_settings_clock(&mut self) {
        if let Some(status) = &mut self.status {
            status.invalidate_clock();
        }
    }

    fn setting_rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(self.px(x), self.px(y), self.px(w) as u32, self.px(h) as u32)
    }
    fn fitting_label(
        &mut self,
        text: &str,
        face: Face,
        size: f32,
        c: Rgba,
        rect: Rect,
    ) -> Result<(), String> {
        let mut size = size;
        while size > 10. && self.measure(text, face, size)? > rect.width() {
            size -= 0.5;
        }
        self.label(text, (face, size, c), rect)
    }
    fn wrapped_label(&mut self, text: &str, size: f32, c: Rgba, rect: Rect) -> Result<(), String> {
        let mut line = String::new();
        let mut y = rect.y();
        let h = self.px(size + 6.) as u32;
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && self.measure(&candidate, Face::Regular, size)? > rect.width() {
                self.fitting_label(
                    &line,
                    Face::Regular,
                    size,
                    c,
                    Rect::new(rect.x(), y, rect.width(), h),
                )?;
                y += h as i32;
                line = word.into();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            self.fitting_label(
                &line,
                Face::Regular,
                size,
                c,
                Rect::new(rect.x(), y, rect.width(), h),
            )?;
        }
        Ok(())
    }
    pub(super) fn settings_page(
        &mut self,
        state: &Settings,
        theme: &Theme,
        data: &dyn DataSource,
    ) -> Result<(), String> {
        // Reuse the status pill, then replace only the title area beneath it.
        self.header(&Screen::Systems, 0, data, theme)?;
        let pill_width = self.status.as_ref().map(|status| {
            let clock = status.clock.clone();
            let battery = status.battery;
            let wifi = status.wifi;
            (clock, battery, wifi)
        });
        let title_right = if let Some((clock, battery, wifi)) = pill_width {
            let cw = self.measure(&clock, Face::Mono, 13.)?;
            let pw = battery
                .map(|b| self.measure(&format!("{}%", b.percent), Face::Mono, 13.))
                .transpose()?;
            status_layout(
                self.frame.width as i32 - self.px(24.),
                (theme.header_height * self.frame.height as f32) as u32,
                self.frame.scale,
                cw,
                pw,
                wifi,
            )
            .pill
            .x() - self.px(12.)
        } else {
            self.frame.width as i32 - self.px(24.)
        };
        let title_rect = Rect::new(0, 0, title_right.max(1) as u32, self.px(50.) as u32);
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.fill_rect(title_rect)?;
        let title = if state.page() == Page::Root {
            self.catalog.text("settings.title")
        } else {
            format!(
                "{} / {}",
                self.catalog.text("settings.title"),
                self.catalog.text(state.page().title())
            )
        };
        self.fitting_label(
            &title,
            Face::Semibold,
            15.,
            theme.text,
            Rect::new(
                self.px(24.),
                0,
                (title_right - self.px(24.)).max(1) as u32,
                self.px(48.) as u32,
            ),
        )?;
        match state.page() {
            Page::Root => self.settings_root(state, theme)?,
            Page::Info => self.settings_info(state, theme)?,
            Page::Soon(_) => {
                let rect = self.setting_rect(24., 90., 592., 100.);
                self.rounded(rect, self.px(12.) as f32, theme.surface, Some(theme.border))?;
                let label = self.catalog.text("settings.soon");
                self.label_aligned(
                    &label,
                    (Face::Semibold, 20., theme.text),
                    rect,
                    Align::Center,
                )?;
            }
            page => {
                let step = if state.error.is_some() { 48. } else { 56. };
                for (index, row) in page.rows().iter().enumerate() {
                    let rect = self.setting_rect(24., 78. + index as f32 * step, 592., 46.);
                    let selected = index == state.cursor;
                    self.rounded(
                        rect,
                        self.px(10.) as f32,
                        if selected {
                            theme.highlight_bg
                        } else {
                            theme.surface
                        },
                        if selected { None } else { Some(theme.border) },
                    )?;
                    let text = if selected {
                        theme.highlight_text
                    } else {
                        theme.text
                    };
                    let secondary = if selected {
                        theme.accent_text_secondary
                    } else {
                        theme.muted
                    };
                    let label = self.catalog.text(row.label);
                    let label_rect = self.setting_rect(
                        38.,
                        78. + index as f32 * step,
                        246.,
                        if row.subtitle.is_some() { 27. } else { 46. },
                    );
                    self.fitting_label(
                        &label,
                        if selected {
                            Face::Semibold
                        } else {
                            Face::Regular
                        },
                        15.,
                        text,
                        label_rect,
                    )?;
                    if let Some(key) = row.subtitle {
                        self.label(
                            &self.catalog.text(key),
                            (Face::Regular, 12., secondary),
                            self.setting_rect(38., 104. + index as f32 * step, 290., 16.),
                        )?;
                    }
                    if state.pending == Some(row.id) {
                        let phase = (SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                            / 100)
                            % 4;
                        let label = format!(
                            "{} {}",
                            ["◴", "◷", "◶", "◵"][phase as usize],
                            self.catalog.text("settings.applying")
                        );
                        self.label_aligned(
                            &label,
                            (Face::Regular, 14., text),
                            self.setting_rect(350., 78. + index as f32 * step, 250., 46.),
                            Align::Right,
                        )?;
                    } else {
                        self.settings_control(row.id, row.kind, state, selected, rect, theme)?;
                    }
                }
            }
        }
        let visible_error = state.error.as_ref().filter(|(id, _)| match state.page() {
            Page::Root => true,
            Page::Info => false,
            page => id.is_none() || page.rows().iter().any(|row| Some(row.id) == *id),
        });
        if let Some((id, detail)) = visible_error {
            let label = id
                .and_then(|id| {
                    [Page::Sound, Page::System, Page::Region]
                        .iter()
                        .flat_map(|p| p.rows())
                        .find(|r| r.id == id)
                })
                .map(|r| self.catalog.text(r.label))
                .unwrap_or_else(|| self.catalog.text("settings.title"));
            let root = state.page() == Page::Root;
            let (x, y, w, h) = if root {
                (386., 320., 230., 108.)
            } else {
                (24., 370., 592., 62.)
            };
            self.rounded(
                self.setting_rect(x, y, w, h),
                self.px(10.) as f32,
                theme.surface_elevated,
                Some(theme.highlight_bg),
            )?;
            let message = if root {
                self.catalog.text("settings.failed")
            } else {
                format!("{} · {}", label, self.catalog.text("settings.failed"))
            };
            self.fitting_label(
                &message,
                Face::Semibold,
                13.,
                theme.text,
                self.setting_rect(x + 14., y + 5., w - 28., 20.),
            )?;
            let detail = if detail.starts_with("settings.") {
                self.catalog.text(detail)
            } else {
                detail.clone()
            };
            if root {
                self.wrapped_label(
                    &detail,
                    11.,
                    theme.muted,
                    self.setting_rect(x + 14., y + 28., w - 28., h - 34.),
                )?;
            } else {
                self.fitting_label(
                    &detail,
                    Face::Mono,
                    11.,
                    theme.muted,
                    self.setting_rect(x + 14., y + 28., w - 28., 22.),
                )?;
            }
        }
        let footer_y = self.px(440.);
        self.divider(0, footer_y, self.frame.width, theme.border)?;
        let hints: &[(char, &str)] = if state.page() == Page::Root {
            &[
                ('A', "hint.open"),
                ('B', "hint.back"),
                ('Y', "settings.info_short"),
            ]
        } else if state.page() == Page::Info || matches!(state.page(), Page::Soon(_)) {
            &[('B', "hint.back")]
        } else {
            &[
                ('L', "settings.change"),
                ('A', "hint.choose"),
                ('B', "hint.back"),
            ]
        };
        self.hints(
            hints,
            theme,
            self.frame.width,
            footer_y,
            self.px(40.) as u32,
        )?;
        if state.loading {
            self.label(
                &self.catalog.text("loading"),
                (Face::Regular, 13., theme.muted),
                self.setting_rect(24., 440., 180., 40.),
            )?;
        }
        if state.reboot_prompt {
            self.scrim(theme, 1.0)?;
            let modal = self.setting_rect(80., 158., 480., 164.);
            self.rounded(
                modal,
                self.px(14.) as f32,
                theme.surface_elevated,
                Some(theme.border),
            )?;
            self.wrapped_label(
                &self.catalog.text("settings.reboot_question"),
                18.,
                theme.text,
                self.setting_rect(104., 180., 432., 65.),
            )?;
            self.hints(
                &[('A', "menu.reboot"), ('B', "settings.later")],
                theme,
                self.px(550.) as u32,
                self.px(264.),
                self.px(34.) as u32,
            )?;
        }
        Ok(())
    }
    fn settings_control(
        &mut self,
        id: Id,
        kind: Kind,
        state: &Settings,
        selected: bool,
        rect: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let text = if selected {
            theme.highlight_text
        } else {
            theme.text
        };
        match kind {
            Kind::Toggle => {
                let on = state.value(id) == "1";
                let track = if selected && on {
                    theme.highlight_text
                } else if on {
                    theme.highlight_bg
                } else {
                    theme.badge
                };
                let knob = if selected && on {
                    theme.highlight_bg
                } else {
                    theme.text
                };
                let x = rect.right() - self.px(52.);
                let y = rect.y() + self.px(13.);
                self.rounded(
                    Rect::new(x, y, self.px(38.) as u32, self.px(20.) as u32),
                    self.px(10.) as f32,
                    track,
                    None,
                )?;
                self.rounded(
                    Rect::new(
                        x + self.px(if on { 20. } else { 2. }),
                        y + self.px(2.),
                        self.px(16.) as u32,
                        self.px(16.) as u32,
                    ),
                    self.px(8.) as f32,
                    knob,
                    None,
                )?;
            }
            Kind::Slider { min, max, .. } => {
                let value = state.value(id).parse::<u8>().unwrap_or(min).clamp(min, max);
                let track =
                    self.setting_rect(345., rect.y() as f32 / self.frame.scale + 20., 180., 6.);
                self.rounded(
                    track,
                    self.px(3.) as f32,
                    if selected {
                        theme.accent_text_secondary
                    } else {
                        theme.badge
                    },
                    None,
                )?;
                let width = track.width() as f32 * f32::from(value) / 100.;
                if width >= 1. {
                    self.rounded(
                        Rect::new(track.x(), track.y(), width as u32, track.height()),
                        self.px(3.) as f32,
                        if selected {
                            theme.highlight_text
                        } else {
                            theme.highlight_bg
                        },
                        None,
                    )?;
                }
                self.label_aligned(
                    &format!("{value}%"),
                    (Face::Mono, 14., text),
                    Rect::new(
                        rect.right() - self.px(74.),
                        rect.y(),
                        self.px(60.) as u32,
                        rect.height(),
                    ),
                    Align::Right,
                )?;
            }
            Kind::Value | Kind::Action => {
                let value = self.setting_value(id, state);
                let value = if kind == Kind::Value {
                    format!("‹  {value}  ›")
                } else {
                    value
                };
                let panel = Rect::new(
                    rect.x() + self.px(290.),
                    rect.y(),
                    rect.width().saturating_sub(self.px(304.) as u32),
                    rect.height(),
                );
                self.fitting_label(&value, Face::Regular, 14., text, panel)?;
            }
        }
        Ok(())
    }
    fn setting_value(&self, id: Id, state: &Settings) -> String {
        match (id, state.value(id)) {
            (Id::Gpu, "panfrost") => "Panfrost".into(),
            (Id::Gpu, "libmali") => "Libmali".into(),
            (Id::Language, "pt_BR") => self.catalog.text("settings.lang_pt"),
            (Id::Language, "en_US") => self.catalog.text("settings.lang_en"),
            (Id::Language, "es_ES") => self.catalog.text("settings.lang_es"),
            (Id::Language, "ru_RU") => self.catalog.text("settings.lang_ru"),
            (Id::Idle | Id::IdleGame, "0") => self.catalog.text("settings.never"),
            (Id::Idle | Id::IdleGame, value) => {
                self.catalog.format("settings.minutes", &[("count", value)])
            }
            (_, "") => self.catalog.text("settings.unavailable"),
            (_, value) => value.into(),
        }
    }
    fn settings_root(&mut self, state: &Settings, theme: &Theme) -> Result<(), String> {
        for (index, (_, key, _)) in CATEGORIES.iter().enumerate() {
            let selected = state.cursor == index;
            let rect = self.setting_rect(24., 66. + index as f32 * 40., 346., 38.);
            self.rounded(
                rect,
                self.px(10.) as f32,
                if selected {
                    theme.highlight_bg
                } else {
                    theme.surface
                },
                None,
            )?;
            let text = if selected {
                theme.highlight_text
            } else {
                theme.text
            };
            self.settings_icon(
                index,
                self.setting_rect(38., 77. + index as f32 * 40., 16., 16.),
                text,
            )?;
            self.fitting_label(
                &self.catalog.text(key),
                Face::Regular,
                15.,
                text,
                self.setting_rect(68., 66. + index as f32 * 40., 282., 38.),
            )?;
        }
        let (_, key, detail) = CATEGORIES[state.cursor.min(CATEGORIES.len() - 1)];
        self.rounded(
            self.setting_rect(386., 66., 230., 240.),
            self.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        self.fitting_label(
            &self.catalog.text(key),
            Face::Semibold,
            18.,
            theme.text,
            self.setting_rect(402., 84., 198., 48.),
        )?;
        self.wrapped_label(
            &self.catalog.text(detail),
            15.,
            theme.muted,
            self.setting_rect(402., 146., 198., 120.),
        )?;
        self.label(
            &self.catalog.text("settings.instant"),
            (Face::Regular, 12., theme.dim_text),
            self.setting_rect(402., 274., 198., 20.),
        )?;
        Ok(())
    }
    fn settings_icon(&mut self, index: usize, rect: Rect, c: Rgba) -> Result<(), String> {
        // Tiny monoline category pictograms: network, speaker, pad, play,
        // globe, appearance, trophy, chip and storage.
        type Segment = ((i32, i32), (i32, i32));
        let lines: &[Segment] = match index {
            0 => &[
                ((0, 5), (8, 0)),
                ((8, 0), (16, 5)),
                ((3, 9), (8, 6)),
                ((8, 6), (13, 9)),
                ((7, 14), (9, 14)),
            ],
            1 => &[
                ((0, 6), (5, 6)),
                ((5, 6), (10, 1)),
                ((10, 1), (10, 15)),
                ((10, 15), (5, 10)),
                ((5, 10), (0, 10)),
                ((0, 10), (0, 6)),
                ((14, 4), (16, 8)),
                ((16, 8), (14, 12)),
            ],
            2 => &[
                ((2, 4), (14, 4)),
                ((14, 4), (16, 13)),
                ((16, 13), (11, 13)),
                ((11, 13), (9, 10)),
                ((9, 10), (7, 10)),
                ((7, 10), (5, 13)),
                ((5, 13), (0, 13)),
                ((0, 13), (2, 4)),
                ((3, 7), (7, 7)),
                ((5, 5), (5, 9)),
                ((12, 7), (13, 7)),
            ],
            3 => &[((3, 1), (14, 8)), ((14, 8), (3, 15)), ((3, 15), (3, 1))],
            4 => &[
                ((0, 8), (16, 8)),
                ((8, 0), (8, 16)),
                ((2, 3), (14, 3)),
                ((2, 13), (14, 13)),
                ((2, 3), (0, 8)),
                ((0, 8), (2, 13)),
                ((14, 3), (16, 8)),
                ((16, 8), (14, 13)),
            ],
            5 => &[
                ((1, 2), (15, 2)),
                ((15, 2), (15, 14)),
                ((15, 14), (1, 14)),
                ((1, 14), (1, 2)),
                ((6, 2), (6, 14)),
            ],
            6 => &[
                ((3, 1), (13, 1)),
                ((13, 1), (12, 9)),
                ((12, 9), (8, 11)),
                ((8, 11), (4, 9)),
                ((4, 9), (3, 1)),
                ((8, 11), (8, 15)),
                ((4, 15), (12, 15)),
            ],
            7 => &[
                ((3, 3), (13, 3)),
                ((13, 3), (13, 13)),
                ((13, 13), (3, 13)),
                ((3, 13), (3, 3)),
                ((0, 6), (3, 6)),
                ((0, 10), (3, 10)),
                ((13, 6), (16, 6)),
                ((13, 10), (16, 10)),
            ],
            _ => &[
                ((1, 2), (15, 2)),
                ((15, 2), (15, 14)),
                ((15, 14), (1, 14)),
                ((1, 14), (1, 2)),
                ((4, 10), (12, 10)),
                ((4, 6), (6, 6)),
            ],
        };
        self.canvas.set_draw_color(color(c));
        for &((x1, y1), (x2, y2)) in lines {
            self.canvas.draw_line(
                (rect.x() + self.px(x1 as f32), rect.y() + self.px(y1 as f32)),
                (rect.x() + self.px(x2 as f32), rect.y() + self.px(y2 as f32)),
            )?;
        }
        Ok(())
    }
    fn settings_info(&mut self, state: &Settings, theme: &Theme) -> Result<(), String> {
        self.label(
            "ArchR",
            (Face::Semibold, 24., theme.text),
            self.setting_rect(24., 56., 94., 38.),
        )?;
        self.fitting_label(
            &state.version,
            Face::Mono,
            18.,
            theme.highlight_bg,
            self.setting_rect(128., 56., 480., 38.),
        )?;
        let device = if state.device.is_empty() {
            self.catalog.text("settings.unavailable")
        } else {
            state.device.clone()
        };
        self.fitting_label(
            &self.catalog.format(
                "settings.device_line",
                &[("device", &device), ("kernel", &state.kernel)],
            ),
            Face::Regular,
            13.,
            theme.muted,
            self.setting_rect(24., 96., 592., 24.),
        )?;
        for (index, card) in state.cards.iter().take(6).enumerate() {
            let x = 24. + (index % 2) as f32 * 304.;
            let y = 128. + (index / 2) as f32 * 102.;
            self.rounded(
                self.setting_rect(x, y, 288., 100.),
                self.px(10.) as f32,
                theme.surface,
                Some(theme.border),
            )?;
            self.label(
                &self.catalog.text(card.title),
                (Face::Semibold, 14., theme.text),
                self.setting_rect(x + 14., y + 8., 260., 22.),
            )?;
            let lines: Vec<_> = card.lines.iter().filter(|line| !line.is_empty()).collect();
            if lines.is_empty() {
                self.label(
                    &self.catalog.text("settings.unavailable"),
                    (Face::Regular, 13., theme.dim_text),
                    self.setting_rect(x + 14., y + 36., 260., 22.),
                )?;
            }
            for (n, line) in lines.iter().take(3).enumerate() {
                let text = if line.starts_with("settings.") {
                    self.catalog.text(line)
                } else {
                    (*line).clone()
                };
                self.fitting_label(
                    &text,
                    Face::Regular,
                    13.,
                    theme.muted,
                    self.setting_rect(x + 14., y + 31. + n as f32 * 17., 260., 18.),
                )?;
            }
            if let Some(percent) = card.percent {
                let bar = self.setting_rect(x + 14., y + 83., 260., 5.);
                self.rounded(bar, self.px(2.) as f32, theme.badge, None)?;
                if percent > 0 {
                    self.rounded(
                        Rect::new(
                            bar.x(),
                            bar.y(),
                            (bar.width() as f32 * f32::from(percent.min(100)) / 100.).max(1.)
                                as u32,
                            bar.height(),
                        ),
                        self.px(2.) as f32,
                        theme.highlight_bg,
                        None,
                    )?;
                }
            }
        }
        Ok(())
    }
}
