//! System › Test the controller (Sistema-testar-controle) and the stick
//! calibration's four steps (Calibrar-centro, -curso, -zona-morta, -resumo).
use super::*;
use crate::pad::Pad;
use crate::primitives::Sketch;
use crate::settings::joypad::{position, Axes, Calibration, Joypad, Step, BINS, CHOICES, REST};

type Point = (f32, f32);
/// The calibration's cards and their circle (Calibrar-centro, -curso).
const CARD_W: f32 = 270.;
const CARD_GAP: f32 = 16.;

/// "+18", "−96", "0": a reading with its sign, as the mockups write it.
fn signed(value: i32) -> String {
    match value {
        0 => "0".into(),
        v if v > 0 => format!("+{v}"),
        v => format!("−{}", v.unsigned_abs()),
    }
}
/// SDL's ±32767 in the events' own range, when the driver said it.
fn event(value: i16, max: Option<i32>) -> i32 {
    max.map_or(i32::from(value), |max| {
        (f64::from(value) * f64::from(max) / 32767.).round() as i32
    })
}

impl View<'_, '_, '_> {
    fn point(&self, (x, y): Point) -> Point {
        let scale = self.ui.frame.scale;
        (x * scale, y * scale)
    }
    fn circle(
        &mut self,
        (x, y): Point,
        r: f32,
        fill: Rgba,
        border: Option<Rgba>,
    ) -> Result<(), String> {
        let rect = self.rect((x - r, y - r, 2. * r, 2. * r));
        let radius = rect.width() as f32 / 2.;
        self.ui.rounded(rect, radius, fill, border)
    }
    fn round_box(
        &mut self,
        bounds: Bounds,
        radius: f32,
        fill: Rgba,
        border: Option<Rgba>,
    ) -> Result<(), String> {
        let radius = self.ui.px(radius) as f32;
        self.ui.rounded(self.rect(bounds), radius, fill, border)
    }
    /// A 1 px dashed circle: the deadzone.
    fn dashed_circle(&mut self, center: Point, r: f32, c: Rgba) -> Result<(), String> {
        let count = ((std::f32::consts::TAU * r / 5.).round() as usize).max(8);
        let mut sketch = Sketch::new();
        for n in 0..count {
            let angle = |t: f32| (n as f32 + t) * std::f32::consts::TAU / count as f32;
            let at = |a: f32| self.point((center.0 + r * a.cos(), center.1 + r * a.sin()));
            sketch.line(at(angle(0.)), at(angle(0.55)), self.ui.frame.scale, c);
        }
        sketch.draw(&mut self.ui.canvas)
    }
    /// The edges of untested keys and empty tracks: between the border and
    /// the dim text.
    fn rim(&self) -> Rgba {
        crate::appearance::mix(self.theme.border, self.theme.dim_text, 0.22)
    }
    /// A stick's circle and its cross.
    fn stick_frame(&mut self, center: Point, r: f32, rim: Rgba) -> Result<(), String> {
        self.circle(center, r, self.theme.background, Some(rim))?;
        self.cross(center, r)
    }
    fn cross(&mut self, center: Point, r: f32) -> Result<(), String> {
        let mut sketch = Sketch::new();
        let one = self.ui.frame.scale;
        let (x, y) = center;
        let border = self.theme.border;
        sketch.line(self.point((x - r, y)), self.point((x + r, y)), one, border);
        sketch.line(self.point((x, y - r)), self.point((x, y + r)), one, border);
        sketch.draw(&mut self.ui.canvas)
    }
    /// Title and paragraph at the top of a step; the paragraph's bottom.
    fn step_heading(
        &mut self,
        (x, w): (f32, f32),
        title: &str,
        text: &str,
        size: f32,
    ) -> Result<f32, String> {
        let title_size = if size > 13. { 22. } else { 20. };
        self.text_face(
            (x, 50., w, 30.),
            &self.tr(title),
            (title_size, self.theme.text, Align::Left),
            Face::Semibold,
        )?;
        self.top_paragraph(
            (x, 84., w),
            &self.tr(text),
            size,
            if size > 13. { 2 } else { 5 },
        )
    }
    /// Wrapped text from the top of `(x, y, w)` in at most `lines` lines;
    /// returns its bottom.
    fn top_paragraph(
        &mut self,
        (x, y, w): (f32, f32, f32),
        text: &str,
        size: f32,
        lines: usize,
    ) -> Result<f32, String> {
        let pitch = self.pitch(size, size * 1.45);
        let bounds = (x, y, w, pitch * lines as f32);
        let (fitted, size, face) = self.fit(bounds, text, size, Face::Regular)?;
        let pitch = self.pitch(size, size * 1.45);
        for (n, line) in fitted.iter().enumerate() {
            self.ui.label_aligned(
                line,
                (face, size, self.theme.muted),
                self.rect((x, y + n as f32 * pitch, w, pitch)),
                Align::Left,
            )?;
        }
        Ok(y + fitted.len() as f32 * pitch)
    }

    /// The step bars on the right of the header, left of the status;
    /// returns where the breadcrumbs must stop.
    pub(super) fn pad_steps(&mut self, state: &Settings, right: f32) -> Result<f32, String> {
        let cal = state.joypad.calibration.as_ref();
        let Some(cal) = cal.filter(|_| state.page() == Page::PadCalibrate) else {
            return Ok(right);
        };
        let current = cal.step.number();
        let label = self
            .ui
            .catalog
            .format("pad.step", &[("n", &current.to_string())]);
        let label_w = self.span(&label, Face::Regular, 12.)?;
        let x = right - 12. - label_w - 6. - 4. * 22. - 3. * 6.;
        for n in 1..=4 {
            let fill = match n {
                n if n < current => self.theme.success,
                n if n == current => self.theme.highlight_bg,
                _ => self.rim(),
            };
            self.round_box((x + (n - 1) as f32 * 28., 19., 22., 6.), 3., fill, None)?;
        }
        self.text(
            (right - 12. - label_w, 0., label_w, 44.),
            &label,
            (12., self.theme.muted, Align::Left),
        )?;
        Ok(x - 8.)
    }

    pub(super) fn pad_test(&mut self, state: &Settings) -> Result<(), String> {
        let pad = &state.joypad;
        self.pad_drawing(pad, (20., 48.))?;
        self.pad_legend(356.)?;
        self.pad_facts(pad, (408., 212.))
    }
    /// The controller (372 × 300 at `origin`): pressed keys in the accent,
    /// tested ones ringed in green.
    fn pad_drawing(&mut self, pad: &Joypad, (ox, oy): Point) -> Result<(), String> {
        let theme = self.theme;
        let rim = self.rim();
        let at = |x: f32, y: f32| (ox + x, oy + y);
        let state = |p: Pad| {
            if pad.held(p) {
                (theme.highlight_bg, None, theme.highlight_text)
            } else if pad.tested(p) {
                (theme.badge, Some(theme.success), theme.muted)
            } else {
                (theme.badge, Some(rim), theme.muted)
            }
        };
        // Shoulders and triggers.
        for (p, x, w) in [
            (Pad::L1, 20., 72.),
            (Pad::L2, 100., 56.),
            (Pad::R2, 216., 56.),
            (Pad::R1, 280., 72.),
        ] {
            let (fill, border, ink) = state(p);
            let (x, y) = at(x, 10.);
            self.round_box((x, y, w, 20.), 8., fill, border)?;
            self.text((x, y, w, 20.), p.name(), (11., ink, Align::Center))?;
        }
        let (x, y) = at(10., 40.);
        self.round_box((x, y, 352., 250.), 34., theme.surface, Some(theme.border))?;
        let (x, y) = at(118., 56.);
        self.round_box((x, y, 136., 96.), 6., theme.background, Some(theme.border))?;
        self.text(
            (x, y, 136., 96.),
            &self.tr("pad.screen"),
            (11., theme.dim_text, Align::Center),
        )?;
        // The D-pad: the cross ringed once all four are tested.
        let arms = [Pad::Up, Pad::Down, Pad::Left, Pad::Right];
        let cross = if arms.iter().all(|p| pad.tested(*p)) {
            theme.success
        } else {
            rim
        };
        for (x, y, w, h) in [(56., 82., 20., 60.), (36., 102., 60., 20.)] {
            let (x, y) = at(x, y);
            self.round_box((x, y, w, h), 4., theme.badge, Some(cross))?;
        }
        let mut arrows = Sketch::new();
        for (p, square, tip, side) in [
            (Pad::Up, (56., 82.), (66., 86.), [(61., 93.), (71., 93.)]),
            (
                Pad::Down,
                (56., 122.),
                (66., 138.),
                [(61., 131.), (71., 131.)],
            ),
            (
                Pad::Left,
                (36., 102.),
                (40., 112.),
                [(47., 107.), (47., 117.)],
            ),
            (
                Pad::Right,
                (76., 102.),
                (92., 112.),
                [(85., 107.), (85., 117.)],
            ),
        ] {
            let ink = if pad.held(p) {
                let (x, y) = at(square.0, square.1);
                self.round_box((x, y, 20., 20.), 4., theme.highlight_bg, None)?;
                theme.highlight_text
            } else if pad.tested(p) {
                theme.success
            } else {
                theme.dim_text
            };
            let points = [tip, side[0], side[1]].map(|(x, y)| self.point(at(x, y)));
            let middle = (
                (points[0].0 + points[1].0 + points[2].0) / 3.,
                (points[0].1 + points[1].1 + points[2].1) / 3.,
            );
            arrows.polygon(middle, &points, ink);
        }
        arrows.draw(&mut self.ui.canvas)?;
        for (p, x, y) in [
            (Pad::X, 306., 88.),
            (Pad::Y, 280., 112.),
            (Pad::A, 332., 112.),
            (Pad::B, 306., 136.),
        ] {
            let (fill, border, ink) = state(p);
            let center = at(x, y);
            self.circle(center, 14., fill, border)?;
            self.text_face(
                (center.0 - 14., center.1 - 14., 28., 28.),
                p.name(),
                (
                    12.,
                    if pad.held(p) { ink } else { theme.muted },
                    Align::Center,
                ),
                Face::Semibold,
            )?;
        }
        let mut small = vec![(Pad::Select, 140.), (Pad::Start, 202.)];
        if pad.has_fn() {
            small.push((Pad::Fn, 171.));
        }
        for (p, x) in small {
            let (fill, border, _) = state(p);
            let y = if p == Pad::Fn { 214. } else { 166. };
            let (x, y) = at(x, y);
            self.round_box((x, y, 30., 12.), 6., fill, border)?;
            self.text(
                (x - 15., y + 14., 60., 16.),
                p.name(),
                (10., theme.dim_text, Align::Center),
            )?;
        }
        let deadzone = |n: usize| {
            pad.status.flatten().map_or(0.1, |axes: Axes| {
                (axes[n].deadzone as f32 / axes[n].max as f32).clamp(0.15, 0.5)
            })
        };
        for (stick, x, label, click) in [
            (0, 98., "pad.left", Pad::L3),
            (1, 274., "pad.right", Pad::R3),
        ] {
            let center = at(x, 226.);
            let ring = if pad.held(click) {
                theme.highlight_bg
            } else if pad.tested(click) {
                theme.success
            } else {
                rim
            };
            self.stick_frame(center, 40., ring)?;
            self.dashed_circle(center, 40. * deadzone(2 * stick), theme.dim_text)?;
            let (dx, dy) = (pad.sticks[2 * stick], pad.sticks[2 * stick + 1]);
            // Past a tenth of the range: the rest's own wobble stays grey.
            let moved = f32::from(dx).hypot(f32::from(dy)) > 3277.;
            let dot = (
                center.0 + 31. * f32::from(dx) / 32767.,
                center.1 + 31. * f32::from(dy) / 32767.,
            );
            let ink = if moved || pad.held(click) {
                theme.highlight_bg
            } else {
                theme.dim_text
            };
            self.circle(dot, 9., ink, None)?;
            let text = format!("{} · {}", self.tr(label).to_uppercase(), click.name());
            self.text(
                (center.0 - 60., center.1 + 48., 120., 16.),
                &text,
                (10., theme.dim_text, Align::Center),
            )?;
        }
        Ok(())
    }
    fn pad_legend(&mut self, y: f32) -> Result<(), String> {
        let theme = self.theme;
        let mut x = 20.;
        for (n, key) in ["pad.pressed", "pad.tested_legend", "pad.deadzone"]
            .into_iter()
            .enumerate()
        {
            let center = (x + 5., y + 8.);
            match n {
                0 => self.circle(center, 5., theme.highlight_bg, None)?,
                1 => {
                    self.circle(center, 5., theme.success, None)?;
                    self.circle(center, 3., theme.background, None)?;
                }
                _ => self.dashed_circle(center, 4.5, theme.dim_text)?,
            }
            let text = self.tr(key);
            let w = self.span(&text, Face::Regular, 12.)?;
            self.text(
                (x + 16., y, w + 2., 16.),
                &text,
                (12., theme.muted, Align::Left),
            )?;
            x += 16. + w + 14.;
        }
        Ok(())
    }
    /// The column on the right: the pad, the sticks' readings and what is
    /// left to test.
    fn pad_facts(&mut self, pad: &Joypad, (x, w): (f32, f32)) -> Result<(), String> {
        let theme = self.theme;
        let mut y = 48.;
        let heading = |view: &mut Self, y: &mut f32, key: &str| -> Result<(), String> {
            let text = view.tr(key).to_uppercase();
            view.text(
                (x + 4., *y + 8., w - 8., 14.),
                &text,
                (11., theme.dim_text, Align::Left),
            )?;
            *y += 26.;
            Ok(())
        };
        heading(self, &mut y, "pad.controller")?;
        let controls = pad.controls();
        let name = pad.device.as_ref().map_or("", |d| d.0.as_str()).to_owned();
        let counts = self.ui.catalog.format(
            "pad.counts",
            &[("buttons", &controls.len().to_string()), ("axes", "4")],
        );
        self.round_box((x, y, w, 52.), 10., theme.surface, Some(theme.border))?;
        self.text(
            (x + 12., y + 7., w - 24., 20.),
            &name,
            (14., theme.text, Align::Left),
        )?;
        self.text(
            (x + 12., y + 27., w - 24., 17.),
            &counts,
            (12., theme.muted, Align::Left),
        )?;
        y += 58.;
        heading(self, &mut y, "pad.sticks")?;
        self.round_box((x, y, w, 56.), 10., theme.surface, Some(theme.border))?;
        let max = |n: usize| pad.status.flatten().map(|axes| axes[n].max);
        for (row, key) in ["pad.left_short", "pad.right_short"]
            .into_iter()
            .enumerate()
        {
            let top = y + 8. + row as f32 * 21.;
            self.text(
                (x + 12., top, 44., 19.),
                &self.tr(key),
                (13., theme.muted, Align::Left),
            )?;
            for (column, letter) in ["X", "Y"].into_iter().enumerate() {
                let n = 2 * row + column;
                let value = format!("{letter} {}", signed(event(pad.sticks[n], max(n))));
                self.text_face(
                    (x + 60. + column as f32 * 72., top, 70., 19.),
                    &value,
                    (13., theme.text, Align::Left),
                    Face::Mono,
                )?;
            }
        }
        y += 62.;
        heading(self, &mut y, "pad.tested")?;
        let done = controls.iter().filter(|p| pad.tested(**p)).count();
        let count = self.ui.catalog.format(
            "pad.tested_count",
            &[
                ("done", &done.to_string()),
                ("total", &controls.len().to_string()),
            ],
        );
        let missing: Vec<&str> = controls
            .iter()
            .filter(|p| !pad.tested(**p))
            .map(|p| p.name())
            .collect();
        let rest = if missing.is_empty() {
            self.tr("pad.all_tested")
        } else {
            self.ui
                .catalog
                .format("pad.missing", &[("list", &missing.join(", "))])
        };
        let pitch = self.pitch(13., 19.);
        let lines = self
            .lines(&rest, 13., self.ui.px(w - 24.) as u32)?
            .len()
            .min(5);
        let h = 16. + pitch * (1 + lines) as f32;
        self.round_box((x, y, w, h), 10., theme.surface, Some(theme.border))?;
        self.text_face(
            (x + 12., y + 8., w - 24., pitch),
            &count,
            (13., theme.success, Align::Left),
            Face::Semibold,
        )?;
        self.text(
            (x + 12., y + 8. + pitch, w - 24., pitch * lines as f32),
            &rest,
            (13., theme.muted, Align::Left),
        )
    }

    pub(super) fn pad_calibrate(&mut self, state: &Settings) -> Result<(), String> {
        let Some(cal) = &state.joypad.calibration else {
            return Ok(());
        };
        match cal.step {
            Step::Center => self.pad_center(cal),
            Step::Travel => self.pad_travel(cal),
            Step::Deadzone => self.pad_deadzone(cal, &state.joypad),
            Step::Summary => self.pad_summary(cal),
        }
    }
    /// The two cards side by side under the heading.
    fn cards(&mut self, top: f32, h: f32, ready: [bool; 2]) -> Result<[f32; 2], String> {
        let left = (640. - 2. * CARD_W - CARD_GAP) / 2.;
        let xs = [left, left + CARD_W + CARD_GAP];
        for (x, ready) in xs.into_iter().zip(ready) {
            let border = if ready {
                self.theme.success
            } else {
                self.theme.border
            };
            self.round_box((x, top, CARD_W, h), 12., self.theme.surface, Some(border))?;
        }
        Ok(xs)
    }
    fn pad_center(&mut self, cal: &Calibration) -> Result<(), String> {
        let theme = self.theme;
        let top = self.step_heading((24., 592.), "pad.center_title", "pad.center_text", 14.)? + 14.;
        let (svg, r) = (150., 68.);
        let xs = self.cards(top, 12. + svg + 8. + 18. + 6. + 16. + 12., [false; 2])?;
        let max = |n: usize| cal.now[n].max as f32;
        for (stick, x) in xs.into_iter().enumerate() {
            let center = (x + CARD_W / 2., top + 12. + svg / 2.);
            let off = |n: usize| {
                let raw = if cal.rest.measuring() {
                    cal.rest.mean(n)
                } else {
                    cal.now[n].raw
                };
                cal.before[n].shown(raw - cal.before[n].center)
            };
            let (ox, oy) = (off(2 * stick), off(2 * stick + 1));
            let dot = (
                center.0 + (ox as f32 / max(2 * stick)).clamp(-1., 1.) * (r - 6.),
                center.1 + (oy as f32 / max(2 * stick + 1)).clamp(-1., 1.) * (r - 6.),
            );
            self.circle(center, r, theme.background, Some(self.rim()))?;
            let wobble = cal
                .rest
                .wobble(2 * stick)
                .max(cal.rest.wobble(2 * stick + 1)) as f32;
            let halo = 11. + wobble / max(2 * stick) * r;
            self.circle(dot, halo + 3., faded(theme.highlight_bg, 0.35), None)?;
            self.circle(dot, halo - 3., theme.background, None)?;
            self.cross(center, r)?;
            self.circle(center, 4., theme.background, Some(theme.dim_text))?;
            self.circle(dot, 6., theme.highlight_bg, None)?;
            let below = top + 12. + svg + 8.;
            let name = self.tr(if stick == 0 { "pad.left" } else { "pad.right" });
            self.text_face(
                (x, below, CARD_W, 18.),
                &name,
                (13., theme.text, Align::Center),
                Face::Semibold,
            )?;
            let values = format!("X {} · Y {}", signed(ox), signed(oy));
            self.text_face(
                (x, below + 24., CARD_W, 16.),
                &values,
                (12., theme.muted, Align::Center),
                Face::Mono,
            )?;
        }
        let y = top + 12. + svg + 8. + 18. + 6. + 16. + 12. + 14.;
        let now = self.ui.frame.now;
        let done = cal.rest.done(now);
        let left = (REST.as_secs_f32() * (1. - done)).ceil() as u32;
        self.text(
            (24., y, 400., 18.),
            &self.tr("pad.measuring"),
            (13., theme.muted, Align::Left),
        )?;
        let seconds = self
            .ui
            .catalog
            .format("pad.seconds", &[("count", &left.to_string())]);
        self.text_face(
            (416., y, 200., 18.),
            &seconds,
            (13., theme.muted, Align::Right),
            Face::Mono,
        )?;
        self.round_box((24., y + 24., 592., 8.), 4., theme.badge, None)?;
        if done > 0. {
            self.round_box(
                (24., y + 24., (592. * done).max(8.), 8.),
                4.,
                theme.highlight_bg,
                None,
            )?;
        }
        Ok(())
    }
    fn pad_travel(&mut self, cal: &Calibration) -> Result<(), String> {
        let theme = self.theme;
        let top = self.step_heading((24., 592.), "pad.travel_title", "pad.travel_text", 14.)? + 12.;
        let (svg, r) = (140., 63.);
        let h = 12. + svg + 6. + 18. + 6. + 34. + 12.;
        let ready = [0, 1].map(|stick| cal.travel.ready(stick, &cal.now, &cal.rest));
        let xs = self.cards(top, h, ready)?;
        for (stick, x) in xs.into_iter().enumerate() {
            let center = (x + CARD_W / 2., top + 12. + svg / 2.);
            self.stick_frame(center, r, self.rim())?;
            // The edge reached so far, closed and filled once it went all around.
            let reach = &cal.travel.reach[stick];
            let edge: Vec<Option<Point>> = (0..BINS)
                .map(|bin| {
                    let angle = (bin as f32 + 0.5) * std::f32::consts::TAU / BINS as f32;
                    let d = reach[bin].min(1.04) * r;
                    (reach[bin] > 0.05).then(|| {
                        self.point((center.0 + d * angle.cos(), center.1 + d * angle.sin()))
                    })
                })
                .collect();
            let closed = edge.iter().all(Option::is_some);
            let stroke = if ready[stick] {
                theme.success
            } else {
                theme.highlight_bg
            };
            let mut sketch = Sketch::new();
            if closed {
                let points: Vec<Point> = edge.iter().flatten().copied().collect();
                sketch.polygon(self.point(center), &points, faded(theme.highlight_bg, 0.12));
            }
            for bin in 0..BINS {
                if let (Some(a), Some(b)) = (edge[bin], edge[(bin + 1) % BINS]) {
                    sketch.line(a, b, 2. * self.ui.frame.scale, stroke);
                }
            }
            sketch.draw(&mut self.ui.canvas)?;
            self.circle(center, 4., theme.background, Some(theme.dim_text))?;
            let (dx, dy) = position(&cal.now, &cal.rest, stick);
            let d = dx.hypot(dy).max(1.);
            let dot = (center.0 + dx / d * r, center.1 + dy / d * r);
            self.circle(dot, 6., theme.highlight_bg, None)?;
            let below = top + 12. + svg + 6.;
            let name = self.tr(if stick == 0 { "pad.left" } else { "pad.right" });
            let status = if ready[stick] {
                self.tr("pad.ready")
            } else {
                let done = (cal.travel.laps(stick).floor() as u32).min(2);
                self.ui
                    .catalog
                    .format("pad.laps", &[("done", &done.to_string())])
            };
            let name_w = self.span(&name, Face::Semibold, 13.)?;
            let status_w = self.span(&status, Face::Regular, 12.)?;
            let start = x + (CARD_W - name_w - 6. - status_w) / 2.;
            self.text_face(
                (start, below, name_w + 1., 18.),
                &name,
                (13., theme.text, Align::Left),
                Face::Semibold,
            )?;
            let ink = if ready[stick] {
                theme.success
            } else {
                theme.muted
            };
            self.text(
                (start + name_w + 6., below, status_w + 1., 18.),
                &status,
                (12., ink, Align::Left),
            )?;
            for (line, n) in [2 * stick, 2 * stick + 1].into_iter().enumerate() {
                let axis = &cal.now[n];
                let side = |positive| {
                    cal.travel
                        .side(n, &cal.rest, axis.max, positive)
                        .map(|t| axis.shown(if positive { t } else { -t }))
                };
                let mut ends = [side(false), side(true)];
                if axis.invert {
                    ends.reverse();
                }
                let [low, high] = ends.map(|end| end.map_or("?".into(), signed));
                let text = format!("{} {low} … {high}", if line == 0 { "X" } else { "Y" });
                let y = below + 24. + line as f32 * 17.;
                self.text_face(
                    (x, y, CARD_W, 17.),
                    &text,
                    (12., theme.muted, Align::Center),
                    Face::Mono,
                )?;
            }
        }
        let y = top + h + 12.;
        let text = self.tr("pad.travel_info");
        let lines = self
            .lines(&text, 13., self.ui.px(568.) as u32)?
            .len()
            .min(2);
        let pitch = self.pitch(13., 19.);
        self.round_box(
            (24., y, 592., 16. + pitch * lines as f32),
            10.,
            theme.surface,
            Some(theme.border),
        )?;
        self.text(
            (36., y + 8., 568., pitch * lines as f32),
            &text,
            (13., theme.muted, Align::Left),
        )
    }
    fn pad_deadzone(&mut self, cal: &Calibration, pad: &Joypad) -> Result<(), String> {
        let theme = self.theme;
        let stick = cal.stick;
        let (center, r) = ((134., 152.), 92.);
        self.stick_frame(center, r, self.rim())?;
        let dz = f32::from(cal.deadzone[stick]) / 100.;
        self.circle(center, (r * dz).max(2.), faded(theme.dim_text, 0.18), None)?;
        self.dashed_circle(center, (r * dz).max(2.), theme.dim_text)?;
        // The stick around its rest, 1 = its shorter side's travel.
        let span = cal.span(stick) as f32;
        let at = |n: usize| {
            let axis = &cal.now[n];
            (axis.shown(axis.raw - cal.rest.mean(n)) as f32 / span).clamp(-1., 1.)
        };
        let dot = (
            center.0 + at(2 * stick) * (r - 7.),
            center.1 + at(2 * stick + 1) * (r - 7.),
        );
        self.circle(dot, 7., theme.highlight_bg, None)?;
        // Which stick: L1 and R1.
        let names = ["pad.left", "pad.right"].map(|key| self.tr(key));
        let widths = [
            self.span(&names[0], Face::Semibold, 13.)? + 24.,
            self.span(&names[1], Face::Semibold, 13.)? + 24.,
        ];
        let mut x = center.0 - (widths[0] + widths[1] + 6.) / 2.;
        for (n, (name, w)) in names.iter().zip(widths).enumerate() {
            let chosen = n == stick;
            let (fill, ink) = if chosen {
                (theme.highlight_bg, theme.highlight_text)
            } else {
                (theme.badge, theme.muted)
            };
            self.round_box((x, 256., w, 26.), 13., fill, None)?;
            let face = if chosen {
                Face::Semibold
            } else {
                Face::Regular
            };
            self.text_face((x, 256., w, 26.), name, (13., ink, Align::Center), face)?;
            x += w + 6.;
        }
        self.text(
            (24., 288., 220., 18.),
            &self.tr("pad.switch"),
            (12., theme.muted, Align::Center),
        )?;
        // The setting on the right.
        let (x, w) = (262., 354.);
        let mut y = self.step_heading((x, w), "pad.deadzone", "pad.deadzone_text", 13.)? + 10.;
        let suggested = self.ui.catalog.format(
            "pad.suggested",
            &[
                ("value", &cal.suggested[stick].to_string()),
                ("wobble", &cal.wobble(stick).to_string()),
            ],
        );
        let lines = self
            .lines(&suggested, 12., self.ui.px(w - 24.) as u32)?
            .len()
            .min(3);
        let pitch = self.pitch(12., 16.);
        let h = 10. + 20. + 8. + 6. + 8. + pitch * lines as f32 + 10.;
        let fill = crate::appearance::mix(theme.surface, theme.highlight_bg, 0.1);
        self.round_box((x, y, w, h), 10., fill, Some(theme.highlight_bg))?;
        self.text_face(
            (x + 12., y + 10., w - 120., 20.),
            &self.tr("pad.center_row"),
            (14., theme.text, Align::Left),
            Face::Semibold,
        )?;
        let value = format!("‹ {}% ›", cal.deadzone[stick]);
        self.text_face(
            (x + w - 112., y + 10., 100., 20.),
            &value,
            (14., theme.text, Align::Right),
            Face::Mono,
        )?;
        let track = (x + 12., y + 38., w - 24., 6.);
        self.round_box(track, 3., self.rim(), None)?;
        let part =
            f32::from(cal.deadzone[stick]) / f32::from(crate::settings::joypad::DEADZONE_MAX);
        if part > 0. {
            self.round_box(
                (track.0, track.1, (track.2 * part).max(6.), 6.),
                3.,
                theme.highlight_bg,
                None,
            )?;
        }
        self.text(
            (x + 12., y + 52., w - 24., pitch * lines as f32),
            &suggested,
            (12., theme.muted, Align::Left),
        )?;
        y += h + 10.;
        let percent = |n: usize| signed((f32::from(pad.sticks[n]) * 100. / 32767.).round() as i32);
        let output = self.ui.catalog.format(
            "pad.output",
            &[("x", &percent(2 * stick)), ("y", &percent(2 * stick + 1))],
        );
        self.text_face(
            (x, y, w, 18.),
            &output,
            (12., theme.muted, Align::Left),
            Face::Mono,
        )
    }
    fn pad_summary(&mut self, cal: &Calibration) -> Result<(), String> {
        let theme = self.theme;
        let mut y =
            self.step_heading((24., 592.), "pad.summary_title", "pad.summary_text", 14.)? + 12.;
        let letter = |n: usize| ["X", "Y"][n % 2];
        let center = |stick: usize| {
            let (n, off) = cal.drift(stick);
            let mut before = format!("{} {}", letter(n), signed(off));
            if cal.drifts(stick) {
                before = format!("{before} {}", self.tr("pad.drifts"));
            }
            (before, format!("{} 0", letter(n)))
        };
        let travel = match cal.short_side() {
            None => "100%".to_owned(),
            Some((n, positive, reach)) => {
                let stick = self.tr(if n < 2 {
                    "pad.left_lower"
                } else {
                    "pad.right_lower"
                });
                let arrow = match (n % 2, positive) {
                    (0, false) => "←",
                    (0, true) => "→",
                    (_, false) => "↑",
                    (_, true) => "↓",
                };
                format!("{stick} {arrow} {reach}%")
            }
        };
        let zones = |values: [u8; 2]| {
            self.ui.catalog.format(
                "pad.deadzones",
                &[
                    ("left", &values[0].to_string()),
                    ("right", &values[1].to_string()),
                ],
            )
        };
        let rows = [
            ("pad.left_center", center(0), theme.success),
            ("pad.right_center", center(1), theme.success),
            (
                "pad.travel_row",
                (travel, self.tr("pad.travel_full")),
                theme.success,
            ),
            (
                "pad.deadzone",
                (
                    zones([cal.deadzone_before(0), cal.deadzone_before(1)]),
                    zones(cal.deadzone),
                ),
                theme.text,
            ),
        ];
        let row_h = 31.;
        let column = (592. - 24. - 110.) / 2.;
        self.round_box(
            (24., y, 592., 27. + rows.len() as f32 * row_h),
            12.,
            theme.surface,
            Some(theme.border),
        )?;
        for (n, key) in ["pad.before", "pad.after"].into_iter().enumerate() {
            let text = self.tr(key).to_uppercase();
            self.text(
                (36. + 110. + n as f32 * column, y, column, 27.),
                &text,
                (11., theme.dim_text, Align::Left),
            )?;
        }
        y += 27.;
        for (key, (before, after), ink) in rows {
            self.ui.divider(
                self.ui.px(24.),
                self.ui.px(y - 1.),
                self.ui.px(592.) as u32,
                theme.border,
            )?;
            self.text(
                (36., y, 106., row_h),
                &self.tr(key),
                (13., theme.muted, Align::Left),
            )?;
            self.text_face(
                (36. + 110., y, column - 8., row_h),
                &before,
                (13., theme.muted, Align::Left),
                Face::Mono,
            )?;
            self.text_face(
                (36. + 110. + column, y, column - 8., row_h),
                &after,
                (13., ink, Align::Left),
                Face::Mono,
            )?;
            y += row_h;
        }
        y += 12.;
        for (n, key) in CHOICES.into_iter().enumerate() {
            let chosen = n == cal.cursor;
            let (fill, border, ink, face, size) = if chosen {
                (
                    theme.highlight_bg,
                    None,
                    theme.highlight_text,
                    Face::Semibold,
                    15.,
                )
            } else {
                (
                    theme.surface,
                    Some(theme.border),
                    theme.text,
                    Face::Regular,
                    14.,
                )
            };
            self.round_box((24., y, 592., 38.), 10., fill, border)?;
            self.text_face(
                (38., y, 564., 38.),
                &self.tr(key),
                (size, ink, Align::Left),
                face,
            )?;
            y += 44.;
        }
        Ok(())
    }
}
