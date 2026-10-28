//! Appearance page (Aparencia): the rows, accent swatches and a live preview.
use super::*;
use crate::appearance::{rgb, ACCENT_RGB};

/// Rows stop short of the preview column.
const LIST_RIGHT: f32 = 372.;
const PREVIEW_X: f32 = 390.;
const PREVIEW_W: f32 = 230.;
const SYSTEMS: [(&str, &str); 4] = [
    ("Game Boy Advance", "146"),
    ("Super Nintendo", "213"),
    ("Mega Drive", "98"),
    ("Nintendo 64", "41"),
];

impl View<'_, '_, '_> {
    pub(super) fn appearance_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        self.rows_at(state, rows, &placements(Page::Appearance, rows), LIST_RIGHT)?;
        let draft = state.appearance();
        let theme = draft.apply(&state.base_theme);
        // Text size is previewed here only; the page itself changes on leaving.
        let scale = std::mem::replace(&mut self.ui.text_scale, draft.text_scale());
        self.ui.frame.text = self.ui.text_scale;
        let preview = self.preview(&theme);
        self.ui.text_scale = scale;
        self.ui.frame.text = scale;
        preview?;
        self.note(
            (PREVIEW_X, 292., PREVIEW_W, 60.),
            &self.tr("appearance.preview_note"),
            12.,
        )
    }
    /// The accent choices as dots; a ring marks the one in effect.
    pub(super) fn swatches(
        &mut self,
        state: &Settings,
        selected: bool,
        (x, y, w, h): Bounds,
    ) -> Result<(), String> {
        let active = state.appearance().apply(&state.base_theme).highlight_bg;
        let ink = self.ink(selected);
        let behind = selection(selected, self.theme.background, self.theme.highlight_bg);
        let mut left = x + w - (ACCENT_RGB.len() as f32 * 27. - 9.);
        for hex in ACCENT_RGB {
            let color = rgb(hex);
            let (cy, ring) = (y + h / 2., color == active);
            if ring {
                for (size, fill) in [(26., ink), (22., behind)] {
                    let bounds = (left + 9. - size / 2., cy - size / 2., size, size);
                    self.ui
                        .rounded(self.rect(bounds), self.ui.px(size / 2.) as f32, fill, None)?;
                }
            }
            self.ui.rounded(
                self.rect((left, cy - 9., 18., 18.)),
                self.ui.px(9.) as f32,
                color,
                None,
            )?;
            left += 27.;
        }
        Ok(())
    }
    /// A miniature launcher drawn with the draft theme.
    fn preview(&mut self, theme: &Theme) -> Result<(), String> {
        self.text(
            (PREVIEW_X, 50., PREVIEW_W, 18.),
            &self.tr("appearance.preview").to_uppercase(),
            (11., self.theme.muted, Align::Left),
        )?;
        let (x, y) = (PREVIEW_X, 74.);
        self.ui.rounded(
            self.rect((x, y, PREVIEW_W, 210.)),
            self.ui.px(12.) as f32,
            theme.background,
            Some(theme.border),
        )?;
        self.text_face(
            (x + 12., y + 10., 90., 18.),
            "ARCHR",
            (10., theme.text, Align::Left),
            Face::Semibold,
        )?;
        let pill = (x + PREVIEW_W - 10. - 86., y + 10., 86., 18.);
        self.ui.rounded(
            self.rect(pill),
            self.ui.px(9.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        self.text_face(
            pill,
            "14:32 · 87%",
            (10., theme.muted, Align::Center),
            Face::Mono,
        )?;
        for (n, (name, count)) in SYSTEMS.iter().enumerate() {
            let row = (x + 10., y + 38. + n as f32 * 28., PREVIEW_W - 20., 24.);
            let on = n == 1;
            if on {
                self.ui.rounded(
                    self.rect(row),
                    self.ui.px(7.) as f32,
                    theme.highlight_bg,
                    None,
                )?;
            }
            let (ink, dim) = if on {
                (theme.highlight_text, theme.accent_text_secondary)
            } else {
                (theme.text, theme.dim_text)
            };
            let inner = (row.0 + 10., row.1, row.2 - 20., row.3);
            self.text_face(
                inner,
                name,
                (12., ink, Align::Left),
                selection(on, Face::Regular, Face::Semibold),
            )?;
            self.text_face(inner, count, (10., dim, Align::Right), Face::Mono)?;
        }
        self.ui.divider(
            self.ui.px(x + 10.),
            self.ui.px(y + 182.),
            self.ui.px(PREVIEW_W - 20.) as u32,
            theme.border,
        )?;
        let hints = format!(
            "A  {}     Y  {}",
            self.tr("hint.open"),
            self.tr("hint.menu")
        );
        self.text(
            (x + 10., y + 186., PREVIEW_W - 20., 16.),
            &hints,
            (10., theme.muted, Align::Right),
        )
    }
}
