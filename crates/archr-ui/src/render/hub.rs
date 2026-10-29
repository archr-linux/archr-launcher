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

        let modules = app.modules.tiles();
        let count = TILES.len() + modules.len();
        let labels: Vec<String> = (0..count)
            .map(|i| match i.checked_sub(TILES.len()) {
                Some(n) => modules[n].1.label.clone(),
                None => self.catalog.text(TILES[i].0),
            })
            .collect();
        // Tile labels may go 2 px under the theme's smallest text, so a
        // long word ("Ferramentas") keeps the theme's font; the frame is
        // rebuilt each draw, so the next screen has the usual minimum.
        let minimum = self.frame.min;
        self.frame.min = (minimum - 2.).max(8.);
        let (label_face, label_size) =
            self.tile_labels(&labels, (px(148.) - px(8.)).max(1) as u32)?;
        // More tiles than the screen holds: the row slides to keep the focus in.
        let span = count as f32 * 170. - 22.;
        let first = if span <= 600. {
            320. - span / 2. + 74.
        } else {
            (320. - tile as f32 * 170.).clamp(640. - 20. - 74. - (count - 1) as f32 * 170., 94.)
        };
        for index in 0..count {
            let (key, glyph) = TILES.get(index).copied().unwrap_or(("", Glyph::Puzzle));
            let module = index.checked_sub(TILES.len()).map(|n| modules[n]);
            let enabled = app.hub_tile_enabled(data, index);
            let focus = row == 0 && tile == index;
            let side = px(if focus { 154. } else { 148. });
            let x = px(first + index as f32 * 170.) - side / 2 + center - px(320.);
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
            let icon = Rect::new(x + (side - size) / 2, y + px(29.), size as u32, size as u32);
            let (label, value) = match module {
                Some((n, entry)) => {
                    let id = &app.modules.modules[n].id;
                    self.module_icon((entry.icon.as_deref(), ""), icon, ink)?;
                    let value = app.modules.value(id, &format!("tile:{}", entry.id));
                    (entry.label.clone(), value.map(str::to_owned))
                }
                None => {
                    self.icon(glyph, icon, ink)?;
                    (self.catalog.text(key), None)
                }
            };
            // Tile text sits on the tile, not the background: no outline.
            self.look.outlining = false;
            if let Some(value) = value {
                // The live value sits under a raised title.
                self.label_aligned(
                    &value,
                    (Face::Regular, 13., ink),
                    Rect::new(
                        x + px(5.),
                        y + px(118.),
                        (side - px(10.)) as u32,
                        px(20.) as u32,
                    ),
                    Align::Center,
                )?;
            }
            let raise = if module.is_some_and(|(n, e)| {
                app.modules
                    .value(&app.modules.modules[n].id, &format!("tile:{}", e.id))
                    .is_some()
            }) {
                px(8.)
            } else {
                0
            };
            // Every tile label shares one size and face, the largest that
            // fits them all; words are never split.
            let room = (side - px(8.)).max(1) as u32;
            let lines = if self.measure(&label, label_face, label_size)? > room {
                self.wrap(&label, label_face, label_size, room)?
            } else {
                vec![label]
            };
            let step = (f32::from(self.frame.font_size(label_size)) * 1.2).round() as i32;
            let top = y + px(114.) - raise - step * lines.len() as i32 / 2;
            for (n, line) in lines.iter().enumerate() {
                self.label_aligned(
                    line,
                    (label_face, label_size, ink),
                    Rect::new(x + px(4.), top + step * n as i32, room, step.max(1) as u32),
                    Align::Center,
                )?;
            }
            self.look.outlining = true;
        }

        self.frame.min = minimum;
        let side = px(46.);
        let gap = px(14.);
        // Module shortcuts sit between Wi-Fi and Suspend.
        let extra = app.modules.shortcuts();
        let count = SHORTCUTS.len() + extra.len();
        let start = center - (side * count as i32 + gap * (count as i32 - 1)) / 2;
        let wifi_on = self.wifi_enabled();
        let builtin = |index: usize| match index.checked_sub(3) {
            Some(n) if n < extra.len() => None,
            Some(n) => Some(3 + n - extra.len()),
            None => Some(index),
        };
        for index in 0..count {
            let focus = row == 1 && shortcut == index;
            let Some(fixed) = builtin(index) else {
                let (_, entry) = extra[index - 3];
                let x = start + index as i32 * (side + gap);
                let rect = Rect::new(x, px(278.), side as u32, side as u32);
                let fill = if focus {
                    theme.highlight_bg
                } else {
                    theme.surface
                };
                self.rounded(rect, side as f32 / 2., fill, Some(theme.border))?;
                let ink = if focus {
                    theme.highlight_text
                } else {
                    theme.text_secondary
                };
                let icon = Rect::new(x + px(11.), px(289.), px(24.) as u32, px(24.) as u32);
                self.module_icon((entry.icon.as_deref(), ""), icon, ink)?;
                continue;
            };
            let glyph = SHORTCUTS[fixed].1;
            let index = fixed;
            let enabled = App::hub_shortcut_enabled(index);
            let active = index == 2 && wifi_on;
            let at = if index >= 3 {
                index + extra.len()
            } else {
                index
            };
            let x = start + at as i32 * (side + gap);
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
            // A system update is waiting: a dot on the update shortcut.
            if index == 1 && app.updates > 0 {
                let dot = px(12.);
                let ring = px(2.);
                let (dx, dy) = (x + side - dot + px(1.), px(277.));
                self.rounded(
                    Rect::new(
                        dx - ring,
                        dy - ring,
                        (dot + ring * 2) as u32,
                        (dot + ring * 2) as u32,
                    ),
                    (dot / 2 + ring) as f32,
                    theme.background,
                    None,
                )?;
                self.rounded(
                    Rect::new(dx, dy, dot as u32, dot as u32),
                    (dot / 2) as f32,
                    theme.highlight_bg,
                    None,
                )?;
            }
        }
        if row == 1 {
            let fixed = builtin(shortcut);
            let key = fixed.map_or("", |n| SHORTCUTS[n].0);
            let label = if let Some((n, entry)) = shortcut
                .checked_sub(3)
                .and_then(|n| extra.get(n))
                .filter(|_| fixed.is_none())
            {
                let value = app.modules.value(
                    &app.modules.modules[*n].id,
                    &format!("shortcut:{}", entry.id),
                );
                match value {
                    Some(value) => format!("{} · {value}", entry.label),
                    None => entry.label.clone(),
                }
            } else if fixed == Some(1) && app.updates > 0 {
                let count = app.updates.to_string();
                self.catalog.plural(
                    "start.update_ready",
                    app.updates as f64,
                    &[("count", &count)],
                )
            } else if fixed == Some(2) {
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
    /// The face and size every tile label is drawn with: the theme's
    /// semibold, else its regular, else the launcher's font, each from 16 px
    /// down to the theme's smallest, as large as fits every word of every
    /// label in `room`.
    fn tile_labels(&mut self, labels: &[String], room: u32) -> Result<(Face, f32), String> {
        let floor = self.frame.min_size();
        for face in [Face::Semibold, Face::Regular, Face::Fallback] {
            let mut size = 16f32;
            loop {
                let mut fits = true;
                for word in labels.iter().flat_map(|l| l.split_whitespace()) {
                    fits &= self.measure(word, face, size)? <= room;
                }
                if fits {
                    return Ok((face, size));
                }
                if size <= floor {
                    break;
                }
                size = (size - 1.).max(floor);
            }
        }
        Ok((Face::Fallback, floor))
    }
}
