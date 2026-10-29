//! Decode and resize off-thread; SDL textures never leave the render thread.
use archr_io::atomic_publish;
use std::{
    collections::{HashSet, VecDeque},
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    time::UNIX_EPOCH,
};

use sdl2::{
    pixels::PixelFormatEnum,
    render::{Texture, TextureCreator},
    video::WindowContext,
};

pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Stable FNV-1a over the complete path, plus source mtime and target bounds.
/// Metadata and all cache I/O belong on the loader thread.
pub fn thumbnail_path(cache: &Path, source: &Path, width: u32, height: u32) -> io::Result<PathBuf> {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in source.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let mtime = fs::metadata(source)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    Ok(cache.join(format!(
        "{hash:016x}-{mtime}-{}x{}.png",
        width.max(1),
        height.max(1)
    )))
}

pub fn load_thumbnail(
    cache: &Path,
    source: &Path,
    width: u32,
    height: u32,
) -> io::Result<Thumbnail> {
    thumbnail(cache, source, (width.max(1), height.max(1)), false)
}

/// Pixel art keeps hard edges: it grows by whole multiples and shrinks
/// without smoothing.
fn thumbnail(
    cache: &Path,
    source: &Path,
    (width, height): (u32, u32),
    pixel: bool,
) -> io::Result<Thumbnail> {
    if (width, height) == (0, 0) {
        // Theme images: drawn at their own size, never resized nor cached.
        let image = decode(source, 4096, 4096)?.to_rgba8();
        return Ok(Thumbnail {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        });
    }
    let path = thumbnail_path(cache, source, width, height)?;
    let cached = decode(&path, width, height).ok();
    let image = match cached {
        Some(image) => image,
        None => {
            let image = decode(source, 16384, 16384)?;
            let image = if pixel {
                let (w, h) = (image.width().max(1), image.height().max(1));
                let k = (width / w).min(height / h);
                let (w, h) = if k >= 1 {
                    (w * k, h * k)
                } else {
                    (width, height)
                };
                image.resize(w, h, image::imageops::FilterType::Nearest)
            } else {
                image.thumbnail(width, height)
            };
            // Cache failure is nonfatal; the thumbnail is still usable.
            if let Err(error) = atomic_publish(&path, false, |file| {
                image
                    .write_to(file, image::ImageFormat::Png)
                    .map_err(io::Error::other)
            }) {
                log::warn!("Cannot cache cover: {error}");
            }
            image
        }
    }
    .to_rgba8();
    Ok(Thumbnail {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

fn decode(path: &Path, width: u32, height: u32) -> io::Result<image::DynamicImage> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(io::Error::other)
}

fn current_thumbnail(name: &str) -> bool {
    let Some((source, size)) = name.strip_suffix(".png").and_then(|s| s.rsplit_once('-')) else {
        return false;
    };
    let Some((hash, mtime)) = source.split_once('-') else {
        return false;
    };
    hash.len() == 16
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && !mtime.is_empty()
        && mtime.bytes().all(|b| b.is_ascii_digit())
        && size.split_once('x').is_some_and(|(w, h)| {
            [w, h].iter().all(|s| {
                s.bytes().all(|b| b.is_ascii_digit()) && s.parse::<u32>().is_ok_and(|n| n > 0)
            })
        })
}

// One streaming pass on the cover worker: no recursion or directory-sized allocation.
fn cleanup_thumbnails(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_file())
            && !entry.file_name().to_str().is_some_and(current_thumbnail)
        {
            if let Err(error) = fs::remove_file(entry.path()) {
                log::warn!("Cannot remove obsolete thumbnail: {error}");
            }
        }
    }
}

type Loaded = (PathBuf, io::Result<Option<Thumbnail>>);

pub struct CoverCache<'a> {
    creator: &'a TextureCreator<WindowContext>,
    requests: SyncSender<Vec<Vec<PathBuf>>>,
    ready: Receiver<Loaded>,
    pending: HashSet<PathBuf>,
    /// Requests issued before `clear`; their results may predate the new art.
    stale: HashSet<PathBuf>,
    textures: VecDeque<(PathBuf, Option<Texture<'a>>)>,
    pixel: bool,
    capacity: usize,
}

impl<'a> CoverCache<'a> {
    pub fn new(
        creator: &'a TextureCreator<WindowContext>,
        dir: PathBuf,
        width: u32,
        height: u32,
    ) -> Self {
        Self::scaled(creator, dir, (width.max(1), height.max(1)), false)
    }
    /// With `pixel`, pictures are scaled without smoothing (pixel art); the
    /// caller gives such a cache its own `dir`. A (0, 0) size keeps every
    /// picture as it is (theme images), and the cache holds 64 of them.
    pub fn scaled(
        creator: &'a TextureCreator<WindowContext>,
        dir: PathBuf,
        (width, height): (u32, u32),
        pixel: bool,
    ) -> Self {
        let (requests, jobs) = mpsc::sync_channel::<Vec<Vec<PathBuf>>>(1);
        let (done, ready) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            cleanup_thumbnails(&dir);
            while let Ok(paths) = jobs.recv() {
                for candidates in paths {
                    let Some(key) = candidates.first() else {
                        continue;
                    };
                    let result = candidates
                        .iter()
                        .find(|path| path.is_file())
                        .map(|path| thumbnail(&dir, path, (width, height), pixel))
                        .transpose();
                    if done.send((key.clone(), result)).is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            creator,
            requests,
            ready,
            pending: HashSet::new(),
            stale: HashSet::new(),
            textures: VecDeque::new(),
            pixel,
            capacity: if (width, height) == (0, 0) { 64 } else { 16 },
        }
    }

    /// Request only the visible selection neighbourhood, never an entire system.
    pub fn request(&mut self, paths: &[PathBuf]) {
        self.queue(paths.iter().map(std::slice::from_ref));
    }

    /// Resolve each ordered candidate group on the worker; the first path is its cache key.
    pub fn request_candidates(&mut self, paths: &[Vec<PathBuf>]) {
        self.queue(paths.iter().map(Vec::as_slice));
    }

    fn queue<'p>(&mut self, paths: impl Iterator<Item = &'p [PathBuf]>) {
        let missing: Vec<_> = paths
            .filter(|paths| {
                paths.first().is_some_and(|key| {
                    !self.pending.contains(key)
                        && !self.textures.iter().any(|(cached, _)| cached == key)
                })
            })
            .take(5)
            .map(<[PathBuf]>::to_vec)
            .collect();
        if !missing.is_empty() && self.requests.try_send(missing.clone()).is_ok() {
            self.pending.extend(
                missing
                    .into_iter()
                    .filter_map(|paths| paths.into_iter().next()),
            );
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((path, result)) = self.ready.try_recv() {
            self.pending.remove(&path);
            if self.stale.remove(&path) {
                continue;
            }
            let texture = result.map_err(|error| error.to_string()).and_then(|thumb| {
                thumb
                    .map(|thumb| {
                        let mut texture = self
                            .creator
                            .create_texture_static(
                                PixelFormatEnum::RGBA32,
                                thumb.width,
                                thumb.height,
                            )
                            .map_err(|error| error.to_string())?;
                        texture
                            .update(None, &thumb.rgba, thumb.width as usize * 4)
                            .map_err(|error| error.to_string())?;
                        texture.set_blend_mode(sdl2::render::BlendMode::Blend);
                        texture.set_scale_mode(if self.pixel {
                            sdl2::render::ScaleMode::Nearest
                        } else {
                            sdl2::render::ScaleMode::Linear
                        });
                        Ok(texture)
                    })
                    .transpose()
            });
            let texture = match texture {
                Ok(texture) => texture,
                Err(error) => {
                    log::warn!("Cover {}: {error}", path.display());
                    None
                }
            };
            self.textures.push_back((path, texture));
            while self.textures.len() > self.capacity {
                self.textures.pop_front();
            }
            changed = true;
        }
        changed
    }

    /// Forget decoded covers, including "no cover" results, after art changed
    /// on disk. Thumbnails on disk are keyed by source mtime and stay valid.
    pub fn clear(&mut self) {
        self.textures.clear();
        self.stale.extend(self.pending.drain());
    }
    /// A decoded texture without touching the recency order, so several can
    /// be borrowed for one frame.
    pub fn peek(&self, path: &Path) -> Option<&Texture<'a>> {
        self.textures
            .iter()
            .find(|(cached, _)| cached == path)
            .and_then(|(_, texture)| texture.as_ref())
    }
    pub fn get(&mut self, path: &PathBuf) -> Option<&Texture<'a>> {
        self.get_mut(path).map(|texture| &*texture)
    }
    pub fn get_mut(&mut self, path: &PathBuf) -> Option<&mut Texture<'a>> {
        let index = self
            .textures
            .iter()
            .position(|(cached, _)| cached == path)?;
        let entry = self.textures.remove(index)?;
        self.textures.push_back(entry);
        self.textures.back_mut()?.1.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_removes_legacy_files_but_preserves_sized_thumbnails_and_directories() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("thumbs");
        cleanup_thumbnails(&cache);
        fs::create_dir(&cache).unwrap();
        let keep = [
            "0123456789abcdef-0-1x1.png",
            "abcdef0123456789-1700000000000000000-210x280.png",
            "abcdef0123456789-1700000000000000000-40x32.png",
        ];
        let remove = [
            "0123456789abcdef-1700000000000000000.png",
            "0123456789abcdef-1700000000000000000.size",
            "0123456789abcdef-1700000000000000000.png.size",
            "0123456789abcdef-0-0x280.png",
            "0123456789abcdef-0-40x0.png",
            "0123456789abcdef-0-40x32.png.size",
            "0123456789abcdef-0-+40x32.png",
            "0123456789abcdeg-0-40x32.png",
            "0123456789abcdef--40x32.png",
            "not-a-thumbnail.png",
        ];
        for name in keep.iter().chain(&remove) {
            fs::write(cache.join(name), b"cache").unwrap();
        }
        let nested = cache.join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("legacy.size"), b"untouched").unwrap();
        cleanup_thumbnails(&cache);
        for name in keep {
            assert!(cache.join(name).is_file(), "removed {name}");
        }
        for name in remove {
            assert!(!cache.join(name).exists(), "retained {name}");
        }
        assert!(nested.join("legacy.size").is_file());
        cleanup_thumbnails(&cache);
        assert_eq!(fs::read_dir(cache).unwrap().count(), 4);
    }
    #[test]
    fn pixel_art_grows_by_whole_multiples_without_smoothing() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("sprite").with_extension("png");
        let mut art = image::RgbaImage::from_pixel(4, 3, image::Rgba([0, 0, 0, 255]));
        art.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        art.save(&source).unwrap();
        let thumb = thumbnail(&dir.path().join("cache"), &source, (30, 30), true).unwrap();
        assert_eq!(
            (thumb.width, thumb.height),
            (28, 21),
            "seven times, the most that fits"
        );
        let pixel = |x: usize, y: usize| thumb.rgba[(y * 28 + x) * 4];
        assert_eq!(
            (pixel(6, 6), pixel(7, 6), pixel(6, 7)),
            (255, 0, 0),
            "hard edges"
        );
    }
}
