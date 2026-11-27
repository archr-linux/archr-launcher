//! The progress window: a long wait's title, its step, a bar, the bytes and
//! speed, and the steps in order. The launcher draws it the same on every page
//! and in every theme; a theme only gives the colours.
use super::*;
use crate::progress::{Progress, Transfer};

impl View<'_, '_, '_> {
    /// `steps` are locale keys, `progress.step` the one running; `cancelling`
    /// once B was pressed and the work has not ended yet.
    pub(super) fn progress_window(
        &mut self,
        title: &str,
        steps: &[&str],
        progress: &Progress,
        cancelling: bool,
    ) -> Result<(), String> {
        let theme = self.theme;
        let (x, y, w, h) = (100., 116., 440., 204.);
        let (left, inner) = (x + 22., w - 44.);
        self.panel((x, y, w, h), false)?;
        let percent = progress.transfer.and_then(|t| t.percent());
        self.text_face(
            (left, y + 18., inner - 76., 24.),
            title,
            (17., theme.text, Align::Left),
            Face::Semibold,
        )?;
        let now = if cancelling {
            self.tr("progress.cancelling")
        } else {
            steps
                .get(progress.step)
                .map_or_else(String::new, |key| self.tr(key))
        };
        self.note((left, y + 44., inner - 76., 16.), &now, 12.)?;
        if let Some(percent) = percent {
            self.text_face(
                (left + inner - 76., y + 20., 76., 28.),
                &format!("{percent}%"),
                (22., theme.text, Align::Right),
                Face::Mono,
            )?;
        }
        // The bar: the bytes while they move; full once that step is over.
        let fill = match percent {
            Some(percent) => f32::from(percent),
            None if progress.step > 0 => 100.,
            None => 0.,
        };
        let track = self.rect((left, y + 74., inner, 8.));
        self.ui
            .rounded(track, self.ui.px(4.) as f32, theme.border, None)?;
        if fill > 0. {
            let bar = self.rect((left, y + 74., (inner * fill / 100.).max(8.), 8.));
            self.ui
                .rounded(bar, self.ui.px(4.) as f32, theme.highlight_bg, None)?;
        }
        if let Some(transfer) = progress.transfer {
            let (text, left_text) = self.transfer_text(transfer);
            self.text_face(
                (left, y + 90., inner - 112., 16.),
                &text,
                (12., theme.muted, Align::Left),
                Face::Mono,
            )?;
            if let Some(left_text) = left_text {
                self.text_face(
                    (left + inner - 112., y + 90., 112., 16.),
                    &left_text,
                    (12., theme.muted, Align::Right),
                    Face::Mono,
                )?;
            }
        }
        // The steps: done and running lit, the rest faint.
        let top = y + 122.;
        let line = self.rect((left, top, inner, 1.));
        self.ui.rounded(line, 0., theme.border, None)?;
        let width = (inner / steps.len().max(1) as f32).min(132.);
        for (n, key) in steps.iter().enumerate() {
            let x = left + n as f32 * width;
            let lit = n <= progress.step;
            let disc = self.rect((x, top + 12., 16., 16.));
            let fill = if lit {
                theme.highlight_bg
            } else {
                theme.border
            };
            self.ui.rounded(disc, self.ui.px(8.) as f32, fill, None)?;
            let ink = if lit {
                theme.highlight_text
            } else {
                theme.muted
            };
            self.text_face(
                (x, top + 12., 16., 16.),
                &(n + 1).to_string(),
                (9., ink, Align::Center),
                Face::Semibold,
            )?;
            let ink = if n == progress.step {
                theme.text
            } else {
                theme.muted
            };
            let label = self.tr(key);
            self.text_face(
                (x + 22., top + 12., width - 26., 16.),
                &label,
                (11., ink, Align::Left),
                Face::Semibold,
            )?;
        }
        let keep_on = self.tr("progress.keep_on");
        self.note((left, top + 42., inner, 16.), &keep_on, 11.)
    }
    /// "9,4 de 17,5 MB · 2,6 MB/s" and "faltam ~4 s" when the speed is known.
    fn transfer_text(&self, transfer: Transfer) -> (String, Option<String>) {
        crate::progress::describe(&self.ui.catalog, transfer)
    }
    /// A row that carries its own wait (Progresso-linha): the title and the
    /// percent, a thin bar, then what it is doing with the bytes and the
    /// time left. `doing` names each step as it runs.
    pub(super) fn progress_row(
        &mut self,
        label: &str,
        (y, height): (f32, f32),
        right: f32,
        (progress, doing, cancelling): (&Progress, &[&str], bool),
    ) -> Result<(), String> {
        let theme = self.theme;
        let (left, inner) = (30., right - 60.);
        let top = y + (height - 42.).max(0.) / 2.;
        let percent = progress.transfer.and_then(|t| t.percent());
        self.text_face(
            (left, top, inner - 60., 18.),
            label,
            (15., theme.text, Align::Left),
            Face::Semibold,
        )?;
        if let Some(percent) = percent {
            self.text_face(
                (left + inner - 60., top, 60., 18.),
                &format!("{percent}%"),
                (14., theme.text, Align::Right),
                Face::Mono,
            )?;
        }
        let fill = match percent {
            Some(percent) => f32::from(percent),
            None if progress.step > 0 => 100.,
            None => 0.,
        };
        let track = self.rect((left, top + 22., inner, 5.));
        self.ui
            .rounded(track, self.ui.px(2.5) as f32, theme.border, None)?;
        if fill > 0. {
            let bar = self.rect((left, top + 22., (inner * fill / 100.).max(5.), 5.));
            self.ui
                .rounded(bar, self.ui.px(2.5) as f32, theme.highlight_bg, None)?;
        }
        let now = if cancelling {
            self.tr("progress.cancelling")
        } else {
            doing
                .get(progress.step)
                .map_or_else(String::new, |key| self.tr(key))
        };
        let (text, time_left) = match progress.transfer {
            Some(transfer) if !cancelling => {
                let (bytes, left) = self.transfer_text(transfer);
                (format!("{now} · {bytes}"), left)
            }
            _ => (now, None),
        };
        self.text_face(
            (left, top + 30., inner - 100., 15.),
            &text,
            (11., theme.muted, Align::Left),
            Face::Mono,
        )?;
        if let Some(time_left) = time_left {
            self.text_face(
                (left + inner - 100., top + 30., 100., 15.),
                &time_left,
                (11., theme.muted, Align::Right),
                Face::Mono,
            )?;
        }
        Ok(())
    }
}
