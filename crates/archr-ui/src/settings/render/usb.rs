//! Network › USB adapters (Rede-adaptadores): the adapters on the left,
//! the selected one's driver, firmware and interface and the chips that
//! work in this image on the right.
use super::*;
use crate::usb::Adapters;

/// The list stops where the 244 px panel begins.
const LIST_RIGHT: f32 = 360.;
const PANEL_X: f32 = 376.;
const PANEL_W: f32 = 244.;
const ROW: f32 = 44.;
/// A section heading and the gaps around it, as in the mockup.
const SECTION: f32 = 26.;

impl View<'_, '_, '_> {
    pub(super) fn usb_page(&mut self, state: &Settings) -> Result<(), String> {
        let Some(usb) = &state.adapters else {
            return Ok(());
        };
        if usb.is_empty() {
            self.note(
                (28., 60., LIST_RIGHT - 40., 60.),
                &self.tr("usb.empty"),
                14.,
            )?;
        } else {
            self.usb_list(state, usb)?;
        }
        self.usb_panel(state, usb)
    }
    fn usb_list(&mut self, state: &Settings, usb: &Adapters) -> Result<(), String> {
        let mut top = VIEW_TOP - 2.;
        let positions: Vec<Placement> = (0..usb.len())
            .map(|n| {
                let section = match n {
                    0 if !usb.now.is_empty() => Some("usb.now"),
                    n if n == usb.now.len() => Some("usb.before"),
                    _ => None,
                };
                top += if section.is_some() { SECTION } else { 0. };
                let place = Placement {
                    top,
                    height: ROW,
                    section,
                    searching: None,
                    trail: 0.,
                };
                top += ROW + ROW_GAP;
                place
            })
            .collect();
        self.list(state, &positions, LIST_RIGHT, |view, n, y, _, selected| {
            let adapter = usb.get(n).expect("listed adapter");
            let ink = view.ink(selected);
            let face = selection(selected, Face::Regular, Face::Semibold);
            let tag = view.tr(adapter.verdict.badge());
            let tag_w = view.ui.measure(&tag, Face::Semibold, 11.)? as f32 / view.ui.frame.scale;
            let text_w = LIST_RIGHT - 12. - tag_w - 14. - 10. - 28.;
            view.text_face(
                (28., y + 3., text_w, 20.),
                &adapter.name,
                (15., ink, Align::Left),
                face,
            )?;
            view.text(
                (28., y + 23., text_w, 17.),
                &adapter.line(&view.ui.catalog),
                (
                    12.,
                    selection(selected, view.theme.muted, view.theme.accent_text_secondary),
                    Align::Left,
                ),
            )?;
            let tone = crate::render::usb_tone(view.theme, adapter.verdict, selected);
            view.ui.tag(
                &tag,
                (view.ui.px(LIST_RIGHT - 12.), view.ui.px(y + ROW / 2.)),
                tone,
            )?;
            Ok(())
        })
    }
    fn usb_panel(&mut self, state: &Settings, usb: &Adapters) -> Result<(), String> {
        let theme = self.theme;
        let (x, w) = (PANEL_X, PANEL_W);
        let mut y = 48.;
        if let Some(adapter) = state.usb_adapter() {
            let catalog = &self.ui.catalog;
            let facts = [
                ("usb.driver", adapter.driver(catalog)),
                ("usb.firmware", adapter.firmware(catalog)),
                ("usb.interface", adapter.interface(catalog)),
            ];
            for (key, value) in facts {
                let label = self.tr(key);
                let label_w =
                    self.ui.measure(&label, Face::Regular, 12.)? as f32 / self.ui.frame.scale;
                // A long file name takes another line rather than shrinking away.
                let room = w - label_w - 10.;
                let lines = self.lines(&value, 12., self.ui.px(room) as u32)?;
                let h = 10. + 16. * lines.len() as f32;
                self.text(
                    (x, y, label_w, 26.),
                    &label,
                    (12., theme.muted, Align::Left),
                )?;
                for (n, line) in lines.iter().enumerate() {
                    self.text(
                        (x + label_w + 10., y + 5. + 16. * n as f32, room, 16.),
                        line,
                        (12., theme.text, Align::Right),
                    )?;
                }
                self.ui.divider(
                    self.ui.px(x),
                    self.ui.px(y + h - 1.),
                    self.ui.px(w) as u32,
                    theme.border,
                )?;
                y += h;
            }
            y += 12.;
        }
        if !usb.table.chips.is_empty() {
            let heading = self.tr("usb.chips").to_uppercase();
            for line in self.lines(&heading, 11., self.ui.px(w) as u32)? {
                self.text((x, y, w, 16.), &line, (11., theme.muted, Align::Left))?;
                y += 16.;
            }
            y += 6.;
            for chip in &usb.table.chips {
                self.ui.icon(
                    Glyph::CircleCheck,
                    self.rect((x, y + 4.5, 14., 14.)),
                    theme.success,
                )?;
                let bands = chip.bands(&self.ui.catalog);
                let bands_w =
                    self.ui.measure(&bands, Face::Regular, 12.)? as f32 / self.ui.frame.scale;
                self.text(
                    (x + 22., y, w - 22. - bands_w - 8., 23.),
                    &chip.name,
                    (12., theme.text, Align::Left),
                )?;
                self.text(
                    (x + w - bands_w, y, bands_w, 23.),
                    &bands,
                    (12., theme.muted, Align::Right),
                )?;
                y += 23.;
            }
            y += 8.;
        }
        let note = self.tr("usb.note");
        let lines = self.lines(&note, 11., self.ui.px(w) as u32)?;
        for line in lines {
            self.text((x, y, w, 16.), &line, (11., theme.dim_text, Align::Left))?;
            y += 16.;
        }
        Ok(())
    }
}
