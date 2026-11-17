//! A game's achievements (RA-jogo): progress, then the list with badges.
//! Both blocks sit in theme rectangles (`achievements_*`), like the details.
use super::*;
use crate::{appearance::mix, cheevos::Fetch};

impl Ui<'_, '_> {
    pub(super) fn achievements(
        &mut self,
        app: &App,
        game: &str,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let current = self.catalog.text("menu.achievements");
        let style = theme.screens.achievements;
        // Under the theme's capsule header the ring carries the title
        // beside it, on the line the title would take.
        let lower = self.game_header((game, game), &current, !style.ring, theme)?;
        let sheet = &app.sheet;
        let Frame { width, height, .. } = self.frame;
        let (done, earned, total_points) = sheet.progress();

        // The newer look: a group of glass, locks on the locked ones.
        let fancy = style.ring || style.glass;
        // Progress: label and counts over a bar, the hardcore chip beside them.
        let block =
            super::details::lowered(pixels(theme.achievements_progress, width, height), lower);
        if style.ring {
            let title = if theme.screens.header.capsule {
                (current.as_str(), 28.)
            } else {
                (game, 24.)
            };
            self.cheevos_ring(sheet, title, block, theme)?;
        }
        let pad = self.px(10.);
        let mut right = block.right() - pad;
        if sheet.hardcore && !style.ring {
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
        if !sheet.items.is_empty() && !style.ring {
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
        let list = super::details::lowered(pixels(theme.achievements_list, width, height), lower);
        let (row, gap) = (self.px(style.row * 480.), self.px(style.gap * 480.));
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
        let shown = sheet.items.len().saturating_sub(first).min(visible) as i32;
        if style.glass && shown > 0 {
            let group = Rect::new(
                list.x(),
                list.y(),
                list.width(),
                (shown * (row + gap) - gap).max(1) as u32,
            );
            self.glass(group, self.px(16.) as f32, &theme.glass_or_default())?;
        }
        for (n, item) in sheet.items.iter().enumerate().skip(first).take(visible) {
            let y = list.y() + (n - first) as i32 * (row + gap);
            let rect = Rect::new(list.x(), y, list.width(), row as u32);
            if style.glass {
                if n == app.cursor {
                    let band = Rect::new(
                        rect.x() + self.px(4.),
                        rect.y() + self.px(3.),
                        rect.width().saturating_sub(self.px(8.) as u32).max(1),
                        (row - self.px(6.)).max(1) as u32,
                    );
                    let lit = Rgba {
                        a: 0x24,
                        ..theme.text
                    };
                    self.rounded(band, self.px(12.) as f32, lit, None)?;
                } else if n + 1 < first + shown as usize && n + 1 != app.cursor {
                    let inset = self.px(16.) + self.px(style.badge * 480.) + self.px(12.);
                    self.divider(
                        rect.x() + inset,
                        rect.bottom() + gap / 2,
                        rect.width()
                            .saturating_sub(inset as u32 + self.px(16.) as u32),
                        Rgba {
                            a: 0x1A,
                            ..theme.text
                        },
                    )?;
                }
            } else if n == app.cursor {
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
            let side = self.px(style.badge * 480.);
            let inset = if style.glass {
                self.px(16.)
            } else {
                self.px(10.)
            };
            let icon = Rect::new(
                rect.x() + inset,
                y + (row - side) / 2,
                side as u32,
                side as u32,
            );
            let badge = item
                .badge
                .as_ref()
                .and_then(|path| art.badges.and_then(|cache| cache.peek(path)));
            match badge {
                Some(texture) if fancy => {
                    let radius = self.px(style.badge_radius * 480.) as f32;
                    let a = if item.unlocked { 255 } else { 110 };
                    self.geometry.cover(
                        &mut self.canvas,
                        texture,
                        icon,
                        radius,
                        a,
                        [0., 0., 1., 1.],
                    )?;
                }
                Some(texture) => self.canvas.copy(texture, None, icon)?,
                None => {
                    let (fill, ink) = if item.unlocked {
                        (theme.highlight_bg, theme.highlight_text)
                    } else {
                        (theme.badge, theme.dim_text)
                    };
                    let radius = self.px(style.badge_radius * 480.) as f32;
                    self.rounded(icon, radius, fade(fill), None)?;
                    let (offset, inner) = if fancy {
                        ((side - side / 2) / 2, side / 2)
                    } else {
                        (self.px(9.), self.px(18.))
                    };
                    let glyph = Rect::new(
                        icon.x() + offset,
                        icon.y() + offset,
                        inner as u32,
                        inner as u32,
                    );
                    let mark = if fancy && !item.unlocked {
                        Glyph::Lock
                    } else {
                        Glyph::Trophy
                    };
                    self.icon(mark, glyph, fade(ink))?;
                }
            }
            let points = self
                .catalog
                .format("cheevos.points", &[("count", &item.points.to_string())]);
            let points_w = self.measure(&points, Face::Mono, 12.)?;
            let points_x = rect.right() - inset - points_w as i32;
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
        let h = self.frame.size(theme.footer_height).round().max(1.) as u32;
        let y = height as i32 - h as i32;
        // The theme's hints ([hints], a capsule) need no rule above them.
        if theme.screens.hints == crate::screens::Hints::default() {
            self.divider(0, y, width, theme.border)?;
        }
        let hints = [
            (HintButton::Pair('L', 'R'), "hint.page"),
            (HintButton::Key('B'), "hint.back"),
        ];
        let left = self.placed_hints(&hints, theme, (width, y, h))?;
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
    /// The progress as a ring with "2/7" inside, a title large (the
    /// game's name, or the screen's under a capsule header, at its size),
    /// the points and the hardcore capsule in the theme's warning color.
    fn cheevos_ring(
        &mut self,
        sheet: &crate::cheevos::Sheet,
        (game, big): (&str, f32),
        block: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let (done, earned, total_points) = sheet.progress();
        let total = sheet.items.len();
        let side = (block.height() as i32).min(self.px(84.)).max(self.px(24.));
        let ring = Rect::new(
            block.x(),
            block.y() + (block.height() as i32 - side) / 2,
            side as u32,
            side as u32,
        );
        let stroke = (side as f32 * 0.1).max(3.);
        let track = Rgba {
            a: 0x30,
            ..theme.text
        };
        self.geometry
            .ring(&mut self.canvas, ring, side as f32 / 2., stroke, track)?;
        if total > 0 && done > 0 {
            let part = done as f32 / total as f32;
            let (cx, cy) = (
                ring.x() as f32 + side as f32 / 2.,
                ring.y() as f32 + side as f32 / 2.,
            );
            let r = side as f32 / 2. - stroke / 2.;
            let steps = (64. * part).ceil().max(2.) as usize;
            let at = |k: usize| {
                let a = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * part * k as f32 / steps as f32;
                (cx + r * a.cos(), cy + r * a.sin())
            };
            let mut sketch = crate::primitives::Sketch::new();
            for k in 0..steps {
                sketch.line(at(k), at(k + 1), stroke, theme.highlight_bg);
            }
            sketch.draw(&mut self.canvas)?;
            // Round ends.
            for p in [at(0), at(steps)] {
                let d = stroke.round().max(1.) as u32;
                let dot = Rect::new(
                    (p.0 - stroke / 2.).round() as i32,
                    (p.1 - stroke / 2.).round() as i32,
                    d,
                    d,
                );
                self.rounded(dot, d as f32 / 2., theme.highlight_bg, None)?;
            }
        }
        if total > 0 {
            let count = format!("{done}/{total}");
            let size = self
                .shrink(&count, Face::Semibold, 17., 10., (side as f32 * 0.7) as u32)?
                .unwrap_or(10.);
            self.label_aligned(
                &count,
                (Face::Semibold, size, theme.text),
                ring,
                Align::Center,
            )?;
        }
        let x = ring.right() + self.px(16.);
        let w = (block.right() - x).max(1) as u32;
        let size = self.shrink(game, Face::Bold, big, 15., w)?.unwrap_or(15.);
        let title_h = (self.frame.size(size / 480.) * 1.25).round() as i32;
        let line_h = self.px(18.);
        let chip_h = if sheet.hardcore {
            self.px(22.) + self.px(6.)
        } else {
            0
        };
        let mut y = ring.y() + (side - title_h - line_h - chip_h) / 2;
        self.label(
            game,
            (Face::Bold, size, theme.text),
            Rect::new(x, y, w, title_h as u32),
        )?;
        y += title_h;
        if total > 0 {
            let counts = self.catalog.format(
                "cheevos.count",
                &[
                    ("done", &done.to_string()),
                    ("total", &total.to_string()),
                    ("points", &earned.to_string()),
                    ("max", &total_points.to_string()),
                ],
            );
            self.label(
                &counts,
                (Face::Regular, 13., theme.text_secondary),
                Rect::new(x, y, w, line_h as u32),
            )?;
        }
        y += line_h + self.px(6.);
        if sheet.hardcore {
            let text = self.catalog.text("cheevos.hardcore");
            let cw = self.measure(&text, Face::Semibold, 12.)? + self.px(20.) as u32;
            let chip = Rect::new(x, y, cw, self.px(22.) as u32);
            self.rounded(chip, chip.height() as f32 / 2., theme.warning, None)?;
            self.label_aligned(
                &text,
                (Face::Semibold, 12., theme.background),
                chip,
                Align::Center,
            )?;
        }
        Ok(())
    }
}
