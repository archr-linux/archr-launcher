//! The card announcing a USB adapter just plugged (Dongle-aviso) and the
//! verdict tag it shares with Network › USB adapters.
use super::*;
use crate::{
    appearance::mix,
    usb::{Card, Verdict},
};

/// Fill, border and ink of a verdict tag.
pub(crate) fn tone(theme: &Theme, verdict: Verdict, selected: bool) -> (Rgba, Rgba, Rgba) {
    if selected {
        let ink = theme.highlight_text;
        return (Rgba { a: 46, ..ink }, Rgba { a: 89, ..ink }, ink);
    }
    if verdict == Verdict::Works {
        let ink = theme.success;
        return (
            mix(theme.background, ink, 0.12),
            mix(theme.background, ink, 0.3),
            ink,
        );
    }
    let amber = theme.warning;
    (
        mix(theme.background, amber, 0.15),
        mix(theme.background, amber, 0.4),
        mix(amber, theme.text, 0.4),
    )
}

impl Ui<'_, '_> {
    /// A small rounded tag ending at `right`, centred on `middle`; its width.
    pub(crate) fn tag(
        &mut self,
        text: &str,
        (right, middle): (i32, i32),
        (fill, border, ink): (Rgba, Rgba, Rgba),
    ) -> Result<i32, String> {
        let w = self.measure(text, Face::Semibold, 11.)? as i32 + self.px(14.);
        let h = self.px(19.);
        let rect = Rect::new(right - w, middle - h / 2, w as u32, h as u32);
        self.rounded(rect, self.px(8.) as f32, fill, Some(border))?;
        self.label_aligned(text, (Face::Semibold, 11., ink), rect, Align::Center)?;
        Ok(w)
    }
    /// Dongle-aviso: over the screen, below the header, until a press (or
    /// five seconds when the adapter simply works).
    pub(super) fn dongle_card(&mut self, card: &Card, theme: &Theme) -> Result<(), String> {
        // The screen behind steps back, as under a dialog.
        self.scrim(theme, 0.9)?;
        let adapter = &card.adapter;
        let works = adapter.verdict == Verdict::Works;
        let (fill, edge, ink) = tone(theme, adapter.verdict, false);
        let (x, w) = (self.px(20.), self.frame.width as i32 - self.px(40.));
        let pad = (self.px(14.), self.px(16.));
        let tile = self.px(40.);
        let text_x = x + pad.1 + tile + self.px(14.);
        let text_w = (x + w - pad.1 - text_x).max(1) as u32;
        let body = self.wrap(
            &adapter.advice(&card.chips, &self.catalog),
            Face::Regular,
            13.,
            text_w,
        )?;
        let step = self.px(19.);
        let h = pad.0 * 2
            + self.px(20. + 4. + 16. + 4. + 2. + 4. + 6. + 20.)
            + step * body.len() as i32;
        let card_rect = Rect::new(x, self.px(54.), w as u32, h as u32);
        self.panel(card_rect, self.px(14.) as f32, theme.surface, edge)?;
        let top = card_rect.y() + pad.0;
        let tile_rect = Rect::new(x + pad.1, top, tile as u32, tile as u32);
        self.rounded(tile_rect, self.px(12.) as f32, fill, None)?;
        let side = self.px(20.);
        let glyph = Rect::new(
            tile_rect.x() + (tile - side) / 2,
            top + (tile - side) / 2,
            side as u32,
            side as u32,
        );
        self.icon(Glyph::Usb, glyph, ink)?;
        // Title and its verdict.
        let tag = self
            .catalog
            .text(if works { "usb.works" } else { "usb.fails" });
        let tag_w = self.tag(&tag, (x + w - pad.1, top + self.px(10.)), (fill, edge, ink))?;
        let title = self.catalog.text(adapter.title());
        let title_w = (text_w as i32 - tag_w - self.px(10.)).max(1) as u32;
        let size = self
            .shrink(&title, Face::Semibold, 16., 13., title_w)?
            .unwrap_or(13.);
        self.label(
            &title,
            (Face::Semibold, size, theme.text),
            Rect::new(text_x, top, title_w, self.px(20.) as u32),
        )?;
        let mut y = top + self.px(24.);
        self.label(
            &format!("{} · {}", adapter.name, adapter.id),
            (Face::Mono, 12., theme.muted),
            Rect::new(text_x, y, text_w, self.px(16.) as u32),
        )?;
        y += self.px(16. + 4. + 2.);
        for line in &body {
            self.label(
                line,
                (Face::Regular, 13., theme.text_secondary),
                Rect::new(text_x, y, text_w, step as u32),
            )?;
            y += step;
        }
        // The answers, left-aligned under the text (the hints right-align
        // from the edge they are given).
        y += self.px(4. + 6.);
        let hints = [('Y', "usb.see"), ('B', "hint.close")];
        let mut width = self.px(16.) * (hints.len() as i32 - 1);
        for (_, key) in hints {
            let label = self.catalog.text(key);
            width += self.measure(&label, Face::Regular, 13.)? as i32 + self.px(26.);
        }
        let right = text_x + width + self.px(24.);
        self.hints(&hints, theme, right.max(0) as u32, y, self.px(20.) as u32)?;
        Ok(())
    }
}
