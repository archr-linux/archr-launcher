//! Dedicated Wi-Fi list and on-screen text-entry views.
use super::*;
use crate::settings::{network::Network, Kind, Settings};

fn network_icons(network: &Network) -> (Glyph, bool, bool) {
    let strength = network.signal.parse::<u8>().unwrap_or(0);
    let signal = match strength {
        0..=35 => Glyph::WifiLow,
        36..=74 => Glyph::Wifi,
        _ => Glyph::WifiHigh,
    };
    (signal, network.security != "open", network.connected)
}

impl View<'_, '_, '_> {
    pub(super) fn wifi(&mut self, state: &Settings) -> Result<(), String> {
        let total = state.networks.len() + 4;
        let first = state.cursor.saturating_sub(6).min(total.saturating_sub(8));
        for pos in 0..8 {
            let n = first + pos;
            if n >= total {
                break;
            }
            let y = 56. + pos as f32 * 46.;
            let selected = state.cursor == n;
            let ink = self.ink(selected);
            self.row_panel((18., y, 405., 43.), selected)?;
            if n == 0 {
                self.label((32., y, 236., 43.), &self.tr("settings.wifi"), 15.)?;
                self.control(
                    Id::WifiSwitch,
                    Kind::Toggle,
                    state,
                    selected,
                    (310., y, 98., 43.),
                )?;
            } else if let Some(network) = state.networks.get(n - 1) {
                let (signal, secured, connected) = network_icons(network);
                self.ui
                    .icon(signal, self.rect((30., y + 12., 18., 18.)), ink)?;
                if secured {
                    self.ui
                        .icon(Glyph::Lock, self.rect((321., y + 6., 12., 12.)), ink)?;
                }
                if connected {
                    self.ui
                        .icon(Glyph::Check, self.rect((321., y + 26., 12., 12.)), ink)?;
                }
                let subtitle = if network.connected {
                    self.tr("settings.connected")
                } else if network.known {
                    self.tr("settings.saved")
                } else {
                    network.security.to_uppercase()
                };
                self.text(
                    (56., y + 1., 256., 23.),
                    &network.ssid,
                    (14., ink, Align::Left),
                )?;
                self.text(
                    (56., y + 23., 256., 16.),
                    &subtitle,
                    (12., ink, Align::Left),
                )?;
                self.text(
                    (347., y, 55., 43.),
                    &format!("{}%", network.signal),
                    (13., ink, Align::Right),
                )?;
            } else {
                let (key, icon) = match n - state.networks.len() {
                    1 => ("settings.hidden", Glyph::Wifi),
                    2 => ("settings.bluetooth", Glyph::Controls),
                    _ => ("settings.services", Glyph::Gear),
                };
                self.ui
                    .icon(icon, self.rect((30., y + 12., 18., 18.)), ink)?;
                self.text((56., y, 333., 43.), &self.tr(key), (14., ink, Align::Left))?;
                self.ui.icon(
                    Glyph::ChevronRight,
                    self.rect((394., y + 14., 12., 14.)),
                    ink,
                )?;
            }
        }
        self.panel((439., 56., 183., 266.), false)?;
        let connected = state.networks.iter().find(|network| network.connected);
        let name = connected.map_or_else(
            || self.tr("settings.not_connected"),
            |network| network.ssid.clone(),
        );
        self.label((452., 69., 159., 58.), &name, 16.)?;
        let detail = connected.map_or_else(
            || self.tr("settings.scan_help"),
            |_| format!("{} · {}", self.tr("settings.connected"), state.access_ip),
        );
        self.note((452., 134., 159., 123.), &detail, 13.)?;
        Ok(())
    }
    pub(super) fn keyboard(&mut self, state: &Settings) -> Result<(), String> {
        let Some(keyboard) = state.keyboard.as_ref() else {
            return Ok(());
        };
        let title = self.tr(&keyboard.title);
        let caption = if keyboard.secret {
            format!("{} {}", self.tr("settings.password_for"), title)
        } else {
            title
        };
        self.note((24., 66., 430., 30.), &caption, 14.)?;
        if keyboard.secret {
            self.note((453., 66., 163., 30.), &self.tr("settings.wpa_length"), 12.)?;
        }
        self.panel((24., 100., 592., 46.), false)?;
        self.label((38., 101., 560., 44.), &keyboard.display(), 18.)?;
        const ALPHA: &[u8; 30] = b"qwertyuiopasdfghjkl  zxcvbnm  ";
        const NUMBERS: &[u8; 30] = b"1234567890!@#$%^&*(  .:/_-+?  ";
        for n in 0..30 {
            let x = 56. + (n % 10) as f32 * 53.;
            let y = 175. + (n / 10) as f32 * 47.;
            let selected = keyboard.selected == n;
            self.panel((x, y, 47., 39.), selected)?;
            if let Some(glyph) = match n {
                20 => Some(Glyph::Shift),
                29 => Some(Glyph::Backspace),
                _ => None,
            } {
                self.ui.icon(
                    glyph,
                    self.rect((x + 14., y + 10., 19., 19.)),
                    self.ink(selected),
                )?;
                continue;
            }
            let text = match n {
                19 => {
                    if keyboard.numeric {
                        "abc".to_owned()
                    } else {
                        "123".to_owned()
                    }
                }
                28 => "_".into(),
                _ => {
                    let ch = if keyboard.numeric {
                        NUMBERS[n]
                    } else {
                        ALPHA[n]
                    } as char;
                    if keyboard.shifted {
                        ch.to_ascii_uppercase().to_string()
                    } else {
                        ch.to_string()
                    }
                }
            };
            self.text(
                (x, y, 47., 39.),
                &text,
                (15., self.ink(selected), Align::Center),
            )?;
        }
        self.panel((444., 330., 172., 48.), true)?;
        let submit = if state.keyboard_target == Some(Id::WifiSwitch) {
            "settings.connect"
        } else {
            "hint.confirm"
        };
        self.text(
            (450., 332., 160., 43.),
            &self.ui.catalog.text(submit),
            (15., self.theme.highlight_text, Align::Center),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_icons_report_strength_encryption_and_connection_separately() {
        let mut net = Network {
            ssid: "Test".into(),
            security: "wpa2".into(),
            signal: "20".into(),
            connected: false,
            known: false,
        };
        assert_eq!(network_icons(&net), (Glyph::WifiLow, true, false));
        net.signal = "60".into();
        assert_eq!(network_icons(&net), (Glyph::Wifi, true, false));
        net.signal = "90".into();
        net.connected = true;
        assert_eq!(network_icons(&net), (Glyph::WifiHigh, true, true));
        net.security = "open".into();
        assert_eq!(network_icons(&net), (Glyph::WifiHigh, false, true));
    }
}
