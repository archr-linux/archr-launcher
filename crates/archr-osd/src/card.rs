//! The battery card (Bateria-critica): a notice over the whole screen, games
//! included. Geometry from the 640x480 design, text with the launcher's
//! fonts and catalogs, in the colours of the active theme (look.rs).
use crate::look::{Look, Palette};
use crate::proto::{Alert, Disc, Report};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use resvg::tiny_skia as sk;
use std::{collections::HashMap, path::Path, time::Duration};

/// How long a card stays after its last message: the critical one is sent
/// every second while it counts.
pub const HOLD: Duration = Duration::from_millis(1500);
pub const HOLD_LOW: Duration = Duration::from_millis(4000);

const BATTERY: &str = include_str!("../../../themes/default/icons/battery-warning.svg");
const ALERT: &str = include_str!("../../../themes/default/icons/triangle-alert.svg");
const PLUG: &str = include_str!("../../../themes/default/icons/plug-zap.svg");
const LOADER: &str = include_str!("../../../themes/default/icons/loader-circle.svg");
const CHECK: &str = include_str!("../../../themes/default/icons/circle-check.svg");
const FAILED: &str = include_str!("../../../themes/default/icons/circle-x.svg");

/// The launcher's installed fonts and the language's catalog.
pub struct Assets {
    regular: Option<FontArc>,
    semibold: Option<FontArc>,
    text: HashMap<String, String>,
    share: std::path::PathBuf,
    /// system.cfg: the language at start, the exit shortcut at each card.
    config: std::path::PathBuf,
    /// The theme's faces the fonts were loaded for.
    faces: (Option<std::path::PathBuf>, Option<std::path::PathBuf>),
    pub palette: Palette,
}

impl Assets {
    /// `share` holds the launcher's themes/ and locales/; `config` is
    /// system.cfg, for system.language.
    pub fn load(share: &Path, config: &Path) -> Self {
        let language = std::fs::read_to_string(config)
            .unwrap_or_default()
            .lines()
            .find_map(|l| l.strip_prefix("system.language=").map(str::to_owned))
            .unwrap_or_default();
        let base = language.split('_').next().unwrap_or("en").to_owned();
        let text = [language, base, "en".into()]
            .iter()
            .find_map(|name| {
                std::fs::read_to_string(share.join("locales").join(format!("{name}.toml"))).ok()
            })
            .and_then(|source| toml::from_str::<toml::Table>(&source).ok())
            .map(|table| {
                table
                    .into_iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k, v.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let mut assets = Self {
            regular: None,
            semibold: None,
            text,
            share: share.to_owned(),
            config: config.to_owned(),
            faces: (None, None),
            palette: Palette::DEFAULT,
        };
        assets.load_fonts();
        assets
    }
    /// The theme's colours and faces; a face that does not load is the
    /// default theme's.
    pub fn restyle(&mut self, look: &Look) {
        self.palette = look.palette;
        let faces = (look.font.clone(), look.font_semibold.clone());
        if faces != self.faces || self.regular.is_none() {
            self.faces = faces;
            self.load_fonts();
        }
    }
    fn load_fonts(&mut self) {
        let read = |path: &Path| {
            std::fs::read(path)
                .ok()
                .and_then(|bytes| FontArc::try_from_vec(bytes).ok())
        };
        let font = |own: &Option<std::path::PathBuf>, name: &str| {
            own.as_deref()
                .and_then(read)
                .or_else(|| read(&self.share.join("themes/default/fonts").join(name)))
        };
        // A theme with only a regular face uses it for the titles too, as
        // the launcher does.
        let semibold = match &self.faces {
            (Some(_), None) => &self.faces.0,
            (_, semibold) => semibold,
        };
        let (regular, semibold) = (
            font(&self.faces.0, "Inter-Regular.ttf"),
            font(semibold, "Inter-SemiBold.ttf"),
        );
        (self.regular, self.semibold) = (regular, semibold);
    }
    fn tr(&self, key: &str, english: &str, values: &[(&str, String)]) -> String {
        let mut text = self
            .text
            .get(key)
            .cloned()
            .unwrap_or_else(|| english.into());
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
    /// The card's texts for a message.
    pub fn card(&self, alert: Alert) -> Card {
        let Palette {
            danger,
            warning,
            accent,
            success,
            ..
        } = self.palette;
        let battery = match alert {
            Alert::Battery(battery) => battery,
            Alert::NoState => {
                // How to leave the game, read now: the player may have
                // changed the shortcut since the overlay started.
                let config = std::fs::read_to_string(&self.config).unwrap_or_default();
                let body = match exit_keys(&config) {
                    Some(keys) => self.tr(
                        "osd.nostate_exit",
                        "This game saves no state. Hold {keys} to quit; save from the game's own menu.",
                        &[("keys", keys)],
                    ),
                    None => self.tr(
                        "osd.nostate",
                        "This emulator does not save a state; use the game's menu.",
                        &[],
                    ),
                };
                return Card {
                    title: self.tr("osd.nostate_title", "No state saved", &[]),
                    body,
                    hint: None,
                    bar: None,
                    critical: false,
                    icon: ALERT,
                    accent: warning,
                };
            }
            Alert::Disc(step) => {
                let (title, body, icon, accent) = match step {
                    Disc::Start => (
                        self.tr("osd.disc_start_title", "Preparing the disc…", &[]),
                        self.tr(
                            "osd.disc_start",
                            "Extracting it from the archive, only the first time.",
                            &[],
                        ),
                        LOADER,
                        accent,
                    ),
                    Disc::NoSpace => (
                        self.tr("osd.disc_nospace_title", "No room for the disc", &[]),
                        self.tr(
                            "osd.disc_nospace",
                            "Free space on the card, or extract the archive on a PC.",
                            &[],
                        ),
                        FAILED,
                        danger,
                    ),
                };
                return Card {
                    title,
                    body,
                    hint: None,
                    bar: None,
                    critical: false,
                    icon,
                    accent,
                };
            }
            Alert::Report(step) => {
                let (title, body, icon, accent) = match step {
                    Report::Start => (
                        self.tr("osd.report_start_title", "Creating the report…", &[]),
                        self.tr("osd.report_start", "It takes about 20 seconds.", &[]),
                        LOADER,
                        accent,
                    ),
                    Report::Done => (
                        self.tr("osd.report_done_title", "Report saved", &[]),
                        self.tr(
                            "osd.report_done",
                            "In roms/relatorios, visible on a PC.",
                            &[],
                        ),
                        CHECK,
                        success,
                    ),
                    Report::Failed => (
                        self.tr("osd.report_failed_title", "No report", &[]),
                        self.tr("osd.report_failed", "The report could not be created.", &[]),
                        FAILED,
                        danger,
                    ),
                };
                return Card {
                    title,
                    body,
                    hint: None,
                    bar: None,
                    critical: false,
                    icon,
                    accent,
                };
            }
        };
        let level = [("level", battery.level.to_string())];
        let title = self.tr("osd.battery_title", "Battery at {level}%", &level);
        match battery.left {
            Some((left, total)) => {
                let count = [("count", left.to_string())];
                let body = if battery.game {
                    self.tr(
                        "osd.battery_game",
                        "Saving the game and powering off in {count} s. It comes back from here at power on.",
                        &count,
                    )
                } else {
                    self.tr("osd.battery_menu", "Powering off in {count} s.", &count)
                };
                Card {
                    title,
                    body,
                    hint: Some(self.tr("osd.battery_cancel", "Connect the charger to cancel", &[])),
                    bar: Some(1.0 - f32::from(left.saturating_sub(1)) / f32::from(total.max(1))),
                    critical: true,
                    icon: BATTERY,
                    accent: danger,
                }
            }
            None => Card {
                title,
                body: self.tr("osd.battery_low", "Connect the charger soon.", &[]),
                hint: None,
                bar: None,
                critical: false,
                icon: BATTERY,
                accent: warning,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub title: String,
    pub body: String,
    pub hint: Option<String>,
    /// How far the countdown went, 0 to 1.
    pub bar: Option<f32>,
    /// The critical card: a veil over the screen and red edges.
    pub critical: bool,
    /// Lucide SVG in the round badge.
    pub icon: &'static str,
    /// The badge's colour.
    pub accent: [u8; 3],
}

fn paint(color: [u8; 3], alpha: u8) -> sk::Paint<'static> {
    let mut paint = sk::Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.set_color_rgba8(color[0], color[1], color[2], alpha);
    paint
}

fn rounded(out: &mut sk::Pixmap, (x, y, w, h): (f32, f32, f32, f32), r: f32, p: &sk::Paint) {
    let path = crate::pill::rounded_rect((x, y, w, h), r);
    out.fill_path(
        &path,
        p,
        sk::FillRule::Winding,
        sk::Transform::identity(),
        None,
    );
}

/// Same lazy interface subset as the SDL renderer; never replace a covered
/// Latin/Cyrillic glyph. FontArc keeps both the primary and fallback type the
/// same without copying the embedded bytes.
fn glyph_font<'a>(primary: &'a FontArc, ch: char) -> &'a FontArc {
    if primary.glyph_id(ch).0 != 0 {
        return primary;
    }
    static CJK: std::sync::OnceLock<FontArc> = std::sync::OnceLock::new();
    let fallback = CJK.get_or_init(|| {
        FontArc::try_from_slice(include_bytes!("../../../fonts/cjk/ArchR-UI-SC.otf"))
            .expect("valid embedded CJK subset")
    });
    if fallback.glyph_id(ch).0 != 0 {
        fallback
    } else {
        primary
    }
}

/// CSS pixels to the font's scale.
fn scale(font: &FontArc, px: f32) -> PxScale {
    font.pt_to_px_scale(px * 0.75).unwrap_or(PxScale::from(px))
}

fn width(font: &FontArc, px: f32, text: &str) -> f32 {
    let mut last = None;
    let mut w = 0.0;
    for c in text.chars() {
        let face = glyph_font(font, c);
        let scaled = face.as_scaled(scale(face, px));
        let id = scaled.glyph_id(c);
        if let Some((previous_face, previous_id)) = last {
            if std::ptr::eq(previous_face, face) {
                w += scaled.kern(previous_id, id);
            }
        }
        w += scaled.h_advance(id);
        last = Some((face, id));
    }
    w
}

/// Wrap words at spaces and oversized words at Unicode character boundaries
/// (Chinese has no word spaces). Every line stays within the card.
fn wrap(font: &FontArc, px: f32, text: &str, max: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        if let Some(line) = lines.last_mut() {
            if width(font, px, &format!("{line} {word}")) <= max {
                line.push(' ');
                line.push_str(word);
                continue;
            }
        }
        if width(font, px, word) <= max {
            lines.push(word.to_owned());
            continue;
        }
        let mut line = String::new();
        for ch in word.chars() {
            if !line.is_empty() && width(font, px, &format!("{line}{ch}")) > max {
                lines.push(std::mem::take(&mut line));
            }
            line.push(ch);
        }
        lines.push(line);
    }
    lines
}

/// Draws `text` with its baseline at `y`.
fn text(
    out: &mut sk::Pixmap,
    font: &FontArc,
    px: f32,
    (x, y): (f32, f32),
    color: [u8; 3],
    text: &str,
) {
    let (w, h) = (out.width() as i32, out.height() as i32);
    let mut caret = x;
    let mut last = None;
    for c in text.chars() {
        let face = glyph_font(font, c);
        let scaled = face.as_scaled(scale(face, px));
        let id = scaled.glyph_id(c);
        if let Some((previous_face, previous_id)) = last {
            if std::ptr::eq(previous_face, face) {
                caret += scaled.kern(previous_id, id);
            }
        }
        let glyph = id.with_scale_and_position(scale(face, px), ab_glyph::point(caret, y));
        caret += scaled.h_advance(id);
        last = Some((face, id));
        let Some(outline) = face.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outline.px_bounds();
        let data = out.data_mut();
        outline.draw(|gx, gy, coverage| {
            let (px, py) = (
                bounds.min.x as i32 + gx as i32,
                bounds.min.y as i32 + gy as i32,
            );
            if px < 0 || py < 0 || px >= w || py >= h || coverage <= 0.0 {
                return;
            }
            let at = ((py * w + px) * 4) as usize;
            let a = coverage.min(1.0);
            for (n, channel) in color.iter().enumerate() {
                let dst = f32::from(data[at + n]);
                data[at + n] = (f32::from(*channel) * a + dst * (1.0 - a)).round() as u8;
            }
            let dst = f32::from(data[at + 3]);
            data[at + 3] = (255.0 * a + dst * (1.0 - a)).round() as u8;
        });
    }
}

fn icon(out: &mut sk::Pixmap, svg: &str, color: [u8; 3], size: u32, (x, y): (i32, i32)) {
    let hex = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
    let pixmap = crate::pill::rasterize(svg, &hex, size);
    out.draw_pixmap(
        x,
        y,
        pixmap.as_ref(),
        &sk::PixmapPaint::default(),
        sk::Transform::identity(),
        None,
    );
}

/// No text under this many pixels, as in the launcher: on a small screen
/// the text keeps it while the card shrinks around it.
const MIN_TEXT: f32 = 10.0;

/// One frame of the card over a `width` x `height` surface.
pub fn render(
    card: &Card,
    assets: &Assets,
    (width, height): (u32, u32),
    opacity: f32,
) -> sk::Pixmap {
    let mut out = sk::Pixmap::new(width.max(1), height.max(1)).expect("surface size");
    let s = (width as f32 / 640.0).min(height as f32 / 480.0).max(0.5);
    let p = assets.palette;
    if card.critical {
        let [r, g, b, a] = p.veil;
        out.fill(sk::Color::from_rgba8(r, g, b, a));
    }
    let (Some(regular), Some(semibold)) = (&assets.regular, &assets.semibold) else {
        return out;
    };
    let pad = 18.0 * s;
    let card_w = 340.0 * s;
    let inner = card_w - 2.0 * pad;
    let body_px = (13.0 * s).max(MIN_TEXT);
    let body_line = body_px * 1.45;
    let lines = wrap(regular, body_px, &card.body, inner);
    let gap = 10.0 * s;
    let mut card_h = pad + 36.0 * s + gap + lines.len() as f32 * body_line + 16.0 * s;
    if card.bar.is_some() {
        card_h += gap + 6.0 * s;
    }
    if card.hint.is_some() {
        card_h += gap + 16.0 * s;
    }
    let x = ((width as f32 - card_w) / 2.0).round();
    let y = ((height as f32 - card_h) / 2.0).round();
    let edge = match (card.critical, card.bar) {
        (true, _) => p.danger_edge,
        _ => p.edge,
    };
    let accent = card.accent;
    rounded(
        &mut out,
        (x - s, y - s, card_w + 2.0 * s, card_h + 2.0 * s),
        17.0 * s,
        &paint(edge, 255),
    );
    rounded(
        &mut out,
        (x, y, card_w, card_h),
        16.0 * s,
        &paint([p.card[0], p.card[1], p.card[2]], p.card[3]),
    );

    // The icon in its tinted circle, the title beside it.
    let circle = 36.0 * s;
    let (cx, cy) = (x + pad, y + pad);
    rounded(
        &mut out,
        (cx, cy, circle, circle),
        circle / 2.0,
        &paint(accent, 36),
    );
    let glyph = (18.0 * s).round() as u32;
    let offset = ((circle - glyph as f32) / 2.0).round();
    icon(
        &mut out,
        card.icon,
        accent,
        glyph,
        ((cx + offset) as i32, (cy + offset) as i32),
    );
    let title_px = (17.0 * s).max(MIN_TEXT);
    text(
        &mut out,
        semibold,
        title_px,
        (cx + circle + 12.0 * s, cy + circle / 2.0 + title_px * 0.36),
        p.text,
        &card.title,
    );
    let mut line_y = cy + circle + gap;
    for line in &lines {
        text(
            &mut out,
            regular,
            body_px,
            (x + pad, line_y + body_px * 1.08),
            p.body,
            line,
        );
        line_y += body_line;
    }
    if let Some(done) = card.bar {
        line_y += gap;
        let bar = (x + pad, line_y, inner, 6.0 * s);
        rounded(&mut out, bar, 3.0 * s, &paint(p.track, 255));
        let fill = (inner * done.clamp(0.0, 1.0)).max(6.0 * s);
        rounded(
            &mut out,
            (bar.0, bar.1, fill, bar.3),
            3.0 * s,
            &paint(p.danger, 255),
        );
        line_y += 6.0 * s;
    }
    if let Some(hint) = &card.hint {
        line_y += gap;
        let size = (14.0 * s).round() as u32;
        icon(
            &mut out,
            PLUG,
            p.muted,
            size,
            ((x + pad) as i32, (line_y + s) as i32),
        );
        let px = (12.0 * s).max(MIN_TEXT);
        text(
            &mut out,
            regular,
            px,
            (x + pad + 20.0 * s, line_y + 12.0 * s),
            p.muted,
            hint,
        );
    }
    if opacity < 1.0 {
        let a = (opacity.clamp(0.0, 1.0) * 256.0) as u16;
        for byte in out.data_mut() {
            *byte = ((*byte as u16 * a) >> 8) as u8;
        }
    }
    out
}

/// The exit shortcut as input_sense reads system.cfg: the hotkey modifier
/// (key.hotkey.a, L1 by default) and the pair of hotkey.exit (or the older
/// key.hotkey.b/.c), as the pad names them; `None` when it is off.
fn exit_keys(config: &str) -> Option<String> {
    let get = |key: &str| {
        config
            .lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('=').map(str::trim))
            .filter(|v| !v.is_empty())
    };
    let word = |v: &str| {
        !v.is_empty()
            && v.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    };
    if get("hotkey.exit.enabled") == Some("0") {
        return None;
    }
    let one = |key, default: &'static str| get(key).filter(|v| word(v)).unwrap_or(default);
    let pair: Vec<&str> = match get("hotkey.exit").map(|v| v.split('+').collect::<Vec<_>>()) {
        Some(parts) if parts.len() == 2 && parts.iter().all(|p| word(p)) => parts,
        _ => vec![
            one("key.hotkey.b", "BTN_SELECT"),
            one("key.hotkey.c", "BTN_START"),
        ],
    };
    let keys = [one("key.hotkey.a", "BTN_TL"), pair[0], pair[1]];
    Some(keys.map(pad_name).join(" + "))
}

/// The launcher's name for an evdev key (archr-ui's pad table), or the key
/// without "BTN_".
fn pad_name(key: &str) -> String {
    const NAMES: [(&str, &str); 17] = [
        ("BTN_EAST", "A"),
        ("BTN_SOUTH", "B"),
        ("BTN_NORTH", "X"),
        ("BTN_WEST", "Y"),
        ("BTN_TL", "L1"),
        ("BTN_TR", "R1"),
        ("BTN_TL2", "L2"),
        ("BTN_TR2", "R2"),
        ("BTN_SELECT", "SELECT"),
        ("BTN_START", "START"),
        ("BTN_THUMBL", "L3"),
        ("BTN_THUMBR", "R3"),
        ("BTN_MODE", "FN"),
        ("BTN_DPAD_UP", "↑"),
        ("BTN_DPAD_DOWN", "↓"),
        ("BTN_DPAD_LEFT", "←"),
        ("BTN_DPAD_RIGHT", "→"),
    ];
    NAMES.iter().find(|(k, _)| *k == key).map_or_else(
        || key.trim_start_matches("BTN_").to_owned(),
        |(_, n)| (*n).to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Battery;

    fn assets(language: &str) -> Assets {
        // One folder per call: the tests run at once.
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = tempfile_dir().join(n.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("system.cfg");
        std::fs::write(&config, format!("system.language={language}\n")).unwrap();
        Assets::load(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
            &config,
        )
    }
    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("archr-osd-card-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn chinese_notifications_have_real_glyphs_and_wrap_inside_the_card() {
        let a = assets("zh_CN");
        let font = a.regular.as_ref().unwrap();
        let mut first = sk::Pixmap::new(200, 40).unwrap();
        let mut second = first.clone();
        text(&mut first, font, 17., (0., 25.), [255; 3], "设置");
        text(&mut second, font, 17., (0., 25.), [255; 3], "语言");
        assert_ne!(
            first.data(),
            second.data(),
            "different Chinese text must not be identical boxes"
        );
        let source = "此模拟器不支持即时存档，请使用游戏内菜单保存。";
        let lines = wrap(font, 14., source, 100.);
        assert_eq!(lines.concat(), source);
        assert!(lines.iter().all(|line| width(font, 14., line) <= 100.));
    }

    #[test]
    fn notification_catalogs_have_glyphs_and_keep_covered_theme_characters() {
        for language in ["de_DE", "zh_CN"] {
            let a = assets(language);
            for font in [a.regular.as_ref().unwrap(), a.semibold.as_ref().unwrap()] {
                for ch in "Größe ÄÖÜ · Русский · AV RetroArch".chars() {
                    assert!(std::ptr::eq(glyph_font(font, ch), font));
                }
                let mut count = 0;
                for (key, value) in &a.text {
                    if !key.starts_with("osd.") {
                        continue;
                    }
                    count += 1;
                    for ch in value.chars().filter(|ch| !ch.is_whitespace()) {
                        let face = glyph_font(font, ch);
                        let id = face.glyph_id(ch);
                        assert_ne!(id.0, 0, "{language} {key}: {ch}");
                        assert!(
                            face.outline_glyph(id.with_scale(scale(face, 17.)))
                                .is_some(),
                            "{language} {key}: no outline for {ch}"
                        );
                    }
                }
                assert!(count > 0, "notification catalog was loaded");
            }
        }
    }

    #[test]
    fn texts_follow_the_message_and_the_language() {
        let a = assets("pt_BR");
        let critical = a.card(Alert::Battery(Battery {
            level: 3,
            left: Some((8, 8)),
            game: true,
        }));
        assert_eq!(critical.title, "Bateria em 3%");
        assert!(critical
            .body
            .starts_with("Salvando o jogo e desligando em 8 s."));
        assert_eq!(critical.bar, Some(0.125));
        let low = a.card(Alert::Battery(Battery {
            level: 10,
            left: None,
            game: false,
        }));
        assert_eq!((low.critical, low.bar, low.hint), (false, None, None));
        let en = assets("xx_YY").card(Alert::Battery(Battery {
            level: 3,
            left: Some((1, 8)),
            game: false,
        }));
        assert_eq!(en.body, "Powering off in 1 s.");
        assert_eq!(en.bar, Some(1.0));
        let nostate = a.card(Alert::NoState);
        assert_eq!(nostate.title, "Sem salvar estado");
        assert_eq!(
            nostate.body,
            "Este jogo não salva estado. Para sair, segure L1 + SELECT + START; para salvar, use o menu do próprio jogo."
        );
        assert_eq!((nostate.critical, nostate.icon), (false, ALERT));
        let report = |language, step| assets(language).card(Alert::Report(step));
        assert_eq!(report("pt_BR", Report::Start).title, "Gerando relatório…");
        assert_eq!(
            report("pt_BR", Report::Start).body,
            "Leva cerca de 20 segundos."
        );
        let disc = |step| assets("pt_BR").card(Alert::Disc(step));
        assert_eq!(disc(Disc::Start).title, "Preparando o disco…");
        assert_eq!(
            (disc(Disc::Start).icon, disc(Disc::Start).accent),
            (LOADER, Palette::DEFAULT.accent)
        );
        assert_eq!(disc(Disc::NoSpace).title, "Sem espaço para o disco");
        assert_eq!(disc(Disc::NoSpace).accent, Palette::DEFAULT.danger);
        let done = report("pt_BR", Report::Done);
        assert_eq!(done.body, "Em roms/relatorios, visível no PC.");
        assert_eq!(
            (done.critical, done.icon, done.accent),
            (false, CHECK, Palette::DEFAULT.success)
        );
        assert_eq!(
            report("xx_YY", Report::Failed).accent,
            Palette::DEFAULT.danger
        );
    }

    #[test]
    fn the_card_takes_the_themes_colours() {
        let mut a = assets("pt_BR");
        let low = Alert::Battery(Battery {
            level: 10,
            left: None,
            game: false,
        });
        assert_eq!(a.card(low).accent, Palette::DEFAULT.warning);
        a.restyle(&crate::look::Look::parse(
            "[colors]\nwarning = \"#FFAA00\"\nsurface = \"#202020\"\n",
        ));
        let card = a.card(low);
        assert_eq!(card.accent, [0xFF, 0xAA, 0]);
        let frame = render(&card, &a, (640, 480), 1.0);
        let c = frame.pixel(320, 250).unwrap().demultiply();
        // Premultiplied storage: one step of rounding.
        assert!(
            [c.red(), c.green(), c.blue()]
                .iter()
                .all(|v| v.abs_diff(0x20) <= 1),
            "{c:?}"
        );
        // A face that does not load is the default one: the text still shows.
        a.restyle(&crate::look::Look::parse("font = \"/nonexistent.ttf\"\n"));
        assert!(a.regular.is_some() && a.semibold.is_some());
        assert_eq!(a.palette, Palette::DEFAULT);
    }

    #[test]
    fn the_card_is_drawn_centered_with_its_veil() {
        let a = assets("pt_BR");
        let card = a.card(Alert::Battery(Battery {
            level: 3,
            left: Some((5, 8)),
            game: true,
        }));
        let frame = render(&card, &a, (640, 480), 1.0);
        let at = |x: u32, y: u32| frame.pixel(x, y).unwrap();
        // The veil in a corner, the card's own colour in its middle.
        assert_eq!(at(4, 4).alpha(), Palette::DEFAULT.veil[3]);
        assert_eq!(at(320, 236).alpha(), 255);
        if let Some(dir) = std::env::var_os("ARCHR_OSD_CAPTURE_DIR") {
            for language in ["pt_BR", "ru", "de", "zh_CN"] {
                let a = assets(language);
                render(&a.card(Alert::NoState), &a, (640, 480), 1.0)
                    .save_png(Path::new(&dir).join(format!("sem-estado-{language}.png")))
                    .unwrap();
            }
            for language in ["pt_BR", "ru", "de", "zh_CN"] {
                let a = assets(language);
                for (name, step) in [
                    ("relatorio-gerando", Report::Start),
                    ("relatorio-salvo", Report::Done),
                    ("relatorio-falhou", Report::Failed),
                ] {
                    render(&a.card(Alert::Report(step)), &a, (640, 480), 1.0)
                        .save_png(Path::new(&dir).join(format!("{name}-{language}.png")))
                        .unwrap();
                }
            }
            for (name, language) in [
                ("bateria-critica", "pt_BR"),
                ("bateria-critica-ru", "ru"),
                ("bateria-critica-de", "de_DE"),
                ("bateria-critica-zh_CN", "zh_CN"),
            ] {
                let a = assets(language);
                let card = a.card(Alert::Battery(Battery {
                    level: 3,
                    left: Some((8, 8)),
                    game: true,
                }));
                render(&card, &a, (640, 480), 1.0)
                    .save_png(Path::new(&dir).join(format!("{name}.png")))
                    .unwrap();
                // The same card on the square and the 3:2 screens.
                for (w, h) in [(720, 720), (480, 320)] {
                    render(&card, &a, (w, h), 1.0)
                        .save_png(Path::new(&dir).join(format!("{name}-{w}x{h}.png")))
                        .unwrap();
                }
            }
        }
    }

    #[test]
    fn the_no_state_card_names_the_exit_shortcut_as_input_sense_reads_it() {
        assert_eq!(exit_keys("").as_deref(), Some("L1 + SELECT + START"));
        assert_eq!(
            exit_keys("key.hotkey.a=BTN_MODE\nhotkey.exit=BTN_TR+BTN_START\n").as_deref(),
            Some("FN + R1 + START")
        );
        // A malformed pair falls back to the older keys, as input_sense does.
        assert_eq!(
            exit_keys("hotkey.exit=select+start\nkey.hotkey.b=BTN_TL2\n").as_deref(),
            Some("L1 + L2 + START")
        );
        assert_eq!(exit_keys("hotkey.exit.enabled=0\n"), None);
        assert_eq!(
            exit_keys("hotkey.exit.enabledx=0\n").as_deref(),
            Some("L1 + SELECT + START")
        );
    }
}
