//! Bounded text textures, lazily loaded font faces, and measured ellipsis.
// TODO(CJK): add a lazy per-glyph fallback font here (Noto Sans CJK subset or
// image-provided font), loaded only for a CJK locale or an uncovered glyph.
// Keep phase-1 Latin/Cyrillic startup free of CJK font I/O and memory costs.
use crate::{
    badges::{self, BadgeKey, SUPERSAMPLE},
    startup::StartupTrace,
    startup_phase,
    theme::Rgba,
};
use sdl2::{
    pixels::Color,
    rect::Rect,
    render::{BlendMode, Canvas, ScaleMode, Texture, TextureCreator},
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
}
struct Entry<'a> {
    source: String,
    width: u32,
    face: Face,
    size: u16,
    color: Rgba,
    texture: Texture<'a>,
}
pub(crate) struct TextCache<'font, 'texture> {
    startup: StartupTrace,
    first_rasterization: StartupTrace,
    ttf: &'font Sdl2TtfContext,
    creator: &'texture TextureCreator<WindowContext>,
    paths: [Vec<PathBuf>; 4],
    fonts: HashMap<(Face, u16), Font<'font, 'static>>,
    entries: VecDeque<Entry<'texture>>,
    badges: VecDeque<(BadgeKey, Texture<'texture>)>,
}
impl<'font, 'texture> TextCache<'font, 'texture> {
    pub fn new(
        ttf: &'font Sdl2TtfContext,
        creator: &'texture TextureCreator<WindowContext>,
        paths: [Vec<PathBuf>; 4],
    ) -> Self {
        Self {
            startup: StartupTrace::default(),
            first_rasterization: StartupTrace::default(),
            ttf,
            creator,
            paths,
            fonts: HashMap::new(),
            entries: VecDeque::new(),
            badges: VecDeque::with_capacity(16),
        }
    }
    pub fn trace_startup(&mut self, trace: StartupTrace) {
        self.startup = trace;
        self.first_rasterization = trace;
    }
    fn font(&mut self, face: Face, size: u16) -> Result<&Font<'font, 'static>, String> {
        if !self.fonts.contains_key(&(face, size)) {
            let paths = &self.paths[face as usize];
            let mut font = paths
                .iter()
                .find_map(|path| {
                    let font = self.ttf.load_font(path, size);
                    startup_phase!(
                        self.startup,
                        "font file open [path={}; face={face:?}; size={size}px; ok={}]",
                        path.display(),
                        font.is_ok()
                    );
                    font.ok()
                })
                .ok_or("error.font")?;
            // Inspect the loaded face, not its filename: legacy themes may
            // rename a regular TTF, or fall back when a preferred face is absent.
            // Inter's static SemiBold has a legacy style of "Regular", but its
            // family name identifies the real weight. Never embolden it twice.
            if matches!(face, Face::Semibold | Face::MonoMedium) {
                let weight = format!(
                    "{} {}",
                    font.face_family_name().unwrap_or_default(),
                    font.face_style_name().unwrap_or_default()
                )
                .to_ascii_lowercase();
                if !font.get_style().contains(FontStyle::BOLD)
                    && !["bold", "demi", "medium"]
                        .iter()
                        .any(|name| weight.contains(name))
                {
                    font.set_style(FontStyle::BOLD);
                }
            }
            self.fonts.insert((face, size), font);
        }
        Ok(self.fonts.get(&(face, size)).expect("loaded face"))
    }
    fn badge(&mut self, key: BadgeKey) -> Result<&Texture<'texture>, String> {
        if let Some(index) = self.badges.iter().position(|(k, _)| *k == key) {
            let hit = self.badges.remove(index).expect("located badge");
            self.badges.push_back(hit);
        } else {
            let surface = badges::render(
                self.font(Face::Semibold, key.size * SUPERSAMPLE as u16)?,
                key,
            )?;
            self.first_rasterization
                .phase_once("first text rasterization");
            let mut texture = self
                .creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            texture.set_blend_mode(BlendMode::Blend);
            texture.set_scale_mode(ScaleMode::Linear);
            if self.badges.len() == 16 {
                self.badges.pop_front();
            }
            self.badges.push_back((key, texture));
            startup_phase!(self.startup, "badge {}", key.letter);
        }
        Ok(&self.badges.back().expect("cached badge").1)
    }
    pub fn prepare_badges(
        &mut self,
        diameter: u32,
        size: u16,
        background: Rgba,
        foreground: Rgba,
    ) -> Result<(), String> {
        for letter in ['A', 'B', 'X', 'Y'] {
            self.badge(BadgeKey {
                letter,
                diameter,
                size,
                background,
                foreground,
            })?;
        }
        Ok(())
    }
    pub fn draw_badge(
        &mut self,
        canvas: &mut Canvas<Window>,
        letter: char,
        size: u16,
        background: Rgba,
        foreground: Rgba,
        panel: Rect,
    ) -> Result<(), String> {
        canvas.copy(
            self.badge(BadgeKey {
                letter,
                diameter: panel.width(),
                size,
                background,
                foreground,
            })?,
            None,
            panel,
        )
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
        // Only the first 512 codepoints can reach layout. Keep a bounded raw
        // prefix as the lookup key: no sanitizing, measuring or fitting on hits.
        let mut end = text.len().min(512 * 4);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let source = &text[..end];
        let index = self.entries.iter().position(|e| {
            e.source == source
                && e.width == limit
                && e.face == style.face
                && e.size == style.size
                && e.color == style.color
        });
        let entry = if let Some(index) = index {
            self.entries.remove(index).expect("located text")
        } else {
            let text = ellipsis(self.font(style.face, style.size)?, &sanitize(source), limit)?;
            if text.is_empty() {
                return Ok(());
            }
            let surface = self
                .font(style.face, style.size)?
                .render(&text)
                .blended(color(style.color))
                .map_err(|e| e.to_string())?;
            self.first_rasterization
                .phase_once("first text rasterization");
            let texture = self
                .creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            Entry {
                source: source.to_owned(),
                width: limit,
                face: style.face,
                size: style.size,
                color: style.color,
                texture,
            }
        };
        let query = entry.texture.query();
        let (w, h) = (query.width, query.height);
        let dx = match align {
            Align::Left => 0,
            Align::Right => panel.width().saturating_sub(w) as i32,
            _ => (panel.width() as i32 - w as i32) / 2,
        };
        let dy = (panel.height() as i32 - h as i32) / 2;
        canvas.copy(
            &entry.texture,
            None,
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
    #[test]
    fn prepared_text_and_badge_hits_need_no_font_loading_or_layout() {
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let window = video.window("cache", 160, 80).build().unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        let creator = canvas.texture_creator();
        let ttf = sdl2::ttf::init().unwrap();
        let mut paths: [Vec<PathBuf>; 4] = std::array::from_fn(|_| vec![]);
        paths[0].push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../themes/default/fonts/Inter-Regular.ttf"
        )));
        paths[1].push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../themes/default/fonts/Inter-SemiBold.ttf"
        )));
        let mut cache = TextCache::new(&ttf, &creator, paths);
        let style = Style {
            face: Face::Regular,
            size: 13,
            color: Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
        };
        let panel = Rect::new(0, 0, 80, 20);
        let source = "Very\0 long source text that must be fitted";
        cache
            .draw(&mut canvas, source, style, panel, Align::Left)
            .unwrap();
        let background = Rgba {
            r: 28,
            g: 37,
            b: 48,
            a: 255,
        };
        cache
            .prepare_badges(20, 11, background, style.color)
            .unwrap();
        assert_eq!(cache.badges.len(), 4);
        assert_eq!(
            cache.font(Face::Semibold, 44).unwrap().get_style(),
            FontStyle::NORMAL,
            "the shipped SemiBold must not be synthetically emboldened"
        );
        for (_, texture) in &cache.badges {
            let q = texture.query();
            assert_eq!((q.width, q.height), (80, 80));
            assert_eq!(texture.scale_mode(), ScaleMode::Linear);
        }
        cache.fonts.clear();
        cache.paths = std::array::from_fn(|_| vec![]);
        // With font loading deliberately unavailable, a fitted-texture hit
        // must still render. Any measure/ellipsis call would fail here.
        cache
            .draw(&mut canvas, source, style, panel, Align::Left)
            .unwrap();
        for letter in ['A', 'B', 'X', 'Y'] {
            cache
                .draw_badge(
                    &mut canvas,
                    letter,
                    11,
                    background,
                    style.color,
                    Rect::new(0, 0, 20, 20),
                )
                .unwrap();
        }
        let badge = BadgeKey {
            letter: 'A',
            diameter: 20,
            size: 11,
            background,
            foreground: style.color,
        };
        // Every visual input is part of the key, never a stale texture hit.
        for changed in [
            BadgeKey {
                letter: 'Z',
                ..badge
            },
            BadgeKey {
                diameter: 21,
                ..badge
            },
            BadgeKey { size: 12, ..badge },
            BadgeKey {
                background: Rgba {
                    r: 64,
                    ..background
                },
                ..badge
            },
            BadgeKey {
                foreground: Rgba {
                    r: 64,
                    ..style.color
                },
                ..badge
            },
        ] {
            assert!(cache.badge(changed).is_err());
        }
        assert_eq!(cache.badges.len(), 4);
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.fonts.is_empty());
        assert!(cache
            .draw(
                &mut canvas,
                source,
                style,
                Rect::new(0, 0, 81, 20),
                Align::Left
            )
            .is_err());
        assert!(cache
            .draw(&mut canvas, "another source", style, panel, Align::Left)
            .is_err());
        for changed in [
            Style { size: 14, ..style },
            Style {
                face: Face::Semibold,
                ..style
            },
            Style {
                color: Rgba {
                    r: 200,
                    ..style.color
                },
                ..style
            },
        ] {
            assert!(cache
                .draw(&mut canvas, source, changed, panel, Align::Left)
                .is_err());
        }
        let root = tempfile::tempdir().unwrap();
        let body = root.path().join("body.ttf");
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../themes/default/fonts/Inter-Regular.ttf"
            ),
            &body,
        )
        .unwrap();
        cache.paths[1].push(body);
        assert!(
            cache
                .font(Face::Semibold, 44)
                .unwrap()
                .get_style()
                .contains(FontStyle::BOLD),
            "a regular-only fallback needs synthesized bold at 4× regardless of filename"
        );
    }
}
