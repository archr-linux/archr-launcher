//! The switcher (Alternador) and the resume at power on (Retomar-boot), in
//! theme rectangles (`switcher_*`, `resume_*`).
use super::*;
use crate::{primitives::crop, resume::Recent};

impl Ui<'_, '_> {
    /// The game's picture filling `rect` with round corners at `alpha`, or a
    /// plain box with the image icon.
    fn recent_picture(
        &mut self,
        game: &Recent,
        rect: Rect,
        (radius, alpha): (f32, u8),
        theme: &Theme,
        art: &Art<'_, '_>,
    ) -> Result<(), String> {
        let texture = game
            .picture
            .as_ref()
            .and_then(|path| art.pictures.and_then(|cache| cache.peek(path)));
        let radius = self.px(radius) as f32;
        if let Some(texture) = texture {
            let size = texture.query();
            let part = crop((size.width, size.height), (rect.width(), rect.height()));
            return self
                .geometry
                .cover(&mut self.canvas, texture, rect, radius, alpha, part);
        }
        let fade = |c: Rgba| Rgba {
            a: (u16::from(c.a) * u16::from(alpha) / 255) as u8,
            ..c
        };
        self.rounded(rect, radius, fade(theme.surface), Some(fade(theme.border)))?;
        let side = (rect.height() as f32 * 0.3).round().max(1.) as u32;
        let icon = Rect::new(
            rect.x() + (rect.width() - side) as i32 / 2,
            rect.y() + (rect.height() - side) as i32 / 2,
            side,
            side,
        );
        self.icon(Glyph::Image, icon, fade(theme.dim_text))
    }
    /// The soft shadow under a raised picture.
    fn lift(&mut self, rect: Rect, radius: f32) -> Result<(), String> {
        if self.shadow.is_none() {
            if let Some(creator) = self.creator {
                self.shadow = Some(Shadow::new(creator)?);
            }
        }
        match &mut self.shadow {
            Some(shadow) => {
                shadow.draw(&mut self.canvas, rect, self.frame.scale, radius, self.soft)
            }
            None => Ok(()),
        }
    }
    /// "Saved now" over the picture, or "No resume" for a game that starts
    /// as usual.
    fn saved_badge(&mut self, game: &Recent, at: (i32, i32), theme: &Theme) -> Result<(), String> {
        let (text, ink) = match (&game.entry, game.left) {
            (Some(_), Some(left)) => {
                let ago = self.catalog.relative_time(left, self.frame.unix);
                let text = self.catalog.format("switcher.saved", &[("ago", &ago)]);
                (text, theme.highlight_bg)
            }
            _ => (self.catalog.text("switcher.no_state"), theme.muted),
        };
        let w = self.measure(&text, Face::Regular, 12.)?;
        let h = self.px(22.) as u32;
        let rect = Rect::new(at.0, at.1, w + self.px(37.) as u32, h);
        let veil = Rgba {
            a: 209,
            ..theme.background
        };
        self.rounded(rect, self.px(10.) as f32, veil, None)?;
        let icon = self
            .frame
            .square(at.0 + self.px(9.), at.1 + self.px(4.5), 13.);
        self.icon(Glyph::Recent, icon, ink)?;
        self.label(
            &text,
            (Face::Regular, 12., theme.text),
            Rect::new(icon.right() + self.px(6.), at.1, w, h),
        )
    }

    pub(super) fn switcher(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        if theme.screens.switcher.depth {
            self.switcher_depth(app, data, theme, art)?;
            return self.switcher_footer(app, theme);
        }
        let Frame { width, height, .. } = self.frame;
        // The selected game behind everything, under the curtain chosen in
        // Appearance; a game without a picture keeps the background.
        if let Some(curtain) = theme.user.switcher_curtain {
            let at = app.cursor.min(app.switcher.len().saturating_sub(1));
            let game = app.switcher.get(at);
            let picture = game
                .and_then(|g| g.picture.as_ref())
                .and_then(|path| art.pictures.and_then(|cache| cache.peek(path)));
            if let (Some(game), Some(_)) = (game, picture) {
                let screen = Rect::new(0, 0, width, height);
                self.recent_picture(game, screen, (0., 255), theme, &art)?;
                let shade = Rgba {
                    a: (u16::from(curtain.min(100)) * 255 / 100) as u8,
                    ..theme.background
                };
                self.rounded(screen, 0., shade, None)?;
            }
        }
        let header = self.frame.size(theme.header_height).round() as u32;
        let right = self.status_header(theme)?;
        let title = self.catalog.text("switcher.title");
        let pad = self.px(24.);
        self.label(
            &title,
            (Face::Semibold, 15., theme.text),
            Rect::new(pad, 0, (right - pad).max(1) as u32, header),
        )?;
        let games = &app.switcher;
        let at = app.cursor.min(games.len().saturating_sub(1));
        let card = pixels(theme.switcher_card, width, height);
        let side = pixels(theme.switcher_side, width, height);
        let info = pixels(theme.switcher_info, width, height);
        if games.is_empty() {
            let text = self.catalog.text(if self.loading {
                "loading"
            } else {
                "switcher.empty"
            });
            self.label_aligned(
                &text,
                (Face::Regular, 15., theme.muted),
                card,
                Align::Center,
            )?;
        }
        // The neighbours, dimmed, `x` away from the selected picture.
        for (n, left) in [(at.checked_sub(1), true), (Some(at + 1), false)] {
            let Some(game) = n.and_then(|n| games.get(n)) else {
                continue;
            };
            let x = if left {
                card.x() - side.x() - side.width() as i32
            } else {
                card.right() + side.x()
            };
            let rect = Rect::new(x, side.y(), side.width(), side.height());
            self.recent_picture(game, rect, (10., 102), theme, &art)?;
        }
        if let Some(game) = games.get(at) {
            let ring = self.px(3.);
            let outer = Rect::new(
                card.x() - ring,
                card.y() - ring,
                card.width() + 2 * ring as u32,
                card.height() + 2 * ring as u32,
            );
            self.lift(outer, self.px(15.) as f32)?;
            self.rounded(outer, self.px(15.) as f32, theme.highlight_bg, None)?;
            self.recent_picture(game, card, (12., 255), theme, &art)?;
            self.saved_badge(
                game,
                (card.x() + self.px(10.), card.y() + self.px(10.)),
                theme,
            )?;

            // Name, then system and when, then a dot per game.
            let name_h = self.px(24.) as u32;
            let floor = self.frame.min_size();
            let room = info.width().saturating_sub(2 * pad as u32).max(1);
            let size = self
                .shrink(&game.name, Face::Semibold, 19., floor, room)?
                .unwrap_or(floor);
            let band = Rect::new(info.x() + pad, info.y(), room, name_h);
            self.label_aligned(
                &game.name,
                (Face::Semibold, size, theme.text),
                band,
                Align::Center,
            )?;
            let system = system_title(&game.system, data, &self.catalog);
            let line = match game.left {
                Some(left) => {
                    let ago = self.catalog.relative_time(left, self.frame.unix);
                    self.catalog
                        .format("switcher.played", &[("system", &system), ("ago", &ago)])
                }
                None => system,
            };
            let band = Rect::new(
                band.x(),
                band.bottom() + self.px(3.),
                room,
                self.px(16.) as u32,
            );
            self.label_aligned(
                &line,
                (Face::Regular, 12., theme.muted),
                band,
                Align::Center,
            )?;
            let (dot, wide, gap) = (self.px(6.), self.px(18.), self.px(5.));
            let count = games.len() as i32;
            let total = (count - 1) * (dot + gap) + wide;
            let mut x = info.x() + (info.width() as i32 - total) / 2;
            let y = band.bottom() + self.px(6.);
            for n in 0..games.len() {
                let (w, ink) = if n == at {
                    (wide, theme.highlight_bg)
                } else {
                    (dot, theme.badge)
                };
                let rect = Rect::new(x, y, w as u32, dot as u32);
                self.rounded(rect, dot as f32 / 2., ink, None)?;
                x += w + gap;
            }
        }
        self.switcher_footer(app, theme)
    }
    /// Where the switcher is in the list, and its buttons.
    fn switcher_footer(&mut self, app: &App, theme: &Theme) -> Result<(), String> {
        let games = &app.switcher;
        let at = app.cursor.min(games.len().saturating_sub(1));
        let summary = if games.is_empty() {
            String::new()
        } else {
            self.catalog.format(
                "switcher.position",
                &[
                    ("n", &(at + 1).to_string()),
                    ("count", &games.len().to_string()),
                ],
            )
        };
        let mut hints: Vec<(HintButton, &str)> = Vec::new();
        if games.len() > 1 {
            hints.push((HintButton::Pair('‹', '›'), "switcher.switch"));
        }
        if !games.is_empty() {
            hints.push(('A'.into(), "switcher.continue"));
            hints.push(('X'.into(), "switcher.remove"));
        }
        hints.push(('B'.into(), "switcher.back"));
        self.progress_footer(&summary, &hints, theme)
    }

    pub(super) fn resume(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Some(boot) = &app.boot else {
            return Ok(());
        };
        if theme.screens.resume.title {
            return self.resume_title(boot, data, theme, art);
        }
        let Frame { width, height, .. } = self.frame;
        let style = theme.screens.resume;
        if let Some(backdrop) = style.backdrop {
            self.blurred_game(&boot.game, &art, theme, backdrop)?;
        }
        let shot = pixels(theme.resume_shot, width, height);
        let info = pixels(theme.resume_info, width, height);
        let radius = self.px(12.) as f32;
        self.lift(shot, radius)?;
        self.recent_picture(&boot.game, shot, (12., 255), theme, &art)?;

        let pad = self.px(24.);
        let room = info.width().saturating_sub(2 * pad as u32).max(1);
        let mut y = info.y();
        let line = |ui: &mut Self,
                    y: &mut i32,
                    text: &str,
                    (face, size, ink): (Face, f32, Rgba),
                    h: f32| {
            let floor = ui.frame.min_size();
            let size = ui.shrink(text, face, size, floor, room)?.unwrap_or(floor);
            let band = Rect::new(info.x() + pad, *y, room, ui.px(h) as u32);
            *y = band.bottom();
            ui.label_aligned(text, (face, size, ink), band, Align::Center)
        };
        let returning = self.catalog.text("resume.returning");
        line(
            self,
            &mut y,
            &returning,
            (Face::Regular, 13., theme.muted),
            18.,
        )?;
        y += self.px(4.);
        let name = boot.game.name.clone();
        line(self, &mut y, &name, (Face::Semibold, 22., theme.text), 28.)?;
        y += self.px(4.);
        let system = system_title(&boot.game.system, data, &self.catalog);
        let when = boot
            .game
            .left
            .map(|left| self.resume_when(left))
            .unwrap_or_default();
        let key = match (&boot.game.entry, boot.power_off) {
            (None, _) => "resume.no_state",
            (Some(_), true) => "resume.saved_off",
            (Some(_), false) => "resume.saved_switch",
        };
        let detail = self
            .catalog
            .format(key, &[("system", &system), ("when", &when)]);
        line(
            self,
            &mut y,
            &detail,
            (Face::Regular, 12., theme.muted),
            16.,
        )?;

        // The countdown, then the two buttons centered under it.
        y += self.px(14.);
        let bar_w = self.px(220.);
        let under = Rect::new(
            info.x() + (info.width() as i32 - bar_w) / 2,
            y,
            bar_w as u32,
            self.px(4.) as u32,
        );
        let bar = style.progress.map_or(under, |r| self.area(r));
        let bar_w = bar.width() as i32;
        self.rounded(bar, self.px(2.) as f32, theme.badge, None)?;
        let done = (bar_w as f32 * boot.progress.clamp(0., 1.)).round() as u32;
        if done > 0 {
            let fill = Rect::new(bar.x(), bar.y(), done, bar.height());
            self.rounded(fill, self.px(2.) as f32, theme.highlight_bg, None)?;
        }
        let hints = [('A', "resume.play_now"), ('B', "resume.to_menu")];
        let mut total = self.px(16.) * (hints.len() as i32 - 1);
        for (_, key) in hints {
            let label = self.catalog.text(key);
            total += self.px(26.) + self.measure(&label, Face::Regular, 13.)? as i32;
        }
        let end = info.x() + (info.width() as i32 + total) / 2 + self.px(24.);
        let h = self.px(24.) as u32;
        self.hints(
            &hints,
            theme,
            end.max(1) as u32,
            under.bottom() + self.px(14.),
            h,
        )?;
        Ok(())
    }
    /// The blur and veil a game's picture takes behind the switcher and
    /// the resume when the theme says nothing: its glass's blur, at least
    /// 24 px, two fifths through the background.
    fn backdrop_or_default(
        theme: &Theme,
        own: Option<crate::screens::Backdrop>,
    ) -> crate::screens::Backdrop {
        own.unwrap_or(crate::screens::Backdrop {
            blur: theme.glass.map_or(0., |g| g.blur).max(24. / 480.),
            ..Default::default()
        })
    }
    /// `game`'s picture over the whole screen, blurred by `backdrop.blur`
    /// (sharp at 0) and showing through the background at its opacity;
    /// the theme's background while it has none.
    fn blurred_game(
        &mut self,
        game: &Recent,
        art: &Art<'_, '_>,
        theme: &Theme,
        backdrop: crate::screens::Backdrop,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let whole = Rect::new(0, 0, width, height);
        let found = game
            .picture
            .as_ref()
            .and_then(|path| art.pictures.and_then(|c| c.peek(path)).map(|t| (path, t)));
        if let Some((path, texture)) = found {
            let steps = super::material::halvings(self.frame.size(backdrop.blur));
            let size = texture.query();
            let part = crop((size.width, size.height), (width, height));
            let src = Rect::new(
                (part[0] * size.width as f32) as i32,
                (part[1] * size.height as f32) as i32,
                (part[2] * size.width as f32).round().max(1.) as u32,
                (part[3] * size.height as f32).round().max(1.) as u32,
            );
            if backdrop.blur <= 0. {
                self.canvas.copy(texture, src, whole)?;
            } else {
                let made = self.blurred(path, steps, |ui| {
                    ui.canvas.copy(texture, src, None).map(|()| true)
                });
                if let Some(index) = made {
                    self.draw_blurred(index, whole, 255)?;
                }
            }
        }
        let veil = (255. * (1. - backdrop.opacity.clamp(0., 1.))).round() as u8;
        self.canvas.set_draw_color(color(Rgba {
            a: veil,
            ..theme.background
        }));
        self.canvas.fill_rect(whole)
    }
    /// The switcher in depth: the selected card large and outlined with a
    /// line above it, its neighbours smaller and dimmed, leaving the screen,
    /// over its picture blurred; its name, system and dots below.
    fn switcher_depth(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let games = &app.switcher;
        let at = app.cursor.min(games.len().saturating_sub(1));
        if let Some(game) = games.get(at) {
            self.blurred_game(game, &art, theme, Self::backdrop_or_default(theme, None))?;
        }
        self.status_header(theme)?;
        let card = pixels(theme.switcher_card, width, height);
        let info = pixels(theme.switcher_info, width, height);
        if games.is_empty() {
            let text = self.catalog.text(if self.loading {
                "loading"
            } else {
                "switcher.empty"
            });
            return self.label_aligned(
                &text,
                (Face::Regular, 15., theme.muted),
                card,
                Align::Center,
            );
        }
        let radius = self.px(18.) as f32;
        // Two neighbours a side, each smaller and dimmer than the last.
        let gap = self.px(18.);
        for step in [2usize, 1] {
            let scale = if step == 1 { 0.78 } else { 0.6 };
            let alpha = if step == 1 { 150 } else { 70 };
            let (w, h) = (
                (card.width() as f32 * scale).round() as i32,
                (card.height() as f32 * scale).round() as i32,
            );
            let y = card.y() + (card.height() as i32 - h) / 2;
            let reach = |k: usize| -> i32 {
                // How far the k-th neighbour's near edge stands from the card.
                (1..k)
                    .map(|j| (card.width() as f32 * if j == 1 { 0.78 } else { 0.6 }) as i32 + gap)
                    .sum::<i32>()
                    + gap
            };
            for (n, left) in [(at.checked_sub(step), true), (at.checked_add(step), false)] {
                let Some(game) = n.and_then(|n| games.get(n)) else {
                    continue;
                };
                let x = if left {
                    card.x() - reach(step) - w
                } else {
                    card.right() + reach(step)
                };
                let rect = Rect::new(x, y, w.max(1) as u32, h.max(1) as u32);
                self.recent_picture(game, rect, (14., alpha), theme, &art)?;
            }
        }
        let game = &games[at];
        self.lift(card, radius)?;
        self.recent_picture(game, card, (18., 255), theme, &art)?;
        let out = self.px(3.);
        let ring = Rect::new(
            card.x() - out,
            card.y() - out,
            card.width() + 2 * out as u32,
            card.height() + 2 * out as u32,
        );
        self.focus_ring(ring, radius + out as f32, theme)?;
        // Above it: playing now, or when it was left.
        let above = match (at, game.left) {
            (0, _) => self.catalog.text("switcher.now"),
            (_, Some(left)) => self.catalog.relative_time(left, self.frame.unix),
            _ => String::new(),
        };
        let line_h = self.px(18.);
        self.caps_label(
            &above,
            (Face::Semibold, 11., theme.text_secondary),
            Rect::new(
                card.x(),
                ring.y() - line_h - self.px(6.),
                card.width(),
                line_h as u32,
            ),
            Align::Center,
        )?;
        self.saved_badge(
            game,
            (card.x() + self.px(12.), card.y() + self.px(12.)),
            theme,
        )?;
        // The name, system and when, and a dot per game.
        let pad = self.px(24.);
        let room = info.width().saturating_sub(2 * pad as u32).max(1);
        let floor = self.frame.min_size();
        let size = self
            .shrink(&game.name, Face::Bold, 21., floor, room)?
            .unwrap_or(floor);
        let band = Rect::new(info.x() + pad, info.y(), room, self.px(26.) as u32);
        self.label_aligned(
            &game.name,
            (Face::Bold, size, theme.text),
            band,
            Align::Center,
        )?;
        let system = system_title(&game.system, data, &self.catalog);
        let band = Rect::new(
            band.x(),
            band.bottom() + self.px(2.),
            room,
            self.px(16.) as u32,
        );
        self.label_aligned(
            &system,
            (Face::Regular, 12., theme.text_secondary),
            band,
            Align::Center,
        )?;
        let (dot, wide, gap) = (self.px(6.), self.px(18.), self.px(5.));
        let count = games.len() as i32;
        let total = (count - 1) * (dot + gap) + wide;
        let mut x = info.x() + (info.width() as i32 - total) / 2;
        let y = band.bottom() + self.px(8.);
        for n in 0..games.len() {
            let (w, ink) = if n == at {
                (wide, theme.text)
            } else {
                (
                    dot,
                    Rgba {
                        a: 0x50,
                        ..theme.text
                    },
                )
            };
            self.rounded(
                Rect::new(x, y, w as u32, dot as u32),
                dot as f32 / 2.,
                ink,
                None,
            )?;
            x += w + gap;
        }
        Ok(())
    }
    /// The resume as a title: "Returning to", the game's name large and its
    /// system in the middle of the screen over its picture blurred, a thin
    /// bar for the countdown and the two buttons.
    fn resume_title(
        &mut self,
        boot: &crate::resume::Boot,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let style = theme.screens.resume;
        self.blurred_game(
            &boot.game,
            &art,
            theme,
            Self::backdrop_or_default(theme, style.backdrop),
        )?;
        let pad = self.px(32.);
        let room = width.saturating_sub(2 * pad as u32).max(1);
        let name = boot.game.name.clone();
        let floor = self.frame.min_size().max(16.);
        let size = self
            .shrink(&name, Face::Bold, 34., floor, room)?
            .unwrap_or(floor);
        let lines = self.wrap(&name, Face::Bold, size, room)?;
        let step = (self.frame.size(size / 480.) * 1.15).round() as i32;
        let shown = lines.len().min(2) as i32;
        let block = self.px(20.) + step * shown + self.px(28.) + self.px(30.) + self.px(30.);
        let mut y = (height as i32 - block) / 2;
        let center = |y: i32, h: i32| Rect::new(pad, y, room, h.max(1) as u32);
        let returning = self.catalog.text("resume.returning");
        self.caps_label(
            &returning,
            (Face::Semibold, 12., theme.text_secondary),
            center(y, self.px(18.)),
            Align::Center,
        )?;
        y += self.px(20.);
        for line in lines.iter().take(2) {
            self.label_aligned(
                line,
                (Face::Bold, size, theme.text),
                center(y, step),
                Align::Center,
            )?;
            y += step;
        }
        let system = system_title(&boot.game.system, data, &self.catalog);
        let when = boot
            .game
            .left
            .map(|left| self.resume_when(left))
            .unwrap_or_default();
        let key = match (&boot.game.entry, boot.power_off) {
            (None, _) => "resume.no_state",
            (Some(_), true) => "resume.saved_off",
            (Some(_), false) => "resume.saved_switch",
        };
        let detail = self
            .catalog
            .format(key, &[("system", &system), ("when", &when)]);
        let size = self
            .shrink(&detail, Face::Regular, 13., self.frame.min_size(), room)?
            .unwrap_or(self.frame.min_size());
        y += self.px(4.);
        self.label_aligned(
            &detail,
            (Face::Regular, size, theme.text_secondary),
            center(y, self.px(20.)),
            Align::Center,
        )?;
        y += self.px(36.);
        let bar_w = (width as i32 * 2 / 5).max(self.px(120.));
        let under = Rect::new(
            (width as i32 - bar_w) / 2,
            y,
            bar_w as u32,
            self.px(3.).max(2) as u32,
        );
        // The theme's place for the countdown, else under the texts; the
        // buttons stay under the texts.
        let bar = style.progress.map_or(under, |r| self.area(r));
        let bar_w = bar.width() as i32;
        let r = bar.height() as f32 / 2.;
        self.rounded(
            bar,
            r,
            Rgba {
                a: 0x30,
                ..theme.text
            },
            None,
        )?;
        let done = (bar_w as f32 * boot.progress.clamp(0., 1.)).round() as u32;
        if done > 0 {
            self.rounded(
                Rect::new(bar.x(), bar.y(), done, bar.height()),
                r,
                theme.text,
                None,
            )?;
        }
        let hints = [('A', "resume.play_now"), ('B', "resume.to_menu")];
        let mut total = self.px(16.) * (hints.len() as i32 - 1);
        for (_, key) in hints {
            let label = self.catalog.text(key);
            total += self.px(26.) + self.measure(&label, Face::Regular, 13.)? as i32;
        }
        let end = (width as i32 + total) / 2 + self.px(24.);
        self.hints(
            &hints,
            theme,
            end.max(1) as u32,
            under.bottom() + self.px(18.),
            self.px(24.) as u32,
        )?;
        Ok(())
    }
    /// "today at 23:40", "yesterday at 23:40", else the day and time.
    fn resume_when(&self, unix: u64) -> String {
        let clock_12 = self.status.as_ref().is_some_and(Status::clock_12);
        let time = crate::status::local_clock(unix, clock_12);
        let day = |t: u64| crate::status::local_time(t).map(|tm| (tm.tm_year, tm.tm_yday));
        let (then, now) = (day(unix), day(self.frame.unix));
        let yesterday = day(self.frame.unix.saturating_sub(86_400));
        if then.is_some() && then == now {
            self.catalog.format("resume.today", &[("time", &time)])
        } else if then.is_some() && then == yesterday {
            self.catalog.format("resume.yesterday", &[("time", &time)])
        } else {
            let tm = crate::status::local_time(unix);
            let (d, m) = tm.map_or((0, 0), |tm| (tm.tm_mday, tm.tm_mon + 1));
            self.catalog.format(
                "resume.date",
                &[
                    ("day", &format!("{d:02}")),
                    ("month", &format!("{m:02}")),
                    ("time", &time),
                ],
            )
        }
    }
}
