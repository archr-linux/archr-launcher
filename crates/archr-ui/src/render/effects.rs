//! Motion that needs whole pictures: the browsing screen is drawn into a
//! texture so it can slide and fade (systems to games, the opening), a
//! picture that gives way is kept to fade out over its successor, and menus
//! and dialogs are drawn apart to grow into place.
use super::{Frame, Ui};
use crate::{
    motion::{Motion, Tween, CROSSFADE, DIALOG, OPENING, SCREEN},
    text::color,
    theme::{Rgba, Theme},
};
use sdl2::{
    pixels::PixelFormatEnum,
    rect::Rect,
    render::{BlendMode, Texture},
};
use std::time::{Duration, Instant};

/// How far a leaving or arriving screen travels.
const SLIDE: f32 = 40.;
/// How far each band rises when the launcher opens, and the bands' stagger.
const RISE: f32 = 8.;
/// Thin bands, so neighbours differ too little to show a seam.
const BANDS: i32 = 24;
const STAGGER: Duration = Duration::from_millis(6);

/// Menus and dialogs that can be up at the same time, each with its own entrance.
#[derive(Clone, Copy)]
pub(crate) enum Pop {
    Menu,
    Dialog,
    Notice,
}
#[derive(Default)]
struct Entrance {
    key: Option<String>,
    shown: bool,
    tween: Option<Tween>,
}

#[derive(Default)]
pub(super) struct Effects<'t> {
    /// This frame's browsing screen, and the previous screen while it leaves.
    page: Option<Texture<'t>>,
    leaving: Option<Texture<'t>>,
    paging: bool,
    screen: Option<bool>,
    slide: Option<(Tween, f32)>,
    opening: Option<Instant>,
    /// The picture that is giving way, where it was, and its fade.
    art: Option<(Texture<'t>, Rect, Tween)>,
    art_key: Option<String>,
    overlay: Option<Texture<'t>>,
    entrances: [Entrance; 3],
}

impl Effects<'_> {
    pub(super) fn active(&self, now: Instant) -> bool {
        self.slide.as_ref().is_some_and(|(t, _)| t.active(now))
            || self.art.as_ref().is_some_and(|(_, _, t)| t.active(now))
            || self.opening.is_some_and(|at| {
                now.saturating_duration_since(at) < OPENING + STAGGER * BANDS as u32
            })
            || self
                .entrances
                .iter()
                .any(|e| e.tween.as_ref().is_some_and(|t| t.active(now)))
    }
    pub(super) fn snap(&mut self) {
        self.slide = None;
        self.art = None;
        self.opening = None;
        for entrance in &mut self.entrances {
            if let Some(tween) = &mut entrance.tween {
                tween.snap();
            }
        }
    }
}

fn target(
    texture: Option<&Texture<'_>>,
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
) -> bool {
    let raw = texture.map_or(std::ptr::null_mut(), Texture::raw);
    // SAFETY: the renderer and the texture (created by its creator) are alive;
    // a null texture selects the window again.
    unsafe { sdl2::sys::SDL_SetRenderTarget(canvas.raw(), raw) == 0 }
}

impl<'t> Ui<'_, 't> {
    fn target_texture(&self, (w, h): (u32, u32)) -> Option<Texture<'t>> {
        if !self.canvas.render_target_supported() {
            return None;
        }
        let mut texture = self
            .creator?
            .create_texture_target(PixelFormatEnum::ARGB8888, w.max(1), h.max(1))
            .ok()?;
        texture.set_blend_mode(BlendMode::Blend);
        Some(texture)
    }
    /// The launcher opens with its screen fading in from the top, band by band.
    pub fn start_opening(&mut self) {
        if self.motion != Motion::Off {
            self.effects.opening = Some(Instant::now());
        }
    }
    /// Starts drawing a browsing screen into the page texture. `art` names
    /// the picture now selected and where it is drawn; when it differs from
    /// the last frame's, the old picture is kept to fade out.
    pub(super) fn begin_page(
        &mut self,
        games: bool,
        art: Option<(String, Rect)>,
        theme: &Theme,
    ) -> Result<(), String> {
        let Frame {
            width, height, now, ..
        } = self.frame;
        let size = (width, height);
        let fits = |t: &Option<Texture<'_>>| {
            t.as_ref()
                .is_some_and(|t| (t.query().width, t.query().height) == size)
        };
        if !fits(&self.effects.page) {
            self.effects.page = self.target_texture(size);
            self.effects.leaving = None;
            self.effects.screen = None;
        }
        let changed = self.effects.screen.is_some_and(|was| was != games);
        if changed && self.effects.page.is_some() {
            let duration = self.motion.fade(SCREEN).max(self.motion.travel(SCREEN));
            if !duration.is_zero() {
                if !fits(&self.effects.leaving) {
                    self.effects.leaving = self.target_texture(size);
                }
                std::mem::swap(&mut self.effects.page, &mut self.effects.leaving);
                let direction = if games { 1. } else { -1. };
                self.effects.slide = Some((Tween::starting(0., 1., duration, now), direction));
                self.effects.art = None;
            }
            if self.effects.page.is_none() {
                self.effects.page = self.target_texture(size);
            }
        }
        self.effects.screen = Some(games);
        // The old picture, cut from the last frame before it is drawn over.
        let key = art.as_ref().map(|(key, _)| key.clone());
        if !changed && key != self.effects.art_key && self.effects.art_key.is_some() {
            self.keep_old_art(art.map(|(_, rect)| rect), now);
        }
        self.effects.art_key = key;
        self.effects.paging =
            target(self.effects.page.as_ref(), &mut self.canvas) && self.effects.page.is_some();
        if !self.effects.paging {
            target(None, &mut self.canvas);
        }
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.clear();
        Ok(())
    }
    fn keep_old_art(&mut self, rect: Option<Rect>, now: Instant) {
        let duration = self.motion.fade(CROSSFADE);
        let reuse = self.effects.art.take().map(|(texture, _, _)| texture);
        let Some(rect) = rect.filter(|_| !duration.is_zero() && self.effects.page.is_some()) else {
            return;
        };
        let kept = reuse
            .filter(|t| (t.query().width, t.query().height) == (rect.width(), rect.height()))
            .or_else(|| self.target_texture((rect.width(), rect.height())));
        let (Some(kept), Some(page)) = (kept, &mut self.effects.page) else {
            return;
        };
        if target(Some(&kept), &mut self.canvas) {
            // The last frame is opaque: a plain copy of its region.
            page.set_alpha_mod(255);
            let copied = self.canvas.copy(page, rect, None).is_ok();
            target(None, &mut self.canvas);
            if copied {
                self.effects.art = Some((kept, rect, Tween::starting(1., 0., duration, now)));
            }
        } else {
            target(None, &mut self.canvas);
        }
    }
    /// The picture that gave way, fading out over the new one.
    pub(super) fn old_art(&mut self) -> Result<(), String> {
        let now = self.frame.now;
        let Some((texture, rect, fade)) = &mut self.effects.art else {
            return Ok(());
        };
        let alpha = fade.value(now);
        if alpha <= 0. {
            self.effects.art = None;
            return Ok(());
        }
        texture.set_alpha_mod((alpha * 255.).round() as u8);
        self.canvas.copy(texture, None, *rect)
    }
    /// Shows the page: sliding against the leaving one, or rising band by
    /// band while the launcher opens.
    pub(super) fn end_page(&mut self, theme: &Theme) -> Result<(), String> {
        if !self.effects.paging {
            return Ok(());
        }
        target(None, &mut self.canvas);
        self.effects.paging = false;
        let Frame {
            width,
            height,
            now,
            scale,
            ..
        } = self.frame;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.clear();
        let full = Rect::new(0, 0, width, height);
        let distance = self.motion.distance(SLIDE) * scale;
        let Some(page) = &mut self.effects.page else {
            return Ok(());
        };
        if let Some((tween, direction)) = self.effects.slide.as_ref().filter(|(t, _)| t.active(now))
        {
            let p = tween.value(now);
            // The old screen leaves in the first 60%, the new one arrives from 30%.
            let out = (p / 0.6).min(1.);
            let arrive = ((p - 0.3) / 0.7).clamp(0., 1.);
            if let Some(leaving) = &mut self.effects.leaving {
                leaving.set_alpha_mod(((1. - out) * 255.).round() as u8);
                let dx = (-direction * distance * out).round() as i32;
                self.canvas
                    .copy(leaving, None, Rect::new(dx, 0, width, height))?;
            }
            page.set_alpha_mod((arrive * 255.).round() as u8);
            let dx = (direction * distance * (1. - arrive)).round() as i32;
            return self
                .canvas
                .copy(page, None, Rect::new(dx, 0, width, height));
        }
        self.effects.slide = None;
        let Some(opened) = self.effects.opening else {
            page.set_alpha_mod(255);
            return self.canvas.copy(page, None, full);
        };
        let elapsed = now.saturating_duration_since(opened);
        let duration = self.motion.fade(OPENING).as_secs_f32().max(0.001);
        let ease = |delay: u32| {
            crate::motion::ease(elapsed.saturating_sub(STAGGER * delay).as_secs_f32() / duration)
        };
        // The screen rises as one piece; a mask lifts band by band from the top.
        let rise = (self.motion.distance(RISE) * scale * (1. - ease(0))).round() as i32;
        page.set_alpha_mod(255);
        self.canvas
            .copy(page, None, Rect::new(0, rise, width, height))?;
        let band = (height as i32 + BANDS - 1) / BANDS;
        let mut done = true;
        for n in 0..BANDS {
            let t = ease(n as u32);
            done &= t >= 1.;
            let veil = Rgba {
                a: ((1. - t) * 255.).round() as u8,
                ..theme.background
            };
            self.canvas.set_draw_color(color(veil));
            self.canvas
                .fill_rect(Rect::new(0, n * band, width, band as u32))?;
        }
        if done {
            self.effects.opening = None;
        }
        Ok(())
    }
    /// Draws a menu or dialog: the scrim fades in and the rest grows from
    /// 94% while it fades in. `key` tells one dialog from the next.
    pub(crate) fn pop_in(
        &mut self,
        pop: Pop,
        key: &str,
        (theme, scrim): (&Theme, f32),
        draw: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let Frame {
            width, height, now, ..
        } = self.frame;
        let fade = self.motion.fade(DIALOG);
        let entrance = &mut self.effects.entrances[pop as usize];
        entrance.shown = true;
        if entrance.key.as_deref() != Some(key) {
            entrance.key = Some(key.to_owned());
            entrance.tween = Some(Tween::starting(0., 1., fade, now));
        }
        let p = entrance.tween.as_ref().map_or(1., |t| t.value(now));
        self.scrim(theme, scrim * p)?;
        if p >= 1. {
            return draw(self);
        }
        if !self
            .effects
            .overlay
            .as_ref()
            .is_some_and(|t| (t.query().width, t.query().height) == (width, height))
        {
            self.effects.overlay = self.target_texture((width, height));
        }
        let Some(overlay) = self.effects.overlay.take() else {
            return draw(self);
        };
        if !target(Some(&overlay), &mut self.canvas) {
            target(None, &mut self.canvas);
            self.effects.overlay = Some(overlay);
            return draw(self);
        }
        self.canvas.set_draw_color(color(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }));
        self.canvas.clear();
        let drawn = draw(self);
        target(None, &mut self.canvas);
        let mut overlay = overlay;
        let grow = if self.motion == Motion::On {
            0.94 + 0.06 * p
        } else {
            1.
        };
        let (w, h) = (width as f32 * grow, height as f32 * grow);
        let dst = Rect::new(
            ((width as f32 - w) / 2.).round() as i32,
            ((height as f32 - h) / 2.).round() as i32,
            w.round() as u32,
            h.round() as u32,
        );
        overlay.set_alpha_mod((p * 255.).round() as u8);
        let copied = self.canvas.copy(&overlay, None, dst);
        self.effects.overlay = Some(overlay);
        drawn.and(copied)
    }
    /// Menus not drawn this frame enter afresh next time.
    pub(super) fn settle_entrances(&mut self) {
        for entrance in &mut self.effects.entrances {
            if !std::mem::take(&mut entrance.shown) {
                entrance.key = None;
                entrance.tween = None;
            }
        }
    }
}
