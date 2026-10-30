//! Moving the ROMs to the exFAT partition (Armazenamento-roms-movendo): the
//! maintenance boot's only screen. A fixed composition, like Settings: the
//! theme colors it and gives its fonts and icon.
use super::*;

/// What the migration is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveStage {
    /// Renaming, checking and shrinking STORAGE, before any copy.
    #[default]
    Preparing,
    Copying,
    Verifying,
}

/// The migration as its progress file tells it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RomsMove {
    pub stage: MoveStage,
    /// The entry being moved (from 1), how many there are, and its name.
    pub entry: (usize, usize, String),
    /// Bytes moved and bytes in all.
    pub bytes: (u64, u64),
    /// Seconds left, once the copy has run long enough to tell.
    pub left: Option<u64>,
}

/// The bar and the line under it.
const BAR: f32 = 420.;
const GAP: f32 = 14.;
/// Text line heights, in reference pixels, for the sizes used here.
fn line(size: f32) -> f32 {
    (size * 1.25).round()
}

impl Ui<'_, '_> {
    /// One frame of the screen; the battery comes from the status worker.
    pub fn draw_roms_move(&mut self, view: &RomsMove, theme: &Theme) -> Result<(), String> {
        self.frame = Frame::from_size(self.canvas.output_size()?);
        (self.frame.font, self.frame.grid, self.frame.min) =
            (theme.font_scale, theme.font_grid, theme.font_min);
        if let Some(status) = &mut self.status {
            status.poll(self.frame.now);
        }
        self.finish(theme, true);
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.clear();
        self.roms_move(view, theme)?;
        self.capture()?;
        self.canvas.present();
        if let Some(status) = &mut self.status {
            status.start();
        }
        Ok(())
    }
    fn roms_move(&mut self, view: &RomsMove, theme: &Theme) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let wide = width.saturating_sub(self.px(40.) as u32).max(1);
        let title = self.catalog.text("roms_move.title");
        let title = self.wrap(&title, Face::Semibold, 20., wide)?;
        let warning = self.catalog.text("roms_move.warning");
        let warning = self.wrap(&warning, Face::Regular, 14., wide)?;
        let footer = self.roms_move_footer(view);
        let footer = self.wrap(&footer, Face::Regular, 12.5, wide)?;
        let rows = |lines: &[String], size: f32| lines.len() as f32 * line(size);
        // Icon, title, warning, bar, row, dots and footer, a gap apart.
        let footer_h = match footer.len() {
            0 => 0.,
            _ => GAP + 10. + rows(&footer, 12.5),
        };
        let total = 44.
            + rows(&title, 20.)
            + rows(&warning, 14.)
            + 8.
            + 10.
            + line(13.)
            + 6.
            + 8.
            + GAP * 5.
            + footer_h;
        let mut y = (height as i32 - self.px(total)) / 2;
        let center = width as i32 / 2;
        let side = self.px(44.);
        let icon = Rect::new(center - side / 2, y, side as u32, side as u32);
        self.icon(Glyph::StorageIn, icon, theme.highlight_bg)?;
        y += side + self.px(GAP);
        let column = center - wide as i32 / 2;
        for (lines, face, size, ink) in [
            (&title, Face::Semibold, 20., theme.text),
            (&warning, Face::Regular, 14., theme.muted),
        ] {
            for text in lines {
                let at = Rect::new(column, y, wide, self.px(line(size)) as u32);
                self.label_aligned(text, (face, size, ink), at, Align::Center)?;
                y += at.height() as i32;
            }
            y += self.px(GAP);
        }
        // The bar: bytes moved, or the entries when there are no bytes yet.
        y += self.px(8.);
        let bar = self.px(BAR) as u32;
        let left = center - bar as i32 / 2;
        let track = Rect::new(left, y, bar, self.px(10.) as u32);
        let radius = track.height() as f32 / 2.;
        self.rounded(track, radius, theme.border, None)?;
        let share = match (view.bytes, view.entry.0, view.entry.1) {
            ((done, all), _, _) if all > 0 => done as f64 / all as f64,
            (_, n, all) if all > 0 => n.saturating_sub(1) as f64 / all as f64,
            _ => 0.,
        };
        let fill = (f64::from(bar) * share.clamp(0., 1.)).round() as u32;
        if fill > 0 {
            // A flat end, as if cut by the track, until it reaches the rounding.
            let fill = fill.max(track.height());
            let done = Rect::new(left, y, fill, track.height());
            self.rounded(done, radius, theme.highlight_bg, None)?;
            if fill + track.height() / 2 < bar {
                let end = track.height() / 2;
                self.canvas.set_draw_color(color(theme.highlight_bg));
                self.canvas.fill_rect(Rect::new(
                    done.right() - end as i32,
                    y,
                    end,
                    track.height(),
                ))?;
            }
        }
        y += track.height() as i32 + self.px(GAP);
        let (what, amount) = self.roms_move_row(view);
        let style = (Face::Mono, 13., theme.text_secondary);
        let row_h = self.px(line(13.)) as u32;
        let amount_w = if amount.is_empty() {
            0
        } else {
            self.measure(&amount, Face::Mono, 13.)?.min(bar)
        };
        let gap = if amount.is_empty() { 0 } else { self.px(12.) };
        let what_w = bar.saturating_sub(amount_w + gap as u32).max(1);
        self.label(&what, style, Rect::new(left, y, what_w, row_h))?;
        if amount_w > 0 {
            let at = Rect::new(left + bar as i32 - amount_w as i32, y, amount_w, row_h);
            self.label_aligned(&amount, style, at, Align::Right)?;
        }
        y += row_h as i32 + self.px(GAP + 6.);
        // Three dots that pulse in turn: it is working, even between updates.
        let dot = self.px(8.);
        let step = dot + self.px(6.);
        let secs = self
            .frame
            .now
            .saturating_duration_since(self.born)
            .as_secs_f32();
        let mut x = center - (step * 2 + dot) / 2;
        for k in 0..3 {
            let (alpha, _) = self.motion.search_dot(secs, k);
            let ink = Rgba {
                a: (f32::from(theme.highlight_bg.a) * alpha) as u8,
                ..theme.highlight_bg
            };
            let at = Rect::new(x, y, dot as u32, dot as u32);
            self.rounded(at, dot as f32 / 2., ink, None)?;
            x += step;
        }
        y += dot + self.px(GAP + 10.);
        for text in &footer {
            let at = Rect::new(column, y, wide, self.px(line(12.5)) as u32);
            self.label_aligned(
                text,
                (Face::Regular, 12.5, theme.dim_text),
                at,
                Align::Center,
            )?;
            y += at.height() as i32;
        }
        Ok(())
    }
    /// "[34/55] snes · conferindo" and "4,8 de 7,8 GB".
    fn roms_move_row(&self, view: &RomsMove) -> (String, String) {
        let catalog = &self.catalog;
        let (n, all, name) = &view.entry;
        let doing = match view.stage {
            MoveStage::Preparing => return (catalog.text("roms_move.preparing"), String::new()),
            MoveStage::Copying => catalog.text("roms_move.copying"),
            MoveStage::Verifying => catalog.text("roms_move.verifying"),
        };
        let what = if *all > 0 {
            format!("[{n}/{all}] {name} · {doing}")
        } else {
            doing
        };
        let (done, total) = view.bytes;
        let amount = if total > 0 {
            let (done, total) = catalog.sizes(done, total);
            catalog.format("roms_move.amount", &[("done", &done), ("total", &total)])
        } else {
            String::new()
        };
        (what, amount)
    }
    /// "Faltam cerca de 3 minutos · bateria 84% · no carregador".
    fn roms_move_footer(&self, view: &RomsMove) -> String {
        let catalog = &self.catalog;
        let mut parts = Vec::new();
        if let Some(secs) = view.left {
            let minutes = secs.div_ceil(60);
            parts.push(match minutes {
                0..=1 if secs < 60 => catalog.text("roms_move.left_soon"),
                0..=59 => catalog.plural(
                    "roms_move.left_minutes",
                    minutes as f64,
                    &[("count", &minutes.to_string())],
                ),
                _ => catalog.format(
                    "roms_move.left_hours",
                    &[
                        ("hours", &(minutes / 60).to_string()),
                        ("minutes", &(minutes % 60).to_string()),
                    ],
                ),
            });
        }
        if let Some(battery) = self.status.as_ref().and_then(|s| s.battery) {
            parts.push(catalog.format(
                "roms_move.battery",
                &[("percent", &battery.percent.to_string())],
            ));
            if battery.charging {
                parts.push(catalog.text("roms_move.charging"));
            }
        }
        parts.join(" · ")
    }
}
