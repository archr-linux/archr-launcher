//! Toasts (Avisos): the cards under the status pill, newest on top. They
//! slide in from their side of the screen and out again; a bar at the
//! bottom shrinks with the time left.
use super::*;
use crate::{
    appearance::mix,
    screens::{Badge, Corner},
    toast::Kind,
};

/// The card's width at 640×480, narrower only on a screen without room.
const WIDTH: f32 = 276.;
const SLIDE: Duration = Duration::from_millis(220);

impl Ui<'_, '_> {
    pub(super) fn toasts(&mut self, theme: &Theme) -> Result<(), String> {
        let cards = self.toasts.shown().to_vec();
        if cards.is_empty() {
            return Ok(());
        }
        let Frame { width, now, .. } = self.frame;
        let style = theme.screens.toast;
        let margin = self.px(24.);
        // The theme's width ([toast] width), the launcher's by default.
        let wide = if style.width == crate::screens::ToastStyle::default().width {
            self.px(WIDTH)
        } else {
            self.frame.size(style.width).round() as i32
        };
        let w = wide.min(width as i32 - 2 * margin);
        let pad = self.px(12.);
        let tile = self.px(28.);
        let text_x = if style.icon == Badge::None {
            pad
        } else {
            pad + tile + self.px(10.)
        };
        let text_w = (w - text_x - pad).max(1) as u32;
        let step = self.px(16.);
        let slide = self.motion.travel(SLIDE).as_secs_f32();
        // In a corner: below the status pill, which is centred in the
        // header. From the top, centered: a banner over the header.
        let header = self.frame.size(theme.header_height).round() as i32;
        let mut next = if style.position == Corner::Top {
            self.px(12.)
        } else {
            header / 2 + self.px(13. + 10.)
        };
        for card in &cards {
            let y = next;
            let toast = &card.toast;
            let (glyph, ink) = match toast.kind {
                Kind::Done => (Glyph::Check, theme.success),
                Kind::Info => (Glyph::Info, theme.highlight_bg),
                Kind::Error => (Glyph::Alert, theme.warning),
            };
            // Two lines at most; the second ellipsizes what is left.
            let mut detail = self.wrap(&toast.detail, Face::Regular, 12., text_w)?;
            if detail.len() > 2 {
                detail = vec![detail[0].clone(), detail[1..].join(" ")];
            }
            let lines = detail.len() as i32;
            let body = if lines > 0 {
                self.px(2.) + step * lines
            } else {
                0
            };
            let h = self.px(14. + 18.) + body + self.px(10. + 3. + 9.);
            let shown = if slide > 0. {
                crate::motion::ease(card.edge(now).as_secs_f32() / slide)
            } else {
                1.
            };
            let shift = ((1. - shown) * (w + margin) as f32).round() as i32;
            let left = match style.position {
                Corner::User => self.toasts.prefs.left,
                Corner::TopLeft => true,
                Corner::TopRight | Corner::Top => false,
            };
            // From the top, centered, they drop in instead of sliding.
            let (x, y) = match style.position {
                Corner::Top => {
                    let drop = ((1. - shown) * (h + y) as f32).round() as i32;
                    ((width as i32 - w) / 2, y - drop)
                }
                _ if left => (margin - shift, y),
                _ => (width as i32 - margin - w + shift, y),
            };
            let rect = Rect::new(x, y, w as u32, h as u32);
            let edge = if toast.kind == Kind::Error {
                mix(theme.border, theme.warning, 0.6)
            } else {
                theme.border
            };
            if style.glass {
                // Over whatever is under it: the surface beneath the
                // glass, the theme's card corners. Nearly opaque: the
                // glass blurs the picture behind, never the tabs or rows
                // a notice passes over, which would read through it.
                let radius = self.frame.size(theme.radius_card).round();
                self.cast_shadow(rect, radius)?;
                self.rounded(
                    rect,
                    radius,
                    Rgba {
                        a: 250,
                        ..theme.surface
                    },
                    None,
                )?;
                self.glass(rect, radius, &theme.glass_or_default())?;
            } else {
                self.panel(rect, self.px(12.) as f32, theme.surface, edge)?;
            }
            if style.icon != Badge::None {
                let disc = Rect::new(x + pad, y + self.px(9.), tile as u32, tile as u32);
                let round = if style.icon == Badge::Square {
                    tile as f32 * 0.28
                } else {
                    tile as f32 / 2.
                };
                self.rounded(disc, round, mix(theme.surface, ink, 0.18), None)?;
                let side = self.px(16.);
                let inset = (tile - side) / 2;
                let icon = Rect::new(disc.x() + inset, disc.y() + inset, side as u32, side as u32);
                self.icon(glyph, icon, ink)?;
            }
            let top = y + self.px(14.);
            self.label(
                &toast.title,
                (Face::Semibold, 14., theme.text),
                Rect::new(x + text_x, top, text_w, self.px(18.) as u32),
            )?;
            for (n, line) in detail.iter().enumerate() {
                let at = top + self.px(18. + 2.) + step * n as i32;
                self.label(
                    line,
                    (Face::Regular, 12., theme.text_secondary),
                    Rect::new(x + text_x, at, text_w, step as u32),
                )?;
            }
            let track = Rect::new(
                x + pad,
                y + h - self.px(9. + 3.),
                (w - 2 * pad) as u32,
                self.px(3.) as u32,
            );
            let radius = track.height() as f32 / 2.;
            self.rounded(track, radius, mix(theme.surface, ink, 0.15), None)?;
            let left = (track.width() as f32 * card.left(now)).round() as u32;
            if left > 0 {
                let bar = Rect::new(track.x(), track.y(), left, track.height());
                self.rounded(bar, radius, ink, None)?;
            }
            next += h + self.px(8.);
        }
        Ok(())
    }
}
