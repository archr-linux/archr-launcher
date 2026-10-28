//! Systems screen artwork panel: console image or a generated typographic card.
use super::{system_title, Frame, Ui};
use crate::{
    covers::CoverCache,
    primitives::Icon,
    state::DataSource,
    text::{Align, Face},
    theme::{Rgba, Theme},
};
use sdl2::{rect::Rect, render::TextureCreator, video::WindowContext};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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
    ("genh", "genesis"),
    ("gbah", "gba"),
    ("gbav", "gba"),
    ("gbch", "gbc"),
    ("gbh", "gb"),
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

/// Greedy wrap into at most two lines; the second may still need an ellipsis.
fn wrap(
    text: &str,
    width: u32,
    mut measure: impl FnMut(&str) -> Result<u32, String>,
) -> Result<Vec<String>, String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let joined = lines.last().map(|line| format!("{line} {word}"));
        match joined {
            Some(joined) if lines.len() >= 2 || measure(&joined)? <= width => {
                *lines.last_mut().expect("joined from a line") = joined;
            }
            _ => lines.push(word.to_owned()),
        }
    }
    Ok(lines)
}

pub(super) struct SystemArt<'a> {
    covers: CoverCache<'a>,
    dirs: Vec<PathBuf>,
    found: HashMap<String, Option<PathBuf>>,
    wanted: Vec<PathBuf>,
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
            found: HashMap::new(),
            wanted: Vec::new(),
        }
    }

    fn path(&mut self, name: &str) -> Option<PathBuf> {
        let dirs: Vec<_> = self.dirs.iter().map(PathBuf::as_path).collect();
        self.found
            .entry(name.to_owned())
            .or_insert_with(|| candidates(&dirs, name).into_iter().find(|p| p.is_file()))
            .clone()
    }

    /// Selected row first, then its neighbours; collections have no image.
    fn want(&mut self, names: &[&str]) {
        self.wanted = names
            .iter()
            .filter(|name| !name.starts_with('@'))
            .filter_map(|name| self.path(name))
            .collect();
    }

    /// Retries a request the loader was too busy to take, then uploads results.
    pub(super) fn tick(&mut self) -> bool {
        self.covers.request(&self.wanted);
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
        if self.art.is_some() {
            return;
        }
        let Some(creator) = self.creator else { return };
        let base = self.root.clone().unwrap_or_else(|| PathBuf::from("/"));
        let dirs = [
            Some(base.join("storage/.config/archr-launcher/systems")),
            self.theme_dir.as_ref().map(|dir| dir.join("systems")),
        ];
        let size = (self.px(IMAGE_BOX.0) as u32, self.px(IMAGE_BOX.1) as u32);
        self.art = Some(SystemArt::new(
            creator,
            base.join("storage/.cache/archr-launcher/thumbs"),
            dirs.into_iter().flatten().collect(),
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
        let near: Vec<&str> = rows[cursor.saturating_sub(1)..rows.len().min(cursor + 2)]
            .iter()
            .map(String::as_str)
            .collect();
        let mut path = None;
        if let Some(art) = &mut self.art {
            art.want(&near);
            if !collection {
                path = art.path(name);
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
        let (face, size) = (Face::Semibold, 128.);
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
        let text = short_name(name);
        let glyphs: Vec<String> = text.chars().map(String::from).collect();
        let avail = rect.width() as f32 - self.px(24.) as f32;
        let advance = |ui: &mut Self, size: f32| -> Result<Vec<f32>, String> {
            glyphs
                .iter()
                .map(|g| Ok(ui.measure(g, face, size)? as f32 - 0.06 * size))
                .collect()
        };
        let natural: f32 = advance(self, size)?.iter().sum();
        let size = (size * (avail / natural).min(1.)).floor().max(24.);
        let advances = advance(self, size)?;
        let mut x = rect.right() as f32 - self.px(12.) as f32 - advances.iter().sum::<f32>();
        let line = (size * 1.25) as u32;
        for (glyph, step) in glyphs.iter().zip(advances) {
            let w = (step + 0.06 * size).ceil().max(1.) as u32;
            self.label(
                glyph,
                (face, size, ink),
                Rect::new(x as i32, rect.y() + self.px(22.), w, line),
            )?;
            x += step;
        }
        Ok(())
    }

    fn panel_text(
        &mut self,
        rect: Rect,
        name: &str,
        data: &dyn DataSource,
        theme: &Theme,
    ) -> Result<(), String> {
        let x = rect.x() + self.px(16.);
        let width = rect.width().saturating_sub(self.px(32.) as u32).max(1);
        let mut bottom = rect.bottom() - self.px(14.);
        let games = if name.starts_with('@') {
            data.collection_len(name)
        } else {
            data.systems()
                .iter()
                .find(|s| s.name == name)
                .map_or(0, |s| s.game_count as usize)
        };
        let count = |key, n: usize| {
            self.catalog
                .plural(key, n as f64, &[("count", &n.to_string())])
        };
        let stats = if name.starts_with('@') {
            count("games.count", games)
        } else {
            self.catalog.format(
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
        let line = self.px(16.);
        bottom -= line;
        self.label(
            &stats,
            (Face::Mono, 12., theme.muted),
            Rect::new(x, bottom, width, line as u32),
        )?;
        bottom -= self.px(4.);
        let title = system_title(name, data, &self.catalog);
        let lines = wrap(&title, width, |s| self.measure(s, Face::Semibold, 22.))?;
        let step = self.px(27.);
        for text in lines.iter().rev() {
            bottom -= step;
            self.label(
                text,
                (Face::Semibold, 22., theme.text),
                Rect::new(x, bottom, width, step as u32),
            )?;
        }
        let maker = data
            .manufacturer(name)
            .map(|(maker, year)| [maker, year])
            .map(|parts| {
                parts
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|parts| !parts.is_empty())
            .map(|parts| parts.join(" · ").to_uppercase());
        if let Some(maker) = maker {
            bottom -= self.px(18.);
            self.spaced(&maker, (x, bottom, width), theme.muted)?;
        }
        Ok(())
    }

    /// 12 px muted caps with 0.12 em tracking; plain (ellipsized) when too wide.
    fn spaced(
        &mut self,
        text: &str,
        (x, y, width): (i32, i32, u32),
        ink: Rgba,
    ) -> Result<(), String> {
        let style = (Face::Regular, 12., ink);
        let gap = self.px(12.) as f32 * 0.12;
        let glyphs: Vec<String> = text.chars().map(String::from).collect();
        let mut advances = Vec::new();
        for glyph in &glyphs {
            advances.push(self.measure(glyph, style.0, style.1)? as f32 + gap);
        }
        let h = self.px(16.) as u32;
        if advances.iter().sum::<f32>() > width as f32 {
            return self.label_aligned(text, style, Rect::new(x, y, width, h), Align::Left);
        }
        let mut at = x as f32;
        for (glyph, step) in glyphs.iter().zip(advances) {
            self.label(glyph, style, Rect::new(at as i32, y, step.ceil() as u32, h))?;
            at += step;
        }
        Ok(())
    }
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
    fn wrap_breaks_at_words_and_caps_at_two_lines() {
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
            ["a", "bb ccc dddd eeeee"]
        );
        assert!(wrap("", 100, width).unwrap().is_empty());
    }
}
