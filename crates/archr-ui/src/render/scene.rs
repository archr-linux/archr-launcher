//! Free screens (docs/temas.md, Telas livres): the elements of a systems or
//! games screen drawn in their order, each in its box. Pictures load on a
//! worker at most one and a half times the screen's size; until one
//! arrives the element is simply not there.
use super::*;
use crate::{
    covers::CoverCache,
    layout::{
        Data, Element, Fill, Font, Hints, Image, ImageFit, Kind, ListInk, Scene, Slices, Text,
        VAlign,
    },
    primitives::nine_slice,
};

/// Decoded pictures a free screen keeps: two full-screen pictures of a
/// 1024x768 screen and the game's, with room to spare.
const SCENE_BUDGET: u64 = 48 << 20;

/// What the screen knows this frame, for the elements' data.
struct Facts<'a> {
    system: String,
    name: String,
    maker: String,
    year: String,
    count: usize,
    games: String,
    favorites: usize,
    game: Option<&'a archr_index::Game>,
    players: String,
    rating: String,
    played: String,
    index: usize,
    total: usize,
    item: String,
    aspect: &'static str,
}
impl Facts<'_> {
    fn value(&self, data: Data) -> String {
        let game = |f: &dyn Fn(&archr_index::Game) -> String| self.game.map(f).unwrap_or_default();
        match data {
            Data::System => self.system.clone(),
            Data::SystemName => self.name.clone(),
            Data::SystemMaker => self.maker.clone(),
            Data::SystemYear => self.year.clone(),
            Data::SystemCount => self.count.to_string(),
            Data::SystemGames => self.games.clone(),
            Data::SystemFavorites => self.favorites.to_string(),
            Data::GameName => game(&|g| g.name.clone()),
            Data::GameDescription => game(&|g| g.details.description.clone()),
            Data::GameGenre => game(&|g| g.details.genre.clone()),
            Data::GameDeveloper => game(&|g| g.details.developer.clone()),
            Data::GamePublisher => game(&|g| g.details.publisher.clone()),
            Data::GameYear => game(&|g| g.year.map(|y| y.to_string()).unwrap_or_default()),
            Data::GamePlayers => self.players.clone(),
            Data::GameRating => self.rating.clone(),
            Data::GameLastPlayed => self.played.clone(),
            Data::GameCover | Data::GameScreenshot | Data::GameLogo => String::new(),
            Data::ListIndex => self.index.to_string(),
            Data::ListTotal => self.total.to_string(),
            Data::Item => self.item.clone(),
            Data::Aspect => self.aspect.to_owned(),
        }
    }
    /// The file of one of the game's pictures.
    fn picture(&self, data: Data) -> Option<PathBuf> {
        let game = self.game?;
        match data {
            Data::GameCover => game.image.clone(),
            Data::GameScreenshot => game.details.screenshot.clone(),
            Data::GameLogo => game.details.logo.clone(),
            _ => None,
        }
    }
    fn has(&self, data: Data) -> bool {
        if data.picture() {
            self.picture(data).is_some()
        } else {
            !self.value(data).is_empty()
        }
    }
}

/// What a free screen keeps between frames.
#[derive(Default)]
pub(super) struct SceneCache<'t> {
    /// Its pictures, made for this folder and screen size.
    pictures: Option<(Option<PathBuf>, (u32, u32), CoverCache<'t>)>,
    /// The font files named, as given and as found in the theme folder
    /// (an empty path when missing: the regular face stands in).
    fonts: (Vec<String>, Option<PathBuf>, Vec<PathBuf>),
    /// The theme with the list element's colors over it.
    pub list: Option<(ListInk, Theme)>,
}
impl SceneCache<'_> {
    pub fn poll(&mut self) -> bool {
        self.pictures.as_mut().is_some_and(|(_, _, c)| c.poll())
    }
    pub fn idle(&self) -> bool {
        self.pictures.as_ref().is_none_or(|(_, _, c)| c.idle())
    }
}

impl<'t> Ui<'_, 't> {
    /// The free screen `scene` of `backdrop` (systems or games).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn scene(
        &mut self,
        app: &App,
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        layout: &Layout,
        scene: &Scene,
    ) -> Result<(), String> {
        let facts = self.facts(backdrop, (rows, cursor), data, theme);
        self.scene_fonts(scene);
        let shown: Vec<&Element> = scene
            .elements
            .iter()
            .filter(|e| visible(e, &facts))
            .collect();
        // Every picture shown, asked for at once: neighbours never evict
        // what is on screen.
        let wanted: Vec<Vec<PathBuf>> = shown
            .iter()
            .filter_map(|e| match &e.kind {
                Kind::Image(image) => Some(self.sources(image, &facts)),
                _ => None,
            })
            .filter(|paths| !paths.is_empty())
            .collect();
        self.scene_pictures(theme.pixel_images)
            .request_candidates(&wanted);
        for element in shown {
            let rect = self.area(element.rect);
            let turn = (element.rotate != 0.).then(|| {
                let (x, y) = element.pivot;
                (
                    f64::from(element.rotate),
                    (
                        rect.x() + (x * rect.width() as f32).round() as i32,
                        rect.y() + (y * rect.height() as f32).round() as i32,
                    ),
                )
            });
            // Pictures, text and boxes take an opacity as they draw; the
            // rest, and anything turned, through a layer.
            let simple = matches!(
                element.kind,
                Kind::Image(_) | Kind::Text(_) | Kind::Fill(Fill { gradient: None, .. })
            );
            let (outer, inner) = if simple && turn.is_none() {
                (1., element.opacity)
            } else {
                (element.opacity, 1.)
            };
            self.layered(outer, turn, |ui| match &element.kind {
                Kind::Image(image) => ui.scene_image(image, rect, inner, &facts, theme),
                Kind::Text(text) => ui.scene_text(text, rect, inner, &facts, theme),
                Kind::Fill(fill) => ui.scene_fill(fill, rect, inner),
                Kind::Frame(slices) => ui.scene_frame(slices, rect, &facts),
                Kind::List(ink) => ui.scene_list(
                    app,
                    backdrop,
                    (rows, cursor),
                    data,
                    (theme, *ink),
                    art,
                    layout,
                ),
                Kind::Hints(hints) => ui.scene_hints(app, data, theme, layout, (hints, rect)),
                Kind::Status(plain) => ui.status_pill(theme, Some(rect), *plain).map(|_| ()),
                Kind::Panel(panel) => {
                    let panel = crate::layout::Panel {
                        rect: element.rect,
                        ..*panel
                    };
                    ui.layout_panel("panel", &panel, theme).map(|_| ())
                }
            })?;
        }
        Ok(())
    }

    fn facts<'d>(
        &self,
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
        data: &'d dyn DataSource,
        theme: &Theme,
    ) -> Facts<'d> {
        let catalog = &self.catalog;
        let (system, game, index, total, item) = match backdrop {
            Screen::Games { system, folder } => {
                let (index, total) = match App::level(data, system, folder) {
                    Some(level) => (level.position(cursor), level.total()),
                    None => (cursor + 1, data.games(system).len()),
                };
                let game = App::list_game(data, system, folder, cursor)
                    .and_then(|g| data.games(system).get(g));
                let item = match App::level(data, system, folder)
                    .and_then(|l| l.folder(cursor).map(str::to_owned))
                {
                    Some(name) => name,
                    None => game
                        .and_then(|g| g.path.file_stem())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                };
                (system.clone(), game, index.min(total), total, item)
            }
            _ => {
                let system = rows.get(cursor).cloned().unwrap_or_default();
                (system.clone(), None, cursor + 1, rows.len(), system)
            }
        };
        let (maker, year) = data.manufacturer(&system).unwrap_or_default();
        let count = system_count(&system, data);
        let rating = game
            .and_then(|g| g.details.rating)
            .map_or_else(String::new, |r| {
                format!("{:.1}", f32::from(r) / 20.).replace('.', &catalog.text("details.decimal"))
            });
        Facts {
            name: system_title(&system, data, catalog),
            maker: maker.to_owned(),
            year: year.to_owned(),
            count,
            games: catalog.plural(
                "games.count",
                count as f64,
                &[("count", &count.to_string())],
            ),
            favorites: data.favorite_count(&system),
            players: game
                .and_then(|g| g.players.as_deref())
                .map(|p| catalog.players(p))
                .unwrap_or_default(),
            played: game
                .and_then(|g| g.last_played)
                .map(|t| catalog.relative_time(t, self.frame.unix))
                .unwrap_or_default(),
            rating,
            game,
            index,
            total,
            item: item.trim_start_matches('@').to_owned(),
            system: system.trim_start_matches('@').to_owned(),
            aspect: theme.shape.aspect.name(),
        }
    }

    /// A catalog phrase (`@genre`): the scene's own, the details', or any key.
    fn phrase(&self, key: &str) -> String {
        for wanted in [
            format!("scene.{key}"),
            format!("details.{key}"),
            key.to_owned(),
        ] {
            if self.catalog.has(&wanted) {
                return self.catalog.text(&wanted);
            }
        }
        key.to_owned()
    }

    /// The font files the scene names, found once in the theme folder.
    fn scene_fonts(&mut self, scene: &Scene) {
        let cache = &mut self.scene_cache.fonts;
        if cache.0 != scene.fonts || cache.1 != self.theme_dir {
            cache.2 = scene
                .fonts
                .iter()
                .map(|file| {
                    self.theme_dir
                        .as_deref()
                        .and_then(|dir| crate::theme::resolve_file(dir, file))
                        .unwrap_or_default()
                })
                .collect();
            cache.0.clone_from(&scene.fonts);
            cache.1.clone_from(&self.theme_dir);
        }
        if let Some(text) = &mut self.text {
            text.set_files(&self.scene_cache.fonts.2);
        }
    }

    /// The scene's picture loader: pictures at most one and a half times
    /// the screen (a full-screen picture still covers it without blur).
    fn scene_pictures(&mut self, pixel: bool) -> &mut CoverCache<'t> {
        let key = (
            self.theme_dir.clone(),
            (self.frame.width, self.frame.height),
        );
        let stale = self
            .scene_cache
            .pictures
            .as_ref()
            .is_none_or(|(dir, size, _)| (dir, size) != (&key.0, &key.1));
        if stale {
            let creator = self.creator.expect("configured before drawing");
            let root = self.root.clone().unwrap_or_else(|| PathBuf::from("/"));
            let (w, h) = key.1;
            let cache = CoverCache::scaled(
                creator,
                root.join("storage/.cache/archr-launcher/thumbs/scene"),
                (w * 3 / 2, h * 3 / 2),
                pixel,
            )
            .with_budget(SCENE_BUDGET);
            self.scene_cache.pictures = Some((key.0, key.1, cache));
        }
        &mut self.scene_cache.pictures.as_mut().expect("made above").2
    }

    /// The files a picture may come from, in order.
    fn sources(&self, image: &Image, facts: &Facts<'_>) -> Vec<PathBuf> {
        image
            .src
            .iter()
            .filter_map(|template| match template.0.as_slice() {
                [crate::layout::Piece::Data(data)] if data.picture() => facts.picture(*data),
                _ => self.theme_file(&template.fill(|d| facts.value(d), |k| self.phrase(k))),
            })
            .collect()
    }
    /// `path` inside the theme folder, when it stays there.
    fn theme_file(&self, path: &str) -> Option<PathBuf> {
        if !crate::layout::safe_path(path) {
            return None;
        }
        Some(self.theme_dir.as_ref()?.join(path))
    }

    fn scene_image(
        &mut self,
        image: &Image,
        rect: Rect,
        opacity: f32,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let paths = self.sources(image, facts);
        let alpha = (opacity * 255.).round() as u8;
        let radius = image.radius * self.frame.height as f32;
        let (pictures, canvas, geometry) = match &mut self.scene_cache.pictures {
            Some((_, _, cache)) => (cache, &mut self.canvas, &mut self.geometry),
            None => return Ok(()),
        };
        let key = paths.first();
        let missing = key.is_none_or(|k| pictures.has(k) && pictures.peek(k).is_none());
        if let Some(texture) = key.and_then(|k| pictures.get_mut(k)) {
            let size = texture.query();
            let (tw, th) = (size.width.max(1) as f32, size.height.max(1) as f32);
            let (dst, part) = match image.fit {
                ImageFit::Stretch => (rect, WHOLE),
                ImageFit::Cover => (
                    rect,
                    crop((size.width, size.height), (rect.width(), rect.height())),
                ),
                ImageFit::Contain => {
                    let k = (rect.width() as f32 / tw).min(rect.height() as f32 / th);
                    let (w, h) = (
                        (tw * k).round().max(1.) as u32,
                        (th * k).round().max(1.) as u32,
                    );
                    let x = match image.align {
                        Align::Left => rect.x(),
                        Align::Right => rect.right() - w as i32,
                        Align::Center => rect.x() + (rect.width() as i32 - w as i32) / 2,
                    };
                    let y = rect.y() + (rect.height() as i32 - h as i32) / 2;
                    (Rect::new(x, y, w, h), WHOLE)
                }
            };
            if let Some(tint) = image.tint {
                texture.set_color_mod(tint.r, tint.g, tint.b);
            }
            let drawn = if radius > 0. {
                geometry.cover(canvas, texture, dst, radius, alpha, part)
            } else {
                let src = Rect::new(
                    (part[0] * tw) as i32,
                    (part[1] * th) as i32,
                    (part[2] * tw).round().max(1.) as u32,
                    (part[3] * th).round().max(1.) as u32,
                );
                texture.set_alpha_mod(alpha);
                let drawn = canvas.copy(texture, src, dst);
                texture.set_alpha_mod(255);
                drawn
            };
            texture.set_color_mod(255, 255, 255);
            return drawn;
        }
        match &image.fallback {
            Some(text) if missing => self.scene_text(text, rect, opacity, facts, theme),
            _ => Ok(()),
        }
    }

    fn scene_text(
        &mut self,
        t: &Text,
        rect: Rect,
        opacity: f32,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let mut text = t.text.fill(|d| facts.value(d), |k| self.phrase(k));
        if t.upper {
            text = text.to_uppercase();
        }
        if text.trim().is_empty() {
            return Ok(());
        }
        let face = match t.font {
            Font::Regular => Face::Regular,
            Font::Semibold => Face::Semibold,
            Font::Bold => Face::Bold,
            Font::Mono => Face::Mono,
            Font::File(n) => Face::File(n),
        };
        let mut size = t.size * self.frame.reference().1;
        if t.shrink && !t.wrap {
            let least = (size * 0.6).max(self.frame.min_size());
            size = self
                .shrink(&text, face, size, least, rect.width())?
                .unwrap_or(least);
        }
        let px = self.frame.font_size(size);
        let line = (f32::from(px) * t.line).round().max(1.) as i32;
        let mut lines = if t.wrap {
            self.wrap(&text, face, size, rect.width())?
        } else {
            vec![text]
        };
        let room = if t.wrap {
            (rect.height() as i32 / line).max(1) as usize
        } else {
            1
        };
        if lines.len() > room {
            lines.truncate(room);
            if let Some(last) = lines.last_mut() {
                last.push('…');
            }
        }
        let block = line * lines.len() as i32;
        let top = match t.valign {
            VAlign::Top => rect.y(),
            VAlign::Middle => rect.y() + (rect.height() as i32 - block) / 2,
            VAlign::Bottom => rect.bottom() - block,
        };
        let fade = |c: Rgba| Rgba {
            a: (f32::from(c.a) * opacity).round() as u8,
            ..c
        };
        let ink = fade(t.color.unwrap_or(theme.text));
        let tall = self.frame.height as f32;
        let tracking = self.tracking(size, px);
        let shadow = t.shadow.map(|s| {
            (
                fade(s.color),
                (s.x * tall).round() as i32,
                (s.y * tall).round() as i32,
            )
        });
        let text_cache = self
            .text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?;
        for (n, text) in lines.iter().enumerate() {
            let at = Rect::new(rect.x(), top + line * n as i32, rect.width(), line as u32);
            let mut style = Style {
                face,
                size: px,
                color: ink,
                tracking,
            };
            if let Some((color, dx, dy)) = shadow {
                let under = Rect::new(at.x() + dx, at.y() + dy, at.width(), at.height());
                let shade = Style { color, ..style };
                text_cache
                    .draw(&mut self.canvas, text, shade, under, t.align)
                    .map_err(|e| localized_error(&self.catalog, e))?;
            }
            style.color = ink;
            text_cache
                .draw(&mut self.canvas, text, style, at, t.align)
                .map_err(|e| localized_error(&self.catalog, e))?;
        }
        Ok(())
    }

    fn scene_fill(&mut self, fill: &Fill, rect: Rect, opacity: f32) -> Result<(), String> {
        let radius = fill.radius * self.frame.height as f32;
        let fade = |c: Rgba| Rgba {
            a: (f32::from(c.a) * opacity).round() as u8,
            ..c
        };
        match fill.gradient {
            Some(gradient) => {
                self.geometry
                    .shaded(&mut self.canvas, rect, radius, &gradient, fill.border)
            }
            None => self.rounded(rect, radius, fade(fill.fill), fill.border.map(fade)),
        }
    }

    fn scene_frame(
        &mut self,
        slices: &Slices,
        rect: Rect,
        facts: &Facts<'_>,
    ) -> Result<(), String> {
        let path = slices.image.fill(|d| facts.value(d), |k| self.phrase(k));
        let Some(path) = self.theme_file(&path) else {
            return Ok(());
        };
        let k = (slices.scale * self.frame.scale).round().max(1.) as u32;
        let Some((texture, canvas)) = self.picture(&path) else {
            return Ok(());
        };
        let size = texture.query();
        for (src, to) in nine_slice(
            (size.width, size.height),
            slices.slice,
            k,
            rect,
            slices.tile,
        ) {
            canvas.copy(texture, src, to)?;
        }
        Ok(())
    }

    /// The screen's list where the scene puts it, in its colors.
    #[allow(clippy::too_many_arguments)]
    fn scene_list(
        &mut self,
        app: &App,
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
        data: &dyn DataSource,
        (theme, ink): (&Theme, ListInk),
        art: Art<'_, '_>,
        layout: &Layout,
    ) -> Result<(), String> {
        let inked;
        let theme = if ink == ListInk::default() {
            theme
        } else {
            if self
                .scene_cache
                .list
                .as_ref()
                .is_none_or(|(i, _)| *i != ink)
            {
                let mut own = theme.clone();
                if let Some(c) = ink.color {
                    own.text_secondary = c;
                    own.muted = c;
                }
                if let Some(c) = ink.fill {
                    own.highlight_bg = c;
                }
                if let Some(c) = ink.selected {
                    own.highlight_text = c;
                    own.accent_text_secondary = c;
                }
                self.scene_cache.list = Some((ink, own));
            }
            inked = self
                .scene_cache
                .list
                .as_ref()
                .expect("made above")
                .1
                .clone();
            &inked
        };
        let lifted = app.moving().is_some();
        match backdrop {
            Screen::Games { system, folder } if layout.list.mark == Mark::Grid => {
                self.games_grid((system, folder), cursor, (data, None), theme, art, layout)
            }
            _ => self.list(backdrop, (rows, lifted), cursor, data, theme, layout),
        }
    }

    /// The footer's hints in the box, packed to its side, in the element's
    /// colors and style.
    fn scene_hints(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        layout: &Layout,
        (hints, rect): (&Hints, Rect),
    ) -> Result<(), String> {
        let keys = self.hint_keys(app, data, layout);
        self.hint_ink = Some(*hints);
        let margin = self.px(24.);
        let (y, h) = (rect.y(), rect.height());
        let drawn = (|| {
            let edge = (rect.right() + margin).max(1) as u32;
            let left = self.faded(0., |ui| ui.hints(&keys, theme, edge, y, h))?;
            let used = rect.right() - left;
            let right = match hints.align {
                Align::Right => rect.right(),
                Align::Left => rect.x() + used,
                Align::Center => rect.x() + (rect.width() as i32 + used) / 2,
            };
            self.hints(&keys, theme, (right + margin).max(1) as u32, y, h)
                .map(|_| ())
        })();
        self.hint_ink = None;
        drawn
    }
}

/// Whether `element` shows for what the screen holds now (`when`).
fn visible(element: &Element, facts: &Facts<'_>) -> bool {
    (element.systems.is_empty()
        || element
            .systems
            .iter()
            .any(|s| s.trim_start_matches('@') == facts.system))
        && element.has.iter().all(|data| facts.has(*data))
}
