//! Network › USB adapters (Rede-adaptadores): plugged now, seen before.
use super::{Effect, Page, Settings};
use crate::{state::Button, usb::Adapter, HintButton};

impl Settings {
    /// The adapter under the cursor.
    pub fn usb_adapter(&self) -> Option<&Adapter> {
        self.adapters.as_ref()?.get(self.cursor)
    }
    /// From the Wi-Fi page: the host reads the adapters again.
    pub(super) fn open_usb_page(&mut self) -> Effect {
        self.adapters = None;
        self.usb_details = false;
        self.open(Page::UsbAdapters);
        Effect::Adapters(None)
    }
    /// From the announcement over any screen: Settings › Network › USB
    /// adapters, so B walks back through Network.
    pub fn open_usb(&mut self) -> Effect {
        self.stack.truncate(1);
        self.cursor = 0;
        self.open(Page::Wifi);
        self.cursor = self.wifi_rows() - 1;
        self.open_usb_page()
    }
    pub(super) fn usb_input(&mut self, button: Button) -> Option<Effect> {
        // Any press closes the details.
        if std::mem::take(&mut self.usb_details) {
            return Some(Effect::None);
        }
        let len = self.adapters.as_ref().map_or(0, |usb| usb.len());
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(len.saturating_sub(1)),
            Button::A => self.usb_details = self.usb_adapter().is_some(),
            // Forgotten while plugged, it stays listed until unplugged.
            Button::X if self.usb_adapter().is_some() => {
                let id = self.usb_adapter().map(|a| a.id.clone());
                self.cursor = self.cursor.saturating_sub(1);
                return Some(Effect::Adapters(id));
            }
            Button::B => return None,
            _ => {}
        }
        Some(Effect::None)
    }
    pub(super) fn usb_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::Key;
        if self.usb_details || self.usb_adapter().is_none() {
            &[(Key('B'), "hint.back")]
        } else {
            &[
                (Key('A'), "usb.details"),
                (Key('X'), "usb.forget"),
                (Key('B'), "hint.back"),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::Adapters;
    const NOW: &str =
        "seq\t1\ndev\t0bda:b733\tRealtek RTL8733BU\twifi+bt\t2.4+5\t-\t-\t-\t-\tno_driver\n";
    const SEEN: &str = "dev\t2604:0013\tAIC8800DC\twifi\t2.4\taic8800_fdrv_usb\taic8800_fdrv_usb\twlan0\t-\tworks\n";
    #[test]
    fn the_announcement_opens_the_page_under_network() {
        let mut state = Settings {
            loading: false,
            ..Settings::default()
        };
        assert_eq!(state.open_usb(), Effect::Adapters(None));
        assert_eq!(state.page(), Page::UsbAdapters);
        assert_eq!(state.parent_page(), Page::Wifi);
        state.adapters = Some(Adapters::new(NOW, SEEN, ""));
        assert_eq!(state.usb_hints()[1].1, "usb.forget");
        state.handle(Button::A);
        assert!(state.usb_details);
        state.handle(Button::A);
        assert!(!state.usb_details, "any press closes the details");
        state.handle(Button::Down);
        assert_eq!(
            state.handle(Button::X),
            Effect::Adapters(Some("2604:0013".into()))
        );
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Wifi);
        assert_eq!(state.cursor, state.wifi_rows() - 1);
    }
}
