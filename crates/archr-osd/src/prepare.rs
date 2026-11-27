//! The screen before a game (Progresso-antes-do-jogo): the game's name, the
//! steps it needs the first time (done, running, waiting), a bar with the
//! bytes, the speed and the time left, and B to cancel. Over the whole
//! output, opaque: the launcher is gone and the game not there yet.
use crate::card::{icon, paint, rounded, text, width, Assets, MIN_TEXT};
use crate::proto::{Prepare, Step};
use resvg::tiny_skia as sk;

const CHECK: &str = include_str!("../../../themes/default/icons/check.svg");

/// How long the screen stays after the last message: archr-prepare sends
/// one every half second while it works.
pub const HOLD: std::time::Duration = std::time::Duration::from_millis(3000);

fn label(assets: &Assets, step: &Step) -> String {
    match step {
        Step::Debian => assets.tr("osd.prepare_debian", "Compatibility mode (Debian)", &[]),
        Step::Runtime(name) => assets.tr(
            "osd.prepare_runtime",
            "Runtime {name}",
            &[("name", name.clone())],
        ),
        Step::Rtp(name) => assets.tr(
            "osd.prepare_rtp",
            "RPG Maker {name} files",
            &[("name", name.clone())],
        ),
        Step::Disc => assets.tr("osd.prepare_disc", "Game disc", &[]),
    }
}

/// "43,1" and "69,5 MB": `part` in the unit of `whole`, as the launcher
/// writes sizes (updates.kb/mb/gb, details.decimal).
fn sizes(assets: &Assets, part: u64, whole: u64) -> (String, String) {
    let (unit, key, english) = match whole {
        0..1_048_576 => (1024., "updates.kb", "{value} KB"),
        1_048_576..1_073_741_824 => (1_048_576., "updates.mb", "{value} MB"),
        _ => (1_073_741_824., "updates.gb", "{value} GB"),
    };
    let comma = assets.tr("details.decimal", ".", &[]);
    let number = |bytes: u64| format!("{:.1}", bytes as f64 / unit).replace('.', &comma);
    let whole = assets.tr(key, english, &[("value", number(whole))]);
    (number(part), whole)
}

/// "Downloading · 43,1 of 69,5 MB · 2,1 MB/s" and "~15 s left".
fn detail(assets: &Assets, prepare: &Prepare) -> (String, Option<String>) {
    let step = &prepare.steps[prepare.current];
    let mut line = if step.unpacks() {
        assets.tr("osd.prepare_unpacking", "Unpacking", &[])
    } else {
        assets.tr("osd.prepare_downloading", "Downloading", &[])
    };
    if prepare.total > 0 {
        let (done, total) = sizes(assets, prepare.done.min(prepare.total), prepare.total);
        let of = assets.tr(
            "updates.of",
            "{done} of {total}",
            &[("done", done), ("total", total)],
        );
        line = format!("{line} · {of}");
    } else if prepare.done > 0 {
        line = format!("{line} · {}", sizes(assets, prepare.done, prepare.done).1);
    }
    if prepare.per_second > 0 {
        let size = sizes(assets, prepare.per_second, prepare.per_second).1;
        let speed = assets.tr("updates.per_second", "{size}/s", &[("size", size)]);
        line = format!("{line} · {speed}");
    }
    let left = (prepare.total > prepare.done && prepare.per_second > 0).then(|| {
        let seconds = (prepare.total - prepare.done).div_ceil(prepare.per_second);
        let time = if seconds < 60 {
            assets.tr(
                "progress.seconds",
                "{count} s",
                &[("count", seconds.to_string())],
            )
        } else {
            assets.tr(
                "progress.minutes",
                "{count} min",
                &[("count", seconds.div_ceil(60).to_string())],
            )
        };
        assets.tr("progress.left", "~{time} left", &[("time", time)])
    });
    (line, left)
}

/// How far the running step is, 0 to 99, when its size is known.
fn percent(prepare: &Prepare) -> Option<u64> {
    (prepare.total > 0).then(|| (prepare.done.min(prepare.total) * 100 / prepare.total).min(99))
}

/// One frame over a `width` x `height` surface.
pub fn render(
    prepare: &Prepare,
    assets: &Assets,
    (width_px, height_px): (u32, u32),
    opacity: f32,
) -> sk::Pixmap {
    let (w, h) = (width_px.max(1), height_px.max(1));
    let mut out = sk::Pixmap::new(w, h).expect("surface size");
    let (fw, fh) = (w as f32, h as f32);
    let s = (fw / 640.0).min(fh / 480.0).max(0.5);
    let p = assets.palette;
    let [r, g, b, _] = p.veil;
    out.fill(sk::Color::from_rgba8(r, g, b, 255));
    let Some((regular, semibold)) = assets.fonts() else {
        return out;
    };
    let px = |size: f32| (size * s).max(MIN_TEXT);
    let centered = |out: &mut sk::Pixmap, font, size: f32, y: f32, color, line: &str| {
        let x = ((fw - width(font, size, line)) / 2.0).max(16.0 * s);
        text(out, font, size, (x, y), color, line);
    };
    // The game, then what this is.
    let title_px = px(20.0);
    let top = fh / 2.0 - 90.0 * s;
    centered(&mut out, semibold, title_px, top, p.text, &prepare.title);
    let subtitle = assets.tr(
        "osd.prepare_subtitle",
        "Getting ready to open · only the first time",
        &[],
    );
    centered(
        &mut out,
        regular,
        px(12.0),
        top + 22.0 * s,
        p.muted,
        &subtitle,
    );

    // The panel: the steps, the bar, the bytes.
    let (pad, row) = (18.0 * s, 28.0 * s);
    let panel_w = (448.0 * s).min(fw - 32.0 * s);
    let x = ((fw - panel_w) / 2.0).round();
    let y = (top + 48.0 * s).round();
    let panel_h = 16.0 * s
        + row * prepare.steps.len() as f32
        + 10.0 * s
        + 8.0 * s
        + 12.0 * s
        + 14.0 * s
        + 16.0 * s;
    rounded(
        &mut out,
        (x - s, y - s, panel_w + 2.0 * s, panel_h + 2.0 * s),
        21.0 * s,
        &paint(p.edge, 255),
    );
    rounded(
        &mut out,
        (x, y, panel_w, panel_h),
        20.0 * s,
        &paint([p.card[0], p.card[1], p.card[2]], 255),
    );
    let inner = panel_w - 2.0 * pad;
    let percent = percent(prepare);
    let body_px = px(13.0);
    let mut line_y = y + 16.0 * s;
    for (n, step) in prepare.steps.iter().enumerate() {
        let middle = line_y + row / 2.0;
        let dot = 18.0 * s;
        let (dx, dy) = (x + pad, middle - dot / 2.0);
        let ink = if n < prepare.current {
            // Done: a filled disc with a dark check (Progresso-antes-do-jogo).
            rounded(
                &mut out,
                (dx, dy, dot, dot),
                dot / 2.0,
                &paint(p.success, 255),
            );
            let mark = (11.0 * s).round();
            let [r, g, b, _] = p.veil;
            icon(
                &mut out,
                CHECK,
                [r, g, b],
                mark as u32,
                (
                    (dx + (dot - mark) / 2.0) as i32,
                    (dy + (dot - mark) / 2.0) as i32,
                ),
            );
            p.body
        } else {
            let ring = if n == prepare.current {
                p.accent
            } else {
                p.track
            };
            rounded(&mut out, (dx, dy, dot, dot), dot / 2.0, &paint(ring, 255));
            let hole = dot - 4.0 * s;
            rounded(
                &mut out,
                (dx + 2.0 * s, dy + 2.0 * s, hole, hole),
                hole / 2.0,
                &paint([p.card[0], p.card[1], p.card[2]], 255),
            );
            if n == prepare.current {
                let core = 8.0 * s;
                rounded(
                    &mut out,
                    (dx + (dot - core) / 2.0, dy + (dot - core) / 2.0, core, core),
                    core / 2.0,
                    &paint(p.accent, 255),
                );
                p.text
            } else {
                p.muted
            }
        };
        let face = if n == prepare.current {
            semibold
        } else {
            regular
        };
        text(
            &mut out,
            face,
            body_px,
            (dx + dot + 10.0 * s, middle + body_px * 0.36),
            ink,
            &label(assets, step),
        );
        if let (true, Some(percent)) = (n == prepare.current, percent) {
            let value = format!("{percent}%");
            let vx = x + panel_w - pad - width(semibold, body_px, &value);
            text(
                &mut out,
                semibold,
                body_px,
                (vx, middle + body_px * 0.36),
                p.text,
                &value,
            );
        }
        line_y += row;
    }
    line_y += 10.0 * s;
    let bar = (x + pad, line_y, inner, 8.0 * s);
    rounded(&mut out, bar, 4.0 * s, &paint(p.track, 255));
    if let Some(percent) = percent.filter(|p| *p > 0) {
        let fill = (inner * percent as f32 / 100.0).max(8.0 * s);
        rounded(
            &mut out,
            (bar.0, bar.1, fill, bar.3),
            4.0 * s,
            &paint(p.accent, 255),
        );
    }
    line_y += 8.0 * s + 12.0 * s;
    let (line, left) = detail(assets, prepare);
    let small = px(12.0);
    text(
        &mut out,
        regular,
        small,
        (x + pad, line_y + small * 0.8),
        p.muted,
        &line,
    );
    if let Some(left) = left {
        let lx = x + panel_w - pad - width(regular, small, &left);
        text(
            &mut out,
            regular,
            small,
            (lx, line_y + small * 0.8),
            p.muted,
            &left,
        );
    }

    // B to go back, in the corner as the launcher shows its hints.
    if prepare.cancel {
        let hint = assets.tr("osd.prepare_cancel", "Cancel and go back", &[]);
        let key = 18.0 * s;
        let pill_h = 30.0 * s;
        let pill_w = 12.0 * s + key + 6.0 * s + width(regular, small, &hint) + 12.0 * s;
        let (hx, hy) = (fw - 24.0 * s - pill_w, fh - 16.0 * s - pill_h);
        rounded(
            &mut out,
            (hx, hy, pill_w, pill_h),
            pill_h / 2.0,
            &paint([p.card[0], p.card[1], p.card[2]], 255),
        );
        let (kx, ky) = (hx + 12.0 * s, hy + (pill_h - key) / 2.0);
        rounded(
            &mut out,
            (kx, ky, key, key),
            key / 2.0,
            &paint(p.track, 255),
        );
        let letter = px(10.0);
        text(
            &mut out,
            semibold,
            letter,
            (
                kx + (key - width(semibold, letter, "B")) / 2.0,
                ky + key / 2.0 + letter * 0.36,
            ),
            p.text,
            "B",
        );
        text(
            &mut out,
            regular,
            small,
            (kx + key + 6.0 * s, hy + pill_h / 2.0 + small * 0.36),
            p.muted,
            &hint,
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
    use std::path::Path;

    fn assets(language: &str) -> Assets {
        let dir = std::env::temp_dir().join(format!(
            "archr-osd-prepare-{}-{language}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("system.cfg");
        std::fs::write(&config, format!("system.language={language}\n")).unwrap();
        Assets::load(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
            &config,
        )
    }
    fn isaac() -> Prepare {
        Prepare {
            title: "The Binding of Isaac: Rebirth".into(),
            steps: vec![Step::Runtime("mono-6.12".into()), Step::Debian],
            current: 1,
            done: 45_193_626,
            total: 72_839_168,
            per_second: 2_202_009,
            cancel: true,
        }
    }

    #[test]
    fn the_detail_line_reads_like_the_launcher_s() {
        let (line, left) = detail(&assets("pt_BR"), &isaac());
        assert_eq!(line, "Baixando · 43,1 de 69,5 MB · 2,1 MB/s");
        assert_eq!(left.as_deref(), Some("faltam ~13 s"));
        let unknown = Prepare {
            total: 0,
            per_second: 0,
            done: 3 * 1_048_576,
            ..isaac()
        };
        assert_eq!(detail(&assets("en"), &unknown).0, "Downloading · 3.0 MB");
        assert_eq!(percent(&isaac()), Some(62));
    }

    #[test]
    fn it_draws_opaque_over_any_screen_size() {
        for size in [(640, 480), (1024, 768), (720, 720)] {
            let frame = render(&isaac(), &assets("pt_BR"), size, 1.0);
            assert_eq!((frame.width(), frame.height()), size);
            // ARCHR_OSD_SNAPSHOTS=dir keeps each frame, to compare with the
            // design.
            if let Some(dir) = std::env::var_os("ARCHR_OSD_SNAPSHOTS") {
                let name = format!("prepare-{}x{}.png", size.0, size.1);
                frame.save_png(Path::new(&dir).join(name)).unwrap();
            }
            assert!(frame.data().chunks_exact(4).all(|px| px[3] == 255));
        }
    }
}
