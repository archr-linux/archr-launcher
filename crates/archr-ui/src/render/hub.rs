//! START hub: a fixed 640×480 composition scaled with the theme's screen height.
use super::{Align, App, DataSource, Face, Glyph, Rect, Rgba, Screen, Theme, Ui};

const TILES: [(&str, Glyph); 3] = [
    ("collections.recent", Glyph::Recent),
    ("start.games", Glyph::Games),
    ("start.tools", Glyph::Tools),
];
const SHORTCUTS: [(&str, Glyph); 6] = [
    ("start.settings", Glyph::Gear),
    ("start.update", Glyph::Download),
    ("", Glyph::Wifi),
    ("menu.suspend", Glyph::Moon),
    ("menu.reboot", Glyph::Restart),
    ("menu.shutdown", Glyph::Power),
];

impl Ui<'_, '_> {
    pub(super) fn hub(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let Screen::StartHub {
            row,
            tile,
            shortcut,
        } = app.screen
        else {
            return Ok(());
        };
        // The title, the shortcut's name and the hints lie over the
        // background: they take the theme's outline. Tiles have their own.
        self.look.outlining = true;
        let drawn = self.hub_page(app, data, theme, (row, tile, shortcut));
        self.look.outlining = false;
        drawn
    }
    fn hub_page(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        (row, tile, shortcut): (usize, usize, usize),
    ) -> Result<(), String> {
        let (width, height) = self.canvas.output_size()?;
        // Unlike `Ui::px`, offsets may be negative, so no lower clamp.
        let scale = self.frame.scale;
        let px = |n: f32| (n * scale).round() as i32;
        let center = width as i32 / 2;
        // A view that titles screens already says "Start" in the header.
        let title = if self.view(theme).systems.brand {
            self.catalog.text("start.title")
        } else {
            String::new()
        };
        self.label_aligned(
            &title,
            (Face::Semibold, 17., theme.text),
            Rect::new(0, px(57.), width, px(28.) as u32),
            Align::Center,
        )?;

        for (index, &(key, glyph)) in TILES.iter().enumerate() {
            let enabled = app.hub_tile_enabled(data, index);
            let focus = row == 0 && tile == index;
            let side = px(if focus { 154. } else { 148. });
            let x = center + px((index as f32 - 1.) * 170.) - side / 2;
            let y = px(104.) - (side - px(148.)) / 2;
            let tile_rect = Rect::new(x, y, side as u32, side as u32);
            if focus {
                // Two translucent shells make an inexpensive, soft blue halo.
                for (spread, alpha) in [(9., 14), (5., 25)] {
                    self.rounded(
                        Rect::new(
                            x - px(spread),
                            y - px(spread),
                            (side + px(spread * 2.)) as u32,
                            (side + px(spread * 2.)) as u32,
                        ),
                        px(18. + spread) as f32,
                        Rgba {
                            a: alpha,
                            ..theme.highlight_bg
                        },
                        None,
                    )?;
                }
            }
            self.rounded(
                tile_rect,
                px(18.) as f32,
                if focus {
                    theme.highlight_bg
                } else {
                    theme.surface
                },
                if focus { None } else { Some(theme.border) },
            )?;
            let ink = if focus {
                theme.highlight_text
            } else if enabled {
                theme.text_secondary
            } else {
                theme.dim_text
            };
            let size = px(48.);
            self.icon(
                glyph,
                Rect::new(x + (side - size) / 2, y + px(29.), size as u32, size as u32),
                ink,
            )?;
            // Shrinks down to the theme's smallest text, then wraps; a
            // label is never cut.
            // A word too wide even then takes the launcher's own font,
            // as in Settings; words are never split.
            let label = self.catalog.text(key);
            let room = (side - px(10.)).max(1) as u32;
            let floor = self.frame.min_size();
            let mut fitted = None;
            for face in [Face::Semibold, Face::Fallback] {
                let mut size = 16.;
                let widest = |ui: &mut Self, size| -> Result<u32, String> {
                    let mut widest = 0;
                    for word in label.split_whitespace() {
                        widest = widest.max(ui.measure(word, face, size)?);
                    }
                    Ok(widest)
                };
                while size > floor && self.measure(&label, face, size)? > room {
                    size = (size - 1.).max(floor);
                }
                if widest(self, size)? <= room || face == Face::Fallback {
                    fitted = Some((face, size));
                    break;
                }
            }
            let (face, size) = fitted.unwrap_or((Face::Semibold, 16.));
            let lines = if self.measure(&label, face, size)? > room {
                self.wrap(&label, face, size, room)?
            } else {
                vec![label]
            };
            let step = (f32::from(self.frame.font_size(size)) * 1.2).round() as i32;
            let top = y + px(114.) - step * lines.len() as i32 / 2;
            self.look.outlining = false;
            for (n, line) in lines.iter().enumerate() {
                self.label_aligned(
                    line,
                    (face, size, ink),
                    Rect::new(x + px(5.), top + step * n as i32, room, step.max(1) as u32),
                    Align::Center,
                )?;
            }
            self.look.outlining = true;
        }

        let side = px(46.);
        let gap = px(14.);
        let start = center - (side * 6 + gap * 5) / 2;
        let wifi_on = self.wifi_enabled();
        for (index, &(_, glyph)) in SHORTCUTS.iter().enumerate() {
            let focus = row == 1 && shortcut == index;
            let enabled = App::hub_shortcut_enabled(index);
            let active = index == 2 && wifi_on;
            let x = start + index as i32 * (side + gap);
            let rect = Rect::new(x, px(278.), side as u32, side as u32);
            self.rounded(
                rect,
                side as f32 / 2.,
                // Selected is always the most marked, offered or not.
                if focus {
                    theme.highlight_bg
                } else {
                    theme.surface
                },
                Some(if focus || active {
                    theme.highlight_bg
                } else {
                    theme.border
                }),
            )?;
            let ink = if focus && enabled {
                theme.highlight_text
            } else if focus {
                // Not offered yet: the accent's ink, faded.
                Rgba {
                    a: 110,
                    ..theme.highlight_text
                }
            } else if active {
                theme.highlight_bg
            } else if enabled {
                theme.text_secondary
            } else {
                theme.dim_text
            };
            self.icon(
                if index == 2 && !wifi_on {
                    Glyph::WifiOff
                } else {
                    glyph
                },
                Rect::new(x + px(11.), px(289.), px(24.) as u32, px(24.) as u32),
                ink,
            )?;
        }
        if row == 1 {
            let key = SHORTCUTS[shortcut].0;
            let label = if shortcut == 1 {
                self.catalog
                    .format("start.soon_label", &[("name", &self.catalog.text(key))])
            } else if shortcut == 2 {
                self.catalog.text(if wifi_on {
                    "start.wifi_on"
                } else {
                    "start.wifi_off"
                })
            } else {
                self.catalog.text(key)
            };
            self.label_aligned(
                &label,
                (Face::Regular, 14., theme.text_secondary),
                Rect::new(
                    px(12.),
                    px(333.),
                    width.saturating_sub(px(24.) as u32),
                    px(22.) as u32,
                ),
                Align::Center,
            )?;
        }
        let footer_y = height as i32 - px(48.);
        self.divider(0, footer_y, width, theme.border)?;
        self.hints(
            &[('A', "hint.open"), ('B', "hint.back")],
            theme,
            width,
            footer_y,
            px(48.) as u32,
        )?;
        Ok(())
    }
}
