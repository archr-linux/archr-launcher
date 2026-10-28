//! Stroked line icons on a 24-unit grid, rasterized once at final size with
//! coverage from the distance to each segment, which is already antialiased.
use sdl2::{pixels::PixelFormatEnum, surface::Surface};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Glyph {
    Recent,
    Games,
    Tools,
    Gear,
    Download,
    Wifi,
    Moon,
    Restart,
    Power,
    Sound,
    Globe,
    Palette,
    Trophy,
    Storage,
}

type Path = Vec<(f32, f32)>;

fn arc(cx: f32, cy: f32, radius: f32, from: f32, to: f32) -> Path {
    (0..=24)
        .map(|i| {
            let angle = from + (to - from) * i as f32 / 24.;
            (cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect()
}

fn paths(glyph: Glyph) -> Vec<Path> {
    let circle = |cx, cy, r| arc(cx, cy, r, 0., TAU);
    match glyph {
        Glyph::Sound => vec![
            vec![
                (3., 9.),
                (7., 9.),
                (12., 4.),
                (12., 20.),
                (7., 15.),
                (3., 15.),
                (3., 9.),
            ],
            arc(12., 12., 7., -0.8, 0.8),
            arc(12., 12., 11., -0.8, 0.8),
        ],
        Glyph::Globe => vec![
            circle(12., 12., 10.),
            vec![(2., 12.), (22., 12.)],
            arc(12., 12., 5., 0., TAU),
        ],
        Glyph::Palette => vec![
            circle(12., 12., 10.),
            circle(7., 9., 1.),
            circle(12., 6., 1.),
            circle(17., 9., 1.),
            vec![(8., 18.), (12., 14.), (16., 18.)],
        ],
        Glyph::Trophy => vec![
            vec![
                (6., 3.),
                (18., 3.),
                (17., 12.),
                (12., 16.),
                (7., 12.),
                (6., 3.),
            ],
            vec![(6., 5.), (2., 5.), (3., 11.), (7., 12.)],
            vec![(18., 5.), (22., 5.), (21., 11.), (17., 12.)],
            vec![(12., 16.), (12., 21.)],
            vec![(7., 21.), (17., 21.)],
        ],
        Glyph::Storage => vec![
            vec![
                (6., 2.),
                (18., 2.),
                (21., 5.),
                (21., 22.),
                (3., 22.),
                (3., 5.),
                (6., 2.),
            ],
            vec![(8., 2.), (8., 8.), (16., 8.), (16., 2.)],
            vec![(7., 22.), (7., 15.), (17., 15.), (17., 22.)],
        ],
        Glyph::Recent => vec![
            vec![(4., 2.), (14., 2.), (19., 7.), (19., 13.)],
            vec![(14., 2.), (14., 7.), (19., 7.)],
            vec![(4., 2.), (4., 21.), (12., 21.)],
            vec![(7., 11.), (13., 11.)],
            vec![(7., 15.), (11., 15.)],
            circle(17., 17., 5.),
            vec![(17., 14.), (17., 17.), (19., 18.)],
        ],
        Glyph::Games => vec![
            vec![
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
            ],
            vec![(8., 10.), (8., 15.)],
            vec![(5.5, 12.5), (10.5, 12.5)],
            circle(16., 11., 0.8),
            circle(19., 13., 0.8),
        ],
        Glyph::Tools => vec![
            vec![
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
            ],
            vec![
                (21., 3.),
                (17., 4.),
                (16., 7.),
                (18., 9.),
                (21., 8.),
                (20., 12.),
                (18., 13.),
            ],
            vec![
                (6., 12.),
                (3., 16.),
                (3., 19.),
                (5., 21.),
                (8., 20.),
                (12., 16.),
            ],
        ],
        Glyph::Gear => vec![
            (0..=32)
                .map(|i| {
                    let a = -PI / 2. + i as f32 * TAU / 32.;
                    let r = if i % 4 == 0 || i % 4 == 3 { 10. } else { 7.5 };
                    (12. + r * a.cos(), 12. + r * a.sin())
                })
                .collect(),
            circle(12., 12., 3.2),
        ],
        Glyph::Download => vec![
            vec![(12., 2.), (12., 16.), (7., 11.)],
            vec![(12., 16.), (17., 11.)],
            vec![(3., 17.), (3., 21.), (21., 21.), (21., 17.)],
        ],
        Glyph::Wifi => vec![
            arc(12., 21., 18., -2.25, -0.89),
            arc(12., 21., 12., -2.22, -0.92),
            arc(12., 21., 6., -2.15, -0.99),
            circle(12., 19., 1.),
        ],
        Glyph::Moon => vec![
            arc(12., 12., 10., -1.16, -5.12),
            arc(21., 12., 10., 2.1, 4.18),
        ],
        Glyph::Restart => {
            // Clockwise ring open at the top; the head sits on its end point.
            let end = 3.98_f32;
            let (tip_x, tip_y) = (12. + 8. * end.cos(), 12. + 8. * end.sin());
            let heading = end + PI / 2.;
            let wing = |turn: f32| {
                let a = heading + PI + turn;
                (tip_x + 5. * a.cos(), tip_y + 5. * a.sin())
            };
            vec![
                arc(12., 12., 8., -1.2, end),
                vec![wing(0.7), (tip_x, tip_y), wing(-0.7)],
            ]
        }
        Glyph::Power => vec![
            arc(12., 12., 9., -PI / 2. + 0.7, 3. * PI / 2. - 0.7),
            vec![(12., 2.), (12., 12.)],
        ],
    }
}

fn distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx * dx + dy * dy;
    let t = if len == 0. {
        0.
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len).clamp(0., 1.)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// White RGBA at `size` pixels, tinted later with a color mod.
pub(crate) fn render(glyph: Glyph, size: u32) -> Result<Surface<'static>, String> {
    let side = size;
    let unit = side as f32 / 24.;
    let half = (size as f32 * 0.073).max(1.75) / 2.;
    let segments: Vec<_> = paths(glyph)
        .iter()
        .flat_map(|path| path.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .map(|(a, b)| ((a.0 * unit, a.1 * unit), (b.0 * unit, b.1 * unit)))
        .collect();
    let mut surface = Surface::new(side, side, PixelFormatEnum::RGBA32)?;
    let pitch = surface.pitch() as usize;
    surface.with_lock_mut(|bytes| {
        for y in 0..side {
            for x in 0..side {
                let p = (x as f32 + 0.5, y as f32 + 0.5);
                let near = segments
                    .iter()
                    .map(|&(a, b)| distance(p, a, b))
                    .fold(f32::MAX, f32::min);
                let coverage = (half + 0.5 - near).clamp(0., 1.);
                let offset = y as usize * pitch + x as usize * 4;
                bytes[offset..offset + 4].copy_from_slice(&[
                    255,
                    255,
                    255,
                    (coverage * 255.) as u8,
                ]);
            }
        }
    });
    Ok(surface)
}
