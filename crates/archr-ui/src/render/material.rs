//! The style primitives of 3.1 themes (docs/temas.md, Estilo): glass over
//! a blurred background, layout panels shaded or of glass, elements drawn
//! at an opacity and the lifted focus. Nothing here runs for a theme that
//! does not ask for it.
use super::{effects::target, Frame, Ui};
use crate::{
    layout::{Panel, PanelStyle},
    primitives::shade,
    text::color,
    theme::{Glass, Rgba, Theme},
};
use sdl2::{
    rect::Rect,
    render::{BlendMode, ScaleMode, Texture},
};
use std::path::{Path, PathBuf};

/// Blurred backgrounds kept at once: the theme's and a few systems'.
const BLURS: usize = 4;

/// The picture under the browsing screen this frame, as `backdrop_pictures`
/// drew it: its path, whether it is a system's (under a curtain) and the
/// part of it that fills the screen.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Behind {
    pub path: PathBuf,
    pub curtain: Option<Rgba>,
    pub src: Rect,
}

/// Textures the materials keep between frames.
#[derive(Default)]
pub(super) struct Materials<'t> {
    /// Each background reduced `halvings` times, made once per picture.
    blurs: Vec<(PathBuf, u32, Texture<'t>)>,
    /// Where an element drawn at an opacity is drawn first.
    layer: Option<Texture<'t>>,
    pub behind: Option<Behind>,
}

/// Halvings that blur by about `px` screen pixels: each one averages 2×2.
pub(super) fn halvings(px: f32) -> u32 {
    (px.max(1.).log2().round() as u32).clamp(1, 6)
}

/// The draw target, viewport and clip in force, to put back afterwards.
struct Saved {
    target: *mut sdl2::sys::SDL_Texture,
    viewport: Rect,
    clip: sdl2::render::ClippingRect,
}
impl Saved {
    fn take(canvas: &sdl2::render::Canvas<sdl2::video::Window>) -> Self {
        Self {
            // SAFETY: the renderer is alive; this only reads its target.
            target: unsafe { sdl2::sys::SDL_GetRenderTarget(canvas.raw()) },
            viewport: canvas.viewport(),
            clip: canvas.clip_rect(),
        }
    }
    fn restore(&self, canvas: &mut sdl2::render::Canvas<sdl2::video::Window>) {
        // SAFETY: the renderer is alive and the saved target, if any, is a
        // texture of it that outlives this call.
        unsafe { sdl2::sys::SDL_SetRenderTarget(canvas.raw(), self.target) };
        canvas.set_viewport(self.viewport);
        canvas.set_clip_rect(self.clip);
    }
}

impl<'t> Ui<'_, 't> {
    /// A layout panel: the theme's `name` frame, its glass, a gradient or
    /// the plain raised card, at the panel's opacity.
    pub(super) fn layout_panel(
        &mut self,
        name: &str,
        panel: &Panel,
        theme: &Theme,
    ) -> Result<Rect, String> {
        let rect = self.area(panel.rect);
        let radius = self.frame.size(panel.radius);
        self.faded(panel.opacity, |ui| match (panel.style, panel.gradient) {
            (PanelStyle::Plain, None) => {
                ui.panel_named(name, rect, radius, (theme.surface, theme.border))
            }
            _ if ui.framed(name, rect)? => Ok(()),
            (PanelStyle::Glass, _) => ui.glass(rect, radius, &theme.glass_or_default()),
            (PanelStyle::Plain, Some(gradient)) => {
                ui.cast_shadow(rect, radius)?;
                ui.geometry
                    .shaded(&mut ui.canvas, rect, radius, &gradient, Some(theme.border))
            }
        })?;
        Ok(rect)
    }
    /// The browsing screen's background, when the theme shades it.
    pub(super) fn shaded_background(&mut self, theme: &Theme) -> Result<(), String> {
        let Some(gradient) = theme.gradient else {
            return Ok(());
        };
        let Frame { width, height, .. } = self.frame;
        shade(&mut self.canvas, Rect::new(0, 0, width, height), &gradient)
    }
    /// A pane of `glass` over `rect`: the background behind it blurred (when
    /// the glass blurs and a picture is there), its fill, a 1 px border and
    /// a 1 px highlight along the top.
    pub(super) fn glass(&mut self, rect: Rect, radius: f32, glass: &Glass) -> Result<(), String> {
        if glass.blur > 0. {
            let steps = halvings(self.frame.size(glass.blur));
            let behind = self.materials.behind.clone();
            let made = behind.as_ref().and_then(|behind| {
                self.blurred(&behind.path, steps, |ui| {
                    ui.smooth_copy(&behind.path, behind.src)
                })
            });
            if let Some(index) = made {
                let Frame { width, height, .. } = self.frame;
                let crop = [
                    rect.x() as f32 / width as f32,
                    rect.y() as f32 / height as f32,
                    rect.width() as f32 / width as f32,
                    rect.height() as f32 / height as f32,
                ];
                let texture = &self.materials.blurs[index].2;
                self.geometry
                    .cover(&mut self.canvas, texture, rect, radius, 255, crop)?;
                if let Some(curtain) = self.materials.behind.as_ref().and_then(|b| b.curtain) {
                    self.rounded(rect, radius, curtain, None)?;
                }
            }
        }
        self.geometry.rounded_lined(
            &mut self.canvas,
            rect,
            radius,
            glass.fill,
            (glass.border, 1),
        )?;
        // The highlight runs between the top corners, a pixel inside.
        let inset = (radius * self.geometry.corners()).ceil() as i32;
        let w = rect.width() as i32 - 2 * inset;
        if w > 0 && glass.highlight.a > 0 {
            self.canvas.set_draw_color(color(glass.highlight));
            self.canvas
                .fill_rect(Rect::new(rect.x() + inset, rect.y() + 1, w as u32, 1))?;
        }
        Ok(())
    }
    /// The blurred copy of the picture behind the screen, made the first
    /// time it is asked for: reduced `steps` times, each a linear 2×2
    /// average, then grown back up to two halvings short of the screen,
    /// each step a tent of four taps (a dual filter), so the screen-sized
    /// copy drawn from it is smooth instead of a mesh of linear creases.
    /// Never once a frame. `first` draws the picture at `path` over the
    /// whole target.
    pub(super) fn blurred(
        &mut self,
        path: &Path,
        steps: u32,
        first: impl FnOnce(&mut Self) -> Result<bool, String>,
    ) -> Option<usize> {
        if let Some(index) = self
            .materials
            .blurs
            .iter()
            .position(|(made, n, _)| made == path && *n == steps)
        {
            return Some(index);
        }
        let mut first = Some(first);
        let Frame { width, height, .. } = self.frame;
        let saved = Saved::take(&self.canvas);
        let mut current: Option<Texture<'t>> = None;
        let (mut w, mut h) = (width, height);
        for _ in 0..steps {
            (w, h) = ((w / 2).max(1), (h / 2).max(1));
            let next = self.target_texture((w, h))?;
            if !target(Some(&next), &mut self.canvas) {
                saved.restore(&mut self.canvas);
                return None;
            }
            self.canvas.set_viewport(None);
            self.canvas.set_clip_rect(None);
            self.canvas.set_draw_color(color(Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            }));
            self.canvas.clear();
            let copied = match current.as_mut() {
                Some(previous) => {
                    previous.set_blend_mode(BlendMode::None);
                    self.canvas.copy(previous, None, None).map(|()| true)
                }
                None => match first.take() {
                    Some(first) => first(self),
                    None => Ok(false),
                },
            };
            if copied != Ok(true) {
                saved.restore(&mut self.canvas);
                return None;
            }
            let mut next = next;
            next.set_scale_mode(ScaleMode::Linear);
            current = Some(next);
        }
        // Back up, each step twice the size, four half-texel taps averaged.
        let kept = steps.min(2);
        for _ in kept..steps {
            let previous = current.as_mut()?;
            (w, h) = (w * 2, h * 2);
            let next = self.target_texture((w, h))?;
            if !target(Some(&next), &mut self.canvas) {
                saved.restore(&mut self.canvas);
                return None;
            }
            self.canvas.set_viewport(None);
            self.canvas.set_clip_rect(None);
            for (n, (dx, dy)) in [(-1, -1), (1, 1), (1, -1), (-1, 1)].into_iter().enumerate() {
                // The first replaces, the others blend in at 1/2, 1/3, 1/4.
                let mode = if n == 0 {
                    BlendMode::None
                } else {
                    BlendMode::Blend
                };
                previous.set_blend_mode(mode);
                previous.set_alpha_mod((255. / (n + 1) as f32).round() as u8);
                let dst = Rect::new(dx - 1, dy - 1, w + 2, h + 2);
                if self.canvas.copy(previous, None, dst).is_err() {
                    saved.restore(&mut self.canvas);
                    return None;
                }
            }
            let mut next = next;
            next.set_scale_mode(ScaleMode::Linear);
            current = Some(next);
        }
        saved.restore(&mut self.canvas);
        let mut blurred = current?;
        blurred.set_alpha_mod(255);
        blurred.set_blend_mode(BlendMode::Blend);
        let blurs = &mut self.materials.blurs;
        if blurs.len() == BLURS {
            blurs.remove(0);
        }
        blurs.push((path.to_path_buf(), steps, blurred));
        Some(blurs.len() - 1)
    }
    /// Forgets the blurred copy kept as `path`, so it is made again.
    pub(super) fn forget_blur(&mut self, path: &Path) {
        self.materials.blurs.retain(|(made, _, _)| made != path);
    }
    /// The blurred picture `index` (from `blurred`) drawn over `dst`, at `alpha`.
    pub(super) fn draw_blurred(
        &mut self,
        index: usize,
        dst: Rect,
        alpha: u8,
    ) -> Result<(), String> {
        let Some((_, _, texture)) = self.materials.blurs.get_mut(index) else {
            return Ok(());
        };
        texture.set_alpha_mod(alpha);
        let drawn = self.canvas.copy(texture, None, dst);
        texture.set_alpha_mod(255);
        drawn
    }
    /// Runs `draw` so that what it draws shows at `opacity`: straight to
    /// the screen when whole, else into a layer laid over it.
    pub(super) fn faded<T>(
        &mut self,
        opacity: f32,
        draw: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        if opacity >= 1. {
            return draw(self);
        }
        let Frame { width, height, .. } = self.frame;
        let fits = |t: &Option<Texture<'_>>| {
            t.as_ref()
                .is_some_and(|t| (t.query().width, t.query().height) == (width, height))
        };
        if !fits(&self.materials.layer) {
            self.materials.layer = self.target_texture((width, height));
        }
        let Some(layer) = self.materials.layer.take() else {
            return draw(self);
        };
        let saved = Saved::take(&self.canvas);
        if !target(Some(&layer), &mut self.canvas) {
            saved.restore(&mut self.canvas);
            self.materials.layer = Some(layer);
            return draw(self);
        }
        self.canvas.set_viewport(None);
        self.canvas.set_draw_color(color(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }));
        self.canvas.clear();
        self.canvas.set_viewport(saved.viewport);
        self.canvas.set_clip_rect(saved.clip);
        let drawn = draw(self);
        saved.restore(&mut self.canvas);
        let mut layer = layer;
        layer.set_alpha_mod((opacity.clamp(0., 1.) * 255.).round() as u8);
        self.canvas.set_viewport(None);
        self.canvas.set_clip_rect(None);
        let copied = self.canvas.copy(&layer, None, None);
        self.canvas.set_viewport(saved.viewport);
        self.canvas.set_clip_rect(saved.clip);
        self.materials.layer = Some(layer);
        drawn.and_then(|value| copied.map(|()| value))
    }
    /// The focused item under `focus = "lift"`: `rect` grown by the theme's
    /// `focus_scale` (as far as `focus`, 0 to 1, has come), with the soft
    /// shadow under it. Returns the grown box to draw the item in; `ring`
    /// then outlines it.
    pub(super) fn lift_focus(
        &mut self,
        rect: Rect,
        radius: f32,
        focus: f32,
        theme: &Theme,
    ) -> Result<Rect, String> {
        let grow = 1. + (theme.focus_scale - 1.) * focus.clamp(0., 1.);
        let (w, h) = (rect.width() as f32 * grow, rect.height() as f32 * grow);
        let grown = Rect::new(
            (rect.center().x() as f32 - w / 2.).round() as i32,
            (rect.center().y() as f32 - h / 2.).round() as i32,
            w.round().max(1.) as u32,
            h.round().max(1.) as u32,
        );
        if focus > 0. {
            self.cast_shadow(grown, radius * grow)?;
        }
        Ok(grown)
    }
    /// The ring around a lifted focus: the theme's `focus_ring`, else the
    /// accent 2 px wide.
    pub(super) fn focus_ring(
        &mut self,
        rect: Rect,
        radius: f32,
        theme: &Theme,
    ) -> Result<(), String> {
        let (ink, width) = theme
            .focus_ring
            .map_or((theme.highlight_bg, 2.), |ring| (ring.color, ring.width));
        if width <= 0. {
            return Ok(());
        }
        let width = width * self.frame.scale;
        self.geometry
            .ring(&mut self.canvas, rect, radius, width, ink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::shading;
    use crate::theme::Gradient;
    use sdl2::pixels::PixelFormatEnum;

    fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba { r, g, b, a }
    }
    /// The screen's RGBA bytes at (x, y).
    fn pixel(ui: &Ui<'_, '_>, x: usize, y: usize) -> [u8; 4] {
        let width = ui.frame.width as usize;
        let bytes = ui
            .canvas
            .read_pixels(None, PixelFormatEnum::RGBA32)
            .unwrap();
        let i = (y * width + x) * 4;
        [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
    }

    #[test]
    fn gradients_run_along_their_angle() {
        let rect = Rect::new(0, 0, 100, 50);
        let across = shading(
            rect,
            &Gradient {
                from: rgba(0, 0, 0, 255),
                to: rgba(200, 0, 0, 255),
                angle: 0.,
            },
        );
        assert_eq!(across((0., 25.)).r, 0);
        assert_eq!(across((50., 0.)).r, 100);
        assert_eq!(across((100., 50.)).r, 200);
        let down = shading(
            rect,
            &Gradient {
                from: rgba(0, 0, 0, 0),
                to: rgba(0, 0, 0, 200),
                angle: 90.,
            },
        );
        assert_eq!((down((100., 0.)).a, down((0., 50.)).a), (0, 200));
    }

    #[test]
    fn the_blur_is_made_once_per_picture_and_softens_it() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let (ttf, creator);
        let mut ui = Ui::new(64, 48, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        ui.configure(&ttf, &creator, &Theme::builtin(), None)
            .unwrap();
        // One-pixel stripes: as sharp as a picture gets.
        let mut stripes = sdl2::surface::Surface::new(64, 48, PixelFormatEnum::RGBA32).unwrap();
        stripes.with_lock_mut(|bytes| {
            for (n, px) in bytes.chunks_mut(4).enumerate() {
                let v = if n % 2 == 0 { 255 } else { 0 };
                px.copy_from_slice(&[v, v, v, 255]);
            }
        });
        let mut picture = creator.create_texture_from_surface(stripes).unwrap();
        picture.set_scale_mode(ScaleMode::Linear);
        let path = Path::new("Stripes.png");
        let mut made = 0;
        let mut draw = |ui: &mut Ui<'_, '_>| {
            made += 1;
            ui.canvas.copy(&picture, None, None).map(|()| true)
        };
        let first = ui.blurred(path, 3, &mut draw).unwrap();
        assert_eq!(ui.blurred(path, 3, &mut draw), Some(first), "kept");
        assert_eq!(made, 1, "made once, never again for this picture");
        let size = ui.materials.blurs[first].2.query();
        assert_eq!(
            (size.width, size.height),
            (16, 12),
            "reduced three times, grown back once"
        );
        // Drawn back over the screen, the stripes are an even grey.
        ui.canvas.set_draw_color(sdl2::pixels::Color::BLACK);
        ui.canvas.clear();
        ui.canvas
            .copy(&ui.materials.blurs[first].2, None, None)
            .unwrap();
        for x in [29, 30, 31] {
            let [r, ..] = pixel(&ui, x, 20);
            assert!((100..=155).contains(&r), "{x}: {r}");
        }
        // A picture that is not loaded yet gives no blur and keeps nothing.
        assert_eq!(ui.blurred(Path::new("Later.png"), 3, |_| Ok(false)), None);
        assert_eq!(ui.materials.blurs.len(), 1);
    }

    #[test]
    fn faded_elements_show_through_and_whole_ones_draw_straight() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let (ttf, creator);
        let mut ui = Ui::new(64, 48, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        ui.configure(&ttf, &creator, &Theme::builtin(), None)
            .unwrap();
        let white = rgba(255, 255, 255, 255);
        for (opacity, low, high) in [(1., 255, 255), (0.5, 110, 145), (0., 0, 0)] {
            ui.canvas.set_draw_color(sdl2::pixels::Color::BLACK);
            ui.canvas.clear();
            ui.faded(opacity, |ui| {
                ui.rounded(Rect::new(10, 10, 30, 20), 0., white, None)
            })
            .unwrap();
            let [r, ..] = pixel(&ui, 20, 15);
            assert!((low..=high).contains(&r), "{opacity}: {r}");
            assert_eq!(pixel(&ui, 50, 40)[0], 0, "around it, untouched");
        }
        assert!(
            ui.materials.layer.is_some(),
            "one layer, kept for next time"
        );
    }

    #[test]
    fn a_lifted_focus_grows_and_its_ring_takes_the_theme_ink() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let (ttf, creator);
        let mut ui = Ui::new(160, 120, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        let mut theme = Theme::builtin();
        ui.configure(&ttf, &creator, &theme, None).unwrap();
        theme.focus_scale = 1.1;
        theme.focus_ring = Some(crate::theme::Ring {
            color: rgba(255, 0, 0, 255),
            width: 2.,
        });
        let rect = Rect::new(40, 30, 40, 40);
        ui.canvas.set_draw_color(sdl2::pixels::Color::WHITE);
        ui.canvas.clear();
        let grown = ui.lift_focus(rect, 0., 1., &theme).unwrap();
        assert_eq!((grown.width(), grown.height()), (44, 44));
        assert_eq!(grown.center(), rect.center());
        assert_eq!(
            ui.lift_focus(rect, 0., 0., &theme).unwrap(),
            rect,
            "not yet focused"
        );
        assert!(pixel(&ui, 60, 75)[0] < 255, "the shadow under it");
        let inside = pixel(&ui, 60, 50);
        ui.focus_ring(grown, 0., &theme).unwrap();
        assert_eq!(pixel(&ui, grown.x() as usize, 50)[..3], [255, 0, 0]);
        assert_eq!(pixel(&ui, 60, 50), inside, "only a ring");
    }

    #[test]
    fn a_new_focus_rises_on_the_motion_and_the_first_is_up_at_once() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let mut ui = Ui::new(160, 120, true).unwrap();
        ui.set_motion(crate::motion::Motion::On);
        assert_eq!(ui.lifting("first"), 1., "the screen opens on it lifted");
        assert!(ui.lifting("next") < 0.05, "a new focus starts low");
        ui.frame.now += std::time::Duration::from_millis(100);
        let rising = ui.lifting("next");
        assert!((0.2..1.).contains(&rising), "{rising}");
        ui.frame.now += std::time::Duration::from_millis(400);
        assert_eq!(ui.lifting("next"), 1., "settled");
        ui.set_motion(crate::motion::Motion::Off);
        assert_eq!(ui.lifting("still"), 1., "no motion: up at once");
    }
}
