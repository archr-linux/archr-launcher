//! Startup-composited 4× button badges, centered by the glyph's actual ink.
use crate::{text::color, theme::Rgba};
use sdl2::{pixels::PixelFormatEnum, rect::Rect, surface::Surface, ttf::Font};

pub(crate) const SUPERSAMPLE: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct BadgeKey {
    pub letter: char,
    pub diameter: u32,
    pub size: u16,
    pub background: Rgba,
    pub foreground: Rgba,
}

/// `font` is loaded at four times the requested final font size. Keep the
/// circle and ink in the same texture so downscaling preserves their alignment.
pub(crate) fn render(font: &Font<'_, '_>, key: BadgeKey) -> Result<Surface<'static>, String> {
    let diameter = key.diameter * SUPERSAMPLE;
    let mut surface = Surface::new(diameter, diameter, PixelFormatEnum::RGBA32)?;
    let pitch = surface.pitch() as usize;
    let radius = diameter as f32 / 2.;
    surface.with_lock_mut(|bytes| {
        for y in 0..diameter {
            for x in 0..diameter {
                let distance = (x as f32 + 0.5 - radius).hypot(y as f32 + 0.5 - radius);
                let coverage = (radius + 0.5 - distance).clamp(0., 1.);
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
        // Half a supersampled pixel is 0.125px at the final size, rather than
        // the 0.5px error of positioning a low-resolution glyph independently.
        let x = ((diameter as f32 - w as f32) / 2.).round() as i32;
        let y = ((diameter as f32 - h as f32) / 2.).round() as i32;
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
    fn supersampled_badge_ink_centers_are_within_quarter_final_pixel() {
        let ttf = sdl2::ttf::init().unwrap();
        let font = ttf
            .load_font(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../themes/default/fonts/Inter-SemiBold.ttf"
                ),
                44,
            )
            .unwrap();
        for letter in ['A', 'B', 'X', 'Y'] {
            let key = BadgeKey {
                letter,
                diameter: 20,
                size: 11,
                background: Rgba {
                    r: 28,
                    g: 37,
                    b: 48,
                    a: 255,
                },
                foreground: Rgba {
                    r: 230,
                    g: 237,
                    b: 243,
                    a: 255,
                },
            };
            let badge = render(&font, key).unwrap();
            assert_eq!((badge.width(), badge.height()), (80, 80));
            // Exercise both the 4× texture and actual 20px linear-filtered
            // presentation through SDL's offscreen surface renderer.
            for side in [80, 20] {
                let target = Surface::new(side, side, PixelFormatEnum::RGBA32).unwrap();
                let mut canvas = target.into_canvas().unwrap();
                let creator = canvas.texture_creator();
                let mut texture = creator.create_texture_from_surface(&badge).unwrap();
                texture.set_scale_mode(sdl2::render::ScaleMode::Linear);
                canvas.copy(&texture, None, None).unwrap();
                let rgba = canvas.read_pixels(None, PixelFormatEnum::RGBA32).unwrap();
                // Scan the actual composited result, not the positioning formula.
                // The circle never exceeds background.r; only glyph ink does.
                let mut bounds = (side, side, 0, 0);
                for y in 0..side {
                    for x in 0..side {
                        if rgba[((y * side + x) * 4) as usize] > 28 {
                            bounds.0 = bounds.0.min(x);
                            bounds.1 = bounds.1.min(y);
                            bounds.2 = bounds.2.max(x + 1);
                            bounds.3 = bounds.3.max(y + 1);
                        }
                    }
                }
                assert!(bounds.2 > bounds.0 && bounds.3 > bounds.1);
                let scale = side as f32 / 20.;
                let dx = ((bounds.0 + bounds.2) as f32 / 2. - side as f32 / 2.) / scale;
                let dy = ((bounds.1 + bounds.3) as f32 / 2. - side as f32 / 2.) / scale;
                println!("BADGE_CENTER {letter} side={side} dx={dx:.3}px dy={dy:.3}px");
                assert!(dx.abs() <= 0.25 && dy.abs() <= 0.25, "{letter}: {dx}, {dy}");
            }
        }
    }
}
