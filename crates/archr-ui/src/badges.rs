//! First-use button badges at final size, centered by the glyph's actual ink.
use crate::{text::color, theme::Rgba};
use sdl2::{pixels::PixelFormatEnum, rect::Rect, surface::Surface, ttf::Font};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct BadgeKey {
    pub letter: char,
    pub diameter: u32,
    pub size: u16,
    pub background: Rgba,
    pub foreground: Rgba,
}

/// `font` is the hinted UI face at `key.size`, so the letter gets the same
/// pixel-snapped stems as the labels beside it, and the texture is drawn 1:1.
/// The circle edge is area-sampled (4x4 per pixel); the ink is placed on whole
/// pixels, at most 0.5px from the exact center, rather than resampled.
pub(crate) fn render(font: &Font<'_, '_>, key: BadgeKey) -> Result<Surface<'static>, String> {
    const SAMPLES: u32 = 4;
    let diameter = key.diameter;
    let mut surface = Surface::new(diameter, diameter, PixelFormatEnum::RGBA32)?;
    let pitch = surface.pitch() as usize;
    let radius = diameter as f32 / 2.;
    surface.with_lock_mut(|bytes| {
        for y in 0..diameter {
            for x in 0..diameter {
                let inside = (0..SAMPLES * SAMPLES)
                    .filter(|i| {
                        let sx = x as f32 + (i % SAMPLES) as f32 / SAMPLES as f32 + 0.125;
                        let sy = y as f32 + (i / SAMPLES) as f32 / SAMPLES as f32 + 0.125;
                        (sx - radius).hypot(sy - radius) <= radius
                    })
                    .count();
                let coverage = inside as f32 / (SAMPLES * SAMPLES) as f32;
                let offset = y as usize * pitch + x as usize * 4;
                bytes[offset..offset + 4].copy_from_slice(&[
                    key.background.r,
                    key.background.g,
                    key.background.b,
                    (f32::from(key.background.a) * coverage).round() as u8,
                ]);
            }
        }
    });
    let glyph = font
        .render_char(key.letter)
        .blended(color(key.foreground))
        .map_err(|e| e.to_string())?
        .convert_format(PixelFormatEnum::RGBA32)?;
    let (w, h, pitch) = (glyph.width(), glyph.height(), glyph.pitch() as usize);
    let mut bounds = (w, h, 0, 0);
    glyph.with_lock(|bytes| {
        for y in 0..h {
            for x in 0..w {
                if bytes[y as usize * pitch + x as usize * 4 + 3] != 0 {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x + 1);
                    bounds.3 = bounds.3.max(y + 1);
                }
            }
        }
    });
    if bounds.2 > bounds.0 && bounds.3 > bounds.1 {
        let (w, h) = (bounds.2 - bounds.0, bounds.3 - bounds.1);
        let x = (diameter as i32 - w as i32) / 2;
        let y = (diameter as i32 - h as i32) / 2;
        glyph.blit(
            Rect::new(bounds.0 as i32, bounds.1 as i32, w, h),
            &mut surface,
            Rect::new(x, y, w, h),
        )?;
    }
    Ok(surface)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn badges_are_final_size_with_area_sampled_edges_and_centered_ink() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        let ttf = sdl2::ttf::init().unwrap();
        let font = ttf
            .load_font(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../themes/default/fonts/Inter-SemiBold.ttf"
                ),
                11,
            )
            .unwrap();
        let background = Rgba {
            r: 28,
            g: 37,
            b: 48,
            a: 255,
        };
        for letter in ['A', 'B', 'X', 'Y'] {
            let key = BadgeKey {
                letter,
                diameter: 20,
                size: 11,
                background,
                foreground: Rgba {
                    r: 230,
                    g: 237,
                    b: 243,
                    a: 255,
                },
            };
            let badge = render(&font, key).unwrap();
            assert_eq!((badge.width(), badge.height()), (20, 20));
            let pitch = badge.pitch() as usize;
            let rgba = badge.with_lock(|bytes| {
                (0..20)
                    .flat_map(|y| bytes[y * pitch..y * pitch + 80].to_vec())
                    .collect::<Vec<_>>()
            });
            let px = |x: usize, y: usize| &rgba[(y * 20 + x) * 4..(y * 20 + x) * 4 + 4];
            // Corners are outside, the rim is partially covered, the ring is solid.
            assert_eq!(px(0, 0)[3], 0);
            let rim = (0..400)
                .filter(|i| (1..255).contains(&rgba[i * 4 + 3]))
                .count();
            assert!(
                rim >= 40,
                "{letter}: only {rim} partially covered rim pixels"
            );
            assert_eq!(px(2, 10)[3], 255);
            // The ring is symmetric: area sampling, not a one-sided threshold.
            for (x, y) in [(3, 3), (1, 6), (6, 1)] {
                assert_eq!(px(x, y)[3], px(19 - x, 19 - y)[3], "{letter} at {x},{y}");
            }
            // Ink is the only thing brighter than the circle; centered on whole pixels.
            let mut bounds = (20, 20, 0, 0);
            for y in 0..20 {
                for x in 0..20 {
                    if px(x, y)[0] > background.r {
                        bounds = (
                            bounds.0.min(x),
                            bounds.1.min(y),
                            bounds.2.max(x + 1),
                            bounds.3.max(y + 1),
                        );
                    }
                }
            }
            assert!(bounds.2 > bounds.0 && bounds.3 > bounds.1);
            let dx = (bounds.0 + bounds.2) as f32 / 2. - 10.;
            let dy = (bounds.1 + bounds.3) as f32 / 2. - 10.;
            assert!(dx.abs() <= 0.5 && dy.abs() <= 0.5, "{letter}: {dx}, {dy}");
        }
    }
}
