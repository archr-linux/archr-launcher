//! Appearance › Start shortcuts (Hub-atalhos-editar): the list with a
//! switch each, the row's preview beside it and the add chooser over it.
use super::*;
use crate::shortcuts::{Builtin, Shortcut, MAX};

/// Right edge of the list; the preview panel takes the rest.
const LIST_RIGHT: f32 = 408.;
const PANEL: (f32, f32) = (424., 196.);
const TOP: f32 = VIEW_TOP + 6.;
const ROW: f32 = 30.;
const PITCH: f32 = ROW + ROW_GAP;

impl View<'_, '_, '_> {
    /// A shortcut's name and icon.
    fn shortcut_face(
        &mut self,
        shortcut: Shortcut,
        icon: Rect,
        ink: Rgba,
    ) -> Result<String, String> {
        match shortcut {
            Shortcut::Builtin(builtin) => {
                self.ui.icon(builtin.glyph(), icon, ink)?;
                Ok(self.tr(builtin.label()))
            }
            Shortcut::Module(_, entry) => {
                self.ui
                    .module_icon((entry.icon.as_deref(), ""), icon, ink)?;
                Ok(entry.label.clone())
            }
        }
    }
    pub(super) fn shortcuts_page(&mut self, state: &Settings) -> Result<(), String> {
        let items = state.shortcut_items();
        let view = &state.modules.view;
        let moving = state.shortcut_moving();
        let bottom = self.view_bottom();
        let selected = TOP + state.cursor as f32 * PITCH;
        let offset = self.ui.settings_scroll.offset(
            state.page(),
            selected,
            selected + ROW + 3.,
            (TOP + items.len() as f32 * PITCH, bottom),
            self.ui.frame.now,
        );
        self.ui.canvas.set_clip_rect(self.rect((
            16.,
            VIEW_TOP,
            LIST_RIGHT + 4.,
            bottom - VIEW_TOP,
        )));
        let drawn: Result<(), String> = (|| {
            for (n, item) in items.iter().enumerate() {
                let y = TOP + n as f32 * PITCH - offset;
                if y + ROW < VIEW_TOP || y > bottom {
                    continue;
                }
                let Some(shortcut) = Shortcut::find(&item.id, view) else {
                    continue;
                };
                let focus = n == state.cursor;
                let lifted = focus && moving;
                // The lifted line stands out of the list, a little taller.
                let (x, y, h) = if lifted {
                    (28., y - 3., ROW + 6.)
                } else {
                    (20., y, ROW)
                };
                let right = LIST_RIGHT - 20. + x;
                if lifted {
                    let shadow = Rgba {
                        a: 90,
                        ..self.theme.scrim
                    };
                    self.ui.rounded(
                        self.rect((x, y + 4., right - x, h)),
                        self.ui.px(10.) as f32,
                        shadow,
                        None,
                    )?;
                }
                if focus {
                    self.panel((x, y, right - x, h), true)?;
                }
                let ink = if focus {
                    self.theme.highlight_text
                } else if item.shown {
                    self.theme.text
                } else {
                    self.theme.dim_text
                };
                let icon = self.rect((x + 12., y + (h - 16.) / 2., 16., 16.));
                let label = self.shortcut_face(shortcut, icon, ink)?;
                let mut text_right = right - 12. - 38. - 10.;
                if lifted {
                    let note = self.tr("shortcuts.moving");
                    let width = self.span(&note, Face::Regular, 11.)?;
                    let at = text_right - width;
                    self.text(
                        (at, y, width + 2., h),
                        &note,
                        (11., self.theme.accent_text_secondary, Align::Right),
                    )?;
                    let arrows = self.rect((at - 18., y + (h - 14.) / 2., 14., 14.));
                    self.ui
                        .icon(Glyph::ArrowUpDown, arrows, self.theme.accent_text_secondary)?;
                    text_right = at - 24.;
                }
                let face = selection(focus, Face::Regular, Face::Semibold);
                self.text_face(
                    (x + 38., y, (text_right - x - 38.).max(20.), h),
                    &label,
                    (14., ink, Align::Left),
                    face,
                )?;
                self.toggle(
                    Id::Shortcut(n.min(255) as u8),
                    (item.shown, false),
                    focus,
                    (right - 12. - 38., y, 38., h),
                )?;
            }
            Ok(())
        })();
        self.ui.canvas.set_clip_rect(None);
        drawn?;
        self.shortcuts_panel(state, &items)?;
        match state.shortcuts.adding {
            Some(at) => self.shortcuts_chooser(state, at),
            None => Ok(()),
        }
    }
    /// The row as the START menu will show it, the limit, what Y adds and
    /// how to go back to the default.
    fn shortcuts_panel(
        &mut self,
        state: &Settings,
        items: &[crate::shortcuts::Item],
    ) -> Result<(), String> {
        let (x, w) = PANEL;
        let theme = self.theme;
        let bottom = self.view_bottom() - 10.;
        self.ui.rounded(
            self.rect((x, TOP, w, bottom - TOP)),
            self.ui.px(14.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        let (left, inner) = (x + 14., w - 28.);
        let mut y = TOP + 14.;
        self.text_face(
            (left, y, inner, 20.),
            &self.tr("shortcuts.preview"),
            (14., theme.text, Align::Left),
            Face::Semibold,
        )?;
        y += 28.;
        let shown: Vec<Shortcut> = items
            .iter()
            .filter(|i| i.shown)
            .filter_map(|i| Shortcut::find(&i.id, &state.modules.view))
            .collect();
        let per_line = ((inner + 6.) / 30.).floor().max(1.) as usize;
        for (n, shortcut) in shown.iter().enumerate() {
            let (cx, cy) = (
                left + (n % per_line) as f32 * 30.,
                y + (n / per_line) as f32 * 30.,
            );
            let dot = self.rect((cx, cy, 24., 24.));
            self.ui
                .rounded(dot, self.ui.px(12.) as f32, theme.badge, None)?;
            let icon = self.rect((cx + 5., cy + 5., 14., 14.));
            self.shortcut_face(*shortcut, icon, theme.text_secondary)?;
        }
        let lines = shown.len().div_ceil(per_line).max(1);
        y += lines as f32 * 30. + 4.;
        let limit = self.tr(&format!("shortcuts.limit|count={MAX}"));
        y = self.block((left, y, inner), &limit, (12., theme.muted))? + 12.;
        let addable = state.shortcut_addable();
        if !addable.is_empty() {
            let heading = self.tr("shortcuts.to_add").to_uppercase();
            y = self.block((left, y, inner), &heading, (11., theme.muted))? + 4.;
            for builtin in addable {
                let name = self.tr(builtin.label());
                y = self.block((left, y, inner), &name, (12., theme.text_secondary))? + 3.;
            }
        }
        // At the panel's foot, kept clear of what is above it.
        let restore = self.tr("shortcuts.restore");
        let height = self.block_height(&restore, 11., inner)?;
        let at = (bottom - 14. - height).max(y + 4.);
        self.block((left, at, inner), &restore, (11., theme.dim_text))
            .map(drop)
    }
    /// Lines of `text` at `size` in `width`, as tall as they are drawn.
    fn block_height(&mut self, text: &str, size: f32, width: f32) -> Result<f32, String> {
        let room = self.rect((0., 0., width, 1.)).width();
        let lines = self.lines(text, size, room)?.len() as f32;
        Ok(lines * self.pitch(size, size + 4.))
    }
    /// `text` wrapped from `y` down; where the next thing may start.
    fn block(
        &mut self,
        (x, y, width): (f32, f32, f32),
        text: &str,
        (size, ink): (f32, Rgba),
    ) -> Result<f32, String> {
        let height = self.block_height(text, size, width)?;
        self.text((x, y, width, height), text, (size, ink, Align::Left))?;
        Ok(y + height)
    }
    /// Y: the built-ins the list lacks, over the page.
    fn shortcuts_chooser(&mut self, state: &Settings, at: usize) -> Result<(), String> {
        let addable: Vec<Builtin> = state.shortcut_addable();
        let theme = self.theme;
        let full = self.rect((0., 0., 640., self.footer_top()));
        self.ui.rounded(full, 0., theme.scrim, None)?;
        let height = 56. + addable.len() as f32 * 38. + 12.;
        let top = ((VIEW_TOP + self.view_bottom() - height) / 2.).round();
        let (x, w) = (180., 280.);
        self.ui.rounded(
            self.rect((x, top, w, height)),
            self.ui.px(14.) as f32,
            theme.surface_elevated,
            Some(theme.border),
        )?;
        self.text_face(
            (x + 18., top + 14., w - 36., 24.),
            &self.tr("shortcuts.add_title"),
            (16., theme.text, Align::Left),
            Face::Semibold,
        )?;
        for (n, builtin) in addable.iter().enumerate() {
            let y = top + 52. + n as f32 * 38.;
            let focus = n == at;
            if focus {
                self.panel((x + 10., y, w - 20., 34.), true)?;
            }
            let ink = self.ink(focus);
            let icon = self.rect((x + 22., y + 9., 16., 16.));
            let label = self.shortcut_face(Shortcut::Builtin(*builtin), icon, ink)?;
            self.text_face(
                (x + 48., y, w - 70., 34.),
                &label,
                (14., ink, Align::Left),
                selection(focus, Face::Regular, Face::Semibold),
            )?;
        }
        Ok(())
    }
}
