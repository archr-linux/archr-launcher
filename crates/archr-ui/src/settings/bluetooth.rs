//! Bluetooth page: device model, `archr-bluetooth` output parsing and input.
use super::{Effect, Id, Page, Settings};
use crate::state::Button;
use std::{collections::HashSet, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Controller,
    Audio,
    Other,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub addr: String,
    pub name: String,
    pub kind: DeviceKind,
    pub paired: bool,
    pub connected: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pair,
    Connect,
    Disconnect,
    Forget,
}

/// `AA:BB:CC:DD:EE:FF`, the only form `archr-bluetooth` accepts.
pub fn valid_address(addr: &str) -> bool {
    addr.len() == 17
        && addr.split(':').count() == 6
        && addr.split(':').all(|byte| {
            byte.len() == 2 && byte.bytes().all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'F'))
        })
}

/// One `<device id=".." name=".." ... />` line; the name may hold quotes, so it
/// ends at the attribute that follows it.
fn line(text: &str) -> Option<(&str, &str, &str, &str, bool)> {
    let text = text.trim().strip_prefix("<device ")?;
    let id = text.strip_prefix("id=\"")?.split('"').next()?;
    let rest = &text[text.find("name=\"")? + 6..];
    let end = rest
        .find("\" status=\"")
        .or_else(|| rest.find("\" type=\""))?;
    let (name, tail) = (&rest[..end], &rest[end..]);
    let field = |key: &str| {
        let from = tail.find(key).map_or("", |at| &tail[at + key.len()..]);
        from.split('"').next().unwrap_or("")
    };
    Some((
        id,
        name,
        field(" type=\""),
        field(" status=\""),
        field(" connected=\"") == "yes",
    ))
}
fn kind(icon: &str) -> DeviceKind {
    match icon {
        "joystick" => DeviceKind::Controller,
        "audio" => DeviceKind::Audio,
        _ => DeviceKind::Other,
    }
}

/// Paired devices (connected first), then the discovered ones that are not
/// paired. `live` is an event log: a removal cancels an earlier addition.
pub fn parse_devices(paired: &str, live: &str) -> Vec<Device> {
    let mut devices: Vec<Device> = paired
        .lines()
        .filter_map(line)
        .filter(|(id, ..)| valid_address(id))
        .map(|(id, name, icon, _, connected)| Device {
            addr: id.into(),
            name: name.into(),
            kind: kind(icon),
            paired: true,
            connected,
        })
        .collect();
    devices.sort_by_key(|device| !device.connected);
    let known: HashSet<String> = devices.iter().map(|d| d.addr.clone()).collect();
    let mut found: Vec<Device> = Vec::new();
    for (id, name, icon, status, _) in live.lines().filter_map(line) {
        found.retain(|device| device.addr != id);
        // An unnamed device reports the text "None".
        if status == "added"
            && valid_address(id)
            && !known.contains(id)
            && !matches!(name, "" | "None")
        {
            found.push(Device {
                addr: id.into(),
                name: name.into(),
                kind: kind(icon),
                paired: false,
                connected: false,
            });
        }
    }
    devices.extend(found);
    devices
}

impl Settings {
    /// Switch row, then the devices when the radio is on.
    pub fn bt_rows(&self) -> usize {
        if self.value(Id::BtSwitch) == "1" {
            self.bt.len() + 1
        } else {
            1
        }
    }
    /// The page's list refresh: running now, and when it last finished.
    pub fn refreshing(&self) -> Option<(bool, Option<Instant>)> {
        match self.page() {
            Page::Wifi => Some((self.scanning, self.scanned)),
            Page::Bluetooth => Some((self.bt_scanning, self.bt_scanned)),
            _ => None,
        }
    }
    /// The device under the cursor on the Bluetooth page.
    pub fn bt_device(&self) -> Option<&Device> {
        (self.page() == Page::Bluetooth)
            .then(|| self.cursor.checked_sub(1))
            .flatten()
            .and_then(|n| self.bt.get(n))
    }
    pub(super) fn bt_input(&mut self, button: Button) -> Option<Effect> {
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(self.bt_rows() - 1),
            Button::X => {
                let device = self.bt_device().filter(|d| d.paired)?;
                self.confirm_forget = Some(device.addr.clone());
            }
            Button::Y if self.value(Id::BtSwitch) == "1" => return Some(Effect::Scan),
            Button::A if self.cursor > 0 => {
                let device = self.bt_device()?.clone();
                let action = match (device.paired, device.connected) {
                    (false, _) => Action::Pair,
                    (true, false) => Action::Connect,
                    (true, true) => Action::Disconnect,
                };
                if action != Action::Disconnect {
                    self.connecting = Some(device.name);
                }
                return Some(Effect::Bluetooth(action, device.addr));
            }
            // The switch row uses the shared row handling.
            Button::A | Button::L | Button::R if self.cursor == 0 => return None,
            Button::B => return None,
            _ => {}
        }
        Some(Effect::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAIRED: &str = "<device id=\"00:1A:2B:3C:4D:5E\" name=\"JBL Go 3\" type=\"\" connected=\"no\" />\n\
        <device id=\"11:22:33:44:55:66\" name=\"8BitDo \"SN30\" Pro\" type=\"joystick\" connected=\"yes\" />\n\
        <device id=\"bad\" name=\"x\" type=\"\" connected=\"no\" />\n";
    const LIVE: &str = "<device id=\"00:1A:2B:3C:4D:5E\" name=\"JBL Go 3\" status=\"added\" type=\"audio\" />\n\
        <device id=\"AA:BB:CC:DD:EE:01\" name=\"Xbox Wireless Controller\" status=\"added\" type=\"joystick\" />\n\
        <device id=\"AA:BB:CC:DD:EE:02\" name=\"Gone\" status=\"added\" type=\"unknown\" />\n\
        <device id=\"AA:BB:CC:DD:EE:02\" name=\"Gone\" status=\"removed\" type=\"unknown\" />\n\
        <device id=\"AA:BB:CC:DD:EE:03\" name=\"None\" status=\"added\" type=\"unknown\" />\n";

    #[test]
    fn paired_come_first_connected_first_and_discovered_exclude_them() {
        let devices = parse_devices(PAIRED, LIVE);
        let names: Vec<_> = devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "8BitDo \"SN30\" Pro",
                "JBL Go 3",
                "Xbox Wireless Controller"
            ]
        );
        assert!(devices[0].connected && devices[0].paired);
        assert_eq!(devices[0].kind, DeviceKind::Controller);
        assert!(!devices[2].paired && devices[2].kind == DeviceKind::Controller);
    }
    #[test]
    fn only_canonical_addresses_are_accepted() {
        assert!(valid_address("AA:BB:CC:DD:EE:0F"));
        for bad in [
            "aa:bb:cc:dd:ee:0f",
            "AA:BB:CC:DD:EE",
            "AA:BB:CC:DD:EE:0F:00",
            "$(x)",
        ] {
            assert!(!valid_address(bad), "{bad}");
        }
    }
    fn on() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.values.insert(Id::BtSwitch, "1".into());
        state.bt = parse_devices(PAIRED, LIVE);
        state.open(Page::Bluetooth);
        state
    }
    #[test]
    fn a_runs_the_action_that_fits_the_device() {
        let mut state = on();
        for (cursor, action, dialog) in [
            (1, Action::Disconnect, false),
            (2, Action::Connect, true),
            (3, Action::Pair, true),
        ] {
            state.connecting = None;
            state.cursor = cursor;
            let effect = state.handle(Button::A);
            assert!(matches!(effect, Effect::Bluetooth(a, _) if a == action));
            assert_eq!(state.connecting.is_some(), dialog);
        }
        assert_eq!(state.handle(Button::B), Effect::None);
    }
    #[test]
    fn x_forgets_only_paired_devices_after_asking() {
        let mut state = on();
        state.cursor = 3;
        state.handle(Button::X);
        assert_eq!(state.confirm_forget, None);
        state.cursor = 2;
        state.handle(Button::X);
        let addr = state.confirm_forget.clone().unwrap();
        assert!(matches!(
            state.handle(Button::A),
            Effect::Bluetooth(Action::Forget, a) if a == addr
        ));
    }
    #[test]
    fn with_the_radio_off_only_the_switch_remains() {
        let mut state = on();
        state.values.insert(Id::BtSwitch, "0".into());
        assert_eq!(state.bt_rows(), 1);
        assert_eq!(state.handle(Button::Y), Effect::None);
        state.handle(Button::Down);
        assert_eq!(state.cursor, 0);
    }
}
