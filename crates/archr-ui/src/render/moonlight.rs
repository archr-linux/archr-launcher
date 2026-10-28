//! Moonlight screen follows the 640×480 two-column mockup.
use super::*;
use crate::moonlight::{Keyboard, Row, State, Status as HostStatus};

impl Ui<'_, '_> {
    fn moon_rect(&self, (x, y, w, h): (f32, f32, f32, f32)) -> Rect {
        Rect::new(self.px(x), self.px(y), self.px(w) as u32, self.px(h) as u32)
    }
    /// Non-game labels wrap at measured character boundaries, never ellipsize.
    fn moon_text(
        &mut self,
        text: &str,
        bounds: (f32, f32, f32, f32),
        size: f32,
        ink: Rgba,
    ) -> Result<(), String> {
        let rect = self.moon_rect(bounds);
        let mut size = size;
        let lines = loop {
            let lines = system_art::wrap(text, rect.width(), |text| {
                self.measure(text, Face::Regular, size)
            })?;
            if lines.len() as f32 * (size + 2.) <= bounds.3 || size <= 10. {
                break lines;
            }
            size -= 1.;
        };
        let top = bounds.1 + (bounds.3 - lines.len() as f32 * (size + 2.)) / 2.;
        for (i, line) in lines.iter().enumerate() {
            self.label(
                line,
                (Face::Regular, size, ink),
                self.moon_rect((bounds.0, top + i as f32 * (size + 2.), bounds.2, size + 2.)),
            )?;
        }
        Ok(())
    }
    pub(super) fn moonlight_page(&mut self, state: &State, theme: &Theme) -> Result<(), String> {
        let right = self.status_header(theme)? as f32 / self.frame.scale;
        self.moon_text(
            &format!("{} / Moonlight", self.catalog.text("start.games")),
            (24., 0., right - 32., 44.),
            15.,
            theme.text,
        )?;
        self.rounded(
            self.moon_rect((
                400.,
                48.,
                220.,
                if state.option.is_some() && state.address().is_some() {
                    340.
                } else {
                    292.
                },
            )),
            self.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        self.moon_text(
            &self.catalog.text("moonlight.computer"),
            (414., 58., 192., 20.),
            12.,
            theme.muted,
        )?;
        self.moon_text(
            state.address().unwrap_or("—"),
            (414., 80., 192., 42.),
            16.,
            theme.text,
        )?;
        self.moon_text(
            &format!(
                "● {}",
                self.catalog.text(if state.busy {
                    "moonlight.working"
                } else {
                    state.status.key()
                })
            ),
            (414., 122., 192., 34.),
            12.,
            if state.status == HostStatus::Online {
                theme.success
            } else {
                theme.muted
            },
        )?;
        self.divider(
            self.px(414.),
            self.px(161.),
            self.px(192.) as u32,
            theme.border,
        )?;
        let o = &state.options;
        let values = [
            format!("{}×{}", o.width, o.height),
            format!("{} fps", o.fps),
            format!("{} Mb/s", o.bitrate / 1000),
            self.catalog.text(o.codec.label()),
        ];
        for (i, key) in [
            "moonlight.resolution",
            "moonlight.fps",
            "moonlight.bitrate",
            "moonlight.codec",
        ]
        .into_iter()
        .enumerate()
        {
            let selected = state.option == Some(i);
            let y = 170. + i as f32 * 39.;
            if selected {
                self.rounded(
                    self.moon_rect((406., y, 208., 38.)),
                    self.px(7.) as f32,
                    theme.highlight_bg,
                    None,
                )?;
            }
            let ink = if selected {
                theme.highlight_text
            } else {
                theme.text
            };
            self.moon_text(
                &self.catalog.text(key),
                (414., y, 98., 38.),
                13.,
                if selected { ink } else { theme.muted },
            )?;
            self.label_aligned(
                &values[i],
                (Face::Regular, 13., ink),
                self.moon_rect((516., y, 90., 38.)),
                Align::Right,
            )?;
        }
        if state.option.is_some() && state.address().is_some() {
            if state.option == Some(4) {
                self.rounded(
                    self.moon_rect((406., 336., 208., 40.)),
                    self.px(7.) as f32,
                    theme.highlight_bg,
                    None,
                )?;
            }
            self.moon_text(
                &self.catalog.text("moonlight.unpair"),
                (414., 336., 192., 40.),
                13.,
                if state.option == Some(4) {
                    theme.highlight_text
                } else {
                    theme.text
                },
            )?;
        }
        let rows = state.rows();
        let empty = state.apps.is_empty() && !state.busy && state.status == HostStatus::Online;
        let visible = (if state.error.is_some() { 5 } else { 6 }) - usize::from(empty);
        let start = state.cursor.saturating_sub(visible - 1);
        let heading = if state.address().is_some() {
            state.address().unwrap_or_default().to_owned()
        } else {
            self.catalog.text("moonlight.computers")
        };
        self.moon_text(&heading, (28., 48., 344., 24.), 11., theme.dim_text)?;
        if empty {
            self.moon_text(
                &self.catalog.text("moonlight.empty"),
                (28., 76., 344., 60.),
                14.,
                theme.muted,
            )?;
        }
        let mut y = if empty { 142. } else { 76. };
        let mut others = false;
        for (index, row) in rows.iter().enumerate().skip(start).take(visible) {
            if matches!(row, Row::Host(_) | Row::Add) && !others {
                self.moon_text(
                    &self.catalog.text("moonlight.other_computers"),
                    (28., y, 344., 24.),
                    11.,
                    theme.dim_text,
                )?;
                y += 26.;
                others = true;
            }
            let selected = index == state.cursor && state.option.is_none();
            if selected {
                self.rounded(
                    self.moon_rect((16., y, 368., 46.)),
                    self.px(10.) as f32,
                    theme.highlight_bg,
                    None,
                )?;
            }
            let ink = if selected {
                theme.highlight_text
            } else {
                theme.text
            };
            let secondary = if selected {
                theme.accent_text_secondary
            } else {
                theme.muted
            };
            match row {
                Row::App(i) => {
                    // App/game names alone may use the common ellipsis behavior.
                    self.label(
                        &state.apps[*i],
                        (
                            if selected {
                                Face::Semibold
                            } else {
                                Face::Regular
                            },
                            15.,
                            ink,
                        ),
                        self.moon_rect((28., y + 3., 344., 22.)),
                    )?;
                    self.moon_text(
                        &self.catalog.text("moonlight.launch"),
                        (28., y + 25., 344., 18.),
                        12.,
                        secondary,
                    )?;
                }
                Row::Host(i) => self.moon_text(&state.hosts[*i], (28., y, 344., 46.), 15., ink)?,
                Row::Pair => self.moon_text(
                    &self.catalog.text("moonlight.pair"),
                    (28., y, 344., 46.),
                    15.,
                    ink,
                )?,
                Row::Add => {
                    self.moon_text(
                        &self.catalog.text("moonlight.add"),
                        (28., y + 2., 344., 22.),
                        15.,
                        ink,
                    )?;
                    self.moon_text(
                        &self.catalog.text("moonlight.add_detail"),
                        (28., y + 24., 344., 20.),
                        12.,
                        secondary,
                    )?;
                }
            }
            y += 48.;
        }
        if let Some(error) = state.error {
            self.moon_text(
                &self.catalog.text(error),
                (28., 370., 344., 56.),
                12.,
                theme.warning,
            )?;
        }
        self.divider(0, self.px(432.), self.frame.width, theme.border)?;
        if let Some(keyboard) = &state.keyboard {
            self.moonlight_keyboard(keyboard, state.error, theme)?;
        }
        if let Some(pin) = &state.pin {
            self.scrim(theme, 1.)?;
            self.rounded(
                self.moon_rect((72., 116., 496., 248.)),
                self.px(12.) as f32,
                theme.surface,
                Some(theme.border),
            )?;
            self.moon_text(
                &self.catalog.text("moonlight.pin"),
                (96., 132., 448., 56.),
                18.,
                theme.text,
            )?;
            self.label_aligned(
                pin,
                (Face::Mono, 64., theme.highlight_bg),
                self.moon_rect((96., 192., 448., 88.)),
                Align::Center,
            )?;
            self.moon_text(
                &self.catalog.text("moonlight.wait_pair"),
                (96., 286., 448., 56.),
                14.,
                theme.muted,
            )?;
        }
        if state.confirm_unpair {
            self.scrim(theme, 1.)?;
            self.rounded(
                self.moon_rect((72., 150., 496., 180.)),
                self.px(12.) as f32,
                theme.surface,
                Some(theme.border),
            )?;
            self.moon_text(
                &self.catalog.text("moonlight.unpair_question"),
                (96., 166., 448., 86.),
                18.,
                theme.text,
            )?;
        }
        let hints: Vec<(HintButton, &str)> = if state.busy {
            vec![('B'.into(), "hint.cancel")]
        } else if state.confirm_unpair {
            vec![
                ('A'.into(), "moonlight.unpair"),
                ('B'.into(), "hint.cancel"),
            ]
        } else if state.keyboard.is_some() {
            vec![
                ('A'.into(), "hint.choose"),
                ('Y'.into(), "moonlight.delete"),
                (HintButton::Start, "moonlight.connect"),
                ('B'.into(), "hint.cancel"),
            ]
        } else if state.option == Some(4) {
            vec![('A'.into(), "moonlight.unpair"), ('B'.into(), "hint.back")]
        } else if state.option.is_some() {
            vec![
                (HintButton::Pair('‹', '›'), "moonlight.change"),
                ('B'.into(), "hint.back"),
            ]
        } else {
            let action = match rows.get(state.cursor) {
                Some(Row::App(_)) => "moonlight.start",
                Some(Row::Pair) => "moonlight.pair_hint",
                _ => "hint.open",
            };
            vec![
                ('A'.into(), action),
                ('X'.into(), "moonlight.refresh"),
                ('Y'.into(), "moonlight.options"),
                ('B'.into(), "hint.back"),
            ]
        };
        self.hints(
            &hints,
            theme,
            self.frame.width,
            self.px(432.),
            self.px(48.) as u32,
        )?;
        Ok(())
    }
    fn moonlight_keyboard(
        &mut self,
        keyboard: &Keyboard,
        error: Option<&str>,
        theme: &Theme,
    ) -> Result<(), String> {
        self.scrim(theme, 1.)?;
        self.rounded(
            self.moon_rect((48., 78., 544., 290.)),
            self.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        self.moon_text(
            &self.catalog.text("moonlight.ip"),
            (68., 90., 504., 30.),
            18.,
            theme.text,
        )?;
        self.moon_text(
            &format!("{}_", keyboard.display()),
            (68., 124., 504., 34.),
            18.,
            theme.text,
        )?;
        let labels = (
            self.catalog.text("settings.space"),
            self.catalog.text("moonlight.connect"),
        );
        self.keyboard_keys(keyboard, 172., (&labels.0, &labels.1), theme)?;
        if let Some(error) = error {
            self.moon_text(
                &self.catalog.text(error),
                (68., 310., 504., 40.),
                13.,
                theme.text,
            )?;
        }
        Ok(())
    }
}
