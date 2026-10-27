//! Interruptible, monotonic ease-out motion. Input never waits for a frame.
use std::time::{Duration, Instant};

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
    pub fn value(&self, now: Instant) -> f32 {
        let Some(began) = self.began else {
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
    pub fn active(&self, now: Instant) -> bool {
        self.began
            .is_some_and(|began| now.saturating_duration_since(began) < self.duration)
    }
}
