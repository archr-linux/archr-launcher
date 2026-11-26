//! The 640×480 information grid and live detail panels.
use super::*;
use crate::settings::{
    information::{CPU, ITEMS, STORAGE},
    InfoCard,
};

impl View<'_, '_, '_> {
    pub(super) fn info(&mut self, state: &Settings) -> Result<(), String> {
        if self.look().inset {
            return self.info_inset(state);
        }
        let info = &state.info;
        self.text_face(
            (24., 44., 77., 36.),
            "ArchR",
            (26., self.theme.text, Align::Left),
            Face::Semibold,
        )?;
        // "v3.0.3 · 20261118.3" on one line; the device line, right-aligned,
        // keeps room for "Game Console R36S Soysauce · kernel 6.12.114".
        self.text_face(
            (113., 44., 175., 36.),
            &info.version,
            (14., self.theme.highlight_bg, Align::Left),
            Face::Mono,
        )?;
        let device = self.ui.catalog.format(
            "settings.device_line",
            &[("device", &info.device), ("kernel", &info.kernel)],
        );
        self.text_face(
            (292., 44., 324., 36.),
            &device,
            (12., self.theme.muted, Align::Right),
            Face::Mono,
        )?;
        for (index, card) in info.cards.iter().take(6).enumerate() {
            let x = 24. + (index % 2) as f32 * 300.;
            let y = 84. + (index / 2) as f32 * 108.;
            let selected = index == state.cursor;
            if selected {
                // An inset fill and two-pixel ring, not the settings selection pill.
                self.ui.rounded(
                    self.rect((x - 3., y - 3., 298., 106.)),
                    self.ui.px(15.) as f32,
                    Rgba {
                        a: 46,
                        ..self.theme.highlight_bg
                    },
                    None,
                )?;
                self.ui.rounded(
                    self.rect((x, y, 292., 100.)),
                    self.ui.px(12.) as f32,
                    self.theme.highlight_bg,
                    None,
                )?;
                self.ui.rounded(
                    self.rect((x + 2., y + 2., 288., 96.)),
                    self.ui.px(10.) as f32,
                    tint(self.theme.surface, self.theme.highlight_bg),
                    None,
                )?;
            } else {
                self.ui.rounded(
                    self.rect((x, y, 292., 100.)),
                    self.ui.px(12.) as f32,
                    self.theme.surface,
                    Some(self.theme.border),
                )?;
            }
            self.ui.icon(
                ITEMS[index].1,
                self.rect((x + 14., y + 12., 16., 16.)),
                self.theme.highlight_bg,
            )?;
            self.note(
                (x + 38., y + 9., 240., 22.),
                &self.tr(ITEMS[index].0).to_uppercase(),
                12.,
            )?;
            let main = card
                .lines
                .first()
                .filter(|s| !s.is_empty())
                .map(|s| self.tr(s))
                .unwrap_or_else(|| self.tr("settings.unavailable"));
            self.text_face(
                (x + 14., y + 33., 264., 25.),
                &main,
                (15., self.theme.text, Align::Left),
                Face::Semibold,
            )?;
            let secondary = card.lines.get(1).map(|s| self.tr(s)).unwrap_or_default();
            if let Some(value) = card.percent {
                self.bar(
                    (x + 14., y + 64., 264., 6.),
                    value,
                    self.theme.highlight_bg,
                    false,
                )?;
                self.text_face(
                    (x + 14., y + 73., 264., 20.),
                    &secondary,
                    (12., self.theme.muted, Align::Left),
                    Face::Mono,
                )?;
            } else {
                self.text_face(
                    (x + 14., y + 61., 264., 28.),
                    &secondary,
                    (12., self.theme.muted, Align::Left),
                    Face::Mono,
                )?;
            }
        }
        Ok(())
    }
    /// The grid in a theme of inset groups (Ellegance), under its large
    /// title: the version and device on one quiet line, the cards as groups
    /// with the rows' margins, the selected one in the theme's focus ring.
    fn info_inset(&mut self, state: &Settings) -> Result<(), String> {
        let info = &state.info;
        let (left, right) = (16., RIGHT);
        let width = (right - left - 8.) / 2.;
        let version = format!("ArchR {}", info.version);
        self.text_face(
            (28., VIEW_TOP - 4., 200., 20.),
            &version,
            (13., self.theme.highlight_bg, Align::Left),
            Face::Mono,
        )?;
        let device = self.ui.catalog.format(
            "settings.device_line",
            &[("device", &info.device), ("kernel", &info.kernel)],
        );
        self.text_face(
            (232., VIEW_TOP - 4., right - 12. - 232., 20.),
            &device,
            (12., self.theme.muted, Align::Right),
            Face::Mono,
        )?;
        // Three rows of cards down to the end of the page's area.
        let top = VIEW_TOP + 22.;
        let height = ((self.view_bottom() - top - 16.) / 3.).min(100.);
        for (index, card) in info.cards.iter().take(6).enumerate() {
            let x = left + (index % 2) as f32 * (width + 8.);
            let y = top + (index / 2) as f32 * (height + 8.);
            let bounds = (x, y, width, height);
            self.ui.rounded(
                self.rect(bounds),
                self.ui.px(14.) as f32,
                self.group_fill(),
                None,
            )?;
            if index == state.cursor {
                self.lit_focus(bounds, 14.)?;
            }
            self.ui.icon(
                ITEMS[index].1,
                self.rect((x + 14., y + 12., 16., 16.)),
                self.theme.highlight_bg,
            )?;
            self.note(
                (x + 38., y + 9., width - 52., 22.),
                &self.tr(ITEMS[index].0).to_uppercase(),
                12.,
            )?;
            let main = card
                .lines
                .first()
                .filter(|s| !s.is_empty())
                .map(|s| self.tr(s))
                .unwrap_or_else(|| self.tr("settings.unavailable"));
            self.text_face(
                (x + 14., y + 31., width - 28., 25.),
                &main,
                (15., self.theme.text, Align::Left),
                Face::Semibold,
            )?;
            let secondary = card.lines.get(1).map(|s| self.tr(s)).unwrap_or_default();
            if let Some(value) = card.percent {
                self.bar(
                    (x + 14., y + 60., width - 28., 6.),
                    value,
                    self.theme.highlight_bg,
                    false,
                )?;
                self.text_face(
                    (x + 14., y + 68., width - 28., 20.),
                    &secondary,
                    (12., self.theme.muted, Align::Left),
                    Face::Mono,
                )?;
            } else {
                self.text_face(
                    (x + 14., y + 58., width - 28., 26.),
                    &secondary,
                    (12., self.theme.muted, Align::Left),
                    Face::Mono,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn info_detail(&mut self, state: &Settings, index: usize) -> Result<(), String> {
        let Some(card) = state.info.cards.get(index) else {
            return Ok(());
        };
        // Under a large title (inset themes) the item's name is the title:
        // the subtitle alone, and the panels 38 px higher, as groups.
        let inset = self.look().inset && self.title_below(state);
        let dy = if inset { -38. } else { 0. };
        if inset {
            let line =
                if card.detail.heading.is_empty() || card.detail.heading == ITEMS[index.min(5)].0 {
                    self.tr(&card.detail.subtitle)
                } else {
                    format!(
                        "{} · {}",
                        self.tr(&card.detail.heading),
                        self.tr(&card.detail.subtitle)
                    )
                };
            self.note((28., VIEW_TOP - 6., 588., 20.), &line, 12.)?;
        } else {
            self.ui.icon(
                ITEMS[index.min(5)].1,
                self.rect((24., 54., 28., 28.)),
                self.theme.highlight_bg,
            )?;
            let heading = if card.detail.heading.is_empty() {
                self.tr(ITEMS[index].0)
            } else {
                self.tr(&card.detail.heading)
            };
            self.text_face(
                (64., 44., 552., 32.),
                &heading,
                (20., self.theme.text, Align::Left),
                Face::Semibold,
            )?;
            self.note((64., 76., 552., 22.), &self.tr(&card.detail.subtitle), 12.)?;
        }
        if index == STORAGE {
            self.detail_panel((24., 106. + dy, 592., 310.), inset)?;
            // Each actual mounted filesystem has a label, usage, and its own bar.
            for (n, meter) in card.detail.meters.iter().take(5).enumerate() {
                let y = 118. + dy + n as f32 * 58.;
                self.label((38., y, 564., 18.), &meter.label, 13.)?;
                self.note((38., y + 20., 564., 18.), &self.tr(&meter.value), 12.)?;
                if let Some(value) = meter.percent {
                    self.bar(
                        (38., y + 43., 564., 6.),
                        value,
                        self.theme.highlight_bg,
                        false,
                    )?;
                }
            }
            if card.detail.meters.is_empty() {
                self.note(
                    (38., 118. + dy, 336., 36.),
                    &self.tr("settings.unavailable"),
                    14.,
                )?;
            }
            return Ok(());
        } else {
            self.detail_panel((24., 106. + dy, 364., 310.), inset)?;
            self.detail_panel((402., 106. + dy, 214., 310.), inset)?;
            for (n, (key, value)) in card.detail.rows.iter().take(8).enumerate() {
                let y = 106. + dy + n as f32 * 34.;
                if n > 0 {
                    self.ui.divider(
                        self.ui.px(24.),
                        self.ui.px(y),
                        self.ui.px(364.) as u32,
                        if inset {
                            self.group_rule()
                        } else {
                            self.theme.border
                        },
                    )?;
                }
                self.note((38., y, 160., 34.), &self.tr(key), 14.)?;
                let value = if value.is_empty() {
                    self.tr("settings.unavailable")
                } else {
                    self.tr(value)
                };
                self.text_face(
                    (205., y, 169., 34.),
                    &value,
                    (14., self.theme.text, Align::Right),
                    Face::Mono,
                )?;
            }
        }
        self.detail_meters(card, index, dy)
    }
    /// A detail panel: the theme's group fill when inset, else the panel.
    fn detail_panel(&mut self, bounds: Bounds, inset: bool) -> Result<(), String> {
        if inset {
            let fill = self.group_fill();
            self.ui
                .rounded(self.rect(bounds), self.ui.px(14.) as f32, fill, None)
        } else {
            self.panel(bounds, false)
        }
    }
    fn detail_meters(&mut self, card: &InfoCard, index: usize, dy: f32) -> Result<(), String> {
        let mut y = 118. + dy;
        let meters = &card.detail.meters;
        if index == CPU {
            self.note(
                (416., y, 186., 25.),
                &self.tr("settings.per_core").to_uppercase(),
                12.,
            )?;
            y += 32.;
            let cores: Vec<_> = meters
                .iter()
                .filter(|m| m.label.starts_with("CPU"))
                .collect();
            let count = cores.len().clamp(1, 8);
            let width = (186. - (count - 1) as f32 * 8.) / count as f32;
            for (n, meter) in cores.iter().take(count).enumerate() {
                let x = 416. + n as f32 * (width + 8.);
                self.ui.rounded(
                    self.rect((x, y, width, 46.)),
                    self.ui.px(6.) as f32,
                    self.theme.border,
                    None,
                )?;
                if let Some(value) = meter.percent {
                    let height = 46. * f32::from(value) / 100.;
                    if height > 0. {
                        self.ui.rounded(
                            self.rect((x, y + 46. - height, width, height)),
                            0.,
                            self.theme.highlight_bg,
                            None,
                        )?;
                    }
                }
                self.text(
                    (x, y + 50., width, 32.),
                    &format!("{}\n{}", meter.label, meter.value),
                    (12., self.theme.muted, Align::Center),
                )?;
            }
            y += 104.;
        }
        for meter in meters
            .iter()
            .filter(|m| index != CPU || !m.label.starts_with("CPU"))
            .take(3)
        {
            self.note(
                (416., y, 186., 30.),
                &self.tr(&meter.label).to_uppercase(),
                12.,
            )?;
            if let Some(value) = meter.percent {
                self.bar(
                    (416., y + 34., 186., 6.),
                    value,
                    self.theme.highlight_bg,
                    false,
                )?;
            }
            let value = if meter.value.is_empty() {
                self.tr("settings.unavailable")
            } else {
                self.tr(&meter.value)
            };
            self.text_face(
                (416., y + 45., 186., 34.),
                &value,
                (12., self.theme.muted, Align::Left),
                Face::Mono,
            )?;
            y += 90.;
        }
        Ok(())
    }
}

fn tint(surface: Rgba, accent: Rgba) -> Rgba {
    let channel =
        |base: u8, accent: u8| ((u16::from(base) * 88 + u16::from(accent) * 12) / 100) as u8;
    Rgba {
        r: channel(surface.r, accent.r),
        g: channel(surface.g, accent.g),
        b: channel(surface.b, accent.b),
        a: 255,
    }
}
