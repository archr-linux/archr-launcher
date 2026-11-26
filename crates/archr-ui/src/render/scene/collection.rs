//! A free screen's carousels and grids (docs/temas.md, Telas livres,
//! Coleções): the screen's items drawn from the theme's model of one, the
//! one in focus larger, brighter, tinted or framed. A carousel glides to the
//! next item with the launcher's list motion; a grid scrolls by rows and its
//! focus grows in. Animations off or reduced: everything moves at once.
use super::*;
use crate::layout::{Axis, Badge};

/// Where one item is drawn this frame.
pub(super) struct Spot {
    pub index: usize,
    /// Its middle and its box at rest, in pixels.
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    /// How far from the focus (0 on it), how much in focus (0 to 1), its
    /// size against the box at rest, opacity and turn.
    away: f32,
    focus: f32,
    scale: f32,
    alpha: f32,
    degrees: f32,
}

/// How many items the screen has: its systems, or the rows of its games list.
fn items(backdrop: &Screen, rows: &[String], data: &dyn DataSource) -> usize {
    match backdrop {
        Screen::Games { system, folder } => App::list_len(data, system, folder),
        _ => rows.len(),
    }
}
/// The list a collection walks, for its motion.
pub(super) fn key(backdrop: &Screen) -> String {
    match backdrop {
        Screen::Games { system, folder } => format!("@scene:{system}/{folder}"),
        _ => "@scene:systems".to_owned(),
    }
}
/// `a` on the way to `b`.
fn mix(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

impl<'t> Ui<'_, 't> {
    /// The items `c` shows now in `area`, the farthest from the focus first,
    /// each with its data; `opacity` is the items' out of focus.
    pub(super) fn spots<'d>(
        &mut self,
        c: &Collection,
        (area, opacity): (Rect, f32),
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
        data: &'d dyn DataSource,
        theme: &Theme,
    ) -> Vec<(Spot, Facts<'d>)> {
        let len = items(backdrop, rows, data);
        if len == 0 {
            return Vec::new();
        }
        let cursor = cursor.min(len - 1);
        let key = key(backdrop);
        let mut spots = match c.shape {
            Shape::Carousel {
                axis,
                visible,
                align,
                wheel,
            } => {
                let at = self.glide(&key, cursor, len, c.wrap);
                let screen = (self.frame.width as f32, self.frame.height as f32);
                carousel(
                    c,
                    (axis, visible, align, wheel),
                    (area, opacity),
                    (at, len),
                    screen,
                )
            }
            Shape::Grid { columns, rows } => {
                self.grid(c, (columns, rows), (area, opacity), &key, cursor, len)
            }
        };
        spots.sort_by(|a, b| b.away.total_cmp(&a.away));
        spots
            .into_iter()
            .map(|spot| {
                let facts = self.facts(backdrop, (rows, spot.index), data, theme);
                (spot, facts)
            })
            .collect()
    }

    /// Where a carousel stands, in items: gliding to `cursor`, counting
    /// every turn of a loop so the last item steps on to the first.
    fn glide(&mut self, key: &str, cursor: usize, len: usize, wrap: bool) -> f32 {
        let now = self.frame.now;
        let same = self.list.key == key
            && self
                .scene_cache
                .glide
                .as_ref()
                .is_some_and(|(k, _, _)| k == key);
        if !same {
            self.scene_cache.glide = Some((key.to_owned(), cursor, cursor as f32));
            key.clone_into(&mut self.list.key);
            self.list.pill.retarget(cursor as f32, now);
            self.list.pill.snap();
        } else if let Some((_, last, at)) = self
            .scene_cache
            .glide
            .as_mut()
            .filter(|(_, last, _)| *last != cursor)
        {
            let mut step = cursor as f32 - *last as f32;
            if wrap && step.abs() > len as f32 / 2. {
                step -= step.signum() * len as f32;
            }
            *at += step;
            *last = cursor;
            self.list.pill.retarget(*at, now);
        }
        self.list.pill.value(now)
    }

    /// A grid's cells in view: the rows scroll only when the focus leaves
    /// them, and the focused item grows in.
    fn grid(
        &mut self,
        c: &Collection,
        (columns, shown): (usize, usize),
        (area, opacity): (Rect, f32),
        key: &str,
        cursor: usize,
        len: usize,
    ) -> Vec<Spot> {
        let now = self.frame.now;
        let tall = self.frame.height as f32;
        let gap = c.spacing.unwrap_or(0.02) * tall;
        let (columns, shown) = (columns.max(1), shown.max(1));
        let cw = ((area.width() as f32 - gap * (columns - 1) as f32) / columns as f32).max(1.);
        let ch = ((area.height() as f32 - gap * (shown - 1) as f32) / shown as f32).max(1.);
        let row = cursor / columns;
        let total = len.div_ceil(columns);
        let fresh = self.list.key != key;
        let before = if fresh {
            0
        } else {
            self.list.scroll.target.round().max(0.) as usize
        };
        let top = before
            .min(row)
            .max((row + 1).saturating_sub(shown))
            .min(total.saturating_sub(shown));
        self.list.scroll.retarget(top as f32, now);
        if fresh {
            key.clone_into(&mut self.list.key);
            self.list.scroll.snap();
        }
        let offset = self.list.scroll.value(now);
        let grown = match &self.scene_cache.focus {
            Some((k, i, tween)) if k == key && *i == cursor => tween.value(now),
            _ => {
                let tween = if fresh {
                    Tween::new(1., Duration::ZERO)
                } else {
                    Tween::starting(0., 1., self.motion.travel(SELECT), now)
                };
                let value = tween.value(now);
                self.scene_cache.focus = Some((key.to_owned(), cursor, tween));
                value
            }
        };
        let mut spots = Vec::new();
        let first = offset.floor().max(0.) as usize;
        let last = ((offset + shown as f32).ceil() as usize).min(total);
        for r in first..last {
            let place = r as f32 - offset;
            // A row sliding out of the box fades as it goes.
            let fade = (1. - (-place).max(place - (shown - 1) as f32).max(0.)).clamp(0., 1.);
            for col in 0..columns {
                let index = r * columns + col;
                if index >= len {
                    break;
                }
                let focus = if index == cursor { grown } else { 0. };
                spots.push(Spot {
                    index,
                    x: area.x() as f32 + col as f32 * (cw + gap) + cw / 2.,
                    y: area.y() as f32 + place * (ch + gap) + ch / 2.,
                    w: cw,
                    h: ch,
                    away: f32::from(index != cursor),
                    focus,
                    scale: mix(1., c.selected.scale, focus),
                    alpha: mix(opacity, c.selected.opacity, focus) * fade,
                    degrees: 0.,
                });
            }
        }
        spots
    }

    /// The collection's items where `spots` put them.
    /// A grid keeps to its box, with room for the focus to grow: the rows
    /// scrolling in and out are cut at its edges.
    pub(super) fn collection(
        &mut self,
        c: &Collection,
        spots: &[(Spot, Facts<'_>)],
        area: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        if let Shape::Grid { rows, .. } = c.shape {
            let cell = area.height() as f32 / rows.max(1) as f32;
            let ring = match &c.selected.frame {
                Some(Ring::Line { width, .. }) => width * self.frame.height as f32,
                _ => 0.,
            };
            let room = ((c.selected.scale - 1.).max(0.) * cell / 2. + ring).ceil() as i32 + 1;
            self.canvas.set_clip_rect(Rect::new(
                area.x() - room,
                area.y() - room,
                area.width() + 2 * room as u32,
                area.height() + 2 * room as u32,
            ));
        }
        let drawn = spots
            .iter()
            .filter(|(spot, _)| spot.alpha > 0.004)
            .try_for_each(|(spot, facts)| self.item(c, spot, facts, theme));
        self.canvas.set_clip_rect(None);
        drawn
    }

    fn item(
        &mut self,
        c: &Collection,
        spot: &Spot,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let (w, h) = (spot.w * spot.scale, spot.h * spot.scale);
        let cell = Rect::new(
            (spot.x - w / 2.).round() as i32,
            (spot.y - h / 2.).round() as i32,
            w.round().max(1.) as u32,
            h.round().max(1.) as u32,
        );
        let tall = self.frame.height as f32;
        let fade = |c: Rgba, k: f32| Rgba {
            a: (f32::from(c.a) * k).round() as u8,
            ..c
        };
        if let Some(fill) = c.item.fill {
            self.rounded(cell, c.item.radius * tall, fade(fill, spot.alpha), None)?;
        }
        let paths = self.files(&c.item.image, facts);
        let Some(picture) = self.item_picture(c, spot, cell, paths.first())? else {
            let missing = paths.first().is_none_or(|k| {
                self.scene_cache
                    .pictures
                    .as_ref()
                    .is_some_and(|(_, _, p)| p.has(k) && p.peek(k).is_none())
            });
            if missing {
                // The name turns to the focus's color as the focus comes.
                let ink = c.item.text.color.unwrap_or(theme.text);
                let ink = match c.selected.color.or(c.selected.tint) {
                    Some(to) => {
                        let k = spot.focus.clamp(0., 1.);
                        let channel =
                            |a: u8, b: u8| mix(f32::from(a), f32::from(b), k).round() as u8;
                        Rgba {
                            r: channel(ink.r, to.r),
                            g: channel(ink.g, to.g),
                            b: channel(ink.b, to.b),
                            a: channel(ink.a, to.a),
                        }
                    }
                    None => ink,
                };
                self.item_name(&c.item.text, facts, spot, c.selected.scale.max(1.), ink)?;
            }
            return Ok(());
        };
        // The game's logo over its picture.
        let logo = self.files(&c.item.logo, facts);
        if let Some(path) = logo.first() {
            let inset = Rect::new(
                picture.x() + picture.width() as i32 / 10,
                picture.y() + picture.height() as i32 / 10,
                (picture.width() * 4 / 5).max(1),
                (picture.height() * 4 / 5).max(1),
            );
            self.logo_over(path, inset, spot.alpha)?;
        }
        let favorite = facts.game.is_some_and(|g| g.favorite);
        if let Some(badge) = c.favorite.as_ref().filter(|_| favorite) {
            self.badge(badge, picture, spot.alpha, facts, theme)?;
        }
        Ok(())
    }

    /// The item's picture in `cell`, with the focus's tint and frame, its
    /// reflection and its turn; the box it took, once it is loaded.
    fn item_picture(
        &mut self,
        c: &Collection,
        spot: &Spot,
        cell: Rect,
        key: Option<&PathBuf>,
    ) -> Result<Option<Rect>, String> {
        let Some(key) = key else {
            return Ok(None);
        };
        let tall = self.frame.height as f32;
        let size = match self
            .scene_cache
            .pictures
            .as_ref()
            .and_then(|(_, _, p)| p.peek(key))
        {
            Some(texture) => texture.query(),
            None => return Ok(None),
        };
        let (tw, th) = (size.width.max(1) as f32, size.height.max(1) as f32);
        let (dst, part) = match c.item.fit {
            ImageFit::Stretch => (cell, WHOLE),
            ImageFit::Cover => (
                cell,
                crop((size.width, size.height), (cell.width(), cell.height())),
            ),
            ImageFit::Contain => {
                let k = (cell.width() as f32 / tw).min(cell.height() as f32 / th);
                let (w, h) = ((tw * k).round().max(1.), (th * k).round().max(1.));
                let x = cell.x() + ((cell.width() as f32 - w) / 2.).round() as i32;
                let y = cell.y() + ((cell.height() as f32 - h) / 2.).round() as i32;
                (Rect::new(x, y, w as u32, h as u32), WHOLE)
            }
        };
        let radius = c.item.radius * tall;
        // A line frame lies behind the picture, grown around it.
        if let Some(Ring::Line {
            color,
            width,
            radius: round,
        }) = c.selected.frame.as_ref().filter(|_| spot.focus > 0.)
        {
            let k = (width * tall).round().max(1.) as i32;
            let around = Rect::new(
                dst.x() - k,
                dst.y() - k,
                dst.width() + 2 * k as u32,
                dst.height() + 2 * k as u32,
            );
            let a = (f32::from(color.a) * spot.focus * spot.alpha).round() as u8;
            let ink = Rgba { a, ..*color };
            self.rounded(around, (round * tall).max(radius + k as f32), ink, None)?;
        }
        let alpha = (spot.alpha * 255.).round() as u8;
        let tint = c.selected.tint.map(|t| {
            let k = spot.focus;
            let channel = |v: u8| mix(255., f32::from(v), k).round() as u8;
            (channel(t.r), channel(t.g), channel(t.b))
        });
        let (pictures, canvas, geometry) = match &mut self.scene_cache.pictures {
            Some((_, _, cache)) => (cache, &mut self.canvas, &mut self.geometry),
            None => return Ok(None),
        };
        let Some(texture) = pictures.get_mut(key) else {
            return Ok(None);
        };
        if let Some((r, g, b)) = tint {
            texture.set_color_mod(r, g, b);
        }
        let src = Rect::new(
            (part[0] * tw) as i32,
            (part[1] * th) as i32,
            (part[2] * tw).round().max(1.) as u32,
            (part[3] * th).round().max(1.) as u32,
        );
        let drawn = if radius > 0. && spot.degrees == 0. {
            geometry.cover(canvas, texture, dst, radius, alpha, part)
        } else {
            texture.set_alpha_mod(alpha);
            canvas.copy_ex(
                texture,
                src,
                dst,
                f64::from(spot.degrees),
                None,
                false,
                false,
            )
        };
        // The picture's lower half again, upside down under it, fading
        // away in bands.
        if c.reflection > 0. && spot.degrees == 0. && drawn.is_ok() {
            const BANDS: u32 = 6;
            let (sh, dh) = ((src.height() / 2).max(1), (dst.height() / 2).max(1));
            for band in 0..BANDS {
                // Band n of the reflection, from the picture's edge down,
                // shows the source from its bottom edge up.
                let (d0, d1) = (dh * band / BANDS, dh * (band + 1) / BANDS);
                let (s0, s1) = (sh * band / BANDS, sh * (band + 1) / BANDS);
                if d1 <= d0 || s1 <= s0 {
                    continue;
                }
                let from = Rect::new(src.x(), src.bottom() - s1 as i32, src.width(), s1 - s0);
                let to = Rect::new(dst.x(), dst.bottom() + 1 + d0 as i32, dst.width(), d1 - d0);
                let k = c.reflection * (1. - (band as f32 + 0.5) / BANDS as f32);
                texture.set_alpha_mod((f32::from(alpha) * k).round() as u8);
                canvas.copy_ex(texture, from, to, 0., None, false, true)?;
            }
        }
        texture.set_alpha_mod(255);
        texture.set_color_mod(255, 255, 255);
        drawn?;
        // A picture frame lies over it.
        if let Some(Ring::Slices(slices)) = c.selected.frame.as_ref().filter(|_| spot.focus > 0.5) {
            let path = slices.image.fill(|_| String::new(), |k| self.phrase(k));
            if let Some(path) = self.theme_file(&path) {
                self.slices(&path, slices, dst)?;
            }
        }
        Ok(Some(dst))
    }

    /// A loaded picture inside `rect`, whole and centered.
    fn logo_over(&mut self, path: &PathBuf, rect: Rect, opacity: f32) -> Result<(), String> {
        let (pictures, canvas) = match &mut self.scene_cache.pictures {
            Some((_, _, cache)) => (cache, &mut self.canvas),
            None => return Ok(()),
        };
        let Some(texture) = pictures.get_mut(path) else {
            return Ok(());
        };
        let size = texture.query();
        let (tw, th) = (size.width.max(1) as f32, size.height.max(1) as f32);
        let k = (rect.width() as f32 / tw).min(rect.height() as f32 / th);
        let (w, h) = ((tw * k).round().max(1.), (th * k).round().max(1.));
        let dst = Rect::new(
            rect.x() + ((rect.width() as f32 - w) / 2.).round() as i32,
            rect.y() + ((rect.height() as f32 - h) / 2.).round() as i32,
            w as u32,
            h as u32,
        );
        texture.set_alpha_mod((opacity * 255.).round() as u8);
        let drawn = canvas.copy(texture, None, dst);
        texture.set_alpha_mod(255);
        drawn
    }

    /// A favorite's mark in its corner of `picture`.
    fn badge(
        &mut self,
        badge: &Badge,
        picture: Rect,
        opacity: f32,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let side = (badge.size * picture.height() as f32).round().max(1.) as i32;
        let inset = (side / 6).max(1);
        let (right, bottom) = badge.corner;
        let x = if right {
            picture.right() - side - inset
        } else {
            picture.x() + inset
        };
        let y = if bottom {
            picture.bottom() - side - inset
        } else {
            picture.y() + inset
        };
        let cell = Rect::new(x, y, side as u32, side as u32);
        let path = badge.image.as_ref().and_then(|t| {
            let path = t.fill(|d| facts.value(d), |k| self.phrase(k));
            self.theme_file(&path)
        });
        // The launcher's star in the accent, unless the theme colors it.
        let tint = badge.tint.or(path.is_none().then_some(theme.highlight_bg));
        self.star(path.as_deref(), cell, 1., tint, opacity)
    }

    /// The item's name where its picture would be: drawn once at the
    /// focus's size and scaled, so a gliding carousel draws no new text.
    fn item_name(
        &mut self,
        t: &Text,
        facts: &Facts<'_>,
        spot: &Spot,
        grow: f32,
        ink: Rgba,
    ) -> Result<(), String> {
        let text = t.text.fill(|d| facts.value(d), |k| self.phrase(k));
        let text = if t.upper { text.to_uppercase() } else { text };
        if text.trim().is_empty() {
            return Ok(());
        }
        let face = face(t.font);
        let width = (spot.w * grow).round().max(1.) as u32;
        let start = t.size * self.frame.reference().1 * grow;
        let least = (start * 0.6).max(self.frame.min_size());
        let size = if t.shrink {
            self.shrink(&text, face, start, least, width)?
                .unwrap_or(least)
        } else {
            start
        };
        let px = self.frame.font_size(size);
        let tall = (spot.h * grow).round().max(1.) as u32;
        let panel = Rect::new(
            (spot.x - width as f32 / 2.).round() as i32,
            (spot.y - tall as f32 / 2.).round() as i32,
            width,
            tall,
        );
        let style = Style {
            face,
            size: px,
            color: ink,
            tracking: self.tracking(size, px),
        };
        let turn = (
            Align::Center,
            spot.scale / grow,
            f64::from(spot.degrees),
            spot.alpha,
        );
        let text_cache = self
            .text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?;
        text_cache
            .draw_ex(&mut self.canvas, &text, style, panel, turn)
            .map_err(|e| localized_error(&self.catalog, e))
    }
}

/// A carousel's items around `at` (in items, counting a loop's turns) on a
/// `screen` of pixels: one of each, at most a step past the `visible` ones,
/// fading there.
fn carousel(
    c: &Collection,
    (axis, visible, align, wheel): (Axis, usize, Align, Option<crate::layout::Wheel>),
    (area, opacity): (Rect, f32),
    (at, len): (f32, usize),
    (sw, sh): (f32, f32),
) -> Vec<Spot> {
    let vertical = axis == Axis::Vertical;
    let (along, across, screen) = if vertical {
        (area.height() as f32, area.width() as f32, sh)
    } else {
        (area.width() as f32, area.height() as f32, sw)
    };
    let spacing = c
        .spacing
        .map_or(along / visible.max(1) as f32, |s| s * screen);
    // At rest, an item takes most of a step along and leaves room across
    // for the focus to grow.
    let grow = c.selected.scale.max(1.);
    let (w, h) = if vertical {
        (across / grow, spacing * 0.8)
    } else {
        (spacing * 0.8, across / grow)
    };
    let (w, h) = (
        c.item.w.map_or(w, |f| f * sw),
        c.item.h.map_or(h, |f| f * sh),
    );
    let half = (visible.max(1) as f32 - 1.) / 2.;
    let (cx, cy) = (
        area.x() as f32 + area.width() as f32 / 2.,
        area.y() as f32 + area.height() as f32 / 2.,
    );
    // Each item once, the nearest copy of it when the ends join.
    let reach = (half + 1.).ceil() as i64;
    let middle = at.round() as i64;
    let n = len as i64;
    let (lo, hi) = if c.wrap {
        let lo = middle - (n - 1) / 2;
        ((middle - reach).max(lo), (middle + reach).min(lo + n - 1))
    } else {
        ((middle - reach).max(0), (middle + reach).min(n - 1))
    };
    let mut spots = Vec::new();
    for k in lo..=hi {
        let d = k as f32 - at;
        let away = d.abs();
        if away >= half + 1. {
            continue;
        }
        let focus = (1. - away).max(0.);
        let scale = mix(1., c.selected.scale, focus);
        let fade = (half + 1. - away).clamp(0., 1.);
        // Across a column, the item keeps to its side of the box.
        let side = match align {
            Align::Left => area.x() as f32 + w * scale / 2.,
            Align::Right => area.right() as f32 - w * scale / 2.,
            Align::Center => cx,
        };
        let (mut x, mut y, mut degrees) = if vertical {
            (side, cy + d * spacing, 0.)
        } else {
            (cx + d * spacing, cy, 0.)
        };
        // A wheel: every item in the middle, turned about the pivot.
        if let Some(wheel) = wheel {
            let (bx, by) = if vertical { (side, cy) } else { (cx, cy) };
            let (px, py) = (
                bx - w / 2. + wheel.pivot.0 * w,
                by - h / 2. + wheel.pivot.1 * h,
            );
            degrees = d * wheel.angle;
            let (sin, cos) = degrees.to_radians().sin_cos();
            let (dx, dy) = (bx - px, by - py);
            (x, y) = (px + dx * cos - dy * sin, py + dx * sin + dy * cos);
        }
        spots.push(Spot {
            index: k.rem_euclid(n) as usize,
            x,
            y,
            w,
            h,
            away,
            focus,
            scale,
            alpha: mix(opacity, c.selected.opacity, focus) * fade,
            degrees,
        });
    }
    spots
}
