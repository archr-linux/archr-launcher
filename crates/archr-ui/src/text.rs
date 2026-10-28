//! Bounded text textures, lazily loaded font faces, and measured ellipsis.
// TODO(CJK): add a lazy per-glyph fallback font here (Noto Sans CJK subset or
// image-provided font), loaded only for a CJK locale or an uncovered glyph.
// Keep phase-1 Latin/Cyrillic startup free of CJK font I/O and memory costs.
use crate::{
    badges::{self, BadgeKey, SUPERSAMPLE},
    icons::{self, Glyph},
    startup::StartupTrace,
    startup_phase,
    theme::Rgba,
};
use sdl2::{
    pixels::Color,
    rect::Rect,
    render::{BlendMode, Canvas, ScaleMode, Texture, TextureCreator},
    rwops::RWops,
    surface::Surface,
    ttf::{Font, FontStyle, Sdl2TtfContext},
    video::{Window, WindowContext},
};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Face {
    Regular,
    Semibold,
    Mono,
    MonoMedium,
}
#[derive(Clone, Copy, PartialEq, Eq)]
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
    style: Style,
    texture: Texture<'a>,
}
struct FontFace<'font> {
    path: PathBuf,
    font: Font<'font, 'static>,
    // Declared after font: the RWops/font is destroyed before its backing bytes.
    _bytes: Box<[u8]>,
    size: u16,
    style: FontStyle,
    synthesize_bold: bool,
}

struct Fonts<'font> {
    startup: StartupTrace,
    ttf: &'font Sdl2TtfContext,
    paths: [Vec<PathBuf>; 4],
    fonts: Vec<FontFace<'font>>,
    resolved: HashMap<Face, usize>,
    measurements: VecDeque<(String, Face, u16, u32)>,
}
impl<'font> Fonts<'font> {
    fn new(ttf: &'font Sdl2TtfContext, paths: [Vec<PathBuf>; 4]) -> Self {
        Self {
            startup: StartupTrace::default(),
            ttf,
            paths,
            fonts: Vec::new(),
            resolved: HashMap::new(),
            measurements: VecDeque::new(),
        }
    }
    fn measure(&mut self, text: &str, face: Face, size: u16) -> Result<u32, String> {
        let source = bounded_source(text);
        if let Some((_, _, _, width)) = self
            .measurements
            .iter()
            .find(|(s, f, z, _)| s == source && *f == face && *z == size)
        {
            return Ok(*width);
        }
        let width = self
            .font(face, size)?
            .size_of(&sanitize(source))
            .map_err(|e| e.to_string())?
            .0;
        startup_phase!(
            self.startup,
            "text measurement [face={face:?}; size={size}px]"
        );
        if self.measurements.len() == 256 {
            self.measurements.pop_front();
        }
        self.measurements
            .push_back((source.to_owned(), face, size, width));
        Ok(width)
    }
    fn font(&mut self, face: Face, size: u16) -> Result<&Font<'font, 'static>, String> {
        let key = face;
        if !self.resolved.contains_key(&key) {
            for path in &self.paths[face as usize] {
                let path = path.canonicalize().unwrap_or_else(|_| path.clone());
                if let Some(index) = self.fonts.iter().position(|loaded| loaded.path == path) {
                    self.resolved.insert(key, index);
                    break;
                }
                let font = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        let bytes = bytes.into_boxed_slice();
                        if bytes.len() > i32::MAX as usize {
                            return Err("Font exceeds SDL RWops size limit".into());
                        }
                        // SAFETY: the boxed allocation never moves or changes. FontFace
                        // owns it and drops font (including RWops) first. The static
                        // view stays private to that font, never escaping independently.
                        let view =
                            unsafe { std::slice::from_raw_parts(bytes.as_ptr(), bytes.len()) };
                        self.ttf
                            .load_font_from_rwops(RWops::from_bytes(view)?, size)
                            .map(|font| (font, bytes))
                    });
                startup_phase!(
                    self.startup,
                    "font file open [path={}; face={face:?}; size={size}px; ok={}]",
                    path.display(),
                    font.is_ok()
                );
                let Ok((font, bytes)) = font else { continue };
                // Static Inter SemiBold reports legacy style Regular; inspect
                // the family as well and never synthesize a shipped weight twice.
                let weight = format!(
                    "{} {}",
                    font.face_family_name().unwrap_or_default(),
                    font.face_style_name().unwrap_or_default()
                )
                .to_ascii_lowercase();
                let style = font.get_style();
                let synthesize_bold = !style.contains(FontStyle::BOLD)
                    && !["bold", "demi", "medium"]
                        .iter()
                        .any(|n| weight.contains(n));
                self.resolved.insert(key, self.fonts.len());
                self.fonts.push(FontFace {
                    path,
                    font,
                    _bytes: bytes,
                    size,
                    style,
                    synthesize_bold,
                });
                break;
            }
        }
        let index = *self.resolved.get(&key).ok_or("error.font")?;
        let loaded = &mut self.fonts[index];
        if loaded.size != size {
            // SAFETY: exclusively owned live handle, never accessed concurrently across the startup handover.
            if unsafe { sdl2::sys::ttf::TTF_SetFontSize(loaded.font.raw(), i32::from(size)) } != 0 {
                return Err(sdl2::get_error());
            }
            loaded.size = size;
        }
        let style = if loaded.synthesize_bold && matches!(face, Face::Semibold | Face::MonoMedium) {
            loaded.style | FontStyle::BOLD
        } else {
            loaded.style
        };
        if loaded.font.get_style() != style {
            loaded.font.set_style(style);
        }
        Ok(&loaded.font)
    }
}

/// CPU-only startup assets. No renderer, texture, or shared font handle.
/// Constructed on one worker and consumed after joining it; TTF_Init/TTF_Quit
/// stay outside that scope, and no other thread may use SDL_ttf while it runs.
pub struct PreparedText<'font> {
    fonts: Fonts<'font>,
    entries: Vec<(String, u32, Style, Surface<'static>)>,
    icons: Vec<((Glyph, u32), Surface<'static>)>,
    icon_loader: icons::IconLoader,
    pub(crate) shadow: Option<Surface<'static>>,
    first_rasterization: StartupTrace,
}
// SAFETY: only exclusively owned memory-backed fonts/RWops and owned CPU
// surfaces cross threads. Their C APIs have no thread affinity. Nothing is
// borrowed from SDL video, and the context outlives the joined worker/assets.
unsafe impl Send for PreparedText<'_> {}

impl<'font> PreparedText<'font> {
    pub(crate) fn new(
        ttf: &'font Sdl2TtfContext,
        paths: [Vec<PathBuf>; 4],
        trace: StartupTrace,
    ) -> Self {
        let mut fonts = Fonts::new(ttf, paths);
        fonts.startup = trace;
        Self {
            fonts,
            entries: Vec::new(),
            icons: Vec::new(),
            icon_loader: icons::IconLoader::new(None, Path::new(icons::DEFAULT_ICONS_DIR)),
            shadow: None,
            first_rasterization: trace,
        }
    }
    pub(crate) fn prepare_icons(
        &mut self,
        theme_dir: Option<&Path>,
        system_icons_dir: &Path,
        icons: impl IntoIterator<Item = (Glyph, u32)>,
    ) {
        self.icon_loader = icons::IconLoader::new(theme_dir, system_icons_dir);
        for (glyph, size) in icons {
            if self.icons.iter().any(|(key, _)| *key == (glyph, size)) {
                continue;
            }
            if let Some(surface) = self.icon_loader.render(glyph, size) {
                self.icons.push(((glyph, size), surface));
                startup_phase!(
                    self.fonts.startup,
                    "icon rasterization [glyph={glyph:?}; size={size}px]"
                );
            }
        }
    }
    pub(crate) fn label(&mut self, source: &str, style: Style, width: u32) -> Result<(), String> {
        let source = bounded_source(source);
        if self
            .entries
            .iter()
            .any(|(s, w, st, _)| s == source && *w == width && *st == style)
        {
            return Ok(());
        }
        let text = ellipsis(
            self.fonts.font(style.face, style.size)?,
            &sanitize(source),
            width,
        )?;
        if text.is_empty() {
            return Ok(());
        }
        let surface = self
            .fonts
            .font(style.face, style.size)?
            .render(&text)
            .blended(color(style.color))
            .map_err(|e| e.to_string())?;
        self.first_rasterization
            .phase_once("first text rasterization");
        self.entries
            .push((source.to_owned(), width, style, surface));
        Ok(())
    }
    pub(crate) fn measure(&mut self, text: &str, face: Face, size: u16) -> Result<u32, String> {
        self.fonts.measure(text, face, size)
    }
}

pub(crate) struct TextCache<'font, 'texture> {
    startup: StartupTrace,
    first_rasterization: StartupTrace,
    fonts: Fonts<'font>,
    creator: &'texture TextureCreator<WindowContext>,
    entries: VecDeque<Entry<'texture>>,
    badges: VecDeque<(BadgeKey, Texture<'texture>)>,
    icons: HashMap<(Glyph, u32), Texture<'texture>>,
    pub(crate) icon_loader: icons::IconLoader,
}
impl<'font, 'texture> TextCache<'font, 'texture> {
    pub fn from_prepared(
        creator: &'texture TextureCreator<WindowContext>,
        prepared: PreparedText<'font>,
    ) -> Result<Self, String> {
        let mut cache = Self {
            startup: prepared.fonts.startup,
            first_rasterization: prepared.first_rasterization,
            fonts: prepared.fonts,
            creator,
            entries: VecDeque::new(),
            badges: VecDeque::with_capacity(16),
            icons: HashMap::new(),
            icon_loader: prepared.icon_loader,
        };
        for (source, width, style, surface) in prepared.entries {
            let texture = creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            cache.entries.push_back(Entry {
                source,
                style,
                width,
                texture,
            });
        }
        for (key, surface) in prepared.icons {
            let mut texture = creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            texture.set_blend_mode(BlendMode::Blend);
            texture.set_scale_mode(ScaleMode::Linear);
            cache.icons.insert(key, texture);
        }
        startup_phase!(cache.startup, "CPU surfaces upload");
        Ok(cache)
    }
    pub fn trace_startup(&mut self, trace: StartupTrace) {
        self.startup = trace;
        self.fonts.startup = trace;
        self.first_rasterization = trace;
    }
    fn font(&mut self, face: Face, size: u16) -> Result<&Font<'font, 'static>, String> {
        self.fonts.font(face, size)
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
            startup_phase!(self.startup, "badge {} [rasterized]", key.letter);
        }
        Ok(&self.badges.back().expect("cached badge").1)
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
    /// Icons are cached white per glyph and size, then tinted to `ink`.
    pub fn draw_icon(
        &mut self,
        canvas: &mut Canvas<Window>,
        glyph: Glyph,
        panel: Rect,
        ink: Rgba,
    ) -> Result<(), String> {
        let key = (glyph, panel.width());
        if !self.icons.contains_key(&key) {
            let Some(surface) = self.icon_loader.render(glyph, panel.width()) else {
                return Ok(());
            };
            startup_phase!(
                self.startup,
                "icon rasterization [glyph={glyph:?}; size={}px]",
                panel.width()
            );
            let mut texture = self
                .creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            texture.set_blend_mode(BlendMode::Blend);
            texture.set_scale_mode(ScaleMode::Linear);
            self.icons.insert(key, texture);
        }
        let texture = self.icons.get_mut(&key).expect("cached icon");
        texture.set_color_mod(ink.r, ink.g, ink.b);
        texture.set_alpha_mod(ink.a);
        canvas.copy(texture, None, panel)
    }
    pub fn measure(&mut self, text: &str, face: Face, size: u16) -> Result<u32, String> {
        self.fonts.measure(text, face, size)
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
        let source = bounded_source(text);
        let index = self
            .entries
            .iter()
            .position(|e| e.source == source && e.style == style && e.width == limit);
        let entry = if let Some(index) = index {
            self.entries.remove(index).expect("located text")
        } else {
            let text = ellipsis(self.font(style.face, style.size)?, &sanitize(source), limit)?;
            if text.is_empty() {
                return Ok(());
            }
            startup_phase!(
                self.startup,
                "text rasterization [face={:?}; size={}px]",
                style.face,
                style.size
            );
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
                style,
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
fn bounded_source(text: &str) -> &str {
    let mut end = text.len().min(512 * 4);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
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

    fn bundled_font(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../themes/default/fonts")
            .join(format!("{name}.ttf"))
    }

    fn load_font<'a>(ttf: &'a Sdl2TtfContext, name: &str, size: u16) -> Font<'a, 'static> {
        ttf.load_font(bundled_font(name), size).unwrap()
    }

    #[test]
    fn ellipsis_is_font_measured_and_unicode_safe() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        let ttf = sdl2::ttf::init().unwrap();
        let font = load_font(&ttf, "Inter-Regular", 17);
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
    fn icons_start_empty_and_cache_by_glyph_and_size_not_tint() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let window = video.window("icons", 96, 96).build().unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        let creator = canvas.texture_creator();
        let ttf = sdl2::ttf::init().unwrap();
        let prepared = PreparedText::new(
            &ttf,
            std::array::from_fn(|_| vec![]),
            StartupTrace::default(),
        );
        let mut cache = TextCache::from_prepared(&creator, prepared).unwrap();
        assert!(cache.icons.is_empty());
        let white = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        for (glyph, size) in [(Glyph::Wifi, 16), (Glyph::Wifi, 24), (Glyph::Games, 48)] {
            let panel = Rect::new(0, 0, size, size);
            cache.draw_icon(&mut canvas, glyph, panel, white).unwrap();
            let original = cache.icons[&(glyph, size)].raw();
            cache
                .draw_icon(
                    &mut canvas,
                    glyph,
                    panel,
                    Rgba {
                        r: 20,
                        a: 128,
                        ..white
                    },
                )
                .unwrap();
            let texture = &cache.icons[&(glyph, size)];
            assert_eq!(original, texture.raw());
            assert_eq!(texture.query().width, size);
            assert_eq!(texture.color_mod(), (20, 255, 255));
            assert_eq!(texture.alpha_mod(), 128);
        }
        assert_eq!(cache.icons.len(), 3);
    }

    #[test]
    fn prepared_measurements_survive_handover_without_font_layout() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let window = video.window("metrics", 160, 80).build().unwrap();
        let canvas = window.into_canvas().software().build().unwrap();
        let creator = canvas.texture_creator();
        let ttf = sdl2::ttf::init().unwrap();
        let paths = std::array::from_fn(|_| vec![bundled_font("Inter-Regular")]);
        let mut prepared = PreparedText::new(&ttf, paths, StartupTrace::default());
        let source = "MO/TO family";
        let width = load_font(&ttf, "Inter-Regular", 17)
            .size_of(source)
            .unwrap()
            .0;
        assert_eq!(prepared.measure(source, Face::Regular, 17).unwrap(), width);
        let mut cache = TextCache::from_prepared(&creator, prepared).unwrap();
        // A hit must not revisit FreeType, even after a different size was used.
        cache.font(Face::Regular, 13).unwrap();
        cache.fonts.fonts.clear();
        cache.fonts.resolved.clear();
        cache.fonts.paths = std::array::from_fn(|_| vec![]);
        assert_eq!(cache.measure(source, Face::Regular, 17).unwrap(), width);
        assert!(cache.measure(source, Face::Regular, 18).is_err());
        assert!(cache.measure(source, Face::Semibold, 17).is_err());
        assert!(cache.measure("different", Face::Regular, 17).is_err());
    }

    #[test]
    fn prepared_text_and_badge_hits_need_no_font_loading_or_layout() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let window = video.window("cache", 160, 80).build().unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        let creator = canvas.texture_creator();
        let ttf = sdl2::ttf::init().unwrap();
        let mut paths: [Vec<PathBuf>; 4] = std::array::from_fn(|_| vec![]);
        paths[0].push(bundled_font("Inter-Regular"));
        paths[1].push(bundled_font("Inter-SemiBold"));
        let mut cache = TextCache::from_prepared(
            &creator,
            PreparedText::new(&ttf, paths, StartupTrace::default()),
        )
        .unwrap();
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
        for letter in ['A', 'B', 'X', 'Y'] {
            cache
                .badge(BadgeKey {
                    letter,
                    diameter: 20,
                    size: 11,
                    background,
                    foreground: style.color,
                })
                .unwrap();
        }
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
        let handle = cache.font(Face::Semibold, 44).unwrap().raw();
        for size in [12, 17, 44, 13, 44] {
            let font = cache.font(Face::Semibold, size).unwrap();
            assert_eq!(font.raw(), handle, "resizing must reuse the physical face");
            let reference = load_font(&ttf, "Inter-SemiBold", size);
            assert_eq!(
                font.size_of("Ação Русский").unwrap(),
                reference.size_of("Ação Русский").unwrap()
            );
        }
        cache.fonts.fonts.clear();
        cache.fonts.resolved.clear();
        cache.fonts.paths = std::array::from_fn(|_| vec![]);
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
        assert!(cache.fonts.fonts.is_empty());
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
        std::fs::copy(bundled_font("Inter-Regular"), &body).unwrap();
        cache.fonts.paths[0].push(body.clone());
        cache.fonts.paths[1].push(body);
        let regular_handle = cache.font(Face::Regular, 13).unwrap().raw();
        let semibold_handle = cache.font(Face::Semibold, 44).unwrap().raw();
        assert_eq!(
            regular_handle, semibold_handle,
            "one handle per physical file"
        );
        assert_eq!(
            cache.font(Face::Regular, 13).unwrap().get_style(),
            FontStyle::NORMAL
        );
        let reference = load_font(&ttf, "Inter-Regular", 13);
        assert_eq!(
            cache.measure("Alias sizes", Face::Regular, 13).unwrap(),
            reference.size_of("Alias sizes").unwrap().0
        );
        assert!(
            cache
                .font(Face::Semibold, 44)
                .unwrap()
                .get_style()
                .contains(FontStyle::BOLD),
            "a regular-only fallback needs synthesized bold at 4× regardless of filename"
        );
        // Read/open/rasterize on a scoped worker, then remove the source file.
        // Resizing still works after handover because the face owns its bytes.
        let path = cache.fonts.paths[0][0].clone();
        let paths = std::array::from_fn(|_| vec![path.clone()]);
        let prepared = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let mut prepared = PreparedText::new(&ttf, paths, StartupTrace::default());
                    prepared.label(source, style, panel.width()).unwrap();
                    prepared
                })
                .join()
                .unwrap()
        });
        std::fs::remove_file(path).unwrap();
        let mut transferred = TextCache::from_prepared(&creator, prepared).unwrap();
        for size in [13, 44, 17, 13] {
            assert_eq!(
                transferred
                    .font(Face::Regular, size)
                    .unwrap()
                    .size_of("Bytes")
                    .unwrap(),
                load_font(&ttf, "Inter-Regular", size)
                    .size_of("Bytes")
                    .unwrap()
            );
        }
        // Deferred badges still render from the handed-over in-memory face.
        transferred.badge(badge).unwrap();
        // CPU-prepared text and cached badges need no font/layout fallback.
        transferred.fonts.fonts.clear();
        transferred.fonts.resolved.clear();
        transferred.fonts.paths = std::array::from_fn(|_| vec![]);
        transferred
            .draw(&mut canvas, source, style, panel, Align::Left)
            .unwrap();
        transferred
            .draw_badge(
                &mut canvas,
                'A',
                11,
                background,
                style.color,
                Rect::new(0, 0, 20, 20),
            )
            .unwrap();
    }
}
