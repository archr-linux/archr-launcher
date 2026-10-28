//! Cached SDL 2.0.18 meshes and one startup-generated 9-slice shadow.
use crate::{startup::StartupTrace, startup_phase, text::color, theme::Rgba};
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    rect::{FPoint, Rect},
    render::{BlendMode, Canvas, ScaleMode, Texture, TextureCreator, Vertex},
    video::{Window, WindowContext},
};
use std::{
    collections::VecDeque,
    f32::consts::{FRAC_PI_2, PI, TAU},
    sync::OnceLock,
};

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
fn outline(rect: Rect, radius: f32) -> [Point; POINTS] {
    let (x, y, w, h) = (
        rect.x() as f32,
        rect.y() as f32,
        rect.width() as f32,
        rect.height() as f32,
    );
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
    fn fan(&mut self, points: &[Point], c: Color, rect: Rect, textured: bool) {
        let base = self.vertices.len() as u16;
        let center = (
            rect.x() as f32 + rect.width() as f32 / 2.,
            rect.y() as f32 + rect.height() as f32 / 2.,
        );
        self.vertices.push(vertex(center, c, (0.5, 0.5)));
        self.vertices.extend(points.iter().map(|&p| {
            vertex(
                p,
                c,
                if textured {
                    (
                        (p.0 - rect.x() as f32) / rect.width() as f32,
                        (p.1 - rect.y() as f32) / rect.height() as f32,
                    )
                } else {
                    (0., 0.)
                },
            )
        }));
        for i in 0..points.len() {
            self.indices.extend([
                base,
                base + (i + 1) as u16,
                base + ((i + 1) % points.len() + 1) as u16,
            ]);
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
    fn rounded(&mut self, rect: Rect, radius: f32, fill: Rgba, border: Option<Rgba>) {
        let points = outline(rect, radius);
        let outer = outline(
            Rect::new(
                rect.x() - 1,
                rect.y() - 1,
                rect.width() + 2,
                rect.height() + 2,
            ),
            radius + 1.,
        );
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
        self.fan(&points, color(edge), rect, false);
        if border.is_some() && rect.width() > 2 && rect.height() > 2 {
            let inner = Rect::new(
                rect.x() + 1,
                rect.y() + 1,
                rect.width() - 2,
                rect.height() - 2,
            );
            self.fan(
                &outline(inner, (radius - 1.).max(0.)),
                color(fill),
                inner,
                false,
            );
        }
    }
    fn star(&mut self, rect: Rect, c: Rgba, filled: bool) {
        static STAR: OnceLock<[Point; 10]> = OnceLock::new();
        let unit = STAR.get_or_init(|| {
            std::array::from_fn(|i| {
                let a = -FRAC_PI_2 + i as f32 * PI / 5.;
                let r = if i % 2 == 0 { 1. } else { 0.46 };
                (r * a.cos(), r * a.sin())
            })
        });
        let center = (
            rect.x() as f32 + rect.width() as f32 / 2.,
            rect.y() as f32 + rect.height() as f32 / 2.,
        );
        let r = rect.width().min(rect.height()) as f32 / 2.;
        let points = unit.map(|p| (center.0 + r * p.0, center.1 + r * p.1));
        if filled {
            self.fan(&points, color(c), rect, false);
        } else {
            for i in 0..10 {
                self.line(points[i], points[(i + 1) % 10], 1.4, c);
            }
        }
    }
    fn clock(&mut self, rect: Rect, c: Rgba) {
        static CIRCLE: OnceLock<[Point; 24]> = OnceLock::new();
        let unit = CIRCLE.get_or_init(|| {
            std::array::from_fn(|i| {
                let a = i as f32 * TAU / 24.;
                (a.cos(), a.sin())
            })
        });
        let (cx, cy) = (
            rect.x() as f32 + rect.width() as f32 / 2.,
            rect.y() as f32 + rect.height() as f32 / 2.,
        );
        let r = rect.width() as f32 / 2. - 1.;
        let points = unit.map(|p| (cx + r * p.0, cy + r * p.1));
        for i in 0..24 {
            self.line(points[i], points[(i + 1) % 24], 1.3, c);
        }
        self.line((cx, cy - r * 0.55), (cx, cy), 1.3, c);
        self.line((cx, cy), (cx + r * 0.4, cy + r * 0.25), 1.3, c);
    }
    fn lightning(&mut self, rect: Rect, c: Rgba) {
        let (x, y, w, h) = (
            rect.x() as f32,
            rect.y() as f32,
            rect.width() as f32,
            rect.height() as f32,
        );
        self.vertices.extend(
            [
                (x + w * 0.85, y),
                (x + w * 0.05, y + h * 0.6),
                (x + w * 0.42, y + h * 0.6),
                (x + w * 0.15, y + h),
                (x + w * 0.95, y + h * 0.4),
                (x + w * 0.58, y + h * 0.4),
            ]
            .map(|p| vertex(p, color(c), (0., 0.))),
        );
        self.indices.extend([0, 1, 2, 0, 2, 5, 2, 3, 4, 2, 4, 5]);
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
    Rounded(Rect, u32, Rgba, Option<Rgba>),
    Cover(Rect, u32),
    Star(Rect, Rgba, bool),
    Clock(Rect, Rgba),
    Lightning(Rect, Rgba),
}
/// Per-renderer bounded LRU: static badges, battery pieces and icons are hits;
/// evicted meshes donate their Vec allocations to the next moving-pill mesh.
pub(crate) struct Geometry {
    pub(crate) startup: StartupTrace,
    meshes: VecDeque<(Key, Mesh)>,
}
impl Geometry {
    pub fn new() -> Self {
        corner_arcs();
        Self {
            startup: StartupTrace::default(),
            meshes: VecDeque::with_capacity(CACHE_SIZE),
        }
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
                Key::Cover(rect, r) => {
                    mesh.fan(&outline(rect, f32::from_bits(r)), Color::WHITE, rect, true)
                }
                Key::Star(rect, c, filled) => {
                    mesh.star(rect, c, filled);
                    startup_phase!(self.startup, "icon star");
                }
                Key::Clock(rect, c) => {
                    mesh.clock(rect, c);
                    startup_phase!(self.startup, "icon clock");
                }
                Key::Lightning(rect, c) => {
                    mesh.lightning(rect, c);
                    startup_phase!(self.startup, "icon lightning");
                }
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
        self.mesh(Key::Rounded(rect, radius.to_bits(), fill, border))
            .draw(canvas, None)
    }
    pub fn cover(
        &mut self,
        canvas: &mut Canvas<Window>,
        texture: &Texture<'_>,
        rect: Rect,
        radius: f32,
        alpha: u8,
    ) -> Result<(), String> {
        let mesh = self.mesh(Key::Cover(rect, radius.to_bits()));
        for vertex in &mut mesh.vertices {
            vertex.color.a = alpha;
        }
        mesh.draw(canvas, Some(texture))
    }
    pub fn star(
        &mut self,
        canvas: &mut Canvas<Window>,
        rect: Rect,
        c: Rgba,
        filled: bool,
    ) -> Result<(), String> {
        self.mesh(Key::Star(rect, c, filled)).draw(canvas, None)
    }
    pub fn clock_icon(
        &mut self,
        canvas: &mut Canvas<Window>,
        rect: Rect,
        c: Rgba,
    ) -> Result<(), String> {
        self.mesh(Key::Clock(rect, c)).draw(canvas, None)
    }
    pub fn lightning(
        &mut self,
        canvas: &mut Canvas<Window>,
        rect: Rect,
        c: Rgba,
    ) -> Result<(), String> {
        self.mesh(Key::Lightning(rect, c)).draw(canvas, None)
    }
}
pub(crate) struct Shadow<'a>(Texture<'a>);
impl<'a> Shadow<'a> {
    pub fn new(creator: &'a TextureCreator<WindowContext>) -> Result<Self, String> {
        let mut bytes = vec![0_u8; 64 * 64 * 4];
        for y in 0..64 {
            for x in 0..64 {
                let qx = (x as f32 - 31.5).abs() - 4.;
                let qy = (y as f32 - 31.5).abs() - 4.;
                let distance = qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - 12.;
                let alpha = (115. * (-0.5 * (distance.max(0.) / 6.).powi(2)).exp()).round() as u8;
                bytes[(y * 64 + x) * 4 + 3] = alpha;
            }
        }
        let mut texture = creator
            .create_texture_static(PixelFormatEnum::RGBA32, 64, 64)
            .map_err(|e| e.to_string())?;
        texture
            .update(None, &bytes, 64 * 4)
            .map_err(|e| e.to_string())?;
        texture.set_blend_mode(BlendMode::Blend);
        texture.set_scale_mode(ScaleMode::Linear);
        Ok(Self(texture))
    }
    pub fn draw(&self, canvas: &mut Canvas<Window>, rect: Rect, scale: f32) -> Result<(), String> {
        let pad = (16. * scale).round() as i32;
        let corner = (12. * scale).round() as i32;
        let shift = (8. * scale).round() as i32;
        let xs = [
            rect.x() - pad,
            rect.x() + corner,
            rect.right() - corner,
            rect.right() + pad,
        ];
        let ys = [
            rect.y() - pad + shift,
            rect.y() + corner + shift,
            rect.bottom() - corner + shift,
            rect.bottom() + pad + shift,
        ];
        let src = [0, 28, 36, 64];
        for y in 0..3 {
            for x in 0..3 {
                if xs[x + 1] > xs[x] && ys[y + 1] > ys[y] {
                    canvas.copy(
                        &self.0,
                        Rect::new(
                            src[x],
                            src[y],
                            (src[x + 1] - src[x]) as u32,
                            (src[y + 1] - src[y]) as u32,
                        ),
                        Rect::new(
                            xs[x],
                            ys[y],
                            (xs[x + 1] - xs[x]) as u32,
                            (ys[y + 1] - ys[y]) as u32,
                        ),
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
        for icon in [
            Key::Star(Rect::new(0, 0, 16, 16), color, true),
            Key::Clock(Rect::new(0, 0, 16, 16), color),
            Key::Lightning(Rect::new(0, 0, 8, 14), color),
        ] {
            let first = geometry.mesh(icon).vertices.as_ptr();
            assert_eq!(geometry.mesh(icon).vertices.as_ptr(), first);
        }
    }
}
