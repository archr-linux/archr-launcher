//! Import from RetroArch (Config-importar-ra): the rows, the note on where to
//! copy, and the list of what applies and what is left out. Fixed layout,
//! like every settings page; the theme only gives colours, fonts and icons.
use super::*;
use crate::settings::raimport::FOLDER;

const NOTE_SIZE: f32 = 13.;
const NOTE_LINE: f32 = 19.;

impl View<'_, '_, '_> {
    pub(super) fn raimport_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let positions = placements(Page::RaImport, rows);
        self.rows_at(state, rows, &positions, RIGHT)?;
        let top = positions.last().map_or(VIEW_TOP, |p| p.bottom()) + 9.;
        self.raimport_note(top)
    }
    /// "Copy to roms/retroarch-import/ …", the folder in the mono face, words
    /// wrapped across the panel.
    fn raimport_note(&mut self, top: f32) -> Result<(), String> {
        let (x, width) = (24., RIGHT - 32.);
        let inner = width - 24.;
        let mut words: Vec<(String, Face)> = Vec::new();
        for (key, face) in [
            ("raimport.copy_to", Face::Regular),
            ("", Face::Mono),
            ("raimport.copy_rest", Face::Regular),
        ] {
            if key.is_empty() {
                words.push((FOLDER.into(), face));
            } else {
                words.extend(
                    self.tr(key)
                        .split_whitespace()
                        .map(|w| (w.to_owned(), face)),
                );
            }
        }
        let space = self.ui.measure(" ", Face::Regular, NOTE_SIZE)? as f32 / self.ui.frame.scale;
        let mut placed: Vec<(f32, usize, String, Face, f32)> = Vec::new();
        let (mut at, mut line) = (0., 0);
        for (word, face) in words {
            let wide = self.ui.measure(&word, face, NOTE_SIZE)? as f32 / self.ui.frame.scale;
            if at > 0. && at + wide > inner {
                (at, line) = (0., line + 1);
            }
            placed.push((at, line, word, face, wide));
            at += wide + space;
        }
        let height = (line + 1) as f32 * NOTE_LINE + 16.;
        self.panel((x, top, width, height), false)?;
        for (at, line, word, face, wide) in placed {
            let ink = if face == Face::Mono {
                self.theme.text
            } else {
                self.theme.muted
            };
            let y = top + 8. + line as f32 * NOTE_LINE;
            self.ui.label_aligned(
                &word,
                (face, NOTE_SIZE, ink),
                self.rect((x + 12. + at, y, wide + 2., NOTE_LINE)),
                Align::Left,
            )?;
        }
        Ok(())
    }
    /// Each item that applies, then each one left out with the reason.
    pub(super) fn raimport_list(&mut self, state: &Settings) -> Result<(), String> {
        let found = state.raimport.found.clone().unwrap_or_default();
        let mut top = VIEW_TOP;
        let mut positions = Vec::new();
        let mut items: Vec<(String, String, bool)> = Vec::new();
        for (n, (name, detail)) in found.apply.iter().enumerate() {
            if n == 0 {
                top += SECTION_HEIGHT;
            }
            positions.push(Placement {
                top,
                height: 30.,
                section: (n == 0).then_some("raimport.will_apply"),
                searching: None,
                trail: 0.,
            });
            top += 30. + ROW_GAP;
            items.push((name.clone(), self.tr(detail), false));
        }
        for (n, (name, why)) in found.ignored.iter().enumerate() {
            if n == 0 {
                top += SECTION_HEIGHT;
            }
            positions.push(Placement {
                top,
                height: 30.,
                section: (n == 0).then_some("raimport.ignored"),
                searching: None,
                trail: 0.,
            });
            top += 30. + ROW_GAP;
            items.push((name.clone(), self.tr(why), true));
        }
        if items.is_empty() {
            return self.text(
                (40., VIEW_TOP + 20., 560., 60.),
                &self.tr("raimport.empty"),
                (15., self.theme.muted, Align::Center),
            );
        }
        let theme = self.theme;
        self.list(state, &positions, RIGHT, |view, n, y, place, selected| {
            let (name, detail, ignored) = &items[n];
            let ink = view.ink(selected);
            let quiet = selection(selected, theme.muted, theme.accent_text_secondary);
            let half = (RIGHT - 46.) / 2.;
            view.text_face(
                (30., y, half + 40., place.height),
                name,
                (13., ink, Align::Left),
                Face::Mono,
            )?;
            let face = if *ignored { Face::Regular } else { Face::Mono };
            view.text_face(
                (70. + half, y, half - 40., place.height),
                detail,
                (12., quiet, Align::Right),
                face,
            )
        })
    }
}
