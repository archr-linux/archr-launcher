//! Grafite presentation at 640×480, scaled by height, with event-driven idle.
pub use crate::text::PreparedText;
use crate::{
    i18n::I18n,
    icons::Glyph,
    motion::Tween,
    primitives::{Geometry, Icon, Shadow},
    startup::StartupTrace,
    startup_phase,
    state::{App, DataSource, QuickItem, Screen},
    status::Status,
    text::{color, Align, Face, Style, TextCache},
    theme::{Rect01, Rgba, Theme},
};
mod hub;

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
    now: Instant,
    unix: u64,
}
impl Frame {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            scale: height as f32 / 480.,
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
        self.px(value).clamp(8, 128) as u16
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
    let name = name
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once(" ("))
        .map_or(name, |(short, _)| short);
    while size > minimum && measure(name, size)? > width {
        size = (size - 1.).max(minimum);
    }
    Ok((name, size))
}

struct ListMotion {
    key: String,
    scroll: Tween,
    pill: Tween,
}
impl ListMotion {
    fn new() -> Self {
        Self {
            key: String::new(),
            scroll: Tween::new(0., Duration::from_millis(90)),
            pill: Tween::new(0., Duration::from_millis(90)),
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
        + percent_width.map_or(0, |w| px(12.) + px(22.) + px(6.) + w);
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
        battery: panel(percent_width.map(|_| px(22.)), px(12.), px(6.)),
        percent: panel(percent_width, px(16.), 0),
    }
}

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
    menu_open: bool,
    cover_key: Option<PathBuf>,
    cover_visible: bool,
    cover_fade: Tween,
    animating: bool,
    has_presented: bool,
    frame: Frame,
    pub catalog: I18n,
    pub loading: bool,
    pub banner: Option<String>,
    pub screenshot: Option<PathBuf>,
    pub transition: Option<String>,
    /// Active theme directory; its `systems/` folder holds console images.
    pub theme_dir: Option<PathBuf>,
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
            geometry: {
                let mut geometry = Geometry::new();
                geometry.startup = trace;
                startup_phase!(trace, "icons geometry cache");
                geometry
            },
            status: None,
            list: ListMotion::new(),
            menu_pill: Tween::new(0., Duration::from_millis(90)),
            menu_open: false,
            cover_key: None,
            cover_visible: false,
            cover_fade: Tween::new(0., Duration::from_millis(120)),
            animating: false,
            has_presented: false,
            frame: Frame::new(width, height),
            catalog: I18n::default(),
            loading: false,
            banner: None,
            screenshot: None,
            transition: None,
            theme_dir: None,
        })
    }
    pub fn configure(
        &mut self,
        ttf: &'font Sdl2TtfContext,
        creator: &'texture TextureCreator<WindowContext>,
        theme: &Theme,
        root: Option<&Path>,
    ) -> Result<(), String> {
        self.configure_prepared(
            creator,
            PreparedText::new(ttf, font_paths(theme, root), self.startup),
            root,
        )
    }
    pub fn configure_prepared(
        &mut self,
        creator: &'texture TextureCreator<WindowContext>,
        mut prepared: PreparedText<'font>,
        root: Option<&Path>,
    ) -> Result<(), String> {
        self.frame = Frame::from_size(self.canvas.output_size()?);
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
        self.text = Some(TextCache::from_prepared(creator, prepared)?);
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
    pub fn page_size(&self, theme: &Theme) -> usize {
        ((1. - theme.header_height - theme.footer_height - 0.04) / (theme.row_height + 2. / 480.))
            .floor()
            .max(1.) as usize
    }
    /// Called before applying input: finish old targets, never queue gestures.
    pub fn snap_animations(&mut self) {
        self.list.scroll.snap();
        self.list.pill.snap();
        self.menu_pill.snap();
        self.cover_fade.snap();
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
            shadow.draw(&mut self.canvas, rect, self.frame.scale)?;
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
        self.frame = Frame::from_size(self.canvas.output_size()?);
        let Frame {
            width, height, now, ..
        } = self.frame;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.clear();
        let (backdrop, cursor) = app.backdrop();
        self.header(backdrop, cursor, data, theme)?;
        if matches!(app.screen, Screen::StartHub { .. }) {
            self.hub(app, data, theme)?;
        } else {
            self.list(backdrop, cursor, data, theme)?;
            // Keep the first present to the prepared list; the panel follows
            // on the redraw requested when tick starts the artwork worker.
            if self.has_presented && matches!(backdrop, Screen::Systems) {
                self.system_panel(&App::system_rows(data), cursor, data, theme)?;
            }
            if let Screen::Games { system } = backdrop {
                self.cover(system, cursor, data, theme, cover)?;
            }
            if self.has_presented {
                self.footer(app, data, theme)?;
            }
        }
        if matches!(app.screen, Screen::QuickMenu { .. }) {
            self.menu(app, data, theme)?;
        } else {
            self.menu_open = false;
        }
        if let Some(banner) = self.banner.clone() {
            self.notice(&banner, theme)?;
        }
        if let Some(message) = self.transition.clone() {
            self.notice(&message, theme)?;
        }
        self.animating = self.list.scroll.active(now)
            || self.list.pill.active(now)
            || self.menu_pill.active(now)
            || self.cover_fade.active(now);
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
            self.geometry.startup = StartupTrace::default();
            if let Some(text) = &mut self.text {
                text.trace_startup(StartupTrace::default());
            }
        }
        if let Some(status) = &mut self.status {
            status.start();
        }
        Ok(())
    }
    fn header(
        &mut self,
        screen: &Screen,
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
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
                let battery = Rect::new(icon.x(), icon.y(), self.px(19.) as u32, icon.height());
                self.rounded(
                    battery,
                    self.px(2.) as f32,
                    theme.surface,
                    Some(theme.muted),
                )?;
                let fill_width = (self.px(15.) as f32 * f32::from(b.percent) / 100.).round() as u32;
                if fill_width > 0 {
                    self.rounded(
                        Rect::new(
                            battery.x() + self.px(2.),
                            battery.y() + self.px(2.),
                            fill_width,
                            self.px(8.) as u32,
                        ),
                        self.frame.scale,
                        theme.muted,
                        None,
                    )?;
                }
                let nub = Rect::new(
                    battery.right() + self.px(1.),
                    battery.y() + self.px(4.),
                    self.px(2.) as u32,
                    self.px(4.) as u32,
                );
                self.rounded(nub, self.frame.scale, theme.muted, None)?;
                if b.charging {
                    let bolt = Rect::new(
                        battery.x() + self.px(5.),
                        battery.y() + self.px(1.),
                        self.px(9.) as u32,
                        self.px(10.) as u32,
                    );
                    self.geometry.icon(
                        &mut self.canvas,
                        Icon::Lightning,
                        bolt,
                        theme.highlight_bg,
                    )?;
                }
            }
            right = layout.pill.x() - self.px(20.);
        }
        if let Screen::Games { system } = screen {
            let title = system_title(system, data, &self.catalog);
            let pos = format!(
                "{}/{}",
                if data.games(system).is_empty() {
                    0
                } else {
                    cursor + 1
                },
                data.games(system).len()
            );
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
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let Frame {
            width, height, now, ..
        } = self.frame;
        let systems = App::system_rows(data);
        let (key, len, collections, is_games) = match screen {
            Screen::Games { system } => (system.clone(), data.games(system).len(), 0, true),
            _ => (
                "@systems".to_owned(),
                systems.len(),
                systems.iter().take_while(|s| s.starts_with('@')).count(),
                false,
            ),
        };
        let top = (theme.header_height * height as f32).round() as i32 + self.px(6.);
        let margin = (theme.margin * height as f32).round() as i32;
        let bottom =
            height as i32 - (theme.footer_height * height as f32).round() as i32 - self.px(6.);
        let list_width = if is_games {
            (theme.list_width * width as f32).round() as u32
        } else {
            let panel = system_art::panel_rect(self.frame, theme);
            (panel.x() - self.px(system_art::PANEL_GAP) - margin).max(1) as u32
        }
        .max(1);
        let [row, selected] = if is_games {
            [theme.game_row_height, theme.game_selected_height]
        } else {
            [theme.row_height, theme.selected_height]
        }
        .map(|h| (h * height as f32).round());
        let extra = selected - row;
        let gap = self.px(2.) as f32;
        let separator = if collections > 0 {
            self.px(15.) as f32
        } else {
            0.
        };
        let y_of =
            |i: usize| i as f32 * (row + gap) + if i >= collections { separator } else { 0. };
        let target = y_of(cursor);
        let available = (bottom - top).max(1) as f32;
        let total = len as f32 * (row + gap) + separator + extra;
        let mut scroll = self.list.scroll.target;
        if target < scroll {
            scroll = target;
        }
        if target + selected > scroll + available {
            scroll = target + selected - available;
        }
        scroll = scroll.clamp(0., (total - available).max(0.));
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
        let offset = self.list.scroll.value(now);
        self.canvas.set_clip_rect(Rect::new(
            margin,
            top,
            list_width,
            (bottom - top).max(1) as u32,
        ));
        if len > 0 {
            let pill = Rect::new(
                margin,
                top + (self.list.pill.value(now) - offset).round() as i32,
                list_width,
                selected.max(1.) as u32,
            );
            self.rounded(
                pill,
                theme.radius_item * height as f32,
                theme.highlight_bg,
                None,
            )?;
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
        // Visit only potentially visible rows, even in a ten-thousand-game list.
        let first = ((offset - separator - extra).max(0.) / (row + gap)).floor() as usize;
        let last = ((offset + available) / (row + gap)).ceil() as usize + 1;
        for i in first..len.min(last) {
            let y = top + (y_of(i) + if i > cursor { extra } else { 0. } - offset).round() as i32;
            let rh = if i == cursor { selected } else { row }.max(1.) as u32;
            if y + rh as i32 <= top || y >= bottom {
                continue;
            }
            let selected = i == cursor;
            let (face, size, c) = row_style(theme, i, collections, cursor, is_games);
            let mut x = margin + self.px(14.);
            let mut right = margin + list_width as i32 - self.px(14.);
            let name = if let Screen::Games { system } = screen {
                let game = &data.games(system)[i];
                if game.favorite {
                    let star = self.frame.square(
                        right - self.px(14.),
                        y + (rh as i32 - self.px(14.)) / 2,
                        14.,
                    );
                    self.geometry.icon(
                        &mut self.canvas,
                        Icon::Star(true),
                        star,
                        selection(selected, theme.highlight_bg, theme.highlight_text),
                    )?;
                    right -= self.px(24.);
                }
                game.name.clone()
            } else {
                let name = systems.get(i).expect("visible system row");
                let count = if name.starts_with('@') {
                    data.collection_len(name)
                } else {
                    data.systems()
                        .iter()
                        .find(|s| s.name == *name)
                        .map_or(0, |s| s.game_count as usize)
                };
                let count = count.to_string();
                let face = selection(selected, Face::Mono, Face::MonoMedium);
                let color = selection(selected, theme.dim_text, theme.accent_text_secondary);
                right = self.packed_label(
                    &count,
                    (face, 13., color),
                    (right, y, rh),
                    u32::MAX,
                    Align::Right,
                )? - self.px(14.);
                if name.starts_with('@') {
                    let icon = self
                        .frame
                        .square(x, y + (rh as i32 - self.px(16.)) / 2, 16.);
                    if name == "@favorites" {
                        self.geometry.icon(
                            &mut self.canvas,
                            Icon::Star(false),
                            icon,
                            selection(selected, theme.highlight_bg, theme.highlight_text),
                        )?;
                    } else {
                        self.geometry.icon(&mut self.canvas, Icon::Clock, icon, c)?;
                    }
                    x += self.px(26.);
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
    fn cover(
        &mut self,
        system: &str,
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        cover: Option<&Texture<'_>>,
    ) -> Result<(), String> {
        let Frame {
            width, height, now, ..
        } = self.frame;
        let rect = pixels(theme.cover, width, height);
        self.panel(
            rect,
            theme.radius_card * height as f32 * 0.75,
            theme.surface,
            theme.border,
        )?;
        let game = data.games(system).get(cursor);
        let key = game.map(|g| g.path.clone());
        if self.cover_key != key {
            self.cover_key = key;
            self.cover_visible = false;
            self.cover_fade = Tween::new(0., Duration::from_millis(120));
        }
        if let Some(cover) = cover {
            if !self.cover_visible {
                self.cover_visible = true;
                self.cover_fade.retarget(1., now);
            }
            let size = cover.query();
            let scale = ((rect.width().saturating_sub(2)) as f32 / size.width as f32)
                .min((rect.height().saturating_sub(2)) as f32 / size.height as f32);
            let w = (size.width as f32 * scale).round().max(1.) as u32;
            let h = (size.height as f32 * scale).round().max(1.) as u32;
            let dst = Rect::new(
                rect.x() + (rect.width() - w) as i32 / 2,
                rect.y() + (rect.height() - h) as i32 / 2,
                w,
                h,
            );
            self.geometry.cover(
                &mut self.canvas,
                cover,
                dst,
                (theme.radius_card * height as f32 * 0.75 - 1.).max(0.),
                (self.cover_fade.value(now) * 255.).round() as u8,
            )?;
        } else {
            self.cover_visible = false;
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
        if let Some(game) = game {
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
            let pw = self.measure(&played, Face::Regular, 12.)?;
            let h = self.px(20.) as u32;
            let y = rect.bottom() + self.px(9.);
            let gap = if pw > 0 { self.px(8.) as u32 } else { 0 };
            self.label(
                &parts.join(" · "),
                (Face::Regular, 12., theme.muted),
                Rect::new(rect.x(), y, rect.width().saturating_sub(pw + gap).max(1), h),
            )?;
            self.label_aligned(
                &played,
                (Face::Regular, 12., theme.muted),
                Rect::new(rect.right() - pw as i32, y, pw.max(1), h),
                Align::Right,
            )?;
        }
        Ok(())
    }
    fn footer(&mut self, app: &App, data: &dyn DataSource, theme: &Theme) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let h = (theme.footer_height * height as f32).round().max(1.) as u32;
        let y = height as i32 - h as i32;
        self.divider(0, y, width, theme.border)?;
        let hint_keys: &[(char, &str)] = match app.screen {
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
        if matches!(app.screen, Screen::Systems) {
            self.label(
                &library_summary(data, &self.catalog),
                (Face::Regular, 13., theme.muted),
                Rect::new(self.px(24.), y, (left - self.px(40.)).max(1) as u32, h),
            )?;
        }
        Ok(())
    }
    fn hints(
        &mut self,
        hints: &[(char, &str)],
        theme: &Theme,
        width: u32,
        y: i32,
        h: u32,
    ) -> Result<i32, String> {
        let mut right = width as i32 - self.px(24.);
        for &(button, key) in hints.iter().rev() {
            let label = self.catalog.text(key);
            right = self.packed_label(
                &label,
                (Face::Regular, 13., theme.muted),
                (right, y, h),
                u32::MAX,
                Align::Left,
            )? - self.px(6.)
                - self.px(20.);
            let badge = self
                .frame
                .square(right, y + (h as i32 - self.px(20.)) / 2, 20.);
            let size = self.px(11.).clamp(8, 128) as u16;
            self.text
                .as_mut()
                .ok_or_else(|| self.catalog.text("error.font_init"))?
                .draw_badge(
                    &mut self.canvas,
                    button,
                    size,
                    theme.badge,
                    theme.text,
                    badge,
                )?;
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
        let Frame {
            width, height, now, ..
        } = self.frame;
        // The mockup combines a dimmed backdrop and its base scrim.
        self.scrim(theme, 1.2)?;
        let items = app.quick_menu_items(data);
        let has_core = matches!(items.first(), Some(QuickItem::Core));
        let panel_h = if has_core { 300. } else { 256. };
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
        let divider_index = if has_core { 2 } else { 1 };
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
                QuickItem::Core => "menu.core",
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
    fn notice(&mut self, message: &str, theme: &Theme) -> Result<(), String> {
        self.scrim(theme, 1.0)?;
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

fn row_style(
    theme: &Theme,
    i: usize,
    collections: usize,
    cursor: usize,
    games: bool,
) -> (Face, f32, Rgba) {
    let color = if i < collections {
        theme.muted
    } else if i.abs_diff(cursor) >= 4 {
        theme.dim_text
    } else if i.abs_diff(cursor) >= 3 {
        theme.text_secondary
    } else {
        theme.text
    };
    (
        selection(i == cursor, Face::Regular, Face::Semibold),
        theme.font_size * 480. - if games { 1. } else { 0. } + if i == cursor { 1. } else { 0. },
        selection(i == cursor, color, theme.highlight_text),
    )
}
fn library_summary(data: &dyn DataSource, catalog: &I18n) -> String {
    let systems = data.systems().iter().filter(|s| s.game_count > 0).count();
    let games = data
        .systems()
        .iter()
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
    root: Option<&Path>,
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
    let top = (theme.header_height * height as f32).round() as i32 + frame.px(6.);
    let bottom =
        height as i32 - (theme.footer_height * height as f32).round() as i32 - frame.px(6.);
    let margin = (theme.margin * height as f32).round() as i32;
    let panel = system_art::panel_rect(frame, theme);
    let list_width = (panel.x() - frame.px(system_art::PANEL_GAP) - margin).max(1) as u32;
    let row = (theme.row_height * height as f32).round();
    let selected = (theme.selected_height * height as f32).round();
    for (i, name) in systems.iter().enumerate() {
        let y = top
            + (i as f32 * (row + frame.px(2.) as f32)
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
        let count = if name.starts_with('@') {
            data.collection_len(name)
        } else {
            data.systems()
                .iter()
                .find(|s| s.name == *name)
                .map_or(0, |s| s.game_count as usize)
        };
        let count_width = label(
            &mut prepared,
            &count.to_string(),
            (
                selection(i == 0, Face::Mono, Face::MonoMedium),
                13.,
                selection(i == 0, theme.dim_text, theme.accent_text_secondary),
            ),
            None,
        )?;
        let x = margin
            + frame.px(14.)
            + if name.starts_with('@') {
                frame.px(26.)
            } else {
                0
            };
        let right = margin + list_width as i32 - frame.px(14.) - count_width as i32 - frame.px(14.);
        let title = system_title(name, data, catalog);
        let (face, size, color) = row_style(theme, i, collections, 0, false);
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
                let font = ttf.load_font(&path, size as u16).unwrap();
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
        assert_eq!(on.pill.width(), 155);
        assert_eq!(off.pill.width(), 127);
        assert_eq!(on.clock.x() - on.wifi.unwrap().right(), 12);
        assert_eq!(on.battery.unwrap().x() - on.clock.right(), 12);
        assert_eq!(on.percent.unwrap().x() - on.battery.unwrap().right(), 6);
    }
}
