//! START hub: a fixed 640×480 composition scaled with the theme's screen height.
use super::{color, Align, App, DataSource, Face, Rect, Rgba, Screen, Theme, Ui};
use sdl2::rect::Point;

const TILES: [&str; 3] = ["start.recent", "start.games", "start.tools"];
const SHORTCUTS: [&str; 6] = [
    "start.settings",
    "start.update",
    "start.wifi",
    "start.suspend",
    "start.reboot",
    "start.shutdown",
];
const TILE_ICONS: [Glyph; 3] = [Glyph::Recent, Glyph::Games, Glyph::Tools];
const SHORTCUT_ICONS: [Glyph; 6] = [
    Glyph::Gear,
    Glyph::Download,
    Glyph::Wifi,
    Glyph::Moon,
    Glyph::Restart,
    Glyph::Power,
];

#[derive(Clone, Copy)]
enum Glyph {
    Recent,
    Games,
    Tools,
    Gear,
    Download,
    Wifi,
    Moon,
    Restart,
    Power,
}

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
        let px = |n: f32| (n * height as f32 / 480.).round() as i32;
        let center = width as i32 / 2;
        self.label_aligned(
            &self.catalog.text("start.title"),
            (Face::Semibold, 17., theme.text),
            Rect::new(0, px(57.), width, px(28.) as u32),
            Align::Center,
        )?;

        let tools = data.systems().iter().any(|s| s.name == "tools");
        for index in 0..3 {
            let enabled = index != 2 || tools;
            let focus = row == 0 && tile == index && enabled;
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
            self.hub_icon(
                TILE_ICONS[index],
                Rect::new(x + (side - size) / 2, y + px(29.), size as u32, size as u32),
                ink,
            )?;
            self.label_aligned(
                &self.catalog.text(TILES[index]),
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
        for (index, &icon) in SHORTCUT_ICONS.iter().enumerate() {
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
                Some(if active && !focus {
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
            self.hub_icon(
                icon,
                Rect::new(x + px(11.), px(289.), px(24.) as u32, px(24.) as u32),
                ink,
            )?;
        }
        if row == 1 {
            let key = SHORTCUTS[shortcut];
            let label = if shortcut < 2 {
                self.catalog
                    .format("start.soon_label", &[("name", &self.catalog.text(key))])
            } else if shortcut == 2 {
                self.catalog.format(
                    "start.wifi_state",
                    &[(
                        "state",
                        &self
                            .catalog
                            .text(if wifi_on { "start.on" } else { "start.off" }),
                    )],
                )
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
        self.hub_icon(
            Glyph::Power,
            Rect::new(px(24.), footer_y + px(15.), px(17.) as u32, px(17.) as u32),
            theme.muted,
        )?;
        self.label(
            &self.catalog.text("start.suspend"),
            (Face::Regular, 13., theme.muted),
            Rect::new(px(48.), footer_y, px(170.) as u32, px(48.) as u32),
        )?;
        self.hints(
            &[('A', "hint.open"), ('B', "hint.back")],
            theme,
            width,
            footer_y,
            px(48.) as u32,
        )?;
        Ok(())
    }

    fn hub_icon(&mut self, glyph: Glyph, rect: Rect, ink: Rgba) -> Result<(), String> {
        let point = |x: f32, y: f32| {
            Point::new(
                rect.x() + (x * rect.width() as f32 / 24.).round() as i32,
                rect.y() + (y * rect.height() as f32 / 24.).round() as i32,
            )
        };
        self.canvas.set_draw_color(color(ink));
        let mut path = |points: &[(f32, f32)]| -> Result<(), String> {
            let points: Vec<_> = points.iter().map(|&(x, y)| point(x, y)).collect();
            self.canvas.draw_lines(points.as_slice())?;
            let shifted: Vec<_> = points
                .iter()
                .map(|p| Point::new(p.x() + 1, p.y()))
                .collect();
            self.canvas.draw_lines(shifted.as_slice())
        };
        let arc = |cx: f32, cy: f32, radius: f32, from: f32, to: f32| -> Vec<(f32, f32)> {
            (0..=24)
                .map(|i| {
                    let angle = from + (to - from) * i as f32 / 24.;
                    (cx + radius * angle.cos(), cy + radius * angle.sin())
                })
                .collect()
        };
        use std::f32::consts::{PI, TAU};
        match glyph {
            Glyph::Recent => {
                path(&[(4., 2.), (14., 2.), (19., 7.), (19., 13.)])?;
                path(&[(14., 2.), (14., 7.), (19., 7.)])?;
                path(&[(4., 2.), (4., 21.), (12., 21.)])?;
                path(&[(7., 11.), (13., 11.)])?;
                path(&[(7., 15.), (11., 15.)])?;
                path(&arc(17., 17., 5., 0., TAU))?;
                path(&[(17., 14.), (17., 17.), (19., 18.)])?;
            }
            Glyph::Games => {
                path(&[
                    (5., 8.),
                    (8., 6.),
                    (16., 6.),
                    (19., 8.),
                    (22., 17.),
                    (21., 20.),
                    (18., 20.),
                    (15., 17.),
                    (9., 17.),
                    (6., 20.),
                    (3., 20.),
                    (2., 17.),
                    (5., 8.),
                ])?;
                path(&[(8., 10.), (8., 15.)])?;
                path(&[(5.5, 12.5), (10.5, 12.5)])?;
                path(&arc(16., 11., 0.8, 0., TAU))?;
                path(&arc(19., 13., 0.8, 0., TAU))?;
            }
            Glyph::Tools => {
                path(&[
                    (3., 3.),
                    (7., 7.),
                    (8., 5.),
                    (10., 7.),
                    (8., 9.),
                    (17., 18.),
                    (19., 20.),
                    (21., 18.),
                    (19., 16.),
                    (10., 7.),
                ])?;
                path(&[
                    (21., 3.),
                    (17., 4.),
                    (16., 7.),
                    (18., 9.),
                    (21., 8.),
                    (20., 12.),
                    (18., 13.),
                ])?;
                path(&[
                    (6., 12.),
                    (3., 16.),
                    (3., 19.),
                    (5., 21.),
                    (8., 20.),
                    (12., 16.),
                ])?;
            }
            Glyph::Gear => {
                let outline: Vec<_> = (0..=32)
                    .map(|i| {
                        let a = -PI / 2. + i as f32 * TAU / 32.;
                        let r = if i % 4 == 0 || i % 4 == 3 { 10. } else { 7.5 };
                        (12. + r * a.cos(), 12. + r * a.sin())
                    })
                    .collect();
                path(&outline)?;
                path(&arc(12., 12., 3.2, 0., TAU))?;
            }
            Glyph::Download => {
                path(&[(12., 2.), (12., 16.), (7., 11.)])?;
                path(&[(12., 16.), (17., 11.)])?;
                path(&[(3., 17.), (3., 21.), (21., 21.), (21., 17.)])?;
            }
            Glyph::Wifi => {
                path(&arc(12., 21., 18., -2.25, -0.89))?;
                path(&arc(12., 21., 12., -2.22, -0.92))?;
                path(&arc(12., 21., 6., -2.15, -0.99))?;
                path(&arc(12., 19., 1., 0., TAU))?;
            }
            Glyph::Moon => {
                path(&arc(12., 12., 10., -1.16, -5.12))?;
                path(&arc(21., 12., 10., 2.1, 4.18))?;
            }
            Glyph::Restart => {
                path(&arc(12., 12., 9., -2.4, 2.8))?;
                path(&[(3., 10.), (3., 3.), (10., 3.)])?;
            }
            Glyph::Power => {
                path(&arc(12., 12., 9., -PI * 0.78, PI * 0.78))?;
                path(&[(12., 2.), (12., 13.)])?;
            }
        }
        Ok(())
    }
}
