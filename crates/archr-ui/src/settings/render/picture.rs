//! Sound and display › Picture (Tela-imagem): a preview strip that shows
//! the curve on screen, the rows beside a card that explains them. The card
//! (`aside`) is shared with the save-sync pages.
use super::*;

/// The card on the right: its left edge and width; the rows stop 16 px
/// before it.
pub(super) const ASIDE: (f32, f32) = (RIGHT - 200., 200.);
pub(super) const ROWS_RIGHT: f32 = ASIDE.0 - 16.;
const PREVIEW: f32 = 64.;
/// Rows start under the preview's heading and strip.
pub(super) const ROWS_TOP: f32 = VIEW_TOP + SECTION_HEIGHT + PREVIEW + 4. + SECTION_HEIGHT + 2.;
/// The bottom half: one colour each for red, orange, yellow, green, blue,
/// purple and white, so a warm temperature and the gamma read at a glance.
const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 0xFF }
}
const SWATCHES: [Rgba; 7] = [
    rgb(0xE5, 0x48, 0x4D),
    rgb(0xF5, 0xA5, 0x24),
    rgb(0xF2, 0xDA, 0x3D),
    rgb(0x3F, 0xB9, 0x50),
    rgb(0x1E, 0x88, 0xFF),
    rgb(0x8E, 0x4E, 0xC6),
    rgb(0xF2, 0xF2, 0xF2),
];

impl View<'_, '_, '_> {
    pub(super) fn picture_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        self.text(
            (28., VIEW_TOP, ROWS_RIGHT - 40., SECTION_HEIGHT),
            &self.tr("image.preview").to_uppercase(),
            (11., self.theme.muted, Align::Left),
        )?;
        let (x, y, w) = (30., VIEW_TOP + SECTION_HEIGHT, ROWS_RIGHT - 44.);
        // The frame first: the cells are drawn over it, a pixel inside.
        self.ui.rounded(
            self.rect((x - 1., y - 1., w + 2., PREVIEW + 2.)),
            self.ui.px(4.) as f32,
            self.theme.border,
            None,
        )?;
        // Sixteen greys, black to white, over the swatches.
        let step = w / 16.;
        for n in 0..16u8 {
            let v = n * 17;
            let cell = self.rect((x + step * f32::from(n), y, step + 0.5, PREVIEW / 2.));
            self.fill(cell, rgb(v, v, v))?;
        }
        let step = w / SWATCHES.len() as f32;
        for (n, colour) in SWATCHES.iter().enumerate() {
            let cell = self.rect((
                x + step * n as f32,
                y + PREVIEW / 2.,
                step + 0.5,
                PREVIEW / 2.,
            ));
            self.fill(cell, *colour)?;
        }
        self.rows_at(state, rows, &placements(Page::Image, rows), ROWS_RIGHT)?;
        self.aside(
            &[
                ("image.aside_live", "image.aside_live_detail"),
                ("image.aside_all", "image.aside_all_detail"),
            ],
            "image.aside_tip",
        )
    }
    fn fill(&mut self, rect: Rect, colour: Rgba) -> Result<(), String> {
        self.ui.canvas.set_draw_color(crate::text::color(colour));
        self.ui.canvas.fill_rect(rect).map_err(|e| e.to_string())
    }
    /// A card of headed paragraphs, and a small line at its foot.
    pub(super) fn aside(&mut self, blocks: &[(&str, &str)], foot: &str) -> Result<(), String> {
        let (x, w) = ASIDE;
        let bottom = self.view_bottom() - 8.;
        let theme = self.theme;
        self.ui.rounded(
            self.rect((x, VIEW_TOP, w, bottom - VIEW_TOP)),
            self.ui.px(14.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        let (left, inner) = (x + 14., w - 28.);
        let mut y = VIEW_TOP + 12.;
        for (title, detail) in blocks {
            self.text_face(
                (left, y, inner, 20.),
                &self.tr(title),
                (14., theme.text, Align::Left),
                Face::Semibold,
            )?;
            y += 22.;
            let text = self.tr(detail);
            let width = self.rect((left, y, inner, 1.)).width();
            let lines = self.lines(&text, 12., width)?.len() as f32;
            self.note((left, y, inner, lines * 17.), &text, 12.)?;
            y += lines * 17. + 10.;
        }
        let text = self.tr(foot);
        let width = self.rect((left, 0., inner, 1.)).width();
        let lines = self.lines(&text, 11., width)?.len() as f32;
        self.note(
            (left, bottom - 12. - lines * 15., inner, lines * 15.),
            &text,
            11.,
        )
    }
}
