//! SDL window, fractional layout and a bounded rendered-text texture cache.
use crate::{
    state::{App, DataSource, QuickItem, Screen},
    theme::{Rect01, Rgba, Theme},
};
use sdl2::{
    image::{InitFlag, Sdl2ImageContext},
    pixels::Color,
    rect::Rect,
    render::{Canvas, Texture, TextureCreator},
    ttf::{Font, Sdl2TtfContext},
    video::{Window, WindowContext},
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

struct TextCache<'font, 'texture> {
    font: Font<'font, 'static>,
    creator: &'texture TextureCreator<WindowContext>,
    entries: VecDeque<(String, Rgba, Texture<'texture>)>,
}
impl TextCache<'_, '_> {
    fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        text: &str,
        color: Rgba,
        x: i32,
        y: i32,
    ) -> Result<(), String> {
        let text: String = text.chars().take(160).collect();
        if text.is_empty() {
            return Ok(());
        }
        let index = self
            .entries
            .iter()
            .position(|(key, rgba, _)| key == &text && *rgba == color);
        let entry = match index {
            Some(index) => self.entries.remove(index).expect("located cache entry"),
            None => {
                let mut display = text.clone();
                let limit = canvas
                    .info()
                    .max_texture_width
                    .min(canvas.output_size()?.0)
                    .max(1);
                while !display.is_empty()
                    && self
                        .font
                        .size_of(&display)
                        .map_err(|error| error.to_string())?
                        .0
                        > limit
                {
                    display.pop();
                }
                if display.is_empty() {
                    return Ok(());
                }
                let surface = self
                    .font
                    .render(&display)
                    .blended(sdl_color(color))
                    .map_err(|error| error.to_string())?;
                let texture = self
                    .creator
                    .create_texture_from_surface(surface)
                    .map_err(|error| error.to_string())?;
                (text, color, texture)
            }
        };
        let size = entry.2.query();
        canvas.copy(&entry.2, None, Rect::new(x, y, size.width, size.height))?;
        self.entries.push_back(entry);
        while self.entries.len() > 128 {
            self.entries.pop_front();
        }
        Ok(())
    }
}

pub struct Ui<'font, 'texture> {
    pub sdl: sdl2::Sdl,
    pub canvas: Canvas<Window>,
    _image: Sdl2ImageContext,
    text: Option<TextCache<'font, 'texture>>,
    pub loading: bool,
    pub banner: Option<String>,
    pub screenshot: Option<PathBuf>,
}
impl<'font, 'texture> Ui<'font, 'texture> {
    pub fn new(width: u32, height: u32, windowed: bool) -> Result<Self, String> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let mut builder = video.window("archr-launcher", width, height);
        builder.position_centered();
        if !windowed {
            builder.fullscreen_desktop();
        }
        let window = builder.build().map_err(|error| error.to_string())?;
        let mut renderer = window.into_canvas();
        // Only the explicitly requested dummy test driver lacks acceleration.
        renderer = if video.current_video_driver() == "dummy" {
            renderer.software()
        } else {
            renderer.accelerated().present_vsync()
        };
        let canvas = renderer.build().map_err(|error| error.to_string())?;
        let image = sdl2::image::init(InitFlag::PNG | InitFlag::JPG)?;
        Ok(Self {
            sdl,
            canvas,
            _image: image,
            text: None,
            loading: false,
            banner: None,
            screenshot: None,
        })
    }

    pub fn configure(
        &mut self,
        ttf: &'font Sdl2TtfContext,
        creator: &'texture TextureCreator<WindowContext>,
        theme: &Theme,
        root: Option<&Path>,
    ) -> Result<(), String> {
        let (_, height) = self.canvas.output_size()?;
        let size = (height as f32 * theme.font_size).round().clamp(8., 128.) as u16;
        let mut fonts = Vec::new();
        if let Some(font) = &theme.font {
            fonts.push(font.clone());
        }
        for font in [
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf",
        ] {
            if let Some(root) = root {
                fonts.push(root.join(font.trim_start_matches('/')));
            }
            fonts.push(PathBuf::from(font));
        }
        for path in fonts {
            if let Ok(font) = ttf.load_font(path, size) {
                self.text = Some(TextCache {
                    font,
                    creator,
                    entries: VecDeque::new(),
                });
                return Ok(());
            }
        }
        Err("Nenhuma fonte disponível (tema, Liberation Sans ou DejaVu Sans).".to_owned())
    }

    pub fn page_size(&self, theme: &Theme) -> usize {
        (0.72 / (theme.font_size * 1.45)).floor().max(1.) as usize
    }

    pub fn draw(
        &mut self,
        app: &App,
        data: &dyn DataSource,
        theme: &Theme,
        cover: Option<&Texture<'_>>,
    ) -> Result<(), String> {
        let (width, height) = self.canvas.output_size()?;
        let margin = (theme.margin * height as f32).round() as i32;
        let line = (theme.font_size * height as f32 * 1.45).round().max(1.) as i32;
        self.canvas.set_draw_color(sdl_color(theme.background));
        self.canvas.clear();
        let title = match &app.screen {
            Screen::Systems => "Sistemas".to_owned(),
            Screen::Games { system } => system_title(system, data),
            Screen::QuickMenu { .. } => "Menu rápido".to_owned(),
        };
        self.label(&title, theme.text, margin, margin)?;
        let start = app.cursor / app.page_size.max(1) * app.page_size.max(1);
        let rows: Vec<String> = match &app.screen {
            Screen::Systems | Screen::QuickMenu { .. } => app
                .system_rows(data)
                .iter()
                .skip(start)
                .take(app.page_size)
                .map(|name| system_title(name, data))
                .collect(),
            Screen::Games { system } => data
                .games(system)
                .iter()
                .skip(start)
                .take(app.page_size)
                .map(|game| format!("{}{}", if game.favorite { "* " } else { "" }, game.name))
                .collect(),
        };
        let is_games = matches!(app.screen, Screen::Games { .. });
        let list_width = if is_games {
            (theme.list_width * width as f32).round() as u32
        } else {
            width.saturating_sub(margin.max(0) as u32 * 2)
        };
        let top = margin + line * 2;
        self.canvas.set_clip_rect(Rect::new(
            margin,
            top,
            list_width.max(1),
            (height as i32 - top - line * 2).max(1) as u32,
        ));
        for (offset, row) in rows.iter().enumerate() {
            let y = top + offset as i32 * line;
            let selected =
                start + offset == app.cursor && !matches!(app.screen, Screen::QuickMenu { .. });
            if selected {
                self.canvas.set_draw_color(sdl_color(theme.highlight_bg));
                self.canvas
                    .fill_rect(Rect::new(margin, y, list_width.max(1), line as u32))?;
            }
            self.label(
                row,
                if selected {
                    theme.highlight_text
                } else {
                    theme.text
                },
                margin + 6,
                y,
            )?;
        }
        if self.loading {
            self.label("Carregando...", theme.dim_text, margin + 6, top)?;
        } else if rows.is_empty() {
            self.label("Nenhum jogo encontrado", theme.dim_text, margin + 6, top)?;
        }
        self.canvas.set_clip_rect(None);
        if let Some(cover) = cover {
            let rect = pixels(theme.cover, width, height);
            let size = cover.query();
            let scale = (rect.width() as f32 / size.width as f32)
                .min(rect.height() as f32 / size.height as f32);
            let w = (size.width as f32 * scale).round().max(1.) as u32;
            let h = (size.height as f32 * scale).round().max(1.) as u32;
            self.canvas.copy(
                cover,
                None,
                Rect::new(
                    rect.x() + (rect.width() - w) as i32 / 2,
                    rect.y() + (rect.height() - h) as i32 / 2,
                    w,
                    h,
                ),
            )?;
        }
        if matches!(app.screen, Screen::QuickMenu { .. }) {
            self.canvas.set_draw_color(sdl_color(theme.highlight_bg));
            self.canvas.fill_rect(Rect::new(
                margin,
                top,
                width.saturating_sub(margin.max(0) as u32 * 2),
                line as u32 * 6,
            ))?;
            for (index, item) in app.quick_menu_items(data).iter().enumerate() {
                let label = match item {
                    QuickItem::Core { emulator, core } => {
                        format!("L/R Núcleo: {emulator} / {core}")
                    }
                    QuickItem::Settings => "Configurações".to_owned(),
                    QuickItem::Suspend => "Suspender".to_owned(),
                    QuickItem::Reboot => "Reiniciar".to_owned(),
                    QuickItem::Shutdown => "Desligar".to_owned(),
                };
                self.label(
                    &format!("{} {label}", if index == app.cursor { ">" } else { " " }),
                    theme.highlight_text,
                    margin + 8,
                    top + index as i32 * line,
                )?;
            }
        }
        if let Some(banner) = self.banner.clone() {
            let y = height as i32 - line * 4;
            self.canvas.set_draw_color(sdl_color(theme.highlight_bg));
            self.canvas
                .fill_rect(Rect::new(0, y, width, line as u32 * 3))?;
            for (offset, part) in banner.split('\n').take(3).enumerate() {
                self.label(part, theme.highlight_text, margin, y + offset as i32 * line)?;
            }
        }
        self.label(
            "A abrir · B voltar · X favorito · Y menu",
            theme.dim_text,
            margin,
            height as i32 - line - margin,
        )?;
        // Explicit PC debug capture only; ordinary frames never touch the disk.
        if let Some(path) = self.screenshot.take() {
            let rgba = self
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)?;
            image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)
                .map_err(|error| error.to_string())?;
        }
        self.canvas.present();
        Ok(())
    }
    fn label(&mut self, text: &str, color: Rgba, x: i32, y: i32) -> Result<(), String> {
        self.text.as_mut().ok_or("Fonte não inicializada")?.draw(
            &mut self.canvas,
            text,
            color,
            x,
            y,
        )
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
fn sdl_color(color: Rgba) -> Color {
    Color::RGBA(color.r, color.g, color.b, color.a)
}
fn system_title(name: &str, data: &dyn DataSource) -> String {
    match name {
        "@favorites" => "Favoritos".to_owned(),
        "@recent" => "Recentes".to_owned(),
        _ => data
            .systems()
            .iter()
            .find(|system| system.name == name)
            .map(|system| system.fullname.clone())
            .unwrap_or_else(|| name.to_owned()),
    }
}
