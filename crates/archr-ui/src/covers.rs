//! Decode and resize off-thread; SDL textures never leave the render thread.
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

/// Stable FNV-1a over the complete path, plus nanosecond source mtime.
/// Metadata and all cache I/O belong on the loader thread.
pub fn thumbnail_path(cache: &Path, source: &Path) -> io::Result<PathBuf> {
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
    Ok(cache.join(format!("{hash:016x}-{mtime}.png")))
}

pub fn load_thumbnail(
    cache: &Path,
    source: &Path,
    width: u32,
    height: u32,
) -> io::Result<Thumbnail> {
    let path = thumbnail_path(cache, source)?;
    let size_path = path.with_extension("size");
    let size = format!("{width}x{height}");
    let cached = (fs::read_to_string(&size_path).ok().as_deref() == Some(&size))
        .then(|| {
            let mut reader = image::ImageReader::open(&path).ok()?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(width.max(1));
            limits.max_image_height = Some(height.max(1));
            limits.max_alloc = Some(64 * 1024 * 1024);
            reader.limits(limits);
            reader.decode().ok()
        })
        .flatten();
    let image = match cached {
        Some(image) => image,
        None => {
            let mut reader = image::ImageReader::open(source)?.with_guessed_format()?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(64 * 1024 * 1024);
            reader.limits(limits);
            let image = reader
                .decode()
                .map_err(io::Error::other)?
                .thumbnail(width.max(1), height.max(1));
            // Cache failure is nonfatal; the thumbnail is still usable.
            let save = || -> io::Result<()> {
                fs::create_dir_all(cache)?;
                let temp = tempfile::NamedTempFile::new_in(cache)?;
                image
                    .save_with_format(temp.path(), image::ImageFormat::Png)
                    .map_err(io::Error::other)?;
                temp.persist(&path).map_err(|error| error.error)?;
                fs::write(size_path, size)
            };
            if let Err(error) = save() {
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

type Loaded = (PathBuf, io::Result<Thumbnail>);

pub struct CoverCache<'a> {
    creator: &'a TextureCreator<WindowContext>,
    requests: SyncSender<Vec<PathBuf>>,
    ready: Receiver<Loaded>,
    pending: HashSet<PathBuf>,
    textures: VecDeque<(PathBuf, Option<Texture<'a>>)>,
}

impl<'a> CoverCache<'a> {
    pub fn new(
        creator: &'a TextureCreator<WindowContext>,
        dir: PathBuf,
        width: u32,
        height: u32,
    ) -> Self {
        let (requests, jobs) = mpsc::sync_channel::<Vec<PathBuf>>(1);
        let (done, ready) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            while let Ok(paths) = jobs.recv() {
                for path in paths {
                    let result = load_thumbnail(&dir, &path, width, height);
                    if done.send((path, result)).is_err() {
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
            textures: VecDeque::new(),
        }
    }

    /// Request only the visible selection neighbourhood, never an entire system.
    pub fn request(&mut self, paths: &[PathBuf]) {
        let missing: Vec<_> = paths
            .iter()
            .filter(|path| {
                !self.pending.contains(*path)
                    && !self.textures.iter().any(|(cached, _)| cached == *path)
            })
            .take(5)
            .cloned()
            .collect();
        if !missing.is_empty() && self.requests.try_send(missing.clone()).is_ok() {
            self.pending.extend(missing);
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((path, result)) = self.ready.try_recv() {
            self.pending.remove(&path);
            let texture = result.map_err(|error| error.to_string()).and_then(|thumb| {
                let mut texture = self
                    .creator
                    .create_texture_static(PixelFormatEnum::RGBA32, thumb.width, thumb.height)
                    .map_err(|error| error.to_string())?;
                texture
                    .update(None, &thumb.rgba, thumb.width as usize * 4)
                    .map_err(|error| error.to_string())?;
                Ok(texture)
            });
            let texture = match texture {
                Ok(texture) => Some(texture),
                Err(error) => {
                    log::warn!("Cover {}: {error}", path.display());
                    None
                }
            };
            self.textures.push_back((path, texture));
            while self.textures.len() > 16 {
                self.textures.pop_front();
            }
            changed = true;
        }
        changed
    }

    pub fn get(&mut self, path: &PathBuf) -> Option<&Texture<'a>> {
        let index = self
            .textures
            .iter()
            .position(|(cached, _)| cached == path)?;
        let entry = self.textures.remove(index)?;
        self.textures.push_back(entry);
        self.textures.back()?.1.as_ref()
    }
}
