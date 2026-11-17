//! The level pill (or capsule): geometry scaled from the 640x480 design,
//! CPU drawing and timing.
use crate::look::{Level, Shape};
use crate::proto::Icon;
use resvg::{tiny_skia as sk, usvg};
use std::time::Duration;

const BACKGROUND: [u8; 4] = [40, 46, 58, 209]; // rgba(40,46,58,.82)
const FILL: [u8; 4] = [0xF1, 0xF3, 0xF7, 255];
/// The capsule's dark glass, its rim and the shine at its top.
const GLASS: [u8; 4] = [22, 25, 31, 166]; // rgba(22,25,31,.65)
const RIM: [u8; 4] = [255, 255, 255, 56];
const SHINE: u8 = 41;
const ICON_DARK: &str = "#3A4350";
const ICON_LIGHT: &str = "#F1F3F7";

/// Layout in surface pixels. The surface is the pill plus room for its shadow;
/// the room across equals the 24 px edge gap, so the layer needs no margin.
/// At the top and bottom edges the pill lies down: width and height swap
/// and the level fills from the left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub pill: (f32, f32, f32, f32),
    pub radius: f32,
    pub icon_size: u32,
    pub icon_bottom: f32,
    pub shadow_dy: f32,
    pub shadow_blur: f32,
    pub lying: bool,
}

impl Geometry {
    pub fn for_output(width: i32, height: i32, level: &Level) -> Self {
        let s = (width as f32 / 640.0).min(height as f32 / 480.0).max(0.5);
        let capsule = level.shape == Shape::Capsule;
        let (pad_x, pad_y) = ((24.0 * s).round(), (32.0 * s).round());
        let (w, h) = if capsule {
            (50.0, 200.0)
        } else {
            (44.0, 136.0)
        };
        let (w, h) = ((w * s).round(), (h * s).round());
        let radius = level
            .radius
            .map_or(if capsule { 18.0 } else { 16.0 }, |r| r * 480.0);
        let lying = level.edge.lying();
        let (pill, size) = if lying {
            ((pad_y, pad_x, h, w), (h + 2.0 * pad_y, w + 2.0 * pad_x))
        } else {
            ((pad_x, pad_y, w, h), (w + 2.0 * pad_x, h + 2.0 * pad_y))
        };
        Self {
            width: size.0 as u32,
            height: size.1 as u32,
            pill,
            radius: radius * s,
            icon_size: ((if capsule { 24.0 } else { 22.0 }) * s).round() as u32,
            icon_bottom: (if capsule { 16.0 } else { 12.0 }) * s,
            shadow_dy: 8.0 * s,
            shadow_blur: 24.0 * s,
            lying,
        }
    }

    /// Where the light fill ends for a 0..=1 level: its top edge, or its
    /// right edge lying down; on a whole pixel so the edge stays crisp.
    pub fn fill_edge(&self, level: f32) -> f32 {
        let (x, y, w, h) = self.pill;
        let level = level.clamp(0.0, 1.0);
        if self.lying {
            (x + w * level).round()
        } else {
            (y + h * (1.0 - level)).round()
        }
    }

    pub fn icon_origin(&self) -> (i32, i32) {
        let (x, y, w, h) = self.pill;
        let size = self.icon_size as f32;
        if self.lying {
            (
                (x + self.icon_bottom).round() as i32,
                (y + (h - size) / 2.0).round() as i32,
            )
        } else {
            (
                (x + (w - size) / 2.0).round() as i32,
                (y + h - self.icon_bottom - size).round() as i32,
            )
        }
    }
}

/// Everything that does not change between frames, built once per geometry
/// and look: for each icon the pill empty (light icon) and full (dark icon).
/// A frame is the empty one beyond the fill's edge and the full one before
/// it, so drawing it is a copy of rows, cheap enough for every frame of a
/// held button.
pub struct Painter {
    pub geometry: Geometry,
    pub level: Level,
    layers: Vec<(Icon, sk::Pixmap, sk::Pixmap)>,
}

impl Painter {
    pub fn new(geometry: Geometry, level: Level) -> Self {
        let (w, h) = (geometry.width, geometry.height);
        let path = rounded_rect(geometry.pill, geometry.radius);
        let mut pill = sk::Mask::new(w, h).expect("pill size");
        pill.fill_path(
            &path,
            sk::FillRule::Winding,
            true,
            sk::Transform::identity(),
        );
        let shadow = shadow(&geometry, &path, &pill);
        let layers = Icon::ALL
            .iter()
            .map(|&icon| {
                let layer = |full| layer(&geometry, &level, &path, &pill, &shadow, icon, full);
                (icon, layer(false), layer(true))
            })
            .collect();
        Self {
            geometry,
            level,
            layers,
        }
    }

    /// One frame: `level` 0..=1, `opacity` 0..=1.
    pub fn paint(&self, icon: Icon, level: f32, opacity: f32) -> sk::Pixmap {
        let g = &self.geometry;
        let (_, empty, full) = self
            .layers
            .iter()
            .find(|(i, _, _)| *i == icon)
            .expect("every icon has layers");
        let mut out = empty.clone();
        let edge = g.fill_edge(level) as usize * 4;
        if g.lying {
            let row = g.width as usize * 4;
            let edge = edge.min(row);
            for (dst, src) in out
                .data_mut()
                .chunks_exact_mut(row)
                .zip(full.data().chunks_exact(row))
            {
                dst[..edge].copy_from_slice(&src[..edge]);
            }
        } else {
            let edge = (edge * g.width as usize).min(out.data().len());
            out.data_mut()[edge..].copy_from_slice(&full.data()[edge..]);
        }
        if opacity < 1.0 {
            let a = (opacity.clamp(0.0, 1.0) * 256.0) as u16;
            for byte in out.data_mut() {
                *byte = ((*byte as u16 * a) >> 8) as u8;
            }
        }
        out
    }
}

/// Light enough for the dark icon.
fn light([r, g, b, _]: [u8; 4]) -> bool {
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b) > 140.0
}

/// Shadow, pill and icon; `full`: the pill filled, the icon dark on it.
/// The capsule's glass also takes a shine at its top and a light rim.
fn layer(
    g: &Geometry,
    level: &Level,
    path: &sk::Path,
    pill: &sk::Mask,
    shadow: &sk::Pixmap,
    icon: Icon,
    full: bool,
) -> sk::Pixmap {
    let capsule = level.shape == Shape::Capsule;
    let track = level
        .track
        .unwrap_or(if capsule { GLASS } else { BACKGROUND });
    let fill = level.fill.unwrap_or(FILL);
    let mut out = shadow.clone();
    let identity = sk::Transform::identity();
    let mut paint = sk::Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.set_color_rgba8(track[0], track[1], track[2], track[3]);
    out.fill_path(path, &paint, sk::FillRule::Winding, identity, None);
    let (x, y, w, h) = g.pill;
    if capsule {
        let stops = vec![
            sk::GradientStop::new(0.0, sk::Color::from_rgba8(255, 255, 255, SHINE)),
            sk::GradientStop::new(1.0, sk::Color::from_rgba8(255, 255, 255, 0)),
        ];
        let reach = if g.lying { h * 0.6 } else { h * 0.35 };
        if let Some(shader) = sk::LinearGradient::new(
            sk::Point::from_xy(x, y),
            sk::Point::from_xy(x, y + reach),
            stops,
            sk::SpreadMode::Pad,
            identity,
        ) {
            let shine = sk::Paint {
                shader,
                anti_alias: true,
                ..Default::default()
            };
            out.fill_path(path, &shine, sk::FillRule::Winding, identity, None);
        }
    }
    if full {
        let (right, bottom) = (
            (g.width as f32).min(x + w + 1.0),
            (g.height as f32).min(y + h + 1.0),
        );
        paint.set_color_rgba8(fill[0], fill[1], fill[2], fill[3]);
        if let Some(rect) = sk::Rect::from_ltrb(x, y, right, bottom) {
            out.fill_rect(rect, &paint, identity, Some(pill));
        }
    }
    if capsule {
        let rim = rounded_rect((x + 0.5, y + 0.5, w - 1.0, h - 1.0), g.radius - 0.5);
        paint.set_color_rgba8(RIM[0], RIM[1], RIM[2], RIM[3]);
        let stroke = sk::Stroke {
            width: 1.0,
            ..Default::default()
        };
        out.stroke_path(&rim, &paint, &stroke, identity, None);
    }
    // Dark where the icon sits on a light fill, light where it sits on the
    // background, so the glyph stays legible at any level.
    let glyph = rasterize(
        icon.svg(),
        if full && light(fill) {
            ICON_DARK
        } else {
            ICON_LIGHT
        },
        g.icon_size,
    );
    let (ix, iy) = g.icon_origin();
    out.draw_pixmap(
        ix,
        iy,
        glyph.as_ref(),
        &sk::PixmapPaint::default(),
        identity,
        None,
    );
    out
}

pub(crate) fn rounded_rect((x, y, w, h): (f32, f32, f32, f32), r: f32) -> sk::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = r * 0.447_715; // 1 - 0.552285: cubic quarter-circle handle
    let (right, bottom) = (x + w, y + h);
    let mut p = sk::PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(right - r, y);
    p.cubic_to(right - k, y, right, y + k, right, y + r);
    p.line_to(right, bottom - r);
    p.cubic_to(right, bottom - k, right - k, bottom, right - r, bottom);
    p.line_to(x + r, bottom);
    p.cubic_to(x + k, bottom, x, bottom - k, x, bottom - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + k, x + k, y, x + r, y);
    p.close();
    p.finish().expect("rounded rect")
}

/// CSS `0 8px 24px rgba(0,0,0,.4)`: the pill's silhouette, moved down and
/// blurred with three box passes (a close Gaussian, sigma = blur / 2). As in
/// CSS, none of it lies under the pill itself.
fn shadow(g: &Geometry, path: &sk::Path, pill: &sk::Mask) -> sk::Pixmap {
    let (w, h) = (g.width as usize, g.height as usize);
    let mut mask = sk::Mask::new(g.width, g.height).expect("shadow size");
    let down = sk::Transform::from_translate(0.0, g.shadow_dy);
    mask.fill_path(path, sk::FillRule::Winding, true, down);
    let mut alpha: Vec<f32> = mask.data().iter().map(|&a| a as f32 * 0.4).collect();
    let radius = (g.shadow_blur / 2.0).round() as usize;
    for _ in 0..3 {
        box_blur(&mut alpha, w, h, 1, w, radius);
        box_blur(&mut alpha, h, w, w, 1, radius);
    }
    let mut out = sk::Pixmap::new(g.width, g.height).expect("shadow size");
    for ((pixel, a), inside) in out
        .data_mut()
        .chunks_exact_mut(4)
        .zip(alpha)
        .zip(pill.data())
    {
        pixel[3] = (a * (1.0 - *inside as f32 / 255.0))
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    out
}

/// Blurs `lines` runs of `len` samples spaced by `step`, each starting `pitch` apart.
fn box_blur(data: &mut [f32], len: usize, lines: usize, step: usize, pitch: usize, r: usize) {
    let mut line = vec![0.0; len];
    let scale = 1.0 / (2 * r + 1) as f32;
    for l in 0..lines {
        for (i, v) in line.iter_mut().enumerate() {
            *v = data[l * pitch + i * step];
        }
        let mut sum: f32 = line.iter().take(r).sum();
        for i in 0..len {
            sum += line.get(i + r).copied().unwrap_or(0.0);
            data[l * pitch + i * step] = sum * scale;
            if i >= r {
                sum -= line[i - r];
            }
        }
    }
}

pub(crate) fn rasterize(svg: &str, color: &str, size: u32) -> sk::Pixmap {
    let mut pixmap = sk::Pixmap::new(size, size).expect("icon size");
    let source = svg.replace("currentColor", color);
    if let Ok(tree) = usvg::Tree::from_str(&source, &usvg::Options::default()) {
        let k = size as f32 / tree.size().width();
        resvg::render(&tree, sk::Transform::from_scale(k, k), &mut pixmap.as_mut());
    }
    pixmap
}

/// How long the level glides after its last change; it is still by then.
pub const TWEEN: Duration = Duration::from_millis(200);
/// Stiffness of the glide (1/s): a critically damped spring.
const SPRING: f32 = 40.0;
pub const FADE_IN: Duration = Duration::from_millis(120);
pub const HOLD: Duration = Duration::from_millis(1500);
pub const FADE_OUT: Duration = Duration::from_millis(300);

/// Timing of one appearance: everything is a function of the elapsed times,
/// so a frame can be produced for any instant. The level follows its target
/// as a critically damped spring that keeps its speed when the target moves:
/// a held button's stream of small steps becomes one even glide.
#[derive(Clone, Copy, Debug)]
pub struct Timeline {
    pub icon: Icon,
    from: f32,
    velocity: f32,
    target: f32,
}

impl Timeline {
    pub fn new(icon: Icon, level: u8) -> Self {
        let level = level as f32 / 100.0;
        Self {
            icon,
            from: level,
            velocity: 0.0,
            target: level,
        }
    }

    /// A new value arriving `since_change` after the previous one.
    pub fn retarget(&mut self, icon: Icon, level: u8, since_change: Duration) {
        (self.from, self.velocity) = self.state(since_change);
        self.target = level as f32 / 100.0;
        self.icon = icon;
    }

    /// Level and its speed per second.
    fn state(&self, since_change: Duration) -> (f32, f32) {
        if since_change >= TWEEN {
            return (self.target, 0.0);
        }
        let t = since_change.as_secs_f32();
        let a = self.from - self.target;
        let b = self.velocity + SPRING * a;
        let decay = (-SPRING * t).exp();
        (
            self.target + (a + b * t) * decay,
            (b - SPRING * (a + b * t)) * decay,
        )
    }

    pub fn level(&self, since_change: Duration) -> f32 {
        self.state(since_change).0.clamp(0.0, 1.0)
    }
}

pub fn opacity(since_shown: Duration, since_change: Duration) -> f32 {
    let fade_in = since_shown.as_secs_f32() / FADE_IN.as_secs_f32();
    let fade_out = 1.0 - since_change.saturating_sub(HOLD).as_secs_f32() / FADE_OUT.as_secs_f32();
    fade_in.min(fade_out).clamp(0.0, 1.0)
}

/// When the next frame is due, or None once the pill has faded out.
pub fn next_frame(since_shown: Duration, since_change: Duration) -> Option<Duration> {
    const FRAME: Duration = Duration::from_millis(16);
    if since_change >= HOLD + FADE_OUT {
        None
    } else if since_change < TWEEN || since_shown < FADE_IN || since_change >= HOLD {
        Some(FRAME)
    } else {
        Some(HOLD - since_change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_matches_the_design_at_640x480() {
        let g = Geometry::for_output(640, 480, &Level::default());
        assert_eq!(g.pill, (24.0, 32.0, 44.0, 136.0));
        assert_eq!((g.width, g.height), (92, 200));
        assert_eq!((g.radius, g.icon_size), (16.0, 22));
        // 24 px from the right edge with the surface anchored flush right.
        assert_eq!(g.width as f32 - (g.pill.0 + g.pill.2), 24.0);
        // Icon centred, its bottom 12 px above the pill's.
        assert_eq!(g.icon_origin(), (35, 134));
        assert_eq!(g.fill_edge(0.6), 86.0);
        assert_eq!(g.fill_edge(0.0), 168.0);
        assert_eq!(g.fill_edge(1.5), 32.0);
    }

    #[test]
    fn geometry_scales_with_the_smaller_axis() {
        let g = Geometry::for_output(1280, 720, &Level::default());
        assert_eq!(g.pill, (36.0, 48.0, 66.0, 204.0));
        assert_eq!(g.icon_size, 33);
        assert_eq!(
            Geometry::for_output(0, 0, &Level::default()),
            Geometry::for_output(320, 240, &Level::default())
        );
    }

    fn pixel(p: &sk::Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = p.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn paint_fills_from_the_bottom_with_the_design_colours() {
        let painter = Painter::new(
            Geometry::for_output(640, 480, &Level::default()),
            Level::default(),
        );
        let p = painter.paint(Icon::Volume2, 0.6, 1.0);
        assert_eq!(pixel(&p, 30, 160), [0xF1, 0xF3, 0xF7, 255]);
        // Premultiplied storage: allow one step of rounding.
        let bg = pixel(&p, 30, 60);
        assert!(
            bg.iter()
                .zip([40, 46, 58, 209])
                .all(|(&a, b)| a.abs_diff(b) <= 1),
            "{bg:?}"
        );
        // Rounded corner: outside the pill only the shadow remains.
        assert_eq!(pixel(&p, 24, 32)[..3], [0, 0, 0]);
        assert!(pixel(&p, 24, 32)[3] < 100);
        // The dark icon is drawn on the fill.
        let dark = (134..156)
            .flat_map(|y| (35..57).map(move |x| (x, y)))
            .any(|(x, y)| pixel(&p, x, y)[..3] == [0x3A, 0x43, 0x50]);
        assert!(dark);
        assert!(painter
            .paint(Icon::Sun, 0.6, 0.0)
            .data()
            .iter()
            .all(|&b| b == 0));
    }

    #[test]
    fn icon_turns_light_above_the_fill() {
        let painter = Painter::new(
            Geometry::for_output(640, 480, &Level::default()),
            Level::default(),
        );
        let p = painter.paint(Icon::VolumeX, 0.0, 1.0);
        let light = (134..156)
            .flat_map(|y| (35..57).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&p, x, y)[0] > 200)
            .count();
        assert!(light > 20, "{light}");
    }

    #[test]
    fn timeline_tweens_then_holds_then_fades() {
        let ms = Duration::from_millis;
        let mut t = Timeline::new(Icon::Volume2, 60);
        assert_eq!(t.level(ms(0)), 0.6);
        t.retarget(Icon::Volume1, 40, ms(500));
        assert_eq!(t.level(ms(0)), 0.6);
        let mut last = 0.6;
        for step in 1..20 {
            let level = t.level(ms(step * 10));
            assert!(level <= last && level >= 0.4, "{step}: {level}");
            last = level;
        }
        assert!((t.level(ms(60)) - 0.46).abs() < 0.01);
        assert!((t.level(TWEEN - ms(1)) - 0.4).abs() < 0.002);
        assert_eq!(t.level(TWEEN), 0.4);
        assert_eq!(t.icon, Icon::Volume1);

        assert_eq!(opacity(ms(0), ms(0)), 0.0);
        assert_eq!(opacity(FADE_IN, ms(0)), 1.0);
        assert_eq!(opacity(ms(5000), HOLD), 1.0);
        assert!((opacity(ms(5000), HOLD + ms(150)) - 0.5).abs() < 0.01);
        assert_eq!(opacity(ms(5000), HOLD + FADE_OUT), 0.0);
    }

    #[test]
    fn a_stream_of_steps_glides_without_jumps() {
        // A held button: +1 every 40 ms. The pill never jumps or turns back
        // and trails the newest level by a steady few percent.
        let ms = Duration::from_millis;
        let mut t = Timeline::new(Icon::Volume1, 20);
        let mut last = 0.2;
        for step in 1..=30u8 {
            t.retarget(Icon::Volume1, 20 + step, ms(40));
            for frame in 0..=40 {
                let level = t.level(ms(frame));
                assert!(
                    level >= last - 1e-6 && level - last < 0.006,
                    "{step}/{frame}: {level}"
                );
                last = level;
            }
        }
        assert!((0.5 - last) < 0.04 && last < 0.5, "{last}");
    }

    #[test]
    fn frames_stop_while_holding_and_after_the_fade() {
        let ms = Duration::from_millis;
        assert_eq!(next_frame(ms(0), ms(0)), Some(ms(16)));
        assert_eq!(next_frame(ms(500), ms(500)), Some(ms(1000)));
        assert_eq!(next_frame(ms(2000), HOLD), Some(ms(16)));
        assert_eq!(next_frame(ms(2000), HOLD + FADE_OUT), None);
    }

    fn capsule(edge: crate::look::Edge) -> Level {
        Level {
            shape: Shape::Capsule,
            edge,
            ..Level::default()
        }
    }

    #[test]
    fn the_capsule_is_taller_and_lies_down_at_the_top_and_bottom() {
        use crate::look::Edge;
        let g = Geometry::for_output(640, 480, &capsule(Edge::Right));
        assert_eq!(g.pill, (24.0, 32.0, 50.0, 200.0));
        assert_eq!(
            (g.width, g.height, g.radius, g.icon_size),
            (98, 264, 18.0, 24)
        );
        assert_eq!(g.icon_origin(), (37, 192));
        let own = Level {
            radius: Some(0.05),
            ..capsule(Edge::Left)
        };
        assert_eq!(Geometry::for_output(640, 480, &own).radius, 24.0);
        let lying = Geometry::for_output(640, 480, &capsule(Edge::Bottom));
        assert!(lying.lying);
        assert_eq!(lying.pill, (32.0, 24.0, 200.0, 50.0));
        assert_eq!((lying.width, lying.height), (264, 98));
        assert_eq!(lying.fill_edge(0.25), 82.0);
        assert_eq!(lying.icon_origin(), (48, 37));
    }

    #[test]
    fn the_capsule_is_glass_with_a_rim_and_a_light_fill() {
        use crate::look::Edge;
        let level = capsule(Edge::Right);
        let painter = Painter::new(Geometry::for_output(640, 480, &level), level);
        let p = painter.paint(Icon::Volume2, 0.5, 1.0);
        // The fill below the middle, the dark icon on it.
        assert_eq!(pixel(&p, 49, 170), [0xF1, 0xF3, 0xF7, 255]);
        let dark = (192..216)
            .flat_map(|y| (37..61).map(move |x| (x, y)))
            .any(|(x, y)| pixel(&p, x, y)[..3] == [0x3A, 0x43, 0x50]);
        assert!(dark);
        // Translucent glass above it, brighter at the top (the shine).
        let low = pixel(&p, 49, 125);
        assert!(low[3].abs_diff(166) <= 2 && low[0] < 40, "{low:?}");
        let high = pixel(&p, 49, 40);
        assert!(
            high[0] > low[0] + 10 && high[3] > low[3],
            "{high:?} {low:?}"
        );
        // The light rim on the glass's edge.
        let rim = pixel(&p, 24, 120);
        assert!(rim[0] > low[0] + 20, "{rim:?}");
        // Lying down it fills from the left.
        let level = capsule(Edge::Top);
        let painter = Painter::new(Geometry::for_output(640, 480, &level), level);
        let p = painter.paint(Icon::Sun, 0.5, 1.0);
        assert_eq!(pixel(&p, 100, 49), [0xF1, 0xF3, 0xF7, 255]);
        assert!(pixel(&p, 160, 49)[3] < 200);
        // A theme's own colours; a dark fill takes the light icon.
        let level = Level {
            fill: Some([0x10, 0x40, 0x90, 255]),
            track: Some([200, 0, 0, 255]),
            ..capsule(Edge::Right)
        };
        let painter = Painter::new(Geometry::for_output(640, 480, &level), level);
        let p = painter.paint(Icon::Volume2, 0.5, 1.0);
        assert_eq!(pixel(&p, 49, 170), [0x10, 0x40, 0x90, 255]);
        assert_eq!(pixel(&p, 49, 125)[0], 200);
        let light = (192..216)
            .flat_map(|y| (37..61).map(move |x| (x, y)))
            .any(|(x, y)| pixel(&p, x, y)[0] > 200);
        assert!(light);
    }

    /// The level over a grey 640x480 screen, in each shape:
    /// `ARCHR_OSD_STYLES=dir cargo test -p archr-osd -- --ignored styles`.
    #[test]
    #[ignore]
    fn styles() {
        use crate::look::Edge;
        let dir = std::path::PathBuf::from(std::env::var("ARCHR_OSD_STYLES").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let cases = [
            ("pill-volume-60", Level::default(), Icon::Volume2, 0.6),
            ("pill-volume-20", Level::default(), Icon::Volume1, 0.2),
            (
                "capsule-volume-60",
                capsule(Edge::Right),
                Icon::Volume2,
                0.6,
            ),
            (
                "capsule-volume-20",
                capsule(Edge::Right),
                Icon::Volume1,
                0.2,
            ),
            (
                "capsule-volume-0-muted",
                capsule(Edge::Right),
                Icon::VolumeX,
                0.0,
            ),
            (
                "capsule-brightness-80-left",
                capsule(Edge::Left),
                Icon::Sun,
                0.8,
            ),
            (
                "capsule-volume-60-top",
                capsule(Edge::Top),
                Icon::Volume2,
                0.6,
            ),
            (
                "capsule-volume-40-bottom",
                capsule(Edge::Bottom),
                Icon::Volume1,
                0.4,
            ),
        ];
        for (name, level, icon, value) in cases {
            let g = Geometry::for_output(640, 480, &level);
            let painter = Painter::new(g, level);
            let mut scene = sk::Pixmap::new(640, 480).unwrap();
            scene.fill(sk::Color::from_rgba8(0x80, 0x80, 0x80, 255));
            let (w, h) = (g.width as i32, g.height as i32);
            let (x, y) = match level.edge {
                Edge::Right => (640 - w, (480 - h) / 2),
                Edge::Left => (0, (480 - h) / 2),
                Edge::Top => ((640 - w) / 2, 0),
                Edge::Bottom => ((640 - w) / 2, 480 - h),
            };
            let frame = painter.paint(icon, value, 1.0);
            scene.draw_pixmap(
                x,
                y,
                frame.as_ref(),
                &sk::PixmapPaint::default(),
                sk::Transform::identity(),
                None,
            );
            scene.save_png(dir.join(format!("{name}.png"))).unwrap();
        }
    }

    /// Writes 640x480 captures over a mockup-like scene:
    /// `ARCHR_OSD_SHOTS=dir cargo test -p archr-osd -- --ignored shots`.
    #[test]
    #[ignore]
    fn shots() {
        let dir = std::path::PathBuf::from(std::env::var("ARCHR_OSD_SHOTS").unwrap());
        let painter = Painter::new(
            Geometry::for_output(640, 480, &Level::default()),
            Level::default(),
        );
        let g = painter.geometry;
        let cases = [
            ("volume-0", "volume 0"),
            ("volume-30", "volume 30"),
            ("volume-60", "volume 60"),
            ("volume-100", "volume 100"),
            ("volume-60-muted", "volume 60 muted"),
            ("brightness-40", "brightness 40"),
        ];
        for (name, text) in cases {
            let m = crate::proto::parse(text).unwrap();
            let mut scene = sk::Pixmap::new(640, 480).unwrap();
            let stops = [
                (0.0, [0x3b, 0x5a, 0x8a]),
                (0.45, [0x1d, 0x2b, 0x45]),
                (1.0, [0x0d, 0x13, 0x20]),
            ]
            .map(|(t, [r, g, b])| sk::GradientStop::new(t, sk::Color::from_rgba8(r, g, b, 255)));
            let shader = sk::RadialGradient::new(
                sk::Point::from_xy(192.0, 192.0),
                sk::Point::from_xy(192.0, 192.0),
                560.0,
                stops.to_vec(),
                sk::SpreadMode::Pad,
                sk::Transform::identity(),
            )
            .unwrap();
            let paint = sk::Paint {
                shader,
                ..Default::default()
            };
            scene.fill_rect(
                sk::Rect::from_xywh(0.0, 0.0, 640.0, 480.0).unwrap(),
                &paint,
                sk::Transform::identity(),
                None,
            );
            let frame = painter.paint(m.icon, m.level as f32 / 100.0, 1.0);
            let (x, y) = (640 - g.width as i32, (480 - g.height as i32) / 2);
            scene.draw_pixmap(
                x,
                y,
                frame.as_ref(),
                &sk::PixmapPaint::default(),
                sk::Transform::identity(),
                None,
            );
            scene.save_png(dir.join(format!("{name}.png"))).unwrap();
        }
    }

    /// The volume at 60 % over launcher screens, drawn with an
    /// osd-theme.toml: ARCHR_OSD_LOOK=<osd-theme.toml>
    /// ARCHR_OSD_SCENES=<dir of *-WxH.png> ARCHR_OSD_OUT=<dir>
    /// `cargo test -p archr-osd -- --ignored over_screens`. Writes
    /// osd-volume-<scene>.png, the level where the OSD puts it.
    #[test]
    #[ignore]
    fn over_screens() {
        use crate::look::Edge;
        let var = |name| std::path::PathBuf::from(std::env::var(name).unwrap());
        let look = crate::look::Look::load(&var("ARCHR_OSD_LOOK"));
        let out = var("ARCHR_OSD_OUT");
        std::fs::create_dir_all(&out).unwrap();
        for entry in std::fs::read_dir(var("ARCHR_OSD_SCENES")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("png") {
                continue;
            }
            let mut scene = sk::Pixmap::load_png(&path).unwrap();
            let (sw, sh) = (scene.width() as i32, scene.height() as i32);
            let level = look.level;
            let g = Geometry::for_output(sw, sh, &level);
            let painter = Painter::new(g, level);
            let (w, h) = (g.width as i32, g.height as i32);
            let (x, y) = match level.edge {
                Edge::Right => (sw - w, (sh - h) / 2),
                Edge::Left => (0, (sh - h) / 2),
                Edge::Top => ((sw - w) / 2, 0),
                Edge::Bottom => ((sw - w) / 2, sh - h),
            };
            let frame = painter.paint(Icon::Volume2, 0.6, 1.0);
            scene.draw_pixmap(
                x,
                y,
                frame.as_ref(),
                &sk::PixmapPaint::default(),
                sk::Transform::identity(),
                None,
            );
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            scene
                .save_png(out.join(format!("osd-volume-{name}")))
                .unwrap();
        }
    }
}
