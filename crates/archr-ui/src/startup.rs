//! Opt-in startup checkpoints. Disabled checkpoints only test the option tag;
//! formatting, clock reads and stderr writes stay inside the enabled branch.
use std::time::Instant;

/// Temporary, independent startup A/B switches. An absent variable enables all;
/// an explicit comma-separated list enables only those names (empty = baseline).
#[derive(Clone, Copy)]
pub(crate) struct Experiments(u8);
impl Experiments {
    const NAMES: [&'static str; 6] = [
        "nowinrecreate",
        "gles2",
        "fontshare",
        "badgecache",
        "shadowcache",
        "lazyassets",
    ];
    pub fn from_env() -> Result<Self, String> {
        let value = std::env::var("ARCHR_LAUNCHER_EXP").ok();
        Self::parse(value.as_deref())
    }
    fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let mut bits = 0;
        if !value.is_empty() && value != "none" {
            for name in value.split(',') {
                let index = Self::NAMES
                    .iter()
                    .position(|n| *n == name.trim())
                    .ok_or_else(|| format!("Unknown ARCHR_LAUNCHER_EXP: {name}"))?;
                bits |= 1 << index;
            }
        }
        Ok(Self(bits))
    }
    pub fn enabled(self, name: &str) -> bool {
        Self::NAMES
            .iter()
            .position(|n| *n == name)
            .is_some_and(|index| self.0 & (1 << index) != 0)
    }
    pub fn names(self) -> String {
        Self::NAMES
            .into_iter()
            .filter(|name| self.enabled(name))
            .collect::<Vec<_>>()
            .join(",")
    }
}
impl Default for Experiments {
    fn default() -> Self {
        Self((1 << Self::NAMES.len()) - 1)
    }
}

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
    fn experiments_default_on_but_explicit_lists_are_exact() {
        assert_eq!(
            Experiments::parse(None).unwrap().names(),
            "nowinrecreate,gles2,fontshare,badgecache,shadowcache,lazyassets"
        );
        for (input, enabled) in [
            ("", ""),
            ("none", ""),
            ("gles2", "gles2"),
            ("fontshare,nowinrecreate", "nowinrecreate,fontshare"),
        ] {
            assert_eq!(Experiments::parse(Some(input)).unwrap().names(), enabled);
        }
        assert!(Experiments::parse(Some("gles22")).is_err());
    }

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
