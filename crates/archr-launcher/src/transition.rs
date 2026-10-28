//! Shared event-loop terminal transition, including delayed core-write handoffs.
use archr_launcher::{
    library::Library,
    state::{App, DataSource, Outcome, SessionAction},
};
use archr_ui::{i18n::I18n, render::Ui, Theme};

/// Blocking durability work is allowed only at a terminal transition: present
/// immediate feedback first, then flush Store and publish the durable handoff.
/// A persistence/handoff failure cancels exit and restores the launcher/banner.
/// `None` requests window-close (Store flush only, without supervisor handoff).
pub fn apply(
    ui: &mut Ui<'_, '_>,
    app: &App,
    library: &mut Library,
    theme: &Theme,
    outcome: Option<Outcome>,
    controllers: &str,
    cover: Option<&sdl2::render::Texture<'_>>,
) -> Result<bool, String> {
    ui.transition = message(outcome.as_ref(), library, &ui.catalog);
    if ui.transition.is_some() {
        ui.draw(app, library, theme, cover)?; // draw includes present, before any flush
    }
    let power = matches!(
        outcome,
        Some(Outcome::Action(
            SessionAction::Shutdown | SessionAction::Reboot
        ))
    );
    let result = match outcome {
        Some(Outcome::Action(action)) if power => library.finish().and_then(|()| {
            #[cfg(not(test))]
            let systemctl = "/usr/bin/systemctl";
            // Unit tests must never invoke the host's power manager.
            #[cfg(test)]
            let systemctl = std::env::var_os("ARCHR_TEST_SYSTEMCTL").unwrap_or_default();
            let command = if action == SessionAction::Shutdown {
                "poweroff"
            } else {
                "reboot"
            };
            let status = std::process::Command::new(systemctl)
                .arg(command)
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(false)
            } else {
                Err(format!("systemctl {command}: {status}"))
            }
        }),
        Some(outcome) => library.handle(outcome, controllers),
        None => library.finish().map(|()| true),
    };
    if !power || result.is_err() {
        ui.transition = None;
    }
    match result {
        Ok(exit) => Ok(exit),
        Err(error) => {
            log::warn!("Operation failed: {error}");
            ui.banner = Some(ui.catalog.text(if error.starts_with("error.") {
                &error
            } else {
                "error.operation"
            }));
            ui.draw(app, library, theme, cover)?;
            Ok(false)
        }
    }
}
fn message(outcome: Option<&Outcome>, data: &dyn DataSource, catalog: &I18n) -> Option<String> {
    Some(match outcome {
        None => catalog.text("notice.closing"),
        Some(Outcome::Launch { system, game }) => catalog.format(
            "notice.opening",
            &[(
                "game",
                &data
                    .games(system)
                    .get(*game)
                    .map(|game| game.name.clone())
                    .unwrap_or_else(|| catalog.text("notice.game")),
            )],
        ),
        Some(Outcome::Action(action)) => catalog.text(match action {
            SessionAction::Shutdown => "notice.shutdown",
            SessionAction::Reboot => "notice.reboot",
            SessionAction::Suspend => "notice.suspend",
        }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    mod common {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../archr-ui/tests/common/mod.rs"
        ));
    }
    use archr_launcher::{paths::Paths, state::SessionAction};
    use std::{
        fs,
        time::{Duration, Instant},
    };

    #[test]
    fn terminal_transitions_cover_failures_and_persistent_power_notices() {
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.run.parent().unwrap()).unwrap();
        // An unwritable handoff location, even when tests run as root.
        fs::write(&paths.run, "not a directory").unwrap();
        let mut library = Library::initial(paths.clone());
        library.start();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !library.ready {
            library.poll();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let ttf;
        let creator;
        let mut ui = Ui::new(640, 480, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        let theme = Theme::builtin();
        ui.configure(&ttf, &creator, &theme, None).unwrap();
        let frame = root.path().join("transition.png");
        ui.screenshot = Some(frame.clone());
        let exit = apply(
            &mut ui,
            &App::new(10),
            &mut library,
            &theme,
            Some(Outcome::Action(SessionAction::Suspend)),
            "",
            None,
        )
        .unwrap();
        assert!(!exit, "failed handoff must return to the event loop");
        assert!(ui.banner.as_ref().is_some_and(|banner| !banner.is_empty()));
        assert!(
            ui.transition.is_none(),
            "failure must dismiss the busy overlay"
        );
        assert!(!paths.run.join("next").exists());
        use sdl2::image::LoadSurface;
        let image = sdl2::surface::Surface::from_file(frame)
            .unwrap()
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        let color = theme.surface_elevated;
        let offset = 220 * image.pitch() as usize + 140 * 4;
        image.with_lock(|bytes| {
            assert_eq!(
                &bytes[offset..offset + 4],
                &[color.r, color.g, color.b, color.a]
            )
        });
        assert!(
            paths.store.join("store.toml").exists(),
            "handoff follows the store flush"
        );
        // Flush failures must also cancel action and window-close transitions.
        fs::remove_file(paths.store.join("store.toml")).unwrap();
        fs::create_dir(paths.store.join("store.toml")).unwrap();
        for outcome in [Some(Outcome::Action(SessionAction::Shutdown)), None] {
            ui.banner = None;
            assert!(!apply(
                &mut ui,
                &App::new(10),
                &mut library,
                &theme,
                outcome,
                "",
                None
            )
            .unwrap());
            assert!(ui.banner.is_some());
            assert!(ui.transition.is_none());
        }
        // Repair the destinations and verify the same launcher can retry.
        fs::remove_dir(paths.store.join("store.toml")).unwrap();
        fs::remove_file(&paths.run).unwrap();
        assert!(apply(
            &mut ui,
            &App::new(10),
            &mut library,
            &theme,
            Some(Outcome::Action(SessionAction::Suspend)),
            "",
            None
        )
        .unwrap());
        assert_eq!(
            fs::read_to_string(paths.run.join("next")).unwrap(),
            "action suspend\n"
        );
        fs::remove_file(paths.run.join("next")).unwrap();
        // The command must observe both the presented notice and flushed Store.
        use std::os::unix::fs::PermissionsExt;
        let systemctl = root.path().join("systemctl");
        fs::write(
            &systemctl,
            r#"#!/bin/sh
dir=${0%/*}
[ -f "$dir/power.png" ] && [ -f "$dir/storage/.config/archr-launcher/store.toml" ] || exit 1
printf '%s\n' "$1" >>"$dir/commands"
"#,
        )
        .unwrap();
        fs::set_permissions(&systemctl, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var("ARCHR_TEST_SYSTEMCTL", &systemctl);
        ui.catalog = I18n::load(root.path(), Some("pt_BR"), None);
        for (action, notice) in [
            (SessionAction::Shutdown, "Desligando…"),
            (SessionAction::Reboot, "Reiniciando…"),
        ] {
            fs::remove_file(paths.store.join("store.toml")).unwrap();
            ui.screenshot = Some(root.path().join("power.png"));
            assert!(!apply(
                &mut ui,
                &App::new(10),
                &mut library,
                &theme,
                Some(Outcome::Action(action)),
                "",
                None
            )
            .unwrap());
            assert_eq!(ui.transition.as_deref(), Some(notice));
            assert!(!paths.run.join("next").exists());
        }
        assert_eq!(
            fs::read_to_string(root.path().join("commands")).unwrap(),
            "poweroff\nreboot\n"
        );
        for program in [
            std::path::Path::new("/bin/false"),
            &root.path().join("missing"),
        ] {
            std::env::set_var("ARCHR_TEST_SYSTEMCTL", program);
            ui.banner = None;
            assert!(!apply(
                &mut ui,
                &App::new(10),
                &mut library,
                &theme,
                Some(Outcome::Action(SessionAction::Reboot)),
                "",
                None
            )
            .unwrap());
            assert!(ui.transition.is_none());
            assert!(ui.banner.is_some());
            assert!(!paths.run.join("next").exists());
        }
        // Exercise the real event loop: later scripted actions and QUIT must be
        // ignored after systemctl succeeds; only external SDL Quit ends the notice.
        let sender = ui.sdl.event().unwrap().event_sender();
        drop(ui);
        std::env::set_var("ARCHR_LAUNCHER_ROOT", root.path());
        std::env::set_var("ARCHR_LAUNCHER_SCREENSHOT", root.path().join("power.png"));
        std::env::set_var("ARCHR_TEST_SYSTEMCTL", &systemctl);
        for (sequence, command) in [
            ("Y,DOWN,DOWN,A,SNAPSHOT,DOWN,A,QUIT", "poweroff\n"),
            ("Y,DOWN,A,SNAPSHOT,DOWN,A,QUIT", "reboot\n"),
        ] {
            fs::remove_file(root.path().join("commands")).unwrap();
            fs::remove_file(root.path().join("power.png")).unwrap();
            let finished = std::sync::atomic::AtomicBool::new(false);
            std::thread::scope(|scope| {
                let observer = scope.spawn(|| {
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while !root.path().join("commands").exists()
                        && !finished.load(std::sync::atomic::Ordering::Relaxed)
                        && Instant::now() < deadline
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    let requested = root.path().join("commands").exists();
                    if requested {
                        std::thread::sleep(Duration::from_millis(600));
                    }
                    if finished.load(std::sync::atomic::Ordering::Relaxed) {
                        return false;
                    }
                    // Also unblock a broken run when the power request never arrived.
                    sender
                        .push_event(sdl2::event::Event::Quit { timestamp: 0 })
                        .unwrap();
                    requested
                });
                let began = Instant::now();
                let result = crate::run(
                    began,
                    archr_ui::startup::StartupTrace::new(began),
                    ["--windowed", "--script", sequence]
                        .into_iter()
                        .map(str::to_owned),
                );
                finished.store(true, std::sync::atomic::Ordering::Relaxed);
                assert!(
                    observer.join().unwrap(),
                    "launcher left the notice before external Quit"
                );
                result.unwrap();
            });
            assert_eq!(
                fs::read_to_string(root.path().join("commands")).unwrap(),
                command
            );
            assert!(!paths.run.join("next").exists());
        }
        std::env::remove_var("ARCHR_LAUNCHER_ROOT");
        std::env::remove_var("ARCHR_LAUNCHER_SCREENSHOT");
        std::env::remove_var("ARCHR_TEST_SYSTEMCTL");
    }
    #[test]
    fn terminal_labels_match_requested_actions_and_display_game_name() {
        use archr_index::Game;
        let data = common::FakeData {
            games: [(
                "nes".into(),
                vec![Game {
                    name: "Super Mario".into(),
                    path: "game.nes".into(),
                    image: None,
                    video: None,
                    favorite: false,
                    year: None,
                    players: None,
                    last_played: None,
                }],
            )]
            .into(),
            ..common::FakeData::default()
        };
        assert_eq!(
            message(
                Some(&Outcome::Launch {
                    system: "nes".into(),
                    game: 0
                }),
                &data,
                &I18n::default()
            )
            .as_deref(),
            Some("Opening Super Mario…")
        );
        for (action, expected) in [
            (SessionAction::Shutdown, "Powering off…"),
            (SessionAction::Reboot, "Restarting…"),
            (SessionAction::Suspend, "Going to sleep…"),
        ] {
            assert_eq!(
                message(Some(&Outcome::Action(action)), &data, &I18n::default()).as_deref(),
                Some(expected)
            );
        }
        assert_eq!(
            message(None, &data, &I18n::default()).as_deref(),
            Some("Closing…")
        );
        assert_eq!(message(Some(&Outcome::None), &data, &I18n::default()), None);
    }
}
