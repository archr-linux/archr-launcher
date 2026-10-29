//! In-game hotkeys and inactivity: system.cfg keys input_sense reads once at
//! start, so every save restarts its unit, as the device quirks do.
use super::Backend;
use archr_config::SystemCfg;
use archr_ui::{
    pad::Pad,
    settings::{
        hotkeys::{hotkey_values, HOTKEYS, MODIFIERS},
        Id, Settings,
    },
};
use std::time::Duration;

/// `systemctl restart` waits for the old input_sense to stop.
const RESTART_TIMEOUT: Duration = Duration::from_secs(15);

/// A hotkey value input_sense can use: known pad buttons, as many as it takes.
fn valid_hotkey(n: usize, value: &str) -> bool {
    let keys: Vec<&str> = value.split('+').collect();
    let count = if HOTKEYS[n].default.contains('+') {
        2
    } else {
        1
    };
    keys.len() == count
        && keys.iter().enumerate().all(|(i, key)| {
            Pad::from_evdev(key).is_some_and(|p| p != Pad::Fn) && !keys[..i].contains(key)
        })
}

impl Backend {
    pub(super) fn load_controls(&self, cfg: &SystemCfg, state: &mut Settings) {
        state
            .values
            .extend(hotkey_values(|key| cfg.get_setting(key, None, None)));
    }
    /// Restart input_sense when it runs; a stopped unit reads the keys anyway.
    fn restart_input(&self) -> Result<(), String> {
        let active =
            self.command_with_timeout("systemctl", &["is-active", "input"], Duration::from_secs(5));
        if active.as_deref() == Ok("active") {
            self.command_with_timeout("systemctl", &["restart", "input"], RESTART_TIMEOUT)?;
        }
        Ok(())
    }
    pub(super) fn apply_controls(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        let entries: Vec<(&str, String)> = match id {
            Id::HotkeyModifier if MODIFIERS.contains(&value) => vec![(id.key(), value.into())],
            Id::Hotkey(n) if valid_hotkey(usize::from(n), value) => {
                vec![(id.key(), value.into())]
            }
            // Volume and brightness also need the key.dpad.events switch:
            // write both so turning one on never wakes the other.
            Id::HotkeyOn(n @ (3 | 4)) if matches!(value, "0" | "1") => {
                let other = Id::HotkeyOn(7 - n);
                let cfg = match self.cfg() {
                    Ok(cfg) => cfg,
                    Err(error) => return Some(Err(error)),
                };
                let other_on = hotkey_values(|key| cfg.get_setting(key, None, None))
                    .into_iter()
                    .find(|(id, _)| *id == other)
                    .map_or("0".into(), |(_, v)| v);
                let dpad = if value == "1" || other_on == "1" {
                    "1"
                } else {
                    "0"
                };
                vec![
                    (id.key(), value.into()),
                    (other.key(), other_on),
                    ("key.dpad.events", dpad.into()),
                ]
            }
            Id::HotkeyOn(_) if matches!(value, "0" | "1") => vec![(id.key(), value.into())],
            Id::Idle | Id::IdleGame if id.choices().contains(&value) => {
                vec![(id.key(), value.into())]
            }
            Id::HotkeyModifier | Id::Hotkey(_) | Id::HotkeyOn(_) | Id::Idle | Id::IdleGame => {
                return Some(Err("Invalid setting value".into()))
            }
            _ => return None,
        };
        let entries: Vec<(&str, &str)> = entries.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Some(
            SystemCfg::set_many_and_save(&self.paths.config, &self.paths.lock, &entries)
                .map_err(|e| e.to_string())
                .and_then(|()| self.restart_input())
                .map(|()| value.into()),
        )
    }
}
