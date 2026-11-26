//! Free screens (docs/temas.md, Telas livres): the elements of a systems,
//! games or details screen drawn in their order, each in its box. Pictures
//! load on a worker at most one and a half times the screen's size (never
//! enlarged); until one arrives the element is simply not there.
use super::*;
use crate::{
    covers::CoverCache,
    layout::{
        Anim, Collection, Data, Element, Fields, Fill, Font, Hints, Image, ImageFit, Kind, ListInk,
        Ring, Scene, Shape, Slices, Stars, Template, Text, VAlign, Video,
    },
    motion::{Motion, Tween, SELECT},
    primitives::nine_slice,
};
use std::collections::HashMap;
mod collection;

/// Decoded pictures a free screen keeps: the full-screen pictures of a
/// 1024x768 screen's system and its two neighbours, and the game's.
const SCENE_BUDGET: u64 = 48 << 20;

/// A focus that moves again sooner than this after the last move (the
/// d-pad held: it repeats every 80 ms) shows the new item at once.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// The launcher's star: Lucide's (ISC), filled.
const STAR_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#fff" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></svg>"##;

/// Most pixels a clip is decoded at (a 640x480 clip's): a larger box is
/// filled by the GPU, so the cost on the RK3326 stays the details' one.
const CLIP_PIXELS: f32 = 640. * 480.;

/// The clip a free screen's `video` element wants played, for the host:
/// decoded at `size` (the box's shape) and drawn over the box.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub file: PathBuf,
    pub size: (u32, u32),
    pub fit: ImageFit,
    pub delay: Duration,
    pub repeat: bool,
}

/// The box's size, made smaller to at most `CLIP_PIXELS`.
fn clip_size(rect: Rect) -> (u32, u32) {
    let (w, h) = (rect.width() as f32, rect.height() as f32);
    let k = (CLIP_PIXELS / (w * h)).sqrt().min(1.);
    (
        (w * k).round().max(1.) as u32,
        (h * k).round().max(1.) as u32,
    )
}

/// An element's box this frame, its opacity (`outer` through a layer,
/// `inner` as it draws), and the layer's move: offset in pixels, size
/// and the point it grows about.
struct Placed {
    rect: Rect,
    rect01: crate::theme::Rect01,
    outer: f32,
    inner: f32,
    moved: Option<(f32, f32, f32, (i32, i32))>,
    /// Not in yet: transparent.
    hidden: bool,
}

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
    item_name: String,
    aspect: &'static str,
    /// The system's logo, picture and clip files, when it has them.
    logo: Option<PathBuf>,
    image: Option<PathBuf>,
    clip: Option<PathBuf>,
    /// The game's rating, 0 to 100.
    stars: Option<u8>,
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
            Data::GameCover
            | Data::GameScreenshot
            | Data::GameLogo
            | Data::GameVideo
            | Data::SystemLogo
            | Data::SystemImage
            | Data::SystemVideo => String::new(),
            Data::ListIndex => self.index.to_string(),
            Data::ListTotal => self.total.to_string(),
            Data::Item => self.item.clone(),
            Data::ItemName => self.item_name.clone(),
            Data::Aspect => self.aspect.to_owned(),
        }
    }
    /// The file of one of the system's or the game's pictures or clips.
    fn picture(&self, data: Data) -> Option<PathBuf> {
        match data {
            Data::SystemLogo => self.logo.clone(),
            Data::SystemImage => self.image.clone(),
            Data::SystemVideo => self.clip.clone(),
            Data::GameVideo => self.game?.video.clone(),
            Data::GameCover => self.game?.image.clone(),
            Data::GameScreenshot => self.game?.details.screenshot.clone(),
            Data::GameLogo => self.game?.details.logo.clone(),
            _ => None,
        }
    }
    fn has(&self, data: Data) -> bool {
        if data.file() {
            self.picture(data).is_some()
        } else {
            !self.value(data).is_empty()
        }
    }
}

/// A text wrapped: the text, the width, the size and the font.
type Wrapping = (String, u32, u16, Font);

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
    /// Each system's logo, picture and clip files, found once per theme.
    pub systems: HashMap<String, [Option<PathBuf>; 3]>,
    /// The launcher's star, drawn at a few sizes.
    stars: Vec<(u32, Texture<'t>)>,
    /// A carousel's place: its list, the cursor it last saw and where it
    /// stands counting every turn (a loop keeps going the same way).
    glide: Option<(String, usize, f32)>,
    /// A grid's focus growing in: its list, the item and how far.
    focus: Option<(String, usize, Tween)>,
    /// The screen's entrance and its focus's arrival (`enter`, `select`).
    show: Show,
    /// The details' scrolling text, wrapped: the text, width, size and
    /// font it was wrapped for.
    scroll: Option<(Wrapping, Vec<String>)>,
    /// Tiled pictures made once at their box's size: picture, box, scale
    /// and slices. Drawn tile by tile, a 4x4 pattern over the screen was
    /// 19 200 copies a frame (Albedo's scanlines), most of the frame's time.
    tiled: Vec<(TileKey, Texture<'t>)>,
}
type TileKey = (PathBuf, (u32, u32), u32, [u32; 4]);
/// Tiled pictures kept: a screen has one or two (a pattern, a background).
const TILED_KEPT: usize = 4;

/// When the screen came in and its focus last moved, for the elements'
/// `enter` and `select`.
#[derive(Default)]
struct Show {
    /// A free screen was drawn this frame, and the frame before.
    drawn: bool,
    before: bool,
    /// The screen and its item in focus, as last drawn.
    key: String,
    item: String,
    /// When the screen came in and the focus came to its item; None at
    /// rest, or when the move came too soon after the last to play.
    entered: Option<Instant>,
    selected: Option<Instant>,
    moved: Option<Instant>,
    /// How long this screen's entrances and arrivals take.
    spans: (Duration, Duration),
}

/// An element on its way in: off its place (pixels), smaller, see-through.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    dx: f32,
    dy: f32,
    scale: f32,
    alpha: f32,
}
impl Pose {
    const REST: Self = Self {
        dx: 0.,
        dy: 0.,
        scale: 1.,
        alpha: 1.,
    };
    fn moved(self) -> bool {
        self.dx.abs() >= 0.5 || self.dy.abs() >= 0.5 || (self.scale - 1.).abs() > 1e-3
    }
    fn then(self, other: Self) -> Self {
        Self {
            dx: self.dx + other.dx,
            dy: self.dy + other.dy,
            scale: self.scale * other.scale,
            alpha: self.alpha * other.alpha,
        }
    }
}
/// How long `anim` takes with `motion`, its delay included: reduced
/// motion only fades, briefly and at once; off, nothing moves.
fn span(anim: &Anim, motion: Motion) -> Duration {
    let duration = Duration::from_secs_f32(anim.duration);
    match motion {
        Motion::On => duration + Duration::from_secs_f32(anim.delay),
        _ => motion.fade(duration),
    }
}
/// Where `anim` has the element `secs` in, on a `screen` (w, h) in pixels.
fn pose(anim: &Anim, secs: f32, motion: Motion, (w, h): (f32, f32)) -> Pose {
    match motion {
        Motion::Off => Pose::REST,
        Motion::Reduced => {
            let fade = span(anim, motion).as_secs_f32().max(1e-3);
            Pose {
                alpha: (secs / fade).clamp(0., 1.),
                ..Pose::REST
            }
        }
        Motion::On => {
            let p = anim.ease.at((secs - anim.delay) / anim.duration);
            Pose {
                dx: anim.from.0 * w * (1. - p),
                dy: anim.from.1 * h * (1. - p),
                scale: anim.scale + (1. - anim.scale) * p,
                alpha: if anim.fade { p } else { 1. },
            }
        }
    }
}
impl SceneCache<'_> {
    pub fn poll(&mut self) -> bool {
        self.pictures.as_mut().is_some_and(|(_, _, c)| c.poll())
    }
    pub fn idle(&self) -> bool {
        self.pictures.as_ref().is_none_or(|(_, _, c)| c.idle())
    }
    /// A grid's focus grown at once, and every element in its place.
    pub fn snap(&mut self) {
        if let Some((_, _, tween)) = &mut self.focus {
            tween.snap();
        }
        (self.show.entered, self.show.selected) = (None, None);
    }
    /// A grid's focus is still growing, or an element still coming in.
    pub fn active(&self, now: Instant) -> bool {
        let show = &self.show;
        let within = |at: Option<Instant>, span: Duration| {
            at.is_some_and(|at| now.saturating_duration_since(at) < span)
        };
        self.focus.as_ref().is_some_and(|(_, _, t)| t.active(now))
            || within(show.entered, show.spans.0)
            || within(show.selected, show.spans.1)
    }
    /// A new frame: a free screen not drawn in the last one comes in
    /// anew when it is (back from Settings, a game, another screen).
    pub fn begin_frame(&mut self) {
        self.show.before = std::mem::take(&mut self.show.drawn);
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
        // The details' scrolling text: how far it reaches below its box,
        // known before the hints (↕) and the host (↑/↓) need it.
        if matches!(backdrop, Screen::Details { .. }) {
            self.details_overflow = 0;
            for e in &shown {
                if let Kind::Text(t) = &e.kind {
                    if t.scroll {
                        let text = t.text.fill(|d| facts.value(d), |k| self.phrase(k));
                        let (lines, _, room) = self.laid(&text, t, self.area(e.rect))?;
                        self.details_overflow = lines.len().saturating_sub(room);
                    }
                }
            }
        }
        let poses = self.poses(scene, &shown, backdrop, (rows, cursor));
        let placed: Vec<Placed> = shown
            .iter()
            .zip(&poses)
            .map(|(e, pose)| self.placed(e, *pose))
            .collect();
        // Every picture shown, asked for at once: neighbours never evict
        // what is on screen.
        // A collection's items, placed for this frame, each with its data;
        // their pictures are asked for first, the focus's before the rest.
        let mut spots = Vec::new();
        for (e, at) in shown.iter().zip(&placed) {
            if let Kind::Collection(c) = &e.kind {
                spots = self.spots(
                    c,
                    (at.rect, e.opacity),
                    backdrop,
                    (rows, cursor),
                    data,
                    theme,
                );
            }
        }
        let mut wanted: Vec<Vec<PathBuf>> = Vec::new();
        for (_, facts) in spots.iter().rev() {
            if let Some(c) = scene.collection() {
                for paths in [&c.item.image, &c.item.logo].map(|src| self.files(src, facts)) {
                    if !paths.is_empty() && !wanted.contains(&paths) {
                        wanted.push(paths);
                    }
                }
            }
        }
        wanted.extend(self.wanted(shown.iter().copied(), &facts));
        // A large picture takes 0.3 s or more to load the first time on
        // the RK3326: the systems on either side are asked for once this
        // one's pictures are in, when three systems' worth fit the budget
        // (more would push each other out, loading forever).
        let mut near: Vec<Vec<PathBuf>> = Vec::new();
        if matches!(backdrop, Screen::Systems) && rows.len() > 1 {
            for next in [cursor + 1, cursor + rows.len() - 1] {
                let facts = self.facts(backdrop, (rows, next % rows.len()), data, theme);
                let next = scene.elements.iter().filter(|e| visible(e, &facts));
                for paths in self.wanted(next, &facts) {
                    if !wanted.contains(&paths) && !near.contains(&paths) {
                        near.push(paths);
                    }
                }
            }
        }
        let pictures = self.scene_pictures(theme.pixel_images);
        pictures.request_candidates(&wanted);
        let held: Option<u64> = wanted.iter().map(|paths| pictures.bytes(&paths[0])).sum();
        if !near.is_empty() && held.is_some_and(|bytes| 3 * bytes <= SCENE_BUDGET) {
            pictures.request_candidates(&near);
            // What is on screen stays the newest, the last to make room.
            pictures.request_candidates(&wanted);
        }
        for (element, at) in shown.into_iter().zip(placed) {
            let Placed {
                rect,
                rect01,
                outer,
                inner,
                moved,
                hidden,
            } = at;
            // Not in yet: nothing to draw (the list and the clip keep
            // their state, so they always draw).
            if hidden && !matches!(element.kind, Kind::List(_) | Kind::Video(_)) {
                continue;
            }
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
            self.layered_moved(outer, turn, moved, |ui| match &element.kind {
                Kind::Image(image) => ui.scene_image(image, rect, inner, &facts, theme),
                Kind::Text(text) => {
                    let first = text.scroll.then_some(app.details_scroll);
                    ui.scene_text(text, (rect, first), inner, &facts, theme)
                }
                Kind::Rating(stars) => match facts.stars {
                    Some(rating) => ui.stars(stars, rating, rect, inner, &facts, theme),
                    None => Ok(()),
                },
                Kind::Fields(fields) => ui.scene_fields(fields, rect, inner, &facts, theme),
                Kind::Fill(fill) => ui.scene_fill(fill, rect, inner),
                Kind::Frame(slices) => ui.scene_frame(slices, rect, &facts),
                Kind::List(ink) => ui.scene_list(
                    app,
                    backdrop,
                    (rows, cursor),
                    data,
                    (theme, ink, &facts),
                    art,
                    layout,
                ),
                Kind::Collection(c) => ui.collection(c, &spots, rect, theme),
                Kind::Video(video) => ui.scene_video(video, (rect, inner), &facts, theme, art),
                Kind::Hints(hints) => ui.scene_hints(app, data, theme, layout, (hints, rect)),
                Kind::Status(plain) => ui.status_pill(theme, Some(rect), *plain).map(|_| ()),
                Kind::Actions => match facts.game {
                    Some(game) => {
                        let buttons = ui.details_buttons(app, game);
                        ui.details_actions((&buttons, app.details_focus), rect, theme)
                    }
                    None => Ok(()),
                },
                Kind::Panel(panel) => {
                    let panel = crate::layout::Panel {
                        rect: rect01,
                        ..*panel
                    };
                    ui.layout_panel("panel", &panel, theme).map(|_| ())
                }
            })?;
        }
        Ok(())
    }

    /// Each shown element's pose this frame: the screen comes in when it
    /// was not drawn the frame before or is another screen (`enter`); the
    /// focus's item comes in when it moves, unless it moved again too soon
    /// after the last (`select`).
    fn poses(
        &mut self,
        scene: &Scene,
        shown: &[&Element],
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
    ) -> Vec<Pose> {
        let (now, motion) = (self.frame.now, self.motion);
        let key = collection::key(backdrop);
        let item = format!("{cursor}:{}", rows.get(cursor).map_or("", String::as_str));
        let longest = |pick: fn(&Element) -> Option<Anim>| {
            scene
                .elements
                .iter()
                .filter_map(pick)
                .map(|a| span(&a, motion))
                .max()
                .unwrap_or_default()
        };
        let spans = (longest(|e| e.enter), longest(|e| e.select));
        let show = &mut self.scene_cache.show;
        show.spans = spans;
        let animated = motion != Motion::Off;
        if !show.before || show.key != key {
            show.key = key;
            show.item = item;
            show.entered = (animated && !spans.0.is_zero()).then_some(now);
            (show.selected, show.moved) = (None, Some(now));
        } else if show.item != item {
            show.item = item;
            let soon = show
                .moved
                .is_some_and(|at| now.saturating_duration_since(at) < DEBOUNCE);
            show.selected = (animated && !soon && !spans.1.is_zero()).then_some(now);
            show.moved = Some(now);
        }
        show.drawn = true;
        let screen = (self.frame.width as f32, self.frame.height as f32);
        let secs =
            |at: Option<Instant>| at.map(|at| now.saturating_duration_since(at).as_secs_f32());
        let (entered, selected) = (secs(show.entered), secs(show.selected));
        shown
            .iter()
            .map(|e| {
                let mut at = Pose::REST;
                if let (Some(anim), Some(secs)) = (&e.enter, entered) {
                    at = at.then(pose(anim, secs, motion, screen));
                }
                if let (Some(anim), Some(secs)) = (&e.select, selected) {
                    at = at.then(pose(anim, secs, motion, screen));
                }
                at
            })
            .collect()
    }

    /// Where `element` draws in `pose`: its box moved and grown when it
    /// draws to fit any box, else through a layer that is.
    fn placed(&self, element: &Element, pose: Pose) -> Placed {
        // Text and the launcher's pieces keep their size in a larger box.
        let fixed = matches!(
            element.kind,
            Kind::Text(_)
                | Kind::Fields(_)
                | Kind::Rating(_)
                | Kind::Collection(_)
                | Kind::Hints(_)
                | Kind::Status(_)
                | Kind::Actions
        );
        // The list is placed by the layout, not by its box.
        let through = pose.moved()
            && (matches!(element.kind, Kind::List(_)) || fixed && (pose.scale - 1.).abs() > 1e-3);
        let (rect01, moved) = if through {
            let rect = self.area(element.rect);
            let middle = (rect.center().x(), rect.center().y());
            (element.rect, Some((pose.dx, pose.dy, pose.scale, middle)))
        } else {
            let r = element.rect;
            let (w, h) = (r.w * pose.scale, r.h * pose.scale);
            let rect01 = crate::theme::Rect01 {
                x: r.x + (r.w - w) / 2. + pose.dx / self.frame.width as f32,
                y: r.y + (r.h - h) / 2. + pose.dy / self.frame.height as f32,
                w,
                h,
            };
            (rect01, None)
        };
        // Pictures, text and boxes take an opacity as they draw; the
        // rest, and anything turned or moved whole, through a layer.
        let simple = matches!(
            element.kind,
            Kind::Image(_)
                | Kind::Text(_)
                | Kind::Rating(_)
                | Kind::Fields(_)
                | Kind::Collection(_)
                | Kind::Video(_)
                | Kind::Fill(Fill { gradient: None, .. })
        );
        let direct = simple && element.rotate == 0. && moved.is_none();
        let (mut outer, mut inner) = if direct {
            (1., element.opacity)
        } else {
            (element.opacity, 1.)
        };
        // A collection's opacity is its items' out of focus: it fades in
        // whole through the layer.
        if direct && !matches!(element.kind, Kind::Collection(_)) {
            inner *= pose.alpha;
        } else {
            outer *= pose.alpha;
        }
        Placed {
            rect: self.area(rect01),
            rect01,
            outer,
            inner,
            moved,
            hidden: pose.alpha < 0.004,
        }
    }

    /// The pictures of `shown` elements, each one's sources in order.
    fn wanted<'e>(
        &self,
        shown: impl Iterator<Item = &'e Element>,
        facts: &Facts<'_>,
    ) -> Vec<Vec<PathBuf>> {
        shown
            .filter_map(|e| match &e.kind {
                Kind::Image(image) => Some(self.sources(image, facts)),
                Kind::Video(Video {
                    still: Some(image), ..
                }) => Some(self.sources(image, facts)),
                _ => None,
            })
            .filter(|paths| !paths.is_empty())
            .collect()
    }

    /// `system`'s logo in the theme, picture and clip, as files that exist.
    fn system_pictures(&mut self, system: &str) -> [Option<PathBuf>; 3] {
        if let Some(found) = self.scene_cache.systems.get(system) {
            return found.clone();
        }
        let first = |files: Vec<PathBuf>| files.into_iter().find(|f| f.is_file());
        let found = [
            first(self.logo_files(system)),
            first(self.system_files(system)),
            first(self.clip_files(system)),
        ];
        self.scene_cache
            .systems
            .insert(system.to_owned(), found.clone());
        found
    }

    fn facts<'d>(
        &mut self,
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
        data: &'d dyn DataSource,
        theme: &Theme,
    ) -> Facts<'d> {
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
            // Its place among the system's games.
            Screen::Details { system, game: at } => {
                let games = data.games(system);
                let game = games.get(*at);
                let item = game
                    .and_then(|g| g.path.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (system.clone(), game, at + 1, games.len(), item)
            }
            _ => {
                let system = rows.get(cursor).cloned().unwrap_or_default();
                (system.clone(), None, cursor + 1, rows.len(), system)
            }
        };
        let [logo, image, clip] = self.system_pictures(&system);
        let catalog = &self.catalog;
        let (maker, year) = data.manufacturer(&system).unwrap_or_default();
        let count = system_count(&system, data);
        let rating = game
            .and_then(|g| g.details.rating)
            .map_or_else(String::new, |r| {
                format!("{:.1}", f32::from(r) / 20.).replace('.', &catalog.text("details.decimal"))
            });
        let item_name = match backdrop {
            Screen::Games { system, folder } => App::level(data, system, folder)
                .and_then(|l| l.folder(cursor).map(str::to_owned))
                .or_else(|| game.map(|g| g.name.clone()))
                .unwrap_or_default(),
            Screen::Details { .. } => game.map(|g| g.name.clone()).unwrap_or_default(),
            _ => system_title(&system, data, catalog),
        };
        Facts {
            item_name,
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
            stars: game.and_then(|g| g.details.rating),
            logo,
            image,
            clip,
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
            let cache = CoverCache::within(
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
        self.files(&image.src, facts)
    }
    /// The files `src` names for what `facts` hold, in order.
    fn files(&self, src: &[Template], facts: &Facts<'_>) -> Vec<PathBuf> {
        src.iter()
            .filter_map(|template| match template.0.as_slice() {
                [crate::layout::Piece::Data(data)] if data.file() => facts.picture(*data),
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
            Some(text) if missing => self.scene_text(text, (rect, None), opacity, facts, theme),
            _ => Ok(()),
        }
    }

    /// The clip the host plays for this element, once it is the one
    /// asked for; its still until then (and without a clip, or with
    /// videos off). The clip asked for is left in `scene_clip`.
    fn scene_video(
        &mut self,
        video: &Video,
        (rect, opacity): (Rect, f32),
        facts: &Facts<'_>,
        theme: &Theme,
        art: Art<'_, '_>,
    ) -> Result<(), String> {
        let file = self
            .files(&video.src, facts)
            .into_iter()
            .find(|f| f.is_file());
        if let Some(file) = file {
            let playing = art.video.filter(|_| art.video_file == Some(file.as_path()));
            self.scene_clip = Some(Clip {
                file,
                size: clip_size(rect),
                fit: video.fit,
                delay: Duration::from_secs_f32(video.delay),
                repeat: video.repeat,
            });
            if let Some((frame, _)) = playing {
                let radius = video.radius * self.frame.height as f32;
                let alpha = (opacity * 255.).round() as u8;
                return self
                    .geometry
                    .cover(&mut self.canvas, frame, rect, radius, alpha, WHOLE);
            }
        }
        match &video.still {
            Some(still) => self.scene_image(still, rect, opacity, facts, theme),
            None => Ok(()),
        }
    }

    /// `first`: the line a scrolling text starts at (`long_text`).
    fn scene_text(
        &mut self,
        t: &Text,
        (rect, first): (Rect, Option<usize>),
        opacity: f32,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let text = t.text.fill(|d| facts.value(d), |k| self.phrase(k));
        let ink = (t.color.unwrap_or(theme.text), opacity);
        match first {
            Some(first) => self.scene_scroll(&text, t, rect, ink, first),
            None => self.scene_words(&text, t, rect, ink),
        }
    }

    /// `text` as `t` lays it in `rect`: its lines, its size and how many
    /// lines the box holds. A scrolling text's lines are kept.
    fn laid(
        &mut self,
        text: &str,
        t: &Text,
        rect: Rect,
    ) -> Result<(Vec<String>, f32, usize), String> {
        let text = if t.upper {
            text.to_uppercase()
        } else {
            text.to_owned()
        };
        let face = face(t.font);
        let mut size = t.size * self.frame.reference().1;
        if t.shrink && !t.wrap {
            let least = (size * 0.6).max(self.frame.min_size());
            size = self
                .shrink(&text, face, size, least, rect.width())?
                .unwrap_or(least);
        }
        let px = self.frame.font_size(size);
        let line = (f32::from(px) * t.line).round().max(1.) as i32;
        let room = if t.wrap {
            (rect.height() as i32 / line).max(1) as usize
        } else {
            1
        };
        let key = (text, rect.width(), px, t.font);
        if let Some((_, lines)) = self
            .scene_cache
            .scroll
            .as_ref()
            .filter(|(k, _)| t.scroll && *k == key)
        {
            return Ok((lines.clone(), size, room));
        }
        let lines = if t.wrap {
            self.wrap(&key.0, face, size, rect.width())?
        } else {
            vec![key.0.clone()]
        };
        if t.scroll {
            self.scene_cache.scroll = Some((key, lines.clone()));
        }
        Ok((lines, size, room))
    }

    /// A text longer than its box from line `first` on (never past its
    /// last page); with more below, its last line fades.
    fn scene_scroll(
        &mut self,
        text: &str,
        t: &Text,
        rect: Rect,
        (ink, opacity): (Rgba, f32),
        first: usize,
    ) -> Result<(), String> {
        let (lines, size, room) = self.laid(text, t, rect)?;
        let first = first.min(lines.len().saturating_sub(room));
        let shown: Vec<String> = lines.iter().skip(first).take(room).cloned().collect();
        let more = first + room < lines.len();
        let fade = |k: f32| (ink, opacity * k);
        for (n, line) in shown.iter().enumerate() {
            let last = more && n + 1 == shown.len();
            let one = vec![line.clone()];
            self.scene_lines(one, (t, size, n), rect, fade(if last { 0.45 } else { 1. }))?;
        }
        Ok(())
    }

    /// `text` written as `t` says, in `ink`.
    fn scene_words(
        &mut self,
        text: &str,
        t: &Text,
        rect: Rect,
        (ink, opacity): (Rgba, f32),
    ) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let (mut lines, size, room) = self.laid(text, t, rect)?;
        if lines.len() > room {
            lines.truncate(room);
            if let Some(last) = lines.last_mut() {
                last.push('…');
            }
        }
        self.scene_lines(lines, (t, size, 0), rect, (ink, opacity))
    }

    /// `lines` of `t` at `size`, the first in the box's line `at`.
    fn scene_lines(
        &mut self,
        lines: Vec<String>,
        (t, size, at): (&Text, f32, usize),
        rect: Rect,
        (ink, opacity): (Rgba, f32),
    ) -> Result<(), String> {
        let face = face(t.font);
        let px = self.frame.font_size(size);
        let line = (f32::from(px) * t.line).round().max(1.) as i32;
        let block = line * lines.len() as i32;
        let top = match t.valign {
            _ if t.scroll => rect.y() + line * at as i32,
            VAlign::Top => rect.y(),
            VAlign::Middle => rect.y() + (rect.height() as i32 - block) / 2,
            VAlign::Bottom => rect.bottom() - block,
        };
        let fade = |c: Rgba| Rgba {
            a: (f32::from(c.a) * opacity).round() as u8,
            ..c
        };
        let ink = fade(ink);
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

    /// A label and value per line, top down; a line without a value is
    /// left out and the next moves up.
    fn scene_fields(
        &mut self,
        fields: &Fields,
        rect: Rect,
        opacity: f32,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let fill = |t: &Template| t.fill(|d| facts.value(d), |k| self.phrase(k));
        let lines: Vec<(String, Option<String>)> = fields
            .rows
            .iter()
            .map(|row| (fill(&row.label), row.value.as_ref().map(fill)))
            .filter(|(_, value)| {
                !fields.hide_empty
                    || value
                        .as_ref()
                        .map_or(facts.stars.is_some(), |v| !v.trim().is_empty())
            })
            .collect();
        let tall = self.frame.height as f32;
        let step = fields
            .row
            .map_or(rect.height() as f32 / fields.rows.len() as f32, |r| {
                r * tall
            });
        let gap = self.px(12.);
        let label_ink = fields.label.color.unwrap_or(theme.muted);
        let value_ink = fields.value.color.unwrap_or(theme.text);
        for (n, (label, value)) in lines.iter().enumerate() {
            let (top, bottom) = (
                (rect.y() as f32 + step * n as f32).round() as i32,
                (rect.y() as f32 + step * (n + 1) as f32).round() as i32,
            );
            if bottom > rect.bottom() + 1 {
                break;
            }
            let line = Rect::new(rect.x(), top, rect.width(), (bottom - top).max(1) as u32);
            let shown = if fields.label.upper {
                label.to_uppercase()
            } else {
                label.clone()
            };
            let width = if shown.trim().is_empty() {
                0
            } else {
                let size = fields.label.size * self.frame.reference().1;
                self.measure(&shown, face(fields.label.font), size)? as i32 + gap
            };
            self.scene_words(label, &fields.label, line, (label_ink, opacity))?;
            let room = Rect::new(
                line.x() + width,
                line.y(),
                (line.width() as i32 - width).max(1) as u32,
                line.height(),
            );
            match (value, facts.stars) {
                (Some(value), _) => {
                    self.scene_words(value, &fields.value, room, (value_ink, opacity))?;
                }
                (None, Some(rating)) => {
                    self.stars(&fields.stars, rating, room, opacity, facts, theme)?;
                }
                (None, None) => {}
            }
            if let Some(c) = fields.divider {
                let a = (f32::from(c.a) * opacity).round() as u8;
                self.divider(line.x(), line.bottom() - 1, line.width(), Rgba { a, ..c })?;
            }
        }
        Ok(())
    }

    /// Five stars for `rating` (0 to 100, in halves) in `rect`.
    fn stars(
        &mut self,
        stars: &Stars,
        rating: u8,
        rect: Rect,
        opacity: f32,
        facts: &Facts<'_>,
        theme: &Theme,
    ) -> Result<(), String> {
        let tall = self.frame.height as f32;
        let side = stars
            .size
            .map_or(rect.height() as f32, |s| s * tall)
            .round()
            .max(1.) as i32;
        let gap = stars.spacing.map_or(side as f32 / 5., |s| s * tall).round() as i32;
        let width = 5 * side + 4 * gap;
        let left = match stars.align {
            Align::Left => rect.x(),
            Align::Right => rect.right() - width,
            Align::Center => rect.x() + (rect.width() as i32 - width) / 2,
        };
        let y = rect.y() + (rect.height() as i32 - side) / 2;
        let file = |t: &Option<Template>| {
            let path = t.as_ref()?.fill(|d| facts.value(d), |k| self.phrase(k));
            self.theme_file(&path)
        };
        let (full, half, empty) = (file(&stars.full), file(&stars.half), file(&stars.empty));
        // The launcher's star is white: the theme's text unless colored.
        let color = stars.color.or(full.is_none().then_some(theme.text));
        let alpha = |k: f32| k * opacity;
        let halves = ((u32::from(rating) + 5) / 10).min(10);
        for n in 0..5 {
            let cell = Rect::new(left + n * (side + gap), y, side as u32, side as u32);
            let left = halves.saturating_sub(2 * n as u32).min(2);
            let whole_half = left == 1 && half.is_some();
            if left < 2 && !whole_half {
                match &empty {
                    Some(path) => {
                        let ink = stars.empty_color;
                        self.star(Some(path), cell, 1., ink, alpha(stars.empty_opacity))?;
                    }
                    None => {
                        let ink = stars.empty_color.or(color);
                        self.star(full.as_deref(), cell, 1., ink, alpha(stars.empty_opacity))?;
                    }
                }
            }
            match (left, &half) {
                (2, _) => self.star(full.as_deref(), cell, 1., color, alpha(1.))?,
                (1, Some(path)) => self.star(Some(path), cell, 1., color, alpha(1.))?,
                (1, None) => self.star(full.as_deref(), cell, 0.5, color, alpha(1.))?,
                _ => {}
            }
        }
        Ok(())
    }

    /// One star (the theme's picture, or the launcher's when `path` is
    /// None) fitted in `cell`, `part` of it from the left.
    fn star(
        &mut self,
        path: Option<&Path>,
        cell: Rect,
        part: f32,
        tint: Option<Rgba>,
        opacity: f32,
    ) -> Result<(), String> {
        let found = match path {
            Some(path) => self.picture(path),
            None => self.launcher_star(cell.width().max(cell.height())),
        };
        let Some((texture, canvas)) = found else {
            return Ok(());
        };
        let size = texture.query();
        let (tw, th) = (size.width.max(1) as f32, size.height.max(1) as f32);
        let k = (cell.width() as f32 / tw).min(cell.height() as f32 / th);
        let (w, h) = ((tw * k).round().max(1.), (th * k).round().max(1.));
        let dst = Rect::new(
            cell.x() + ((cell.width() as f32 - w) / 2.).round() as i32,
            cell.y() + ((cell.height() as f32 - h) / 2.).round() as i32,
            (w * part).round().max(1.) as u32,
            h as u32,
        );
        let src = Rect::new(0, 0, (tw * part).round().max(1.) as u32, th as u32);
        let mut alpha = opacity;
        if let Some(t) = tint {
            texture.set_color_mod(t.r, t.g, t.b);
            alpha *= f32::from(t.a) / 255.;
        }
        texture.set_alpha_mod((alpha * 255.).round() as u8);
        let drawn = canvas.copy(texture, src, dst);
        texture.set_alpha_mod(255);
        texture.set_color_mod(255, 255, 255);
        drawn
    }

    /// The launcher's star, `side` pixels square.
    fn launcher_star(&mut self, side: u32) -> Option<(&mut Texture<'t>, &mut Canvas<Window>)> {
        let stars = &mut self.scene_cache.stars;
        if !stars.iter().any(|(s, _)| *s == side) {
            let creator = self.creator?;
            let image = crate::icons::rasterize_picture(STAR_SVG.as_bytes(), side).ok()?;
            let mut texture = creator
                .create_texture_static(
                    sdl2::pixels::PixelFormatEnum::RGBA32,
                    image.width(),
                    image.height(),
                )
                .ok()?;
            texture
                .update(None, image.as_raw(), image.width() as usize * 4)
                .ok()?;
            texture.set_blend_mode(BlendMode::Blend);
            if stars.len() >= 4 {
                stars.remove(0);
            }
            stars.push((side, texture));
        }
        let texture = stars.iter_mut().find(|(s, _)| *s == side)?;
        Some((&mut texture.1, &mut self.canvas))
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
        match self.theme_file(&path) {
            Some(path) => self.slices(&path, slices, rect),
            None => Ok(()),
        }
    }
    /// The theme picture at `path` in nine slices over `rect`.
    pub(super) fn slices(
        &mut self,
        path: &Path,
        slices: &Slices,
        rect: Rect,
    ) -> Result<(), String> {
        let k = (slices.scale * self.frame.scale).round().max(1.) as u32;
        if slices.tile && self.tiled(path, slices, rect, k)? {
            return Ok(());
        }
        let Some((texture, canvas)) = self.picture(path) else {
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

    /// A tiled picture over `rect` in one copy: its tiles are laid once in
    /// a texture of the box's size, kept while the box and the picture stay.
    /// False when no texture can be drawn into (the caller tiles as before).
    fn tiled(&mut self, path: &Path, slices: &Slices, rect: Rect, k: u32) -> Result<bool, String> {
        // A plain pattern (no edges) is the same tiled into any box from its
        // corner: one texture per picture, at the largest box asked, of which
        // each frame copies its box's part. A box that grows or shrinks (an
        // entrance that scales) then makes no new texture every frame.
        let plain = slices.slice == [0; 4];
        let fits = |kept: &TileKey| {
            kept.0 == path
                && kept.2 == k
                && kept.3 == slices.slice
                && if plain {
                    kept.1 .0 >= rect.width() && kept.1 .1 >= rect.height()
                } else {
                    kept.1 == (rect.width(), rect.height())
                }
        };
        if let Some(at) = self
            .scene_cache
            .tiled
            .iter()
            .position(|(kept, _)| fits(kept))
        {
            let entry = self.scene_cache.tiled.remove(at);
            let part = Rect::new(0, 0, rect.width(), rect.height());
            self.canvas.copy(&entry.1, part, rect)?;
            self.scene_cache.tiled.push(entry);
            return Ok(true);
        }
        // Made a little larger than asked when plain, so a box growing a
        // few pixels at a time does not make one each frame.
        let size = if plain {
            let (w, h) = (
                self.frame.width.max(rect.width()),
                self.frame.height.max(rect.height()),
            );
            (w + w / 8, h + h / 8)
        } else {
            (rect.width(), rect.height())
        };
        // The older one of this picture goes: this one covers it.
        self.scene_cache.tiled.retain(|(kept, _)| {
            !(plain && kept.0 == path && kept.2 == k && kept.3 == slices.slice)
        });
        let key: TileKey = (path.to_path_buf(), size, k, slices.slice);
        let Some(mut laid) = self.target_texture(size) else {
            return Ok(false);
        };
        let Some((texture, canvas)) = self.picture(path) else {
            // Not loaded yet: nothing drawn, as when tiled by hand.
            return Ok(true);
        };
        let tile = texture.query();
        // SAFETY: the renderer is alive; the target now drawn into (the page
        // texture, or the window when null) is put back below.
        let before = unsafe { sdl2::sys::SDL_GetRenderTarget(canvas.raw()) };
        if !super::effects::target(Some(&laid), canvas) {
            return Ok(false);
        }
        let ink = canvas.draw_color();
        canvas.set_draw_color(sdl2::pixels::Color::RGBA(0, 0, 0, 0));
        canvas.clear();
        canvas.set_draw_color(ink);
        // The tiles' own pixels, alpha included, not blended over the clear.
        texture.set_blend_mode(sdl2::render::BlendMode::None);
        let local = Rect::new(0, 0, size.0, size.1);
        let mut laid_out = Ok(());
        for (src, to) in nine_slice((tile.width, tile.height), slices.slice, k, local, true) {
            laid_out = canvas.copy(texture, src, to);
            if laid_out.is_err() {
                break;
            }
        }
        texture.set_blend_mode(sdl2::render::BlendMode::Blend);
        // SAFETY: as above; `before` came from this renderer.
        unsafe { sdl2::sys::SDL_SetRenderTarget(canvas.raw(), before) };
        laid_out?;
        laid.set_blend_mode(sdl2::render::BlendMode::Blend);
        let part = Rect::new(0, 0, rect.width(), rect.height());
        self.canvas.copy(&laid, part, rect)?;
        self.scene_cache.tiled.push((key, laid));
        if self.scene_cache.tiled.len() > TILED_KEPT {
            self.scene_cache.tiled.remove(0);
        }
        Ok(true)
    }

    /// The screen's list where the scene puts it, in its colors.
    #[allow(clippy::too_many_arguments)]
    fn scene_list(
        &mut self,
        app: &App,
        backdrop: &Screen,
        (rows, cursor): (&[String], usize),
        data: &dyn DataSource,
        (theme, ink, facts): (&Theme, &ListInk, &Facts<'_>),
        art: Art<'_, '_>,
        layout: &Layout,
    ) -> Result<(), String> {
        let inked;
        let theme = if ink.colors() == [None; 3] {
            theme
        } else {
            if self
                .scene_cache
                .list
                .as_ref()
                .is_none_or(|(i, _)| i.colors() != ink.colors())
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
                self.scene_cache.list = Some((ink.clone(), own));
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
        let style = &ink.style;
        let frame = style.frame.as_ref().and_then(|slices| {
            let path = slices.image.fill(|d| facts.value(d), |k| self.phrase(k));
            Some((self.theme_file(&path)?, slices.clone()))
        });
        self.list_look = Some(ListLook {
            face: style.font.map(face),
            align: style.align,
            frame,
        });
        let drawn = match backdrop {
            Screen::Games { system, folder } if layout.list.mark == Mark::Grid => {
                self.games_grid((system, folder), cursor, (data, None), theme, art, layout)
            }
            _ => self.list(backdrop, (rows, lifted), cursor, data, theme, layout),
        };
        self.list_look = None;
        drawn
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

/// A free screen's list beyond its colors: the face, where the names sit
/// and the selected row's picture, as `list` draws them.
pub(super) struct ListLook {
    pub face: Option<Face>,
    pub align: Option<Align>,
    pub frame: Option<(PathBuf, Slices)>,
}

fn face(font: Font) -> Face {
    match font {
        Font::Regular => Face::Regular,
        Font::Semibold => Face::Semibold,
        Font::Bold => Face::Bold,
        Font::Mono => Face::Mono,
        Font::File(n) => Face::File(n),
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
