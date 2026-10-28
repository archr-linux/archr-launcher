//! Netplay lobby: rooms grouped by origin on the left, the selected room on the right.
use super::{selection, Align, Face, Glyph, Rect, Rgba, Theme, Ui};
use crate::netplay::{Group, Lobby, Rom, Room};

const ROW: f32 = 46.;
const HEADER: f32 = 22.;
const LIST: (f32, f32, f32, f32) = (16., 48., 378., 378.);

enum Item<'a> {
    Header(Group),
    Room(usize, &'a Room),
}

impl Ui<'_, '_> {
    fn at(&self, (x, y, w, h): (f32, f32, f32, f32)) -> Rect {
        Rect::new(self.px(x), self.px(y), self.px(w) as u32, self.px(h) as u32)
    }
    pub(super) fn lobby_page(&mut self, lobby: &Lobby, theme: &Theme) -> Result<(), String> {
        let right = self.status_header(theme)?;
        let crumb = format!("{} /", self.catalog.text("start.games"));
        let crumb_w = self.measure(&crumb, Face::Regular, 13.)?;
        self.label(
            &crumb,
            (Face::Regular, 13., theme.muted),
            Rect::new(self.px(24.), 0, crumb_w, self.px(44.) as u32),
        )?;
        let title_x = self.px(24.) + crumb_w as i32 + self.px(8.);
        let title = self.catalog.text("netplay.title");
        self.label(
            &title,
            (Face::Semibold, 15., theme.text),
            Rect::new(
                title_x,
                0,
                (right - title_x).max(1) as u32,
                self.px(44.) as u32,
            ),
        )?;
        let message = if lobby.rooms.is_empty() {
            Some(match (lobby.loading, lobby.failed) {
                (true, _) => "loading",
                (_, true) => "netplay.failed",
                _ => "netplay.empty",
            })
        } else {
            None
        };
        if let Some(key) = message {
            self.label(
                &self.catalog.text(key),
                (Face::Regular, 15., theme.muted),
                self.at((28., 60., 366., 24.)),
            )?;
        }
        self.rooms(lobby, theme)?;
        if let Some(room) = lobby.rooms.get(lobby.cursor) {
            self.room_panel(room, theme)?;
        }
        let footer = self.px(432.);
        self.divider(0, footer, self.frame.width, theme.border)?;
        let note = if lobby.loading && !lobby.rooms.is_empty() {
            Some("loading")
        } else {
            (lobby.failed && !lobby.rooms.is_empty()).then_some("netplay.failed")
        };
        if let Some(key) = note {
            self.label(
                &self.catalog.text(key),
                (Face::Regular, 13., theme.muted),
                self.at((24., 432., 300., 48.)),
            )?;
        }
        if let Some(selected) = lobby.menu {
            return self.join_menu(lobby, selected, theme);
        }
        let hints = [
            ('A', "hint.choose"),
            ('Y', "netplay.refresh"),
            ('B', "hint.back"),
        ];
        self.hints(&hints, theme, self.frame.width, footer, self.px(48.) as u32)
            .map(drop)
    }
    fn rooms(&mut self, lobby: &Lobby, theme: &Theme) -> Result<(), String> {
        let mut items = Vec::new();
        for (index, room) in lobby.rooms.iter().enumerate() {
            if index == 0 || lobby.rooms[index - 1].group != room.group {
                items.push(Item::Header(room.group));
            }
            items.push(Item::Room(index, room));
        }
        let height = |item: &Item| match item {
            Item::Header(_) => HEADER,
            Item::Room(..) => ROW + 2.,
        };
        // Scroll so the selected row ends inside the list; partly hidden rows are skipped.
        let mut y = 0.;
        let mut offset = 0.;
        for item in &items {
            y += height(item);
            if matches!(item, Item::Room(i, _) if *i == lobby.cursor) {
                offset = (y - LIST.3).max(0.);
            }
        }
        y = -offset;
        for item in &items {
            let h = height(item);
            if y >= 0. && y + h <= LIST.3 + 2. {
                let top = LIST.1 + y;
                match item {
                    Item::Header(group) => self.label(
                        &self.catalog.text(group.title()),
                        (Face::Regular, 11., theme.dim_text),
                        self.at((LIST.0 + 12., top + 6., LIST.2 - 24., 16.)),
                    )?,
                    Item::Room(index, room) => {
                        self.room_row(room, *index == lobby.cursor, (LIST.0, top), theme)?
                    }
                }
            }
            y += h;
        }
        Ok(())
    }
    fn room_row(
        &mut self,
        room: &Room,
        selected: bool,
        (x, y): (f32, f32),
        theme: &Theme,
    ) -> Result<(), String> {
        let rect = self.at((x, y, LIST.2, ROW));
        if selected {
            self.rounded(
                rect,
                theme.radius_item * self.frame.height as f32,
                theme.highlight_bg,
                None,
            )?;
        }
        let ink = selection(selected, theme.text, theme.highlight_text);
        let sub_ink = selection(selected, theme.muted, theme.accent_text_secondary);
        let status = self.catalog.text(room.rom.label());
        let status_ink = if selected {
            theme.highlight_text
        } else {
            rom_ink(room.rom, theme)
        };
        let status_x = self.packed_label(
            &status,
            (Face::Mono, 12., status_ink),
            (rect.right() - self.px(12.), rect.y(), rect.height()),
            u32::MAX,
            Align::Left,
        )?;
        let mut text_end = status_x;
        if room.locked {
            let icon = Rect::new(
                status_x - self.px(22.),
                rect.y() + self.px(16.),
                self.px(14.) as u32,
                self.px(14.) as u32,
            );
            self.icon(Glyph::Lock, icon, sub_ink)?;
            text_end = icon.x();
        }
        let text_w = (text_end - rect.x() - self.px(24.)).max(1) as u32;
        let face = selection(selected, Face::Regular, Face::Semibold);
        self.label(
            &room.game,
            (face, 15., ink),
            Rect::new(
                rect.x() + self.px(12.),
                rect.y() + self.px(5.),
                text_w,
                self.px(20.) as u32,
            ),
        )?;
        let sub = format!("{} · {}", room.host, room.place);
        self.label(
            &sub,
            (Face::Regular, 12., sub_ink),
            Rect::new(
                rect.x() + self.px(12.),
                rect.y() + self.px(25.),
                text_w,
                self.px(16.) as u32,
            ),
        )
    }
    fn room_panel(&mut self, room: &Room, theme: &Theme) -> Result<(), String> {
        let outer = self.at((410., 48., 210., 202.));
        self.rounded(
            outer,
            theme.radius_card * self.frame.height as f32,
            theme.surface,
            Some(theme.border),
        )?;
        self.label(
            &self.catalog.text("netplay.room"),
            (Face::Regular, 12., theme.muted),
            self.at((424., 62., 182., 16.)),
        )?;
        self.label(
            &room.game,
            (Face::Semibold, 16., theme.text),
            self.at((424., 85., 182., 22.)),
        )?;
        let relay = if room.relay.is_empty() {
            self.catalog.text("netplay.none")
        } else {
            room.relay.clone()
        };
        let latency = room
            .latency_ms
            .map_or("—".to_owned(), |ms| format!("{ms} ms"));
        let rows = [
            ("netplay.host", room.host.as_str(), Face::Regular),
            ("netplay.core", room.core.as_str(), Face::Regular),
            ("netplay.relay", relay.as_str(), Face::Regular),
            ("netplay.latency", latency.as_str(), Face::Mono),
        ];
        for (n, (key, value, face)) in rows.into_iter().enumerate() {
            let y = 114. + n as f32 * 23.;
            let label = self.catalog.text(key);
            let label_w = self.measure(&label, Face::Regular, 13.)?;
            self.label(
                &label,
                (Face::Regular, 13., theme.muted),
                self.at((424., y, 182., 16.)),
            )?;
            let room_for_value = (self.px(182.) as u32)
                .saturating_sub(label_w + self.px(10.) as u32)
                .max(1);
            self.packed_label(
                value,
                (face, 13., theme.text),
                (self.px(606.), self.px(y), self.px(16.) as u32),
                room_for_value,
                Align::Left,
            )?;
        }
        self.divider(
            self.px(424.),
            self.px(209.),
            self.px(182.) as u32,
            theme.border,
        )?;
        let ink = rom_ink(room.rom, theme);
        let mut x = 424.;
        if room.rom == Rom::Same {
            self.icon(Glyph::Check, self.at((x, 218., 14., 14.)), ink)?;
            x += 20.;
        }
        self.label(
            &self.catalog.text(room.rom.note()),
            (Face::Regular, 12., ink),
            self.at((x, 217., 606. - x, 16.)),
        )
    }
    fn join_menu(&mut self, lobby: &Lobby, selected: usize, theme: &Theme) -> Result<(), String> {
        self.scrim(theme, 1.2)?;
        let panel = self.modal(theme, (340., 32., 176., -self.px(20.)))?;
        let Some(room) = lobby.rooms.get(lobby.cursor) else {
            return Ok(());
        };
        self.label(
            &room.game,
            (Face::Semibold, 16., theme.text),
            self.frame.row(panel, 24., 16., 24.),
        )?;
        self.label(
            &room.host,
            (Face::Regular, 12., theme.muted),
            self.frame.row(panel, 24., 40., 16.),
        )?;
        for (n, key) in ["netplay.join", "netplay.watch"].into_iter().enumerate() {
            let row = self.frame.row(panel, 12., 72. + n as f32 * 44., 40.);
            let on = n == selected;
            if on {
                self.rounded(
                    row,
                    theme.radius_item * self.frame.height as f32,
                    theme.highlight_bg,
                    None,
                )?;
            }
            self.label(
                &self.catalog.text(key),
                (
                    selection(on, Face::Regular, Face::Semibold),
                    15.,
                    selection(on, theme.text, theme.highlight_text),
                ),
                Rect::new(
                    row.x() + self.px(12.),
                    row.y(),
                    row.width().saturating_sub(self.px(24.) as u32).max(1),
                    row.height(),
                ),
            )?;
        }
        let height = self.frame.height;
        let footer = (theme.footer_height * height as f32).round().max(1.) as u32;
        self.hints(
            &[('A', "hint.choose"), ('B', "hint.close")],
            theme,
            self.frame.width,
            height as i32 - footer as i32,
            footer,
        )
        .map(drop)
    }
}

fn rom_ink(rom: Rom, theme: &Theme) -> Rgba {
    match rom {
        Rom::Same => theme.success,
        Rom::Different | Rom::Unknown => theme.warning,
        Rom::NoRom | Rom::NoCore => theme.dim_text,
    }
}
