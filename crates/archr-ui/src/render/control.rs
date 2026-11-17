//! The Start menu as a control center (docs/temas.md, `[start]`,
//! `style = "control"`): the hour large at the top left, the network and
//! the battery at the right, blocks of glass for Recent, Games and Tools
//! (and the modules' tiles), the shortcuts in circles and the focused
//! item's caption, over the screen it opened on, blurred. The items and the
//! moves are the tiles' own.
use super::hub::{badge, pending, radio_caption, TILES};
use super::{library_summary, Align, App, DataSource, Face, Glyph, Rect, Theme, Ui};
use crate::shortcuts::{Builtin, Shortcut};
use crate::theme::Glass;

/// Left and right margin, and the top of the blocks, in reference pixels.
const MARGIN: f32 = 32.;
const TOP: f32 = 118.;
const GAP: f32 = 14.;

impl Ui<'_, '_> {
    pub(super) fn hub_control(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        (row, tile, shortcut): (usize, usize, usize),
    ) -> Result<(), String> {
        let style = theme.screens.start;
        let scale = self.frame.scale;
        let px = |n: f32| (n * scale).round() as i32;
        // The composition is laid out from the top, 480 reference pixels
        // high and as wide as staged; the theme may place its parts.
        let wide = self.frame.width as f32 / scale;
        let room = wide - 2. * MARGIN;
        // A theme box in reference pixels: (x, y, w, h).
        let place = |ui: &Self, rect: crate::theme::Rect01| {
            let r = ui.area(rect);
            (
                r.x() as f32 / scale,
                r.y() as f32 / scale,
                r.width() as f32 / scale,
                r.height() as f32 / scale,
            )
        };
        // The screen behind is blurred already: the panes only tint it.
        let pane = Glass {
            blur: 0.,
            ..theme.glass_or_default()
        };
        let clock = style
            .clock_at
            .map_or((MARGIN, 24., wide - 2. * MARGIN), |r| {
                let (x, y, w, _) = place(self, r);
                (x, y, w)
            });
        self.control_status(theme, clock, style.clock * 480.)?;

        let widgets = app.modules.widgets();
        let row = if row == 2 && widgets.is_empty() {
            0
        } else {
            row
        };
        let modules = app.modules.tiles();
        let count = TILES.len() + modules.len();
        // Three blocks across their box, as tall as it.
        let (left, top, across, tile_h) = match style.tiles {
            Some(r) => place(self, r),
            None => (
                MARGIN,
                TOP,
                room,
                if widgets.is_empty() { 150. } else { 108. },
            ),
        };
        let tile_w = ((across - 2. * GAP) / 3.).min(if style.tiles.is_some() {
            f32::MAX
        } else {
            184.
        });
        let step = tile_w + GAP;
        let span = count as f32 * step - GAP;
        // More blocks than fit: the row slides to keep the focus in.
        let first = if span <= across {
            left + (across - span) / 2.
        } else {
            (left + across / 2. - tile_w / 2. - tile as f32 * step)
                .clamp(left + across - span, left)
        };
        let radius = px(style.radius * 480.) as f32;
        for index in 0..count {
            let (key, glyph) = TILES.get(index).copied().unwrap_or(("", Glyph::Puzzle));
            let module = index.checked_sub(TILES.len()).map(|n| modules[n]);
            let focus = row == 0 && tile == index;
            let enabled = app.hub_tile_enabled(data, index);
            let x = first + index as f32 * step;
            let mut rect = Rect::new(px(x), px(top), px(tile_w) as u32, px(tile_h) as u32);
            let ink = if focus {
                let grow = self.lifting(("start", index));
                rect = self.lift_focus(rect, radius, grow, theme)?;
                self.rounded(rect, radius, theme.text, None)?;
                theme.background
            } else {
                self.glass(rect, radius, &pane)?;
                if enabled {
                    theme.text
                } else {
                    theme.muted
                }
            };
            let pad = px(18.);
            let side = px(30.);
            let icon = Rect::new(rect.x() + pad, rect.y() + pad, side as u32, side as u32);
            let label = match module {
                Some((n, entry)) => {
                    self.module_icon((entry.icon.as_deref(), ""), icon, ink)?;
                    let id = &app.modules.modules[n].id;
                    if let Some(value) = app.modules.value(id, &format!("tile:{}", entry.id)) {
                        let value = value.to_owned();
                        let at = Rect::new(
                            rect.x() + pad,
                            rect.bottom() - pad - px(44.),
                            rect.width().saturating_sub(2 * pad as u32).max(1),
                            px(18.) as u32,
                        );
                        self.label(&value, (Face::Regular, 13., ink), at)?;
                    }
                    entry.label.clone()
                }
                None => {
                    self.icon(glyph, icon, ink)?;
                    self.catalog.text(key)
                }
            };
            let at = Rect::new(
                rect.x() + pad,
                rect.bottom() - pad - px(22.),
                rect.width().saturating_sub(2 * pad as u32).max(1),
                px(22.) as u32,
            );
            let size = self
                .shrink(&label, Face::Semibold, 17., 12., at.width())?
                .unwrap_or(12.);
            self.label(&label, (Face::Semibold, size, ink), at)?;
        }

        let mut y = top + tile_h + 26.;
        if !widgets.is_empty() {
            // The modules' widgets in a band under the blocks, in the
            // tiles' 640-wide coordinates.
            let band = (320. - room / 2., top + tile_h + 14., room, 64.);
            self.hub_widgets(app, theme, row, &widgets, Some(band))?;
            y = band.1 + band.3 + 16.;
        }
        let shortcuts = app.hub_shortcuts();
        let n = shortcuts.len().max(1) as f32;
        // In a row centered in their box (and level in its height), or
        // under the blocks.
        let (left, across, tall) = match style.shortcuts {
            Some(r) => {
                let (x, top, w, h) = place(self, r);
                y = top;
                (x, w, h)
            }
            None => (MARGIN, room, f32::MAX),
        };
        let side = (style.shortcut * 480.)
            .min((across - GAP * (n - 1.)) / n)
            .min(tall);
        if tall < f32::MAX {
            y += (tall - side) / 2.;
        }
        let start = left + (across - side * n - GAP * (n - 1.)) / 2.;
        let wifi_on = self.wifi_enabled();
        let bluetooth_on = self.bluetooth_enabled();
        for (index, item) in shortcuts.iter().enumerate() {
            let focus = row == 1 && shortcut == index;
            let active = (*item == Shortcut::Builtin(Builtin::Wifi) && wifi_on)
                || (*item == Shortcut::Builtin(Builtin::Bluetooth) && bluetooth_on);
            let x = start + index as f32 * (side + GAP);
            let rect = Rect::new(px(x), px(y), px(side) as u32, px(side) as u32);
            let round = rect.width() as f32 / 2.;
            let ink = if focus {
                self.cast_shadow(rect, round)?;
                self.rounded(rect, round, theme.text, None)?;
                theme.background
            } else if active {
                self.rounded(rect, round, theme.highlight_bg, None)?;
                theme.highlight_text
            } else {
                self.glass(rect, round, &pane)?;
                theme.text
            };
            // Powering off is the one to think twice about.
            let ink = if *item == Shortcut::Builtin(Builtin::Shutdown) {
                theme.danger
            } else {
                ink
            };
            let glyph = px(side * 0.46);
            let icon = Rect::new(
                rect.center().x() - glyph / 2,
                rect.center().y() - glyph / 2,
                glyph as u32,
                glyph as u32,
            );
            match item {
                Shortcut::Module(_, entry) => {
                    self.module_icon((entry.icon.as_deref(), ""), icon, ink)?
                }
                Shortcut::Builtin(Builtin::Wifi) if !wifi_on => {
                    self.icon(Glyph::WifiOff, icon, ink)?
                }
                Shortcut::Builtin(builtin) => self.icon(builtin.glyph(), icon, ink)?,
            }
            if let Some(dot) = badge(app, item, rect, scale) {
                self.rounded(dot, dot.height() as f32 / 2., theme.highlight_bg, None)?;
            }
        }

        // The focused item's name and what it holds or does.
        let (name, detail) = match row {
            0 => self.tile_caption(app, data, tile),
            1 => match shortcuts.get(shortcut) {
                Some(item) => self.shortcut_caption(app, item),
                None => Default::default(),
            },
            _ => widgets
                .get(app.hub_widget.min(widgets.len().saturating_sub(1)))
                .map(|(_, entry)| (entry.label.clone(), String::new()))
                .unwrap_or_default(),
        };
        let caption = y + side + 16.;
        let line = |top: f32, h: f32| {
            Rect::new(
                px(left.min(MARGIN)),
                px(top),
                px(across.max(room)) as u32,
                px(h) as u32,
            )
        };
        self.fitted(
            &name,
            (Face::Semibold, 16., theme.text),
            line(caption, 22.),
            Align::Center,
        )?;
        if !detail.is_empty() {
            self.fitted(
                &detail,
                (Face::Regular, 12., theme.text_secondary),
                line(caption + 23., 18.),
                Align::Center,
            )?;
        }
        Ok(())
    }

    /// The hour large at the top left with the name under it; the network
    /// and the battery in words at the right.
    fn control_status(
        &mut self,
        theme: &Theme,
        (left, top, wide): (f32, f32, f32),
        size: f32,
    ) -> Result<(), String> {
        let scale = self.frame.scale;
        let px = |n: f32| (n * scale).round() as i32;
        let Some((clock, battery, wifi, network)) = self
            .status
            .as_ref()
            .map(|s| (s.clock.clone(), s.battery, s.wifi, s.network.clone()))
        else {
            return Ok(());
        };
        let tall = size * 1.2;
        self.label(
            &clock,
            (Face::Regular, size, theme.text),
            Rect::new(px(left), px(top), px(wide / 2.) as u32, px(tall) as u32),
        )?;
        self.label(
            "Arch R",
            (Face::Semibold, 13., theme.text_secondary),
            Rect::new(
                px(left),
                px(top + tall - 2.),
                px(wide / 2.) as u32,
                px(18.) as u32,
            ),
        )?;
        let (y, h) = (px(top + 10.), px(20.) as u32);
        let mut right = px(left + wide);
        let text = (Face::Regular, 13., theme.text_secondary);
        let icon = px(16.);
        if let Some(battery) = battery.filter(|_| theme.screens.status.battery) {
            right = self.packed_label(
                &format!("{}%", battery.percent),
                text,
                (right, y, h),
                u32::MAX,
                Align::Right,
            )?;
            right -= px(6.) + icon;
            let glyph = Rect::new(right, y + (h as i32 - icon) / 2, icon as u32, icon as u32);
            self.icon(super::battery_glyph(battery), glyph, theme.text_secondary)?;
            right -= px(14.);
        }
        if theme.screens.status.radios && wifi.radio() {
            let joined = network
                .as_ref()
                .map(|(ssid, _)| ssid.clone())
                .filter(|ssid| !ssid.is_empty() && wifi == crate::status::Radio::Connected);
            let words = joined.unwrap_or_else(|| {
                self.catalog.text(if wifi == crate::status::Radio::Off {
                    "start.wifi_off"
                } else {
                    "start.wifi_on"
                })
            });
            let cap = px(wide / 3.) as u32;
            right = self.packed_label(&words, text, (right, y, h), cap, Align::Right)?;
            right -= px(6.) + icon;
            let glyph = Rect::new(right, y + (h as i32 - icon) / 2, icon as u32, icon as u32);
            self.icon(Glyph::Wifi, glyph, theme.text_secondary)?;
        }
        Ok(())
    }

    /// A block's name, and what it holds: the recent games, the library,
    /// the tools, a module's value.
    fn tile_caption(&self, app: &App, data: &dyn DataSource, tile: usize) -> (String, String) {
        let games = |n: usize| {
            self.catalog
                .plural("games.count", n as f64, &[("count", &n.to_string())])
        };
        match tile {
            0 => (
                self.catalog.text(TILES[0].0),
                games(data.collection_len("@recent")),
            ),
            1 => (
                self.catalog.text(TILES[1].0),
                library_summary(data, &self.catalog),
            ),
            2 => {
                let tools = data
                    .systems()
                    .iter()
                    .find(|s| s.name == "tools")
                    .map_or(0, |s| s.game_count as usize);
                (self.catalog.text(TILES[2].0), games(tools))
            }
            n => {
                let tiles = app.modules.tiles();
                let Some((module, entry)) = tiles.get(n - TILES.len()) else {
                    return Default::default();
                };
                let id = &app.modules.modules[*module].id;
                let value = app.modules.value(id, &format!("tile:{}", entry.id));
                (entry.label.clone(), value.unwrap_or_default().to_owned())
            }
        }
    }

    /// A shortcut's name, and its state: the network joined, the updates
    /// waiting, a module's value.
    fn shortcut_caption(&self, app: &App, item: &Shortcut<'_>) -> (String, String) {
        match *item {
            Shortcut::Module(n, entry) => {
                let value = app.modules.value(
                    &app.modules.modules[n].id,
                    &format!("shortcut:{}", entry.id),
                );
                (entry.label.clone(), value.unwrap_or_default().to_owned())
            }
            Shortcut::Builtin(builtin) => {
                let name = self.catalog.text(builtin.label());
                let count = pending(app, item);
                let radios = self
                    .status
                    .as_ref()
                    .map(|s| ((s.wifi, s.bluetooth), s.network.clone()));
                let detail = match radios {
                    Some((radios, network)) => {
                        radio_caption(&self.catalog, builtin, radios, network.as_ref())
                            .map(|(words, address)| format!("{words}{address}"))
                    }
                    None => None,
                };
                let detail = match (detail, builtin) {
                    (Some(detail), _) => detail,
                    (None, Builtin::Update) if count > 0 => self.catalog.plural(
                        "start.update_ready",
                        count as f64,
                        &[("count", &count.to_string())],
                    ),
                    (None, _) if count > 0 => self.catalog.plural(
                        "start.store_updates",
                        count as f64,
                        &[("name", &name), ("count", &count.to_string())],
                    ),
                    _ => String::new(),
                };
                (name, detail)
            }
        }
    }
}
