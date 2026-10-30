//! `--roms-migrate-screen`: the screen of the maintenance boot that moves
//! the ROMs to the exFAT partition (archr-roms-migrate.target). No
//! compositor and no GPU: SDL's software renderer draws each frame and it is
//! copied to the console's framebuffer (fb.rs). It only shows what
//! archr-roms-migrate writes; the migration never waits for it, and when it
//! fails the migration goes on, on the text console.
//!
//! STORAGE is unmounted while it shrinks, and a file kept open there would
//! stop that: system.cfg is read once at start, everything else comes from
//! the packaged default theme.
use archr_config::SystemCfg;
use archr_launcher::paths::Paths;
use archr_ui::{
    i18n::I18n,
    render::{MoveStage, RomsMove, Ui, DOT_STEP},
    startup::StartupTrace,
    Theme, DEFAULT_THEME_DIR,
};
use sdl2::event::Event;
use std::{path::Path, sync::mpsc, time::Duration};

/// archr-roms-migrate's progress file.
pub const PROGRESS: &str = "/run/archr/roms-migrate/progress";
/// How often the file is read.
const READ_EVERY: Duration = Duration::from_millis(500);
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
    let mut screen = match windowed {
        true => None,
        false => Some(crate::fb::Framebuffer::open()?),
    };
    let size = screen.as_ref().map_or((640, 480), |fb| fb.size());
    if screen.is_some() {
        // Software rendering into memory; fb.rs shows it.
        sdl2::hint::set_with_priority("SDL_VIDEODRIVER", "dummy", &sdl2::hint::Hint::Override);
    }
    let progress = watch(file);
    let ttf = sdl2::ttf::init().map_err(|error| error.to_string())?;
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    // Declared first: the textures it makes must outlive the Ui.
    let creator;
    let mut ui = Ui::with_video(sdl, video, size, true, StartupTrace::default())?;
    creator = ui.canvas.texture_creator();
    ui.catalog = catalog;
    ui.default_theme_dir = Some(default);
    ui.configure(&ttf, &creator, &theme, paths.root.as_deref())?;
    let capture = std::env::var_os("ARCHR_LAUNCHER_SCREENSHOT");
    let began = std::time::Instant::now();
    let mut events = ui.sdl.event_pump()?;
    let mut view = RomsMove::default();
    // One frame per step of the dots, then sleep: nothing spins between them.
    for step in 0.. {
        // SDL turns SIGTERM and SIGINT into Quit.
        if events.poll_iter().any(|e| matches!(e, Event::Quit { .. })) {
            return Ok(());
        }
        while let Ok(next) = progress.try_recv() {
            view = next;
        }
        let shot = capture.is_some() && began.elapsed() >= CAPTURE_AFTER;
        if shot {
            ui.screenshot = capture.clone().map(Into::into);
        }
        ui.draw_roms_move(&view, step, &theme)?;
        if let Some(fb) = &mut screen {
            let pixels = ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::ARGB8888)?;
            fb.show(&pixels, size.0 as usize * 4);
        }
        if shot {
            return Ok(());
        }
        let next = DOT_STEP * (step + 1);
        std::thread::sleep(next.saturating_sub(began.elapsed()));
    }
    Ok(())
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
