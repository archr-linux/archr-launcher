//! System › Test the controller (Sistema-testar-controle) and the stick
//! calibration behind it (Calibrar-centro, -curso, -zona-morta, -resumo).
//! The joypad driver's sysfs `calibration` file and `archr-joypad-cal` do the
//! work; this holds what the pages show and the order of the steps. The host
//! reads the file and runs the tool off SDL's thread (`Command`).
//!
//! Leaving without saving puts every axis back with `archr-joypad-cal set`
//! and the values read when the calibration opened, never with `reset`:
//! `reset` also deletes the saved calibration, which may be what was in use.
use super::{Page, Settings};
use crate::pad::Pad;
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

/// The driver's axes: the left stick's x and y, then the right stick's.
pub const AXES: [&str; 4] = ["x", "y", "rx", "ry"];
/// Start + Select held this long leave the tester; Y held this long opens
/// the calibration. A short press of any control only tests it.
pub const HOLD: Duration = Duration::from_secs(1);
/// The rest is measured for this long without a move.
pub const REST: Duration = Duration::from_secs(3);
/// Turns along the edge each stick makes in the travel step.
pub const LAPS: f32 = 3.;
/// Directions of the travel trace around each stick.
pub const BINS: usize = 48;
/// The deadzone row's limit (the tool takes up to 49).
pub const DEADZONE_MAX: u8 = 30;
/// How often the host reads the driver's file while a step measures.
pub const SAMPLE_EVERY: Duration = Duration::from_millis(33);

/// One axis as the driver reports it: the ADC reading now (`raw`, never
/// changed by the calibration) and the calibration applied to the events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Axis {
    pub raw: i32,
    pub center: i32,
    /// The center was set, not measured by the driver at boot.
    pub fixed: bool,
    /// Gain of each side, in percent.
    pub neg: i32,
    pub pos: i32,
    pub deadzone: i32,
    /// The events' range, ± this.
    pub max: i32,
    /// The events leave with the sign swapped.
    pub invert: bool,
}
pub type Axes = [Axis; 4];

/// `<axis> raw= center= fixed= neg= pos= deadzone= max= invert=`, one line
/// per axis; None unless the four axes are there with a range.
pub fn parse(text: &str) -> Option<Axes> {
    let mut out = [None; 4];
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let Some(n) = words.next().and_then(|w| AXES.iter().position(|a| *a == w)) else {
            continue;
        };
        let mut axis = Axis::default();
        for (key, value) in words.filter_map(|w| w.split_once('=')) {
            let Ok(value) = value.parse::<i32>() else {
                continue;
            };
            match key {
                "raw" => axis.raw = value,
                "center" => axis.center = value,
                "fixed" => axis.fixed = value == 1,
                "neg" => axis.neg = value,
                "pos" => axis.pos = value,
                "deadzone" => axis.deadzone = value,
                "max" => axis.max = value,
                "invert" => axis.invert = value == 1,
                _ => {}
            }
        }
        out[n] = Some(axis).filter(|a| a.max > 0);
    }
    let [x, y, rx, ry] = out;
    Some([x?, y?, rx?, ry?])
}
impl Axis {
    /// `archr-joypad-cal set` arguments that put this axis back as it was.
    pub fn restore(&self, name: &str) -> Vec<String> {
        let center = if self.fixed {
            self.center.to_string()
        } else {
            "auto".into()
        };
        vec![
            "set".into(),
            name.into(),
            format!("center={center}"),
            format!("neg={}", self.neg),
            format!("pos={}", self.pos),
            format!("deadzone={}", self.deadzone),
        ]
    }
    /// A raw distance from the center as the games see its direction.
    pub fn shown(&self, offset: i32) -> i32 {
        if self.invert {
            -offset
        } else {
            offset
        }
    }
}

/// What the host does for these pages, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// `archr-joypad-cal path` and `status`: whether calibration is offered,
    /// and the axes now (`Settings::joypad_opened`).
    Open,
    /// Read the driver's file every `SAMPLE_EVERY`, or stop.
    Sample(bool),
    /// `archr-joypad-cal fit AXIS CENTER MIN MAX DEADZONE%`, each axis.
    Fit([Fit; 4]),
    Save,
    Reset,
    /// `archr-joypad-cal set` each axis back to these.
    Restore(Box<Axes>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fit {
    pub center: i32,
    pub min: i32,
    pub max: i32,
    pub deadzone: u8,
}
impl Fit {
    pub fn args(&self, axis: &str) -> Vec<String> {
        vec![
            "fit".into(),
            axis.into(),
            self.center.to_string(),
            self.min.to_string(),
            self.max.to_string(),
            self.deadzone.to_string(),
        ]
    }
}

/// A control's press or release, or a stick axis as the games get it
/// (SDL's ±32767): the tester takes these instead of the launcher's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Button(Pad, bool),
    /// Left x, left y, right x, right y.
    Stick(usize, i16),
}

/// A move this far from the rest so far (5 % of the range) starts the
/// count again.
fn moved(max: i32) -> i32 {
    (max / 20).max(8)
}

/// The center step: readings while nothing moves.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rest {
    since: Option<Instant>,
    sum: [i64; 4],
    count: i64,
    low: [i32; 4],
    high: [i32; 4],
}
impl Rest {
    /// One reading; a move starts over. True once `REST` passed still.
    pub fn add(&mut self, axes: &Axes, now: Instant) -> bool {
        if self.count > 0 && (0..4).any(|n| (axes[n].raw - self.mean(n)).abs() > moved(axes[n].max))
        {
            *self = Self::default();
        }
        let since = *self.since.get_or_insert(now);
        for (n, axis) in axes.iter().enumerate() {
            if self.count == 0 {
                (self.low[n], self.high[n]) = (axis.raw, axis.raw);
            }
            self.sum[n] += i64::from(axis.raw);
            self.low[n] = self.low[n].min(axis.raw);
            self.high[n] = self.high[n].max(axis.raw);
        }
        self.count += 1;
        now.saturating_duration_since(since) >= REST
    }
    pub fn measuring(&self) -> bool {
        self.count > 0
    }
    /// The average reading of axis `n`.
    pub fn mean(&self, n: usize) -> i32 {
        if self.count == 0 {
            return 0;
        }
        (self.sum[n] as f64 / self.count as f64).round() as i32
    }
    /// The farthest a reading of axis `n` strayed from the average.
    pub fn wobble(&self, n: usize) -> i32 {
        let mean = self.mean(n);
        (self.high[n] - mean).max(mean - self.low[n]).max(0)
    }
    /// How much of `REST` has passed still, 0 to 1.
    pub fn done(&self, now: Instant) -> f32 {
        self.since.map_or(0., |since| {
            (now.saturating_duration_since(since).as_secs_f32() / REST.as_secs_f32()).min(1.)
        })
    }
}

/// The travel step: the extremes of each axis and the edge each stick drew.
#[derive(Clone, Debug, PartialEq)]
pub struct Travel {
    pub low: [i32; 4],
    pub high: [i32; 4],
    /// The farthest each stick went in each direction, 1 = the range.
    pub reach: [[f32; BINS]; 2],
    turned: [f32; 2],
    last: [Option<f32>; 2],
}
impl Default for Travel {
    fn default() -> Self {
        Self {
            low: [i32::MAX; 4],
            high: [i32::MIN; 4],
            reach: [[0.; BINS]; 2],
            turned: [0.; 2],
            last: [None; 2],
        }
    }
}
impl Travel {
    pub fn add(&mut self, axes: &Axes, rest: &Rest) {
        for (n, axis) in axes.iter().enumerate() {
            self.low[n] = self.low[n].min(axis.raw);
            self.high[n] = self.high[n].max(axis.raw);
        }
        for stick in 0..2 {
            let (x, y) = position(axes, rest, stick);
            let r = x.hypot(y);
            let angle = y.atan2(x);
            let bin = (((angle + TAU) % TAU) / TAU * BINS as f32) as usize % BINS;
            self.reach[stick][bin] = self.reach[stick][bin].max(r.min(1.2));
            // Turns count only along the edge.
            if r > 0.6 {
                if let Some(last) = self.last[stick] {
                    let step = (angle - last + TAU / 2.).rem_euclid(TAU) - TAU / 2.;
                    self.turned[stick] += step;
                }
                self.last[stick] = Some(angle);
            } else {
                self.last[stick] = None;
            }
        }
    }
    pub fn laps(&self, stick: usize) -> f32 {
        self.turned[stick].abs() / TAU
    }
    /// A side of axis `n` reached far enough to be measured: a quarter of
    /// the range. The tool needs MIN < CENTER < MAX.
    pub fn side(&self, n: usize, rest: &Rest, max: i32, positive: bool) -> Option<i32> {
        let center = rest.mean(n);
        let travel = if positive {
            self.high[n].saturating_sub(center)
        } else {
            center.saturating_sub(self.low[n])
        };
        (travel >= max / 4).then_some(travel)
    }
    pub fn ready(&self, stick: usize, axes: &Axes, rest: &Rest) -> bool {
        self.laps(stick) >= LAPS
            && [2 * stick, 2 * stick + 1].iter().all(|&n| {
                [false, true]
                    .iter()
                    .all(|&side| self.side(n, rest, axes[n].max, side).is_some())
            })
    }
    /// The shorter side of a stick's two axes: the deadzone's 100 %.
    fn span(&self, stick: usize, axes: &Axes, rest: &Rest) -> i32 {
        [2 * stick, 2 * stick + 1]
            .iter()
            .flat_map(|&n| [false, true].map(|side| self.side(n, rest, axes[n].max, side)))
            .map(|side| side.unwrap_or(1))
            .min()
            .unwrap_or(1)
            .max(1)
    }
}
/// A stick's reading around the measured rest, as the games see its
/// direction, 1 = the range (`max`).
pub fn position(axes: &Axes, rest: &Rest, stick: usize) -> (f32, f32) {
    let at = |n: usize| {
        let axis = &axes[n];
        axis.shown(axis.raw - rest.mean(n)) as f32 / axis.max as f32
    };
    (at(2 * stick), at(2 * stick + 1))
}
/// The deadzone the rest asks for: its wobble in percent of the travel,
/// rounded up, at least 2 %.
pub fn suggest(wobble: i32, span: i32) -> u8 {
    let percent = (100 * wobble.max(0).unsigned_abs()).div_ceil(span.max(1).unsigned_abs());
    percent.clamp(2, u32::from(DEADZONE_MAX)) as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Center,
    Travel,
    Deadzone,
    Summary,
}
impl Step {
    pub fn number(self) -> usize {
        self as usize + 1
    }
}
/// The summary's choices, in their order.
pub const CHOICES: [&str; 3] = ["pad.save", "pad.test_first", "pad.factory"];

#[derive(Clone, Debug, PartialEq)]
pub struct Calibration {
    pub step: Step,
    /// The driver when the calibration opened, put back when it is left
    /// without saving.
    pub before: Axes,
    /// The newest reading.
    pub now: Axes,
    pub rest: Rest,
    pub travel: Travel,
    /// Each stick's deadzone, in percent of its shorter side, and the one
    /// its rest asked for.
    pub deadzone: [u8; 2],
    pub suggested: [u8; 2],
    /// The stick the deadzone step shows.
    pub stick: usize,
    /// The summary's choice.
    pub cursor: usize,
    /// A `fit` went out: leaving without saving puts `before` back.
    pub changed: bool,
    /// Saving or going back to the console's defaults.
    pub busy: bool,
}
impl Calibration {
    fn new(before: Axes) -> Self {
        Self {
            step: Step::Center,
            before,
            now: before,
            rest: Rest::default(),
            travel: Travel::default(),
            deadzone: [2; 2],
            suggested: [2; 2],
            stick: 0,
            cursor: 0,
            changed: false,
            busy: false,
        }
    }
    pub fn span(&self, stick: usize) -> i32 {
        self.travel.span(stick, &self.now, &self.rest)
    }
    /// A stick's rest wobble, in percent of its travel.
    pub fn wobble(&self, stick: usize) -> u8 {
        let wobble = self
            .rest
            .wobble(2 * stick)
            .max(self.rest.wobble(2 * stick + 1));
        (100 * wobble.unsigned_abs())
            .div_ceil(self.span(stick).unsigned_abs())
            .min(100) as u8
    }
    fn fits(&self) -> [Fit; 4] {
        std::array::from_fn(|n| Fit {
            center: self.rest.mean(n),
            min: self.travel.low[n],
            max: self.travel.high[n],
            deadzone: self.deadzone[n / 2],
        })
    }
    /// A stick's deadzone before, in percent of the travel measured now.
    pub fn deadzone_before(&self, stick: usize) -> u8 {
        let dz = self.before[2 * stick].deadzone;
        ((100 * i64::from(dz.max(0)) + i64::from(self.span(stick)) / 2)
            / i64::from(self.span(stick))) as u8
    }
    /// How far the rest sits from the center used before, as the games see
    /// it, for the stick's axis that strays most: axis index and distance.
    pub fn drift(&self, stick: usize) -> (usize, i32) {
        let off = |n: usize| self.before[n].shown(self.rest.mean(n) - self.before[n].center);
        let (x, y) = (2 * stick, 2 * stick + 1);
        if off(y).abs() > off(x).abs() {
            (y, off(y))
        } else {
            (x, off(x))
        }
    }
    /// The rest is far enough from that center to move the character.
    pub fn drifts(&self, stick: usize) -> bool {
        let (n, off) = self.drift(stick);
        off.abs() > self.before[n].deadzone
    }
    /// The side that reached the least of the range with the calibration
    /// before: axis, direction (as the games see it) and percent; None
    /// when every side reached it.
    pub fn short_side(&self) -> Option<(usize, bool, u32)> {
        (0..4)
            .flat_map(|n| [false, true].map(|positive| (n, positive)))
            .filter_map(|(n, positive)| {
                let before = &self.before[n];
                let edge = if positive {
                    self.travel.high[n]
                } else {
                    self.travel.low[n]
                };
                let gain = if positive { before.pos } else { before.neg };
                let reach = i64::from((edge - before.center).abs()) * i64::from(gain)
                    / i64::from(before.max);
                let shown = before.shown(if positive { 1 } else { -1 }) > 0;
                (reach < 99).then_some((n, shown, reach.max(0) as u32))
            })
            .min_by_key(|side| side.2)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Joypad {
    /// The built-in pad's name and whether it has FN, once the host said.
    pub device: Option<(String, bool)>,
    /// The driver's axes when the tester opened; Some(None): this pad has
    /// no calibration. None while the host asks.
    pub status: Option<Option<Axes>>,
    /// Controls held now and those pressed since the tester opened, by
    /// `Pad` order.
    pub down: u32,
    pub seen: u32,
    /// The sticks as games get them: left x, y, right x, y.
    pub sticks: [i16; 4],
    /// Since when Start + Select, and Y, are held.
    exit_held: Option<Instant>,
    y_held: Option<Instant>,
    pub calibration: Option<Calibration>,
    /// Work for the host, taken by `Settings::joypad_commands`.
    commands: Vec<Command>,
}
fn bit(pad: Pad) -> u32 {
    1 << pad as u32
}
/// The controls the tester draws, in the order "missing" lists them.
pub fn controls(fn_key: bool) -> Vec<Pad> {
    use Pad::*;
    let mut pads = vec![A, B, X, Y, L1, R1, L2, R2, Select, Start, L3, R3];
    if fn_key {
        pads.push(Fn);
    }
    pads.extend([Up, Down, Left, Right]);
    pads
}
impl Joypad {
    pub fn held(&self, pad: Pad) -> bool {
        self.down & bit(pad) != 0
    }
    pub fn tested(&self, pad: Pad) -> bool {
        self.seen & bit(pad) != 0
    }
    pub fn has_fn(&self) -> bool {
        self.device.as_ref().is_some_and(|d| d.1)
    }
    pub fn controls(&self) -> Vec<Pad> {
        controls(self.has_fn())
    }
    /// The calibration can open from here: the pad has one, and this is
    /// not the tester opened from the calibration's summary.
    pub fn offers_calibration(&self) -> bool {
        matches!(self.status, Some(Some(_))) && self.calibration.is_none()
    }
    fn send(&mut self, command: Command) {
        self.commands.push(command);
    }
}

impl Settings {
    pub(super) fn joypad_page(&self) -> bool {
        matches!(self.page(), Page::PadTest | Page::PadCalibrate)
    }
    /// The tester or a calibration step is open: every control goes to
    /// `joypad_input`, none to the launcher's actions or shortcuts.
    pub fn joypad_raw(&self) -> bool {
        self.joypad_page()
    }
    /// The host fills in the built-in pad while the tester shows.
    pub fn joypad_wants_device(&self) -> bool {
        self.joypad_page() && self.joypad.device.is_none()
    }
    pub fn joypad_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.joypad.commands)
    }
    /// System › Test the controller.
    pub(super) fn open_joypad(&mut self) {
        self.joypad = Joypad::default();
        self.joypad.send(Command::Open);
        self.open(Page::PadTest);
    }
    /// `archr-joypad-cal` answered: the axes, or None without calibration.
    pub fn joypad_opened(&mut self, status: Option<Axes>) {
        self.joypad.status = Some(status);
    }
    pub fn joypad_input(&mut self, input: Input, now: Instant) {
        let pad = match input {
            Input::Stick(n, value) => {
                if let Some(stick) = self.joypad.sticks.get_mut(n) {
                    *stick = value;
                }
                return;
            }
            Input::Button(pad, false) => {
                self.joypad.down &= !bit(pad);
                if matches!(pad, Pad::Start | Pad::Select) {
                    self.joypad.exit_held = None;
                }
                if pad == Pad::Y {
                    self.joypad.y_held = None;
                }
                return;
            }
            Input::Button(pad, true) => pad,
        };
        // A press already held (a trigger's axis moving) is not a new one.
        let fresh = !self.joypad.held(pad);
        self.joypad.down |= bit(pad);
        if !fresh {
            return;
        }
        self.error = None;
        match self.page() {
            Page::PadTest => {
                self.joypad.seen |= bit(pad);
                if self.joypad.held(Pad::Start) && self.joypad.held(Pad::Select) {
                    self.joypad.exit_held = Some(now);
                }
                if pad == Pad::Y && self.joypad.offers_calibration() {
                    self.joypad.y_held = Some(now);
                }
            }
            Page::PadCalibrate => self.calibration_press(pad),
            _ => {}
        }
    }
    /// The holds of the tester; true when the page changed.
    pub fn joypad_tick(&mut self, now: Instant) -> bool {
        if self.page() != Page::PadTest {
            return false;
        }
        let long = |since: Option<Instant>| {
            since.is_some_and(|at| now.saturating_duration_since(at) >= HOLD)
        };
        if long(self.joypad.exit_held) {
            self.joypad.exit_held = None;
            self.joypad.down = 0;
            self.stack.pop();
            self.cursor = self.stack.last().expect("root page").1;
            return true;
        }
        if long(self.joypad.y_held) {
            self.joypad.y_held = None;
            if let Some(Some(before)) = self.joypad.status {
                self.joypad.calibration = Some(Calibration::new(before));
                self.joypad.send(Command::Sample(true));
                self.open(Page::PadCalibrate);
                return true;
            }
        }
        false
    }
    /// Pressed on a calibration step (physical controls: the sticks move
    /// the launcher's directions, so only the D-pad adjusts).
    fn calibration_press(&mut self, pad: Pad) {
        let Some(cal) = self.joypad.calibration.as_mut() else {
            return;
        };
        let commands = &mut self.joypad.commands;
        match (cal.step, pad) {
            _ if cal.busy => {}
            (Step::Center, Pad::B) => self.leave_calibration(),
            (Step::Travel, Pad::X) => cal.travel = Travel::default(),
            (Step::Travel, Pad::B) => {
                cal.step = Step::Center;
                cal.rest = Rest::default();
                cal.travel = Travel::default();
            }
            (Step::Deadzone, Pad::L1 | Pad::R1) => cal.stick = usize::from(pad == Pad::R1),
            (Step::Deadzone, Pad::Left | Pad::Right) => {
                let dz = cal.deadzone[cal.stick];
                let next = if pad == Pad::Left {
                    dz.saturating_sub(1)
                } else {
                    (dz + 1).min(DEADZONE_MAX)
                };
                if next != dz {
                    cal.deadzone[cal.stick] = next;
                    commands.push(Command::Fit(cal.fits()));
                }
            }
            (Step::Deadzone, Pad::A) => {
                cal.step = Step::Summary;
                cal.cursor = 0;
                commands.push(Command::Sample(false));
            }
            (Step::Deadzone, Pad::B) => {
                cal.step = Step::Travel;
                cal.travel = Travel::default();
            }
            (Step::Summary, Pad::Up) => cal.cursor = cal.cursor.saturating_sub(1),
            (Step::Summary, Pad::Down) => cal.cursor = (cal.cursor + 1).min(CHOICES.len() - 1),
            (Step::Summary, Pad::B) => {
                cal.step = Step::Deadzone;
                commands.push(Command::Sample(true));
            }
            (Step::Summary, Pad::A) if cal.cursor == 1 => {
                self.joypad.seen = 0;
                self.open(Page::PadTest);
            }
            (Step::Summary, Pad::A) => {
                cal.busy = true;
                commands.push(if cal.cursor == 0 {
                    Command::Save
                } else {
                    Command::Reset
                });
            }
            _ => {}
        }
    }
    /// Out of the calibration without saving: the driver as it was.
    fn leave_calibration(&mut self) {
        let Some(cal) = self.joypad.calibration.take() else {
            return;
        };
        self.joypad.send(Command::Sample(false));
        if cal.changed {
            self.joypad.send(Command::Restore(Box::new(cal.before)));
        }
        self.close_calibration();
    }
    fn close_calibration(&mut self) {
        if self.page() == Page::PadCalibrate {
            self.stack.pop();
            self.cursor = self.stack.last().expect("root page").1;
        }
    }
    /// A reading of the driver's file, taken at `now`.
    pub fn joypad_read(&mut self, axes: Axes, now: Instant) {
        let Some(cal) = self.joypad.calibration.as_mut() else {
            return;
        };
        cal.now = axes;
        match cal.step {
            Step::Center => {
                if cal.rest.add(&axes, now) {
                    cal.step = Step::Travel;
                    cal.travel = Travel::default();
                }
            }
            Step::Travel => {
                cal.travel.add(&axes, &cal.rest);
                if (0..2).all(|stick| cal.travel.ready(stick, &axes, &cal.rest)) {
                    cal.step = Step::Deadzone;
                    cal.stick = 0;
                    cal.suggested = std::array::from_fn(|stick| {
                        let wobble = cal
                            .rest
                            .wobble(2 * stick)
                            .max(cal.rest.wobble(2 * stick + 1));
                        suggest(wobble, cal.span(stick))
                    });
                    cal.deadzone = cal.suggested;
                    cal.changed = true;
                    self.joypad.commands.push(Command::Fit(cal.fits()));
                }
            }
            Step::Deadzone | Step::Summary => {}
        }
    }
    /// The answer to a `Command` the host ran.
    pub fn joypad_done(&mut self, command: &Command, result: Result<(), String>) {
        if let Err(error) = &result {
            log::warn!("archr-joypad-cal {command:?}: {error}");
        }
        let ok = result.is_ok();
        let message = match command {
            Command::Save | Command::Reset => {
                if let Some(cal) = self.joypad.calibration.as_mut() {
                    cal.busy = false;
                }
                if !ok {
                    let key = if *command == Command::Save {
                        "pad.save_failed"
                    } else {
                        "pad.factory_failed"
                    };
                    self.fail(None, key.into());
                    return;
                }
                // Kept or replaced: nothing to put back.
                self.joypad.calibration = None;
                self.close_calibration();
                self.joypad.status = None;
                self.joypad.send(Command::Open);
                if *command == Command::Save {
                    "pad.saved"
                } else {
                    "pad.factory_done"
                }
            }
            Command::Fit(_) if !ok => {
                self.fail(None, "pad.fit_failed".into());
                return;
            }
            Command::Restore(_) if !ok => {
                self.fail(None, "pad.restore_failed".into());
                return;
            }
            _ => return,
        };
        self.note(message.into());
    }
    pub(super) fn joypad_hints(&self) -> &'static [(crate::HintButton, &'static str)] {
        use crate::HintButton::{Key, Pair, StartSelect};
        let step = self.joypad.calibration.as_ref().map(|c| (c.step, c.busy));
        match (self.page(), step) {
            (Page::PadTest, _) if self.joypad.offers_calibration() => &[
                (Key('Y'), "pad.hold_calibrate"),
                (StartSelect, "pad.hold_exit"),
            ],
            (Page::PadTest, _) => &[(StartSelect, "pad.hold_exit")],
            (_, Some((_, true))) => &[],
            (_, Some((Step::Center, _))) => &[(Key('B'), "hint.cancel")],
            (_, Some((Step::Travel, _))) => &[(Key('X'), "pad.redo"), (Key('B'), "hint.back")],
            (_, Some((Step::Deadzone, _))) => &[
                (Pair('‹', '›'), "hint.adjust"),
                (Key('A'), "pad.continue"),
                (Key('B'), "hint.back"),
            ],
            _ => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
        }
    }
    /// The note on the left of the footer.
    pub(crate) fn joypad_note(&self) -> Option<&'static str> {
        let cal = self.joypad.calibration.as_ref();
        let cal = cal.filter(|_| self.page() == Page::PadCalibrate)?;
        Some(match cal.step {
            _ if cal.busy => "pad.working",
            Step::Center => "pad.center_note",
            Step::Travel => "pad.travel_note",
            Step::Deadzone => "pad.deadzone_note",
            Step::Summary => "pad.summary_note",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Id;

    const STATUS: &str = "\
x raw=1810 center=1790 fixed=0 neg=100 pos=100 deadzone=40 max=1800 invert=0
y raw=1700 center=1796 fixed=0 neg=100 pos=100 deadzone=40 max=1800 invert=1
rx raw=1802 center=1809 fixed=1 neg=104 pos=98 deadzone=40 max=1800 invert=0
ry raw=1798 center=1795 fixed=0 neg=100 pos=100 deadzone=40 max=1800 invert=0
";
    fn axes() -> Axes {
        parse(STATUS).expect("four axes")
    }
    /// The four axes with these raw readings.
    fn at(raw: [i32; 4]) -> Axes {
        let mut axes = axes();
        for (axis, raw) in axes.iter_mut().zip(raw) {
            axis.raw = raw;
        }
        axes
    }
    fn press(state: &mut Settings, pad: Pad, now: Instant) {
        state.joypad_input(Input::Button(pad, true), now);
        state.joypad_input(Input::Button(pad, false), now);
    }
    /// The tester open, with calibration, on the R36S pad.
    fn tester() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::System);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::PadTestLink)
            .expect("System links to the tester");
        state.handle(crate::state::Button::A);
        assert_eq!(state.page(), Page::PadTest);
        assert_eq!(state.joypad_commands(), [Command::Open]);
        assert!(state.joypad_raw() && state.joypad_wants_device());
        state.joypad.device = Some(("r36s_Gamepad".into(), true));
        state.joypad_opened(Some(axes()));
        state
    }
    /// The calibration open on its first step.
    fn calibrating(now: Instant) -> Settings {
        let mut state = tester();
        state.joypad_input(Input::Button(Pad::Y, true), now);
        assert!(
            !state.joypad_tick(now + Duration::from_millis(900)),
            "a tap tests Y"
        );
        assert!(state.joypad_tick(now + HOLD));
        assert_eq!(state.page(), Page::PadCalibrate);
        assert_eq!(state.joypad_commands(), [Command::Sample(true)]);
        state.joypad_input(Input::Button(Pad::Y, false), now);
        state
    }
    /// The rest measured: both sticks still, a little off the old center.
    fn rested(state: &mut Settings, now: Instant) -> Instant {
        let mut t = now;
        for n in 0..=91 {
            let wobble = [0, 6, -4, 2][n % 4];
            state.joypad_read(at([1808 + wobble, 1700 - wobble, 1802, 1798]), t);
            t += SAMPLE_EVERY;
        }
        t
    }
    /// Three turns of both sticks along the edge.
    fn turned(state: &mut Settings, now: Instant) {
        let mut t = now;
        for step in 0..=3 * BINS * 2 + 2 {
            let angle = step as f32 * TAU / (2 * BINS) as f32;
            let (cos, sin) = (angle.cos(), angle.sin());
            state.joypad_read(
                at([
                    1808 + (1700. * cos) as i32,
                    1700 + (1650. * sin) as i32,
                    1802 + (1750. * cos) as i32,
                    1798 + (1760. * sin) as i32,
                ]),
                t,
            );
            t += SAMPLE_EVERY;
        }
    }
    fn calibration(state: &Settings) -> &Calibration {
        state.joypad.calibration.as_ref().expect("calibrating")
    }

    #[test]
    fn the_driver_lines_parse_and_restore_exactly() {
        let axes = axes();
        assert_eq!(axes[1].raw, 1700);
        assert!(axes[1].invert && !axes[0].invert);
        assert!(axes[2].fixed && !axes[0].fixed);
        assert_eq!((axes[2].neg, axes[2].pos, axes[2].max), (104, 98, 1800));
        assert_eq!(
            axes[0].restore("x"),
            [
                "set",
                "x",
                "center=auto",
                "neg=100",
                "pos=100",
                "deadzone=40"
            ]
        );
        assert_eq!(axes[2].restore("rx")[2], "center=1809");
        assert_eq!(
            parse(&STATUS.replace("ry raw", "rz raw")),
            None,
            "four axes"
        );
        assert_eq!(parse(&STATUS.replace("max=1800", "max=0")), None);
        assert_eq!(parse(""), None);
        // Unknown keys and odd words are skipped.
        let extra = STATUS.replace("invert=0\n", "invert=0 flat=3 junk\n");
        assert_eq!(parse(&extra), Some(axes));
        assert_eq!(
            Fit {
                center: 1808,
                min: 100,
                max: 3500,
                deadzone: 5
            }
            .args("x"),
            ["fit", "x", "1808", "100", "3500", "5"]
        );
    }

    #[test]
    fn the_rest_restarts_on_a_move_and_reports_its_wobble() {
        let now = Instant::now();
        let mut rest = Rest::default();
        assert!(!rest.add(&at([1800, 1800, 1800, 1800]), now));
        assert!(!rest.add(&at([1806, 1797, 1800, 1800]), now + Duration::from_secs(2)));
        // 5 % of the range away: the count starts again from here.
        let later = now + Duration::from_millis(2500);
        assert!(!rest.add(&at([1800, 1900, 1800, 1800]), later));
        assert_eq!(rest.mean(1), 1900);
        assert_eq!(rest.done(later), 0.);
        assert!(!rest.add(
            &at([1804, 1890, 1800, 1800]),
            later + Duration::from_secs(2)
        ));
        assert!(rest.add(&at([1796, 1905, 1800, 1800]), later + REST));
        assert_eq!(rest.mean(1), 1898);
        assert_eq!(rest.wobble(1), 8);
        assert_eq!(rest.wobble(0), 4);
        assert_eq!(rest.done(later + REST), 1.);
    }

    #[test]
    fn the_travel_counts_turns_along_the_edge_only() {
        let now = Instant::now();
        let mut rest = Rest::default();
        rest.add(&at([1800; 4]), now);
        let mut travel = Travel::default();
        // Wiggling near the center turns nothing.
        for n in 0..200 {
            let s = if n % 2 == 0 { 300 } else { -300 };
            travel.add(&at([1800 + s, 1800 - s, 1800, 1800]), &rest);
        }
        assert_eq!(travel.laps(0), 0.);
        assert!(!travel.ready(0, &at([1800; 4]), &rest));
        for step in 0..=8 * BINS {
            let angle = -(step as f32) * TAU / (4 * BINS) as f32;
            let r = 1700.;
            travel.add(
                &at([
                    1800 + (r * angle.cos()) as i32,
                    1800 + (r * angle.sin()) as i32,
                    1800,
                    1800,
                ]),
                &rest,
            );
        }
        assert!((travel.laps(0) - 2.).abs() < 0.01, "turned the other way");
        assert!(
            !travel.ready(0, &at([1800; 4]), &rest),
            "two of three turns"
        );
        assert_eq!(travel.side(0, &rest, 1800, true), Some(1700));
        assert!(
            travel.reach[0].iter().all(|r| *r > 0.9),
            "the whole edge drawn"
        );
        assert_eq!(
            travel.side(2, &rest, 1800, false),
            None,
            "the right stick never moved"
        );
    }

    #[test]
    fn the_suggestion_rounds_the_wobble_up_with_a_floor() {
        assert_eq!(suggest(0, 1700), 2);
        assert_eq!(suggest(34, 1700), 2);
        assert_eq!(suggest(35, 1700), 3);
        assert_eq!(suggest(85, 1700), 5);
        assert_eq!(suggest(1700, 1700), DEADZONE_MAX);
    }

    #[test]
    fn a_full_calibration_measures_fits_and_saves() {
        let now = Instant::now();
        let mut state = calibrating(now);
        assert_eq!(state.joypad_hints()[0].1, "hint.cancel");
        let t = rested(&mut state, now);
        let cal = calibration(&state);
        assert_eq!(cal.step, Step::Travel, "advanced on its own");
        assert_eq!((cal.rest.mean(0), cal.rest.mean(1)), (1809, 1699));
        assert_eq!(cal.rest.wobble(0), 5);
        // The old center 1796 with y inverted: 97 up as the games see it.
        assert_eq!(cal.drift(0), (1, 97));
        assert!(cal.drifts(0) && !cal.drifts(1));
        turned(&mut state, t);
        let cal = calibration(&state);
        assert_eq!(cal.step, Step::Deadzone, "both sticks turned three times");
        assert_eq!(cal.suggested, [2, 2]);
        assert!(cal.changed);
        let Some(Command::Fit(fits)) = state.joypad_commands().pop() else {
            panic!("the measurement goes to the driver");
        };
        assert_eq!(fits[0].center, 1809);
        assert!(fits[0].min < 1809 - 1650 && fits[0].max > 1809 + 1650);
        assert_eq!(fits[1].deadzone, 2);
        // R1 picks the right stick, the D-pad moves its deadzone.
        press(&mut state, Pad::R1, t);
        press(&mut state, Pad::Right, t);
        press(&mut state, Pad::Right, t);
        let commands = state.joypad_commands();
        assert_eq!(commands.len(), 2, "every change previews");
        let Command::Fit(fits) = &commands[1] else {
            panic!("a fit");
        };
        assert_eq!((fits[0].deadzone, fits[3].deadzone), (2, 4));
        press(&mut state, Pad::A, t);
        assert_eq!(calibration(&state).step, Step::Summary);
        assert_eq!(state.joypad_commands(), [Command::Sample(false)]);
        assert_eq!(state.joypad_note(), Some("pad.summary_note"));
        press(&mut state, Pad::A, t);
        assert_eq!(state.joypad_commands(), [Command::Save]);
        assert!(state.joypad_hints().is_empty(), "one thing at a time");
        press(&mut state, Pad::A, t);
        assert!(state.joypad_commands().is_empty());
        state.joypad_done(&Command::Save, Ok(()));
        assert_eq!(state.page(), Page::PadTest);
        assert!(state.joypad.calibration.is_none(), "nothing to put back");
        assert_eq!(state.joypad_commands(), [Command::Open], "the new values");
        assert_eq!(state.error, Some((None, "pad.saved".into())));
    }

    #[test]
    fn cancelling_puts_back_what_a_fit_changed() {
        let now = Instant::now();
        // Cancelled before anything went to the driver: nothing to put back.
        let mut state = calibrating(now);
        rested(&mut state, now);
        press(&mut state, Pad::B, now);
        assert_eq!(
            calibration(&state).step,
            Step::Center,
            "B goes one step back"
        );
        press(&mut state, Pad::B, now);
        assert_eq!(state.page(), Page::PadTest);
        assert_eq!(state.joypad_commands(), [Command::Sample(false)]);
        // After the deadzone step's fit: the values read at the start.
        let mut state = calibrating(now);
        let t = rested(&mut state, now);
        turned(&mut state, t);
        state.joypad_commands();
        for _ in 0..3 {
            press(&mut state, Pad::B, t);
        }
        assert_eq!(state.page(), Page::PadTest);
        assert_eq!(
            state.joypad_commands(),
            [Command::Sample(false), Command::Restore(Box::new(axes()))]
        );
        state.joypad_done(&Command::Restore(Box::new(axes())), Err("refused".into()));
        assert_eq!(state.error, Some((None, "pad.restore_failed".into())));
    }

    #[test]
    fn the_summary_tests_first_and_resets_to_the_console() {
        let now = Instant::now();
        let mut state = calibrating(now);
        let t = rested(&mut state, now);
        turned(&mut state, t);
        press(&mut state, Pad::A, t);
        state.joypad_commands();
        press(&mut state, Pad::Down, t);
        press(&mut state, Pad::A, t);
        assert_eq!(state.page(), Page::PadTest, "the tester over the summary");
        assert!(!state.joypad.offers_calibration());
        assert_eq!(state.joypad_hints().len(), 1, "only the way out");
        state.joypad_input(Input::Button(Pad::Start, true), t);
        state.joypad_input(Input::Button(Pad::Select, true), t);
        assert!(state.joypad_tick(t + HOLD));
        assert_eq!(state.page(), Page::PadCalibrate);
        assert_eq!(calibration(&state).step, Step::Summary);
        press(&mut state, Pad::Down, t);
        press(&mut state, Pad::Down, t);
        press(&mut state, Pad::A, t);
        assert_eq!(state.joypad_commands(), [Command::Reset]);
        state.joypad_done(&Command::Reset, Err("refused".into()));
        assert_eq!(state.page(), Page::PadCalibrate, "still there to try again");
        assert_eq!(state.error, Some((None, "pad.factory_failed".into())));
        press(&mut state, Pad::A, t);
        state.joypad_done(&Command::Reset, Ok(()));
        assert_eq!(state.page(), Page::PadTest);
        assert_eq!(state.joypad_commands(), [Command::Reset, Command::Open]);
        assert_eq!(state.error, Some((None, "pad.factory_done".into())));
    }

    #[test]
    fn the_tester_tests_every_control_and_only_the_held_pair_leaves() {
        let now = Instant::now();
        let mut state = tester();
        for pad in [Pad::B, Pad::Start, Pad::L1, Pad::Fn, Pad::Up] {
            press(&mut state, pad, now);
            assert_eq!(state.page(), Page::PadTest, "{pad:?} only tests");
        }
        state.joypad_input(Input::Button(Pad::L2, true), now);
        assert!(state.joypad.held(Pad::L2));
        state.joypad_input(Input::Stick(1, -32768), now);
        assert_eq!(state.joypad.sticks, [0, -32768, 0, 0]);
        assert_eq!(state.joypad.controls().len(), 17);
        let tested = state
            .joypad
            .controls()
            .iter()
            .filter(|p| state.joypad.tested(**p))
            .count();
        assert_eq!(tested, 6);
        // Start + Select: a short hold stays, a long one leaves.
        state.joypad_input(Input::Button(Pad::Select, true), now);
        state.joypad_input(Input::Button(Pad::Start, true), now);
        assert!(!state.joypad_tick(now + Duration::from_millis(500)));
        state.joypad_input(Input::Button(Pad::Start, false), now);
        assert!(!state.joypad_tick(now + HOLD), "let go in time");
        state.joypad_input(Input::Button(Pad::Start, true), now + HOLD);
        assert!(state.joypad_tick(now + 2 * HOLD));
        assert_eq!(state.page(), Page::System);
        // Without calibration, Y only tests.
        let mut state = tester();
        state.joypad_opened(None);
        assert!(!state.joypad.offers_calibration());
        state.joypad_input(Input::Button(Pad::Y, true), now);
        assert!(!state.joypad_tick(now + HOLD));
        assert_eq!(state.page(), Page::PadTest);
    }

    #[test]
    fn the_summary_compares_with_the_calibration_before() {
        let now = Instant::now();
        let mut state = calibrating(now);
        let t = rested(&mut state, now);
        turned(&mut state, t);
        let cal = calibration(&state);
        // Down on the left stick, inverted, read 1554 of 1800 from the old
        // center: 86 % of the range, up as the games see it.
        let (axis, positive, reach) = cal.short_side().expect("one side is short");
        assert_eq!((axis, positive), (1, false));
        assert!((85..=87).contains(&reach), "{reach}");
        assert_eq!(cal.deadzone_before(0), 2, "40 of about 1650");
        assert!(cal.wobble(0) <= 1);
    }
}
