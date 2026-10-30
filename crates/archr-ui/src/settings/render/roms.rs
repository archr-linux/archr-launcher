//! ROMs on the PC (Armazenamento-roms): the rows, the split of the card after
//! the move, the note on how it works, and the list of names to change.
//! Fixed layout, like every settings page; the theme only gives colours,
//! fonts and icons.
use super::*;
use crate::settings::roms::size;

/// "After" heading, the bar of both partitions and their sizes.
const AFTER: f32 = 56.;
const NOTE_SIZE: f32 = 12.5;

impl View<'_, '_, '_> {
    pub(super) fn roms_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let mut positions = placements(Page::Roms, rows);
        let check = state.roms.scan.as_ref().and_then(|s| s.check.clone());
        let now = rows.iter().position(|r| r.id == Id::RomsNow);
        let after = check.as_ref().zip(now).filter(|(c, _)| c.part_mib > 0);
        if let Some((_, at)) = after {
            positions[at].trail = AFTER;
            for place in &mut positions[at + 1..] {
                place.top += AFTER;
            }
        }
        self.list(state, &positions, RIGHT, |view, n, y, place, selected| {
            view.row(state, &rows[n], (y, place.height), RIGHT, selected)?;
            match after {
                Some((check, at)) if at == n => view.roms_after(check, y + place.height + ROW_GAP),
                _ => Ok(()),
            }
        })?;
        let status = &state.storage.roms;
        let note = if status.exfat() {
            ("roms.on_pc", "roms.note_exfat")
        } else if status.ext4() || status.armed() {
            ("roms.how", "roms.note")
        } else {
            return Ok(());
        };
        let top = positions.last().map_or(VIEW_TOP, |p| p.end()) + 9.;
        self.roms_note(top, note)
    }
    /// The card after the move: the system partition and the ROMs one.
    fn roms_after(&mut self, check: &crate::settings::roms::Check, top: f32) -> Result<(), String> {
        self.text(
            (28., top, RIGHT - 40., SECTION_HEIGHT),
            &self.tr("roms.after").to_uppercase(),
            (11., self.theme.muted, Align::Left),
        )?;
        let (x, y, width) = (28., top + SECTION_HEIGHT + 4., RIGHT - 44.);
        let total = (check.storage_mib + check.part_mib).max(1) as f32;
        let gap = width * 0.03;
        let system = ((width - gap) * check.storage_mib as f32 / total).round();
        let roms = x + system + gap;
        // Square inner ends, as the mockup clips one rounded track.
        self.capsule((x, y, width, 10.), self.theme.dim_text)?;
        let back = self.theme.background;
        self.ui.canvas.set_draw_color(crate::text::color(back));
        self.ui
            .canvas
            .fill_rect(self.rect((x + system, y, gap, 10.)))?;
        self.capsule((roms, y, x + width - roms, 10.), self.theme.highlight_bg)?;
        let accent = self.theme.highlight_bg;
        self.ui.canvas.set_draw_color(crate::text::color(accent));
        self.ui.canvas.fill_rect(self.rect((roms, y, 5., 10.)))?;
        let labels = y + 12.;
        for (key, mib, align) in [
            ("roms.after_system", check.storage_mib, Align::Left),
            ("roms.after_roms", check.part_mib, Align::Right),
        ] {
            let text = self.tr(&format!("{key}|size={}", size(mib)));
            self.text(
                (x, labels, width, 18.),
                &text,
                (12., self.theme.muted, align),
            )?;
        }
        Ok(())
    }
    /// "How it works": a bold title and the explanation, shrunk to fit
    /// above the footer instead of being cut.
    fn roms_note(&mut self, top: f32, (title, key): (&str, &str)) -> Result<(), String> {
        let (x, width) = (20., RIGHT - 24.);
        let height = VIEW_BOTTOM - top;
        if height < 48. {
            return Ok(());
        }
        // Smaller until it fits; the theme's smallest size is the floor.
        let text = self.tr(key);
        let inner = self.ui.px(width - 24.) as u32;
        let floor = self.ui.frame.min_size().max(11.);
        let mut size = NOTE_SIZE;
        let body = loop {
            let body = self.lines(&text, size, inner)?;
            if 40. + body.len() as f32 * (size + 6.) <= height || size <= floor {
                break body;
            }
            size -= 0.5;
        };
        let (step, face) = (size + 6., Face::Regular);
        let used = 18. + 22. + body.len() as f32 * step;
        self.panel((x, top, width, used.min(height)), false)?;
        self.text_face(
            (x + 12., top + 9., width - 24., 20.),
            &self.tr(title),
            (NOTE_SIZE, self.theme.text, Align::Left),
            Face::Semibold,
        )?;
        for (n, line) in body.iter().enumerate() {
            self.ui.label_aligned(
                line,
                (face, size, self.theme.muted),
                self.rect((x + 12., top + 31. + n as f32 * step, width - 24., step)),
                Align::Left,
            )?;
        }
        Ok(())
    }
    /// Each name exFAT refuses (or the log says was changed) and the new one.
    pub(super) fn roms_list(&mut self, state: &Settings) -> Result<(), String> {
        let names = state
            .roms
            .scan
            .as_ref()
            .map(|s| s.names.clone())
            .unwrap_or_default();
        if names.is_empty() {
            return self.text(
                (40., VIEW_TOP + 20., 560., 60.),
                &self.tr("roms.list_empty"),
                (15., self.theme.muted, Align::Center),
            );
        }
        let positions: Vec<Placement> = (0..names.len())
            .map(|n| Placement {
                top: VIEW_TOP + n as f32 * (40. + ROW_GAP),
                height: 40.,
                section: None,
                searching: None,
                trail: 0.,
            })
            .collect();
        let theme = self.theme;
        let arrow = self.tr("roms.becomes");
        self.list(state, &positions, RIGHT, |view, n, y, _, selected| {
            let (path, name) = &names[n];
            let ink = view.ink(selected);
            let quiet = selection(selected, theme.muted, theme.accent_text_secondary);
            // File names are game names: the label API may shorten them.
            view.ui.label_aligned(
                path,
                (Face::Mono, 13., ink),
                view.rect((30., y + 3., RIGHT - 46., 18.)),
                Align::Left,
            )?;
            view.ui.label_aligned(
                &format!("{arrow} {name}"),
                (Face::Mono, 12., quiet),
                view.rect((30., y + 21., RIGHT - 46., 16.)),
                Align::Left,
            )
        })
    }
}
