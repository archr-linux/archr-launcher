//! Systems screen artwork panel: console image or a generated typographic card.
use super::{pixels, selection, system_count, system_title, Frame, Ui};
use crate::icons::Glyph;
use crate::{
    appearance::mix,
    covers::CoverCache,
    i18n::I18n,
    layout::{Info, Layout, List},
    state::DataSource,
    text::{Align, Face},
    theme::{Rgba, Theme},
};
use sdl2::{rect::Rect, render::TextureCreator, video::WindowContext};
use std::path::{Path, PathBuf};

/// Decoded console pictures are this size, whatever box shows them.
const IMAGE_BOX: (f32, f32) = (210., 160.);
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
    // The Moonlight collection is drawn like a system, as its plain name.
    ("@moonlight", "MOONLIGHT"),
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
pub(super) fn wrap(
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
            .filter(|name| has_picture(name))
            .map(|name| candidates(&dirs, name.trim_start_matches('@')))
            .collect();
    }
    /// The cache key of `name`'s picture, if it can have one.
    fn key(&self, name: &str) -> Option<PathBuf> {
        let dirs: Vec<_> = self.dirs.iter().map(PathBuf::as_path).collect();
        has_picture(name)
            .then(|| {
                candidates(&dirs, name.trim_start_matches('@'))
                    .into_iter()
                    .next()
            })
            .flatten()
    }

    /// Retries a request the loader was too busy to take, then uploads results.
    pub(super) fn tick(&mut self) -> bool {
        self.covers.request_candidates(&self.wanted);
        self.covers.poll()
    }
}

fn has_picture(name: &str) -> bool {
    !name.starts_with('@') || name == crate::moonlight::SYSTEM
}
/// Entries `cursor` and its neighbours, the selection first.
fn nearby(rows: &[String], cursor: usize, reach: usize) -> Vec<&str> {
    let mut near = vec![cursor];
    for step in 1..=reach {
        near.extend(cursor.checked_sub(step));
        near.push(cursor + step);
    }
    near.into_iter()
        .filter_map(|i| rows.get(i))
        .map(String::as_str)
        .collect()
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
        layout: &Layout,
    ) -> Result<(), String> {
        let Some(name) = rows.get(cursor) else {
            return Ok(());
        };
        let height = self.frame.height as f32;
        let panel = layout.panel.map(|p| (self.area(p.rect), p.radius * height));
        if let Some((rect, radius)) = panel {
            self.panel(rect, radius, theme.surface, theme.border)?;
            self.canvas.set_clip_rect(rect);
        }
        // Tiles draw their own pictures.
        if let Some(art) = layout
            .art
            .filter(|_| layout.list.mark != crate::layout::Mark::Tile)
        {
            let rect = self.area(art.rect);
            let path = self.art.as_mut().and_then(|a| {
                a.want(&nearby(rows, cursor, 1));
                a.key(name)
            });
            if !self.console_image(rect, path.as_ref(), 255) {
                self.generated_card(panel.map_or(rect, |p| p.0), name, theme)?;
            }
        }
        if let Some((rect, _)) = panel {
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
        }
        if let Some(info) = layout.info {
            self.panel_text(&info, name, data, theme)?;
        }
        self.canvas.set_clip_rect(None);
        if let Some(dots) = layout.dots {
            self.dots(self.area(dots), rows.len(), cursor, theme)?;
        }
        Ok(())
    }

    /// Systems as a row of tiles, the selected one large in the middle.
    pub(super) fn tiles(
        &mut self,
        rows: &[String],
        cursor: usize,
        lifted: bool,
        theme: &Theme,
        list: &List,
    ) -> Result<(), String> {
        let Frame { height, now, .. } = self.frame;
        let area = self.area(list.rect);
        if self.list.key != "@tiles" {
            self.list.key = "@tiles".into();
            self.list.pill.retarget(cursor as f32, now);
            self.list.pill.snap();
        } else {
            self.list.pill.retarget(cursor as f32, now);
        }
        if rows.is_empty() {
            let text = self.catalog.text(if self.loading {
                "loading"
            } else {
                "empty.systems"
            });
            return self.label_aligned(
                &text,
                (Face::Regular, 16., theme.muted),
                area,
                Align::Center,
            );
        }
        if let Some(art) = &mut self.art {
            art.want(&nearby(rows, cursor, 2));
        }
        let at = self.list.pill.value(now);
        let [small, big, gap] = [list.row, list.selected, list.gap].map(|v| v * height as f32);
        let reach = big / 2. + gap + small / 2.;
        let (cx, cy) = (area.center().x() as f32, area.center().y() as f32);
        let span = (area.width() as f32 / (small + gap)).ceil() as usize / 2 + 2;
        let first = (at.floor() as usize).saturating_sub(span);
        let mut tiles: Vec<(usize, f32, f32)> = (first
            ..rows.len().min(at.ceil() as usize + span + 1))
            .map(|i| {
                let d = i as f32 - at;
                let (offset, side) = if d.abs() < 1. {
                    (d * reach, big + (small - big) * d.abs())
                } else {
                    (d.signum() * (reach + (d.abs() - 1.) * (small + gap)), small)
                };
                (i, cx + offset, side)
            })
            .collect();
        // Nearest last, so the selection's shadow lies over its neighbours.
        tiles.sort_by_key(|tile| std::cmp::Reverse(tile.0.abs_diff(cursor)));
        self.canvas.set_clip_rect(area);
        for (i, x, side) in tiles {
            let focus = ((side - small) / (big - small).max(1.)).clamp(0., 1.);
            let rect = Rect::new(
                (x - side / 2.).round() as i32,
                (cy - side / 2.).round() as i32,
                side.round().max(1.) as u32,
                side.round().max(1.) as u32,
            );
            let radius = list.radius * height as f32 * side / big;
            let fill = mix(theme.surface, theme.surface_elevated, focus);
            if i == cursor && lifted {
                self.rounded(
                    rect,
                    radius,
                    Rgba {
                        a: 40,
                        ..theme.highlight_bg
                    },
                    None,
                )?;
                self.geometry
                    .dashed(&mut self.canvas, rect, radius, theme.highlight_bg)?;
            } else if i == cursor {
                self.panel(rect, radius, theme.highlight_bg, theme.highlight_bg)?;
                let border = self.px(2.);
                let inner = Rect::new(
                    rect.x() + border,
                    rect.y() + border,
                    rect.width().saturating_sub(2 * border as u32).max(1),
                    rect.height().saturating_sub(2 * border as u32).max(1),
                );
                self.rounded(inner, (radius - border as f32).max(0.), fill, None)?;
            } else {
                self.rounded(rect, radius, fill, None)?;
            }
            // Neighbours sit back at 70%.
            let alpha = (255. * (0.7 + 0.3 * focus)).round() as u8;
            self.tile_face(rect, &rows[i], theme, alpha, focus)?;
        }
        self.canvas.set_clip_rect(None);
        Ok(())
    }

    /// The picture inside a tile, or the system's short name.
    fn tile_face(
        &mut self,
        rect: Rect,
        name: &str,
        theme: &Theme,
        alpha: u8,
        focus: f32,
    ) -> Result<(), String> {
        let pad = (rect.width() as f32 * 0.14).round() as i32;
        let inner = Rect::new(
            rect.x() + pad,
            rect.y() + pad,
            rect.width().saturating_sub(2 * pad as u32).max(1),
            rect.height().saturating_sub(2 * pad as u32).max(1),
        );
        let path = self.art.as_ref().and_then(|a| a.key(name));
        if self.console_image(inner, path.as_ref(), alpha) {
            return Ok(());
        }
        let ink = Rgba {
            a: alpha,
            ..mix(theme.dim_text, theme.text_secondary, focus)
        };
        if name.starts_with('@') && name != crate::moonlight::SYSTEM {
            let side = (inner.width().min(inner.height()) as f32 * 0.5) as u32;
            let glyph = selection(name == "@favorites", Glyph::Clock, Glyph::Star);
            let icon = Rect::new(
                inner.center().x() - side as i32 / 2,
                inner.center().y() - side as i32 / 2,
                side,
                side,
            );
            return self.icon(glyph, icon, ink);
        }
        let text = short_name(name);
        let mut size = rect.height() as f32 / self.frame.scale * 0.16;
        while size > 10. && self.measure(&text, Face::Semibold, size)? > inner.width() {
            size -= 1.;
        }
        self.label_aligned(&text, (Face::Semibold, size, ink), inner, Align::Center)
    }

    /// Where the selection sits among the systems: a short window of dots.
    fn dots(&mut self, rect: Rect, len: usize, cursor: usize, theme: &Theme) -> Result<(), String> {
        const MOST: usize = 5;
        let shown = len.min(MOST);
        if shown < 2 {
            return Ok(());
        }
        let start = cursor.saturating_sub(MOST / 2).min(len - shown);
        let (dot, wide, gap) = (self.px(6.), self.px(18.), self.px(6.));
        let total = dot * (shown as i32 - 1) + wide + gap * (shown as i32 - 1);
        let mut x = rect.center().x() - total / 2;
        let y = rect.center().y() - dot / 2;
        for n in start..start + shown {
            let w = if n == cursor { wide } else { dot };
            let ink = selection(n == cursor, theme.border, theme.highlight_bg);
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

    /// Draws the loaded image contained in `area`, with a soft shadow; false
    /// while absent or loading.
    fn console_image(&mut self, area: Rect, path: Option<&PathBuf>, alpha: u8) -> bool {
        let (Some(art), Some(path)) = (&mut self.art, path) else {
            return false;
        };
        let Some(texture) = art.covers.get_mut(path) else {
            return false;
        };
        let (box_w, box_h) = (area.width() as i32, area.height() as i32);
        let size = texture.query();
        let scale = (box_w as f32 / size.width as f32).min(box_h as f32 / size.height as f32);
        let w = (size.width as f32 * scale).round().max(1.) as u32;
        let h = (size.height as f32 * scale).round().max(1.) as u32;
        let dst = Rect::new(
            area.x() + (box_w - w as i32) / 2,
            area.y() + (box_h - h as i32) / 2,
            w,
            h,
        );
        // Three stacked ellipses approximate a blurred contact shadow.
        let black = |a: u8| Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: (u16::from(a) * u16::from(alpha) / 255) as u8,
        };
        for (span, thick, shade) in [(0.9, 14., 26), (0.72, 10., 30), (0.5, 6., 36)] {
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
                    .rounded(&mut self.canvas, shadow, sh as f32 / 2., black(shade), None);
        }
        texture.set_alpha_mod(alpha);
        self.canvas.copy(texture, None, dst).is_ok()
    }

    fn generated_card(&mut self, rect: Rect, name: &str, theme: &Theme) -> Result<(), String> {
        let ink = theme.surface_elevated;
        if name.starts_with('@') && name != crate::moonlight::SYSTEM {
            let side = self.px(96.);
            let icon = self.frame.square(
                rect.right() - self.px(24.) - side,
                rect.y() + self.px(40.),
                96.,
            );
            let glyph = if name == "@favorites" {
                Glyph::Star
            } else {
                Glyph::Clock
            };
            return self.icon(glyph, icon, ink);
        }
        let label = generated_label(self.frame, rect, name, theme, |text, face, size| {
            self.measure(text, face, size)
        })?;
        self.label_aligned(&label.text, label.style, label.rect, Align::Right)
    }

    fn panel_text(
        &mut self,
        info: &Info,
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
            info,
            name,
            data,
            &self.catalog,
            theme,
            |text, face, size| text_cache.measure(text, face, frame.font_size(size)),
        )?;
        for label in labels {
            self.label_aligned(&label.text, label.style, label.rect, info.align)?;
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
    let mut size = (size * (width as f32 / natural as f32).min(1.))
        .floor()
        .max(12.);
    // Glyph hinting is not linear in size, so confirm the estimate; a label
    // that overflows would be ellipsized by the text cache.
    while size > 12. && measure(&text, face, size)? > width {
        size -= 1.;
    }
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

/// Manufacturer and year as the definitions give them, without icon glyphs
/// or the "System" placeholder.
fn maker(data: &dyn DataSource, name: &str) -> Vec<String> {
    let Some((maker, year)) = data.manufacturer(name) else {
        return Vec::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for (index, part) in [maker, year].into_iter().enumerate() {
        // ES definitions embed private-use icon glyphs and put the
        // category "System" where real manufacturer/year data is absent.
        let part = part
            .chars()
            .filter(|c| {
                !matches!(*c,
                '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}')
            })
            .collect::<String>()
            .trim()
            .to_owned();
        if !part.is_empty()
            && !part.eq_ignore_ascii_case("system")
            && !(index == 1 && part == "0000")
            && !parts.iter().any(|p| p.eq_ignore_ascii_case(&part))
        {
            parts.push(part);
        }
    }
    parts
}

/// Text, style, line height and the space above it.
type Block = (String, (Face, f32, Rgba), f32, f32);
/// The system's name and details in `info`'s box, wrapped, never cut.
fn panel_labels(
    frame: Frame,
    info: &Info,
    name: &str,
    data: &dyn DataSource,
    catalog: &I18n,
    theme: &Theme,
    mut measure: impl FnMut(&str, Face, f32) -> Result<u32, String>,
) -> Result<Vec<PanelLabel>, String> {
    let rect = pixels(info.rect, frame.width, frame.height);
    let (title, meta) = (info.title * 480., info.meta * 480.);
    let games = system_count(name, data);
    let count = |key, n: usize| catalog.plural(key, n as f64, &[("count", &n.to_string())]);
    let maker = maker(data, name);
    let title_text = system_title(name, data, catalog);
    let title_block = (
        title_text,
        (Face::Semibold, title, theme.text),
        title * 27. / 22.,
    );
    let muted = |face| (face, meta, theme.muted);
    // Top to bottom: text, style, line height, space above.
    let blocks: Vec<Block> = if info.inline {
        let mut details = vec![if name == crate::moonlight::SYSTEM {
            catalog.text("moonlight.description")
        } else if data.is_tools(name) {
            count("tools.count", games)
        } else {
            count("games.count", games)
        }];
        details.extend(maker);
        vec![
            (title_block.0, title_block.1, title * 1.25, 0.),
            (
                details.join(" · "),
                muted(Face::Regular),
                meta * 1.45,
                meta * 0.4,
            ),
        ]
    } else {
        let stats = if name == crate::moonlight::SYSTEM {
            catalog.text("moonlight.description")
        } else if name.starts_with('@') {
            count("games.count", games)
        } else if data.is_tools(name) {
            count("tools.count", games)
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
        let maker = maker.join(" · ").to_uppercase();
        let mut blocks = Vec::new();
        if !maker.is_empty() {
            blocks.push((maker, muted(Face::Regular), meta * 1.5, 0.));
        }
        blocks.push((title_block.0, title_block.1, title_block.2, 0.));
        blocks.push((stats, muted(Face::Mono), meta * 16. / 12., meta / 3.));
        blocks
    };
    let width = rect.width();
    let mut laid = Vec::new();
    for (text, style, step, above) in blocks {
        let lines = wrap(&text, width, |s| measure(s, style.0, style.1))?;
        laid.push((lines, style, frame.px(step), frame.px(above)));
    }
    let mut labels = Vec::new();
    let mut place = |lines: Vec<String>, style, step: i32, y: i32| {
        for (n, line) in lines.into_iter().enumerate() {
            labels.push(PanelLabel {
                text: line,
                style,
                rect: Rect::new(rect.x(), y + n as i32 * step, width, step as u32),
            });
        }
    };
    if info.bottom {
        let mut bottom = rect.bottom();
        for (lines, style, step, above) in laid.into_iter().rev() {
            bottom -= step * lines.len() as i32;
            place(lines, style, step, bottom);
            bottom -= above;
        }
    } else {
        let mut top = rect.y();
        for (lines, style, step, above) in laid {
            top += above;
            let height = step * lines.len() as i32;
            place(lines, style, step, top);
            top += height;
        }
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_shows_only_known_manufacturer_and_year_without_icon_placeholders() {
        struct Metadata<'a>(&'a str, &'a str);
        impl DataSource for Metadata<'_> {
            fn systems(&self) -> &[archr_index::cache::SystemEntry] {
                &[]
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
            fn manufacturer(&self, _: &str) -> Option<(&str, &str)> {
                Some((self.0, self.1))
            }
        }
        let theme = Theme::builtin();
        let frame = Frame::new(640, 480);
        for (maker, year, expected) in [
            ("\u{f013} System", "System", ""),
            ("", "", ""),
            ("  ", "  ", ""),
            ("Nintendo", "", "NINTENDO"),
            ("", "1983", "1983"),
            (" Nintendo ", " 1983 ", "NINTENDO · 1983"),
            ("\u{f013} Nintendo", "1983", "NINTENDO · 1983"),
            ("Nintendo", "0000", "NINTENDO"),
            ("Nintendo", "Nintendo", "NINTENDO"),
            ("System", "1983", "1983"),
        ] {
            let info = crate::layout::View::lista(&theme).systems.info.unwrap();
            let labels = panel_labels(
                frame,
                &info,
                "music",
                &Metadata(maker, year),
                &I18n::default(),
                &theme,
                |text, _, _| Ok(text.chars().count() as u32 * 5),
            )
            .unwrap();
            let meta = labels
                .iter()
                .filter(|label| label.style.0 == Face::Regular)
                .map(|label| label.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(meta, expected, "maker={maker:?}, year={year:?}");
        }
    }

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
    fn watermark_shrinks_until_its_measured_width_fits() {
        let theme = Theme::builtin();
        let frame = Frame::new(640, 480);
        let rect = Rect::new(374, 50, 250, 374);
        // Rounded per-glyph advances make width non-linear in size.
        let advance =
            |text: &str, size: f32| text.chars().count() as u32 * (size * 0.703).ceil() as u32;
        for name in ["n64", "gba", "psx", "snes", "megadrive"] {
            let label = generated_label(frame, rect, name, &theme, |text, _, size| {
                Ok(advance(text, size))
            })
            .unwrap();
            assert!(
                advance(&label.text, label.style.1) <= label.rect.width(),
                "{name}"
            );
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
            let font = crate::text::production_font(&ttf, &fonts.join(format!("{face}.ttf")), size);
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
