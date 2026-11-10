//! Error reports (Relatorios-ajustes): the rows, then what a report holds
//! on the right; and the last report's text (Relatorios-ver), monospace,
//! as it was sent. Fixed layout; the theme gives colours and fonts.
use super::*;
use crate::settings::reports::{COLUMNS, PAGE_LINES};

/// Left edge and width of the side panel; the rows stop 16 px before it.
const PANEL: (f32, f32) = (420., 200.);
const ROWS_RIGHT: f32 = PANEL.0 - 16.;
/// The viewer's text box: its left, top and width, and its line pitch.
const TEXT: (f32, f32, f32) = (20., VIEW_TOP + 4., 600.);
const STEP: f32 = 18.;

impl View<'_, '_, '_> {
    pub(super) fn reports_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        self.rows_at(
            state,
            rows,
            &placements(Page::ErrorReports, rows),
            ROWS_RIGHT,
        )?;
        let (x, w) = PANEL;
        let panel = self.view_bottom() - VIEW_TOP - 4.;
        self.panel((x, VIEW_TOP + 4., w, panel), false)?;
        let (left, inner) = (x + 14., w - 28.);
        // Small screens raise the smallest size: measure at the drawn one.
        let least = self.ui.frame.min_size();
        let small = |size: f32| size.max(least);
        let mut y = VIEW_TOP + 18.;
        for (title, body) in [
            ("reports.goes", "reports.goes_detail"),
            ("reports.stays", "reports.stays_detail"),
        ] {
            self.text_face(
                (left, y, inner, 20.),
                &self.tr(title),
                (14., self.theme.text, Align::Left),
                Face::Semibold,
            )?;
            y += 24.;
            let (size, text) = (small(12.), self.tr(body));
            let width = self.rect((left, y, inner, 1.)).width();
            let height = self.lines(&text, size, width)?.len() as f32 * self.pitch(size, 18.);
            self.note((left, y, inner, height), &text, 12.)?;
            y += height + 12.;
        }
        let (size, note) = (small(11.), self.tr("reports.deleted"));
        let width = self.rect((left, 0., inner, 1.)).width();
        let height = self.lines(&note, size, width)?.len() as f32 * self.pitch(size, 15.);
        let bottom = VIEW_TOP + 4. + panel - 14.;
        self.note((left, bottom - height, inner, height), &note, 11.)
    }
    pub(super) fn report_text(&mut self, state: &Settings) -> Result<(), String> {
        let Some(last) = state.reports_last() else {
            return Ok(());
        };
        let (x, y, w) = TEXT;
        // As tall as the screen allows; a taller one shows more lines.
        let h = self.view_bottom() - y - 6.;
        self.panel((x, y, w, h), false)?;
        // As large as a full line allows, 12 px at most.
        let room = self.rect((0., 0., w - 32., 1.)).width();
        let full = "M".repeat(COLUMNS);
        let floor = self.ui.frame.min_size().max(8.);
        let mut size = 12.;
        while size > floor && self.ui.measure(&full, Face::Mono, size)? > room {
            size -= 0.5;
        }
        let shown = (((h - 24.) / STEP) as usize).max(PAGE_LINES);
        let first = state.reports.scroll.min(last.lines.len());
        for (n, line) in last.lines[first..].iter().take(shown).enumerate() {
            if line.is_empty() {
                continue;
            }
            self.ui.label_aligned(
                line,
                (Face::Mono, size, self.theme.muted),
                self.rect((x + 16., y + 12. + n as f32 * STEP, w - 32., STEP)),
                Align::Left,
            )?;
        }
        Ok(())
    }
    /// The viewer's footer: when it went out, or that it waits.
    pub(super) fn report_status(&self, state: &Settings) -> Option<String> {
        let last = state.reports_last()?;
        Some(if last.sent {
            self.tr(&format!("reports.sent_footer|time={}", last.sent_at))
        } else {
            self.tr("reports.queued_footer")
        })
    }
}
