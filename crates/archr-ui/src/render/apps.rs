//! An app's screens (API 4) in the theme's games and details layouts: the
//! module says what is there, these draw it like the launcher's own.
use super::details::{Button as SheetButton, Icon, Sheet};
use super::*;
use crate::apps::{Layout as Kind, Screen as AppScreen, Screens, Session};
use crate::modules::Tone;

/// The chips band above an app's list, in reference pixels.
const CHIPS: f32 = 30.;

impl Ui<'_, '_> {
    /// A tone's ink: theme colours only.
    pub(super) fn tone(&self, tone: Tone, theme: &Theme) -> Rgba {
        match tone {
            Tone::Plain => theme.text_secondary,
            Tone::Good => theme.success,
            Tone::Info => crate::appearance::mix(theme.highlight_bg, theme.text, 0.45),
            Tone::Bad => theme.danger,
            Tone::Muted => theme.muted,
        }
    }
    /// The USB card's tag in a tone's ink; returns its width.
    pub(super) fn tone_tag(
        &mut self,
        text: &str,
        ink: Rgba,
        at: (i32, i32),
    ) -> Result<i32, String> {
        self.tag(
            text,
            at,
            (Rgba { a: 36, ..ink }, Rgba { a: 96, ..ink }, ink),
        )
    }

    pub(super) fn app_page(
        &mut self,
        session: &Session,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let view = self.view(theme);
        let layout = view.games.clone();
        let Some(screen) = session.top() else {
            // Started, its first screen not here yet.
            self.begin_page(true, None, theme)?;
            self.backdrop_pictures(theme, None)?;
            // The app's name as the screen's title, as its screens will have it.
            let right = self.status_header(theme)?;
            let (pad, h) = (
                self.px(24.),
                self.frame.size(theme.header_height).round() as u32,
            );
            let title = Rect::new(pad, 0, (right - pad - self.px(10.)).max(1) as u32, h);
            self.label(&session.name, (Face::Semibold, 15., theme.text), title)?;
            let list = self.area(layout.list.rect);
            if session.waiting(self.frame.now) == Some(true) {
                let text = self
                    .catalog
                    .format("apps.silent", &[("name", &session.name)]);
                let lines = self.wrap(&text, Face::Regular, 15., list.width())?;
                let step = self.px(22.);
                let top = list.center().y() - step * lines.len() as i32 / 2;
                for (n, line) in lines.iter().enumerate() {
                    let at = Rect::new(list.x(), top + step * n as i32, list.width(), step as u32);
                    self.label_aligned(line, (Face::Regular, 15., theme.muted), at, Align::Center)?;
                }
            } else {
                // The busy pill, in the middle of the empty list.
                let text = self.catalog.text("loading");
                let w = self.measure(&text, Face::Regular, 13.)? as i32 + self.px(76.);
                let h = self.px(36.);
                let pill = Rect::new(
                    list.center().x() - w / 2,
                    list.center().y() - h / 2,
                    w.max(1) as u32,
                    h as u32,
                );
                let edge = Some(theme.border);
                self.rounded(pill, h as f32 / 2., theme.surface_elevated, edge)?;
                let pad = self.px(16.);
                self.scraping(
                    pill,
                    (&text, (Face::Regular, 13., theme.text)),
                    pad,
                    false,
                    theme,
                )?;
            }
            self.app_footer(session, None, theme)?;
            return self.end_page(theme);
        };
        let screens = Screens::new(data, session, screen);
        match screen.layout {
            Kind::Collection => {
                self.app_collection(session, screen, &screens, theme, layout, art)?
            }
            Kind::Detail => self.app_detail(session, screen, theme, layout, art)?,
        }
        let tall = self.frame.reference().1;
        if let Some(text) = &session.busy {
            self.staged((640., tall), |ui| ui.app_busy(text, theme))?;
        }
        if let Some(ask) = &session.ask {
            self.staged((640., tall), |ui| ui.app_ask(ask, theme))?;
        }
        if let Some(keyboard) = &session.keyboard {
            self.staged((640., tall), |ui| ui.app_keyboard(keyboard, theme))?;
        }
        Ok(())
    }

    fn app_collection(
        &mut self,
        session: &Session,
        screen: &AppScreen,
        data: &Screens<'_>,
        theme: &Theme,
        mut layout: Layout,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let key = session.key();
        let cursor = screen.cursor;
        // The chips (and the offline tag) take a band off the top of the list.
        let band = if screen.chips.is_empty() && session.offline.is_none() {
            None
        } else {
            let list = self.area(layout.list.rect);
            let h = self.px(CHIPS);
            let cut = h as f32 / self.frame.height as f32;
            layout.list.rect.y += cut;
            layout.list.rect.h = (layout.list.rect.h - cut).max(0.);
            Some(Rect::new(list.x(), list.y(), list.width(), h as u32))
        };
        let picture = layout
            .art
            .filter(|_| layout.list.mark != Mark::Tile)
            .and_then(|picture| {
                let game = data.games(&key).get(cursor)?;
                Some((game.path.display().to_string(), self.area(picture.rect)))
            });
        self.begin_page(true, picture, theme)?;
        self.backdrop_pictures(theme, None)?;
        self.decor(theme, &layout.decor, true)?;
        if let Some(wash) = layout.backdrop {
            self.backdrop(art.cover, wash, theme)?;
        }
        let games = Screen::Games {
            system: key.clone(),
        };
        self.header(&games, cursor, data, theme, &layout)?;
        if let Some(band) = band {
            self.app_chips(screen, session.offline.as_deref(), band, theme)?;
        }
        let loading = std::mem::replace(&mut self.loading, session.busy.is_some());
        let drawn = self.list(&games, (&[], false), cursor, data, theme, &layout);
        self.loading = loading;
        drawn?;
        self.cover(&key, cursor, data, theme, art.cover, &layout)?;
        self.app_footer(session, Some(screen), theme)?;
        self.decor(theme, &layout.decor, false)?;
        self.end_page(theme)
    }

    /// The filters in a row, the selected one lit; L and R move along it.
    fn app_chips(
        &mut self,
        screen: &AppScreen,
        offline: Option<&str>,
        band: Rect,
        theme: &Theme,
    ) -> Result<(), String> {
        let mut right = band.right();
        if let Some(text) = offline {
            let text = if text.is_empty() {
                self.catalog.text("apps.offline")
            } else {
                text.to_owned()
            };
            let text = text.as_str();
            let middle = band.y() + band.height() as i32 / 2;
            right -= self.tone_tag(text, theme.warning, (right, middle))? + self.px(8.);
        }
        let h = self.px(22.);
        let y = band.y() + (band.height() as i32 - h) / 2;
        let gap = self.px(6.);
        let mut widths = Vec::new();
        for chip in &screen.chips {
            widths.push(self.measure(&chip.label, Face::Semibold, 12.)? as i32 + self.px(20.));
        }
        let selected = screen.chips.iter().position(|c| c.selected).unwrap_or(0);
        let arrow = self.px(16.);
        let (first, end) = chip_window(&widths, gap, right - band.x(), selected, arrow);
        let (left, cut) = (first > 0, end < widths.len());
        let middle = y + (h - self.px(14.)) / 2;
        if left {
            let at = self.frame.square(band.x(), middle, 14.);
            self.icon(Glyph::ChevronLeft, at, theme.muted)?;
        }
        if cut {
            let at = self.frame.square(right - self.px(14.), middle, 14.);
            self.icon(Glyph::ChevronRight, at, theme.muted)?;
        }
        let mut x = band.x() + if left { arrow } else { 0 };
        for (chip, &w) in screen.chips.iter().zip(&widths).take(end).skip(first) {
            let rect = Rect::new(x, y, w as u32, h as u32);
            let (fill, edge, ink) = if chip.selected {
                (theme.highlight_bg, None, theme.highlight_text)
            } else {
                (theme.surface, Some(theme.border), theme.text_secondary)
            };
            self.rounded(rect, h as f32 / 2., fill, edge)?;
            let face = selection(chip.selected, Face::Regular, Face::Semibold);
            self.label_aligned(&chip.label, (face, 12., ink), rect, Align::Center)?;
            x += w + gap;
        }
        Ok(())
    }

    fn app_detail(
        &mut self,
        session: &Session,
        screen: &AppScreen,
        theme: &Theme,
        layout: Layout,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        self.begin_page(true, None, theme)?;
        self.backdrop_pictures(theme, None)?;
        self.decor(theme, &layout.decor, true)?;
        self.look.outlining = true;
        let crumbs = self.crumbs(&session.name, &screen.title, theme);
        self.look.outlining = false;
        crumbs?;
        let sheet = Sheet {
            title: &screen.title,
            subtitle: [&screen.subtitle]
                .into_iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect(),
            facts: screen
                .facts
                .iter()
                .map(|f| {
                    let ink = match f.tone {
                        Tone::Plain => theme.text,
                        tone => self.tone(tone, theme),
                    };
                    (f.title.clone(), f.value.clone(), ink)
                })
                .collect(),
            rating: None,
            text: screen.text.trim(),
            placeholder: session.name.clone(),
            buttons: screen
                .buttons
                .iter()
                .map(|b| SheetButton {
                    label: b.label.clone(),
                    icon: b.icon.as_deref().map(Icon::Svg),
                    busy: None,
                    working: false,
                    disabled: false,
                    progress: b.progress,
                })
                .collect(),
            focus: screen.focus,
            scroll: screen.scroll,
        };
        // Only the module's own picture: never a logo, cover or video.
        let art = Art {
            shot: art.shot,
            ..Art::default()
        };
        self.sheet(&sheet, theme, art)?;
        self.app_footer(session, Some(screen), theme)?;
        self.decor(theme, &layout.decor, false)?;
        self.end_page(theme)
    }

    fn app_footer(
        &mut self,
        session: &Session,
        screen: Option<&AppScreen>,
        theme: &Theme,
    ) -> Result<(), String> {
        let height = self.frame.height;
        let h = self.frame.size(theme.footer_height).round().max(1.) as u32;
        let y = height as i32 - h as i32;
        let text = |ui: &Self, key: &str| ui.catalog.text(key);
        let mut hints: Vec<(HintButton, String)> = Vec::new();
        match screen {
            Some(s) if s.layout == Kind::Collection => {
                if !s.items.is_empty() {
                    hints.push(('A'.into(), text(self, "hint.open")));
                }
                for (key, action) in [('X', &s.x), ('Y', &s.y)] {
                    if let Some(action) = action {
                        hints.push((key.into(), action.label.clone()));
                    }
                }
                hints.push((HintButton::Select, text(self, "apps.search")));
            }
            Some(s) => {
                if let Some(button) = s.buttons.get(s.focus) {
                    hints.push(('A'.into(), button.label.clone()));
                }
                if self.details_overflow > 0 {
                    hints.push(('↕'.into(), text(self, "details.scroll")));
                }
                if s.pictures() > 1 {
                    let n = format!("{}/{}", s.picture + 1, s.pictures());
                    let label = self.catalog.format("apps.pictures", &[("n", &n)]);
                    hints.push(('X'.into(), label));
                }
            }
            None => {}
        }
        if session.settings && screen.is_some() {
            hints.push((HintButton::Start, text(self, "apps.settings")));
        }
        hints.push(('B'.into(), text(self, "hint.back")));
        self.look.outlining = true;
        let drawn = self.app_hints(screen, &hints, theme, (y, h));
        self.look.outlining = false;
        drawn
    }
    fn app_hints(
        &mut self,
        screen: Option<&AppScreen>,
        hints: &[(HintButton, String)],
        theme: &Theme,
        (y, h): (i32, u32),
    ) -> Result<(), String> {
        let width = self.frame.width;
        let left = self.hint_labels(hints, theme, width, y, h)?;
        let note = screen.map_or("", |s| s.footer.as_str());
        if !note.is_empty() {
            let room = (left - self.px(40.)).max(1) as u32;
            self.label(
                note,
                (Face::Regular, 13., theme.muted),
                Rect::new(self.px(24.), y, room, h),
            )?;
        }
        Ok(())
    }

    /// `screen.busy`: the search's dots and the module's text, in a pill.
    fn app_busy(&mut self, text: &str, theme: &Theme) -> Result<(), String> {
        let tall = self.frame.reference().1;
        let w = (self.measure(text, Face::Regular, 13.)? as f32 / self.frame.scale + 76.).min(560.);
        let rect = Rect::new(
            self.px(320. - w / 2.),
            self.px(tall - 96.),
            self.px(w) as u32,
            self.px(36.) as u32,
        );
        self.rounded(
            rect,
            rect.height() as f32 / 2.,
            theme.surface_elevated,
            Some(theme.border),
        )?;
        let pad = self.px(16.);
        self.scraping(
            rect,
            (text, (Face::Regular, 13., theme.text)),
            pad,
            false,
            theme,
        )?;
        Ok(())
    }

    /// `screen.confirm`: the question and its two answers.
    fn app_ask(&mut self, ask: &crate::apps::Ask, theme: &Theme) -> Result<(), String> {
        self.scrim(theme, 1.2)?;
        let tall = self.frame.reference().1;
        // A removal is the launcher's question, in its own words.
        let text = if ask.launcher {
            self.catalog
                .format("apps.remove_question", &[("name", &ask.text)])
        } else {
            ask.text.clone()
        };
        let lines = self.wrap(&text, Face::Semibold, 16., self.px(392.) as u32)?;
        let lines = &lines[..lines.len().min(6)];
        let h = 40. + lines.len() as f32 * 22. + 72.;
        let top = (tall - h) / 2.;
        let panel = Rect::new(
            self.px(104.),
            self.px(top),
            self.px(432.) as u32,
            self.px(h) as u32,
        );
        self.rounded(
            panel,
            self.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        for (n, line) in lines.iter().enumerate() {
            let at = Rect::new(
                self.px(124.),
                self.px(top + 24. + n as f32 * 22.),
                self.px(392.) as u32,
                self.px(22.) as u32,
            );
            self.label(line, (Face::Semibold, 16., theme.text), at)?;
        }
        let row = top + 40. + lines.len() as f32 * 22.;
        let answer = |text: &str, key: &str| {
            if text.is_empty() {
                self.catalog.text(key)
            } else {
                text.to_owned()
            }
        };
        let (no, yes) = if ask.launcher {
            (
                self.catalog.text("hint.cancel"),
                self.catalog.text("apps.remove"),
            )
        } else {
            (
                answer(&ask.no, "hint.cancel"),
                answer(&ask.yes, "hint.confirm"),
            )
        };
        for (n, (label, on)) in [(&no, !ask.on_yes), (&yes, ask.on_yes)]
            .into_iter()
            .enumerate()
        {
            let rect = Rect::new(
                self.px(124. + n as f32 * 200.),
                self.px(row),
                self.px(192.) as u32,
                self.px(40.) as u32,
            );
            let (fill, edge, ink) = if on {
                (theme.highlight_bg, None, theme.highlight_text)
            } else {
                (theme.surface, Some(theme.border), theme.text)
            };
            self.rounded(rect, self.px(10.) as f32, fill, edge)?;
            let face = selection(on, Face::Regular, Face::Semibold);
            self.label_aligned(label, (face, 14., ink), rect, Align::Center)?;
        }
        Ok(())
    }

    /// SELECT's search: the launcher's keyboard over the app.
    fn app_keyboard(
        &mut self,
        keyboard: &crate::keyboard::Keyboard,
        theme: &Theme,
    ) -> Result<(), String> {
        self.scrim(theme, 1.)?;
        let at = |ui: &Self, (x, y, w, h): (f32, f32, f32, f32)| {
            Rect::new(ui.px(x), ui.px(y), ui.px(w) as u32, ui.px(h) as u32)
        };
        self.rounded(
            at(self, (48., 78., 544., 290.)),
            self.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        let title = self.catalog.text("apps.search");
        self.label(
            &title,
            (Face::Regular, 18., theme.text),
            at(self, (68., 90., 504., 30.)),
        )?;
        let text = format!("{}_", keyboard.display());
        self.label(
            &text,
            (Face::Regular, 18., theme.text),
            at(self, (68., 124., 504., 34.)),
        )?;
        let labels = (
            self.catalog.text("settings.space"),
            self.catalog.text("apps.search"),
        );
        self.keyboard_keys(keyboard, 172., (&labels.0, &labels.1), theme)
    }
}

/// The chips that fit in `room` (pixels), as `first..end`: the row scrolls
/// so the selected one shows, keeping `arrow` free on each side it cuts.
fn chip_window(widths: &[i32], gap: i32, room: i32, selected: usize, arrow: i32) -> (usize, usize) {
    let span =
        |range: std::ops::Range<usize>| widths[range].iter().map(|w| w + gap).sum::<i32>() - gap;
    if widths.is_empty() || span(0..widths.len()) <= room {
        return (0, widths.len());
    }
    let selected = selected.min(widths.len() - 1);
    // Room for the selected chip and those before it, behind the arrows.
    let mut first = 0;
    while first < selected
        && span(first..selected + 1) > room - arrow * (1 + usize::from(first > 0) as i32)
    {
        first += 1;
    }
    let inner = room - arrow * (usize::from(first > 0) as i32);
    let mut end = first + 1;
    while end < widths.len() {
        let rest = if end + 1 < widths.len() { arrow } else { 0 };
        if span(first..end + 1) > inner - rest {
            break;
        }
        end += 1;
    }
    (first, end)
}

#[cfg(test)]
mod tests {
    use super::chip_window;

    #[test]
    fn the_chips_scroll_to_the_selected_one() {
        // Eight chips, room for about five: All, GB, GBC, GBA, NES, Mega Drive, PICO-8, PortMaster.
        let widths = [40, 36, 44, 44, 44, 90, 66, 90];
        let (gap, room, arrow) = (6, 300, 16);
        let fits = |(first, end): (usize, usize)| {
            let used: i32 = widths[first..end].iter().map(|w| w + gap).sum::<i32>() - gap;
            let arrows = arrow * (i32::from(first > 0) + i32::from(end < widths.len()));
            used + arrows <= room
        };
        for selected in 0..widths.len() {
            let window = chip_window(&widths, gap, room, selected, arrow);
            assert!(
                window.0 <= selected && selected < window.1,
                "{selected}: {window:?}"
            );
            assert!(fits(window), "{selected}: {window:?}");
        }
        assert_eq!(chip_window(&widths, gap, room, 0, arrow).0, 0);
        assert!(
            chip_window(&widths, gap, room, 0, arrow).1 < widths.len(),
            "an arrow says more"
        );
        assert_eq!(
            chip_window(&widths, gap, room, 7, arrow).1,
            widths.len(),
            "the last shows"
        );
        assert_eq!(
            chip_window(&widths[..3], gap, room, 2, arrow),
            (0, 3),
            "all fit"
        );
    }
}
