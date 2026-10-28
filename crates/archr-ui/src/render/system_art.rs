//! Systems screen artwork panel: console image or a generated typographic card.
use super::{system_title, Frame, PreparedText, Ui};
use crate::{
    covers::CoverCache,
    i18n::I18n,
    primitives::Icon,
    state::DataSource,
    text::{Align, Face, Style},
    theme::{Rgba, Theme},
};
use sdl2::{rect::Rect, render::TextureCreator, video::WindowContext};
use std::path::{Path, PathBuf};

pub(super) const PANEL_WIDTH: f32 = 250.;
pub(super) const PANEL_GAP: f32 = 14.;
pub(super) const IMAGE_BOX: (f32, f32) = (210., 160.);
const EXTENSIONS: [&str; 3] = ["png", "jpg", "webp"];

/// Systems that share the picture of another; the rest use their own name.
const IMAGE_ALIASES: &[(&str, &str)] = &[
    ("arcade", "mame"),
    ("atomiswave", "dreamcast"),
    ("cps1", "mame"),
    ("cps2", "mame"),
    ("cps3", "mame"),
    ("famicom", "nes"),
    ("fbn", "mame"),
    ("gbah", "gba"),
    ("gbav", "gba"),
    ("gbch", "gbc"),
    ("gbh", "gb"),
    ("genh", "genesis"),
    ("ggh", "gamegear"),
    ("megadrive", "genesis"),
    ("megadrive-japan", "genesis"),
    ("megadriveh", "genesis"),
    ("naomi", "dreamcast"),
    ("neocd", "neogeo"),
    ("nesh", "nes"),
    ("pcenginecd", "pcengine"),
    ("pspminis", "psp"),
    ("satellaview", "snes"),
    ("segacd", "megacd"),
    ("sfc", "snes"),
    ("snesh", "snes"),
    ("snesmsu1", "snes"),
    ("sufami", "snes"),
    ("tg16cd", "tg16"),
];

/// Watermark text where the uppercased system name would be long or unclear.
const SHORT_NAMES: &[(&str, &str)] = &[
    ("amigacd32", "CD32"),
    ("amstradcpc", "CPC"),
    ("atari2600", "2600"),
    ("atari5200", "5200"),
    ("atari7800", "7800"),
    ("atarijaguar", "JAG"),
    ("colecovision", "COLECO"),
    ("dreamcast", "DC"),
    ("gamegear", "GG"),
    ("genesis", "GEN"),
    ("mastersystem", "SMS"),
    ("megacd", "MCD"),
    ("megadrive", "MD"),
    ("megadrive-japan", "MD"),
    ("megadriveh", "MD"),
    ("pcengine", "PCE"),
    ("pcenginecd", "PCE-CD"),
    ("psx", "PS1"),
    ("saturn", "SAT"),
    ("scummvm", "SVM"),
    ("segacd", "SCD"),
    ("supergrafx", "SGX"),
    ("wonderswan", "WS"),
    ("wonderswancolor", "WSC"),
];

fn lookup(table: &[(&'static str, &'static str)], name: &str) -> Option<&'static str> {
    table.iter().find(|(key, _)| *key == name).map(|(_, v)| *v)
}

pub(super) fn short_name(name: &str) -> String {
    lookup(SHORT_NAMES, name).map_or_else(|| name.to_uppercase(), str::to_owned)
}

/// User directory first, then the theme; within each, the system's own file
/// before its alias.
fn candidates(dirs: &[&Path], name: &str) -> Vec<PathBuf> {
    let keys = [Some(name), lookup(IMAGE_ALIASES, name)];
    let mut found = Vec::new();
    for dir in dirs {
        for key in keys.iter().flatten() {
            found.extend(
                EXTENSIONS
                    .iter()
                    .map(|ext| dir.join(format!("{key}.{ext}"))),
            );
        }
    }
    found
}

/// Greedy word wrapping, splitting oversized words without dropping characters.
fn wrap(
    text: &str,
    width: u32,
    mut measure: impl FnMut(&str) -> Result<u32, String>,
) -> Result<Vec<String>, String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        if let Some(line) = lines.last_mut() {
            let joined = format!("{line} {word}");
            if measure(&joined)? <= width {
                *line = joined;
                continue;
            }
        }
        if measure(word)? <= width {
            lines.push(word.to_owned());
            continue;
        }
        let mut line = String::new();
        for c in word.chars() {
            let joined = format!("{line}{c}");
            if !line.is_empty() && measure(&joined)? > width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(c);
        }
        lines.push(line);
    }
    Ok(lines)
}

pub(super) struct SystemArt<'a> {
    covers: CoverCache<'a>,
    dirs: Vec<PathBuf>,
    wanted: Vec<Vec<PathBuf>>,
}

impl<'a> SystemArt<'a> {
    pub(super) fn new(
        creator: &'a TextureCreator<WindowContext>,
        cache: PathBuf,
        dirs: Vec<PathBuf>,
        (width, height): (u32, u32),
    ) -> Self {
        Self {
            covers: CoverCache::new(creator, cache, width, height),
            dirs,
            wanted: Vec::new(),
        }
    }

    /// Selected row first, then its neighbours; collections have no image.
    fn want(&mut self, names: &[&str]) {
        let dirs: Vec<_> = self.dirs.iter().map(PathBuf::as_path).collect();
        self.wanted = names
            .iter()
            .filter(|name| !name.starts_with('@'))
            .map(|name| candidates(&dirs, name))
            .collect();
    }

    /// Retries a request the loader was too busy to take, then uploads results.
    pub(super) fn tick(&mut self) -> bool {
        self.covers.request_candidates(&self.wanted);
        self.covers.poll()
    }
}

/// Panel between the top bar and the footer, on the right edge.
pub(super) fn panel_rect(frame: Frame, theme: &Theme) -> Rect {
    let top = (theme.header_height * frame.height as f32).round() as i32 + frame.px(6.);
    let bottom = frame.height as i32
        - (theme.footer_height * frame.height as f32).round() as i32
        - frame.px(8.);
    let margin = (theme.margin * frame.height as f32).round() as i32;
    let width = frame.px(PANEL_WIDTH) as u32;
    Rect::new(
        frame.width as i32 - margin - width as i32,
        top,
        width,
        (bottom - top).max(1) as u32,
    )
}

impl Ui<'_, '_> {
    pub(super) fn ensure_art(&mut self) {
        let Some(creator) = self.creator else { return };
        let base = self.root.clone().unwrap_or_else(|| PathBuf::from("/"));
        let dirs = [
            Some(base.join("storage/.config/archr-launcher/systems")),
            self.theme_dir.as_ref().map(|dir| dir.join("systems")),
        ]
        .into_iter()
        .flatten()
        .collect();
        if self.art.as_ref().is_some_and(|art| art.dirs == dirs) {
            return;
        }
        let size = (self.px(IMAGE_BOX.0) as u32, self.px(IMAGE_BOX.1) as u32);
        self.art = Some(SystemArt::new(
            creator,
            base.join("storage/.cache/archr-launcher/thumbs"),
            dirs,
            size,
        ));
    }

    pub(super) fn system_panel(
        &mut self,
        rows: &[String],
        cursor: usize,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let Some(name) = rows.get(cursor) else {
            return Ok(());
        };
        let rect = panel_rect(self.frame, theme);
        let radius = self.px(14.) as f32;
        self.panel(rect, radius, theme.surface, theme.border)?;
        self.canvas.set_clip_rect(rect);
        let collection = name.starts_with('@');
        let near: Vec<&str> = [Some(cursor), cursor.checked_sub(1), cursor.checked_add(1)]
            .into_iter()
            .flatten()
            .filter_map(|i| rows.get(i))
            .map(String::as_str)
            .collect();
        let mut path = None;
        if let Some(art) = &mut self.art {
            art.want(&near);
            if !collection {
                path = art.wanted.first().and_then(|paths| paths.first()).cloned();
            }
        }
        if !self.console_image(rect, path.as_ref()) {
            self.generated_card(rect, name, theme)?;
        }
        let pad = self.px(16.);
        self.rounded(
            Rect::new(
                rect.x() + pad,
                rect.y() + pad,
                self.px(28.) as u32,
                self.px(4.) as u32,
            ),
            self.px(2.) as f32,
            theme.highlight_bg,
            None,
        )?;
        self.panel_text(rect, name, data, theme)?;
        self.canvas.set_clip_rect(None);
        Ok(())
    }

    /// Draws the loaded image with a soft shadow; false while absent or loading.
    fn console_image(&mut self, rect: Rect, path: Option<&PathBuf>) -> bool {
        let (Some(art), Some(path)) = (&mut self.art, path) else {
            return false;
        };
        let Some(texture) = art.covers.get(path) else {
            return false;
        };
        let (box_w, box_h) = (self.frame.px(IMAGE_BOX.0), self.frame.px(IMAGE_BOX.1));
        let size = texture.query();
        let scale = (box_w as f32 / size.width as f32).min(box_h as f32 / size.height as f32);
        let w = (size.width as f32 * scale).round().max(1.) as u32;
        let h = (size.height as f32 * scale).round().max(1.) as u32;
        let left = rect.x() + (rect.width() as i32 - box_w) / 2;
        let dst = Rect::new(
            left + (box_w - w as i32) / 2,
            rect.y() + self.frame.px(46.) + (box_h - h as i32) / 2,
            w,
            h,
        );
        // Three stacked ellipses approximate a blurred contact shadow.
        let black = |a| Rgba {
            r: 0,
            g: 0,
            b: 0,
            a,
        };
        for (span, thick, alpha) in [(0.9, 14., 26), (0.72, 10., 30), (0.5, 6., 36)] {
            let sw = (w as f32 * span) as u32;
            let sh = self.frame.px(thick) as u32;
            let shadow = Rect::new(
                dst.center().x() - sw as i32 / 2,
                dst.bottom() - sh as i32 / 2 - self.frame.px(3.),
                sw,
                sh,
            );
            // Errors only skip the decoration.
            let _ =
                self.geometry
                    .rounded(&mut self.canvas, shadow, sh as f32 / 2., black(alpha), None);
        }
        self.canvas.copy(texture, None, dst).is_ok()
    }

    fn generated_card(&mut self, rect: Rect, name: &str, theme: &Theme) -> Result<(), String> {
        let ink = theme.surface_elevated;
        if name.starts_with('@') {
            let side = self.px(96.);
            let icon = self.frame.square(
                rect.right() - self.px(24.) - side,
                rect.y() + self.px(40.),
                96.,
            );
            let glyph = if name == "@favorites" {
                Icon::Star(false)
            } else {
                Icon::Clock
            };
            return self.geometry.icon(&mut self.canvas, glyph, icon, ink);
        }
        let label = generated_label(self.frame, rect, name, theme, |text, face, size| {
            self.measure(text, face, size)
        })?;
        self.label_aligned(&label.text, label.style, label.rect, Align::Right)
    }

    fn panel_text(
        &mut self,
        rect: Rect,
        name: &str,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let frame = self.frame;
        let text_cache = self
            .text
            .as_mut()
            .ok_or_else(|| self.catalog.text("error.font_init"))?;
        let labels = panel_labels(
            frame,
            rect,
            name,
            data,
            &self.catalog,
            theme,
            |text, face, size| text_cache.measure(text, face, frame.font_size(size)),
        )?;
        for label in labels {
            self.label(&label.text, label.style, label.rect)?;
        }
        Ok(())
    }
}

struct PanelLabel {
    text: String,
    style: (Face, f32, Rgba),
    rect: Rect,
}

fn generated_label(
    frame: Frame,
    rect: Rect,
    name: &str,
    theme: &Theme,
    mut measure: impl FnMut(&str, Face, f32) -> Result<u32, String>,
) -> Result<PanelLabel, String> {
    let text = short_name(name);
    let (face, size) = (Face::Semibold, 128.);
    let width = rect.width().saturating_sub(frame.px(24.) as u32);
    let natural = measure(&text, face, size)?.max(1);
    let size = (size * (width as f32 / natural as f32).min(1.))
        .floor()
        .max(12.);
    Ok(PanelLabel {
        text,
        style: (face, size, theme.surface_elevated),
        rect: Rect::new(
            rect.x() + frame.px(12.),
            rect.y() + frame.px(22.),
            width,
            frame.px(size * 1.25) as u32,
        ),
    })
}

fn panel_labels(
    frame: Frame,
    rect: Rect,
    name: &str,
    data: &dyn DataSource,
    catalog: &I18n,
    theme: &Theme,
    mut measure: impl FnMut(&str, Face, f32) -> Result<u32, String>,
) -> Result<Vec<PanelLabel>, String> {
    let mut labels = Vec::new();
    let mut stacked = |text: &str,
                       style: (Face, f32, Rgba),
                       (x, mut bottom, width): (i32, i32, u32),
                       step: f32|
     -> Result<i32, String> {
        let lines = wrap(text, width, |s| measure(s, style.0, style.1))?;
        let step = frame.px(step);
        for line in lines.into_iter().rev() {
            bottom -= step;
            labels.push(PanelLabel {
                text: line,
                style,
                rect: Rect::new(x, bottom, width, step as u32),
            });
        }
        Ok(bottom)
    };
    let x = rect.x() + frame.px(16.);
    let width = rect.width().saturating_sub(frame.px(32.) as u32).max(1);
    let mut bottom = rect.bottom() - frame.px(14.);
    let games = if name.starts_with('@') {
        data.collection_len(name)
    } else {
        data.systems()
            .iter()
            .find(|s| s.name == name)
            .map_or(0, |s| s.game_count as usize)
    };
    let count = |key, n: usize| catalog.plural(key, n as f64, &[("count", &n.to_string())]);
    let stats = if name.starts_with('@') {
        count("games.count", games)
    } else {
        catalog.format(
            "panel.stats",
            &[
                ("games", &count("games.count", games)),
                (
                    "favorites",
                    &count("favorites.count", data.favorite_count(name)),
                ),
            ],
        )
    };
    bottom = stacked(
        &stats,
        (Face::Mono, 12., theme.muted),
        (x, bottom, width),
        16.,
    )? - frame.px(4.);
    let title = system_title(name, data, catalog);
    bottom = stacked(
        &title,
        (Face::Semibold, 22., theme.text),
        (x, bottom, width),
        27.,
    )?;
    let maker = data
        .manufacturer(name)
        .map(|(maker, year)| {
            [maker, year]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
                .to_uppercase()
        })
        .filter(|maker| !maker.is_empty());
    if let Some(maker) = maker {
        stacked(
            &maker,
            (Face::Regular, 12., theme.muted),
            (x, bottom, width),
            18.,
        )?;
    }
    Ok(labels)
}

/// Prepare exactly the generated panel shown before the artwork worker starts.
pub(super) fn prepare_panel(
    prepared: &mut PreparedText<'_>,
    frame: Frame,
    name: &str,
    data: &dyn DataSource,
    catalog: &I18n,
    theme: &Theme,
) -> Result<(), String> {
    let rect = panel_rect(frame, theme);
    let mut labels = Vec::new();
    if !name.starts_with('@') {
        labels.push(generated_label(
            frame,
            rect,
            name,
            theme,
            |text, face, size| prepared.measure(text, face, frame.font_size(size)),
        )?);
    }
    labels.extend(panel_labels(
        frame,
        rect,
        name,
        data,
        catalog,
        theme,
        |text, face, size| prepared.measure(text, face, frame.font_size(size)),
    )?);
    for label in labels {
        let (face, size, color) = label.style;
        prepared.label(
            &label.text,
            Style {
                face,
                size: frame.font_size(size),
                color,
            },
            label.rect.width(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_prefers_user_then_theme_and_own_name_before_alias() {
        let user = Path::new("/u");
        let theme = Path::new("/t");
        let found = candidates(&[user, theme], "sfc");
        assert_eq!(found.first().unwrap(), Path::new("/u/sfc.png"));
        assert_eq!(found[3], Path::new("/u/snes.png"));
        assert_eq!(found[6], Path::new("/t/sfc.png"));
        assert_eq!(found.last().unwrap(), Path::new("/t/snes.webp"));
        assert_eq!(candidates(&[user], "nes").len(), 3);
    }

    #[test]
    fn artwork_resolves_candidates_only_after_request_and_caches_misses() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let ui = Ui::new(640, 480, true).unwrap();
        let creator = ui.canvas.texture_creator();
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user");
        let theme = root.path().join("theme");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&theme).unwrap();
        let mut art = SystemArt::new(
            &creator,
            root.path().join("cache"),
            vec![user.clone(), theme.clone()],
            (2, 2),
        );
        art.want(&["sfc", "@favorites"]);
        // Files appear after want(): only the worker may resolve them.
        image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
            .save(user.join("snes.png"))
            .unwrap();
        image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]))
            .save(theme.join("sfc.png"))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            art.tick();
            if art.covers.get(&user.join("sfc.png")).is_some() {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "artwork never loaded");
            std::thread::yield_now();
        }
        assert_eq!(
            art.covers.get(&user.join("sfc.png")).unwrap().query().width,
            2,
            "user alias takes precedence over theme's own image"
        );
        // A miss is retained, not probed again on every frame.
        art.want(&["missing"]);
        while !art.tick() {
            assert!(std::time::Instant::now() < deadline, "miss never returned");
            std::thread::yield_now();
        }
        image::RgbaImage::new(1, 1)
            .save(user.join("missing.png"))
            .unwrap();
        for _ in 0..100 {
            art.tick();
            assert!(art.covers.get(&user.join("missing.png")).is_none());
            std::thread::yield_now();
        }
    }

    #[test]
    fn artwork_invalidates_when_theme_or_configuration_changes() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let (creator, ttf);
        let mut ui = Ui::new(640, 480, true).unwrap();
        creator = ui.canvas.texture_creator();
        ttf = sdl2::ttf::init().unwrap();
        let root = tempfile::tempdir().unwrap();
        let theme = Theme::builtin();
        ui.configure(&ttf, &creator, &theme, Some(root.path()))
            .unwrap();
        ui.ensure_art();
        ui.art.as_mut().unwrap().want(&["nes"]);
        ui.theme_dir = Some(root.path().join("new-theme"));
        ui.ensure_art();
        let art = ui.art.as_ref().unwrap();
        assert_eq!(art.dirs[1], root.path().join("new-theme/systems"));
        assert!(art.wanted.is_empty(), "old requests must be discarded");
        ui.art.as_mut().unwrap().want(&["nes"]);
        ui.configure(&ttf, &creator, &theme, Some(root.path()))
            .unwrap();
        assert!(
            ui.art.is_none(),
            "reconfiguration must discard textures and misses"
        );
        ui.ensure_art();
        assert!(ui.art.as_ref().unwrap().wanted.is_empty());
    }

    #[test]
    fn short_names_use_table_then_uppercase() {
        assert_eq!(short_name("psx"), "PS1");
        assert_eq!(short_name("megadrive"), "MD");
        assert_eq!(short_name("snes"), "SNES");
    }

    #[test]
    fn alias_tables_have_sorted_unique_keys() {
        for table in [IMAGE_ALIASES, SHORT_NAMES] {
            assert!(table.windows(2).all(|w| w[0].0 < w[1].0));
        }
    }

    #[test]
    fn wrap_preserves_long_system_names_manufacturers_and_counts() {
        let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
        let ttf = sdl2::ttf::init().unwrap();
        let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/fonts");
        for (text, face, size) in [
            ("Super Nintendo Entertainment System", "Inter-SemiBold", 22),
            (
                "International Business Machines · 1981",
                "Inter-Regular",
                12,
            ),
            (
                "18446744073709551615 jogos · 18446744073709551615 favoritos",
                "JetBrainsMono-Regular",
                12,
            ),
        ] {
            let font = ttf
                .load_font(fonts.join(format!("{face}.ttf")), size)
                .unwrap();
            let lines = wrap(text, 218, |s| Ok(font.size_of(s).unwrap().0)).unwrap();
            assert_eq!(lines.join(" "), text);
            assert!(lines
                .iter()
                .all(|line| font.size_of(line).unwrap().0 <= 218));
        }
        let word = "SistemaSemEspaços日本語";
        let lines = wrap(word, 30, |s| Ok(s.chars().count() as u32 * 10)).unwrap();
        assert_eq!(lines.concat(), word);
        assert!(lines.iter().all(|line| line.chars().count() <= 3));
    }

    #[test]
    fn wrap_breaks_at_words_without_losing_overflow() {
        let width = |s: &str| Ok(s.len() as u32 * 10);
        assert_eq!(
            wrap("Super Nintendo", 200, width).unwrap(),
            ["Super Nintendo"]
        );
        assert_eq!(
            wrap("Sega Mega Drive", 100, width).unwrap(),
            ["Sega Mega", "Drive"]
        );
        assert_eq!(
            wrap("a bb ccc dddd eeeee", 30, width).unwrap(),
            ["a", "bb", "ccc", "ddd", "d", "eee", "ee"]
        );
        assert!(wrap("", 100, width).unwrap().is_empty());
    }
}
