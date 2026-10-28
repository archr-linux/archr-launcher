//! Opt-in startup checkpoints. Disabled checkpoints only test the option tag;
//! formatting, clock reads and stderr writes stay inside the enabled branch.
use std::time::Instant;

#[derive(Clone, Copy, Default)]
pub struct StartupTrace {
    began: Option<Instant>,
}

impl StartupTrace {
    /// Read the switch once, using the instant captured at entry to main.
    pub fn new(began: Instant) -> Self {
        Self {
            began: std::env::var_os("ARCHR_LAUNCHER_TRACE")
                .is_some_and(|value| value == "1")
                .then_some(began),
        }
    }

    #[inline]
    pub fn began(self) -> Option<Instant> {
        self.began
    }

    /// Disable this checkpoint after its first successful operation.
    #[inline]
    pub fn phase_once(&mut self, phase: &str) {
        if let Some(began) = self.began {
            self.began = None;
            eprintln!(
                "archr-launcher: startup {phase}: {} ms",
                began.elapsed().as_millis()
            );
        }
    }
}

/// Keep even dynamic phase arguments unevaluated when tracing is disabled.
#[macro_export]
macro_rules! startup_phase {
    ($trace:expr, $($phase:tt)*) => {
        if let Some(began) = $trace.began() {
            eprintln!(
                "archr-launcher: startup {}: {} ms",
                format_args!($($phase)*),
                began.elapsed().as_millis()
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_checkpoints_do_not_evaluate_details() {
        let mut evaluated = false;
        startup_phase!(StartupTrace::default(), "{}", {
            evaluated = true;
            "detail"
        });
        assert!(!evaluated);
    }
}
