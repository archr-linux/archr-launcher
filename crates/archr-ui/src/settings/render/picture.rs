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
        let blocks: Vec<(String, String)> = blocks
            .iter()
            .map(|(title, detail)| (self.tr(title), self.tr(detail)))
            .collect();
        let foot = self.tr(foot);
        self.aside_card(&blocks, Some(&foot), ASIDE, false)
    }
    /// The card from texts already in the user's language, at `(x, w)`:
    /// down to the footer with the foot line at its bottom, or `fit`ted to
    /// its texts with the foot line right under them (a module page in a
    /// theme of grouped rows, Ellegance-Modulo-tuner-tensao).
    pub(super) fn aside_card(
        &mut self,
        blocks: &[(String, String)],
        foot: Option<&str>,
        (x, w): (f32, f32),
        fit: bool,
    ) -> Result<(), String> {
        let theme = self.theme;
        let (left, inner) = (x + 14., w - 28.);
        let width = self.rect((left, 0., inner, 1.)).width();
        // A fitted card measures its texts as the small screens draw them,
        // at the screen-pixel floor; the other keeps its fixed lines.
        let floor = if fit { self.ui.frame.min_size() } else { 0. };
        let (title_size, gap): (f32, f32) = if fit { (13., 6.) } else { (14., 10.) };
        let head = title_size.max(floor) + 6.;
        let (text_size, foot_size) = (12_f32.max(floor), 11_f32.max(floor));
        let (step, foot_step) = if fit {
            (
                self.pitch(text_size, text_size + 5.),
                self.pitch(foot_size, foot_size + 4.),
            )
        } else {
            (17., 15.)
        };
        // Every paragraph's lines first: a fitted card is as tall as they are.
        let mut texts = Vec::new();
        for (title, detail) in blocks {
            let lines = self.lines(detail, text_size, width)?.len() as f32;
            texts.push((title, detail, lines));
        }
        let foot_lines = match foot {
            Some(foot) => self.lines(foot, foot_size, width)?.len() as f32,
            None => 0.,
        };
        let content: f32 = texts
            .iter()
            .map(|(title, _, lines)| {
                let head = if title.is_empty() { 0. } else { head };
                head + lines * step + gap
            })
            .sum();
        let bottom = if fit {
            let foot = if foot_lines > 0. {
                2. + foot_lines * foot_step
            } else {
                -gap
            };
            (VIEW_TOP + 12. + content + foot + 12.).min(self.view_bottom() - 8.)
        } else {
            self.view_bottom() - 8.
        };
        if fit {
            self.group((x, VIEW_TOP, w, bottom - VIEW_TOP), &[])?;
        } else {
            self.ui.rounded(
                self.rect((x, VIEW_TOP, w, bottom - VIEW_TOP)),
                self.ui.px(14.) as f32,
                theme.surface,
                Some(theme.border),
            )?;
        }
        let mut y = VIEW_TOP + 12.;
        for (title, detail, lines) in texts {
            if !title.is_empty() {
                self.text_face(
                    (left, y, inner, head),
                    title,
                    (title_size, theme.text, Align::Left),
                    Face::Semibold,
                )?;
                y += head + if fit { 0. } else { 2. };
            }
            self.note((left, y, inner, lines * step), detail, 12.)?;
            y += lines * step + gap;
        }
        let Some(foot) = foot else {
            return Ok(());
        };
        let top = if fit {
            y + 2.
        } else {
            bottom - 12. - foot_lines * foot_step
        };
        let ink = if fit { theme.dim_text } else { theme.muted };
        // On a small screen a fitted card stops at the footer: the foot line
        // keeps the lines left there, the last one shortened.
        let room = if fit {
            (bottom - 10. - top).min(foot_lines * foot_step)
        } else {
            foot_lines * foot_step
        };
        if room < foot_step {
            return Ok(());
        }
        self.text((left, top, inner, room), foot, (11., ink, Align::Left))
    }
}
