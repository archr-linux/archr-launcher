//! The battery card (Bateria-critica): a notice over the whole screen, games
//! included. Geometry from the 640x480 design, text with the launcher's
//! fonts and catalogs.
use crate::proto::{Alert, Report};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use resvg::tiny_skia as sk;
use std::{collections::HashMap, path::Path, time::Duration};

const VEIL: [u8; 4] = [5, 8, 11, 115]; // rgba(5,8,11,.45)
const CARD: [u8; 4] = [17, 22, 28, 240]; // rgba(17,22,28,.94)
const DANGER: [u8; 3] = [0xF8, 0x51, 0x49];
const DANGER_EDGE: [u8; 3] = [0x5C, 0x23, 0x28];
const WARNING: [u8; 3] = [0xE3, 0xA0, 0x08];
const ACCENT: [u8; 3] = [0x1E, 0x88, 0xFF];
const SUCCESS: [u8; 3] = [0x3F, 0xB9, 0x50];
const EDGE: [u8; 3] = [0x1C, 0x23, 0x2C];
const TEXT: [u8; 3] = [0xE6, 0xED, 0xF3];
const BODY: [u8; 3] = [0xC5, 0xCF, 0xDA];
const MUTED: [u8; 3] = [0x9B, 0xA7, 0xB4];
const TRACK: [u8; 3] = [0x1C, 0x25, 0x30];

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
    regular: Option<FontVec>,
    semibold: Option<FontVec>,
    text: HashMap<String, String>,
}

impl Assets {
    /// `share` holds the launcher's themes/ and locales/; `config` is
    /// system.cfg, for system.language.
    pub fn load(share: &Path, config: &Path) -> Self {
        let font = |name: &str| {
            std::fs::read(share.join("themes/default/fonts").join(name))
                .ok()
                .and_then(|bytes| FontVec::try_from_vec(bytes).ok())
        };
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
        Self {
            regular: font("Inter-Regular.ttf"),
            semibold: font("Inter-SemiBold.ttf"),
            text,
        }
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
        let battery = match alert {
            Alert::Battery(battery) => battery,
            Alert::NoState => {
                return Card {
                    title: self.tr("osd.nostate_title", "No state saved", &[]),
                    body: self.tr(
                        "osd.nostate",
                        "This emulator does not save a state; use the game's menu.",
                        &[],
                    ),
                    hint: None,
                    bar: None,
                    critical: false,
                    icon: ALERT,
                    accent: WARNING,
                }
            }
            Alert::Report(step) => {
                let (title, body, icon, accent) = match step {
                    Report::Start => (
                        self.tr("osd.report_start_title", "Creating the report…", &[]),
                        self.tr("osd.report_start", "It takes about 20 seconds.", &[]),
                        LOADER,
                        ACCENT,
                    ),
                    Report::Done => (
                        self.tr("osd.report_done_title", "Report saved", &[]),
                        self.tr(
                            "osd.report_done",
                            "In roms/relatorios, visible on a PC.",
                            &[],
                        ),
                        CHECK,
                        SUCCESS,
                    ),
                    Report::Failed => (
                        self.tr("osd.report_failed_title", "No report", &[]),
                        self.tr("osd.report_failed", "The report could not be created.", &[]),
                        FAILED,
                        DANGER,
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
                    accent: DANGER,
                }
            }
            None => Card {
                title,
                body: self.tr("osd.battery_low", "Connect the charger soon.", &[]),
                hint: None,
                bar: None,
                critical: false,
                icon: BATTERY,
                accent: WARNING,
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

/// CSS pixels to the font's scale.
fn scale(font: &FontVec, px: f32) -> PxScale {
    font.pt_to_px_scale(px * 0.75).unwrap_or(PxScale::from(px))
}

fn width(font: &FontVec, px: f32, text: &str) -> f32 {
    let font = font.as_scaled(scale(font, px));
    let mut last = None;
    let mut w = 0.0;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(prev) = last {
            w += font.kern(prev, id);
        }
        w += font.h_advance(id);
        last = Some(id);
    }
    w
}

/// Words never split: lines of at most `max` pixels.
fn wrap(font: &FontVec, px: f32, text: &str, max: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if width(font, px, &format!("{line} {word}")) <= max => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

/// Draws `text` with its baseline at `y`.
fn text(
    out: &mut sk::Pixmap,
    font: &FontVec,
    px: f32,
    (x, y): (f32, f32),
    color: [u8; 3],
    text: &str,
) {
    let scaled = font.as_scaled(scale(font, px));
    let (w, h) = (out.width() as i32, out.height() as i32);
    let mut caret = x;
    let mut last = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(prev) = last {
            caret += scaled.kern(prev, id);
        }
        let glyph = id.with_scale_and_position(scale(font, px), ab_glyph::point(caret, y));
        caret += scaled.h_advance(id);
        last = Some(id);
        let Some(outline) = font.outline_glyph(glyph) else {
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

/// One frame of the card over a `width` x `height` surface.
pub fn render(
    card: &Card,
    assets: &Assets,
    (width, height): (u32, u32),
    opacity: f32,
) -> sk::Pixmap {
    let mut out = sk::Pixmap::new(width.max(1), height.max(1)).expect("surface size");
    let s = (width as f32 / 640.0).min(height as f32 / 480.0).max(0.5);
    if card.critical {
        out.fill(sk::Color::from_rgba8(VEIL[0], VEIL[1], VEIL[2], VEIL[3]));
    }
    let (Some(regular), Some(semibold)) = (&assets.regular, &assets.semibold) else {
        return out;
    };
    let pad = 18.0 * s;
    let card_w = 340.0 * s;
    let inner = card_w - 2.0 * pad;
    let body_px = 13.0 * s;
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
        (true, _) => DANGER_EDGE,
        _ => EDGE,
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
        &paint([CARD[0], CARD[1], CARD[2]], CARD[3]),
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
    let title_px = 17.0 * s;
    text(
        &mut out,
        semibold,
        title_px,
        (cx + circle + 12.0 * s, cy + circle / 2.0 + title_px * 0.36),
        TEXT,
        &card.title,
    );
    let mut line_y = cy + circle + gap;
    for line in &lines {
        text(
            &mut out,
            regular,
            body_px,
            (x + pad, line_y + body_px * 1.08),
            BODY,
            line,
        );
        line_y += body_line;
    }
    if let Some(done) = card.bar {
        line_y += gap;
        let bar = (x + pad, line_y, inner, 6.0 * s);
        rounded(&mut out, bar, 3.0 * s, &paint(TRACK, 255));
        let fill = (inner * done.clamp(0.0, 1.0)).max(6.0 * s);
        rounded(
            &mut out,
            (bar.0, bar.1, fill, bar.3),
            3.0 * s,
            &paint(DANGER, 255),
        );
        line_y += 6.0 * s;
    }
    if let Some(hint) = &card.hint {
        line_y += gap;
        let size = (14.0 * s).round() as u32;
        icon(
            &mut out,
            PLUG,
            MUTED,
            size,
            ((x + pad) as i32, (line_y + s) as i32),
        );
        let px = 12.0 * s;
        text(
            &mut out,
            regular,
            px,
            (x + pad + 20.0 * s, line_y + 12.0 * s),
            MUTED,
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
        assert_eq!((nostate.critical, nostate.icon), (false, ALERT));
        let report = |language, step| assets(language).card(Alert::Report(step));
        assert_eq!(report("pt_BR", Report::Start).title, "Gerando relatório…");
        assert_eq!(
            report("pt_BR", Report::Start).body,
            "Leva cerca de 20 segundos."
        );
        let done = report("pt_BR", Report::Done);
        assert_eq!(done.body, "Em roms/relatorios, visível no PC.");
        assert_eq!(
            (done.critical, done.icon, done.accent),
            (false, CHECK, SUCCESS)
        );
        assert_eq!(report("xx_YY", Report::Failed).accent, DANGER);
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
        assert_eq!(at(4, 4).alpha(), VEIL[3]);
        assert_eq!(at(320, 236).alpha(), 255);
        if let Some(dir) = std::env::var_os("ARCHR_OSD_CAPTURE_DIR") {
            for language in ["pt_BR", "ru"] {
                let a = assets(language);
                render(&a.card(Alert::NoState), &a, (640, 480), 1.0)
                    .save_png(Path::new(&dir).join(format!("sem-estado-{language}.png")))
                    .unwrap();
            }
            for language in ["pt_BR", "ru"] {
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
            for (name, language) in [("bateria-critica", "pt_BR"), ("bateria-critica-ru", "ru")] {
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
}
