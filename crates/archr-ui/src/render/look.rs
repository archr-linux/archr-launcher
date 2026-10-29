//! Theme pictures on the browsing screens (docs/temas.md, Imagens do tema):
//! the background, nine-slice frames over boxes, placed decorations and the
//! outline around text drawn straight over the background. Pictures decode
//! on the cover worker; until one arrives the plain drawing stands in.
use super::*;
use crate::{
    covers::CoverCache,
    layout::Decor,
    primitives::nine_slice,
    theme::{Frame9, Outline},
};

/// What the active theme dresses, refreshed when the theme changes.
#[derive(Default)]
pub(super) struct Look<'t> {
    /// Frames apply while a browsing screen is drawn, never in Settings.
    pub on: bool,
    frames: Vec<(String, Frame9)>,
    background: Option<PathBuf>,
    /// Outline of text over the background, and whether the text being
    /// drawn now gets it.
    outline: Option<Outline>,
    pub outlining: bool,
    /// The next moment an animated decoration changes frame.
    pub due: Option<Instant>,
    images: Option<(Option<PathBuf>, bool, CoverCache<'t>)>,
    /// Pictures of the active view still to load ahead, so no screen pops
    /// in; each is asked for until it arrives once. Made for this folder and
    /// view.
    wanted: Vec<PathBuf>,
    wanted_for: Option<(Option<PathBuf>, String)>,
}

impl<'t> Ui<'_, 't> {
    /// Takes the theme's pictures, and starts their loader for its folder.
    pub(super) fn look(&mut self, theme: &Theme, on: bool) {
        let look = &mut self.look;
        look.on = on;
        look.due = None;
        look.outlining = false;
        if look.frames != theme.frames {
            look.frames.clone_from(&theme.frames);
        }
        if look.background != theme.background_image {
            look.background.clone_from(&theme.background_image);
        }
        look.outline = theme.text_outline;
        let key = (self.theme_dir.clone(), theme.pixel_images);
        let stale = look
            .images
            .as_ref()
            .is_none_or(|(dir, pixel, _)| (dir, pixel) != (&key.0, &key.1));
        if let (true, Some(creator)) = (stale, self.creator) {
            let root = self.root.clone().unwrap_or_else(|| PathBuf::from("/"));
            let cache = CoverCache::scaled(
                creator,
                root.join("storage/.cache/archr-launcher/thumbs/theme"),
                (0, 0),
                key.1,
            );
            look.images = Some((key.0.clone(), key.1, cache));
            look.wanted_for = None;
        }
        let view = Some((key.0, theme.active_view.clone()));
        if look.wanted_for != view {
            look.wanted_for = view;
            let active = theme.view(&theme.active_view);
            let decor = active.systems.decor.iter().chain(&active.games.decor);
            let mut wanted: Vec<PathBuf> = theme
                .frames
                .iter()
                .map(|(_, frame)| frame.image.clone())
                .chain(theme.background_image.clone())
                .chain(decor.filter_map(|d| theme.decor_file(&d.image).map(Path::to_path_buf)))
                .collect();
            wanted.sort();
            wanted.dedup();
            look.wanted = wanted;
        }
    }
    /// Uploads pictures that finished decoding; true when one arrived.
    pub(super) fn look_tick(&mut self) -> bool {
        let due = self.look.due.is_some_and(|due| Instant::now() >= due);
        let look = &mut self.look;
        let loaded = look.images.as_mut().is_some_and(|(_, _, cache)| {
            let loaded = cache.poll();
            // Arrived (or found missing) once: never asked for again here, so
            // more pictures than the cache holds cannot loop.
            look.wanted.retain(|path| !cache.has(path));
            if !look.wanted.is_empty() {
                cache.request(&look.wanted);
            }
            loaded
        });
        due || loaded
    }
    /// The theme picture at `path`, asking the loader for it the first time.
    fn picture(&mut self, path: &Path) -> Option<(&mut Texture<'t>, &mut Canvas<Window>)> {
        let (_, _, cache) = self.look.images.as_mut()?;
        let path = path.to_path_buf();
        cache.request(std::slice::from_ref(&path));
        Some((cache.get_mut(&path)?, &mut self.canvas))
    }
    pub(super) fn has_frame(&self, name: &str) -> bool {
        self.look.on && self.look.frames.iter().any(|(n, _)| n == name)
    }
    /// Draws the theme's `name` frame over `rect`; false when the theme has
    /// none (or it is still loading) and the caller draws the plain box.
    pub(super) fn framed(&mut self, name: &str, rect: Rect) -> Result<bool, String> {
        if !self.has_frame(name) {
            return Ok(false);
        }
        let Some(frame) = self.look.frames.iter().find(|(n, _)| n == name) else {
            return Ok(false);
        };
        let frame = frame.1.clone();
        let scale = self.frame.scale;
        let k = (frame.scale * scale).round().max(1.) as u32;
        let [l, t, r, b] = frame.pad.map(|p| (p * scale).round() as i32);
        let dst = Rect::new(
            rect.x() - l,
            rect.y() - t,
            (rect.width() as i32 + l + r).max(1) as u32,
            (rect.height() as i32 + t + b).max(1) as u32,
        );
        let Some((texture, canvas)) = self.picture(&frame.image) else {
            return Ok(false);
        };
        let size = texture.query();
        for (src, to) in nine_slice((size.width, size.height), frame.slice, k, dst, frame.tile) {
            canvas.copy(texture, src, to)?;
        }
        Ok(true)
    }
    /// The background picture over the whole screen, and the header and
    /// footer frames.
    pub(super) fn backdrop_pictures(&mut self, theme: &Theme) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        if let Some(path) = self.look.background.clone() {
            if let Some((texture, canvas)) = self.picture(&path) {
                let size = texture.query();
                let part = crop((size.width, size.height), (width, height));
                let src = Rect::new(
                    (part[0] * size.width as f32) as i32,
                    (part[1] * size.height as f32) as i32,
                    (part[2] * size.width as f32).round().max(1.) as u32,
                    (part[3] * size.height as f32).round().max(1.) as u32,
                );
                canvas.copy(texture, src, None)?;
            }
        }
        let h = |fraction: f32| (fraction * height as f32).round().max(1.) as u32;
        self.framed("header", Rect::new(0, 0, width, h(theme.header_height)))?;
        let footer = h(theme.footer_height);
        self.framed(
            "footer",
            Rect::new(0, (height - footer) as i32, width, footer),
        )?;
        Ok(())
    }
    /// Decorations of one layer: `back` under the list and panels, else over.
    pub(super) fn decor(
        &mut self,
        theme: &Theme,
        decor: &[Decor],
        back: bool,
    ) -> Result<(), String> {
        let Frame {
            width,
            height,
            scale,
            now,
            ..
        } = self.frame;
        let elapsed = now.saturating_duration_since(self.born).as_secs_f32();
        let still = self.motion == Motion::Off;
        for d in decor.iter().filter(|d| d.back == back) {
            let frames = d.frames.max(1);
            let index = if still || d.fps <= 0. || frames == 1 {
                0
            } else {
                let step = (elapsed * d.fps).floor();
                let next = self.born + Duration::from_secs_f32((step + 1.) / d.fps);
                self.look.due = Some(self.look.due.map_or(next, |due| due.min(next)));
                step as u32 % frames
            };
            let k = (d.scale * scale).round().max(1.) as u32;
            let (x, y) = (
                (d.x * width as f32).round() as i32,
                (d.y * height as f32).round() as i32,
            );
            let Some(path) = theme.decor_file(&d.image) else {
                continue;
            };
            if let Some((texture, canvas)) = self.picture(path) {
                let size = texture.query();
                let w = (size.width / frames).max(1);
                canvas.copy(
                    texture,
                    Rect::new((index * w) as i32, 0, w, size.height),
                    Rect::new(x, y, w * k, size.height * k),
                )?;
            }
        }
        Ok(())
    }
    /// The theme's outline, its width in pixels, when the text being drawn
    /// gets it.
    pub(super) fn outline(&self) -> Option<(Outline, i32)> {
        self.look
            .outline
            .filter(|_| self.look.outlining && self.look.on)
            .map(|o| (o, (o.width * self.frame.scale).round().max(1.) as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn pictures_load_once_even_beyond_the_cache_capacity() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("decor")).unwrap();
        let mut decor = String::from("[view.lista.systems]\ndecor = [");
        for n in 0..80 {
            image::RgbaImage::new(4, 4)
                .save(dir.path().join(format!("decor/{n}.png")))
                .unwrap();
            decor += &format!("{{ image = 'decor/{n}.png' }},");
        }
        std::fs::write(dir.path().join("theme.toml"), decor + "]").unwrap();
        let theme = Theme::load(dir.path());
        let (ttf, creator);
        let mut ui = Ui::new(640, 480, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        ui.theme_dir = Some(dir.path().to_path_buf());
        ui.configure(&ttf, &creator, &theme, None).unwrap();
        ui.look(&theme, true);
        assert_eq!(
            ui.look.wanted.len(),
            80,
            "the active view's pictures, once each"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ui.look.wanted.is_empty() && Instant::now() < deadline {
            ui.look_tick();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(ui.look.wanted.is_empty(), "every picture arrived");
        std::thread::sleep(Duration::from_millis(50));
        ui.look_tick();
        assert!(!ui.look_tick(), "nothing left to load: no endless redraws");
    }
}
