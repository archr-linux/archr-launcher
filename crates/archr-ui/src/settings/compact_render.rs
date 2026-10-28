//! Compact 640×480 service, game and regional layouts from the screen mockups.
use super::*;
use crate::settings::{Id, Kind, Page, Settings};

impl View<'_, '_, '_> {
    pub(super) fn compact(&mut self, state: &Settings) -> Result<(), String> {
        let rows = state.visible_rows();
        let placements = state.compact_rows();
        let selected = placements.get(state.cursor);
        let offset = selected
            .map(|r| (r.top + r.height - 426.).max(0.))
            .unwrap_or(0.);
        for (index, (row, placement)) in rows.iter().zip(&placements).enumerate() {
            let y = placement.top - offset;
            if let Some(section) = placement.section {
                if y >= 77. && y + placement.height <= 434. {
                    self.note((20., y - 21., 406., 19.), &self.tr(section), 11.)?;
                }
            }
            if y < 55. || y + placement.height > 434. {
                continue;
            }
            let active = index == state.cursor;
            let disabled = state.page() == Page::Scraper || row.id == Id::IndexNow;
            let h = placement.height;
            self.panel((16., y, 410., h), active && !disabled)?;
            let ink = if disabled {
                self.theme.muted
            } else {
                self.ink(active)
            };
            let label = self.tr(row.label);
            let has_detail = state.page() == Page::Services;
            let title_h = if has_detail { 22. } else { h };
            let title_size = if h < 40. { 13. } else { 14. };
            let value_x = if row.kind == Kind::Toggle { 348. } else { 232. };
            let label_w = value_x - 35.;
            self.text(
                (28., y, label_w, title_h),
                &label,
                (title_size, ink, Align::Left),
            )?;
            if has_detail {
                if let Some(key) = row.subtitle {
                    self.text(
                        (28., y + 21., label_w, h - 21.),
                        &self.tr(key),
                        (
                            12.,
                            if active {
                                self.theme.accent_text_secondary
                            } else {
                                self.theme.muted
                            },
                            Align::Left,
                        ),
                    )?;
                }
            }
            let control = (value_x, y, 410. - value_x, h);
            if state.pending == Some(row.id) {
                self.text(
                    control,
                    &self.tr("settings.applying"),
                    (12., ink, Align::Right),
                )?;
            } else if disabled {
                self.text(
                    (value_x, y, 404. - value_x, h),
                    &self.tr("settings.soon"),
                    (12., ink, Align::Right),
                )?;
            } else {
                self.control(row.id, row.kind, state, active, control)?;
            }
        }
        self.panel((439., 56., 183., 270.), false)?;
        let (heading, detail) = match state.page() {
            Page::Services => (
                self.tr("settings.services"),
                if state.access_ip.is_empty() {
                    self.tr("settings.ssh_detail")
                } else {
                    format!(
                        "SSH: root@{}\nHTTP: http://{}",
                        state.access_ip, state.access_ip
                    )
                },
            ),
            Page::Netplay => (
                self.tr("settings.netplay"),
                self.tr("settings.netplay_detail"),
            ),
            Page::Region => (
                self.tr("settings.region"),
                self.tr("settings.region_detail"),
            ),
            _ => (self.tr("settings.scraper"), self.tr("settings.soon")),
        };
        self.label((452., 72., 155., 44.), &heading, 17.)?;
        self.note((452., 125., 155., 155.), &detail, 13.)?;
        Ok(())
    }
}
