//! The list of a long value row: a panel beside the row over the dimmed
//! page (Lista-media), or a page of groups and choices (Lista-longa).
//! Fixed layout; the theme gives colours, fonts and icons, and with
//! `[settings] groups = "inset"` the choices in a rounded group with rules.
use super::*;
use crate::settings::picker::Picker;

/// One choice in either form, and the panel's heading.
const ENTRY: f32 = 30.;
const HEADER: f32 = 30.;
const PANEL_WIDTH: f32 = 300.;
/// The page's column of groups, and the gap before the choices.
const GROUPS: f32 = 176.;
const GAP: f32 = 12.;

/// A line of the list: a group's heading, never selected, or a choice.
#[derive(Clone, Copy, PartialEq)]
enum Line {
    Heading(usize),
    Choice(usize),
}
/// The choices in order, each group opened by its heading.
fn lines(picker: &Picker) -> Vec<Line> {
    let mut lines = Vec::new();
    for n in 0..picker.choices.len() {
        if let Some(g) = picker.groups.iter().position(|(_, start)| *start == n) {
            lines.push(Line::Heading(g));
        }
        lines.push(Line::Choice(n));
    }
    lines
}

/// The first of `shown` rows out of `len` that keeps `at` near the middle.
fn window(at: usize, shown: usize, len: usize) -> usize {
    at.saturating_sub(shown / 2).min(len.saturating_sub(shown))
}

impl View<'_, '_, '_> {
    /// `text` on one line, cut with an ellipsis where it does not fit.
    fn line(
        &mut self,
        bounds: Bounds,
        text: &str,
        (size, ink, align): (f32, Rgba, Align),
        face: Face,
    ) -> Result<(), String> {
        let rect = self.rect(bounds);
        let text = if self.ui.measure(text, face, size)? > rect.width() {
            self.ellipsized(text, size, face, rect.width())?
        } else {
            text.to_owned()
        };
        self.ui.label_aligned(&text, (face, size, ink), rect, align)
    }
    /// `shown` lines around the cursor from `top`, between `left` and
    /// `right`, and a thin bar beyond `right` when the list scrolls.
    fn picker_entries(
        &mut self,
        state: &Settings,
        picker: &Picker,
        shown: usize,
        (left, top, right): (f32, f32, f32),
    ) -> Result<(), String> {
        let current = state.chosen(picker.row.id);
        let lines = lines(picker);
        let at = lines.iter().position(|l| *l == Line::Choice(picker.cursor));
        let first = window(at.unwrap_or(0), shown, lines.len());
        for (k, line) in lines.iter().enumerate().skip(first).take(shown) {
            let y = top + (k - first) as f32 * ENTRY;
            let n = match *line {
                Line::Choice(n) => n,
                Line::Heading(g) => {
                    // Like a section label: small capitals just above its rows.
                    let heading = self.tr(&picker.groups[g].0).to_uppercase();
                    let muted = self.theme.muted;
                    self.line(
                        (left + 32., y + 10., right - left - 44., ENTRY - 12.),
                        &heading,
                        (11., muted, Align::Left),
                        Face::Regular,
                    )?;
                    continue;
                }
            };
            let code = &picker.choices[n];
            let selected = n == picker.cursor;
            // A rule between two choices of a group.
            if self.look().inset && k > first && matches!(lines[k - 1], Line::Choice(_)) {
                let ink = self.group_rule();
                self.ui.divider(
                    self.ui.px(left + 14.),
                    self.ui.px(y - 1.),
                    self.ui.px(right - left - 28.) as u32,
                    ink,
                )?;
            }
            self.row_panel((left, y, right - left, ENTRY - 2.), selected)?;
            let ink = self.ink(selected);
            let face = selection(selected, Face::Regular, Face::Semibold);
            if code == current {
                let tick = self.rect((left + 10., y + (ENTRY - 2. - 14.) / 2., 14., 14.));
                self.ui.icon(Glyph::Check, tick, ink)?;
            }
            let (text, note) = state.picker_entry(code, &self.ui.catalog);
            let mut end = right - 12.;
            if let Some(note) = note {
                let width = self.span(&note, Face::Regular, 12.)?.min(120.);
                let muted = selection(selected, self.theme.muted, self.theme.accent_text_secondary);
                self.line(
                    (end - width, y, width, ENTRY - 2.),
                    &note,
                    (12., muted, Align::Right),
                    Face::Regular,
                )?;
                end -= width + 12.;
            }
            let x = left + 32.;
            self.line(
                (x, y, (end - x).max(20.), ENTRY - 2.),
                &text,
                (14., ink, Align::Left),
                face,
            )?;
        }
        let len = lines.len() as f32;
        if shown as f32 >= len {
            return Ok(());
        }
        // The track is the whole list, the thumb what is in view.
        let track = shown as f32 * ENTRY - 2.;
        let bar = |from: f32, to: f32| (right + 4., top + track * from, 3., track * (to - from));
        let thumb = bar(first as f32 / len, (first + shown) as f32 / len);
        for (bounds, fill) in [
            (bar(0., 1.), self.theme.border),
            (thumb, self.theme.highlight_bg),
        ] {
            self.ui
                .rounded(self.rect(bounds), self.ui.px(1.5) as f32, fill, None)?;
        }
        Ok(())
    }
    /// Lista-media: the page dimmed, the row in the accent above it and the
    /// choices in a panel below the row, or above where there is no room.
    pub(super) fn picker_panel(
        &mut self,
        state: &Settings,
        rows: &[Row],
        picker: &Picker,
    ) -> Result<(), String> {
        let positions = placements(state.page(), rows);
        let offset = self.ui.settings_scroll.tween.value(self.ui.frame.now);
        let place = positions.get(state.cursor).copied();
        let (top, bottom) = (VIEW_TOP, self.view_bottom());
        let (row_top, row_bottom) =
            place.map_or((top, top), |p| (p.top - offset, p.bottom() - offset));
        if let (Some(place), Some(row)) = (place, rows.get(state.cursor)) {
            self.row_panel((16., row_top, RIGHT - 16., place.height), true)?;
            self.row(state, row, (row_top, place.height), RIGHT, true)?;
        }
        // Below or above the row, whichever holds more; over it only when
        // neither side holds five choices.
        let len = picker.choices.len();
        let room =
            |space: f32| (((space - HEADER - 10.) / ENTRY).floor().max(0.) as usize).min(len);
        let height = |shown: usize| HEADER + shown as f32 * ENTRY + 6.;
        let (below, above) = (bottom - row_bottom, row_top - top);
        let side = room(below.max(above));
        let (shown, y) = if side >= len.min(5) && below >= above {
            (side, row_bottom + 4.)
        } else if side >= len.min(5) {
            (side, row_top - 4. - height(side))
        } else {
            let shown = room(bottom - top + 4.);
            (shown, (row_top - HEADER).clamp(top, bottom - height(shown)))
        };
        let height = height(shown);
        let x = RIGHT - PANEL_WIDTH;
        let (radius, edge) = if self.look().inset {
            (14., self.group_rule())
        } else {
            (10., self.theme.border)
        };
        self.ui.rounded(
            self.rect((x, y, PANEL_WIDTH, height)),
            self.ui.px(radius) as f32,
            self.theme.surface_elevated,
            Some(edge),
        )?;
        let mut title = self.tr(&state.label(&picker.row));
        if matches!(picker.row.id, Id::Emulator | Id::Core) {
            title = format!("{title} · {}", state.games.fullname());
        }
        self.line(
            (x + 14., y + 4., PANEL_WIDTH - 28., HEADER - 4.),
            &title.to_uppercase(),
            (11., self.theme.muted, Align::Left),
            Face::Regular,
        )?;
        self.picker_entries(state, picker, shown, (x + 4., y + HEADER, RIGHT - 10.))
    }
    /// Lista-longa: the groups on the left, every choice on the right under
    /// its group's heading with a thin scroll bar, the cursor's group marked.
    pub(super) fn picker_page(&mut self, state: &Settings) -> Result<(), String> {
        let Some(picker) = &state.picker else {
            return Ok(());
        };
        let (top, bottom) = (VIEW_TOP, self.view_bottom());
        let shown_rows = ((bottom - top) / ENTRY).floor() as usize;
        let mut left = 16.;
        if !picker.groups.is_empty() {
            let current = picker.group();
            let count = picker.groups.len();
            let shown = count.min(shown_rows);
            let first = window(current, shown, count);
            if self.look().inset {
                self.group((left, top - 2., GROUPS, shown as f32 * ENTRY + 2.), &[])?;
            }
            for (n, (heading, _)) in picker.groups.iter().enumerate().skip(first).take(shown) {
                let y = top + (n - first) as f32 * ENTRY;
                let here = n == current;
                if here {
                    self.panel((left, y, GROUPS, ENTRY - 2.), false)?;
                }
                let (ink, face) = if here {
                    (self.theme.text, Face::Semibold)
                } else {
                    (self.theme.muted, Face::Regular)
                };
                self.line(
                    (left + 12., y, GROUPS - 24., ENTRY - 2.),
                    &self.tr(heading),
                    (14., ink, Align::Left),
                    face,
                )?;
            }
            left += GROUPS + GAP;
        }
        let shown = (picker.choices.len() + picker.groups.len()).min(shown_rows);
        if self.look().inset {
            let w = RIGHT - 10. - left;
            self.group((left, top - 2., w, shown as f32 * ENTRY + 2.), &[])?;
        }
        self.picker_entries(state, picker, shown, (left, top, RIGHT - 10.))
    }
    /// The footer's "<group> · 12 of 30", or "12 of 30" for a list in one piece.
    pub(super) fn picker_status(&self, picker: &Picker) -> String {
        let group = picker.group();
        let (start, end) = picker.span(group);
        let n = (picker.cursor - start + 1).to_string();
        let total = (end - start).to_string();
        match picker.groups.get(group) {
            Some((heading, _)) => self.ui.catalog.format(
                "picker.position",
                &[("group", &self.tr(heading)), ("n", &n), ("total", &total)],
            ),
            None => self
                .ui
                .catalog
                .format("picker.count", &[("n", &n), ("total", &total)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_group_opens_with_a_heading_that_is_never_a_choice() {
        let picker = Picker {
            row: Page::Region.rows()[1],
            choices: ["a", "b", "c"].map(String::from).to_vec(),
            groups: vec![("x".into(), 0), ("y".into(), 2)],
            cursor: 2,
        };
        use Line::{Choice, Heading};
        let expected = [Heading(0), Choice(0), Choice(1), Heading(1), Choice(2)];
        assert!(lines(&picker) == expected);
        let whole = Picker {
            groups: Vec::new(),
            ..picker
        };
        assert!(lines(&whole) == [Choice(0), Choice(1), Choice(2)]);
    }
}
