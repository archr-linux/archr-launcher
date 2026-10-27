//! Bounded text textures, lazily loaded font faces, and measured ellipsis.
use crate::theme::Rgba;
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{Canvas, Texture, TextureCreator},
    ttf::{Font, FontStyle, Sdl2TtfContext},
    video::{Window, WindowContext},
};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Face {
    Regular,
    Semibold,
    Mono,
    MonoMedium,
    Button,
}
#[derive(Clone, Copy)]
pub(crate) struct Style {
    pub face: Face,
    pub size: u16,
    pub color: Rgba,
}
#[derive(Clone, Copy)]
pub(crate) enum Align {
    Left,
    Center,
    Right,
    Glyph,
}
struct Entry<'a> {
    text: String,
    face: Face,
    size: u16,
    color: Rgba,
    texture: Texture<'a>,
    ink: Rect,
}
pub(crate) struct TextCache<'font, 'texture> {
    ttf: &'font Sdl2TtfContext,
    creator: &'texture TextureCreator<WindowContext>,
    paths: [Vec<PathBuf>; 5],
    fonts: HashMap<(Face, u16), Font<'font, 'static>>,
    entries: VecDeque<Entry<'texture>>,
}
impl<'font, 'texture> TextCache<'font, 'texture> {
    pub fn new(
        ttf: &'font Sdl2TtfContext,
        creator: &'texture TextureCreator<WindowContext>,
        paths: [Vec<PathBuf>; 5],
    ) -> Self {
        Self {
            ttf,
            creator,
            paths,
            fonts: HashMap::new(),
            entries: VecDeque::new(),
        }
    }
    fn font(&mut self, face: Face, size: u16) -> Result<&Font<'font, 'static>, String> {
        if !self.fonts.contains_key(&(face, size)) {
            let paths = &self.paths[face as usize];
            let mut font = paths
                .iter()
                .find_map(|path| self.ttf.load_font(path, size).ok())
                .ok_or("Nenhuma fonte disponível (tema, Liberation Sans ou DejaVu Sans).")?;
            // The compact bold badges are synthesized from Regular. Other
            // faces prefer the official static semibold/medium weights.
            if matches!(face, Face::Button)
                || (matches!(face, Face::Semibold | Face::MonoMedium)
                    && paths
                        .first()
                        .is_some_and(|p| p.to_string_lossy().contains("Regular")))
            {
                font.set_style(FontStyle::BOLD);
            }
            self.fonts.insert((face, size), font);
        }
        Ok(self.fonts.get(&(face, size)).expect("loaded face"))
    }
    pub fn measure(&mut self, text: &str, face: Face, size: u16) -> Result<u32, String> {
        self.font(face, size)?
            .size_of(&sanitize(text))
            .map(|s| s.0)
            .map_err(|e| e.to_string())
    }
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        text: &str,
        style: Style,
        panel: Rect,
        align: Align,
    ) -> Result<(), String> {
        // SDL's software renderer reports zero for an unspecified maximum,
        // not a one-pixel texture limit.
        let maximum = canvas.info().max_texture_width;
        let limit = if maximum == 0 {
            panel.width()
        } else {
            panel.width().min(maximum)
        };
        let text = ellipsis(self.font(style.face, style.size)?, &sanitize(text), limit)?;
        if text.is_empty() {
            return Ok(());
        }
        let index = self.entries.iter().position(|e| {
            e.text == text && e.face == style.face && e.size == style.size && e.color == style.color
        });
        let entry = if let Some(index) = index {
            self.entries.remove(index).expect("located text")
        } else {
            let surface = self
                .font(style.face, style.size)?
                .render(&text)
                .blended(color(style.color))
                .map_err(|e| e.to_string())?
                .convert_format(PixelFormatEnum::RGBA32)?;
            // Keep the real ink bounds, not the TTF line box. Button glyphs use
            // this source rectangle so A/B/X/Y are optically centered.
            let (w, h, pitch) = (surface.width(), surface.height(), surface.pitch() as usize);
            let mut bounds = (w, h, 0, 0);
            surface.with_lock(|bytes| {
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
            let ink = if bounds.2 > bounds.0 {
                Rect::new(
                    bounds.0 as i32,
                    bounds.1 as i32,
                    bounds.2 - bounds.0,
                    bounds.3 - bounds.1,
                )
            } else {
                Rect::new(0, 0, 1, 1)
            };
            let texture = self
                .creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            Entry {
                text,
                face: style.face,
                size: style.size,
                color: style.color,
                texture,
                ink,
            }
        };
        let query = entry.texture.query();
        let (src, w, h) = if matches!(align, Align::Glyph) {
            (Some(entry.ink), entry.ink.width(), entry.ink.height())
        } else {
            (None, query.width, query.height)
        };
        let dx = match align {
            Align::Left => 0,
            Align::Right => panel.width().saturating_sub(w) as i32,
            _ => (panel.width() as i32 - w as i32) / 2,
        };
        let dy = (panel.height() as i32 - h as i32) / 2;
        canvas.copy(
            &entry.texture,
            src,
            Rect::new(panel.x() + dx, panel.y() + dy, w, h),
        )?;
        self.entries.push_back(entry);
        while self.entries.len() > 128 {
            self.entries.pop_front();
        }
        Ok(())
    }
}
pub(crate) fn color(c: Rgba) -> Color {
    Color::RGBA(c.r, c.g, c.b, c.a)
}
fn sanitize(s: &str) -> String {
    s.chars()
        .take(512)
        .map(|c| if c == '\0' { '\u{fffd}' } else { c })
        .collect()
}
fn ellipsis(font: &Font<'_, '_>, text: &str, limit: u32) -> Result<String, String> {
    let width = |s: &str| font.size_of(s).map(|s| s.0).map_err(|e| e.to_string());
    if text.is_empty() || width(text)? <= limit {
        return Ok(text.to_owned());
    }
    if width("…")? > limit {
        return Ok(String::new());
    }
    let mut boundaries: Vec<_> = text.char_indices().map(|(i, _)| i).collect();
    boundaries.push(text.len());
    let mut lo = 0;
    let mut hi = boundaries.len() - 1;
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&format!("{}…", &text[..boundaries[mid]]))? <= limit {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Ok(format!("{}…", &text[..boundaries[lo]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ellipsis_is_font_measured_and_unicode_safe() {
        let ttf = sdl2::ttf::init().unwrap();
        let font = ttf
            .load_font(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../themes/default/fonts/Inter-Regular.ttf"
                ),
                17,
            )
            .unwrap();
        let fitted = ellipsis(
            &font,
            "The Legend of Zelda: A Link to the Past — edição 日本語",
            180,
        )
        .unwrap();
        assert!(fitted.ends_with('…'));
        assert!(font.size_of(&fitted).unwrap().0 <= 180);
        assert_eq!(ellipsis(&font, "Kirby", 180).unwrap(), "Kirby");
        assert_eq!(ellipsis(&font, "Kirby", 1).unwrap(), "");
    }
}
