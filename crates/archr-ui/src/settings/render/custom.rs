//! Custom accent page (Aparencia / Cor personalizada): a sample, three
//! sliders on gradient tracks, the hex code and the live preview.
use super::*;
use crate::appearance::{hex, hsv_to_rgb, mix, rgb, Appearance};

const LEFT: f32 = 16.;
const LIST_RIGHT: f32 = 372.;
const PREVIEW_X: f32 = 390.;
const PREVIEW_W: f32 = 230.;
const SLIDER: f32 = 56.;
const HEX_ROW: f32 = 40.;
/// Top of the first slider; a 6 px gap separates the rows.
const FIRST: f32 = 112.;

impl View<'_, '_, '_> {
    pub(super) fn custom_page(&mut self, state: &Settings) -> Result<(), String> {
        let custom = state.custom;
        let color = rgb(custom.rgb());
        self.sample(color)?;
        let labels = [
            "appearance.hue",
            "appearance.saturation",
            "appearance.brightness",
        ];
        for (n, label) in labels.iter().enumerate() {
            let y = FIRST + n as f32 * (SLIDER + 6.);
            let selected = state.cursor == n;
            self.row_panel((LEFT, y, LIST_RIGHT - LEFT, SLIDER), selected)?;
            let face = selection(selected, Face::Regular, Face::Semibold);
            let level = custom.level(n);
            let unit = if n == 0 { "°" } else { "%" };
            self.text_face(
                (30., y + 9., 200., 20.),
                &self.tr(label),
                (15., self.ink(selected), Align::Left),
                face,
            )?;
            self.text_face(
                (230., y + 9., 128., 20.),
                &format!("{level}{unit}"),
                (
                    13.,
                    selection(
                        selected,
                        self.theme.dim_text,
                        self.theme.accent_text_secondary,
                    ),
                    Align::Right,
                ),
                Face::Mono,
            )?;
            let (hue, sat, val) = (custom.hue, custom.sat, custom.val);
            let max = [360., 100., 100.][n];
            let at = move |t: f32| match n {
                0 => rgb(hsv_to_rgb((t * 360.) as u16, 100, 100)),
                1 => mix(
                    rgb(hsv_to_rgb(hue, 0, val)),
                    rgb(hsv_to_rgb(hue, 100, val)),
                    t,
                ),
                _ => mix(rgb(0), rgb(hsv_to_rgb(hue, sat, 100)), t),
            };
            self.track((30., y + 37., 328., 10.), f32::from(level) / max, at)?;
        }
        let y = FIRST + 3. * (SLIDER + 6.);
        let selected = state.cursor == 3;
        self.row_panel((LEFT, y, LIST_RIGHT - LEFT, HEX_ROW), selected)?;
        self.text_face(
            (30., y, 200., HEX_ROW),
            &self.tr("appearance.hex"),
            (15., self.ink(selected), Align::Left),
            selection(selected, Face::Regular, Face::Semibold),
        )?;
        self.text_face(
            (230., y, 108., HEX_ROW),
            &hex(custom.rgb()),
            (
                14.,
                selection(
                    selected,
                    self.theme.dim_text,
                    self.theme.accent_text_secondary,
                ),
                Align::Right,
            ),
            Face::Mono,
        )?;
        self.ui.icon(
            Glyph::ChevronRight,
            self.rect((344., y + (HEX_ROW - 14.) / 2., 14., 14.)),
            self.ink(selected),
        )?;
        let draft = Appearance {
            accent: hex(custom.rgb()),
            ..state.appearance()
        };
        self.draft_preview(state, &draft)?;
        self.note(
            (PREVIEW_X, 292., PREVIEW_W, 60.),
            &self.tr("appearance.contrast_note"),
            12.,
        )
    }
    /// The colour as a large dot with a ring, and what it is.
    fn sample(&mut self, color: Rgba) -> Result<(), String> {
        for (size, fill) in [
            (54., self.theme.text),
            (50., self.theme.background),
            (44., color),
        ] {
            let bounds = (28. + 22. - size / 2., 78. - size / 2., size, size);
            self.ui
                .rounded(self.rect(bounds), self.ui.px(size / 2.) as f32, fill, None)?;
        }
        self.text_face(
            (86., 57., 274., 20.),
            &self.tr("appearance.custom"),
            (15., self.theme.text, Align::Left),
            Face::Semibold,
        )?;
        self.note(
            (86., 79., 274., 17.),
            &self.tr("appearance.custom_note"),
            12.,
        )
    }
    /// A gradient track with a round knob at `at` (0 to 1). The ends are discs
    /// in the end colours; the middle is drawn in narrow vertical bands.
    fn track(
        &mut self,
        (x, y, w, h): Bounds,
        at: f32,
        color_at: impl Fn(f32) -> Rgba,
    ) -> Result<(), String> {
        let radius = self.ui.px(h / 2.) as f32;
        for (left, t) in [(x, 0.), (x + w - h, 1.)] {
            self.ui
                .rounded(self.rect((left, y, h, h)), radius, color_at(t), None)?;
        }
        let mut band = x + h / 2.;
        while band < x + w - h / 2. {
            let rect = self.rect((band, y, 2.5, h));
            self.ui
                .canvas
                .set_draw_color(color(color_at((band - x) / w)));
            self.ui.canvas.fill_rect(rect)?;
            band += 2.;
        }
        let knob = x + w * at;
        let shadow = Rgba { a: 110, ..rgb(0) };
        for (size, fill) in [(21., shadow), (18., rgb(0xFFFFFF))] {
            let bounds = (knob - size / 2., y + h / 2. - size / 2., size, size);
            self.ui
                .rounded(self.rect(bounds), self.ui.px(size / 2.) as f32, fill, None)?;
        }
        Ok(())
    }
}
