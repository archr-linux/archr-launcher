//! Diagnostics (Sistema-diagnostico): the rows, then the note on what the
//! report holds. Fixed layout, like every settings page; the theme only
//! gives colours, fonts and icons.
use super::*;

const NOTE_SIZE: f32 = 12.5;

impl View<'_, '_, '_> {
    pub(super) fn diag_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let positions = placements(Page::Diagnostics, rows);
        self.rows_at(state, rows, &positions, RIGHT)?;
        let top = positions.last().map_or(VIEW_TOP, |p| p.end()) + 6.;
        self.rich_note((20., top, RIGHT - 28.), &self.tr("diag.note"))
    }
    /// A note panel whose text marks `*emphasis*` (text colour) and
    /// `` `paths` `` (monospace); smaller until it fits above the footer.
    pub(super) fn rich_note(
        &mut self,
        (x, top, width): (f32, f32, f32),
        text: &str,
    ) -> Result<(), String> {
        let room = self.view_bottom() - top;
        let floor = self.ui.frame.min_size().max(11.);
        let mut size = NOTE_SIZE;
        let lines = loop {
            let lines = self.runs(text, size, width - 24.)?;
            if 18. + lines.len() as f32 * (size * 1.45) <= room || size <= floor {
                break lines;
            }
            size -= 0.5;
        };
        let step = (size * 1.45).round();
        let height = 18. + lines.len() as f32 * step;
        self.panel((x, top, width, height.min(room)), false)?;
        for (n, line) in lines.iter().enumerate() {
            let y = top + 9. + n as f32 * step;
            for (left, word, style) in line {
                let ink = match style {
                    Style::Plain => self.theme.muted,
                    Style::Strong | Style::Code => self.theme.text,
                };
                let face = style.face();
                self.ui.label_aligned(
                    word,
                    (face, size, ink),
                    self.rect((x + 12. + left, y, width - 24. - left, step)),
                    Align::Left,
                )?;
            }
        }
        Ok(())
    }
    /// Words laid out greedily: each line holds (x, text, style).
    fn runs(&mut self, text: &str, size: f32, width: f32) -> Result<Vec<Line>, String> {
        // Each word is one or two pieces: a mark may close before
        // punctuation ("entram.*" or "`roms/`,"), which stays plain.
        let mut words: Vec<Vec<(String, Style)>> = Vec::new();
        let mut style = Style::Plain;
        for word in text.split(' ').filter(|w| !w.is_empty()) {
            let mut word = word.to_owned();
            for (mark, opened) in [('*', Style::Strong), ('`', Style::Code)] {
                if let Some(rest) = word.strip_prefix(mark) {
                    word = rest.to_owned();
                    style = opened;
                }
            }
            let close = word.find(['*', '`']);
            let pieces = match close {
                Some(at) => {
                    let after = word[at + 1..].to_owned();
                    word.truncate(at);
                    let pieces = vec![(word, style), (after, Style::Plain)];
                    style = Style::Plain;
                    pieces
                }
                None => vec![(word, style)],
            };
            words.push(pieces.into_iter().filter(|(t, _)| !t.is_empty()).collect());
        }
        let space = self.ui.measure(" ", Face::Regular, size)? as f32 / self.ui.frame.scale;
        let mut lines = vec![Vec::new()];
        let mut x = 0.;
        for pieces in words {
            let mut widths = Vec::new();
            for (text, style) in &pieces {
                widths
                    .push(self.ui.measure(text, style.face(), size)? as f32 / self.ui.frame.scale);
            }
            if x > 0. && x + widths.iter().sum::<f32>() > width {
                lines.push(Vec::new());
                x = 0.;
            }
            for ((text, style), w) in pieces.into_iter().zip(widths) {
                lines.last_mut().expect("a line").push((x, text, style));
                x += w;
            }
            x += space;
        }
        Ok(lines)
    }
}

/// One line of a note: each word's x, text and style.
type Line = Vec<(f32, String, Style)>;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    Plain,
    Strong,
    Code,
}
impl Style {
    fn face(self) -> Face {
        if self == Self::Code {
            Face::Mono
        } else {
            Face::Regular
        }
    }
}
