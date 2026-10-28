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
        let (width, height) = self.canvas.output_size()?;
        // Unlike `Ui::px`, offsets may be negative, so no lower clamp.
        let scale = self.frame.scale;
        let px = |n: f32| (n * scale).round() as i32;
        let center = width as i32 / 2;
        self.label_aligned(
            &self.catalog.text("start.title"),
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
            self.label_aligned(
                &self.catalog.text(key),
                (Face::Semibold, 16., ink),
                Rect::new(
                    x + px(5.),
                    y + px(100.),
                    (side - px(10.)) as u32,
                    px(28.) as u32,
                ),
                Align::Center,
            )?;
        }

        let side = px(46.);
        let gap = px(14.);
        let start = center - (side * 6 + gap * 5) / 2;
        let wifi_on = self.status.as_ref().is_some_and(|s| s.wifi);
        for (index, &(_, glyph)) in SHORTCUTS.iter().enumerate() {
            let focus = row == 1 && shortcut == index;
            let enabled = index >= 2;
            let active = index == 2 && wifi_on;
            let x = start + index as i32 * (side + gap);
            let rect = Rect::new(x, px(278.), side as u32, side as u32);
            self.rounded(
                rect,
                side as f32 / 2.,
                if focus && enabled {
                    theme.highlight_bg
                } else {
                    theme.surface
                },
                Some(if focus && !enabled {
                    Rgba {
                        a: 150,
                        ..theme.highlight_bg
                    }
                } else if active && !focus {
                    theme.highlight_bg
                } else {
                    theme.border
                }),
            )?;
            let ink = if focus && enabled {
                theme.highlight_text
            } else if active {
                theme.highlight_bg
            } else if enabled {
                theme.text_secondary
            } else {
                theme.dim_text
            };
            self.icon(
                glyph,
                Rect::new(x + px(11.), px(289.), px(24.) as u32, px(24.) as u32),
                ink,
            )?;
        }
        if row == 1 {
            let key = SHORTCUTS[shortcut].0;
            let label = if shortcut < 2 {
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
