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
        let style = theme.screens.keyboard;
        // The theme's keys, or the launcher's 34 px with 6 px between.
        let (kh, gap, radius) = if style == crate::screens::KeyboardStyle::default() {
            (34., 6., 8.)
        } else {
            (
                style.key_h * 480.,
                style.key_gap * 480.,
                style.key_radius * 480.,
            )
        };
        let (row, column) = keyboard.selected();
        for (r, keys) in keyboard.rows().iter().enumerate() {
            let width = keys.iter().map(|k| k.width() + gap).sum::<f32>() - gap;
            let mut x = 320. - width / 2.;
            let y = top + r as f32 * (kh + gap);
            for (c, key) in keys.iter().enumerate() {
                let w = key.width();
                let rect = at(self, x, y, w, kh);
                x += w + gap;
                let selected = (r, c) == (row, column);
                let accent = selected || *key == Key::Submit;
                let modifier = !matches!(key, Key::Char(t) if t.len() == 1);
                if style.sheet {
                    // On the sheet: the focused key light, the others glass.
                    let (fill, ink) = if selected {
                        (theme.text, theme.background)
                    } else if *key == Key::Submit {
                        (theme.highlight_bg, theme.highlight_text)
                    } else if modifier {
                        (
                            Rgba {
                                a: 0x30,
                                ..theme.text
                            },
                            theme.text,
                        )
                    } else {
                        (
                            Rgba {
                                a: 0x1C,
                                ..theme.text
                            },
                            theme.text,
                        )
                    };
                    self.rounded(rect, self.px(radius) as f32, fill, None)?;
                    self.key_face(
                        keyboard,
                        key,
                        (rect, x - w - gap, w),
                        (space, submit),
                        (ink, selected),
                    )?;
                    continue;
                }
                let fill = selection(accent, theme.surface, theme.highlight_bg);
                self.rounded(
                    rect,
                    self.px(radius) as f32,
                    fill,
                    Some(selection(accent, theme.border, theme.highlight_bg)),
                )?;
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
                    let icon = at(
                        self,
                        x - w - gap + (w - 18.) / 2.,
                        y + (kh - 18.) / 2.,
                        18.,
                        18.,
                    );
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
    /// A key's glyph or word in `ink` over its `rect`; its `left` and `w`
    /// in reference pixels.
    fn key_face(
        &mut self,
        keyboard: &Keyboard,
        key: &Key,
        (rect, left, w): (Rect, f32, f32),
        (space, submit): (&str, &str),
        (ink, bold): (Rgba, bool),
    ) -> Result<(), String> {
        let glyph = match key {
            Key::Shift => Some(Glyph::Shift),
            Key::Back => Some(Glyph::Backspace),
            _ => None,
        };
        if let Some(glyph) = glyph {
            let side = self.px(18.);
            let icon = Rect::new(
                self.px(left + (w - 18.) / 2.),
                rect.y() + (rect.height() as i32 - side) / 2,
                side as u32,
                side as u32,
            );
            return self.icon(glyph, icon, ink);
        }
        let text = match key {
            Key::Char(text) if keyboard.shifted => text.to_uppercase(),
            Key::Char(text) | Key::Layer(_, text) => (*text).to_owned(),
            Key::Space => space.to_owned(),
            _ => submit.to_owned(),
        };
        let face = if bold || *key == Key::Submit {
            Face::Semibold
        } else {
            Face::Regular
        };
        let mut size = if *key == Key::Submit { 14. } else { 15. };
        while size > 12. && self.measure(&text, face, size)? > self.px(w - 8.) as u32 {
            size -= 1.;
        }
        self.label_aligned(&text, (face, size, ink), rect, Align::Center)
    }
}
