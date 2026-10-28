//! The level pill: geometry scaled from the 640x480 design, CPU drawing and timing.
use crate::proto::Icon;
use resvg::{tiny_skia as sk, usvg};
use std::time::Duration;

const BACKGROUND: [u8; 4] = [40, 46, 58, 209]; // rgba(40,46,58,.82)
const FILL: [u8; 3] = [0xF1, 0xF3, 0xF7];
const ICON_DARK: &str = "#3A4350";
const ICON_LIGHT: &str = "#F1F3F7";

/// Layout in surface pixels. The surface is the pill plus room for its shadow;
/// the horizontal room equals the 24 px edge gap, so the layer needs no margin.
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
}

impl Geometry {
    pub fn for_output(width: i32, height: i32) -> Self {
        let s = (width as f32 / 640.0).min(height as f32 / 480.0).max(0.5);
        let (pad_x, pad_y) = ((24.0 * s).round(), (32.0 * s).round());
        let (w, h) = ((44.0 * s).round(), (136.0 * s).round());
        Self {
            width: (w + 2.0 * pad_x) as u32,
            height: (h + 2.0 * pad_y) as u32,
            pill: (pad_x, pad_y, w, h),
            radius: 16.0 * s,
            icon_size: (22.0 * s).round() as u32,
            icon_bottom: 12.0 * s,
            shadow_dy: 8.0 * s,
            shadow_blur: 24.0 * s,
        }
    }

    /// Top edge of the light fill for a 0..=1 level, on a whole pixel so the
    /// edge stays crisp.
    pub fn fill_top(&self, level: f32) -> f32 {
        let (_, y, _, h) = self.pill;
        (y + h * (1.0 - level.clamp(0.0, 1.0))).round()
    }

    pub fn icon_origin(&self) -> (i32, i32) {
        let (x, y, w, h) = self.pill;
        let size = self.icon_size as f32;
        (
            (x + (w - size) / 2.0).round() as i32,
            (y + h - self.icon_bottom - size).round() as i32,
        )
    }
}

/// Everything that does not change between frames, built once per geometry.
pub struct Painter {
    pub geometry: Geometry,
    shadow: sk::Pixmap,
    pill: sk::Mask,
    icons: Vec<(Icon, sk::Pixmap, sk::Pixmap)>,
}

impl Painter {
    pub fn new(geometry: Geometry) -> Self {
        let (w, h) = (geometry.width, geometry.height);
        let path = rounded_rect(geometry.pill, geometry.radius);
        let mut pill = sk::Mask::new(w, h).expect("pill size");
        pill.fill_path(
            &path,
            sk::FillRule::Winding,
            true,
            sk::Transform::identity(),
        );
        let icons = Icon::ALL
            .iter()
            .map(|&icon| {
                let raster = |color| rasterize(icon.svg(), color, geometry.icon_size);
                (icon, raster(ICON_DARK), raster(ICON_LIGHT))
            })
            .collect();
        Self {
            shadow: shadow(&geometry, &path, &pill),
            geometry,
            pill,
            icons,
        }
    }

    /// One frame: `level` 0..=1, `opacity` 0..=1.
    pub fn paint(&self, icon: Icon, level: f32, opacity: f32) -> sk::Pixmap {
        let g = &self.geometry;
        let mut out = sk::Pixmap::new(g.width, g.height).expect("surface size");
        let identity = sk::Transform::identity();
        let plain = sk::PixmapPaint::default();
        out.draw_pixmap(0, 0, self.shadow.as_ref(), &plain, identity, None);

        let mut paint = sk::Paint {
            anti_alias: true,
            ..Default::default()
        };
        paint.set_color_rgba8(BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], BACKGROUND[3]);
        let path = rounded_rect(g.pill, g.radius);
        out.fill_path(&path, &paint, sk::FillRule::Winding, identity, None);

        let (x, _, w, _) = g.pill;
        let top = g.fill_top(level);
        let bottom = (g.height as f32).min(g.pill.1 + g.pill.3 + 1.0);
        paint.set_color_rgba8(FILL[0], FILL[1], FILL[2], 255);
        if let Some(rect) = sk::Rect::from_ltrb(x, top, x + w, bottom) {
            out.fill_rect(rect, &paint, identity, Some(&self.pill));
        }

        // Dark where the icon sits on the fill, light where it sits on the
        // background, so the glyph stays legible at any level.
        if let Some((_, dark, light)) = self.icons.iter().find(|(i, _, _)| *i == icon) {
            let (ix, iy) = g.icon_origin();
            for (pixmap, above) in [(light, true), (dark, false)] {
                let mut clip = sk::Mask::new(g.width, g.height).expect("clip size");
                let rect = if above {
                    sk::Rect::from_ltrb(0.0, 0.0, g.width as f32, top)
                } else {
                    sk::Rect::from_ltrb(0.0, top, g.width as f32, g.height as f32)
                };
                if let Some(rect) = rect {
                    clip.fill_path(
                        &sk::PathBuilder::from_rect(rect),
                        sk::FillRule::Winding,
                        true,
                        identity,
                    );
                    out.draw_pixmap(ix, iy, pixmap.as_ref(), &plain, identity, Some(&clip));
                }
            }
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

fn rounded_rect((x, y, w, h): (f32, f32, f32, f32), r: f32) -> sk::Path {
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

fn rasterize(svg: &str, color: &str, size: u32) -> sk::Pixmap {
    let mut pixmap = sk::Pixmap::new(size, size).expect("icon size");
    let source = svg.replace("currentColor", color);
    if let Ok(tree) = usvg::Tree::from_str(&source, &usvg::Options::default()) {
        let k = size as f32 / tree.size().width();
        resvg::render(&tree, sk::Transform::from_scale(k, k), &mut pixmap.as_mut());
    }
    pixmap
}

pub const TWEEN: Duration = Duration::from_millis(120);
pub const FADE_IN: Duration = Duration::from_millis(120);
pub const HOLD: Duration = Duration::from_millis(1500);
pub const FADE_OUT: Duration = Duration::from_millis(300);

/// Timing of one appearance: everything is a function of the elapsed times,
/// so a frame can be produced for any instant.
#[derive(Clone, Copy, Debug)]
pub struct Timeline {
    pub icon: Icon,
    from: f32,
    target: f32,
}

impl Timeline {
    pub fn new(icon: Icon, level: u8) -> Self {
        let level = level as f32 / 100.0;
        Self {
            icon,
            from: level,
            target: level,
        }
    }

    /// A new value arriving `since_change` after the previous one.
    pub fn retarget(&mut self, icon: Icon, level: u8, since_change: Duration) {
        self.from = self.level(since_change);
        self.target = level as f32 / 100.0;
        self.icon = icon;
    }

    pub fn level(&self, since_change: Duration) -> f32 {
        let t = (since_change.as_secs_f32() / TWEEN.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        self.from + (self.target - self.from) * eased
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
        let g = Geometry::for_output(640, 480);
        assert_eq!(g.pill, (24.0, 32.0, 44.0, 136.0));
        assert_eq!((g.width, g.height), (92, 200));
        assert_eq!((g.radius, g.icon_size), (16.0, 22));
        // 24 px from the right edge with the surface anchored flush right.
        assert_eq!(g.width as f32 - (g.pill.0 + g.pill.2), 24.0);
        // Icon centred, its bottom 12 px above the pill's.
        assert_eq!(g.icon_origin(), (35, 134));
        assert_eq!(g.fill_top(0.6), 86.0);
        assert_eq!(g.fill_top(0.0), 168.0);
        assert_eq!(g.fill_top(1.5), 32.0);
    }

    #[test]
    fn geometry_scales_with_the_smaller_axis() {
        let g = Geometry::for_output(1280, 720);
        assert_eq!(g.pill, (36.0, 48.0, 66.0, 204.0));
        assert_eq!(g.icon_size, 33);
        assert_eq!(Geometry::for_output(0, 0), Geometry::for_output(320, 240));
    }

    fn pixel(p: &sk::Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = p.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn paint_fills_from_the_bottom_with_the_design_colours() {
        let painter = Painter::new(Geometry::for_output(640, 480));
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
        let painter = Painter::new(Geometry::for_output(640, 480));
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
        assert!((t.level(ms(60)) - 0.45).abs() < 0.01);
        assert_eq!(t.level(TWEEN), 0.4);
        assert_eq!(t.icon, Icon::Volume1);

        assert_eq!(opacity(ms(0), ms(0)), 0.0);
        assert_eq!(opacity(FADE_IN, ms(0)), 1.0);
        assert_eq!(opacity(ms(5000), HOLD), 1.0);
        assert!((opacity(ms(5000), HOLD + ms(150)) - 0.5).abs() < 0.01);
        assert_eq!(opacity(ms(5000), HOLD + FADE_OUT), 0.0);
    }

    #[test]
    fn frames_stop_while_holding_and_after_the_fade() {
        let ms = Duration::from_millis;
        assert_eq!(next_frame(ms(0), ms(0)), Some(ms(16)));
        assert_eq!(next_frame(ms(500), ms(500)), Some(ms(1000)));
        assert_eq!(next_frame(ms(2000), HOLD), Some(ms(16)));
        assert_eq!(next_frame(ms(2000), HOLD + FADE_OUT), None);
    }

    /// Writes 640x480 captures over a mockup-like scene:
    /// `ARCHR_OSD_SHOTS=dir cargo test -p archr-osd -- --ignored shots`.
    #[test]
    #[ignore]
    fn shots() {
        let dir = std::path::PathBuf::from(std::env::var("ARCHR_OSD_SHOTS").unwrap());
        let painter = Painter::new(Geometry::for_output(640, 480));
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
}
