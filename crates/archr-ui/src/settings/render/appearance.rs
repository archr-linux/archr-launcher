//! Appearance page (Aparencia): the rows, accent swatches and a live preview.
use super::*;
use crate::appearance::{mix, rgb, Appearance, ACCENTS, ACCENT_RGB};

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
        self.draft_preview(state, &draft)?;
        self.note(
            (PREVIEW_X, 292., PREVIEW_W, 60.),
            &self.tr("appearance.preview_note"),
            12.,
        )
    }
    /// The preview column for `draft`, whose text size shows here only: the
    /// page itself changes on leaving.
    pub(super) fn draft_preview(
        &mut self,
        state: &Settings,
        draft: &Appearance,
    ) -> Result<(), String> {
        let theme = draft.apply(&state.base_theme);
        let scale = std::mem::replace(&mut self.ui.text_scale, draft.text_scale());
        self.ui.frame.text = self.ui.text_scale;
        let preview = self.preview(&theme);
        self.ui.text_scale = scale;
        self.ui.frame.text = scale;
        preview
    }
    /// The accent choices as dots: the theme's own first, the custom colour
    /// last as a colour wheel; a ring marks the one in effect.
    pub(super) fn swatches(
        &mut self,
        state: &Settings,
        selected: bool,
        (x, y, w, h): Bounds,
    ) -> Result<(), String> {
        let choices = state.accent_choices();
        let chosen = state.accent_index();
        let ink = self.ink(selected);
        let behind = selection(selected, self.theme.background, self.theme.highlight_bg);
        let dots = choices.len();
        let mut left = x + w - (dots as f32 * 26. - 8.);
        for (n, choice) in choices.iter().enumerate() {
            let color = if choice.is_empty() {
                Some(state.base_theme.highlight_bg)
            } else {
                ACCENTS
                    .iter()
                    .position(|name| name == choice)
                    .map(|i| rgb(ACCENT_RGB[i]))
            };
            let (cy, ring) = (y + h / 2., n == chosen);
            if ring {
                for (size, fill) in [(26., ink), (22., behind)] {
                    let bounds = (left + 9. - size / 2., cy - size / 2., size, size);
                    self.ui
                        .rounded(self.rect(bounds), self.ui.px(size / 2.) as f32, fill, None)?;
                }
            }
            if selected && !ring {
                // A thin dark edge keeps a dot of the row's own colour visible.
                let edge = (left - 1., cy - 10., 20., 20.);
                self.ui.rounded(
                    self.rect(edge),
                    self.ui.px(10.) as f32,
                    self.theme.background,
                    None,
                )?;
            }
            match color {
                Some(color) => self.ui.rounded(
                    self.rect((left, cy - 9., 18., 18.)),
                    self.ui.px(9.) as f32,
                    color,
                    None,
                )?,
                None => self.wheel(self.rect((left, cy - 9., 18., 18.)), behind)?,
            }
            left += 26.;
        }
        Ok(())
    }
    /// A conic rainbow disc with a hole in the middle, drawn pixel by pixel.
    fn wheel(&mut self, rect: Rect, hole: Rgba) -> Result<(), String> {
        const STOPS: [u32; 7] = [
            0xEF4444, 0xF59E0B, 0x22C55E, 0x06B6D4, 0x3B82F6, 0xA855F7, 0xEF4444,
        ];
        let radius = rect.width() as f32 / 2.;
        let canvas = &mut self.ui.canvas;
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        for dy in 0..rect.height() {
            for dx in 0..rect.width() {
                let (px, py) = (dx as f32 + 0.5 - radius, dy as f32 + 0.5 - radius);
                let distance = px.hypot(py);
                let cover = (radius - distance + 0.5).clamp(0., 1.);
                if cover == 0. {
                    continue;
                }
                let turn = (py.atan2(px).to_degrees() + 450.) % 360. / 60.;
                let (n, part) = (turn as usize, turn.fract());
                let paint = if distance < radius * 0.44 {
                    hole
                } else {
                    mix(rgb(STOPS[n]), rgb(STOPS[n + 1]), part)
                };
                canvas.set_draw_color(color(Rgba {
                    a: (cover * 255.) as u8,
                    ..paint
                }));
                canvas.fill_rect(Rect::new(rect.x() + dx as i32, rect.y() + dy as i32, 1, 1))?;
            }
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
