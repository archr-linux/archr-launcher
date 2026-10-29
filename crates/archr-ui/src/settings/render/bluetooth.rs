//! Bluetooth list (Config-bluetooth): switch, paired and available devices.
use super::*;
use crate::settings::bluetooth::DeviceKind;

impl View<'_, '_, '_> {
    pub(super) fn bluetooth(&mut self, state: &Settings) -> Result<(), String> {
        let on = state.value(Id::BtSwitch) == "1";
        let mut positions = vec![Placement {
            top: VIEW_TOP,
            height: 40.,
            section: None,
            searching: None,
        }];
        // Divider under the switch: 4 px margins around a 1 px rule.
        let mut top = VIEW_TOP + 40. + ROW_GAP + 9.;
        let devices = if on { state.bt.as_slice() } else { &[] };
        // Paired devices come first; the pairing hint row closes the list.
        let rows = devices.len() + usize::from(on);
        for n in 0..rows {
            let paired = |i: usize| devices.get(i).is_some_and(|d| d.paired);
            let section = match devices.get(n) {
                Some(d) if d.paired && n == 0 => Some("settings.bt_paired"),
                _ if !paired(n) && (n == 0 || paired(n - 1)) => Some("settings.bt_available"),
                _ => None,
            };
            top += if section.is_some() {
                SECTION_HEIGHT
            } else {
                0.
            };
            positions.push(Placement {
                top,
                height: 40.,
                section,
                searching: None,
            });
            top += 40. + ROW_GAP;
        }
        self.list(
            state,
            &positions,
            RIGHT,
            |view, n, y, place, selected| match n.checked_sub(1) {
                None => view.switch(
                    state,
                    Id::BtSwitch,
                    "settings.bluetooth",
                    (y, place.height),
                    selected,
                ),
                Some(i) => view.bt_row(state, devices.get(i), (y, place.height), selected),
            },
        )
    }
    fn bt_row(
        &mut self,
        state: &Settings,
        device: Option<&crate::settings::bluetooth::Device>,
        (y, h): (f32, f32),
        selected: bool,
    ) -> Result<(), String> {
        let ink = self.ink(selected);
        let face = selection(selected, Face::Regular, Face::Semibold);
        let muted = selection(selected, self.theme.muted, self.theme.accent_text_secondary);
        let Some(device) = device else {
            if state.searching() {
                self.searching("settings.scan_bt", y + 3., 0.)?;
            }
            return self.text(
                (28., y + h - 19., 560., 18.),
                &self.tr("settings.bt_hint"),
                (12., muted, Align::Left),
            );
        };
        let kind = match device.kind {
            DeviceKind::Controller => Some("settings.bt_controller"),
            DeviceKind::Audio => Some("settings.bt_audio"),
            DeviceKind::Other => None,
        };
        let state_text = match (device.paired, device.connected) {
            (false, _) => None,
            (_, true) => Some("settings.bt_connected"),
            _ => Some("settings.bt_disconnected"),
        };
        let detail: Vec<String> = kind
            .into_iter()
            .chain(state_text)
            .map(|k| self.tr(k))
            .collect();
        let detail = detail.join(" · ");
        let title = if detail.is_empty() { h } else { h - 18. };
        self.ui.label_aligned(
            &device.name,
            (face, 15., ink),
            self.rect((
                28.,
                y + if detail.is_empty() { 0. } else { 3. },
                480.,
                title,
            )),
            Align::Left,
        )?;
        if !detail.is_empty() {
            self.text(
                (28., y + title - 1., 480., 18.),
                &detail,
                (12., muted, Align::Left),
            )?;
        }
        let side = selection(
            selected,
            self.theme.text_secondary,
            self.theme.highlight_text,
        );
        let icon = if device.connected {
            Glyph::Check
        } else {
            Glyph::ChevronRight
        };
        self.ui
            .icon(icon, self.rect((596., y + (h - 14.) / 2., 14., 14.)), side)?;
        if !device.paired {
            self.text(
                (500., y, 92., h),
                &self.tr("settings.bt_pair"),
                (14., side, Align::Right),
            )?;
        }
        Ok(())
    }
}
