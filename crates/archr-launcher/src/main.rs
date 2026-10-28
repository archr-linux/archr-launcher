//! SDL integration. Disk scanning and locked config updates run in workers.
mod script;
mod transition;
use archr_config::SystemCfg;
use archr_launcher::{
    library::Library,
    netplay,
    paths::Paths,
    state::{App, DataSource, Outcome, Screen},
    wifi,
};
use archr_ui::{
    controllers::Controllers,
    covers::CoverCache,
    i18n::I18n,
    input::{button_event, Repeat},
    netplay::{Effect, Lobby},
    render::{prepare_first_frame, Ui},
    startup::StartupTrace,
    startup_phase, theme_dir, Theme,
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

/// Wayland app_id matched by the compositor rules (sway focus/seat, OSD).
const APP_ID: &str = "archr-launcher";

fn main() {
    let began = Instant::now();
    // SDL reads this once at video init; set it while still single-threaded.
    // Only the launcher's own helpers inherit it; games start from the session.
    std::env::set_var("SDL_VIDEO_WAYLAND_WMCLASS", APP_ID);
    init_logging();
    if let Err(error) = run(
        began,
        StartupTrace::new(began),
        std::env::args().skip(1),
        #[cfg(test)]
        |_, _, _| {},
    ) {
        eprintln!("archr-launcher: {error}");
        std::process::exit(1);
    }
}
fn run(
    began: Instant,
    trace: StartupTrace,
    mut args: impl Iterator<Item = String>,
    #[cfg(test)] mut observe: impl FnMut(&App, &Library, Option<&Outcome>),
) -> Result<(), String> {
    let mut windowed = false;
    let mut script_source = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--windowed" => windowed = true,
            "--script" => {
                script_source = Some(args.next().ok_or("--script requires a button sequence")?)
            }
            "--help" => {
                println!("archr-launcher [--windowed]");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    if script_source.is_some() && !windowed {
        return Err("--script requires --windowed".to_owned());
    }
    let mut script = script_source
        .as_deref()
        .map(script::Script::parse)
        .transpose()?;
    let paths = Paths::new(std::env::var_os("ARCHR_LAUNCHER_ROOT").map(PathBuf::from));
    startup_phase!(trace, "args/paths");
    // SDL_ttf's global FreeType library is initialized before the worker and
    // remains alive until every handed-over font is dropped. Only the worker
    // uses TTF until join; video/window/EGL stay on this thread throughout.
    let ttf = sdl2::ttf::init().map_err(|error| error.to_string())?;
    startup_phase!(trace, "TTF_Init");
    let creator;
    let (mut ui, (theme, theme_path, catalog, mut library, prepared, netplay_on)) =
        std::thread::scope(|scope| {
            let (sized, size) = std::sync::mpsc::sync_channel(1);
            let paths = &paths;
            let ttf = &ttf;
            let worker = scope.spawn(move || -> Result<_, String> {
                let config = SystemCfg::load(&paths.config).ok();
                startup_phase!(trace, "system.cfg read");
                let language = config
                    .as_ref()
                    .and_then(|cfg| cfg.get_local("system", None, "language"));
                let catalog = I18n::load(
                    &paths.at("/usr/share/archr-launcher/locales"),
                    language.as_deref(),
                    std::env::var("LANG").ok().as_deref(),
                );
                startup_phase!(trace, "locale selection");
                let name = config
                    .as_ref()
                    .and_then(|cfg| cfg.get("launcher", None, "theme"))
                    .unwrap_or_else(|| "default".to_owned());
                let theme_path = theme_dir(
                    &paths.at("/storage/.config/archr-launcher/themes"),
                    &paths.at("/usr/share/archr-launcher/themes"),
                    &name,
                );
                let mut theme = theme_path
                    .as_deref()
                    .map_or_else(Theme::builtin, Theme::load);
                theme.font = theme.font.as_deref().map(|font| paths.remap(font));
                startup_phase!(trace, "theme load");
                // No per-system scans, controller enumeration or cover work yet.
                let library = Library::initial_traced(paths.clone(), trace);
                let size = size
                    .recv()
                    .map_err(|_| "Video initialization failed".to_owned())?;
                let mut prepared = prepare_first_frame(
                    ttf,
                    &theme,
                    (
                        paths.root.as_deref(),
                        theme_path.as_deref(),
                        &paths.at(archr_ui::DEFAULT_ICONS_DIR),
                    ),
                    &catalog,
                    &library,
                    size,
                    trace,
                )?;
                if let Some(banner) = &library.banner {
                    prepared.prepare_notice(&banner.render(&catalog), &theme, size)?;
                }
                startup_phase!(trace, "CPU assets ready");
                let netplay_on = config
                    .as_ref()
                    .and_then(|cfg| cfg.get_setting("netplay", None, None))
                    .is_some_and(|value| value == "1");
                Ok((theme, theme_path, catalog, library, prepared, netplay_on))
            });
            let sdl = sdl2::init()?;
            let video = sdl.video()?;
            startup_phase!(trace, "SDL_Init video");
            let size = if windowed {
                (640, 480)
            } else {
                let mode = video.desktop_display_mode(0)?;
                (mode.w.max(1) as u32, mode.h.max(1) as u32)
            };
            // A stopped receiver is reported by join, preserving the worker error.
            let _ = sized.send(size);
            let ui = Ui::with_video(sdl, video, (640, 480), windowed, trace);
            // Always join, including window/renderer errors, before TTF can quit.
            let assets = worker
                .join()
                .map_err(|_| "Startup worker panicked".to_owned())?;
            startup_phase!(trace, "worker join");
            Ok::<_, String>((ui?, assets?))
        })?;
    creator = ui.canvas.texture_creator();
    ui.catalog = catalog;
    ui.theme_dir = theme_path;
    ui.configure_prepared(&creator, prepared, paths.root.as_deref())?;
    let mut app = App::new(ui.page_size(&theme));
    app.netplay = netplay_on;
    ui.banner = library.banner.take().map(|m| m.render(&ui.catalog));
    ui.draw(&app, &library, &theme, None)?;
    eprintln!(
        "archr-launcher: first frame {} ms ({} system rows)",
        began.elapsed().as_millis(),
        App::system_rows(&library).len()
    );
    library.start();
    let (width, height) = ui.canvas.output_size()?;
    let mut settings = archr_launcher::settings::Runtime::new(paths.clone());
    let mut rooms = netplay::Runtime::new(paths.clone());
    let mut was_browsing = true;
    let mut moonlight = archr_launcher::moonlight::Runtime::new(paths.clone(), (width, height));
    let mut covers = CoverCache::new(
        &creator,
        paths.cache.join("thumbs"),
        (theme.cover.w * width as f32).round().max(1.) as u32,
        (theme.cover.h * height as f32).round().max(1.) as u32,
    );
    let mut controllers = Controllers::new_traced(&ui.sdl, trace)?;
    let mut events = ui.sdl.event_pump()?;
    let mut repeat = Repeat::default();
    let mut dirty = false;
    let mut deferred = None;
    let mut closing = false;
    let mut banner_until = Instant::now() + Duration::from_secs(8);
    loop {
        // A successful power request owns the screen until the OS stops us.
        // Store is already flushed; no more input, scans, scripts or handoffs.
        if ui.transition.is_some() {
            match events.wait_event_timeout(1000) {
                Some(Event::Quit { .. }) => return Ok(()),
                Some(Event::Window {
                    win_event: WindowEvent::Exposed | WindowEvent::SizeChanged(..),
                    ..
                }) => ui.draw(&app, &library, &theme, None)?,
                _ => {}
            }
            continue;
        }
        let previous_setting = settings.state.pending;
        let (settings_dirty, language) = settings.poll();
        if settings_dirty
            && previous_setting == Some(archr_ui::settings::Id::Timezone)
            && settings.state.pending.is_none()
            && settings.state.error.is_none()
        {
            ui.invalidate_settings_clock();
        }
        // The clock format's own reply: a failed scan elsewhere never blocks it.
        if previous_setting == Some(archr_ui::settings::Id::ClockFormat)
            && settings.state.pending.is_none()
        {
            ui.set_settings_clock_format(settings.state.value(archr_ui::settings::Id::ClockFormat));
        }
        dirty |= settings_dirty;
        dirty |= moonlight.poll();
        if let Some(language) = language {
            ui.catalog = I18n::load(
                &paths.at("/usr/share/archr-launcher/locales"),
                Some(&language),
                None,
            );
        }
        if let Some(lobby) = &mut ui.lobby {
            dirty |= rooms.poll(lobby);
        }
        // No change notification exists: re-read `global.netplay` on returning
        // to the systems screen (e.g. from the settings pages).
        let browsing = app.screen == Screen::Systems && !settings.active && ui.lobby.is_none();
        if browsing && !was_browsing {
            rooms.recheck();
        }
        was_browsing = browsing;
        if let Some(on) = rooms.enabled() {
            app.netplay = on;
        }
        dirty |= ui.tick() || ui.animating();
        let selected_system = if app.screen == Screen::Systems {
            app.rows(&library).get(app.cursor).cloned()
        } else {
            None
        };
        if library.poll() {
            if let Some(name) = selected_system {
                if let Some(index) = app.rows(&library).iter().position(|system| *system == name) {
                    app.cursor = index;
                }
            }
            dirty = true;
        }
        if let Some(banner) = library.banner.take() {
            ui.banner = Some(banner.render(&ui.catalog));
            banner_until = Instant::now() + Duration::from_secs(8);
            dirty = true;
        }
        if ui.banner.is_some() && Instant::now() >= banner_until {
            ui.banner = None;
            dirty = true;
        }
        if let Screen::Games { system } = &app.screen {
            library.ensure(system);
            ui.loading = library.loading(system);
            app.cursor = app
                .cursor
                .min(library.games(system).len().saturating_sub(1));
        } else {
            ui.loading = false;
        }
        let (selected, neighbours) = cover_paths(&app, &library);
        covers.request(&neighbours);
        dirty |= covers.poll();
        if dirty {
            ui.moonlight = moonlight.active.then(|| moonlight.state.clone());
            let cover = selected.as_ref().and_then(|path| covers.get(path));
            ui.draw_with_settings(
                &app,
                &library,
                &theme,
                cover,
                settings.active.then_some(&settings.state),
            )?;
            dirty = false;
        }
        #[cfg(test)]
        observe(&app, &library, deferred.as_ref());
        let mut button = None;
        let mut close_requested = false;
        if let Some(driver) = &mut script {
            if let Some(step) = driver.poll(
                library.ready
                    && !ui.loading
                    && library.pending_writes == 0
                    && !settings.busy()
                    && (!moonlight.active
                        || !moonlight.state.busy
                        || moonlight.state.pin.is_some()),
            )? {
                match step {
                    script::Step::Button(b) => button = Some(b),
                    script::Step::Quit => close_requested = true,
                    script::Step::Snapshot => {
                        ui.screenshot = Some(
                            std::env::var_os("ARCHR_LAUNCHER_SCREENSHOT")
                                .map(PathBuf::from)
                                .ok_or("SNAPSHOT requires ARCHR_LAUNCHER_SCREENSHOT")?,
                        );
                        dirty = true;
                    }
                    script::Step::Wait => {}
                }
            }
        }
        if let Some(event) = events.wait_event_timeout(if button.is_some() || ui.animating() {
            0
        } else {
            repeat.wait_ms(Instant::now())
        }) {
            match event {
                Event::Quit { .. } => close_requested = true,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } if windowed => close_requested = true,
                Event::JoyDeviceAdded { .. } | Event::JoyDeviceRemoved { .. } => {
                    controllers.refresh();
                    repeat.clear();
                }
                Event::Window {
                    win_event: WindowEvent::Exposed | WindowEvent::SizeChanged(..),
                    ..
                } => dirty = true,
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => repeat.clear(),
                _ => {
                    if let Some(mapped) = button_event(&event, &controllers, &mut repeat) {
                        button = Some(mapped);
                    }
                }
            }
        }
        let mut outcome = None;
        closing |= close_requested;
        if closing {
            settings.close();
            moonlight.close();
        }
        close_requested = closing && !settings.mutations_pending() && !moonlight.state.busy;
        if closing {
            button = None;
            repeat.clear();
        }
        if close_requested {
            deferred = None;
            repeat.clear();
        } else if let Some(button) = button.or_else(|| repeat.poll(Instant::now())) {
            ui.snap_animations();

            let requested = if ui.lobby.is_some() {
                lobby_input(&mut ui.lobby, &mut rooms, button)
            } else if moonlight.active {
                moonlight.handle(button)
            } else if settings.active {
                settings.handle(button)
            } else {
                app.handle(button, &library)
            };
            let requested = if requested == Outcome::Settings {
                settings.open();
                Outcome::None
            } else if requested == Outcome::Moonlight {
                moonlight.open();
                Outcome::None
            } else {
                requested
            };
            let requested = if requested == Outcome::OpenNetplay {
                ui.lobby = Some(Lobby::default());
                rooms.refresh();
                Outcome::None
            } else {
                requested
            };
            if requested == Outcome::ToggleWifi {
                if let Err(error) =
                    wifi::toggle(&paths, !ui.wifi_enabled(), ui.wifi_refresh_signal())
                {
                    log::warn!("Cannot start Wi-Fi worker: {error}");
                    ui.banner = Some(ui.catalog.text("error.operation"));
                    banner_until = Instant::now() + Duration::from_secs(8);
                }
                outcome = Some(Outcome::None);
            } else {
                let terminal = matches!(
                    requested,
                    Outcome::Launch { .. }
                        | Outcome::Netplay(_)
                        | Outcome::MoonlightStream(_)
                        | Outcome::Action(_)
                );
                if terminal
                    && (library.pending_writes > 0
                        || settings.mutations_pending()
                        || moonlight.state.busy)
                {
                    deferred = Some(requested);
                } else {
                    if terminal {
                        deferred = None;
                    }
                    outcome = Some(requested);
                }
            }
            dirty = true;
        }
        if !close_requested
            && outcome.is_none()
            && library.pending_writes == 0
            && !settings.mutations_pending()
            && !moonlight.state.busy
        {
            outcome = deferred.take().filter(|_| !library.write_failed);
        }
        if close_requested || outcome.is_some() {
            if let Some(outcome) = &outcome {
                arm_terminal_snapshot(&mut ui, &mut script, outcome)?;
            }
            if transition::apply(
                &mut ui,
                &app,
                &mut library,
                &theme,
                outcome,
                &controllers.configuration(),
                selected.as_ref().and_then(|path| covers.get(path)),
            )? {
                return Ok(());
            }
            dirty = true;
            banner_until = Instant::now() + Duration::from_secs(8);
        }
    }
}
/// Lobby input: closing and refreshing are local; a chosen room becomes a terminal outcome.
fn lobby_input(
    lobby: &mut Option<Lobby>,
    rooms: &mut netplay::Runtime,
    button: archr_launcher::state::Button,
) -> Outcome {
    match lobby.as_mut().map(|lobby| lobby.handle(button)) {
        Some(Effect::Close) => *lobby = None,
        Some(Effect::Refresh) => rooms.refresh(),
        Some(Effect::Join(join)) => return Outcome::Netplay(join),
        _ => {}
    }
    Outcome::None
}
fn cover_paths(app: &App, data: &dyn DataSource) -> (Option<PathBuf>, Vec<PathBuf>) {
    let (screen, cursor) = app.backdrop();
    let Screen::Games { system } = screen else {
        return (None, Vec::new());
    };
    let games = data.games(system);
    let selected = games.get(cursor).and_then(|game| game.image.clone());
    let mut paths = Vec::new();
    if let Some(path) = &selected {
        paths.push(path.clone());
    }
    for index in cursor.saturating_sub(2)..=cursor.saturating_add(2) {
        if let Some(path) = games.get(index).and_then(|game| game.image.clone()) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    (selected, paths)
}

fn arm_terminal_snapshot(
    ui: &mut Ui<'_, '_>,
    script: &mut Option<script::Script>,
    outcome: &Outcome,
) -> Result<(), String> {
    if matches!(
        outcome,
        Outcome::Launch { .. }
            | Outcome::Netplay(_)
            | Outcome::MoonlightStream(_)
            | Outcome::Action(_)
    ) && script
        .as_mut()
        .is_some_and(script::Script::take_terminal_snapshot)
    {
        ui.screenshot = Some(
            std::env::var_os("ARCHR_LAUNCHER_SCREENSHOT")
                .map(PathBuf::from)
                .ok_or("SNAPSHOT requires ARCHR_LAUNCHER_SCREENSHOT")?,
        );
    }
    Ok(())
}

fn init_logging() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("archr-launcher: {}", record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}
