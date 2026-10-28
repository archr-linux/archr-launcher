//! Disposable, bounded PNGs for deterministic startup rasters.
use crate::Theme;
use archr_io::{atomic_publish, read_bytes, Symlinks::Reject};
use sdl2::{pixels::PixelFormatEnum, surface::Surface};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Cursor,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct RasterCache {
    dir: PathBuf,
    key: String,
}
impl RasterCache {
    pub fn new(dir: PathBuf, key: String) -> Self {
        Self { dir, key }
    }
    pub fn theme_key(theme: &Theme, scale: f32, fonts: &[PathBuf]) -> String {
        let mut hash = DefaultHasher::new();
        // Bump on any raster algorithm change. Font metadata also invalidates
        // an installed font replacement without reading font bytes at startup.
        ("raster-v1", format!("{theme:?}"), scale.to_bits()).hash(&mut hash);
        for path in fonts {
            path.hash(&mut hash);
            if let Ok(meta) = path.metadata() {
                (meta.len(), meta.modified().ok()).hash(&mut hash);
            }
        }
        format!("{:016x}", hash.finish())
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("raster-{}-{name}.png", self.key))
    }
    pub fn surface(
        &self,
        name: &str,
        width: u32,
        height: u32,
        generate: impl FnOnce() -> Result<Surface<'static>, String>,
    ) -> Result<(Surface<'static>, bool), String> {
        let path = self.path(name);
        if let Some(surface) = load(&path, width, height) {
            return Ok((surface, true));
        }
        let surface = generate()?;
        let result = atomic_publish(&path, false, |file| {
            let pitch = surface.pitch() as usize;
            let mut pixels = Vec::with_capacity((width * height * 4) as usize);
            surface.with_lock(|bytes| {
                for y in 0..height as usize {
                    pixels.extend_from_slice(&bytes[y * pitch..y * pitch + width as usize * 4]);
                }
            });
            image::write_buffer_with_format(
                file,
                &pixels,
                width,
                height,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(std::io::Error::other)
        });
        if let Err(error) = result {
            log::warn!("Cannot cache raster {}: {error}", path.display());
        }
        Ok((surface, false))
    }
}
fn load(path: &Path, width: u32, height: u32) -> Option<Surface<'static>> {
    let bytes = read_bytes(path, 4 * 1024 * 1024, Reject).ok()?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.into_rgba8();
    if image.dimensions() != (width, height) {
        return None;
    }
    let mut surface = Surface::new(width, height, PixelFormatEnum::RGBA32).ok()?;
    let pitch = surface.pitch() as usize;
    surface.with_lock_mut(|bytes| {
        for y in 0..height as usize {
            let row = y * width as usize * 4;
            bytes[y * pitch..y * pitch + width as usize * 4]
                .copy_from_slice(&image.as_raw()[row..row + width as usize * 4]);
        }
    });
    Some(surface)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl2::{pixels::PixelFormatEnum, surface::Surface};

    #[test]
    fn png_hits_preserve_all_rgba_bytes_and_bad_files_regenerate() {
        let root = tempfile::tempdir().unwrap();
        let cache = RasterCache::new(root.path().to_owned(), "test".into());
        let make = || {
            let mut surface = Surface::new(2, 1, PixelFormatEnum::RGBA32).unwrap();
            surface.with_lock_mut(|b| b[..8].copy_from_slice(&[1, 2, 3, 0, 4, 5, 6, 128]));
            Ok(surface)
        };
        let (first, hit) = cache.surface("badge-A", 2, 1, make).unwrap();
        assert!(!hit);
        let (second, hit) = cache
            .surface("badge-A", 2, 1, || panic!("must load PNG"))
            .unwrap();
        assert!(hit);
        first.with_lock(|a| second.with_lock(|b| assert_eq!(a, b)));
        for invalid in [b"broken".as_slice(), b"\x89PNG\r\n\x1a\n".as_slice()] {
            std::fs::write(cache.path("badge-A"), invalid).unwrap();
            assert!(!cache.surface("badge-A", 2, 1, make).unwrap().1);
        }
        let wrong = image::RgbaImage::new(3, 1);
        wrong.save(cache.path("badge-A")).unwrap();
        assert!(!cache.surface("badge-A", 2, 1, make).unwrap().1);
        // Cache failures are never a reason to lose the UI.
        let blocked = root.path().join("not-a-directory");
        std::fs::write(&blocked, []).unwrap();
        assert!(RasterCache::new(blocked, "test".into())
            .surface("badge-A", 2, 1, make)
            .is_ok());
    }

    #[test]
    fn theme_scale_and_font_changes_invalidate_rasters() {
        let root = tempfile::tempdir().unwrap();
        let font = root.path().join("font.ttf");
        std::fs::write(&font, b"one").unwrap();
        let theme = crate::Theme::builtin();
        let fallback = root.path().join("fallback.ttf");
        std::fs::write(&fallback, b"fallback").unwrap();
        let fonts = [font.clone(), fallback.clone()];
        let key = RasterCache::theme_key(&theme, 1., &fonts);
        let mut changed = theme.clone();
        changed.badge.r += 1;
        assert_ne!(key, RasterCache::theme_key(&changed, 1., &fonts));
        assert_ne!(key, RasterCache::theme_key(&theme, 2., &fonts));
        std::fs::write(&fallback, b"new fallback even if the preferred face exists").unwrap();
        assert_ne!(key, RasterCache::theme_key(&theme, 1., &fonts));
        let key = RasterCache::theme_key(&theme, 1., &fonts);
        std::fs::write(&font, b"replacement").unwrap();
        assert_ne!(key, RasterCache::theme_key(&theme, 1., &fonts));
    }
}
