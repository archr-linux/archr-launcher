//! Controls pages (Controles-launcher, Atalhos-jogos): button capsules on the
//! right of each row, and the dashed prompt while a press is awaited.
use super::*;

const PILL: f32 = 24.;
const GAP: f32 = 5.;

impl View<'_, '_, '_> {
    /// Capsules right-aligned in the control area, joined by "+"; `None` is
    /// the dashed "press a button" prompt.
    pub(super) fn pads(
        &mut self,
        names: &[Option<&str>],
        selected: bool,
        (x, y, w, h): Bounds,
    ) -> Result<(), String> {
        let theme = self.theme;
        let (fill, ink, plus) = selection(
            selected,
            (theme.badge, theme.text, theme.dim_text),
            (
                theme.highlight_text,
                theme.highlight_bg,
                theme.accent_text_secondary,
            ),
        );
        let top = y + (h - PILL) / 2.;
        let mut right = x + w;
        for (n, name) in names.iter().enumerate().rev() {
            let (text, face, size, pad) = match name {
                Some(name) => ((*name).to_owned(), Face::Semibold, 12., 9.),
                None => (self.tr("controls.press"), Face::Regular, 12., 10.),
            };
            let width = (self.ui.measure(&text, face, size)? as f32 / self.ui.frame.scale
                + 2. * pad)
                .max(PILL);
            let left = right - width;
            let pill = self.rect((left, top, width, PILL));
            let radius = self.ui.px(PILL / 2.) as f32;
            let text_ink = if name.is_some() {
                self.ui.rounded(pill, radius, fill, None)?;
                ink
            } else {
                let line = selection(selected, theme.highlight_bg, theme.highlight_text);
                self.ui
                    .geometry
                    .dashed(&mut self.ui.canvas, pill, radius, line)?;
                selection(selected, theme.text, theme.highlight_text)
            };
            self.ui
                .label_aligned(&text, (face, size, text_ink), pill, Align::Center)?;
            right = left - GAP;
            if n > 0 {
                self.text((right - 8., y, 8., h), "+", (12., plus, Align::Center))?;
                right -= 8. + GAP;
            }
        }
        Ok(())
    }
}
