//! `--roms-migrate-screen`: the screen of the maintenance boot that moves
//! the ROMs to the exFAT partition (archr-roms-migrate.target). No
//! compositor: SDL draws on KMSDRM. It only shows what archr-roms-migrate
//! writes; the migration never waits for it, and when it fails the
//! migration goes on, on the text console.
//!
//! STORAGE is unmounted while it shrinks, and a file kept open there would
//! stop that: system.cfg is read once at start, everything else comes from
//! the packaged default theme.
use archr_config::SystemCfg;
use archr_launcher::paths::Paths;
use archr_ui::{
    i18n::I18n,
    render::{MoveStage, RomsMove, Ui},
    startup::StartupTrace,
    Theme, DEFAULT_THEME_DIR,
};
use sdl2::event::Event;
use std::{path::Path, sync::mpsc, time::Duration};

/// archr-roms-migrate's progress file.
pub const PROGRESS: &str = "/run/archr/roms-migrate/progress";
/// How often the file is read, and the frame time (the dots move). Mesa
/// draws in software here (no GPU module in this boot): 10 frames a second
/// keep the dots smooth; on an R36S that is about half of one core.
const READ_EVERY: Duration = Duration::from_millis(500);
const FRAME_MS: u32 = 100;
/// A capture (ARCHR_LAUNCHER_SCREENSHOT) is taken this far in, once the
/// progress and the battery have been read.
const CAPTURE_AFTER: Duration = Duration::from_millis(1500);

/// The progress file: STAGE, PROGRESS=n/total, ENTRY, BYTES=done/total, LEFT.
pub fn parse(text: &str) -> RomsMove {
    fn pair<T: std::str::FromStr>(value: &str) -> Option<(T, T)> {
        let (a, b) = value.split_once('/')?;
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    }
    let mut view = RomsMove::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "STAGE" => {
                view.stage = match value {
                    "copying" => MoveStage::Copying,
                    "verifying" => MoveStage::Verifying,
                    _ => MoveStage::Preparing,
                }
            }
            "PROGRESS" => {
                if let Some((n, all)) = pair(value) {
                    (view.entry.0, view.entry.1) = (n, all);
                }
            }
            "ENTRY" => view.entry.2 = value.to_owned(),
            "BYTES" => view.bytes = pair(value).unwrap_or_default(),
            "LEFT" => view.left = value.parse().ok(),
            _ => {}
        }
    }
    view
}

/// Reads the progress file off the drawing thread; sends what changed.
fn watch(file: &Path) -> mpsc::Receiver<RomsMove> {
    let (send, receive) = mpsc::channel();
    let file = file.to_owned();
    let _ = std::thread::Builder::new()
        .name("roms-move".into())
        .spawn(move || {
            let mut last = None;
            loop {
                let text = archr_io::read_utf8(&file, 4096, archr_io::Symlinks::Follow)
                    .unwrap_or_default();
                let view = parse(&text);
                if last.as_ref() != Some(&view) {
                    if send.send(view.clone()).is_err() {
                        return;
                    }
                    last = Some(view);
                }
                std::thread::sleep(READ_EVERY);
            }
        });
    receive
}

pub fn run(paths: &Paths, file: &Path, windowed: bool) -> Result<(), String> {
    let language = SystemCfg::load(&paths.config)
        .ok()
        .and_then(|cfg| cfg.get_local("system", None, "language"));
    let catalog = I18n::load(
        &paths.at("/usr/share/archr-launcher/locales"),
        language.as_deref(),
        std::env::var("LANG").ok().as_deref(),
    );
    let default = paths.at(DEFAULT_THEME_DIR);
    let mut theme = if default.join("theme.toml").is_file() {
        Theme::load(&default)
    } else {
        Theme::builtin()
    };
    theme.font = theme.font.as_deref().map(|font| paths.remap(font));
    let progress = watch(file);
    let ttf = sdl2::ttf::init().map_err(|error| error.to_string())?;
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let size = if windowed {
        (640, 480)
    } else {
        let mode = video.desktop_display_mode(0)?;
        (mode.w.max(1) as u32, mode.h.max(1) as u32)
    };
    // Declared first: the textures it makes must outlive the Ui.
    let creator;
    let mut ui = Ui::with_video(sdl, video, size, windowed, StartupTrace::default())?;
    creator = ui.canvas.texture_creator();
    ui.catalog = catalog;
    ui.default_theme_dir = Some(default);
    ui.configure(&ttf, &creator, &theme, paths.root.as_deref())?;
    let capture = std::env::var_os("ARCHR_LAUNCHER_SCREENSHOT");
    let began = std::time::Instant::now();
    let mut events = ui.sdl.event_pump()?;
    let mut view = RomsMove::default();
    loop {
        // SDL turns SIGTERM and SIGINT into Quit.
        if let Some(Event::Quit { .. }) = events.wait_event_timeout(FRAME_MS) {
            return Ok(());
        }
        while let Ok(next) = progress.try_recv() {
            view = next;
        }
        let shot = capture.is_some() && began.elapsed() >= CAPTURE_AFTER;
        if shot {
            ui.screenshot = capture.clone().map(Into::into);
        }
        ui.draw_roms_move(&view, &theme)?;
        if shot {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_progress_file_gives_stage_entry_bytes_and_time_left() {
        let view = parse(
            "STAGE=verifying\nPROGRESS=34/55\nENTRY=snes\nBYTES=5153960755/8375186227\nLEFT=180\n",
        );
        assert_eq!(
            view,
            RomsMove {
                stage: MoveStage::Verifying,
                entry: (34, 55, "snes".into()),
                bytes: (5153960755, 8375186227),
                left: Some(180),
            }
        );
    }
    #[test]
    fn a_missing_or_torn_file_is_the_preparing_screen() {
        assert_eq!(parse(""), RomsMove::default());
        let torn = parse("STAGE=copying\nPROGRESS=3/\nBYTES=12");
        assert_eq!(torn.stage, MoveStage::Copying);
        assert_eq!((torn.entry.0, torn.entry.1, torn.bytes), (0, 0, (0, 0)));
        // The file written before this screen existed: entries only.
        let old = parse("PROGRESS=2/9\nENTRY=nes\n");
        assert_eq!(
            (old.stage, old.entry),
            (MoveStage::Preparing, (2, 9, "nes".into()))
        );
    }
}
