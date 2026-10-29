//! Cached SDL 2.0.18 meshes and a lazily loaded/generated 9-slice shadow.
use crate::{text::color, theme::Rgba};
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    rect::{FPoint, Rect},
    render::{BlendMode, Canvas, ScaleMode, Texture, TextureCreator, Vertex},
    video::{Window, WindowContext},
};
use std::{collections::VecDeque, f32::consts::FRAC_PI_2, sync::OnceLock};

type Point = (f32, f32);
const SEGMENTS: usize = 8;
const POINTS: usize = (SEGMENTS + 1) * 4;
const CACHE_SIZE: usize = 128;
fn vertex(p: Point, c: Color, uv: Point) -> Vertex {
    Vertex {
        position: FPoint::new(p.0, p.1),
        color: c,
        tex_coord: FPoint::new(uv.0, uv.1),
    }
}
fn corner_arcs() -> &'static [[Point; SEGMENTS + 1]; 4] {
    static ARCS: OnceLock<[[Point; SEGMENTS + 1]; 4]> = OnceLock::new();
    ARCS.get_or_init(|| {
        std::array::from_fn(|corner| {
            std::array::from_fn(|step| {
                let angle = -FRAC_PI_2
                    + corner as f32 * FRAC_PI_2
                    + step as f32 * FRAC_PI_2 / SEGMENTS as f32;
                (angle.cos(), angle.sin())
            })
        })
    })
}
fn bounds(rect: Rect) -> (f32, f32, f32, f32) {
    (
        rect.x() as f32,
        rect.y() as f32,
        rect.width() as f32,
        rect.height() as f32,
    )
}
fn center(rect: Rect) -> Point {
    (
        rect.x() as f32 + rect.width() as f32 / 2.,
        rect.y() as f32 + rect.height() as f32 / 2.,
    )
}
/// Pixel bounds shared by nine-slice geometry and row layout.
pub(crate) fn between(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect::new(
        left,
        top,
        (right - left).max(1) as u32,
        (bottom - top).max(1) as u32,
    )
}
/// `rect` grown by `by` pixels on every side (shrunk when negative).
fn grown(rect: Rect, by: i32) -> Rect {
    let size = |n: u32| (n as i32 + 2 * by).max(1) as u32;
    Rect::new(
        rect.x() - by,
        rect.y() - by,
        size(rect.width()),
        size(rect.height()),
    )
}
fn slice_axis(start: i32, end: i32, pad: i32, corner: i32, shift: i32) -> [i32; 4] {
    [start - pad, start + corner, end - corner, end + pad].map(|n| n + shift)
}
fn outline(rect: Rect, radius: f32) -> [Point; POINTS] {
    let (x, y, w, h) = bounds(rect);
    let r = radius.min(w / 2.).min(h / 2.).max(0.);
    let centers = [
        (x + w - r, y + r),
        (x + w - r, y + h - r),
        (x + r, y + h - r),
        (x + r, y + r),
    ];
    let arcs = corner_arcs();
    std::array::from_fn(|i| {
        let corner = i / (SEGMENTS + 1);
        let (dx, dy) = arcs[corner][i % (SEGMENTS + 1)];
        (centers[corner].0 + r * dx, centers[corner].1 + r * dy)
    })
}
struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}
impl Mesh {
    fn new() -> Self {
        Self {
            vertices: Vec::with_capacity(160),
            indices: Vec::with_capacity(512),
        }
    }
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
    fn fan(&mut self, points: &[Point], c: Color, rect: Rect, crop: Option<[f32; 4]>) {
        let base = self.vertices.len() as u16;
        let uv = |p: Point| {
            crop.map_or((0., 0.), |[u, v, w, h]| {
                (
                    u + w * (p.0 - rect.x() as f32) / rect.width() as f32,
                    v + h * (p.1 - rect.y() as f32) / rect.height() as f32,
                )
            })
        };
        let center = center(rect);
        self.vertices.push(vertex(center, c, uv(center)));
        self.vertices
            .extend(points.iter().map(|&p| vertex(p, c, uv(p))));
        for i in 0..points.len() {
            self.indices.extend([
                base,
                base + (i + 1) as u16,
                base + ((i + 1) % points.len() + 1) as u16,
            ]);
        }
    }
    /// Filled, with a `width` pixel border when `border` is given.
    fn rounded(&mut self, rect: Rect, radius: f32, fill: Rgba, border: Option<(Rgba, u32)>) {
        let points = outline(rect, radius);
        let outer = outline(grown(rect, 1), radius + 1.);
        let (border, width) = border.map_or((None, 0), |(c, w)| (Some(c), w));
        let edge = border.unwrap_or(fill);
        for (&p, &q) in points.iter().zip(&outer) {
            self.vertices.push(vertex(p, color(edge), (0., 0.)));
            self.vertices
                .push(vertex(q, Color::RGBA(edge.r, edge.g, edge.b, 0), (0., 0.)));
        }
        for i in 0..POINTS {
            let a = (i * 2) as u16;
            let b = ((i + 1) % POINTS * 2) as u16;
            self.indices.extend([a, a + 1, b, b, a + 1, b + 1]);
        }
        self.fan(&points, color(edge), rect, None);
        if border.is_some() && rect.width() > 2 * width && rect.height() > 2 * width {
            let inner = grown(rect, -(width as i32));
            self.fan(
                &outline(inner, (radius - width as f32).max(0.)),
                color(fill),
                inner,
                None,
            );
        }
    }
    fn line(&mut self, from: Point, to: Point, width: f32, c: Rgba) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = dx.hypot(dy).max(0.01);
        let (nx, ny) = (-dy / length * width / 2., dx / length * width / 2.);
        let base = self.vertices.len() as u16;
        self.vertices.extend(
            [
                (from.0 + nx, from.1 + ny),
                (to.0 + nx, to.1 + ny),
                (to.0 - nx, to.1 - ny),
                (from.0 - nx, from.1 - ny),
            ]
            .map(|p| vertex(p, color(c), (0., 0.))),
        );
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    /// Dashed stroke centred 1 unit inside the rounded outline; a unit is 1/22
    /// of the height, so the 44px row gets 2px strokes with 5px dashes.
    fn dashed(&mut self, rect: Rect, radius: f32, c: Rgba) {
        let unit = rect.height() as f32 / 22.;
        let inset = (unit / 2.).round().max(1.) as i32;
        let inner = Rect::new(
            rect.x() + inset,
            rect.y() + inset,
            rect.width().saturating_sub(2 * inset as u32).max(1),
            rect.height().saturating_sub(2 * inset as u32).max(1),
        );
        let points = outline(inner, radius - inset as f32);
        let (mut drawing, mut left) = (true, 2.5 * unit);
        for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
            let length = (b.0 - a.0).hypot(b.1 - a.1);
            let at = |t: f32| {
                (
                    a.0 + (b.0 - a.0) * t / length,
                    a.1 + (b.1 - a.1) * t / length,
                )
            };
            let mut t = 0.;
            while t < length {
                let step = left.min(length - t);
                if drawing {
                    self.line(at(t), at(t + step), unit, c);
                }
                t += step;
                left -= step;
                if left <= 0. {
                    drawing = !drawing;
                    left = unit * if drawing { 2.5 } else { 1.5 };
                }
            }
        }
    }
    fn draw(
        &self,
        canvas: &mut Canvas<Window>,
        texture: Option<&Texture<'_>>,
    ) -> Result<(), String> {
        canvas.render_geometry(&self.vertices, texture, self.indices.as_slice())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Rounded(Rect, u32, Rgba, Option<(Rgba, u32)>),
    Cover(Rect, u32, [u32; 4]),
    Dashed(Rect, u32, Rgba),
}
/// Per-renderer bounded LRU for panels and cover meshes. Evicted meshes
/// donate their Vec allocations to the next moving-pill mesh.
pub(crate) struct Geometry {
    meshes: VecDeque<(Key, Mesh)>,
    /// The theme's style: every radius times `corners`, borders `border` pixels.
    corners: f32,
    border: u32,
}
impl Geometry {
    pub fn new() -> Self {
        corner_arcs();
        Self {
            meshes: VecDeque::with_capacity(CACHE_SIZE),
            corners: 1.,
            border: 1,
        }
    }
    /// The theme's corner factor and border width in pixels, for every shape.
    pub fn style(&mut self, corners: f32, border: u32) {
        (self.corners, self.border) = (corners, border.max(1));
    }
    fn mesh(&mut self, key: Key) -> &mut Mesh {
        if let Some(index) = self.meshes.iter().position(|(k, _)| *k == key) {
            let hit = self.meshes.remove(index).expect("located mesh");
            self.meshes.push_back(hit);
        } else {
            let mut mesh = if self.meshes.len() == CACHE_SIZE {
                self.meshes.pop_front().expect("full mesh cache").1
            } else {
                Mesh::new()
            };
            mesh.clear();
            match key {
                Key::Rounded(rect, r, fill, border) => {
                    mesh.rounded(rect, f32::from_bits(r), fill, border)
                }
                Key::Cover(rect, r, crop) => mesh.fan(
                    &outline(rect, f32::from_bits(r)),
                    Color::WHITE,
                    rect,
                    Some(crop.map(f32::from_bits)),
                ),
                Key::Dashed(rect, r, c) => mesh.dashed(rect, f32::from_bits(r), c),
            }
            self.meshes.push_back((key, mesh));
        }
        &mut self.meshes.back_mut().expect("cached mesh").1
    }
    pub fn rounded(
        &mut self,
        canvas: &mut Canvas<Window>,
        rect: Rect,
        radius: f32,
        fill: Rgba,
        border: Option<Rgba>,
    ) -> Result<(), String> {
        let radius = radius * self.corners;
        let border = border.map(|c| (c, self.border));
        self.mesh(Key::Rounded(rect, radius.to_bits(), fill, border))
            .draw(canvas, None)
    }
    pub fn dashed(
        &mut self,
        canvas: &mut Canvas<Window>,
        rect: Rect,
        radius: f32,
        c: Rgba,
    ) -> Result<(), String> {
        self.mesh(Key::Dashed(rect, (radius * self.corners).to_bits(), c))
            .draw(canvas, None)
    }
    pub fn cover(
        &mut self,
        canvas: &mut Canvas<Window>,
        texture: &Texture<'_>,
        rect: Rect,
        radius: f32,
        alpha: u8,
        crop: [f32; 4],
    ) -> Result<(), String> {
        let radius = radius * self.corners;
        let mesh = self.mesh(Key::Cover(rect, radius.to_bits(), crop.map(f32::to_bits)));
        for vertex in &mut mesh.vertices {
            vertex.color.a = alpha;
        }
        mesh.draw(canvas, Some(texture))
    }
}
/// `rect` shaded from `left` to `right`.
pub(crate) fn gradient(
    canvas: &mut Canvas<Window>,
    rect: Rect,
    left: Rgba,
    right: Rgba,
) -> Result<(), String> {
    let (x0, y0, x1, y1) = (
        rect.x() as f32,
        rect.y() as f32,
        rect.right() as f32,
        rect.bottom() as f32,
    );
    let (l, r) = (color(left), color(right));
    let corners = [
        vertex((x0, y0), l, (0., 0.)),
        vertex((x1, y0), r, (0., 0.)),
        vertex((x1, y1), r, (0., 0.)),
        vertex((x0, y1), l, (0., 0.)),
    ];
    canvas.render_geometry(&corners, None, [0u16, 1, 2, 0, 2, 3].as_slice())
}
/// Texture fraction (u, v, w, h) that fills a `width`×`height` box without
/// distortion: everything (`contain` is the caller's job) or a centered crop.
pub(crate) fn crop(texture: (u32, u32), (width, height): (u32, u32)) -> [f32; 4] {
    let source = texture.0 as f32 / texture.1.max(1) as f32;
    let target = width as f32 / height.max(1) as f32;
    if source > target {
        let w = target / source;
        [(1. - w) / 2., 0., w, 1.]
    } else {
        let h = source / target;
        [0., (1. - h) / 2., 1., h]
    }
}
pub(crate) const WHOLE: [f32; 4] = [0., 0., 1., 1.];

/// One axis of a nine-slice: (source start, source length, screen start,
/// screen length) for the near edge, the middle (split into repeats when
/// `tile`) and the far edge.
fn slice_spans(
    n: u32,
    (near, far): (u32, u32),
    k: u32,
    (start, length): (i32, u32),
    tile: bool,
) -> Vec<(u32, u32, i32, u32)> {
    let (near, far) = (near.min(n), far.min(n - near.min(n)));
    let middle = n - near - far;
    // Edges keep their size; on a box too small for both they shrink alike.
    let (mut a, mut b) = (near * k, far * k);
    if a + b > length {
        a = length * near / (near + far).max(1);
        b = length - a;
    }
    let inside = length - a - b;
    let mut spans = vec![(0, near, start, a)];
    if middle > 0 && inside > 0 {
        let step = if tile { middle * k } else { inside };
        let mut at = 0;
        while at < inside {
            let part = step.min(inside - at);
            let source = if tile {
                part.div_ceil(k).min(middle)
            } else {
                middle
            };
            spans.push((near, source, start + (a + at) as i32, part));
            at += part;
        }
    }
    spans.push((n - far, far, start + (length - b) as i32, b));
    spans.retain(|&(_, source, _, screen)| source > 0 && screen > 0);
    spans
}
/// (source, screen) rectangles that draw a `size` picture as a nine-slice
/// frame over `dst`: `slice` (left, top, right, bottom) picture pixels at the
/// edges stay whole at `k` screen pixels each; the rest stretches or repeats.
pub(crate) fn nine_slice(
    size: (u32, u32),
    slice: [u32; 4],
    k: u32,
    dst: Rect,
    tile: bool,
) -> Vec<(Rect, Rect)> {
    let k = k.max(1);
    let xs = slice_spans(
        size.0,
        (slice[0], slice[2]),
        k,
        (dst.x(), dst.width()),
        tile,
    );
    let ys = slice_spans(
        size.1,
        (slice[1], slice[3]),
        k,
        (dst.y(), dst.height()),
        tile,
    );
    let mut parts = Vec::with_capacity(xs.len() * ys.len());
    for &(sy, sh, dy, dh) in &ys {
        for &(sx, sw, dx, dw) in &xs {
            parts.push((
                Rect::new(sx as i32, sy as i32, sw, sh),
                Rect::new(dx, dy, dw, dh),
            ));
        }
    }
    parts
}
pub(crate) struct Shadow<'a>(Texture<'a>);
impl<'a> Shadow<'a> {
    pub fn new(creator: &'a TextureCreator<WindowContext>) -> Result<Self, String> {
        Self::from_surface(creator, Self::surface()?)
    }
    pub fn surface() -> Result<sdl2::surface::Surface<'static>, String> {
        let mut surface = sdl2::surface::Surface::new(64, 64, PixelFormatEnum::RGBA32)?;
        let pitch = surface.pitch() as usize;
        surface.with_lock_mut(|bytes| {
            for y in 0..64 {
                for x in 0..64 {
                    let qx = (x as f32 - 31.5).abs() - 4.;
                    let qy = (y as f32 - 31.5).abs() - 4.;
                    let distance = qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - 12.;
                    let alpha =
                        (115. * (-0.5 * (distance.max(0.) / 6.).powi(2)).exp()).round() as u8;
                    bytes[y * pitch + x * 4..y * pitch + x * 4 + 4]
                        .copy_from_slice(&[0, 0, 0, alpha]);
                }
            }
        });
        Ok(surface)
    }
    pub fn from_surface(
        creator: &'a TextureCreator<WindowContext>,
        surface: sdl2::surface::Surface<'_>,
    ) -> Result<Self, String> {
        let mut texture = creator
            .create_texture_from_surface(surface)
            .map_err(|e| e.to_string())?;
        texture.set_blend_mode(BlendMode::Blend);
        texture.set_scale_mode(ScaleMode::Linear);
        Ok(Self(texture))
    }
    /// Under `rect`, whose corners are rounded by `radius`: the shadow grows
    /// with the radius, so it follows the corners instead of poking out.
    pub fn draw(
        &self,
        canvas: &mut Canvas<Window>,
        rect: Rect,
        scale: f32,
        radius: f32,
    ) -> Result<(), String> {
        let fit = rect.width().min(rect.height()) as f32 / 24.;
        let k = (radius / 12.).max(scale).min(fit);
        let pad = (16. * k).round() as i32;
        let corner = (12. * k).round() as i32;
        let shift = (8. * k).round() as i32;
        let xs = slice_axis(rect.x(), rect.right(), pad, corner, 0);
        let ys = slice_axis(rect.y(), rect.bottom(), pad, corner, shift);
        let src = [0, 28, 36, 64];
        for y in 0..3 {
            for x in 0..3 {
                if xs[x + 1] > xs[x] && ys[y + 1] > ys[y] {
                    canvas.copy(
                        &self.0,
                        between(src[x], src[y], src[x + 1], src[y + 1]),
                        between(xs[x], ys[y], xs[x + 1], ys[y + 1]),
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nine_slices_keep_corners_whole_and_cover_the_box_exactly() {
        let dst = Rect::new(10, 20, 100, 50);
        let parts = nine_slice((12, 12), [4; 4], 2, dst, false);
        assert_eq!(parts.len(), 9);
        assert_eq!(parts[0], (Rect::new(0, 0, 4, 4), Rect::new(10, 20, 8, 8)));
        assert_eq!(parts[4], (Rect::new(4, 4, 4, 4), Rect::new(18, 28, 84, 34)));
        assert_eq!(parts[8], (Rect::new(8, 8, 4, 4), Rect::new(102, 62, 8, 8)));
        let area: u32 = parts.iter().map(|(_, d)| d.width() * d.height()).sum();
        assert_eq!(area, 100 * 50, "no gaps, no overlaps");
        // Repeating: the 84 px middle takes ten whole 8 px copies and a 4 px rest.
        let tiled = nine_slice((12, 12), [4; 4], 2, dst, true);
        let row: Vec<_> = tiled.iter().filter(|(_, d)| d.y() == 20).collect();
        assert_eq!(row.len(), 2 + 11);
        assert_eq!(row[11], &(Rect::new(4, 0, 2, 4), Rect::new(98, 20, 4, 8)));
        let area: u32 = tiled.iter().map(|(_, d)| d.width() * d.height()).sum();
        assert_eq!(area, 100 * 50);
        // A box smaller than both edges shrinks them alike; no slice stretches all.
        let tiny = nine_slice((12, 12), [4; 4], 2, Rect::new(0, 0, 10, 10), false);
        assert_eq!(tiny.len(), 4);
        assert_eq!(nine_slice((12, 12), [0; 4], 2, dst, false).len(), 1);
    }
    #[test]
    fn borders_take_the_theme_width_and_square_corners_stay_square() {
        let ink = |r| Rgba {
            r,
            g: 0,
            b: 0,
            a: 255,
        };
        let mut mesh = Mesh::new();
        mesh.rounded(Rect::new(10, 10, 40, 30), 0., ink(1), Some((ink(2), 3)));
        let fill: Vec<_> = mesh.vertices.iter().filter(|v| v.color.r == 1).collect();
        let xs = fill.iter().map(|v| v.position.x);
        let ys = fill.iter().map(|v| v.position.y);
        assert_eq!(xs.clone().fold(f32::MAX, f32::min), 13.);
        assert_eq!(xs.fold(f32::MIN, f32::max), 47.);
        assert_eq!(ys.fold(f32::MAX, f32::min), 13.);
        // Radius 0: every outline point sits on a corner of the rectangle.
        assert!(outline(Rect::new(10, 10, 40, 30), 0.)
            .iter()
            .all(|p| [10., 50.].contains(&p.0) && [10., 40.].contains(&p.1)));
    }
    #[test]
    fn cached_meshes_retain_buffers_and_recycle_evicted_storage() {
        let mut geometry = Geometry::new();
        let color = Rgba {
            r: 30,
            g: 136,
            b: 255,
            a: 255,
        };
        let key = |x| Key::Rounded(Rect::new(x, 10, 20, 20), 10f32.to_bits(), color, None);
        let first = geometry.mesh(key(0));
        let pointers = (first.vertices.as_ptr(), first.indices.as_ptr());
        let hit = geometry.mesh(key(0));
        assert_eq!((hit.vertices.as_ptr(), hit.indices.as_ptr()), pointers);
        for i in 1..CACHE_SIZE {
            geometry.mesh(key(i as i32));
        }
        let recycled = geometry.mesh(key(CACHE_SIZE as i32));
        assert_eq!(
            (recycled.vertices.as_ptr(), recycled.indices.as_ptr()),
            pointers
        );
        assert_eq!(geometry.meshes.len(), CACHE_SIZE);
        assert!(std::ptr::eq(corner_arcs(), corner_arcs()));
    }
}
