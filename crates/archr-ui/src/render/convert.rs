//! Converter-save: take the cartridge saves to the new emulator?
use super::*;
use crate::{appearance::mix, convert::Offer};

impl Ui<'_, '_> {
    pub(crate) fn convert_question(&mut self, offer: &Offer, theme: &Theme) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let footer = (theme.footer_height * height as f32).round().max(1.) as u32;
        self.scrim(theme, 1.2)?;
        let w = self.px(400.).min(width as i32 - self.px(32.)).max(1);
        let pad = self.px(20.);
        let inner = (w - 2 * pad).max(1) as u32;
        let gap = self.px(12.);
        let badge = self.px(36.);
        let title_room = (inner as i32 - badge - gap).max(1) as u32;
        let question = self.catalog.format(
            if offer.rom.is_some() {
                "convert.title_game"
            } else {
                "convert.title"
            },
            &[("from", &offer.from_name), ("to", &offer.to_name)],
        );
        let titles = self.wrap(&question, Face::Semibold, 17., title_room)?;
        let title_step = self.px(21.);
        let title_h = (title_step * titles.len() as i32).max(badge);
        let mut detail = self
            .catalog
            .format("convert.detail", &[("keep", &offer.keep)]);
        if offer.replace > 0 {
            let n = offer.replace.to_string();
            detail.push(' ');
            detail.push_str(&self.catalog.plural(
                "convert.replace",
                offer.replace as f64,
                &[("count", &n)],
            ));
        }
        let body = self.wrap(&detail, Face::Regular, 13., inner)?;
        let body_step = self.px(19.);
        let icon = self.px(14.);
        let warn_room = (inner as i32 - icon - self.px(8.)).max(1) as u32;
        let warning = self.wrap(
            &self.catalog.text("convert.states"),
            Face::Regular,
            12.,
            warn_room,
        )?;
        let warn_step = self.px(17.);
        let item_h = self.px(54.);
        let button_h = self.px(40.);
        let h = pad
            + title_h
            + gap
            + item_h
            + gap
            + body_step * body.len() as i32
            + gap
            + warn_step * warning.len() as i32
            + gap
            + self.px(2.)
            + button_h
            + self.px(18.);
        let room = height as i32 - footer as i32;
        let panel = Rect::new(
            (width as i32 - w) / 2,
            ((room - h) / 2).max(0),
            w as u32,
            h as u32,
        );
        self.panel(panel, self.px(16.) as f32, theme.surface, theme.border)?;
        let accent = theme.highlight_bg;
        let x = panel.x() + pad;
        let mut y = panel.y() + pad;
        let circle = Rect::new(x, y + (title_h - badge) / 2, badge as u32, badge as u32);
        self.rounded(circle, badge as f32 / 2., Rgba { a: 36, ..accent }, None)?;
        let side = self.px(18.);
        let glyph = Rect::new(
            circle.x() + (badge - side) / 2,
            circle.y() + (badge - side) / 2,
            side as u32,
            side as u32,
        );
        self.icon(Glyph::ArrowLeftRight, glyph, accent)?;
        let text_x = circle.right() + gap;
        let text_top = y + (title_h - title_step * titles.len() as i32) / 2;
        for (n, line) in titles.iter().enumerate() {
            self.label(
                line,
                (Face::Semibold, 17., theme.text),
                Rect::new(
                    text_x,
                    text_top + n as i32 * title_step,
                    title_room,
                    title_step as u32,
                ),
            )?;
        }
        y += title_h + gap;
        // From → to, with how many saves each side.
        let item = Rect::new(x, y, inner, item_h as u32);
        self.rounded(
            item,
            self.px(10.) as f32,
            theme.background,
            Some(theme.border),
        )?;
        let half = (inner as i32 - self.px(24. + 18. + 20.)) / 2;
        let count = |ext: &str| {
            let n = offer.count.to_string();
            let n = if offer.up_to {
                self.catalog.format("convert.up_to", &[("count", &n)])
            } else {
                n
            };
            format!("{n} × {ext}")
        };
        let counts = [count(&offer.from_ext), count(&offer.to_ext)];
        for (n, name) in [&offer.from_name, &offer.to_name].into_iter().enumerate() {
            let (left, align) = if n == 0 {
                (item.x() + self.px(12.), Align::Left)
            } else {
                (item.right() - self.px(12.) - half, Align::Right)
            };
            let middle = item.y() + item_h / 2;
            self.label_aligned(
                name,
                (Face::Semibold, 13., theme.text),
                Rect::new(
                    left,
                    middle - self.px(18.),
                    half as u32,
                    self.px(18.) as u32,
                ),
                align,
            )?;
            self.label_aligned(
                &counts[n],
                (Face::Mono, 12., theme.muted),
                Rect::new(left, middle + self.px(1.), half as u32, self.px(16.) as u32),
                align,
            )?;
        }
        let arrow = Rect::new(
            item.x() + (inner as i32 - side) / 2,
            item.y() + (item_h - side) / 2,
            side as u32,
            side as u32,
        );
        self.icon(Glyph::ArrowRight, arrow, accent)?;
        y += item_h + gap;
        for line in &body {
            self.label(
                line,
                (Face::Regular, 13., theme.muted),
                Rect::new(x, y, inner, body_step as u32),
            )?;
            y += body_step;
        }
        y += gap;
        let amber = mix(theme.warning, theme.text, 0.4);
        self.icon(
            Glyph::Alert,
            Rect::new(x, y + (warn_step - icon) / 2, icon as u32, icon as u32),
            amber,
        )?;
        for line in &warning {
            self.label(
                line,
                (Face::Regular, 12., amber),
                Rect::new(x + icon + self.px(8.), y, warn_room, warn_step as u32),
            )?;
            y += warn_step;
        }
        y += gap + self.px(2.);
        // Convert, then "Not now"; the focused one is filled.
        let half = ((inner as i32 - self.px(10.)) / 2).max(1) as u32;
        for (n, key) in ["convert.go", "convert.later"].into_iter().enumerate() {
            let rect = Rect::new(
                x + n as i32 * (half as i32 + self.px(10.)),
                y,
                half,
                button_h as u32,
            );
            let focused = (n == 0) == offer.on_convert;
            let (fill, border, ink, face) = if focused {
                (accent, None, theme.highlight_text, Face::Semibold)
            } else {
                (theme.badge, Some(theme.border), theme.text, Face::Regular)
            };
            let frame = if focused { "button_selected" } else { "button" };
            if !self.framed(frame, rect)? {
                self.rounded(rect, self.px(10.) as f32, fill, border)?;
            }
            let label = self.catalog.text(key);
            let floor = self.frame.min_size();
            let room = rect.width().saturating_sub(self.px(40.) as u32).max(1);
            let size = self
                .shrink(&label, face, 15., floor, room)?
                .unwrap_or(floor);
            if n == 1 {
                self.label_aligned(&label, (face, size, ink), rect, Align::Center)?;
                continue;
            }
            let lw = self.measure(&label, face, size)? as i32;
            let side = self.px(15.);
            let start = rect.x() + (rect.width() as i32 - side - self.px(8.) - lw) / 2;
            let glyph = Rect::new(
                start,
                rect.y() + (button_h - side) / 2,
                side as u32,
                side as u32,
            );
            self.icon(Glyph::ArrowLeftRight, glyph, ink)?;
            self.label(
                &label,
                (face, size, ink),
                Rect::new(
                    glyph.right() + self.px(8.),
                    rect.y(),
                    lw.max(1) as u32,
                    rect.height(),
                ),
            )?;
        }
        // The footer answers the question.
        let fy = height as i32 - footer as i32;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.fill_rect(Rect::new(0, fy, width, footer))?;
        self.divider(0, fy, width, theme.border)?;
        self.hints(
            &[
                (HintButton::Pair('‹', '›'), "hint.choose"),
                ('A'.into(), "hint.confirm"),
                ('B'.into(), "hint.cancel"),
            ],
            theme,
            width,
            fy,
            footer,
        )?;
        Ok(())
    }
}
