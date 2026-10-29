//! A game's achievements (RA-jogo): progress, then the list with badges.
//! Both blocks sit in theme rectangles (`achievements_*`), like the details.
use super::*;
use crate::{appearance::mix, cheevos::Fetch};

/// Row height and gap at the 640×480 reference.
const ROW: f32 = 50.;
const GAP: f32 = 2.;

impl Ui<'_, '_> {
    pub(super) fn achievements(
        &mut self,
        app: &App,
        game: &str,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let current = self.catalog.text("menu.achievements");
        self.crumbs(game, &current, theme)?;
        let sheet = &app.sheet;
        let Frame { width, height, .. } = self.frame;
        let (done, earned, total_points) = sheet.progress();

        // Progress: label and counts over a bar, the hardcore chip beside them.
        let block = pixels(theme.achievements_progress, width, height);
        let pad = self.px(10.);
        let mut right = block.right() - pad;
        if sheet.hardcore {
            let text = self.catalog.text("cheevos.hardcore");
            let w = self.measure(&text, Face::Regular, 12.)? + self.px(18.) as u32;
            let chip = Rect::new(
                right - w as i32,
                block.y() + (block.height() as i32 - self.px(22.)) / 2,
                w,
                self.px(22.) as u32,
            );
            self.rounded(
                chip,
                self.px(11.) as f32,
                theme.background,
                Some(theme.highlight_bg),
            )?;
            self.label_aligned(
                &text,
                (Face::Regular, 12., theme.highlight_bg),
                chip,
                Align::Center,
            )?;
            right = chip.x() - self.px(14.);
        }
        let x = block.x() + pad;
        let w = (right - x).max(1) as u32;
        if !sheet.items.is_empty() {
            let line = Rect::new(x, block.y(), w, self.px(20.) as u32);
            self.label(
                &self.catalog.text("cheevos.progress"),
                (Face::Regular, 13., theme.muted),
                line,
            )?;
            let counts = self.catalog.format(
                "cheevos.count",
                &[
                    ("done", &done.to_string()),
                    ("total", &sheet.items.len().to_string()),
                    ("points", &earned.to_string()),
                    ("max", &total_points.to_string()),
                ],
            );
            self.label_aligned(&counts, (Face::Mono, 13., theme.text), line, Align::Right)?;
            let bar = Rect::new(x, block.bottom() - self.px(6.), w, self.px(6.) as u32);
            self.rounded(bar, self.px(3.) as f32, theme.border, None)?;
            let fill = (w as usize * done / sheet.items.len()) as u32;
            if fill > 0 {
                let fill = Rect::new(x, bar.y(), fill.max(bar.height()), bar.height());
                self.rounded(fill, self.px(3.) as f32, theme.highlight_bg, None)?;
            }
        }

        // The list, a page at a time; the page follows the cursor.
        let list = pixels(theme.achievements_list, width, height);
        let (row, gap) = (self.px(ROW), self.px(GAP));
        let visible = ((list.height() as i32 + gap) / (row + gap)).max(1) as usize;
        self.achievements_page = visible;
        let message = match sheet.fetch {
            Fetch::Loading if sheet.items.is_empty() => Some("loading"),
            Fetch::Failed(key) => Some(key),
            _ if sheet.items.is_empty() => Some("cheevos.empty"),
            _ => None,
        };
        if let Some(key) = message {
            self.label_aligned(
                &self.catalog.text(key),
                (Face::Regular, 14., theme.muted),
                Rect::new(list.x(), list.y(), list.width(), self.px(80.) as u32),
                Align::Center,
            )?;
        }
        let first = app.cursor / visible * visible;
        for (n, item) in sheet.items.iter().enumerate().skip(first).take(visible) {
            let y = list.y() + (n - first) as i32 * (row + gap);
            let rect = Rect::new(list.x(), y, list.width(), row as u32);
            if n == app.cursor {
                self.rounded(
                    rect,
                    self.px(10.) as f32,
                    theme.surface,
                    Some(theme.highlight_bg),
                )?;
            }
            // A locked achievement reads at about half strength (opacity .55).
            let fade = |c: Rgba| {
                if item.unlocked {
                    c
                } else {
                    mix(theme.background, c, 0.55)
                }
            };
            let icon = Rect::new(
                rect.x() + self.px(10.),
                y + (row - self.px(36.)) / 2,
                self.px(36.) as u32,
                self.px(36.) as u32,
            );
            let badge = item
                .badge
                .as_ref()
                .and_then(|path| art.badges.and_then(|cache| cache.peek(path)));
            match badge {
                Some(texture) => self.canvas.copy(texture, None, icon)?,
                None => {
                    let (fill, ink) = if item.unlocked {
                        (theme.highlight_bg, theme.highlight_text)
                    } else {
                        (theme.badge, theme.dim_text)
                    };
                    self.rounded(icon, self.px(8.) as f32, fade(fill), None)?;
                    let glyph = Rect::new(
                        icon.x() + self.px(9.),
                        icon.y() + self.px(9.),
                        self.px(18.) as u32,
                        self.px(18.) as u32,
                    );
                    self.icon(Glyph::Trophy, glyph, fade(ink))?;
                }
            }
            let points = self
                .catalog
                .format("cheevos.points", &[("count", &item.points.to_string())]);
            let points_w = self.measure(&points, Face::Mono, 12.)?;
            let points_x = rect.right() - self.px(10.) - points_w as i32;
            self.label(
                &points,
                (Face::Mono, 12., fade(theme.muted)),
                Rect::new(points_x, y, points_w, row as u32),
            )?;
            let text_x = icon.right() + self.px(12.);
            let text_w = (points_x - self.px(12.) - text_x).max(1) as u32;
            let title_size = self
                .shrink(&item.title, Face::Semibold, 14., 12., text_w)?
                .unwrap_or(12.);
            // The description wraps to a second, smaller line before it is cut.
            let mut lines = self.wrap(&item.description, Face::Regular, 12., text_w)?;
            let small = lines.len() > 1;
            if small {
                lines = self.wrap(&item.description, Face::Regular, 11., text_w)?;
                lines.truncate(2);
            }
            let line_h = self.px(if small { 13. } else { 16. });
            let block_h = self.px(18.) + self.px(2.) + line_h * lines.len() as i32;
            let top = y + (row - block_h) / 2;
            self.label(
                &item.title,
                (Face::Semibold, title_size, fade(theme.text)),
                Rect::new(text_x, top, text_w, self.px(18.) as u32),
            )?;
            for (k, line) in lines.iter().enumerate() {
                self.label(
                    line,
                    (
                        Face::Regular,
                        if small { 11. } else { 12. },
                        fade(theme.muted),
                    ),
                    Rect::new(
                        text_x,
                        top + self.px(20.) + k as i32 * line_h,
                        text_w,
                        line_h as u32,
                    ),
                )?;
            }
        }

        // Footer: where the list came from, then the buttons.
        let h = (theme.footer_height * height as f32).round().max(1.) as u32;
        let y = height as i32 - h as i32;
        self.divider(0, y, width, theme.border)?;
        let hints = [
            (HintButton::Pair('L', 'R'), "hint.page"),
            (HintButton::Key('B'), "hint.back"),
        ];
        let left = self.hints(&hints, theme, width, y, h)?;
        let status = match sheet.fetch {
            Fetch::Fresh => Some("settings.updated_now"),
            Fetch::Saved => Some("cheevos.saved"),
            _ => None,
        };
        if let Some(key) = status {
            self.label(
                &self.catalog.text(key),
                (Face::Regular, 13., theme.muted),
                Rect::new(self.px(24.), y, (left - self.px(40.)).max(1) as u32, h),
            )?;
        }
        Ok(())
    }
}
