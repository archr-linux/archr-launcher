//! Grafite presentation at 640×480, scaled by height, with event-driven idle.
use crate::{
    i18n::I18n,
    motion::Tween,
    primitives::{Geometry, Shadow},
    startup::StartupTrace,
    startup_phase,
    state::{App, DataSource, QuickItem, Screen},
    status::Status,
    text::{color, Align, Face, Style, TextCache},
    theme::{Rect01, Rgba, Theme},
};
use sdl2::{
    image::{InitFlag, Sdl2ImageContext},
    rect::Rect,
    render::{BlendMode, Canvas, Texture, TextureCreator},
    ttf::Sdl2TtfContext,
    video::{Window, WindowContext},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

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
    _image: Sdl2ImageContext,
    text: Option<TextCache<'font, 'texture>>,
    shadow: Option<Shadow<'texture>>,
    geometry: Geometry,
    status: Option<Status>,
    list: ListMotion,
    menu_pill: Tween,
    menu_open: bool,
    cover_key: Option<PathBuf>,
    cover_visible: bool,
    cover_fade: Tween,
    animating: bool,
    scale: f32,
    pub catalog: I18n,
    pub loading: bool,
    pub banner: Option<String>,
    pub screenshot: Option<PathBuf>,
    pub transition: Option<String>,
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
        let mut builder = video.window("archr-launcher", width, height);
        builder.position_centered();
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
        let image = sdl2::image::init(InitFlag::PNG | InitFlag::JPG)?;
        startup_phase!(trace, "SDL_image init");
        Ok(Self {
            startup: trace,
            sdl,
            canvas,
            _image: image,
            text: None,
            shadow: None,
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
            scale: height as f32 / 480.,
            catalog: I18n::default(),
            loading: false,
            banner: None,
            screenshot: None,
            transition: None,
        })
    }
    pub fn configure(
        &mut self,
        ttf: &'font Sdl2TtfContext,
        creator: &'texture TextureCreator<WindowContext>,
        theme: &Theme,
        root: Option<&Path>,
    ) -> Result<(), String> {
        self.scale = self.canvas.output_size()?.1 as f32 / 480.;
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
        let mut text = TextCache::new(ttf, creator, paths);
        text.trace_startup(self.startup);
        text.measure(
            &self.catalog.text("systems.title"),
            Face::Semibold,
            self.px(12.) as u16,
        )
        .map_err(|error| {
            if error.starts_with("error.") {
                self.catalog.text(&error)
            } else {
                error
            }
        })?;
        text.prepare_badges(
            self.px(20.) as u32,
            self.px(11.).clamp(8, 128) as u16,
            theme.badge,
            theme.text,
        )?;
        self.text = Some(text);
        self.shadow = Some(Shadow::new(creator)?);
        startup_phase!(self.startup, "shadow texture");
        self.status = Some(Status::new(root, Instant::now()));
        startup_phase!(self.startup, "status initialization");
        Ok(())
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
        self.status.as_mut().is_some_and(|s| s.poll(Instant::now()))
    }
    fn px(&self, value: f32) -> i32 {
        (value * self.scale).round().max(1.) as i32
    }
    fn label(
        &mut self,
        text: &str,
        face: Face,
        size: f32,
        c: Rgba,
        panel: Rect,
        align: Align,
    ) -> Result<(), String> {
        let size = self.px(size).clamp(8, 128) as u16;
        self.text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?
            .draw(
                &mut self.canvas,
                text,
                Style {
                    face,
                    size,
                    color: c,
                },
                panel,
                align,
            )
            .map_err(|error| {
                if error.starts_with("error.") {
                    self.catalog.text(&error)
                } else {
                    error
                }
            })
    }
    fn measure(&mut self, text: &str, face: Face, size: f32) -> Result<u32, String> {
        let size = self.px(size).clamp(8, 128) as u16;
        self.text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?
            .measure(text, face, size)
            .map_err(|error| {
                if error.starts_with("error.") {
                    self.catalog.text(&error)
                } else {
                    error
                }
            })
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
    fn shadow(&mut self, rect: Rect) -> Result<(), String> {
        if let Some(shadow) = &self.shadow {
            shadow.draw(&mut self.canvas, rect, self.scale)?;
        }
        Ok(())
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
        let (width, height) = self.canvas.output_size()?;
        self.scale = height as f32 / 480.;
        let now = Instant::now();
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.clear();
        let (backdrop, cursor) = app.backdrop();
        self.header(backdrop, cursor, data, theme, width, height)?;
        self.list(app, backdrop, cursor, data, theme, width, height, now)?;
        if let Screen::Games { system } = backdrop {
            self.cover(system, cursor, data, theme, cover, width, height, now)?;
        }
        self.footer(app, data, theme, width, height)?;
        if matches!(app.screen, Screen::QuickMenu { .. }) {
            self.menu(app, data, theme, width, height, now)?;
        } else {
            self.menu_open = false;
        }
        if let Some(banner) = self.banner.clone() {
            self.notice(&banner, theme, width, height)?;
        }
        if let Some(message) = self.transition.clone() {
            self.notice(&message, theme, width, height)?;
        }
        self.animating = self.list.scroll.active(now)
            || self.list.pill.active(now)
            || self.menu_pill.active(now)
            || self.cover_fade.active(now);
        // The explicit SNAPSHOT debug mode is the only draw-time disk access.
        if let Some(path) = self.screenshot.take() {
            let rgba = self
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)?;
            image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)
                .map_err(|e| e.to_string())?;
        }
        startup_phase!(self.startup, "first frame draw");
        self.canvas.present();
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
        width: u32,
        height: u32,
    ) -> Result<(), String> {
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
            let layout = status_layout(right, h, self.scale, clock_width, percent_width, wifi);
            self.rounded(
                layout.pill,
                layout.pill.height() as f32 / 2.,
                theme.surface,
                Some(theme.border),
            )?;
            if let Some(icon) = layout.wifi {
                self.canvas.set_draw_color(color(theme.highlight_bg));
                let x = icon.center().x();
                // The arcs and dot occupy ten pixels, centered within the 16px box.
                let y = icon.center().y() + self.px(3.);
                for radius in [4., 8.] {
                    let r = self.px(radius);
                    let points = [
                        sdl2::rect::Point::new(x - r, y - r / 2),
                        sdl2::rect::Point::new(x - r / 2, y - r),
                        sdl2::rect::Point::new(x + r / 2, y - r),
                        sdl2::rect::Point::new(x + r, y - r / 2),
                    ];
                    self.canvas.draw_lines(points.as_slice())?;
                }
                self.canvas.fill_rect(Rect::new(
                    x - self.px(1.),
                    y,
                    self.px(2.) as u32,
                    self.px(2.) as u32,
                ))?;
            }
            self.label(
                &clock,
                Face::Mono,
                13.,
                theme.muted,
                layout.clock,
                Align::Left,
            )?;
            if let (Some(b), Some(icon), Some(panel), Some(text)) =
                (battery, layout.battery, layout.percent, percent)
            {
                self.label(&text, Face::Mono, 13., theme.muted, panel, Align::Left)?;
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
                        self.scale,
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
                self.rounded(nub, self.scale, theme.muted, None)?;
                if b.charging {
                    let bolt = Rect::new(
                        battery.x() + self.px(5.),
                        battery.y() + self.px(1.),
                        self.px(9.) as u32,
                        self.px(10.) as u32,
                    );
                    self.geometry
                        .lightning(&mut self.canvas, bolt, theme.highlight_bg)?;
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
                Face::Semibold,
                15.,
                theme.text,
                Rect::new(pad, 0, title_w, h),
                Align::Left,
            )?;
            self.label(
                &pos,
                Face::Mono,
                13.,
                theme.muted,
                Rect::new(pad + title_w as i32 + self.px(10.), 0, pw, h),
                Align::Left,
            )?;
        } else {
            let mut x = pad;
            for glyph in self.catalog.text("systems.title").chars() {
                let glyph = &glyph.to_string();
                let w = self.measure(glyph, Face::Semibold, 12.)?;
                self.label(
                    glyph,
                    Face::Semibold,
                    12.,
                    theme.text,
                    Rect::new(x, 0, w, h),
                    Align::Left,
                )?;
                x += w as i32 + self.px(1.);
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        app: &App,
        screen: &Screen,
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        width: u32,
        height: u32,
        now: Instant,
    ) -> Result<(), String> {
        let systems = app.system_rows(data);
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
            width.saturating_sub(margin.max(0) as u32 * 2)
        }
        .max(1);
        let row = ((if is_games {
            theme.game_row_height
        } else {
            theme.row_height
        }) * height as f32)
            .round();
        let selected = ((if is_games {
            theme.game_selected_height
        } else {
            theme.selected_height
        }) * height as f32)
            .round();
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
            let distance = i.abs_diff(cursor);
            let c = if selected {
                theme.highlight_text
            } else if i < collections {
                theme.muted
            } else if distance >= 4 {
                theme.dim_text
            } else if distance >= 3 {
                theme.text_secondary
            } else {
                theme.text
            };
            let face = if selected {
                Face::Semibold
            } else {
                Face::Regular
            };
            let size = theme.font_size * 480. - if is_games { 1. } else { 0. }
                + if selected { 1. } else { 0. };
            let mut x = margin + self.px(14.);
            let mut right = margin + list_width as i32 - self.px(14.);
            let name = if let Screen::Games { system } = screen {
                let game = &data.games(system)[i];
                if game.favorite {
                    let star = Rect::new(
                        right - self.px(14.),
                        y + (rh as i32 - self.px(14.)) / 2,
                        self.px(14.) as u32,
                        self.px(14.) as u32,
                    );
                    self.geometry.star(
                        &mut self.canvas,
                        star,
                        if selected {
                            theme.highlight_text
                        } else {
                            theme.highlight_bg
                        },
                        true,
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
                let cw = self.measure(
                    &count,
                    if selected {
                        Face::MonoMedium
                    } else {
                        Face::Mono
                    },
                    13.,
                )?;
                self.label(
                    &count,
                    if selected {
                        Face::MonoMedium
                    } else {
                        Face::Mono
                    },
                    13.,
                    if selected {
                        theme.accent_text_secondary
                    } else {
                        theme.dim_text
                    },
                    Rect::new(right - cw as i32, y, cw, rh),
                    Align::Right,
                )?;
                right -= cw as i32 + self.px(14.);
                if name.starts_with('@') {
                    let icon = Rect::new(
                        x,
                        y + (rh as i32 - self.px(16.)) / 2,
                        self.px(16.) as u32,
                        self.px(16.) as u32,
                    );
                    if name == "@favorites" {
                        self.geometry.star(
                            &mut self.canvas,
                            icon,
                            if selected {
                                theme.highlight_text
                            } else {
                                theme.highlight_bg
                            },
                            false,
                        )?;
                    } else {
                        self.geometry.clock_icon(&mut self.canvas, icon, c)?;
                    }
                    x += self.px(26.);
                }
                system_title(name, data, &self.catalog)
            };
            self.label(
                &name,
                face,
                size,
                c,
                Rect::new(x, y, (right - x).max(1) as u32, rh),
                Align::Left,
            )?;
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
                Face::Regular,
                16.,
                theme.muted,
                Rect::new(
                    margin + self.px(14.),
                    top,
                    list_width.saturating_sub(self.px(28.) as u32).max(1),
                    self.px(38.) as u32,
                ),
                Align::Left,
            )?;
        }
        self.canvas.set_clip_rect(None);
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn cover(
        &mut self,
        system: &str,
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
        cover: Option<&Texture<'_>>,
        width: u32,
        height: u32,
        now: Instant,
    ) -> Result<(), String> {
        let rect = pixels(theme.cover, width, height);
        self.shadow(rect)?;
        self.rounded(
            rect,
            theme.radius_card * height as f32 * 0.75,
            theme.surface,
            Some(theme.border),
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
            self.label(
                &system_title(system, data, &self.catalog),
                Face::Regular,
                13.,
                theme.dim_text,
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
                .map(|timestamp| {
                    self.catalog.relative_time(
                        timestamp,
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    )
                })
                .unwrap_or_default();
            let pw = self.measure(&played, Face::Regular, 12.)?;
            let h = self.px(20.) as u32;
            let y = rect.bottom() + self.px(9.);
            let gap = if pw > 0 { self.px(8.) as u32 } else { 0 };
            self.label(
                &parts.join(" · "),
                Face::Regular,
                12.,
                theme.muted,
                Rect::new(rect.x(), y, rect.width().saturating_sub(pw + gap).max(1), h),
                Align::Left,
            )?;
            self.label(
                &played,
                Face::Regular,
                12.,
                theme.muted,
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
        width: u32,
        height: u32,
    ) -> Result<(), String> {
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
            let systems = data.systems().iter().filter(|s| s.game_count > 0).count();
            let games = data
                .systems()
                .iter()
                .map(|s| u64::from(s.game_count))
                .sum::<u64>();
            self.label(
                &self.catalog.format(
                    "library.summary",
                    &[
                        (
                            "systems",
                            &self.catalog.plural(
                                "systems.count",
                                systems as f64,
                                &[("count", &systems.to_string())],
                            ),
                        ),
                        (
                            "games",
                            &self.catalog.plural(
                                "games.count",
                                games as f64,
                                &[("count", &games.to_string())],
                            ),
                        ),
                    ],
                ),
                Face::Regular,
                13.,
                theme.muted,
                Rect::new(self.px(24.), y, (left - self.px(40.)).max(1) as u32, h),
                Align::Left,
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
            let label = label.as_str();
            let w = self.measure(label, Face::Regular, 13.)?;
            self.label(
                label,
                Face::Regular,
                13.,
                theme.muted,
                Rect::new(right - w as i32, y, w, h),
                Align::Left,
            )?;
            right -= w as i32 + self.px(6.) + self.px(20.);
            let badge = Rect::new(
                right,
                y + (h as i32 - self.px(20.)) / 2,
                self.px(20.) as u32,
                self.px(20.) as u32,
            );
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
    fn scrim(
        &mut self,
        theme: &Theme,
        width: u32,
        height: u32,
        strength: f32,
    ) -> Result<(), String> {
        let scrim = Rgba {
            a: (f32::from(theme.scrim.a) * strength).round().min(255.) as u8,
            ..theme.scrim
        };
        self.canvas.set_draw_color(color(scrim));
        self.canvas.fill_rect(Rect::new(0, 0, width, height))
    }
    fn menu(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        width: u32,
        height: u32,
        now: Instant,
    ) -> Result<(), String> {
        // The mockup combines a dimmed backdrop and its base scrim.
        self.scrim(theme, width, height, 1.2)?;
        let items = app.quick_menu_items(data);
        let has_core = matches!(items.first(), Some(QuickItem::Core { .. }));
        let panel_h = self.px(if has_core { 300. } else { 256. }) as u32;
        let panel_w = (self.px(340.) as u32)
            .min(width.saturating_sub(self.px(32.) as u32))
            .max(1);
        let panel = Rect::new(
            (width - panel_w) as i32 / 2,
            // Optical centering: the approved modal sits 20px above center.
            ((height.saturating_sub(panel_h)) as i32 / 2 - self.px(20.)).max(0),
            panel_w,
            panel_h,
        );
        self.shadow(panel)?;
        self.rounded(
            panel,
            theme.radius_card * height as f32,
            theme.surface_elevated,
            Some(theme.border),
        )?;
        let (title, subtitle) = if let Screen::QuickMenu {
            system: Some(system),
            game: Some(game),
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
            Face::Semibold,
            16.,
            theme.text,
            Rect::new(x, panel.y() + self.px(16.), content_w, self.px(24.) as u32),
            Align::Left,
        )?;
        self.label(
            &subtitle,
            Face::Regular,
            12.,
            theme.muted,
            Rect::new(x, panel.y() + self.px(40.), content_w, self.px(16.) as u32),
            Align::Left,
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
            let c = if selected {
                theme.highlight_text
            } else {
                theme.text
            };
            let label = match item {
                QuickItem::Core { .. } => "menu.core",
                QuickItem::Suspend => "menu.suspend",
                QuickItem::Reboot => "menu.reboot",
                QuickItem::Shutdown => "menu.shutdown",
            };
            let label = self.catalog.text(label);
            let label = label.as_str();
            let face = if selected {
                Face::Semibold
            } else {
                Face::Regular
            };
            let mut label_width = content_w;
            if let QuickItem::Core { core, .. } = item {
                let default_core = self.catalog.text("menu.default");
                let core = if core.is_empty() { &default_core } else { core };
                let space = self.px(8.);
                let lw = self.measure("L", Face::Mono, 12.)?;
                let rw = self.measure("R", Face::Mono, 12.)?;
                let cw = self.measure(core, Face::Regular, 15.)?.min(content_w / 2);
                let end = panel.right() - self.px(24.);
                let cc = if selected {
                    theme.accent_text_secondary
                } else {
                    theme.muted
                };
                self.label(
                    "R",
                    Face::Mono,
                    12.,
                    cc,
                    Rect::new(end - rw as i32, y, rw, item_height),
                    Align::Left,
                )?;
                self.label(
                    core,
                    Face::Regular,
                    15.,
                    c,
                    Rect::new(end - rw as i32 - space - cw as i32, y, cw, item_height),
                    Align::Right,
                )?;
                self.label(
                    "L",
                    Face::Mono,
                    12.,
                    cc,
                    Rect::new(
                        end - rw as i32 - space * 2 - cw as i32 - lw as i32,
                        y,
                        lw,
                        item_height,
                    ),
                    Align::Left,
                )?;
                label_width = content_w
                    .saturating_sub(cw + lw + rw + (space * 3) as u32)
                    .max(1);
            }
            self.label(
                label,
                face,
                15.,
                c,
                Rect::new(x, y, label_width, item_height),
                Align::Left,
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
    fn notice(
        &mut self,
        message: &str,
        theme: &Theme,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.scrim(theme, width, height, 1.0)?;
        let lines: Vec<_> = message.split('\n').take(3).collect();
        let w = (self.px(380.) as u32)
            .min(width.saturating_sub(self.px(48.) as u32))
            .max(1);
        let h = self.px(64. + lines.len() as f32 * 24.) as u32;
        let panel = Rect::new(
            (width - w) as i32 / 2,
            (height.saturating_sub(h)) as i32 / 2,
            w,
            h,
        );
        self.shadow(panel)?;
        self.rounded(
            panel,
            theme.radius_card * height as f32,
            theme.surface_elevated,
            Some(theme.border),
        )?;
        for (i, line) in lines.iter().enumerate() {
            self.label(
                line,
                Face::Semibold,
                16.,
                theme.text,
                Rect::new(
                    panel.x() + self.px(24.),
                    panel.y() + self.px(32. + i as f32 * 24.),
                    w.saturating_sub(self.px(48.) as u32).max(1),
                    self.px(24.) as u32,
                ),
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

#[cfg(test)]
mod tests {
    use super::*;

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
