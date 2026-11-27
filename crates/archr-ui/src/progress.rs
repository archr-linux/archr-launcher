//! The progress of a long wait (a download, an install, a copy), the same on
//! every page and in every theme: the launcher draws it, a theme only colours
//! it. Bytes come from the worker; speed and time left are worked out here.
use std::time::{Duration, Instant};

/// Bytes moved so far; `total` 0 when nothing tells the size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transfer {
    pub done: u64,
    pub total: u64,
    /// Bytes per second, 0 until a second has passed.
    pub per_second: u64,
}
impl Transfer {
    /// How far, 0 to 100, when the total is known. An estimate can run
    /// short: it never shows 100 before the step ends.
    pub fn percent(&self) -> Option<u8> {
        (self.total > 0).then(|| (self.done.min(self.total) * 100 / self.total).min(99) as u8)
    }
    /// Seconds still to go at this speed.
    pub fn seconds_left(&self) -> Option<u64> {
        (self.total > self.done && self.per_second > 0)
            .then(|| (self.total - self.done).div_ceil(self.per_second))
    }
}

/// Where a wait is: the step running (an index into the steps its screen
/// names) and, while that step moves bytes, how many.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub step: usize,
    pub transfer: Option<Transfer>,
}

/// Turns byte counts sampled over time into a `Transfer` with its speed,
/// averaged over the last few seconds so it does not jump with every read.
#[derive(Clone, Debug)]
pub struct Meter {
    total: u64,
    samples: Vec<(Instant, u64)>,
}
impl Meter {
    const WINDOW: Duration = Duration::from_secs(4);
    pub fn new(total: u64) -> Self {
        Self {
            total,
            samples: Vec::new(),
        }
    }
    pub fn read(&mut self, done: u64) -> Transfer {
        self.read_at(done, Instant::now())
    }
    pub fn read_at(&mut self, done: u64, now: Instant) -> Transfer {
        self.samples.push((now, done));
        self.samples
            .retain(|(at, _)| now.saturating_duration_since(*at) <= Self::WINDOW);
        let per_second = match self.samples.first() {
            Some(&(at, first)) if now.saturating_duration_since(at) >= Duration::from_secs(1) => {
                let seconds = now.saturating_duration_since(at).as_secs_f64();
                (done.saturating_sub(first) as f64 / seconds) as u64
            }
            _ => 0,
        };
        Transfer {
            done,
            total: self.total,
            per_second,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_and_time_left_follow_the_bytes() {
        let t = Transfer {
            done: 32,
            total: 64,
            per_second: 8,
        };
        assert_eq!(t.percent(), Some(50));
        assert_eq!(t.seconds_left(), Some(4));
        // An estimate passed: never 100% before the step ends.
        let over = Transfer {
            done: 70,
            total: 64,
            per_second: 8,
        };
        assert_eq!(over.percent(), Some(99));
        assert_eq!(over.seconds_left(), None);
        let unknown = Transfer {
            done: 10,
            total: 0,
            per_second: 5,
        };
        assert_eq!(unknown.percent(), None);
        assert_eq!(unknown.seconds_left(), None);
    }

    #[test]
    fn the_meter_averages_over_the_last_seconds() {
        let start = Instant::now();
        let mut meter = Meter::new(100);
        assert_eq!(meter.read_at(0, start).per_second, 0);
        assert_eq!(
            meter
                .read_at(10, start + Duration::from_millis(500))
                .per_second,
            0
        );
        assert_eq!(
            meter.read_at(20, start + Duration::from_secs(2)).per_second,
            10
        );
        // Old samples leave the window.
        let late = meter.read_at(60, start + Duration::from_secs(6));
        assert_eq!(late.per_second, 10);
        assert_eq!(late.total, 100);
    }
}
