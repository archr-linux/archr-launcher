//! Vibration and stick LEDs: archr-rumble and archr-stick-leds read their
//! system.cfg keys on `apply`. `supported` runs once per launcher; a missing
//! tool or a failing answer keeps both pages away (R36S, PC).
use super::Backend;
use archr_ui::settings::{Id, Settings};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);

/// The tool that owns a row's key.
fn owner(id: Id) -> Option<&'static str> {
    match id {
        Id::Rumble | Id::RumbleStrength | Id::RumbleTest => Some("archr-rumble"),
        Id::LedsMode | Id::LedsColor | Id::LedsBrightness => Some("archr-stick-leds"),
        _ => None,
    }
}

impl Backend {
    pub(super) fn load_hardware(&self, state: &mut Settings) {
        let supported = |name| {
            self.command_with_timeout(name, &["supported"], TIMEOUT)
                .is_ok()
        };
        (state.rumble_supported, state.leds_supported) = *self
            .hardware
            .get_or_init(|| (supported("archr-rumble"), supported("archr-stick-leds")));
    }
    pub(super) fn apply_hardware(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        let name = owner(id)?;
        if id == Id::RumbleTest {
            return Some(
                self.command_with_timeout(name, &["test"], TIMEOUT)
                    .map(|_| String::new())
                    .map_err(|error| {
                        log::warn!("Vibration test failed: {error}");
                        "rumble.test_failed".to_owned()
                    }),
            );
        }
        let valid = match id {
            Id::RumbleStrength => value.parse::<u8>().is_ok_and(|n| n <= 100),
            _ => id.choices().contains(&value),
        };
        if !valid {
            return Some(Err("Invalid setting value".into()));
        }
        let old = match self.cfg() {
            Ok(cfg) => cfg.get_setting(id.key(), None, None),
            Err(error) => return Some(Err(error)),
        };
        if let Err(error) = self.save(id, value) {
            return Some(Err(error));
        }
        // What is saved is what the console does: a failed apply puts back
        // the value it had.
        Some(match self.command_with_timeout(name, &["apply"], TIMEOUT) {
            Ok(_) => Ok(value.into()),
            Err(error) => {
                log::warn!("{name} apply failed: {error}");
                let restored = self.save(id, old.as_deref().unwrap_or(id.default_value()));
                if let Err(error) = restored {
                    log::warn!("Cannot restore {}: {error}", id.key());
                }
                Err("controls.apply_failed".into())
            }
        })
    }
}
