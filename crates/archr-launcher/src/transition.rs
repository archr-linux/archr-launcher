//! Shared event-loop terminal transition, including delayed core-write handoffs.
use archr_launcher::{
    audio::Audio,
    library::Library,
    state::{App, DataSource, Outcome, SessionAction},
};
use archr_ui::{i18n::I18n, render::Ui, Theme};
use std::{
    os::unix::process::CommandExt,
    process::{Command, Stdio},
};

/// Blocking durability work is allowed only at a terminal transition: present
/// immediate feedback first, then flush Store and publish the durable handoff.
/// A persistence/handoff failure cancels exit and restores the launcher/banner.
/// `None` requests window-close (Store flush only, without supervisor handoff).
#[allow(clippy::too_many_arguments)]
pub fn apply(
    ui: &mut Ui<'_, '_>,
    audio: &mut Audio,
    app: &App,
    library: &mut Library,
    theme: &Theme,
    outcome: Option<Outcome>,
    controllers: &str,
    cover: Option<&sdl2::render::Texture<'_>>,
) -> Result<bool, String> {
    // A game, tool or power request must find the sound output free.
    if outcome.as_ref().is_none_or(is_terminal) {
        audio.release();
    }
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
        Some(Outcome::Action(action)) => library.finish().and_then(|()| {
            #[cfg(not(test))]
            let systemctl = library.paths.at("/usr/bin/systemctl").into_os_string();
            // Unit tests must never invoke the host's power manager.
            #[cfg(test)]
            let systemctl = std::env::var_os("ARCHR_TEST_SYSTEMCTL").unwrap_or_default();
            let (program, argument) = match action {
                SessionAction::Suspend => {
                    #[cfg(not(test))]
                    let functions = library
                        .paths
                        .at("/etc/profile.d/001-functions")
                        .into_os_string();
                    #[cfg(test)]
                    let functions = std::env::var_os("ARCHR_TEST_FUNCTIONS").unwrap_or_default();
                    let mode = Command::new("/usr/bin/bash")
                        .args([
                            "-c",
                            ". \"$1\"; get_setting system.suspendmode",
                            "archr-suspend",
                        ])
                        .arg(functions)
                        .output()
                        .map_err(|e| e.to_string())?;
                    if !mode.status.success() {
                        return Err(format!("get_setting: {}", mode.status));
                    }
                    if matches!(
                        String::from_utf8_lossy(&mode.stdout).trim_end_matches('\n'),
                        "" | "off"
                    ) {
                        #[cfg(not(test))]
                        let fake_suspend = library
                            .paths
                            .at("/usr/bin/archr-fake-suspend")
                            .into_os_string();
                        #[cfg(test)]
                        let fake_suspend =
                            std::env::var_os("ARCHR_TEST_FAKE_SUSPEND").unwrap_or_default();
                        (fake_suspend, "power")
                    } else {
                        (systemctl, "suspend")
                    }
                }
                SessionAction::Shutdown => (systemctl, "poweroff"),
                SessionAction::Reboot => (systemctl, "reboot"),
            };
            let mut command = Command::new(program);
            command.arg(argument);
            if action == SessionAction::Suspend {
                let mut child = command
                    .process_group(0)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                // Reap without blocking the UI, even while fake-suspend sleeps.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(false)
            } else {
                let status = command.status().map_err(|e| e.to_string())?;
                if status.success() {
                    Ok(false)
                } else {
                    Err(format!("systemctl {argument}: {status}"))
                }
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
/// Outcomes that end this launcher session or hand the screen to another program.
pub fn is_terminal(outcome: &Outcome) -> bool {
    matches!(
        outcome,
        Outcome::Launch { .. }
            | Outcome::Netplay(_)
            | Outcome::MoonlightStream(_)
            | Outcome::Action(_)
    )
}
fn message(outcome: Option<&Outcome>, data: &dyn DataSource, catalog: &I18n) -> Option<String> {
    Some(match outcome {
        None => catalog.text("notice.closing"),
        Some(Outcome::MoonlightStream(stream)) => {
            catalog.format("notice.opening", &[("game", &stream.app)])
        }
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
        Some(Outcome::Netplay(join)) => catalog.format("notice.opening", &[("game", &join.game)]),
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
    use archr_ui::sounds::Sounds;
    use sdl2::{
        event::{Event, WindowEvent},
        keyboard::{Keycode, Mod},
    };
    use std::{
        fs,
        path::Path,
        time::{Duration, Instant},
    };

    fn invoke(ui: &mut Ui<'_, '_>, library: &mut Library, action: Option<SessionAction>) -> bool {
        let app = App::new(10);
        let theme = Theme::builtin();
        let outcome = action.map(Outcome::Action);
        let mut audio = Audio::new(Sounds::default(), false, 0);
        apply(ui, &mut audio, &app, library, &theme, outcome, "", None).unwrap()
    }

    fn key_down(key: Keycode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(key),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        }
    }

    #[test]
    fn terminal_transitions_cover_failures_and_persistent_power_notices() {
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
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
        library.finish().unwrap();
        // Flush failures must also cancel action and window-close transitions.
        fs::remove_file(paths.store.join("store.toml")).unwrap();
        fs::create_dir(paths.store.join("store.toml")).unwrap();
        for outcome in [
            Some(SessionAction::Shutdown),
            Some(SessionAction::Suspend),
            None,
        ] {
            ui.banner = None;
            assert!(!invoke(&mut ui, &mut library, outcome));
            assert!(ui.banner.is_some());
            assert!(ui.transition.is_none());
        }
        // Repair the destinations and verify the same launcher can retry.
        fs::remove_dir(paths.store.join("store.toml")).unwrap();
        library.finish().unwrap();
        // The command must observe both the presented notice and flushed Store.
        use std::os::unix::fs::PermissionsExt;
        let systemctl = root.path().join("systemctl");
        fs::write(
            &systemctl,
            r#"#!/bin/sh
dir=${0%/*}
[ -f "$dir/power.png" ] && [ -f "$dir/storage/.config/archr-launcher/store.toml" ] || exit 1
case "$1" in
    power|suspend)
        [ "$(awk '{print $5}' /proc/$$/stat)" = "$$" ] || exit 1
        for fd in 0 1 2; do
            [ "$(readlink /proc/$$/fd/$fd)" = /dev/null ] || exit 1
        done
        printf '%s %s\n' "$1" "$$" >"$dir/child"
        i=0
        while [ ! -f "$dir/release" ] && [ "$i" -lt 500 ]; do
            /bin/sleep 0.01
            i=$((i + 1))
        done
        ;;
    *) printf '%s\n' "$1" >>"$dir/commands" ;;
esac
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
            assert!(!invoke(&mut ui, &mut library, Some(action)));
            assert_eq!(ui.transition.as_deref(), Some(notice));
            assert!(!paths.run.join("next").exists());
        }
        assert_eq!(
            fs::read_to_string(root.path().join("commands")).unwrap(),
            "poweroff\nreboot\n"
        );
        for program in [Path::new("/bin/false"), &root.path().join("missing")] {
            std::env::set_var("ARCHR_TEST_SYSTEMCTL", program);
            ui.banner = None;
            assert!(!invoke(&mut ui, &mut library, Some(SessionAction::Reboot)));
            assert!(ui.transition.is_none());
            assert!(ui.banner.is_some());
            assert!(!paths.run.join("next").exists());
        }
        // The settings helper selects a detached, nonblocking command on
        // each request. The child observes the flush and presentation first.
        let functions = root.path().join("functions");
        // Self-contained shell lookup fixture; no sibling checkout dependency.
        fs::write(&functions, format!(r#"get_setting() {{
    for key in "system.$1" "$1" "global.$1"; do
        value=$(awk -v key="$key" 'index($0,key "=")==1 {{sub(/^[^=]*=/, ""); gsub(/^[ \t]+|[ \t]+$/, ""); print; exit}}' '{}')
        if [ -n "$value" ]; then printf '%s\n' "$value"; return; fi
    done
}}
"#, paths.config.display())).unwrap();
        let fake_suspend = root.path().join("fake-suspend");
        fs::copy(&systemctl, &fake_suspend).unwrap();
        std::env::set_var("ARCHR_TEST_FUNCTIONS", &functions);
        std::env::set_var("ARCHR_TEST_FAKE_SUSPEND", &fake_suspend);
        std::env::set_var("ARCHR_TEST_SYSTEMCTL", &systemctl);
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        for (config, expected) in [
            ("", "power"),
            ("system.suspendmode=off\nsystem.suspendmode=mem", "power"),
            ("system.suspendmode=mem\nsystem.suspendmode=off", "suspend"),
            ("system.suspendmode= off ", "power"),
            ("global.system.suspendmode=mem", "suspend"),
        ] {
            fs::write(&paths.config, config).unwrap();
            fs::remove_file(paths.store.join("store.toml")).unwrap();
            fs::remove_file(root.path().join("power.png")).unwrap();
            ui.screenshot = Some(root.path().join("power.png"));
            assert!(!invoke(&mut ui, &mut library, Some(SessionAction::Suspend)));
            assert!(
                ui.transition.is_none(),
                "suspend must not idle the event loop"
            );
            assert!(!paths.run.join("next").exists());
            let deadline = Instant::now() + Duration::from_secs(5);
            let record = loop {
                if let Ok(record) = fs::read_to_string(root.path().join("child")) {
                    if record.ends_with('\n') {
                        break record;
                    }
                }
                assert!(Instant::now() < deadline, "suspend command did not start");
                std::thread::sleep(Duration::from_millis(5));
            };
            let (argument, pid) = record.trim().split_once(' ').unwrap();
            assert_eq!(argument, expected);
            let proc = Path::new("/proc").join(pid);
            assert!(proc.exists(), "suspend waited for its child");
            ui.draw(&App::new(10), &library, &theme, None).unwrap();
            fs::write(root.path().join("release"), []).unwrap();
            while proc.exists() {
                assert!(Instant::now() < deadline, "detached child was not reaped");
                std::thread::sleep(Duration::from_millis(5));
            }
            fs::remove_file(root.path().join("child")).unwrap();
            fs::remove_file(root.path().join("release")).unwrap();
        }
        std::env::remove_var("ARCHR_TEST_FUNCTIONS");
        std::env::remove_var("ARCHR_TEST_FAKE_SUSPEND");
        // Drive real SDL input while a core write is blocked. The latest power
        // request must supersede the older deferred game, then remain idle.
        fs::create_dir_all(paths.systems.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.at("/storage/roms/nes")).unwrap();
        fs::write(paths.at("/storage/roms/nes/A.nes"), []).unwrap();
        fs::write(&paths.systems, "<systemList><system><name>nes</name><path>/storage/roms/nes</path><extension>.nes</extension><emulators><emulator name=\"retroarch\"><cores><core>a</core><core>b</core></cores></emulator></emulators></system></systemList>").unwrap();
        // A failed load makes Store read-only, not the terminal actions.
        let invalid = b"not TOML [";
        fs::write(paths.store.join("store.toml"), invalid).unwrap();
        let mut readonly = Library::initial(paths.clone());
        readonly.start();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !readonly.ready || readonly.loading("nes") {
            readonly.poll();
            readonly.ensure("nes");
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let warning = ui.catalog.text("error.store_load");
        assert_eq!(readonly.banner.take().unwrap().render(&ui.catalog), warning);
        ui.banner = Some(warning.clone());
        fs::remove_file(root.path().join("commands")).unwrap();
        assert!(!invoke(
            &mut ui,
            &mut readonly,
            Some(SessionAction::Shutdown)
        ));
        assert_eq!(ui.transition.as_deref(), Some("Desligando…"));
        assert_eq!(
            fs::read_to_string(root.path().join("commands")).unwrap(),
            "poweroff\n"
        );
        assert_eq!(fs::read(paths.store.join("store.toml")).unwrap(), invalid);
        // Only a hand-off frees the sound output; other outcomes leave it be.
        std::env::set_var("SDL_AUDIODRIVER", "dummy");
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default");
        let mut audio = Audio::new(Sounds::load(&shipped), true, 100);
        assert!(audio.is_open());
        let mut go = |audio: &mut Audio, outcome| {
            apply(
                &mut ui,
                audio,
                &App::new(10),
                &mut readonly,
                &theme,
                Some(outcome),
                "",
                None,
            )
            .unwrap()
        };
        assert!(!go(&mut audio, Outcome::None));
        assert!(audio.is_open());
        let launch = Outcome::Launch {
            system: "nes".into(),
            game: 0,
        };
        assert!(go(&mut audio, launch));
        assert!(!audio.is_open(), "the emulator needs the output");
        assert!(sdl2::mixer::query_spec().is_err());
        let next = fs::read_to_string(paths.run.join("next")).unwrap();
        assert!(next.starts_with("game "));
        assert!(next.contains("A.nes' '-Pnes' '--core=a' '--emulator=retroarch'"));
        assert_eq!(ui.banner.as_deref(), Some(warning.as_str()));
        assert!(invoke(&mut ui, &mut readonly, None));
        assert_eq!(fs::read(paths.store.join("store.toml")).unwrap(), invalid);
        fs::remove_file(paths.run.join("next")).unwrap();
        drop(readonly);
        let sender = ui.sdl.event().unwrap().event_sender();
        drop(ui);
        std::env::set_var("ARCHR_LAUNCHER_ROOT", root.path());
        std::env::set_var("ARCHR_TEST_SYSTEMCTL", &systemctl);
        for (target, command) in [(4, "poweroff\n"), (3, "reboot\n")] {
            fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
            fs::write(&paths.lock, std::process::id().to_string()).unwrap();
            let mut phase = 0;
            fs::remove_file(root.path().join("commands")).unwrap();
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
                        sender.push_event(key_down(Keycode::Return)).unwrap();
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
                    ["--windowed"].into_iter().map(str::to_owned),
                    |app, library, deferred| {
                        use archr_launcher::state::Screen;
                        let key = match phase {
                            0 if library.ready => Some(Keycode::Return),
                            1 if matches!(app.screen, Screen::Games { .. })
                                && !library.loading("nes") =>
                            {
                                Some(Keycode::Y)
                            }
                            2 if matches!(app.screen, Screen::QuickMenu { .. }) => {
                                Some(Keycode::Right)
                            }
                            3 if library.pending_writes == 1 => Some(Keycode::Backspace),
                            4 if matches!(app.screen, Screen::Games { .. }) => {
                                Some(Keycode::Return)
                            }
                            5 if deferred.is_some() => Some(Keycode::Y),
                            // Core, Details, Suspend, Reboot (3), Shutdown (4).
                            6 if app.cursor == 0 => Some(Keycode::Down),
                            7 if app.cursor == 1 => Some(Keycode::Down),
                            8 if app.cursor == 2 => Some(Keycode::Down),
                            9 if target == 4 && app.cursor == 3 => Some(Keycode::Down),
                            9 | 10 if app.cursor == target => {
                                assert_eq!(library.pending_writes, 1);
                                fs::remove_file(&paths.lock).unwrap();
                                phase = 11;
                                None
                            }
                            11 if library.pending_writes == 0 => {
                                assert!(!library.write_failed);
                                assert!(
                                    matches!(deferred, Some(Outcome::Launch { system, game: 0 }) if system == "nes")
                                );
                                Some(Keycode::Return)
                            }
                            _ => None,
                        };
                        if let Some(key) = key {
                            phase += 1;
                            sender.push_event(key_down(key)).unwrap();
                            sender
                                .push_event(Event::Window {
                                    timestamp: 0,
                                    window_id: 0,
                                    win_event: WindowEvent::FocusLost,
                                })
                                .unwrap();
                        }
                    },
                );
                finished.store(true, std::sync::atomic::Ordering::Relaxed);
                assert!(
                    observer.join().unwrap(),
                    "launcher left the notice before external Quit"
                );
                result.unwrap();
            });
            assert_eq!(phase, 12, "power input followed a real pending core write");
            assert_eq!(
                fs::read_to_string(root.path().join("commands")).unwrap(),
                command
            );
            assert!(!paths.run.join("next").exists());
        }
        std::env::remove_var("ARCHR_LAUNCHER_ROOT");
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
                    details: Default::default(),
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
