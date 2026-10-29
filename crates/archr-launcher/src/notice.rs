//! How long the screen's notice stays up, and what takes it down early.
use std::time::{Duration, Instant};

/// Confirmations and news leave quickly; errors stay long enough to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Error,
}
impl Severity {
    /// The one place notice durations are set.
    pub const fn duration(self) -> Duration {
        match self {
            Self::Info => Duration::from_secs(3),
            Self::Error => Duration::from_secs(7),
        }
    }
    /// A locale key's severity: `error.*` keys are errors.
    pub fn of(key: &str) -> Self {
        if key.starts_with("error.") {
            Self::Error
        } else {
            Self::Info
        }
    }
}

/// When the notice shown now leaves.
#[derive(Clone, Copy, Debug)]
pub struct Timer {
    until: Instant,
}
impl Timer {
    pub fn new(severity: Severity, now: Instant) -> Self {
        Self {
            until: now + severity.duration(),
        }
    }
    /// A new notice: its own time, from now.
    pub fn restart(&mut self, severity: Severity, now: Instant) {
        *self = Self::new(severity, now);
    }
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.until
    }
}

/// Any press takes the notice down and still does its own work: a notice
/// only informs, never asks, so no press is wasted on it.
pub fn dismiss(banner: &mut Option<String>) {
    *banner = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmations_leave_quickly_and_errors_stay_to_be_read() {
        let now = Instant::now();
        assert_eq!(Severity::of("error.store_save"), Severity::Error);
        assert_eq!(Severity::of("details.updated"), Severity::Info);
        let info = Timer::new(Severity::Info, now);
        assert!(!info.expired(now + Duration::from_millis(2900)));
        assert!(info.expired(now + Duration::from_secs(3)));
        let mut error = Timer::new(Severity::Error, now);
        assert!(!error.expired(now + Duration::from_secs(6)));
        assert!(error.expired(now + Duration::from_secs(8)));
        error.restart(Severity::Info, now + Duration::from_secs(5));
        assert!(
            !error.expired(now + Duration::from_secs(7)),
            "from the restart"
        );
    }
    #[test]
    fn any_press_takes_the_notice_down() {
        let mut banner = Some("Game data updated".to_owned());
        dismiss(&mut banner);
        assert_eq!(banner, None);
    }
}
