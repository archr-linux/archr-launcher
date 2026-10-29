//! Grafite presentation at 640×480, scaled by height, with event-driven idle.
pub use crate::text::PreparedText;
use crate::{
    i18n::I18n,
    icons::Glyph,
    layout::{Backdrop, Header, Layout, List, Mark, Scroll, View},
    motion::{Motion, Tween, CROSSFADE, SELECT},
    primitives::{crop, gradient, Geometry, Shadow, WHOLE},
    startup::StartupTrace,
    startup_phase,
    state::{App, DataSource, QuickItem, Screen},
    status::{Battery, Status},
    text::{color, Align, Face, Style, TextCache},
    theme::{Rect01, Rgba, Theme},
    HintButton,
};
pub use details::Art;
mod details;
mod effects;
pub(crate) use effects::Pop;
mod hub;
mod keyboard;
mod moonlight;
mod netplay;

use sdl2::{
    rect::Rect,
    render::{BlendMode, Canvas, Texture, TextureCreator},
    ttf::Sdl2TtfContext,
    video::{Window, WindowContext},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod system_art;
use system_art::SystemArt;

#[derive(Clone, Copy)]
struct Frame {
    width: u32,
    height: u32,
    scale: f32,
    /// User text-size multiplier, applied to every font size.
    text: f32,
    now: Instant,
    unix: u64,
}
impl Frame {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            scale: height as f32 / 480.,
            text: 1.,
            now: Instant::now(),
            unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    fn from_size((width, height): (u32, u32)) -> Self {
        Self::new(width, height)
    }
    fn px(self, value: f32) -> i32 {
        (value * self.scale).round().max(1.) as i32
    }
    fn row(self, panel: Rect, inset: f32, y: f32, height: f32) -> Rect {
        Rect::new(
            panel.x() + self.px(inset),
            panel.y() + self.px(y),
            panel
                .width()
                .saturating_sub(self.px(inset * 2.) as u32)
                .max(1),
            self.px(height) as u32,
        )
    }
    fn square(self, x: i32, y: i32, side: f32) -> Rect {
        Rect::new(x, y, self.px(side) as u32, self.px(side) as u32)
    }
    fn font_size(self, value: f32) -> u16 {
        self.px(value * self.text).clamp(8, 128) as u16
    }
}
fn localized_error(catalog: &I18n, error: String) -> String {
    if error.starts_with("error.") {
        catalog.text(&error)
    } else {
        error
    }
}

fn selection<T>(selected: bool, normal: T, highlighted: T) -> T {
    if selected {
        highlighted
    } else {
        normal
    }
}

/// List offset of row `i` when `cursor` is the selected row: rows below it
/// make room for the taller pill, and rows after the collections for the divider.
fn slot(
    i: usize,
    cursor: usize,
    collections: usize,
    (pitch, divider, extra): (f32, f32, f32),
) -> f32 {
    i as f32 * pitch
        + if collections > 0 && i >= collections {
            divider
        } else {
            0.
        }
        + if i > cursor { extra } else { 0. }
}

/// Side of the Lucide chevrons that flank a lifted row.
const CHEVRON: f32 = 18.;

/// "Name (detail)" without its trailing parenthetical, when it has one.
fn without_detail(name: &str) -> &str {
    name.strip_suffix(')')
        .and_then(|s| s.rsplit_once(" ("))
        .map_or(name, |(short, _)| short)
}

/// How much of the move status survives: everything, a compact position, or
/// the name alone.
#[derive(Clone, Copy)]
enum MoveStatus {
    Full,
    Short,
    Name,
}

fn status_text(
    catalog: &I18n,
    name: &str,
    (n, total): (usize, usize),
    status: MoveStatus,
) -> String {
    let key = match status {
        MoveStatus::Full => "move.status",
        MoveStatus::Short => "move.status_short",
        MoveStatus::Name => return name.to_owned(),
    };
    catalog.format(
        key,
        &[
            ("name", name),
            ("n", &n.to_string()),
            ("total", &total.to_string()),
        ],
    )
}

/// Status text is never ellipsized: full, compact, then the name without its
/// parenthetical, shrinking to 10px, and finally without the position.
fn fit_status(
    name: &str,
    width: u32,
    mut form: impl FnMut(&str, MoveStatus) -> String,
    mut measure: impl FnMut(&str, f32) -> Result<u32, String>,
) -> Result<(String, f32), String> {
    let short = without_detail(name);
    let tries = [
        (name, MoveStatus::Full, 13.),
        (name, MoveStatus::Short, 13.),
    ]
    .into_iter()
    .chain([
        (short, MoveStatus::Full, 13.),
        (short, MoveStatus::Short, 13.),
    ])
    .chain([12., 11., 10.].map(|size| (short, MoveStatus::Short, size)))
    .chain([13., 12., 11., 10.].map(|size| (short, MoveStatus::Name, size)));
    let mut last = (String::new(), 10.);
    for (name, status, size) in tries {
        last = (form(name, status), size);
        if measure(&last.0, size)? <= width {
            break;
        }
    }
    Ok(last)
}

/// Keep system rows single-line; parenthetical detail remains in the artwork panel.
fn fit_system_name(
    name: &str,
    width: u32,
    (mut size, minimum): (f32, f32),
    mut measure: impl FnMut(&str, f32) -> Result<u32, String>,
) -> Result<(&str, f32), String> {
    if measure(name, size)? <= width {
        return Ok((name, size));
    }
    let name = without_detail(name);
    while size > minimum && measure(name, size)? > width {
        size = (size - 1.).max(minimum);
    }
    Ok((name, size))
}

struct ListMotion {
    key: String,
    scroll: Tween,
    pill: Tween,
    /// Cursor of the previous frame while a row is lifted, and the neighbour
    /// it displaced, which glides into its new place.
    held: Option<(usize, usize)>,
    swap_row: usize,
    swap: Tween,
}
impl ListMotion {
    fn new() -> Self {
        Self {
            key: String::new(),
            scroll: Tween::new(0., SELECT),
            pill: Tween::new(0., SELECT),
            held: None,
            swap_row: 0,
            swap: Tween::new(0., SELECT),
        }
    }
}
struct StatusLayout {
    pill: Rect,
    wifi: Option<Rect>,
    clock: Rect,
    battery: Option<Rect>,
    percent: Option<Rect>,
}

fn status_layout(
    right: i32,
    header_height: u32,
    scale: f32,
    clock_width: u32,
    percent_width: Option<u32>,
    wifi: bool,
) -> StatusLayout {
    let px = |v: f32| (v * scale).round().max(1.) as u32;
    let width = px(12.) * 2
        + clock_width
        + if wifi { px(16.) + px(12.) } else { 0 }
        + percent_width.map_or(0, |w| px(12.) + px(24.) + px(6.) + w);
    let center = header_height as i32 / 2;
    let pill = Rect::new(
        right - width as i32,
        center - px(26.) as i32 / 2,
        width,
        px(26.),
    );
    let mut x = pill.x() + px(12.) as i32;
    let mut panel = |width: Option<u32>, height: u32, gap: u32| {
        width.map(|w| {
            let rect = Rect::new(x, center - height as i32 / 2, w, height);
            x += (w + gap) as i32;
            rect
        })
    };
    StatusLayout {
        pill,
        wifi: panel(wifi.then_some(px(16.)), px(16.), px(12.)),
        clock: panel(Some(clock_width), px(16.), px(12.)).expect("clock is always visible"),
        battery: panel(percent_width.map(|_| px(24.)), px(24.), px(6.)),
        percent: panel(percent_width, px(16.), 0),
    }
}

fn battery_glyph(battery: Battery) -> Glyph {
    if battery.charging {
        Glyph::BatteryCharging
    } else {
        match battery.percent {
            0 => Glyph::Battery,
            1..=25 => Glyph::BatteryLow,
            26..=69 => Glyph::BatteryMedium,
            _ => Glyph::BatteryFull,
        }
    }
}

#[path = "settings/render.rs"]
mod settings_render;

pub struct Ui<'font, 'texture> {
    startup: StartupTrace,
    pub sdl: sdl2::Sdl,
    pub canvas: Canvas<Window>,
    text: Option<TextCache<'font, 'texture>>,
    shadow: Option<Shadow<'texture>>,
    creator: Option<&'texture TextureCreator<WindowContext>>,
    art: Option<SystemArt<'texture>>,
    root: Option<PathBuf>,
    geometry: Geometry,
    status: Option<Status>,
    list: ListMotion,
    menu_pill: Tween,
    settings_scroll: settings_render::Scroll,
    settings_banner: settings_render::Banner,
    menu_open: bool,
    /// Each settings switch's last state and its knob's position.
    switches: std::collections::BTreeMap<crate::settings::Id, (bool, Tween)>,
    cover_key: Option<PathBuf>,
    cover_visible: bool,
    cover_fade: Tween,
    details_wrap: details::Wrapped,
    /// Description lines below the details view, for the host's scroll limit.
    pub details_overflow: usize,
    /// The active view and the theme it was resolved from.
    view: Option<(Theme, View)>,
    animating: bool,
    /// Look-and-feel choices: text-size multiplier and how much things move.
    pub text_scale: f32,
    motion: Motion,
    effects: effects::Effects<'texture>,
    has_presented: bool,
    frame: Frame,
    pub catalog: I18n,
    pub loading: bool,
    pub banner: Option<String>,
    pub screenshot: Option<PathBuf>,
    /// Open Netplay lobby; it replaces the browsing screens while present.
    pub lobby: Option<crate::netplay::Lobby>,
    pub transition: Option<String>,
    pub moonlight: Option<crate::moonlight::State>,
    /// Active theme directory; its `systems/` folder holds console images.
    pub theme_dir: Option<PathBuf>,
    /// Root-mapped installed default theme: icons and console images the
    /// selected theme does not bring come from here.
    pub default_theme_dir: Option<PathBuf>,
}
impl<'font, 'texture> Ui<'font, 'texture> {
    pub fn new(width: u32, height: u32, windowed: bool) -> Result<Self, String> {
        Self::new_traced(width, height, windowed, StartupTrace::default())
    }
    pub fn new_traced(
        width: u32,
        height: u32,
        windowed: bool,
        trace: StartupTrace,
    ) -> Result<Self, String> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        startup_phase!(trace, "SDL_Init video");
        Self::with_video(sdl, video, (width, height), windowed, trace)
    }
    pub fn with_video(
        sdl: sdl2::Sdl,
        video: sdl2::VideoSubsystem,
        (width, height): (u32, u32),
        windowed: bool,
        trace: StartupTrace,
    ) -> Result<Self, String> {
        let gl_window = video.current_video_driver() != "dummy";
        if gl_window {
            // Normal priority preserves an explicit SDL_RENDER_DRIVER override.
            sdl2::hint::set("SDL_RENDER_DRIVER", "opengles2");
            let gl = video.gl_attr();
            gl.set_context_profile(sdl2::video::GLProfile::GLES);
            gl.set_context_version(2, 0);
            gl.set_depth_size(0);
            gl.set_stencil_size(0);
            gl.set_multisample_buffers(0);
            gl.set_multisample_samples(0);
        }
        let mut builder = video.window("archr-launcher", width, height);
        builder.position_centered();
        if gl_window {
            builder.opengl();
        }
        if !windowed {
            builder.fullscreen_desktop();
        }
        let window = builder.build().map_err(|e| e.to_string())?;
        startup_phase!(trace, "window creation");
        let renderer = window.into_canvas();
        let mut canvas = if video.current_video_driver() == "dummy" {
            renderer.software()
        } else {
            renderer.accelerated().present_vsync()
        }
        .build()
        .map_err(|e| e.to_string())?;
        startup_phase!(trace, "renderer / GL context creation");
        canvas.set_blend_mode(BlendMode::Blend);
        Ok(Self {
            startup: trace,
            sdl,
            canvas,
            text: None,
            shadow: None,
            creator: None,
            art: None,
            root: None,
            geometry: Geometry::new(),
            status: None,
            list: ListMotion::new(),
            menu_pill: Tween::new(0., SELECT),
            settings_scroll: settings_render::Scroll::default(),
            settings_banner: settings_render::Banner::default(),
            menu_open: false,
            switches: std::collections::BTreeMap::new(),
            lobby: None,
            cover_key: None,
            cover_visible: false,
            cover_fade: Tween::new(0., CROSSFADE),
            details_wrap: details::Wrapped::default(),
            details_overflow: 0,
            view: None,
            animating: false,
            text_scale: 1.,
            motion: Motion::On,
            effects: effects::Effects::default(),
            has_presented: false,
            frame: Frame::new(width, height),
            catalog: I18n::default(),
            loading: false,
            banner: None,
            screenshot: None,
            transition: None,
            moonlight: None,
            theme_dir: None,
            default_theme_dir: None,
        })
    }
    pub fn configure(
        &mut self,
        ttf: &'font Sdl2TtfContext,
        creator: &'texture TextureCreator<WindowContext>,
        theme: &Theme,
        root: Option<&Path>,
    ) -> Result<(), String> {
        let mut prepared = PreparedText::new(ttf, font_paths(theme, root), self.startup);
        prepared.prepare_icons(
            self.theme_dir.as_deref(),
            &self.default_theme_dir.as_ref().map_or_else(
                || PathBuf::from(crate::icons::DEFAULT_ICONS_DIR),
                |dir| dir.join("icons"),
            ),
            [],
        );
        self.configure_prepared(creator, prepared, root)
    }
    pub fn configure_prepared(
        &mut self,
        creator: &'texture TextureCreator<WindowContext>,
        mut prepared: PreparedText<'font>,
        root: Option<&Path>,
    ) -> Result<(), String> {
        self.frame = Frame::from_size(self.canvas.output_size()?);
        self.frame.text = self.text_scale;
        self.creator = Some(creator);
        self.root = root.map(Path::to_path_buf);
        self.art = None;
        self.shadow = prepared
            .shadow
            .take()
            .map(|surface| Shadow::from_surface(creator, surface))
            .transpose()?;
        if self.shadow.is_some() {
            startup_phase!(self.startup, "shadow texture");
        }
        let text = TextCache::from_prepared(creator, prepared)?;
        self.text = Some(text);
        self.status = Some(Status::new(root, Instant::now()));
        startup_phase!(self.startup, "status initialization");
        Ok(())
    }
    pub fn wifi_enabled(&self) -> bool {
        self.status.as_ref().is_some_and(|s| s.wifi)
    }
    pub fn wifi_refresh_signal(&self) -> Option<impl FnOnce() + Send + 'static> {
        self.status.as_ref().and_then(|s| s.refresh_signal())
    }
    /// Called before applying input: finish old targets, never queue gestures.
    pub fn snap_animations(&mut self) {
        self.list.scroll.snap();
        self.list.pill.snap();
        self.list.swap.snap();
        self.menu_pill.snap();
        self.settings_scroll.tween.snap();
        self.settings_banner.fade.snap();
        for (_, switch) in self.switches.values_mut() {
            switch.snap();
        }
        self.cover_fade.snap();
        self.effects.snap();
    }
    pub fn motion(&self) -> Motion {
        self.motion
    }
    /// Full motion, short fades only, or everything instant.
    pub fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
        for tween in [
            &mut self.list.scroll,
            &mut self.list.pill,
            &mut self.list.swap,
            &mut self.menu_pill,
            &mut self.settings_scroll.tween,
        ] {
            tween.set_duration(motion.travel(SELECT));
        }
        self.cover_fade.set_duration(motion.fade(CROSSFADE));
        self.settings_banner
            .fade
            .set_duration(motion.fade(crate::motion::NOTICE));
        self.switches.clear();
    }
    pub fn animating(&self) -> bool {
        self.animating
    }
    /// Timer work, separate from rendering; sysfs refresh is rate limited.
    pub fn tick(&mut self) -> bool {
        // Artwork loading starts after the first frame, never before it.
        let created = self.art.is_none() && self.creator.is_some();
        self.ensure_art();
        let art = self.art.as_mut().is_some_and(SystemArt::tick);
        let status = self.status.as_mut().is_some_and(|s| s.poll(Instant::now()));
        created || art || status
    }
    fn px(&self, value: f32) -> i32 {
        self.frame.px(value)
    }
    /// The layout of `theme`'s active view, resolved once per theme change.
    fn view(&mut self, theme: &Theme) -> View {
        if self.view.as_ref().is_none_or(|(seen, _)| seen != theme) {
            self.view = Some((theme.clone(), theme.view(&theme.active_view)));
        }
        self.view.as_ref().expect("resolved view").1.clone()
    }
    /// A theme rectangle on this screen.
    fn area(&self, rect: Rect01) -> Rect {
        pixels(rect, self.frame.width, self.frame.height)
    }
    fn label(&mut self, text: &str, style: (Face, f32, Rgba), panel: Rect) -> Result<(), String> {
        self.label_aligned(text, style, panel, Align::Left)
    }
    fn label_aligned(
        &mut self,
        text: &str,
        (face, size, color): (Face, f32, Rgba),
        panel: Rect,
        align: Align,
    ) -> Result<(), String> {
        let style = Style {
            face,
            size: self.frame.font_size(size),
            color,
        };
        let text_cache = self
            .text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?;
        text_cache
            .draw(&mut self.canvas, text, style, panel, align)
            .map_err(|e| localized_error(&self.catalog, e))
    }
    fn packed_label(
        &mut self,
        text: &str,
        style: (Face, f32, Rgba),
        (right, y, h): (i32, i32, u32),
        cap: u32,
        align: Align,
    ) -> Result<i32, String> {
        let w = self.measure(text, style.0, style.1)?.min(cap);
        let left = right - w as i32;
        self.label_aligned(text, style, Rect::new(left, y, w, h), align)?;
        Ok(left)
    }
    fn measure(&mut self, text: &str, face: Face, size: f32) -> Result<u32, String> {
        let size = self.frame.font_size(size);
        self.text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?
            .measure(text, face, size)
            .map_err(|e| localized_error(&self.catalog, e))
    }
    fn rounded(
        &mut self,
        rect: Rect,
        radius: f32,
        fill: Rgba,
        border: Option<Rgba>,
    ) -> Result<(), String> {
        self.geometry
            .rounded(&mut self.canvas, rect, radius, fill, border)
    }
    fn panel(&mut self, rect: Rect, radius: f32, fill: Rgba, border: Rgba) -> Result<(), String> {
        if self.shadow.is_none() {
            if let Some(creator) = self.creator {
                self.shadow = Some(Shadow::new(creator)?);
                startup_phase!(self.startup, "shadow texture");
            }
        }
        if let Some(shadow) = &self.shadow {
            shadow.draw(&mut self.canvas, rect, self.frame.scale, radius)?;
        }
        self.rounded(rect, radius, fill, Some(border))
    }
    fn icon(&mut self, glyph: Glyph, panel: Rect, ink: Rgba) -> Result<(), String> {
        let text = self
            .text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?;
        text.draw_icon(&mut self.canvas, glyph, panel, ink)
    }
    fn divider(&mut self, x: i32, y: i32, w: u32, c: Rgba) -> Result<(), String> {
        self.canvas.set_draw_color(color(c));
        self.canvas.fill_rect(Rect::new(x, y, w, 1))
    }
    pub fn draw(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        cover: Option<&Texture<'_>>,
    ) -> Result<(), String> {
        let art = Art {
            cover,
            ..Art::default()
        };
        self.draw_with_settings(app, data, theme, art, None)
    }
    pub fn draw_with_settings(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        art: Art<'_, '_>,
        settings: Option<&crate::settings::Settings>,
    ) -> Result<(), String> {
        self.frame = Frame::from_size(self.canvas.output_size()?);
        self.frame.text = self.text_scale;
        let Frame {
            width, height, now, ..
        } = self.frame;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.clear();
        if let Some(lobby) = self.lobby.take() {
            let drawn = self.lobby_page(&lobby, theme);
            self.lobby = Some(lobby);
            drawn?;
            self.animating = false;
        } else if let Some(moonlight) = self.moonlight.take() {
            let result = self.moonlight_page(&moonlight, theme);
            self.moonlight = Some(moonlight);
            result?;
            self.animating = false;
        } else if let Some(settings) = settings {
            self.settings_page(settings, theme)?;
            self.animating = self.settings_scroll.tween.active(now)
                || self.settings_banner.busy(now)
                || self.switches.values().any(|(_, knob)| knob.active(now));
        } else {
            let (backdrop, cursor) = app.backdrop();
            let view = self.view(theme);
            // The details screen shares the games page: no slide between them.
            let games = !matches!(backdrop, Screen::Systems);
            let layout = view.screen(games);
            let rows = if matches!(backdrop, Screen::Systems) {
                app.rows(data)
            } else {
                Vec::new()
            };
            // The picture that crossfades when the selection changes.
            let picture = layout
                .art
                .filter(|_| layout.list.mark != Mark::Tile)
                .and_then(|picture| {
                    let key = match backdrop {
                        Screen::Games { system } => data
                            .games(system)
                            .get(cursor)
                            .map(|g| g.path.display().to_string()),
                        Screen::Systems => rows.get(cursor).cloned(),
                        _ => None,
                    };
                    key.map(|key| (key, self.area(picture.rect)))
                });
            self.begin_page(games, picture, theme)?;
            if let Screen::Details { system, game } = backdrop {
                let name = data
                    .games(system)
                    .get(*game)
                    .map_or("", |g| g.name.as_str());
                self.details_header(system, name, data, theme)?;
                self.details(app, (system, *game), data, theme, art)?;
                self.footer(app, data, theme, layout)?;
            } else {
                if let (Screen::Games { .. }, Some(wash)) = (backdrop, layout.backdrop) {
                    self.backdrop(art.cover, wash, theme)?;
                }
                self.header(backdrop, cursor, data, theme, layout)?;
                if matches!(app.screen, Screen::StartHub { .. }) {
                    self.hub(app, data, theme)?;
                } else {
                    let lifted = app.moving().is_some();
                    self.list(backdrop, (&rows, lifted), cursor, data, theme, layout)?;
                    // Secondary visuals follow the redraw requested by the artwork worker.
                    if self.has_presented && matches!(backdrop, Screen::Systems) {
                        self.system_panel(&rows, cursor, data, theme, layout)?;
                        if let Some(panel) = layout.panel.filter(|_| lifted) {
                            self.canvas.set_draw_color(color(Rgba {
                                a: 140,
                                ..theme.background
                            }));
                            self.canvas.fill_rect(self.area(panel.rect))?;
                        }
                    }
                    if let Screen::Games { system } = backdrop {
                        self.cover(system, cursor, data, theme, art.cover, layout)?;
                    }
                    if self.has_presented {
                        self.footer(app, data, theme, layout)?;
                    }
                }
            }
            self.end_page(theme)?;
            if matches!(app.screen, Screen::QuickMenu { .. }) {
                self.menu(app, data, theme)?;
            } else {
                self.menu_open = false;
            }
            self.animating = self.list.scroll.active(now)
                || self.list.pill.active(now)
                || self.list.swap.active(now)
                || self.menu_pill.active(now)
                || self.cover_fade.active(now);
        }
        // The launch overlay is the last frame before a handoff: never mid-fade.
        if let Some(message) = self.transition.clone() {
            self.scrim(theme, 1.0)?;
            self.notice_panel(&message, theme)?;
        } else if let Some(banner) = self.banner.clone() {
            self.notice(&banner, theme)?;
        }
        self.settle_entrances();
        self.animating |= self.effects.active(now);
        // Explicit debug readback; normal rendering does no disk I/O.
        if let Some(path) = self.screenshot.take() {
            let rgba = self
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)?;
            image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)
                .map_err(|e| e.to_string())?;
        }
        startup_phase!(self.startup, "first frame draw");
        self.canvas.present();
        self.has_presented = true;
        if self.startup.began().is_some() {
            self.startup.phase_once("first present");
            if let Some(text) = &mut self.text {
                text.trace_startup(StartupTrace::default());
            }
        }
        if let Some(status) = &mut self.status {
            status.start();
        }
        Ok(())
    }
    fn status_header(&mut self, theme: &Theme) -> Result<i32, String> {
        let Frame { width, height, .. } = self.frame;
        let h = (theme.header_height * height as f32).round() as u32;
        let pad = self.px(24.);
        let status = self
            .status
            .as_ref()
            .map(|s| (s.clock.clone(), s.battery, s.wifi));
        let mut right = width as i32 - pad;
        if let Some((clock, battery, wifi)) = status {
            let percent = battery.map(|b| format!("{}%", b.percent));
            let clock_width = self.measure(&clock, Face::Mono, 13.)?;
            let percent_width = percent
                .as_deref()
                .map(|s| self.measure(s, Face::Mono, 13.))
                .transpose()?;
            let layout =
                status_layout(right, h, self.frame.scale, clock_width, percent_width, wifi);
            self.rounded(
                layout.pill,
                layout.pill.height() as f32 / 2.,
                theme.surface,
                Some(theme.border),
            )?;
            if let Some(icon) = layout.wifi {
                self.icon(Glyph::Wifi, icon, theme.highlight_bg)?;
            }
            self.label(&clock, (Face::Mono, 13., theme.muted), layout.clock)?;
            if let (Some(b), Some(icon), Some(panel), Some(text)) =
                (battery, layout.battery, layout.percent, percent)
            {
                self.label(&text, (Face::Mono, 13., theme.muted), panel)?;
                self.icon(
                    battery_glyph(b),
                    icon,
                    if b.charging {
                        theme.highlight_bg
                    } else {
                        theme.muted
                    },
                )?;
            }
            right = layout.pill.x() - self.px(20.);
        }
        Ok(right)
    }
    fn header(
        &mut self,
        screen: &Screen,
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        layout: &Layout,
    ) -> Result<(), String> {
        let pad = self.px(24.);
        let right = if layout.status {
            self.status_header(theme)?
        } else {
            self.frame.width as i32 - pad
        };
        let h = (theme.header_height * self.frame.height as f32).round() as u32;
        if let Screen::Games { system } = screen {
            let title = system_title(system, data, &self.catalog);
            let total = data.games(system).len();
            let n = if total == 0 { 0 } else { cursor + 1 };
            if layout.header == Header::Compact {
                let text = self.catalog.format(
                    "games.position",
                    &[
                        ("system", &title),
                        ("n", &n.to_string()),
                        ("total", &total.to_string()),
                    ],
                );
                let width = (right - pad - self.px(10.)).max(1) as u32;
                return self.label(
                    &text,
                    (Face::Regular, 13., theme.muted),
                    Rect::new(pad, 0, width, h),
                );
            }
            let pos = format!("{n}/{total}");
            let pw = self.measure(&pos, Face::Mono, 13.)?;
            let title_w = self
                .measure(&title, Face::Semibold, 15.)?
                .min((right - pad - self.px(10.) - pw as i32).max(1) as u32);
            self.label(
                &title,
                (Face::Semibold, 15., theme.text),
                Rect::new(pad, 0, title_w, h),
            )?;
            self.label(
                &pos,
                (Face::Mono, 13., theme.muted),
                Rect::new(pad + title_w as i32 + self.px(10.), 0, pw, h),
            )?;
        } else {
            let mut x = pad;
            for glyph in self.catalog.text("systems.title").chars() {
                let glyph = &glyph.to_string();
                let w = self.measure(glyph, Face::Semibold, 12.)?;
                self.label(
                    glyph,
                    (Face::Semibold, 12., theme.text),
                    Rect::new(x, 0, w, h),
                )?;
                x += w as i32 + self.px(1.);
            }
        }
        Ok(())
    }
    fn list(
        &mut self,
        screen: &Screen,
        (systems, lifted): (&[String], bool),
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        layout: &Layout,
    ) -> Result<(), String> {
        let l = &layout.list;
        let (key, len, collections, is_games) = match screen {
            Screen::Games { system } => (system.clone(), data.games(system).len(), 0, true),
            _ if l.mark == Mark::Tile => {
                return self.tiles(systems, cursor, lifted, theme, l);
            }
            _ => (
                "@systems".to_owned(),
                systems.len(),
                systems.iter().take_while(|s| s.starts_with('@')).count(),
                false,
            ),
        };
        let Frame { height, now, .. } = self.frame;
        let area = self.area(l.rect);
        let (top, bottom, margin, list_width) = (area.y(), area.bottom(), area.x(), area.width());
        // Games never use tiles; such a theme gets the pill.
        let pill = l.mark != Mark::Bar;
        let [row, selected, gap] = [l.row, l.selected, l.gap].map(|h| (h * height as f32).round());
        let extra = selected - row;
        let separator = if collections > 0 {
            self.px(15.) as f32
        } else {
            0.
        };
        let metrics = (row + gap, self.px(15.) as f32, extra);
        let y_of = |i: usize| slot(i, cursor, collections, metrics);
        let target = y_of(cursor);
        let available = (bottom - top).max(1) as f32;
        let total = len as f32 * (row + gap) + separator + extra;
        let scroll = match l.scroll {
            Scroll::Center => target + (selected - available) / 2.,
            Scroll::Edge => {
                let previous = self.list.scroll.target;
                previous.min(target).max(target + selected - available)
            }
        }
        .clamp(0., (total - available).max(0.));
        if self.list.key != key {
            self.list.key = key;
            self.list.scroll.retarget(scroll, now);
            self.list.scroll.snap();
            self.list.pill.retarget(target, now);
            self.list.pill.snap();
        } else {
            self.list.scroll.retarget(scroll, now);
            self.list.pill.retarget(target, now);
        }
        if let Some((prev, was)) = self
            .list
            .held
            .filter(|&(p, _)| lifted && p.abs_diff(cursor) == 1)
        {
            // The displaced row now sits at the lifted row's old index; it
            // starts from where the previous layout actually drew it.
            let before = slot(cursor, prev, was, metrics);
            self.list.swap_row = prev;
            self.list
                .swap
                .retarget(before - slot(prev, cursor, collections, metrics), now);
            self.list.swap.snap();
            self.list.swap.retarget(0., now);
        }
        self.list.held = lifted.then_some((cursor, collections));
        let offset = self.list.scroll.value(now);
        self.canvas.set_clip_rect(area);
        // A lifted row sits slightly right of the list, in an outline.
        let lift = if lifted { self.px(6.) } else { 0 };
        let radius = l.radius * height as f32;
        let mark_y = top + (self.list.pill.value(now) - offset).round() as i32;
        if len > 0 && lifted {
            let pill = Rect::new(
                margin + lift,
                mark_y,
                list_width - lift as u32,
                selected.max(1.) as u32,
            );
            let fill = Rgba {
                a: 40,
                ..theme.highlight_bg
            };
            self.rounded(pill, radius, fill, None)?;
            self.geometry
                .dashed(&mut self.canvas, pill, radius, theme.highlight_bg)?;
        } else if len > 0 && pill {
            let pill = Rect::new(margin, mark_y, list_width, selected.max(1.) as u32);
            self.rounded(pill, radius, theme.highlight_bg, None)?;
        } else if len > 0 {
            let h = (selected * 0.55).round().max(1.) as u32;
            let bar = Rect::new(
                margin,
                mark_y + (selected as i32 - h as i32) / 2,
                self.px(4.) as u32,
                h,
            );
            self.rounded(bar, self.px(2.) as f32, theme.highlight_bg, None)?;
        }
        if collections > 0 {
            let y = top
                + (collections as f32 * (row + gap) + self.px(6.) as f32 - offset
                    + if cursor < collections { extra } else { 0. })
                .round() as i32;
            self.divider(
                margin + self.px(14.),
                y,
                list_width.saturating_sub(self.px(28.) as u32),
                theme.border,
            )?;
        }
        // On a pill the selection's ink is the accent's; beside a bar it stays plain.
        let on_pill = pill || lifted;
        let inset = if pill { self.px(14.) } else { 0 };
        // Visit only potentially visible rows, even in a ten-thousand-game list.
        let first = ((offset - separator - extra).max(0.) / (row + gap)).floor() as usize;
        let last = ((offset + available) / (row + gap)).ceil() as usize + 1;
        for i in first..len.min(last) {
            let glide = if i == self.list.swap_row {
                self.list.swap.value(now)
            } else {
                0.
            };
            let y = top + (y_of(i) + glide - offset).round() as i32;
            let rh = if i == cursor { selected } else { row }.max(1.) as u32;
            if y + rh as i32 <= top || y >= bottom {
                continue;
            }
            let selected = i == cursor;
            let (face, size, c) = row_style(theme, l, i, collections, cursor);
            let accent_ink = selected && on_pill;
            let mut x = margin + inset + if selected && !pill { self.px(14.) } else { 0 };
            let mut right = margin + list_width as i32 - inset;
            let name = if let Screen::Games { system } = screen {
                let game = &data.games(system)[i];
                if game.favorite {
                    let star = self.frame.square(
                        right - self.px(14.),
                        y + (rh as i32 - self.px(14.)) / 2,
                        14.,
                    );
                    self.icon(
                        Glyph::Star,
                        star,
                        selection(accent_ink, theme.highlight_bg, theme.highlight_text),
                    )?;
                    right -= self.px(24.);
                }
                game.name.clone()
            } else {
                let name = systems.get(i).expect("visible system row");
                if lifted && selected {
                    x += lift;
                    let mid = y + rh as i32 / 2;
                    let rect = self.frame.square(
                        right - self.px(CHEVRON - 4.),
                        mid - self.px(CHEVRON / 2.),
                        CHEVRON,
                    );
                    self.icon(Glyph::ChevronsUpDown, rect, theme.highlight_bg)?;
                    right -= self.px(CHEVRON + 4.);
                    let icon = self.frame.square(x, mid - self.px(8.), 16.);
                    self.icon(Glyph::ArrowUpDown, icon, c)?;
                    x += self.px(26.);
                } else {
                    let count = if name == crate::moonlight::SYSTEM {
                        String::new()
                    } else {
                        system_count(name, data).to_string()
                    };
                    let face = selection(selected, Face::Mono, Face::MonoMedium);
                    let color = selection(accent_ink, theme.dim_text, theme.accent_text_secondary);
                    right = self.packed_label(
                        &count,
                        (face, 13., color),
                        (right, y, rh),
                        u32::MAX,
                        Align::Right,
                    )? - self.px(14.);
                    // Moonlight is drawn like a regular system row, without an icon.
                    if name.starts_with('@') && name != crate::moonlight::SYSTEM {
                        let icon = self
                            .frame
                            .square(x, y + (rh as i32 - self.px(16.)) / 2, 16.);
                        if name == "@favorites" {
                            self.icon(
                                Glyph::Star,
                                icon,
                                selection(accent_ink, theme.highlight_bg, theme.highlight_text),
                            )?;
                        } else {
                            self.icon(Glyph::Clock, icon, c)?;
                        }
                        x += self.px(26.);
                    }
                }
                system_title(name, data, &self.catalog)
            };
            let width = (right - x).max(1) as u32;
            let (name, size) = if is_games {
                (name.as_str(), size)
            } else {
                let minimum = f32::from(self.frame.font_size(0.)) / self.frame.scale;
                fit_system_name(&name, width, (size, minimum), |name, size| {
                    self.measure(name, face, size)
                })?
            };
            self.label(name, (face, size, c), Rect::new(x, y, width, rh))?;
        }
        if len == 0 {
            self.label(
                &self.catalog.text(if self.loading {
                    "loading"
                } else if is_games {
                    "empty.games"
                } else {
                    "empty.systems"
                }),
                (Face::Regular, 16., theme.muted),
                Rect::new(
                    margin + self.px(14.),
                    top,
                    list_width.saturating_sub(self.px(28.) as u32).max(1),
                    self.px(38.) as u32,
                ),
            )?;
        }
        self.canvas.set_clip_rect(None);
        Ok(())
    }
    /// The cover again behind everything, faded into the background from the left.
    fn backdrop(
        &mut self,
        cover: Option<&Texture<'_>>,
        wash: Backdrop,
        theme: &Theme,
    ) -> Result<(), String> {
        let Some(cover) = cover else { return Ok(()) };
        let Frame { width, height, .. } = self.frame;
        let screen = Rect::new(0, 0, width, height);
        let size = cover.query();
        let alpha = wash.opacity * self.cover_fade.value(self.frame.now);
        self.geometry.cover(
            &mut self.canvas,
            cover,
            screen,
            0.,
            (alpha * 255.).round() as u8,
            crop((size.width, size.height), (width, height)),
        )?;
        let solid = (wash.solid * width as f32).round() as u32;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas
            .fill_rect(Rect::new(0, 0, solid.max(1), height))?;
        gradient(
            &mut self.canvas,
            Rect::new(solid as i32, 0, width.saturating_sub(solid).max(1), height),
            theme.background,
            Rgba {
                a: (wash.edge * 255.).round() as u8,
                ..theme.background
            },
        )
    }
    fn cover(
        &mut self,
        system: &str,
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        cover: Option<&Texture<'_>>,
        layout: &Layout,
    ) -> Result<(), String> {
        let Frame { height, now, .. } = self.frame;
        if let Some(panel) = layout.panel {
            let rect = self.area(panel.rect);
            self.panel(
                rect,
                panel.radius * height as f32,
                theme.surface,
                theme.border,
            )?;
        }
        let game = data.games(system).get(cursor);
        let key = game.map(|g| g.path.clone());
        if self.cover_key != key {
            self.cover_key = key;
            self.cover_visible = false;
            self.cover_fade = Tween::new(0., self.motion.fade(CROSSFADE));
        }
        match (layout.art, cover) {
            (Some(art), Some(cover)) => {
                if !self.cover_visible {
                    self.cover_visible = true;
                    self.cover_fade.retarget(1., now);
                }
                // One pixel inside the frame's border.
                let rect = self.area(art.rect);
                let inner = Rect::new(
                    rect.x() + 1,
                    rect.y() + 1,
                    rect.width().saturating_sub(2).max(1),
                    rect.height().saturating_sub(2).max(1),
                );
                let size = cover.query();
                let (dst, part) = match art.fit {
                    crate::layout::Fit::Cover => (
                        inner,
                        crop((size.width, size.height), (inner.width(), inner.height())),
                    ),
                    crate::layout::Fit::Contain => {
                        let scale = (inner.width() as f32 / size.width as f32)
                            .min(inner.height() as f32 / size.height as f32);
                        let w = (size.width as f32 * scale).round().max(1.) as u32;
                        let h = (size.height as f32 * scale).round().max(1.) as u32;
                        let dst = Rect::new(
                            rect.x() + (rect.width() as i32 - w as i32) / 2,
                            rect.y() + (rect.height() as i32 - h as i32) / 2,
                            w,
                            h,
                        );
                        (dst, WHOLE)
                    }
                };
                self.geometry.cover(
                    &mut self.canvas,
                    cover,
                    dst,
                    art.radius * height as f32,
                    (self.cover_fade.value(now) * 255.).round() as u8,
                    part,
                )?;
            }
            (Some(art), None) => {
                self.cover_visible = false;
                let rect = self.area(art.rect);
                self.label_aligned(
                    &system_title(system, data, &self.catalog),
                    (Face::Regular, 13., theme.dim_text),
                    Rect::new(
                        rect.x() + self.px(12.),
                        rect.y(),
                        rect.width().saturating_sub(self.px(24.) as u32).max(1),
                        rect.height(),
                    ),
                    Align::Center,
                )?;
            }
            (None, _) => {}
        }
        self.old_art()?;
        if let (Some(game), Some(meta)) = (game, layout.meta) {
            let rect = self.area(meta.rect);
            let size = meta.size * 480.;
            let mut parts = Vec::new();
            if let Some(year) = game.year {
                parts.push(year.to_string());
            }
            if let Some(players) = &game.players {
                parts.push(self.catalog.players(players));
            }
            let played = game
                .last_played
                .map(|timestamp| self.catalog.relative_time(timestamp, self.frame.unix))
                .unwrap_or_default();
            let pw = self.measure(&played, Face::Regular, size)?;
            let gap = if pw > 0 { self.px(8.) as u32 } else { 0 };
            let (y, h) = (rect.y(), rect.height());
            self.label(
                &parts.join(" · "),
                (Face::Regular, size, theme.muted),
                Rect::new(rect.x(), y, rect.width().saturating_sub(pw + gap).max(1), h),
            )?;
            self.label_aligned(
                &played,
                (Face::Regular, size, theme.muted),
                Rect::new(rect.right() - pw as i32, y, pw.max(1), h),
                Align::Right,
            )?;
        }
        Ok(())
    }
    fn footer(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        layout: &Layout,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let h = (theme.footer_height * height as f32).round().max(1.) as u32;
        let y = height as i32 - h as i32;
        if layout.divider {
            self.divider(0, y, width, theme.border)?;
        }
        let moving = app.moving();
        let details: Vec<(char, &str)>;
        let hint_keys: &[(char, &str)] = match app.screen {
            Screen::Details { .. } => {
                let action = crate::state::DETAILS_ACTIONS[app.details_focus.min(2)];
                details = [
                    ('A', action),
                    ('↕', "details.scroll"),
                    ('Y', "details.options"),
                    ('B', "hint.back"),
                ]
                .into_iter()
                .filter(|(button, _)| *button != '↕' || self.details_overflow > 0)
                .collect();
                &details
            }
            Screen::Systems if moving.is_some() => &[
                ('↕', "hint.move"),
                ('A', "hint.confirm"),
                ('B', "hint.cancel"),
            ],
            Screen::Systems => &[('A', "hint.open"), ('Y', "hint.menu")],
            Screen::Games { .. } => &[
                ('A', "hint.play"),
                ('B', "hint.back"),
                ('X', "hint.favorite"),
                ('Y', "hint.menu"),
            ],
            _ => &[('A', "hint.choose"), ('B', "hint.close")],
        };
        let left = self.hints(hint_keys, theme, width, y, h)?;
        if matches!(app.screen, Screen::Systems) && (layout.summary || moving.is_some()) {
            let width = (left - self.px(40.)).max(1) as u32;
            let (text, size, ink) = match moving {
                Some((name, n, total)) => {
                    let title = system_title(name, data, &self.catalog);
                    let (catalog, frame, fonts) = (&self.catalog, self.frame, &mut self.text);
                    let (text, size) = fit_status(
                        &title,
                        width,
                        |name, status| status_text(catalog, name, (n, total), status),
                        |text, size| {
                            fonts
                                .as_mut()
                                .ok_or_else(|| catalog.text("error.font_init"))?
                                .measure(text, Face::Regular, frame.font_size(size))
                                .map_err(|e| localized_error(catalog, e))
                        },
                    )?;
                    (text, size, theme.text)
                }
                None => (library_summary(data, &self.catalog), 13., theme.muted),
            };
            self.label(
                &text,
                (Face::Regular, size, ink),
                Rect::new(self.px(24.), y, (left - self.px(40.)).max(1) as u32, h),
            )?;
        }
        Ok(())
    }
    fn hints<B: Copy + Into<HintButton>>(
        &mut self,
        hints: &[(B, &str)],
        theme: &Theme,
        width: u32,
        y: i32,
        h: u32,
    ) -> Result<i32, String> {
        let mut right = width as i32 - self.px(24.);
        for &(button, key) in hints.iter().rev() {
            let start = matches!(button.into(), HintButton::Start);
            let buttons = match button.into() {
                HintButton::Key(key) => [Some(key), None],
                HintButton::Pair(left, right) => [Some(left), Some(right)],
                HintButton::Start => [None, None],
            };
            let paired = buttons[1].is_some();
            let label = self.catalog.text(key);
            right = self.packed_label(
                &label,
                (Face::Regular, 13., theme.muted),
                (right, y, h),
                u32::MAX,
                Align::Left,
            )? - self.px(6.)
                - self.px(if start {
                    45.
                } else if paired {
                    50.
                } else {
                    20.
                });
            if start {
                let badge = Rect::new(
                    right,
                    y + (h as i32 - self.px(20.)) / 2,
                    self.px(45.) as u32,
                    self.px(20.) as u32,
                );
                self.rounded(badge, self.px(5.) as f32, theme.badge, None)?;
                self.label_aligned(
                    "START",
                    (Face::Regular, 10., theme.text),
                    badge,
                    Align::Center,
                )?;
            }
            if paired {
                self.label_aligned(
                    "/",
                    (Face::Regular, 13., theme.muted),
                    Rect::new(right + self.px(20.), y, self.px(10.) as u32, h),
                    Align::Center,
                )?;
            }
            for (n, button) in buttons.into_iter().flatten().enumerate() {
                let badge = self.frame.square(
                    right + self.px(n as f32 * 30.),
                    y + (h as i32 - self.px(20.)) / 2,
                    20.,
                );
                let size = self.px(11.).clamp(8, 128) as u16;
                self.text
                    .as_mut()
                    .ok_or_else(|| self.catalog.text("error.font_init"))?
                    .draw_badge(
                        &mut self.canvas,
                        if button == '↕' { ' ' } else { button },
                        size,
                        theme.badge,
                        theme.text,
                        badge,
                    )?;
                if button == '↕' {
                    let icon = self.frame.square(
                        badge.x() + (badge.width() as i32 - self.px(14.)) / 2,
                        badge.y() + (badge.height() as i32 - self.px(14.)) / 2,
                        14.,
                    );
                    self.icon(Glyph::ArrowUpDown, icon, theme.text)?;
                }
            }
            right -= self.px(16.);
        }
        Ok(right + self.px(16.))
    }
    fn scrim(&mut self, theme: &Theme, strength: f32) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let scrim = Rgba {
            a: (f32::from(theme.scrim.a) * strength).round().min(255.) as u8,
            ..theme.scrim
        };
        self.canvas.set_draw_color(color(scrim));
        self.canvas.fill_rect(Rect::new(0, 0, width, height))
    }
    fn modal(&mut self, theme: &Theme, spec: (f32, f32, f32, i32)) -> Result<Rect, String> {
        let (w, clearance, h, optical_offset) = spec;
        let Frame { width, height, .. } = self.frame;
        let w = (self.px(w) as u32)
            .min(width.saturating_sub(self.px(clearance) as u32))
            .max(1);
        let h = self.px(h) as u32;
        let panel = Rect::new(
            (width - w) as i32 / 2,
            (height.saturating_sub(h) as i32 / 2 + optical_offset).max(0),
            w,
            h,
        );
        self.panel(
            panel,
            theme.radius_card * height as f32,
            theme.surface_elevated,
            theme.border,
        )?;
        Ok(panel)
    }
    fn menu(&mut self, app: &App, data: &dyn DataSource, theme: &Theme) -> Result<(), String> {
        // The mockup combines a dimmed backdrop and its base scrim.
        self.pop_in(effects::Pop::Menu, "menu", (theme, 1.2), |ui| {
            ui.menu_panel(app, data, theme)
        })?;
        // Above the scrim and in place: they replace the footer's own hints.
        let Frame { width, height, .. } = self.frame;
        let footer = (theme.footer_height * height as f32).round().max(1.) as u32;
        self.hints(
            &[('A', "hint.choose"), ('B', "hint.close")],
            theme,
            width,
            height as i32 - footer as i32,
            footer,
        )?;
        Ok(())
    }
    fn menu_panel(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let Frame { height, now, .. } = self.frame;
        let items = app.quick_menu_items(data);
        let has_core = matches!(items.first(), Some(QuickItem::Core));
        let (base_h, base_items) = if has_core || items.first() == Some(&QuickItem::Settings) {
            (300., 4)
        } else {
            (256., 3)
        };
        let panel_h = base_h + 42. * items.len().saturating_sub(base_items) as f32;
        // Optical centering: the approved modal sits 20px above center.
        let panel = self.modal(theme, (340., 32., panel_h, -self.px(20.)))?;
        let (title, subtitle) = if let Screen::QuickMenu {
            game: Some((system, game)),
        } = &app.screen
        {
            (
                data.games(system)
                    .get(*game)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| self.catalog.text("menu.title")),
                system_title(system, data, &self.catalog),
            )
        } else {
            (
                self.catalog.text("menu.title"),
                self.catalog.text("systems.title"),
            )
        };
        let x = panel.x() + self.px(24.);
        let content_w = panel.width().saturating_sub(self.px(48.) as u32).max(1);
        self.label(
            &title,
            (Face::Semibold, 16., theme.text),
            self.frame.row(panel, 24., 16., 24.),
        )?;
        self.label(
            &subtitle,
            (Face::Regular, 12., theme.muted),
            self.frame.row(panel, 24., 40., 16.),
        )?;
        let top = panel.y() + self.px(64.);
        let row = self.px(42.);
        let divider_index = items
            .iter()
            .position(|item| *item == QuickItem::Reboot)
            .unwrap_or(1);
        let core_extra = if has_core { self.px(4.) } else { 0 };
        let separator = self.px(10.);
        let offset = |index: usize| {
            index as i32 * row
                + if index > 0 { core_extra } else { 0 }
                + if index >= divider_index { separator } else { 0 }
        };
        let target = offset(app.cursor) as f32;
        self.menu_pill.retarget(target, now);
        if !self.menu_open {
            self.menu_pill.snap();
            self.menu_open = true;
        }
        let pill = Rect::new(
            panel.x() + self.px(12.),
            top + self.menu_pill.value(now).round() as i32,
            panel.width().saturating_sub(self.px(24.) as u32).max(1),
            self.px(if has_core && app.cursor == 0 {
                44.
            } else {
                40.
            }) as u32,
        );
        self.rounded(
            pill,
            theme.radius_item * height as f32,
            theme.highlight_bg,
            None,
        )?;
        for (index, item) in items.iter().enumerate() {
            let y = top + offset(index);
            let item_height = self.px(if has_core && index == 0 { 44. } else { 40. }) as u32;
            if index == divider_index {
                self.divider(x, y - self.px(6.), content_w, theme.border)?;
            }
            let selected = index == app.cursor;
            let c = selection(selected, theme.text, theme.highlight_text);
            let label = match item {
                QuickItem::Details => "menu.details",
                QuickItem::Settings => "settings.title",
                QuickItem::Netplay => "netplay.menu",
                QuickItem::Core => "menu.core",
                QuickItem::Move => "menu.move",
                QuickItem::RestoreOrder => "menu.restore_order",
                QuickItem::Suspend => "menu.suspend",
                QuickItem::Reboot => "menu.reboot",
                QuickItem::Shutdown => "menu.shutdown",
            };
            let label = self.catalog.text(label);
            let label = label.as_str();
            let face = selection(selected, Face::Regular, Face::Semibold);
            let mut label_width = content_w;
            if let QuickItem::Core = item {
                let (_, core) = app.selected_core();
                let default_core = self.catalog.text("menu.default");
                let core = if core.is_empty() { &default_core } else { core };
                let space = self.px(8.);
                let end = panel.right() - self.px(24.);
                let cc = selection(selected, theme.muted, theme.accent_text_secondary);
                let mut right = end;
                for (text, style, cap, align) in [
                    ("R", (Face::Mono, 12., cc), u32::MAX, Align::Left),
                    (core, (Face::Regular, 15., c), content_w / 2, Align::Right),
                    ("L", (Face::Mono, 12., cc), u32::MAX, Align::Left),
                ] {
                    right = self.packed_label(text, style, (right, y, item_height), cap, align)?
                        - space;
                }
                label_width = content_w.saturating_sub((end - right) as u32).max(1);
            }
            self.label(
                label,
                (face, 15., c),
                Rect::new(x, y, label_width, item_height),
            )?;
        }
        Ok(())
    }
    fn notice(&mut self, message: &str, theme: &Theme) -> Result<(), String> {
        self.pop_in(effects::Pop::Notice, message, (theme, 1.0), |ui| {
            ui.notice_panel(message, theme)
        })
    }
    fn notice_panel(&mut self, message: &str, theme: &Theme) -> Result<(), String> {
        let lines: Vec<_> = message.split('\n').take(3).collect();
        let panel = self.modal(theme, (380., 48., 64. + lines.len() as f32 * 24., 0))?;
        for (i, line) in lines.iter().enumerate() {
            self.label_aligned(
                line,
                (Face::Semibold, 16., theme.text),
                self.frame.row(panel, 24., 32. + i as f32 * 24., 24.),
                Align::Center,
            )?;
        }
        Ok(())
    }
}
pub fn pixels(rect: Rect01, width: u32, height: u32) -> Rect {
    Rect::new(
        (rect.x * width as f32).round() as i32,
        (rect.y * height as f32).round() as i32,
        (rect.w * width as f32).round().max(1.) as u32,
        (rect.h * height as f32).round().max(1.) as u32,
    )
}
fn system_title(name: &str, data: &dyn DataSource, catalog: &I18n) -> String {
    match name {
        "@favorites" => catalog.text("collections.favorites"),
        "@recent" => catalog.text("collections.recent"),
        _ => data
            .systems()
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.fullname.clone())
            .unwrap_or_else(|| name.to_owned()),
    }
}

/// Every unselected row shares one ink, so moving never repaints the rest
/// of the list.
fn row_style(
    theme: &Theme,
    list: &List,
    i: usize,
    collections: usize,
    cursor: usize,
) -> (Face, f32, Rgba) {
    let selected = i == cursor;
    let color = if selected {
        selection(list.mark == Mark::Bar, theme.highlight_text, theme.text)
    } else if i < collections {
        theme.muted
    } else {
        theme.text_secondary
    };
    (
        selection(selected, Face::Regular, Face::Semibold),
        selection(selected, list.font, list.selected_font) * 480.,
        color,
    )
}
/// Games in a system or collection.
fn system_count(name: &str, data: &dyn DataSource) -> usize {
    if name.starts_with('@') {
        data.collection_len(name)
    } else {
        data.systems()
            .iter()
            .find(|s| s.name == name)
            .map_or(0, |s| s.game_count as usize)
    }
}
fn library_summary(data: &dyn DataSource, catalog: &I18n) -> String {
    let systems = data
        .systems()
        .iter()
        .filter(|s| !s.name.starts_with('@') && s.game_count > 0 && !data.is_tools(&s.name))
        .count();
    let games = data
        .systems()
        .iter()
        .filter(|s| !s.name.starts_with('@') && !data.is_tools(&s.name))
        .map(|s| u64::from(s.game_count))
        .sum::<u64>();
    let count = |key, n: u64| catalog.plural(key, n as f64, &[("count", &n.to_string())]);
    catalog.format(
        "library.summary",
        &[
            ("systems", &count("systems.count", systems as u64)),
            ("games", &count("games.count", games)),
        ],
    )
}

/// First systems screen only: no SDL video calls, textures, sysfs or cover I/O.
pub fn prepare_first_frame<'font>(
    ttf: &'font Sdl2TtfContext,
    theme: &Theme,
    (root, theme_dir, system_icons_dir): (Option<&Path>, Option<&Path>, &Path),
    catalog: &I18n,
    data: &dyn DataSource,
    (width, height): (u32, u32),
    trace: StartupTrace,
) -> Result<PreparedText<'font>, String> {
    let frame = Frame::new(width, height);
    let mut prepared = PreparedText::new(ttf, font_paths(theme, root), trace);
    let style = |face, size, color| Style {
        face,
        size: frame.font_size(size),
        color,
    };
    let label = |prepared: &mut PreparedText,
                 text: &str,
                 (face, size, color),
                 width: Option<u32>|
     -> Result<u32, String> {
        let style = style(face, size, color);
        let width = match width {
            Some(w) => w,
            None => prepared.measure(text, face, style.size)?,
        };
        prepared.label(text, style, width)?;
        Ok(width)
    };
    label(
        &mut prepared,
        &Status::new(root, Instant::now()).clock,
        (Face::Mono, 13., theme.muted),
        None,
    )?;
    for glyph in catalog.text("systems.title").chars() {
        label(
            &mut prepared,
            &glyph.to_string(),
            (Face::Semibold, 12., theme.text),
            None,
        )?;
    }
    let systems = App::system_rows(data);
    let collections = systems.iter().take_while(|s| s.starts_with('@')).count();
    let view = theme.view(&theme.active_view);
    let list = &view.systems.list;
    let area = pixels(list.rect, width, height);
    let (top, bottom, margin, list_width) = (area.y(), area.bottom(), area.x(), area.width());
    let [row, selected, gap] =
        [list.row, list.selected, list.gap].map(|h| (h * height as f32).round());
    let inset = if list.mark == Mark::Pill {
        frame.px(14.)
    } else {
        0
    };
    let mut first_icons = Vec::new();
    // Tiles show art, not labels: nothing to rasterize ahead.
    let rows = if list.mark == Mark::Tile {
        &[][..]
    } else {
        &systems[..]
    };
    for (i, name) in rows.iter().enumerate() {
        let y = top
            + (i as f32 * (row + gap)
                + if i >= collections && collections > 0 {
                    frame.px(15.) as f32
                } else {
                    0.
                }
                + if i > 0 { selected - row } else { 0. })
            .round() as i32;
        if y >= bottom {
            break;
        }
        if name.starts_with('@') && name != crate::moonlight::SYSTEM {
            first_icons.push((
                if name == "@favorites" {
                    Glyph::Star
                } else {
                    Glyph::Clock
                },
                frame.px(16.) as u32,
            ));
        }
        let count = system_count(name, data);
        let accent = i == 0 && list.mark == Mark::Pill;
        let count_width = label(
            &mut prepared,
            &if name == crate::moonlight::SYSTEM {
                String::new()
            } else {
                count.to_string()
            },
            (
                selection(i == 0, Face::Mono, Face::MonoMedium),
                13.,
                selection(accent, theme.dim_text, theme.accent_text_secondary),
            ),
            None,
        )?;
        let x = margin
            + inset
            + if i == 0 && inset == 0 {
                frame.px(14.)
            } else {
                0
            }
            + if name.starts_with('@') {
                frame.px(26.)
            } else {
                0
            };
        let right = margin + list_width as i32 - inset - count_width as i32 - frame.px(14.);
        let title = system_title(name, data, catalog);
        let (face, size, color) = row_style(theme, list, i, collections, 0);
        let width = (right - x).max(1) as u32;
        let minimum = f32::from(frame.font_size(0.)) / frame.scale;
        let (title, size) = fit_system_name(&title, width, (size, minimum), |text, size| {
            prepared.measure(text, face, frame.font_size(size))
        })?;
        label(&mut prepared, title, (face, size, color), Some(width))?;
    }
    if systems.is_empty() {
        label(
            &mut prepared,
            &catalog.text("empty.systems"),
            (Face::Regular, 16., theme.muted),
            Some(list_width.saturating_sub(frame.px(28.) as u32).max(1)),
        )?;
    }
    prepared.prepare_icons(theme_dir, system_icons_dir, first_icons);
    // Deferred settings and move overlays must not read icons on the render thread.
    prepared.preload_icons(&[
        Glyph::ArrowUpDown,
        Glyph::ChevronsUpDown,
        Glyph::Shift,
        Glyph::Backspace,
        Glyph::Eye,
        Glyph::EyeOff,
        Glyph::Signal,
        Glyph::SignalLow,
        Glyph::SignalMedium,
        Glyph::SignalHigh,
        Glyph::Lock,
        Glyph::Check,
    ]);
    Ok(prepared)
}

impl PreparedText<'_> {
    /// Error banners are part of the initial frame too, including their shadow.
    pub fn prepare_notice(
        &mut self,
        message: &str,
        theme: &Theme,
        (width, height): (u32, u32),
    ) -> Result<(), String> {
        let frame = Frame::new(width, height);
        let panel_width = (frame.px(380.) as u32)
            .min(width.saturating_sub(frame.px(48.) as u32))
            .max(1);
        let width = panel_width.saturating_sub(frame.px(48.) as u32).max(1);
        for line in message.split('\n').take(3) {
            self.label(
                line,
                Style {
                    face: Face::Semibold,
                    size: frame.font_size(16.),
                    color: theme.text,
                },
                width,
            )?;
        }
        self.shadow = Some(Shadow::surface()?);
        Ok(())
    }
}

fn font_paths(theme: &Theme, root: Option<&Path>) -> [Vec<PathBuf>; 4] {
    let defaults = |mono: bool| {
        let names = if mono {
            [
                "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
            ]
        } else {
            [
                "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
            ]
        };
        let mut paths = Vec::new();
        for name in names {
            if let Some(root) = root {
                paths.push(root.join(name.trim_start_matches('/')));
            }
            paths.push(PathBuf::from(name));
        }
        paths
    };
    let mut paths: [Vec<PathBuf>; 4] = std::array::from_fn(|i| defaults(i == 2 || i == 3));
    for (i, preferred) in [
        &theme.font,
        &theme.font_semibold,
        &theme.font_mono,
        &theme.font_mono_medium,
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(font) = preferred {
            paths[i].insert(0, font.clone());
        }
    }
    // A theme with one face remains valid; use that face before host fallbacks.
    if theme.font_semibold.is_none() {
        if let Some(font) = &theme.font {
            paths[1].insert(0, font.clone());
        }
    }
    if theme.font_mono_medium.is_none() {
        if let Some(font) = &theme.font_mono {
            paths[3].insert(0, font.clone());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_are_neither_systems_nor_games_in_the_library_summary() {
        struct Data(Vec<archr_index::cache::SystemEntry>);
        impl DataSource for Data {
            fn systems(&self) -> &[archr_index::cache::SystemEntry] {
                &self.0
            }
            fn games(&self, _: &str) -> &[archr_index::Game] {
                &[]
            }
            fn origin(&self, _: &str, _: usize) -> Option<(String, usize)> {
                None
            }
            fn cores(&self, _: &str) -> Vec<(String, String)> {
                vec![]
            }
            fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
                None
            }
            fn is_tools(&self, system: &str) -> bool {
                system == "tools"
            }
        }
        let entry = |name: &str, game_count| archr_index::cache::SystemEntry {
            name: name.into(),
            fullname: name.into(),
            game_count,
            dir_mtime: 0,
            source_fingerprint: 0,
        };
        let data = Data(vec![
            entry("@moonlight", 1),
            entry("nes", 3),
            entry("tools", 7),
        ]);
        let catalog = I18n::load(Path::new("/nonexistent"), Some("en"), None);
        assert_eq!(library_summary(&data, &catalog), "1 system · 3 games");
    }
    #[test]
    fn move_status_fits_beside_its_hints_in_every_locale_without_ellipsis() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        let ttf = sdl2::ttf::init().unwrap();
        let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/fonts");
        let font = |size: f32| {
            crate::text::production_font(&ttf, &fonts.join("Inter-Regular.ttf"), size as u16)
        };
        let measure = |text: &str, size: f32| Ok(font(size).size_of(text).unwrap().0);
        for code in ["en", "pt_BR", "es", "ru"] {
            let i = I18n::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales"),
                Some(code),
                None,
            );
            // Footer: 24px margins, three badges with 6px gap, 16px between hints, 40px clearance.
            let hints: u32 = ["hint.move", "hint.confirm", "hint.cancel"]
                .iter()
                .map(|key| measure(&i.text(key), 13.).unwrap() + 26)
                .sum::<u32>()
                + 32;
            // The label ends 16px left of the hints; 24px margins on both sides.
            let width = 640 - 24 - hints - 40 - 24 + 24;
            for name in [
                "Nintendo 64",
                "Super Nintendo Entertainment System (Super Famicom)",
                "MO/TO family (TO7, TO7/70, TO8, TO8D, TO9, TO9+, MO5 and MO6)",
                "Игровая приставка Sega Mega Drive (Genesis)",
            ] {
                let (text, size) = fit_status(
                    name,
                    width,
                    |name, status| status_text(&i, name, (12, 34), status),
                    measure,
                )
                .unwrap();
                assert!(
                    measure(&text, size).unwrap() <= width,
                    "{code}: {text} at {size}"
                );
            }
        }
    }

    #[test]
    fn displaced_row_starts_from_its_previous_position_across_the_separator() {
        let metrics = (40., 15., 6.);
        // Rows: 2 collections, then systems; the first system (index 2) is lifted
        // above Recentes (index 1), leaving a single leading collection.
        let drawn_before = slot(1, 2, 2, metrics);
        let target_after = slot(2, 1, 1, metrics);
        assert_eq!(drawn_before, 40.);
        let start = drawn_before - target_after;
        assert_eq!(
            target_after + start,
            drawn_before,
            "no jump on the first frame"
        );
        // Within one section the offset is the row pitch plus the pill growth.
        assert_eq!(slot(4, 3, 2, metrics) - slot(3, 4, 2, metrics), 40. + 6.);
    }

    #[test]
    fn system_names_fit_selected_and_unselected_rows_without_ellipsis() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        let ttf = sdl2::ttf::init().unwrap();
        let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/fonts");
        let full = "MO/TO family (TO7, TO7/70, TO8, TO8D, TO9, TO9+, MO5 and MO6)";
        for selected in [false, true] {
            let face = selection(selected, "Inter-Regular", "Inter-SemiBold");
            let initial = selection(selected, 17., 18.);
            let path = fonts.join(format!("{face}.ttf"));
            let measure = |text: &str, size: f32| {
                let font = crate::text::production_font(&ttf, &path, size as u16);
                Ok(font.size_of(text).unwrap().0)
            };
            for (text, width, want) in [
                (full, 1000, full),
                (full, 280, "MO/TO family"),
                (full, 70, "MO/TO family"),
                (
                    "Super Nintendo Entertainment System",
                    200,
                    "Super Nintendo Entertainment System",
                ),
                ("Name (region) revision", 140, "Name (region) revision"),
            ] {
                let (name, size) = fit_system_name(text, width, (initial, 8.), measure).unwrap();
                assert_eq!(name, want, "selected={selected}, width={width}");
                assert!(measure(name, size).unwrap() <= width);
                assert!((8. ..=initial).contains(&size));
                if width == 70 {
                    assert!(size < initial);
                }
                if width == 1000 || width == 280 {
                    assert_eq!(size, initial);
                }
            }
            let minimum_width = measure("MO/TO family", 8.).unwrap();
            let (name, size) =
                fit_system_name(full, minimum_width, (initial, 8.), measure).unwrap();
            assert_eq!((name, size), ("MO/TO family", 8.));
        }
    }

    #[test]
    fn panel_rows_preserve_single_rounding_of_combined_insets() {
        for height in [240, 350, 480, 720, 960, 1080] {
            let frame = Frame::new(640, height);
            for width in [1, 37, 340, 680] {
                let panel = Rect::new(10, 20, width, 256);
                assert_eq!(
                    frame.row(panel, 24., 16., 24.),
                    Rect::new(
                        panel.x() + frame.px(24.),
                        panel.y() + frame.px(16.),
                        width.saturating_sub(frame.px(48.) as u32).max(1),
                        frame.px(24.) as u32,
                    ),
                    "height={height}, width={width}"
                );
            }
        }
    }

    #[test]
    fn battery_glyph_tracks_charge_and_level() {
        for (percent, expected) in [
            (0, Glyph::Battery),
            (1, Glyph::BatteryLow),
            (25, Glyph::BatteryLow),
            (26, Glyph::BatteryMedium),
            (69, Glyph::BatteryMedium),
            (70, Glyph::BatteryFull),
            (100, Glyph::BatteryFull),
        ] {
            assert_eq!(
                battery_glyph(crate::status::Battery {
                    percent,
                    charging: false
                }),
                expected
            );
            assert_eq!(
                battery_glyph(crate::status::Battery {
                    percent,
                    charging: true
                }),
                Glyph::BatteryCharging
            );
        }
    }

    #[test]
    fn status_pill_orders_centers_and_hides_optional_items() {
        for scale in [1., 2.] {
            for wifi in [false, true] {
                for percent_width in [None, Some(24)] {
                    let layout = status_layout(
                        (616. * scale) as i32,
                        (44. * scale) as u32,
                        scale,
                        (39. * scale) as u32,
                        percent_width.map(|w| (w as f32 * scale) as u32),
                        wifi,
                    );
                    assert_eq!(layout.pill.right(), (616. * scale) as i32);
                    assert_eq!(layout.pill.height(), (26. * scale) as u32);
                    assert_eq!(layout.wifi.is_some(), wifi);
                    assert_eq!(layout.battery.is_some(), percent_width.is_some());
                    if let Some(battery) = layout.battery {
                        assert_eq!(battery.width(), (24. * scale) as u32);
                    }
                    assert_eq!(layout.percent.is_some(), percent_width.is_some());
                    let mut right = layout.pill.x() + (12. * scale) as i32;
                    for rect in [
                        layout.wifi,
                        Some(layout.clock),
                        layout.battery,
                        layout.percent,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        assert!(
                            rect.x() >= right,
                            "status items overlap or are out of order"
                        );
                        assert_eq!(rect.center().y(), (22. * scale) as i32);
                        right = rect.right();
                    }
                    assert_eq!(right + (12. * scale) as i32, layout.pill.right());
                }
            }
        }
        let on = status_layout(616, 44, 1., 39, Some(24), true);
        let off = status_layout(616, 44, 1., 39, Some(24), false);
        assert_eq!(on.pill.width(), 157);
        assert_eq!(off.pill.width(), 129);
        assert_eq!(on.clock.x() - on.wifi.unwrap().right(), 12);
        assert_eq!(off.wifi, None);
        assert_eq!(off.clock.x() - off.pill.x(), 12);
        assert_eq!(on.battery.unwrap().x() - on.clock.right(), 12);
        assert_eq!(on.percent.unwrap().x() - on.battery.unwrap().right(), 6);
    }
}
