//! MangoHud page (MangoHud, MangoHud-itens): the rows beside a preview of the
//! overlay over a game scene, and the items grid.
use super::*;
use crate::{appearance::rgb, settings::mangohud::ITEMS};

/// Rows stop short of the preview column.
const LIST_RIGHT: f32 = 372.;
const PREVIEW_X: f32 = 390.;
const PREVIEW_W: f32 = 230.;
const SCENE_Y: f32 = 72.;
const SCENE_H: f32 = 172.;
/// The overlay's own colors, not the theme's: this pictures MangoHud.
const INK: u32 = 0xE6EDF3;
const FAINT: u32 = 0x9BA7B4;
const GPU: u32 = 0x2EC4B6;
const CPU: u32 = 0x1E88FF;
const RAM: u32 = 0xF59E0B;
const BATTERY: u32 = 0x22C55E;
/// Frametime graph samples, 0..16 from the top.
const GRAPH: [f32; 14] = [9., 8., 9., 7., 9., 8., 12., 8., 9., 8., 9., 6., 9., 8.];
/// Text, color, face and size of one piece of an overlay line.
type Piece = (String, Rgba, Face, f32);

fn mix(from: u32, to: u32, part: f32) -> Rgba {
    let (a, b) = (rgb(from), rgb(to));
    let blend = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * part) as u8;
    Rgba {
        r: blend(a.r, b.r),
        g: blend(a.g, b.g),
        b: blend(a.b, b.b),
        a: 255,
    }
}

impl View<'_, '_, '_> {
    pub(super) fn mangohud_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        self.rows_at(state, rows, &placements(Page::Mangohud, rows), LIST_RIGHT)?;
        self.text(
            (PREVIEW_X, 50., PREVIEW_W, 18.),
            &self.tr("appearance.preview").to_uppercase(),
            (11., self.theme.muted, Align::Left),
        )?;
        self.scene()?;
        // A wide overlay never spills out of the scene.
        let scene = self.rect((PREVIEW_X, SCENE_Y, PREVIEW_W, SCENE_H));
        self.ui.canvas.set_clip_rect(scene);
        let overlay = self.overlay(state);
        self.ui.canvas.set_clip_rect(None);
        overlay?;
        let note = if state.mangohud_supported {
            "mangohud.preview_note"
        } else {
            "mangohud.unsupported_note"
        };
        self.note(
            (PREVIEW_X, SCENE_Y + SCENE_H + 8., PREVIEW_W, 80.),
            &self.tr(note),
            12.,
        )
    }
    /// Sky and grass in vertical gradients inside a 1 px rounded border.
    fn scene(&mut self) -> Result<(), String> {
        let radius = self.ui.px(12.) as f32;
        let outer = self.rect((PREVIEW_X, SCENE_Y, PREVIEW_W, SCENE_H));
        self.ui.rounded(outer, radius, self.theme.border, None)?;
        let rect = self.rect((PREVIEW_X + 1., SCENE_Y + 1., PREVIEW_W - 2., SCENE_H - 2.));
        let radius = radius - 1.;
        self.ui.rounded(rect, radius, rgb(0x3A6DB5), None)?;
        let ground = Rect::new(
            rect.x(),
            rect.bottom() - 2 * radius as i32,
            rect.width(),
            2 * radius as u32,
        );
        self.ui.rounded(ground, radius, rgb(0x2E6B2A), None)?;
        let (top, bottom) = (rect.y(), rect.bottom());
        let horizon = top + (rect.height() as f32 * 0.555) as i32;
        for y in top + radius as i32..bottom - radius as i32 {
            let ink = if y < horizon {
                mix(
                    0x3A6DB5,
                    0x76B0E8,
                    (y - top) as f32 / (horizon - top) as f32,
                )
            } else {
                mix(
                    0x3F8F3A,
                    0x2E6B2A,
                    (y - horizon) as f32 / (bottom - horizon) as f32,
                )
            };
            self.ui.canvas.set_draw_color(color(ink));
            self.ui
                .canvas
                .fill_rect(Rect::new(rect.x(), y, rect.width(), 1))?;
        }
        Ok(())
    }
    /// The overlay lines for the chosen layout and items, in MangoHud's order.
    fn overlay_lines(state: &Settings, size: f32) -> (Vec<Vec<Piece>>, bool) {
        let items = state.mangohud_items();
        let has = |code: &str| items.contains(&code);
        let layout = state.value(Id::MangohudLayout);
        let piece = |text: &str, ink: u32| (text.to_owned(), rgb(ink), Face::Mono, size);
        let fps = vec![
            ("60".to_owned(), rgb(INK), Face::Semibold, size * 1.37),
            (" FPS".to_owned(), rgb(FAINT), Face::Mono, size * 0.95),
        ];
        if layout == "fps" {
            return (vec![fps], false);
        }
        let stat = |label: &str, ink: u32, parts: &[(&str, &str)]| -> Option<Vec<Piece>> {
            let shown: Vec<&str> = parts
                .iter()
                .filter(|(code, _)| has(code))
                .map(|(_, sample)| *sample)
                .collect();
            let shown = if layout == "horizontal" {
                &shown[..shown.len().min(1)]
            } else {
                &shown[..]
            };
            (!shown.is_empty()).then(|| {
                vec![
                    piece(label, ink),
                    piece(&format!(" {}", shown.join(" · ")), INK),
                ]
            })
        };
        let mut lines: Vec<Vec<Piece>> = [
            has("time").then(|| vec![piece("14:32", INK)]),
            has("game").then(|| vec![piece("retroarch", INK)]),
            stat(
                "GPU",
                GPU,
                &[
                    ("gpu_load", "64%"),
                    ("gpu_temp", "61°C"),
                    ("gpu_freq", "520MHz"),
                ],
            ),
            stat(
                "CPU",
                CPU,
                &[
                    ("cpu_load", "38%"),
                    ("cpu_temp", "58°C"),
                    ("cpu_freq", "1.3GHz"),
                ],
            ),
            stat("RAM", RAM, &[("ram", "412 MB")]),
            stat("BAT", BATTERY, &[("battery", "81%"), ("battery", "3.1 W")]),
            has("fps").then_some(fps),
        ]
        .into_iter()
        .flatten()
        .collect();
        if layout == "horizontal" {
            let mut line = Vec::new();
            for (n, pieces) in lines.into_iter().enumerate() {
                if n > 0 {
                    line.push(piece(" ", INK));
                }
                line.extend(pieces);
            }
            lines = vec![line];
        }
        (lines, has("frametime") && layout != "horizontal")
    }
    /// The overlay box where the position puts it, with the chosen size and
    /// background opacity.
    fn overlay(&mut self, state: &Settings) -> Result<(), String> {
        let mut size = match state.value(Id::MangohudSize) {
            "18" => 8.,
            "30" => 11.5,
            _ => 9.5,
        };
        let scale = self.ui.frame.scale;
        // Shrink a wide horizontal line until it fits the scene.
        let (lines, graph, widths) = loop {
            let (lines, graph) = Self::overlay_lines(state, size);
            let mut widths = Vec::new();
            for line in &lines {
                let mut width = 0.;
                for (text, _, face, size) in line {
                    width += self.ui.measure(text, *face, *size)? as f32 / scale;
                }
                widths.push(width);
            }
            let widest = widths.iter().copied().fold(0., f32::max);
            if widest <= PREVIEW_W - 32. || size <= 7. {
                break (lines, graph, widths);
            }
            size -= 0.5;
        };
        let line_height = size
            * if state.value(Id::MangohudLayout) == "compact" {
                1.4
            } else {
                1.55
            };
        let graph_size = (104. * size / 9.5, 16. * size / 9.5);
        let width = widths
            .iter()
            .copied()
            .fold(if graph { graph_size.0 } else { 0. }, f32::max)
            + 16.;
        let height = lines.len() as f32 * line_height
            + size * 0.4
            + if graph { graph_size.1 + 2. } else { 0. }
            + 10.;
        let position = state.value(Id::MangohudPosition);
        let x = if position.ends_with("left") {
            PREVIEW_X + 8.
        } else if position.ends_with("right") {
            PREVIEW_X + PREVIEW_W - 8. - width
        } else {
            PREVIEW_X + (PREVIEW_W - width) / 2.
        };
        let bottom = position.starts_with("bottom");
        let y = if bottom {
            SCENE_Y + SCENE_H - 8. - height
        } else {
            SCENE_Y + 8.
        };
        let alpha = state
            .value(Id::MangohudAlpha)
            .parse::<u16>()
            .unwrap_or(50)
            .min(100);
        let fill = Rgba {
            r: 10,
            g: 12,
            b: 16,
            a: (alpha * 255 / 100) as u8,
        };
        if alpha > 0 {
            let radius = self.ui.px(6.) as f32;
            self.ui
                .rounded(self.rect((x, y, width, height)), radius, fill, None)?;
        }
        let mut top = y + 5.;
        for line in &lines {
            let mut left = x + 8.;
            for (text, ink, face, size) in line {
                let w = self.ui.measure(text, *face, *size)? as f32 / scale;
                self.ui.label_aligned(
                    text,
                    (*face, *size, *ink),
                    self.rect((left, top, w + 2., line_height)),
                    Align::Left,
                )?;
                left += w;
            }
            top += line_height;
        }
        if graph {
            let (gw, gh) = graph_size;
            let step = gw / (GRAPH.len() - 1) as f32;
            let points: Vec<sdl2::rect::Point> = GRAPH
                .iter()
                .enumerate()
                .map(|(n, v)| {
                    sdl2::rect::Point::new(
                        self.ui.px(x + 8. + n as f32 * step),
                        self.ui.px(top + 2. + v * gh / 16.),
                    )
                })
                .collect();
            self.ui.canvas.set_draw_color(color(rgb(BATTERY)));
            self.ui.canvas.draw_lines(points.as_slice())?;
        }
        // The caption keeps clear of the overlay.
        let caption = if bottom {
            (PREVIEW_X + 8., SCENE_Y + 4., PREVIEW_W - 16., 16.)
        } else {
            (
                PREVIEW_X + 8.,
                SCENE_Y + SCENE_H - 20.,
                PREVIEW_W - 16.,
                16.,
            )
        };
        self.text(
            caption,
            &self.tr("mangohud.game_running"),
            (
                10.,
                Rgba {
                    a: 178,
                    ..rgb(0xFFFFFF)
                },
                Align::Right,
            ),
        )
    }
    /// Items page: a two-column grid of check boxes.
    pub(super) fn mangohud_items(&mut self, state: &Settings) -> Result<(), String> {
        let items = state.mangohud_items();
        let mut heading = self.ui.catalog.format(
            "mangohud.items_heading",
            &[
                ("count", &items.len().to_string()),
                ("total", &ITEMS.len().to_string()),
            ],
        );
        if state.value(Id::MangohudLayout) != "custom" {
            heading = format!("{heading} · {}", self.tr("mangohud.items_custom"));
        }
        self.text(
            (28., 50., 584., 20.),
            &heading.to_uppercase(),
            (11., self.theme.dim_text, Align::Left),
        )?;
        let radius = self.ui.px(10.) as f32;
        for (n, (code, label)) in ITEMS.iter().enumerate() {
            let cell = (
                16. + (n % 2) as f32 * 305.,
                74. + (n / 2) as f32 * 46.,
                299.,
                40.,
            );
            let selected = n == state.cursor;
            let back = selection(selected, self.theme.surface, self.theme.highlight_bg);
            self.ui.rounded(self.rect(cell), radius, back, None)?;
            self.text_face(
                (cell.0 + 12., cell.1, cell.2 - 54., cell.3),
                &self.tr(label),
                (14., self.ink(selected), Align::Left),
                selection(selected, Face::Regular, Face::Semibold),
            )?;
            let check = (cell.0 + cell.2 - 30., cell.1 + 11., 18., 18.);
            let corner = self.ui.px(5.) as f32;
            if items.contains(code) {
                let accent = self.theme.highlight_bg;
                self.ui
                    .rounded(self.rect(check), corner, accent, Some(accent))?;
                self.ui.icon(
                    Glyph::Check,
                    self.rect((check.0 + 3., check.1 + 3., 12., 12.)),
                    self.theme.highlight_text,
                )?;
            } else {
                let edge = selection(
                    selected,
                    self.theme.dim_text,
                    self.theme.accent_text_secondary,
                );
                self.ui
                    .rounded(self.rect(check), corner, back, Some(edge))?;
            }
        }
        Ok(())
    }
}
