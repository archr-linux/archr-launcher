//! Key grid of the shared on-screen keyboard (Wifi-senha mockup geometry).
use super::*;
use crate::keyboard::{Key, Keyboard};

impl Ui<'_, '_> {
    /// Rows are centered on the 640 px screen, 34 px tall with 6 px gaps.
    pub(crate) fn keyboard_keys(
        &mut self,
        keyboard: &Keyboard,
        top: f32,
        (space, submit): (&str, &str),
        theme: &Theme,
    ) -> Result<(), String> {
        let at = |ui: &Self, x: f32, y: f32, w: f32, h: f32| {
            Rect::new(ui.px(x), ui.px(y), ui.px(w) as u32, ui.px(h) as u32)
        };
        let (row, column) = keyboard.selected();
        for (r, keys) in keyboard.rows().iter().enumerate() {
            let width = keys.iter().map(|k| k.width() + 6.).sum::<f32>() - 6.;
            let mut x = 320. - width / 2.;
            let y = top + r as f32 * 40.;
            for (c, key) in keys.iter().enumerate() {
                let w = key.width();
                let rect = at(self, x, y, w, 34.);
                x += w + 6.;
                let selected = (r, c) == (row, column);
                let accent = selected || *key == Key::Submit;
                let fill = selection(accent, theme.surface, theme.highlight_bg);
                self.rounded(
                    rect,
                    self.px(8.) as f32,
                    fill,
                    Some(selection(accent, theme.border, theme.highlight_bg)),
                )?;
                let modifier = !matches!(key, Key::Char(t) if t.len() == 1);
                let ink = if accent {
                    theme.highlight_text
                } else if modifier && !(*key == Key::Shift && keyboard.shifted) {
                    theme.muted
                } else {
                    theme.text
                };
                let glyph = match key {
                    Key::Shift => Some(Glyph::Shift),
                    Key::Back => Some(Glyph::Backspace),
                    _ => None,
                };
                if let Some(glyph) = glyph {
                    let icon = at(self, x - w - 6. + (w - 18.) / 2., y + 8., 18., 18.);
                    self.icon(glyph, icon, ink)?;
                    continue;
                }
                let text = match key {
                    Key::Char(text) if keyboard.shifted => text.to_uppercase(),
                    Key::Char(text) | Key::Layer(_, text) => (*text).to_owned(),
                    Key::Space => space.to_owned(),
                    _ => submit.to_owned(),
                };
                let face = selection(accent, Face::Regular, Face::Semibold);
                let mut size = if *key == Key::Submit { 14. } else { 15. };
                while size > 12. && self.measure(&text, face, size)? > self.px(w - 8.) as u32 {
                    size -= 1.;
                }
                self.label_aligned(&text, (face, size, ink), rect, Align::Center)?;
            }
        }
        Ok(())
    }
}
