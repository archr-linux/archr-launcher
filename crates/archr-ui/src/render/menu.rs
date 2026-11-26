//! The game menu and the quick menu as a context card (docs/temas.md,
//! `[menu]`, `style = "context"`): a card of glass beside the focused item,
//! the game's name and "System · year" on top, Play, Favorite and Details
//! side by side, then the rows with their icons, values and arrows, the
//! power actions under a section title and the destructive ones in red.
//! The items, their order and what they do are the menu's own.
use super::{
    quick_key, system_title, Align, App, DataSource, Face, Frame, Glyph, Rect, Rgba, Screen, Theme,
    Ui,
};
use crate::state::QuickItem;
use crate::theme::Glass;

fn alpha(c: Rgba, a: u8) -> Rgba {
    Rgba { a, ..c }
}

/// The icon at a row's left.
fn glyph(item: &QuickItem) -> Glyph {
    match item {
        QuickItem::Play => Glyph::Play,
        QuickItem::Favorite => Glyph::Star,
        QuickItem::Details => Glyph::Info,
        QuickItem::Saves => Glyph::Storage,
        QuickItem::Shots => Glyph::Image,
        QuickItem::Achievements => Glyph::Trophy,
        QuickItem::Settings => Glyph::Gear,
        QuickItem::Netplay => Glyph::Globe,
        QuickItem::Core => Glyph::Cpu,
        QuickItem::Mangohud => Glyph::Gpu,
        QuickItem::Npu => Glyph::Image,
        QuickItem::Compat => Glyph::Puzzle,
        QuickItem::Move => Glyph::ArrowUpDown,
        QuickItem::RestoreOrder => Glyph::List,
        QuickItem::Hide => Glyph::EyeOff,
        QuickItem::ShowHidden => Glyph::Eye,
        QuickItem::ForgetRecent => Glyph::Trash,
        QuickItem::Search => Glyph::Search,
        QuickItem::Folders => Glyph::Folder,
        QuickItem::FavoritesFirst => Glyph::Star,
        QuickItem::Suspend => Glyph::Moon,
        QuickItem::Reboot => Glyph::Restart,
        QuickItem::Shutdown => Glyph::Power,
        QuickItem::Module(_) => Glyph::Puzzle,
        QuickItem::Delete(_) => Glyph::Trash,
    }
}
/// Rows that open another screen: an arrow at their right.
fn opens(item: &QuickItem) -> bool {
    matches!(
        item,
        QuickItem::Details
            | QuickItem::Saves
            | QuickItem::Shots
            | QuickItem::Achievements
            | QuickItem::Settings
            | QuickItem::Netplay
            | QuickItem::Search
    )
}
/// The section a row opens, by its title.
fn section(item: &QuickItem) -> Option<&'static str> {
    match item {
        QuickItem::Suspend | QuickItem::Reboot | QuickItem::Shutdown => {
            Some("menu.section_console")
        }
        QuickItem::Search | QuickItem::Folders | QuickItem::FavoritesFirst => {
            Some("menu.section_list")
        }
        _ => None,
    }
}

/// Where a card `w`×`h` stands: beside `focus` (right, else left), else
/// under or over it, else in the middle; always `m` inside the screen.
pub(super) fn anchor(
    focus: Option<Rect>,
    (w, h): (i32, i32),
    (sw, sh): (i32, i32),
    m: i32,
) -> (i32, i32) {
    let middle = ((sw - w) / 2, (sh - h) / 2);
    let Some(f) = focus else {
        return middle;
    };
    let clamp_y = |y: i32| y.clamp(m, (sh - m - h).max(m));
    let clamp_x = |x: i32| x.clamp(m, (sw - m - w).max(m));
    if f.right() + m + w <= sw - m {
        (f.right() + m, clamp_y(f.y()))
    } else if f.x() - m - w >= m {
        (f.x() - m - w, clamp_y(f.y()))
    } else if f.bottom() + m + h <= sh - m {
        (clamp_x(f.right() - w), f.bottom() + m)
    } else if f.y() - m - h >= m {
        (clamp_x(f.right() - w), f.y() - m - h)
    } else {
        // No room around it: at the side away from it, level with it.
        let x = if f.center().x() < sw / 2 {
            sw - m - w
        } else {
            m
        };
        (clamp_x(x), clamp_y(f.y()))
    }
}

impl Ui<'_, '_> {
    pub(super) fn menu_card(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let style = theme.screens.menu;
        let Frame { width, height, .. } = self.frame;
        // The button hints stay at the bottom: the card stops above them.
        let footer = self.frame.size(theme.footer_height).round().max(0.) as u32;
        let height = height.saturating_sub(footer).max(1);
        let px = |ui: &Self, n: f32| ui.px(n);
        let items = app.quick_menu_items(data);
        let quick = app.quick_row(data);
        let game = match &app.screen {
            Screen::QuickMenu { game: Some(game) } => Some(game.clone()),
            _ => None,
        };
        let (title, subtitle) = match &game {
            Some((system, n)) => {
                let found = data.games(system).get(*n);
                let mut line = system_title(system, data, &self.catalog);
                if let Some(year) = found.and_then(|g| g.year) {
                    line = format!("{line} · {year}");
                }
                (
                    found.map_or_else(|| self.catalog.text("menu.title"), |g| g.name.clone()),
                    line,
                )
            }
            None => (
                self.catalog.text("menu.title"),
                app.list_system().map_or_else(
                    || self.catalog.text("systems.title"),
                    |system| system_title(system, data, &self.catalog),
                ),
            ),
        };
        // The card's size: header, quick actions, rows and section titles.
        let margin = px(self, 16.);
        // The theme's box: the card as wide as it, as tall as it at most.
        let boxed = style.rect.map(|r| self.area(r));
        let w = boxed
            .map_or(px(self, style.width * 480.), |b| b.width() as i32)
            .min(width as i32 - 2 * margin)
            .max(1);
        let pad = px(self, 16.);
        let row = px(self, style.row * 480.);
        let head = px(self, 58.);
        let quick_h = if quick > 0 { px(self, 66.) } else { 0 };
        let rows = &items[quick..];
        let titled = |n: usize| {
            let s = section(&rows[n]);
            s.is_some() && (n == 0 || section(&rows[n - 1]) != s)
        };
        let title_h = px(self, 24.);
        let mut offsets = Vec::with_capacity(rows.len());
        let mut y = 0;
        for n in 0..rows.len() {
            if titled(n) {
                y += title_h;
            }
            offsets.push(y);
            y += row;
        }
        let list_h = y;
        let most =
            boxed.map_or(height as i32 - 2 * margin, |b| b.height() as i32) - head - quick_h - pad;
        let shown = list_h.min(most.max(row));
        let h = head + quick_h + shown + pad;
        let (x, top) = if let Some(b) = boxed {
            (b.x(), b.y())
        } else if style.anchor_focus {
            anchor(self.focus_at, (w, h), (width as i32, height as i32), margin)
        } else {
            anchor(None, (w, h), (width as i32, height as i32), margin)
        };
        let card = Rect::new(x, top, w as u32, h as u32);
        let radius = px(self, style.radius * 480.) as f32;
        self.cast_shadow(card, radius)?;
        // Over a dimmed screen: the theme's surface under the glass.
        self.rounded(card, radius, alpha(theme.surface, 214), None)?;
        let pane = Glass {
            blur: 0.,
            ..theme.glass_or_default()
        };
        self.glass(card, radius, &pane)?;
        let inner = (w - 2 * pad).max(1) as u32;
        let left = x + pad;
        self.label(
            &title,
            (Face::Bold, 17., theme.text),
            Rect::new(left, top + px(self, 12.), inner, px(self, 24.) as u32),
        )?;
        self.label(
            &subtitle,
            (Face::Regular, 12., theme.text_secondary),
            Rect::new(left, top + px(self, 35.), inner, px(self, 17.) as u32),
        )?;
        let favorite = game
            .as_ref()
            .and_then(|(s, n)| data.games(s).get(*n))
            .is_some_and(|g| g.favorite);
        // Play, Favorite and Details side by side; Play light, the focused
        // one ringed.
        if quick > 0 {
            let gap = px(self, 8.);
            let bh = px(self, 56.);
            let bw = (inner as i32 - gap * (quick as i32 - 1)) / quick as i32;
            let by = top + head;
            for (n, item) in items[..quick].iter().enumerate() {
                let rect = Rect::new(
                    left + n as i32 * (bw + gap),
                    by,
                    bw.max(1) as u32,
                    bh as u32,
                );
                let r = px(self, 14.) as f32;
                let play = *item == QuickItem::Play;
                let ink = if play {
                    self.rounded(rect, r, theme.text, None)?;
                    theme.background
                } else {
                    self.rounded(rect, r, alpha(theme.text, 0x1C), None)?;
                    theme.text
                };
                if app.cursor == n {
                    // Just outside the button, so it shows on the light one.
                    let out = px(self, 3.);
                    let ring = Rect::new(
                        rect.x() - out,
                        rect.y() - out,
                        rect.width() + 2 * out as u32,
                        rect.height() + 2 * out as u32,
                    );
                    self.focus_ring(ring, r + out as f32, theme)?;
                }
                let side = px(self, 18.);
                let icon = Rect::new(
                    rect.center().x() - side / 2,
                    rect.y() + px(self, 10.),
                    side as u32,
                    side as u32,
                );
                let ink_icon = if *item == QuickItem::Favorite && favorite {
                    theme.warning
                } else {
                    ink
                };
                self.icon(glyph(item), icon, ink_icon)?;
                let label = self.catalog.text(quick_key(item));
                let at = Rect::new(
                    rect.x() + 2,
                    rect.y() + px(self, 32.),
                    rect.width() - 4,
                    px(self, 18.) as u32,
                );
                let size = self
                    .shrink(&label, Face::Semibold, 12., 9., at.width())?
                    .unwrap_or(9.);
                self.label_aligned(&label, (Face::Semibold, size, ink), at, Align::Center)?;
            }
        }
        // The rows, scrolled so the focused one shows.
        let list_top = top + head + quick_h;
        let cursor = app.cursor.checked_sub(quick);
        let scroll = match cursor {
            Some(c) if list_h > shown => {
                let end = offsets[c] + row;
                (end - shown).clamp(0, list_h - shown)
            }
            _ => 0,
        };
        let clip = Rect::new(x, list_top, w as u32, shown.max(1) as u32);
        let saved = self.canvas.clip_rect();
        self.canvas.set_clip_rect(clip);
        for (n, item) in rows.iter().enumerate() {
            let y = list_top + offsets[n] - scroll;
            if y + row < list_top || y > list_top + shown {
                continue;
            }
            if titled(n) {
                let key = section(item).unwrap_or_default();
                self.caps_label(
                    &self.catalog.text(key),
                    (Face::Semibold, 10., theme.muted),
                    Rect::new(
                        left + px(self, 4.),
                        y - title_h + px(self, 6.),
                        inner,
                        px(self, 16.) as u32,
                    ),
                    Align::Left,
                )?;
            }
            let focused = cursor == Some(n);
            let band = Rect::new(
                x + px(self, 6.),
                y,
                (w - 2 * px(self, 6.)).max(1) as u32,
                row.max(1) as u32,
            );
            if focused {
                self.rounded(band, px(self, 12.) as f32, alpha(theme.text, 0x24), None)?;
            }
            let danger = matches!(
                item,
                QuickItem::Shutdown | QuickItem::ForgetRecent | QuickItem::Delete(_)
            );
            let ink = if danger { theme.danger } else { theme.text };
            let side = px(self, 17.);
            let icon = Rect::new(left, y + (row - side) / 2, side as u32, side as u32);
            self.icon(glyph(item), icon, ink)?;
            let label = match item {
                QuickItem::Module(m) => app
                    .quick_module(*m, data)
                    .map(|(_, q)| q.label.clone())
                    .unwrap_or_default(),
                _ => self.catalog.text(quick_key(item)),
            };
            let face = if focused {
                Face::Semibold
            } else {
                Face::Regular
            };
            let end = band.right() - px(self, 10.);
            let mid = (y, row as u32);
            let mut right = end;
            let dim = theme.text_secondary;
            if opens(item) {
                right = self.packed_label(
                    "›",
                    (Face::Regular, 17., dim),
                    (right, mid.0, mid.1),
                    u32::MAX,
                    Align::Left,
                )? - px(self, 8.);
            }
            let switch = match item {
                QuickItem::Folders => {
                    Some(app.list_system().is_some_and(|s| data.shows_folders(s)))
                }
                QuickItem::FavoritesFirst => Some(data.favorites_first()),
                _ => None,
            };
            if let Some(on) = switch {
                let (sw, sh) = (px(self, 34.), px(self, 20.));
                let track = Rect::new(right - sw, y + (row - sh) / 2, sw as u32, sh as u32);
                let fill = if on {
                    theme.success
                } else {
                    alpha(theme.text, 0x30)
                };
                self.rounded(track, sh as f32 / 2., fill, None)?;
                let knob = sh - px(self, 4.);
                let kx = if on {
                    track.right() - px(self, 2.) - knob
                } else {
                    track.x() + px(self, 2.)
                };
                self.rounded(
                    Rect::new(kx, track.y() + px(self, 2.), knob as u32, knob as u32),
                    knob as f32 / 2.,
                    theme.text,
                    None,
                )?;
                right = track.x() - px(self, 8.);
            }
            let value = match item {
                QuickItem::Saves | QuickItem::Shots => {
                    self.progress_value(app, *item).map(|(_, short)| short)
                }
                QuickItem::Core => Some(app.core_label(&self.catalog)),
                QuickItem::Mangohud => Some(self.catalog.text(match app.selected_mangohud().0 {
                    "" => "menu.as_system",
                    "1" => "settings.on",
                    _ => "settings.off",
                })),
                QuickItem::Npu => Some(self.catalog.text(match app.selected_npu().0 {
                    "" => "menu.as_system",
                    value => crate::settings::npu_label(value),
                })),
                QuickItem::Compat => Some(self.catalog.text(super::compat_texts(app).0)),
                _ => None,
            };
            let tx = icon.right() + px(self, 12.);
            if let Some(value) = value {
                // Half the row at least; more when the label leaves it free.
                let room = right - tx - self.measure(&label, face, 14.)? as i32 - px(self, 16.);
                let cap = room.max((inner / 2) as i32) as u32;
                right = self.packed_label(
                    &value,
                    (Face::Regular, 13., dim),
                    (right, mid.0, mid.1),
                    cap,
                    Align::Right,
                )? - px(self, 8.);
            }
            let tw = (right - tx).max(1) as u32;
            self.label(&label, (face, 14., ink), Rect::new(tx, y, tw, row as u32))?;
        }
        self.canvas.set_clip_rect(saved);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::anchor;
    use sdl2::rect::Rect;

    #[test]
    fn the_card_stands_beside_the_focus_and_inside_the_screen() {
        let screen = (640, 480);
        // A cover on the left: the card at its right, level with it.
        let at = anchor(Some(Rect::new(40, 100, 140, 180)), (300, 250), screen, 16);
        assert_eq!(at, (196, 100));
        // A cover on the right: at its left.
        let at = anchor(Some(Rect::new(460, 300, 140, 160)), (300, 250), screen, 16);
        assert_eq!(at, (144, 214), "moved up to stay on the screen");
        // A whole-width row: under it, right-aligned.
        let at = anchor(Some(Rect::new(16, 80, 608, 40)), (300, 250), screen, 16);
        assert_eq!(at, (324, 136));
        assert_eq!(anchor(None, (300, 250), screen, 16), (170, 115));
        // A cell in the middle of a grid: at the far side, beside it.
        let at = anchor(Some(Rect::new(188, 150, 132, 110)), (300, 300), screen, 16);
        assert_eq!(at, (324, 150));
    }
}
