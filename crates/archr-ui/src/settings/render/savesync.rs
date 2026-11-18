//! Save sync: the rows beside their card (Saves-sync, Saves-destino), and
//! a conflict (Saves-conflito) with its two copies side by side. A theme
//! with a `[dialog]` style (Ellegance) shows the conflict as its dialog,
//! glass, disc and buttons; the others as a page.
use super::*;
use crate::settings::savesync::Conflict;

impl View<'_, '_, '_> {
    pub(super) fn savesync_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let right = picture::ROWS_RIGHT;
        self.rows_at(state, rows, &placements(state.page(), rows), right)?;
        match state.savesync_aside() {
            Some((blocks, foot)) => self.aside(blocks, foot),
            None => Ok(()),
        }
    }
    /// The mark of a destination row: a dot in a ring, or in a theme with
    /// grouped rows (Ellegance) a check on the chosen one only.
    pub(super) fn savesync_radio(
        &mut self,
        chosen: bool,
        selected: bool,
        (x, y, w, h): Bounds,
    ) -> Result<(), String> {
        let ink = if selected {
            self.theme.highlight_text
        } else {
            self.theme.highlight_bg
        };
        if self.look().inset {
            if chosen {
                let mark = self.rect((x + w - 16., y + (h - 16.) / 2., 16., 16.));
                self.ui.icon(Glyph::Check, mark, ink)?;
            }
            return Ok(());
        }
        // A ring: the rim, then the row's own ground inside it.
        let ring = self.rect((x + w - 18., y + (h - 18.) / 2., 18., 18.));
        let rim = if selected { ink } else { self.theme.muted };
        self.ui.rounded(ring, ring.width() as f32 / 2., rim, None)?;
        let ground = if selected {
            self.theme.highlight_bg
        } else {
            self.theme.background
        };
        let hole = self.rect((x + w - 16., y + (h - 14.) / 2., 14., 14.));
        self.ui
            .rounded(hole, hole.width() as f32 / 2., ground, None)?;
        if chosen {
            let dot = self.rect((x + w - 13., y + (h - 8.) / 2., 8., 8.));
            self.ui.rounded(dot, dot.width() as f32 / 2., ink, None)?;
        }
        Ok(())
    }
    pub(super) fn savesync_conflict_page(&mut self, state: &Settings) -> Result<(), String> {
        let Some(conflict) = state.savesync_conflict() else {
            return self.text(
                (40., VIEW_TOP + 20., 560., 40.),
                &self.tr("savesync.no_conflicts"),
                (16., self.theme.muted, Align::Left),
            );
        };
        let style = self.theme.screens.dialog;
        if style != crate::screens::DialogStyle::default() {
            return self.conflict_dialog(state, conflict, style);
        }
        // Saves-conflito: title, game, the two cards and where the copy
        // not chosen goes.
        let (x, w) = (40., 560.);
        let mut y = VIEW_TOP + 10.;
        self.text_face(
            (x, y, w, 28.),
            &self.tr("savesync.changed_both"),
            (20., self.theme.text, Align::Left),
            Face::Semibold,
        )?;
        y += 30.;
        self.text(
            (x, y, w, 20.),
            &self.conflict_name(conflict),
            (13., self.theme.muted, Align::Left),
        )?;
        y += 34.;
        self.conflict_cards(state, conflict, (x, y, w, 96.), false)?;
        y += 96. + 18.;
        let text = self.tr("savesync.aside_conflicts");
        let width = self.rect((x, y, w, 1.)).width();
        let lines = self.lines(&text, 13., width)?.len() as f32;
        self.note((x, y, w, lines * 19.), &text, 13.)
    }
    fn conflict_name(&self, conflict: &Conflict) -> String {
        format!("{} · {}", conflict.game(), conflict.system().to_uppercase())
    }
    /// The two copies: this console's, then the folder's; the one A keeps
    /// is in focus.
    fn conflict_cards(
        &mut self,
        state: &Settings,
        conflict: &Conflict,
        (x, y, w, h): Bounds,
        ring: bool,
    ) -> Result<(), String> {
        let cw = (w - 12.) / 2.;
        let by = match conflict.remote.by.as_str() {
            "" => self.tr("savesync.there_detail"),
            by => self.tr(&format!("savesync.from|by={}", by.replace('|', ""))),
        };
        let cards = [
            (
                "savesync.here",
                conflict.local.at,
                self.tr("savesync.here_detail"),
            ),
            ("savesync.there", conflict.remote.at, by),
        ];
        for (n, (label, at, who)) in cards.into_iter().enumerate() {
            let bounds = (x + n as f32 * (cw + 12.), y, cw, h);
            let focused = usize::from(state.savesync.choice) == n;
            let (title, muted) = if ring {
                let fill = Rgba {
                    a: if focused { 0x1F } else { 0x0F },
                    ..self.theme.text
                };
                self.ui
                    .rounded(self.rect(bounds), self.ui.px(16.) as f32, fill, None)?;
                if focused {
                    self.lit_focus(bounds, 16.)?;
                }
                (self.theme.text, self.theme.muted)
            } else if focused {
                self.row_panel(bounds, true)?;
                (self.theme.highlight_text, self.theme.accent_text_secondary)
            } else {
                self.panel(bounds, false)?;
                (self.theme.text, self.theme.muted)
            };
            let (left, inner) = (bounds.0 + 14., cw - 28.);
            self.text(
                (left, y + 12., inner, 16.),
                &self.tr(label).to_uppercase(),
                (11., muted, Align::Left),
            )?;
            self.text_face(
                (left, y + 32., inner, 24.),
                &self.tr(&state.savesync_when(at)),
                (16., title, Align::Left),
                Face::Semibold,
            )?;
            // Two lines: a console's name can be long.
            self.text((left, y + 58., inner, 30.), &who, (12., muted, Align::Left))?;
        }
        Ok(())
    }
    /// Ellegance's conflict: the dialog of the theme, its disc, the cards
    /// and the three answers, the one A takes lit.
    fn conflict_dialog(
        &mut self,
        state: &Settings,
        conflict: &Conflict,
        style: crate::screens::DialogStyle,
    ) -> Result<(), String> {
        let w: f32 = 440.;
        let x = ((640. - w) / 2.).round();
        let tw = w - 32.;
        let note = self.tr("savesync.aside_conflicts");
        let width = self.rect((x + 16., 0., tw, 1.)).width();
        let note_h = self.lines(&note, 12., width)?.len() as f32 * 17.;
        let total = 16. + 44. + 10. + 22. + 20. + 12. + 92. + 12. + note_h + 14. + 36. + 16.;
        let top = ((VIEW_TOP + 432. - total) / 2.).round().max(VIEW_TOP - 30.);
        let rect = self.rect((x, top, w, total));
        let radius = self.ui.px(style.radius.map_or(14., |r| r * 480.)) as f32;
        if style.glass {
            self.ui.cast_shadow(rect, radius)?;
            self.ui.rounded(
                rect,
                radius,
                Rgba {
                    a: 240,
                    ..self.theme.surface
                },
                None,
            )?;
            self.ui
                .glass(rect, radius, &self.theme.glass_or_default())?;
        } else {
            self.ui
                .rounded(rect, radius, self.theme.surface, Some(self.theme.border))?;
        }
        let mut y = top + 16.;
        let disc = self.rect((320. - 22., y, 44., 44.));
        let ring = mix(self.theme.surface, self.theme.warning, 0.22);
        self.ui
            .rounded(disc, disc.width() as f32 / 2., ring, None)?;
        let glyph = self.rect((320. - 10., y + 12., 20., 20.));
        self.ui
            .icon(Glyph::ArrowLeftRight, glyph, self.theme.warning)?;
        y += 44. + 10.;
        self.text_face(
            (x + 16., y, tw, 22.),
            &self.tr("savesync.changed_both"),
            (16., self.theme.text, Align::Center),
            Face::Semibold,
        )?;
        y += 22.;
        self.text(
            (x + 16., y, tw, 20.),
            &self.conflict_name(conflict),
            (13., self.theme.muted, Align::Center),
        )?;
        y += 20. + 12.;
        self.conflict_cards(state, conflict, (x + 16., y, tw, 92.), true)?;
        y += 92. + 12.;
        self.text(
            (x + 16., y, tw, note_h),
            &note,
            (12., self.theme.muted, Align::Center),
        )?;
        y += note_h + 14.;
        let answers = [
            ("savesync.keep_this", state.savesync.choice == 0),
            ("savesync.use_pc", state.savesync.choice == 1),
            ("savesync.later", false),
        ];
        let bw = (tw - 16.) / 3.;
        for (n, (key, lit)) in answers.into_iter().enumerate() {
            let at = (x + 16. + n as f32 * (bw + 8.), y, bw, 36.);
            let (fill, ink) = if lit {
                (self.theme.highlight_bg, self.theme.highlight_text)
            } else {
                (self.theme.surface_elevated, self.theme.text)
            };
            let pill = self.rect(at);
            self.ui
                .rounded(pill, pill.height() as f32 / 2., fill, None)?;
            self.text_face(at, &self.tr(key), (13., ink, Align::Center), Face::Semibold)?;
        }
        Ok(())
    }
}
