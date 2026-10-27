//! SDL 2.0.18 geometry primitives and one startup-generated 9-slice shadow.
use crate::{text::color, theme::Rgba};
use sdl2::{
    pixels::{Color, PixelFormatEnum},
    rect::{FPoint, Rect},
    render::{BlendMode, Canvas, ScaleMode, Texture, TextureCreator, Vertex},
    video::{Window, WindowContext},
};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

type Point = (f32, f32);
fn vertex(p: Point, c: Color, uv: Point) -> Vertex {
    Vertex {
        position: FPoint::new(p.0, p.1),
        color: c,
        tex_coord: FPoint::new(uv.0, uv.1),
    }
}
fn outline(rect: Rect, radius: f32) -> Vec<Point> {
    let (x, y, w, h) = (
        rect.x() as f32,
        rect.y() as f32,
        rect.width() as f32,
        rect.height() as f32,
    );
    let r = radius.min(w / 2.).min(h / 2.).max(0.);
    let mut points = Vec::with_capacity(36);
    for (cx, cy, a) in [
        (x + w - r, y + r, -FRAC_PI_2),
        (x + w - r, y + h - r, 0.),
        (x + r, y + h - r, FRAC_PI_2),
        (x + r, y + r, PI),
    ] {
        for step in 0..=8 {
            let angle = a + step as f32 * FRAC_PI_2 / 8.;
            points.push((cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    points
}
fn fan(
    points: &[Point],
    c: Color,
    texture: Option<&Texture<'_>>,
    rect: Rect,
) -> (Vec<Vertex>, Vec<u16>) {
    let center = (
        rect.x() as f32 + rect.width() as f32 / 2.,
        rect.y() as f32 + rect.height() as f32 / 2.,
    );
    let uv = |p: Point| {
        (
            (p.0 - rect.x() as f32) / rect.width() as f32,
            (p.1 - rect.y() as f32) / rect.height() as f32,
        )
    };
    let mut vertices = vec![vertex(center, c, uv(center))];
    vertices.extend(
        points
            .iter()
            .map(|&p| vertex(p, c, if texture.is_some() { uv(p) } else { (0., 0.) })),
    );
    let mut indices = Vec::with_capacity(points.len() * 3);
    for i in 0..points.len() {
        indices.extend([0, (i + 1) as u16, ((i + 1) % points.len() + 1) as u16]);
    }
    (vertices, indices)
}
fn fill(canvas: &mut Canvas<Window>, rect: Rect, radius: f32, c: Rgba) -> Result<(), String> {
    let (vertices, indices) = fan(&outline(rect, radius), color(c), None, rect);
    canvas.render_geometry(&vertices, None, indices.as_slice())
}
pub(crate) fn rounded(
    canvas: &mut Canvas<Window>,
    rect: Rect,
    radius: f32,
    fill_color: Rgba,
    border: Option<Rgba>,
) -> Result<(), String> {
    // A transparent one-pixel geometry fringe smooths the silhouette without
    // an extra render target or per-frame rasterization/blur.
    let points = outline(rect, radius);
    let expanded = Rect::new(
        rect.x() - 1,
        rect.y() - 1,
        rect.width() + 2,
        rect.height() + 2,
    );
    let outer = outline(expanded, radius + 1.);
    let edge = border.unwrap_or(fill_color);
    let mut vertices = Vec::with_capacity(points.len() * 2);
    for (&p, &q) in points.iter().zip(&outer) {
        vertices.push(vertex(p, color(edge), (0., 0.)));
        vertices.push(vertex(q, Color::RGBA(edge.r, edge.g, edge.b, 0), (0., 0.)));
    }
    let mut indices = Vec::with_capacity(points.len() * 6);
    for i in 0..points.len() {
        let a = (i * 2) as u16;
        let b = ((i + 1) % points.len() * 2) as u16;
        indices.extend([a, a + 1, b, b, a + 1, b + 1]);
    }
    canvas.render_geometry(&vertices, None, indices.as_slice())?;
    fill(canvas, rect, radius, border.unwrap_or(fill_color))?;
    if border.is_some() && rect.width() > 2 && rect.height() > 2 {
        fill(
            canvas,
            Rect::new(
                rect.x() + 1,
                rect.y() + 1,
                rect.width() - 2,
                rect.height() - 2,
            ),
            (radius - 1.).max(0.),
            fill_color,
        )?;
    }
    Ok(())
}
pub(crate) fn cover(
    canvas: &mut Canvas<Window>,
    texture: &Texture<'_>,
    rect: Rect,
    radius: f32,
    alpha: u8,
) -> Result<(), String> {
    let (vertices, indices) = fan(
        &outline(rect, radius),
        Color::RGBA(255, 255, 255, alpha),
        Some(texture),
        rect,
    );
    canvas.render_geometry(&vertices, Some(texture), indices.as_slice())
}
pub(crate) fn line(
    canvas: &mut Canvas<Window>,
    from: Point,
    to: Point,
    width: f32,
    c: Rgba,
) -> Result<(), String> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx.hypot(dy).max(0.01);
    let (nx, ny) = (-dy / length * width / 2., dx / length * width / 2.);
    let vertices = [
        (from.0 + nx, from.1 + ny),
        (to.0 + nx, to.1 + ny),
        (to.0 - nx, to.1 - ny),
        (from.0 - nx, from.1 - ny),
    ]
    .map(|p| vertex(p, color(c), (0., 0.)));
    canvas.render_geometry(&vertices, None, &[0_u16, 1, 2, 0, 2, 3])
}
pub(crate) fn star(
    canvas: &mut Canvas<Window>,
    rect: Rect,
    c: Rgba,
    filled: bool,
) -> Result<(), String> {
    let center = (
        rect.x() as f32 + rect.width() as f32 / 2.,
        rect.y() as f32 + rect.height() as f32 / 2.,
    );
    let radius = rect.width().min(rect.height()) as f32 / 2.;
    let points: Vec<_> = (0..10)
        .map(|i| {
            let a = -FRAC_PI_2 + i as f32 * PI / 5.;
            let r = radius * if i % 2 == 0 { 1. } else { 0.46 };
            (center.0 + r * a.cos(), center.1 + r * a.sin())
        })
        .collect();
    if filled {
        let (vertices, indices) = fan(&points, color(c), None, rect);
        canvas.render_geometry(&vertices, None, indices.as_slice())
    } else {
        for i in 0..10 {
            line(canvas, points[i], points[(i + 1) % 10], 1.4, c)?;
        }
        Ok(())
    }
}
pub(crate) fn clock_icon(canvas: &mut Canvas<Window>, rect: Rect, c: Rgba) -> Result<(), String> {
    let (cx, cy) = (
        rect.x() as f32 + rect.width() as f32 / 2.,
        rect.y() as f32 + rect.height() as f32 / 2.,
    );
    let r = rect.width() as f32 / 2. - 1.;
    for i in 0..24 {
        let a = i as f32 * TAU / 24.;
        let b = (i + 1) as f32 * TAU / 24.;
        line(
            canvas,
            (cx + r * a.cos(), cy + r * a.sin()),
            (cx + r * b.cos(), cy + r * b.sin()),
            1.3,
            c,
        )?;
    }
    line(canvas, (cx, cy - r * 0.55), (cx, cy), 1.3, c)?;
    line(canvas, (cx, cy), (cx + r * 0.4, cy + r * 0.25), 1.3, c)
}
pub(crate) fn lightning(canvas: &mut Canvas<Window>, rect: Rect, c: Rgba) -> Result<(), String> {
    let (x, y, w, h) = (
        rect.x() as f32,
        rect.y() as f32,
        rect.width() as f32,
        rect.height() as f32,
    );
    let points = [
        (x + w * 0.65, y),
        (x + w * 0.1, y + h * 0.55),
        (x + w * 0.48, y + h * 0.55),
        (x + w * 0.35, y + h),
        (x + w * 0.9, y + h * 0.4),
        (x + w * 0.52, y + h * 0.4),
    ];
    let vertices: Vec<_> = points
        .iter()
        .map(|&p| vertex(p, color(c), (0., 0.)))
        .collect();
    canvas.render_geometry(&vertices, None, &[0_u16, 1, 2, 0, 2, 5, 2, 3, 4, 2, 4, 5])
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
