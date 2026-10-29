//! Interruptible, monotonic ease-out motion. Input never waits for a frame.
use std::time::{Duration, Instant};

/// The selection gliding to the next entry, lists scrolling.
pub const SELECT: Duration = Duration::from_millis(180);
/// Systems to games and back: the old screen leaves, the new one arrives.
pub const SCREEN: Duration = Duration::from_millis(220);
/// A cover or console picture giving way to the next one.
pub const CROSSFADE: Duration = Duration::from_millis(200);
/// Menus and dialogs appearing.
pub const DIALOG: Duration = Duration::from_millis(160);
/// Notices rising from the bottom.
pub const NOTICE: Duration = Duration::from_millis(200);
/// A switch's knob crossing over.
pub const SWITCH: Duration = Duration::from_millis(150);
/// Each band of the screen fading in when the launcher opens.
pub const OPENING: Duration = Duration::from_millis(300);
/// Fades never run longer than this when motion is reduced.
const SHORT_FADE: Duration = Duration::from_millis(120);

/// The Appearance choice: full motion, short fades only, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    On,
    Reduced,
    Off,
}
impl Motion {
    pub const CODES: [&'static str; 3] = ["on", "reduced", "off"];
    /// Saved value; "1" and "0" are how earlier versions saved on and off.
    pub fn parse(code: &str) -> Self {
        match code {
            "reduced" => Self::Reduced,
            "off" | "0" => Self::Off,
            _ => Self::On,
        }
    }
    pub fn code(self) -> &'static str {
        Self::CODES[self as usize]
    }
    /// How long something takes to move or grow: only with full motion.
    pub fn travel(self, full: Duration) -> Duration {
        if self == Self::On {
            full
        } else {
            Duration::ZERO
        }
    }
    /// How long a fade takes: shortened when reduced, instant when off.
    pub fn fade(self, full: Duration) -> Duration {
        match self {
            Self::On => full,
            Self::Reduced => full.min(SHORT_FADE),
            Self::Off => Duration::ZERO,
        }
    }
    /// Search dot `n` (opacity, lift in pixels) `secs` into the animation.
    /// Reduced motion only fades; off holds the dots still.
    pub fn search_dot(self, secs: f32, n: usize) -> (f32, f32) {
        if self == Self::Off {
            return (0.6, 0.);
        }
        let p = cycle(secs, 1.2, 0.15 * n as f32);
        let peak = if p < 0.4 {
            p / 0.4
        } else {
            ((0.8 - p) / 0.4).max(0.)
        };
        (0.25 + 0.75 * peak, self.distance(3.) * peak)
    }
    /// Opacity of placeholder row `n` while a search has found nothing yet.
    pub fn search_skeleton(self, secs: f32, n: usize) -> f32 {
        if self == Self::Off {
            return 0.5;
        }
        0.35 + 0.35 * (1. - (2. * cycle(secs, 1.4, 0.15 * n as f32) - 1.).abs())
    }
    /// Pixels something travels: none unless motion is on.
    pub fn distance(self, full: f32) -> f32 {
        if self == Self::On {
            full
        } else {
            0.
        }
    }
}

/// Position in a looping animation of `period` seconds, started `delay` late.
fn cycle(secs: f32, period: f32, delay: f32) -> f32 {
    ((secs - delay) / period).rem_euclid(1.)
}

pub struct Tween {
    from: f32,
    pub target: f32,
    began: Option<Instant>,
    duration: Duration,
}
impl Tween {
    pub fn new(value: f32, duration: Duration) -> Self {
        Self {
            from: value,
            target: value,
            began: None,
            duration,
        }
    }
    /// From `from` to `to`, starting now.
    pub fn starting(from: f32, to: f32, duration: Duration, now: Instant) -> Self {
        let mut tween = Self::new(from, duration);
        tween.retarget(to, now);
        tween
    }
    pub fn value(&self, now: Instant) -> f32 {
        let Some(began) = self.began.filter(|_| !self.duration.is_zero()) else {
            return self.target;
        };
        let t = (now.saturating_duration_since(began).as_secs_f32() / self.duration.as_secs_f32())
            .clamp(0.0, 1.0);
        self.from + (self.target - self.from) * (1.0 - (1.0 - t).powi(3))
    }
    pub fn retarget(&mut self, target: f32, now: Instant) {
        if target != self.target {
            self.from = self.value(now);
            self.target = target;
            self.began = Some(now);
        }
    }
    pub fn snap(&mut self) {
        self.from = self.target;
        self.began = None;
    }
    /// Later motion takes `duration`; zero makes every change instant.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }
    pub fn active(&self, now: Instant) -> bool {
        self.began
            .is_some_and(|began| now.saturating_duration_since(began) < self.duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_on_and_off_values_still_parse() {
        assert_eq!(Motion::parse("1"), Motion::On);
        assert_eq!(Motion::parse("0"), Motion::Off);
        for motion in [Motion::On, Motion::Reduced, Motion::Off] {
            assert_eq!(Motion::parse(motion.code()), motion);
        }
        assert_eq!(Motion::parse("anything"), Motion::On);
    }
    #[test]
    fn reduced_keeps_short_fades_only_and_off_is_instant() {
        assert_eq!(Motion::On.travel(SELECT), SELECT);
        assert_eq!(Motion::On.fade(CROSSFADE), CROSSFADE);
        assert_eq!(Motion::Reduced.travel(SCREEN), Duration::ZERO);
        assert_eq!(Motion::Reduced.distance(40.), 0.);
        assert!(Motion::Reduced.fade(CROSSFADE) > Duration::ZERO);
        assert!(Motion::Reduced.fade(CROSSFADE) < CROSSFADE);
        assert_eq!(
            Motion::Reduced.fade(Duration::from_millis(80)),
            Duration::from_millis(80)
        );
        assert_eq!(Motion::Off.fade(CROSSFADE), Duration::ZERO);
        assert_eq!(Motion::Off.travel(SELECT), Duration::ZERO);
    }
    #[test]
    fn zero_duration_tweens_jump_and_long_ones_glide_until_interrupted() {
        let now = Instant::now();
        let mut instant = Tween::new(0., Duration::ZERO);
        instant.retarget(10., now);
        assert_eq!(instant.value(now), 10.);
        assert!(!instant.active(now));
        let mut glide = Tween::starting(0., 10., SELECT, now);
        let half = glide.value(now + SELECT / 2);
        assert!(
            half > 5. && half < 10.,
            "ease-out is past halfway at half time"
        );
        assert!(glide.active(now + SELECT / 2));
        glide.snap();
        assert_eq!(glide.value(now), 10.);
        assert!(!glide.active(now));
    }
    #[test]
    fn search_dots_pulse_in_sequence_and_respect_the_motion_choice() {
        let (first, lift) = Motion::On.search_dot(0.48, 0);
        assert!(
            (first - 1.).abs() < 1e-3 && (lift - 3.).abs() < 1e-3,
            "peak"
        );
        assert!(
            Motion::On.search_dot(0.48, 1).0 < first,
            "the next one lags"
        );
        assert_eq!(Motion::On.search_dot(0., 0), Motion::On.search_dot(1.2, 0));
        let (fading, flat) = Motion::Reduced.search_dot(0.48, 0);
        assert_eq!((fading, flat), (first, 0.), "reduced only changes opacity");
        assert_eq!(
            Motion::Off.search_dot(0., 0),
            Motion::Off.search_dot(0.48, 2)
        );
        assert_eq!(
            Motion::Off.search_skeleton(0., 0),
            Motion::Off.search_skeleton(0.7, 1)
        );
        let (low, high) = (
            Motion::On.search_skeleton(0., 0),
            Motion::On.search_skeleton(0.7, 0),
        );
        assert!(low < high && high <= 0.7 + 1e-3, "rows breathe");
    }
}
