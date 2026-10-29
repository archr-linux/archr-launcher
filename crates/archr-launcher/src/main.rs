//! SDL integration. Disk scanning and locked config updates run in workers.
mod script;
mod transition;
use archr_config::SystemCfg;
use archr_launcher::{
    appearance,
    audio::{self, Audio},
    cheevos::Lookup,
    library::Library,
    modules::{self, Modules, Notice},
    netplay,
    paths::Paths,
    preview::Preview,
    rescrape::Rescrape,
    state::{App, DataSource, Outcome, Screen},
    wifi,
};
use archr_ui::{
    controllers::Controllers,
    covers::CoverCache,
    i18n::I18n,
    input::{button_event, Repeat},
    netplay::{Effect, Lobby},
    render::{prepare_first_frame, Art, Ui},
    settings::Id,
    sounds::{Event as Sound, Sounds},
    startup::StartupTrace,
    startup_phase,
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
/// "Check at startup" waits this long, so it never slows the first screens.
const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(20);

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
                println!("archr-launcher [--windowed] [--check-module <dir>]");
                return Ok(());
            }
            "--check-module" => {
                let dir = PathBuf::from(args.next().ok_or("--check-module requires a folder")?);
                return match modules::check_module(&dir) {
                    Ok(warnings) => {
                        warnings.iter().for_each(|w| println!("warning: {w}"));
                        println!("{}: ok", dir.display());
                        Ok(())
                    }
                    Err(errors) => Err(errors.join("\n")),
                };
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
    #[allow(clippy::type_complexity)]
    let (
        mut ui,
        (
            mut theme,
            (base, look),
            theme_path,
            catalog,
            mut library,
            prepared,
            (netplay_on, cheevos_on, mangohud_on),
        ),
    ) = std::thread::scope(|scope| {
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
            let look = appearance::saved(config.as_ref());
            let (base, theme_path) = appearance::load_theme(paths, &look.theme);
            let theme = look.apply(&base);
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
            let setting = |key: &str| {
                config
                    .as_ref()
                    .and_then(|cfg| cfg.get_setting(key, None, None))
            };
            let netplay_on = setting("netplay").is_some_and(|value| value == "1");
            let cheevos_on = setting(Id::Cheevos.key()).is_some_and(|value| value == "1")
                && setting(archr_launcher::settings::CHEEVOS_TOKEN).is_some();
            let mangohud_on = archr_launcher::settings::mangohud_supported(paths);
            Ok((
                theme,
                (base, look),
                theme_path,
                catalog,
                library,
                prepared,
                (netplay_on, cheevos_on, mangohud_on),
            ))
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
    ui.default_theme_dir = Some(paths.at(archr_ui::DEFAULT_THEME_DIR));
    ui.text_scale = look.text_scale();
    ui.set_motion(look.animations);
    ui.start_opening();
    ui.configure_prepared(&creator, prepared, paths.root.as_deref())?;
    let mut app = App::new(1);
    app.set_view(&theme.view(&theme.active_view));
    app.netplay = netplay_on;
    app.cheevos = cheevos_on;
    app.mangohud = mangohud_on;
    ui.banner = library.banner.take().map(|m| m.render(&ui.catalog));
    ui.draw(&app, &library, &theme, None)?;
    eprintln!(
        "archr-launcher: first frame {} ms ({} system rows)",
        began.elapsed().as_millis(),
        App::system_rows(&library).len()
    );
    library.start();
    // Modules start after the first frame; daemons two seconds later.
    let mut modules = Modules::start(modules::Config::production(&paths, ui.catalog.language()));
    let mut module_focus = None;
    let startup_config = SystemCfg::load(&paths.config).ok();
    let (sounds_on, sounds_volume) = audio::saved(startup_config.as_ref());
    let mut audio = Audio::new(
        theme_sounds(ui.theme_dir.as_deref(), &paths),
        sounds_on,
        sounds_volume,
    );
    audio.play(Sound::Boot);
    let mut seen_errors = 0;
    let (width, height) = ui.canvas.output_size()?;
    let mut settings = archr_launcher::settings::Runtime::new(paths.clone());
    let saved = |id: Id| {
        startup_config
            .as_ref()
            .and_then(|cfg| cfg.get_setting(id.key(), None, None))
            .unwrap_or_else(|| id.default_value().into())
    };
    // Read before Settings first opens: the details screen needs them now.
    for id in [Id::UpdateNotify, Id::PreviewVideo] {
        settings.state.values.insert(id, saved(id));
    }
    let mut update_check =
        (saved(Id::UpdateBoot) == "1").then(|| Instant::now() + UPDATE_CHECK_DELAY);
    // An update still running from before (the launcher restarted): its screen.
    settings.watch_update();
    settings.state.base_theme = base;
    settings.state.base_dir = ui.theme_dir.clone();
    let mut rooms = netplay::Runtime::new(paths.clone());
    let mut was_browsing = true;
    let mut moonlight = archr_launcher::moonlight::Runtime::new(paths.clone(), (width, height));
    let mut cover_box = cover_size(&theme, (width, height));
    let mut covers = CoverCache::new(
        &creator,
        paths.cache.join("thumbs"),
        cover_box.0,
        cover_box.1,
    );
    // Details art: the screen shot box and the logo title, at their own sizes.
    let sized = |rect: archr_ui::theme::Rect01| {
        let w = (rect.w * width as f32).round().max(1.) as u32;
        (w, (rect.h * height as f32).round().max(1.) as u32)
    };
    let (shot_w, shot_h) = sized(theme.details_shot);
    let (logo_w, logo_h) = sized(theme.details_title);
    let mut shots = CoverCache::new(&creator, paths.cache.join("thumbs"), shot_w, shot_h);
    let mut logos = CoverCache::new(&creator, paths.cache.join("thumbs"), logo_w, logo_h);
    // Achievement badges, at the list icon's size.
    let side = (36. * height as f32 / 480.).round().max(1.) as u32;
    let mut badges = CoverCache::new(&creator, paths.cache.join("thumbs"), side, side);
    let mut preview = Preview::new(&creator);
    let mut cheevos: Option<Lookup> = None;
    let mut rescrape: Option<Rescrape> = None;
    // The ROM on the details screen, followed across re-reads of its system.
    let mut details_rom: Option<PathBuf> = None;
    let mut controllers = Controllers::new_traced(&ui.sdl, trace)?;
    let buttons = startup_config
        .as_ref()
        .and_then(|cfg| cfg.get_setting(Id::Buttons.key(), None, None))
        .unwrap_or_default();
    controllers.set_remap(archr_ui::pad::Remap::saved(&buttons));
    ui.face_letters = archr_ui::pad::face_letters(&archr_ui::pad::bindings(&buttons));
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
        if !settings.state.loading {
            controllers.set_remap(settings.state.remap());
            ui.face_letters = archr_ui::pad::face_letters(&settings.state.bindings());
            audio.configure(
                settings.state.value(Id::Sounds) == "1",
                settings.state.value(Id::SoundsVolume).parse().unwrap_or(60),
            );
        }
        if settings.state.error_seq != seen_errors {
            seen_errors = settings.state.error_seq;
            audio.play(Sound::Error);
        }
        if settings.take_launch_saved() {
            library.reload_config();
        }
        if let Some(look) = settings.state.applied.take() {
            let base = &settings.state.base_theme;
            let next = look.apply(base);
            // Fonts and system art load with the theme; colors apply at once.
            let reload = next.font != theme.font
                || next.font_semibold != theme.font_semibold
                || settings.state.base_dir != ui.theme_dir;
            theme = next;
            app.set_view(&theme.view(&theme.active_view));
            ui.theme_dir = settings.state.base_dir.clone();
            audio.set_sounds(theme_sounds(ui.theme_dir.as_deref(), &paths));
            ui.text_scale = look.text_scale();
            ui.set_motion(look.animations);
            if reload {
                ui.configure(&ttf, &creator, &theme, paths.root.as_deref())?;
            }
            // Another view may show covers at another size.
            if cover_size(&theme, (width, height)) != cover_box {
                cover_box = cover_size(&theme, (width, height));
                covers = CoverCache::new(
                    &creator,
                    paths.cache.join("thumbs"),
                    cover_box.0,
                    cover_box.1,
                );
            }
            dirty = true;
        }
        if let Some(systems) = settings.take_scraped() {
            library.reload(&systems);
            covers.clear();
            dirty = true;
        }
        dirty |= moonlight.poll();
        if update_check.is_some_and(|at| Instant::now() >= at) {
            update_check = None;
            // Unless the Updates page already looked meanwhile.
            if settings.state.updates.checked.is_none() {
                settings.check_updates();
            }
        }
        if settings.take_reattached() {
            settings.open_updates();
            dirty = true;
        }
        let updates = if settings.state.value(Id::UpdateNotify) == "1" {
            settings.state.updates.count()
        } else {
            0
        };
        dirty |= app.updates != updates;
        app.updates = updates;
        for event in modules.poll() {
            match event {
                modules::Event::View(view) => {
                    app.modules = view.clone();
                    settings.state.set_modules(view);
                }
                modules::Event::Store(reply) => settings.state.module_store_reply(reply),
                modules::Event::Notice(notice) => {
                    let (key, name) = match notice {
                        Notice::Disabled(name) => ("modules.disabled_notice", name),
                        Notice::RolledBack(name) => ("modules.rolled_back", name),
                        Notice::TooMany(name) => ("modules.too_many", name),
                        Notice::Text(text) => ("", text),
                    };
                    ui.banner = Some(if key.is_empty() {
                        name
                    } else {
                        ui.catalog.format(key, &[("name", &name)])
                    });
                    banner_until = Instant::now() + Duration::from_secs(8);
                    audio.play(Sound::Notice);
                }
            }
            dirty = true;
        }
        if let Some(language) = &language {
            modules.language(language);
        }
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
        audio.focus(
            browsing.then_some(selected_system.as_deref()).flatten(),
            Instant::now(),
        );
        // What the user rests on, for modules (debounced by the host).
        let focus = match &app.screen {
            _ if settings.active => None,
            Screen::Systems => selected_system.clone().map(|s| (s, None)),
            Screen::Games { system } => library.origin(system, app.cursor).map(|(real, game)| {
                let key = library.game_key(&real, game).unwrap_or_default();
                let name = library.games(&real).get(game).map(|g| g.name.clone());
                (real, Some((key, name.unwrap_or_default())))
            }),
            _ => None,
        };
        if focus.is_some() && focus != module_focus {
            if let Some((system, game)) = &focus {
                modules.focus(system, game.as_ref().map(|(k, n)| (k.as_str(), n.as_str())));
            }
            module_focus = focus;
        }
        audio.poll();
        if library.poll() {
            if let Some(name) = selected_system {
                if let Some(index) = app.rows(&library).iter().position(|system| *system == name) {
                    app.cursor = index;
                }
            }
            dirty = true;
        }
        if let Screen::Details { system, game } = app.backdrop().0.clone() {
            library.ensure(&system);
            let games = library.games(&system);
            match (&details_rom, games.get(game)) {
                (Some(rom), Some(shown)) if shown.path != *rom => {
                    if let Some(index) = games.iter().position(|g| g.path == *rom) {
                        app.retarget_details(index);
                    }
                }
                (None, Some(shown)) => details_rom = Some(shown.path.clone()),
                _ => {}
            }
        } else {
            details_rom = None;
        }
        if let Some(key) = rescrape.as_mut().and_then(Rescrape::poll) {
            let system = rescrape.take().map(|r| r.system).unwrap_or_default();
            library.reload(&[system]);
            covers.clear();
            shots.clear();
            logos.clear();
            ui.banner = Some(ui.catalog.text(key));
            banner_until = Instant::now() + Duration::from_secs(8);
            dirty = true;
        }
        if !matches!(app.screen, Screen::Achievements { .. }) {
            cheevos = None;
        }
        if let Some(sheet) = cheevos.as_mut().and_then(Lookup::poll) {
            app.sheet = sheet;
            dirty = true;
        }
        // The game menu offers achievements once settings know the account.
        if !settings.state.loading {
            app.cheevos =
                settings.state.value(Id::Cheevos) == "1" && !settings.state.cheevos.user.is_empty();
            app.mangohud = settings.state.mangohud_supported;
        }
        app.rescraping = rescrape.is_some();
        app.can_rescrape = archr_scraper::available() && !settings.state.scrape.running;
        if let Some(banner) = library.banner.take() {
            audio.play(if banner.key.starts_with("error.") {
                Sound::Error
            } else {
                Sound::Notice
            });
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
        let (shot, logo) = details_art(&app, &library);
        shots.request(shot.as_slice());
        logos.request(logo.as_slice());
        badges.request(&badge_paths(&app));
        dirty |= covers.poll() | shots.poll() | logos.poll() | badges.poll();
        let video = match &app.screen {
            Screen::Details { system, game }
                if !settings.active && settings.state.value(Id::PreviewVideo) == "1" =>
            {
                let path = library
                    .games(system)
                    .get(*game)
                    .and_then(|g| g.video.clone());
                // The art box inside its 1px frame.
                let (w, h) = sized(theme.details_shot);
                path.map(|path| {
                    (
                        path,
                        (w.saturating_sub(2).max(1), h.saturating_sub(2).max(1)),
                    )
                })
            }
            _ => None,
        };
        dirty |= preview.poll(video, Instant::now());
        if dirty {
            ui.moonlight = moonlight.active.then(|| moonlight.state.clone());
            let art = Art {
                cover: selected.as_ref().and_then(|path| covers.get(path)),
                shot: shot.as_ref().and_then(|path| shots.get(path)),
                logo: logo.as_ref().and_then(|path| logos.get(path)),
                video: preview.frame(),
                badges: Some(&badges),
            };
            ui.draw_with_settings(
                &app,
                &library,
                &theme,
                art,
                settings.active.then_some(&settings.state),
            )?;
            app.details_overflow = ui.details_overflow;
            app.sheet_page = ui.achievements_page;
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
            let now = Instant::now();
            repeat
                .wait_ms(now)
                .min(audio.wait_ms(now))
                .min(preview.wait_ms(now))
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
                // An open "press a button" prompt takes the next press itself.
                _ if settings.active && settings.state.capturing() => {
                    if let Some(pad) = controllers.pressed(&event) {
                        settings.capture(pad);
                        dirty = true;
                    }
                }
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
            audio.wake();
            let before = (app.screen.clone(), app.cursor);
            let mark = settings
                .active
                .then(|| audio::Mark::of(&settings.state, true));
            let overlay = ui.lobby.is_some() || moonlight.active;
            let requested = if ui.lobby.is_some() {
                lobby_input(&mut ui.lobby, &mut rooms, button)
            } else if moonlight.active {
                moonlight.handle(button)
            } else if settings.active {
                settings.handle(button)
            } else {
                app.handle(button, &library)
            };
            let cue = if overlay {
                audio::by_button(button)
            } else if let Some(before) = &mark {
                let after = audio::Mark::of(&settings.state, settings.active);
                audio::settings(button, before, &after)
            } else {
                audio::browse(
                    button,
                    (&before.0, before.1),
                    (&app.screen, app.cursor),
                    &requested,
                )
            };
            if let Some(cue) = cue {
                audio.play(cue);
            }
            let requested = if requested == Outcome::Settings {
                // The Games page opens on the system being browsed.
                let system = match &app.screen {
                    Screen::Games { system } => Some(system.clone()),
                    Screen::Systems => app.rows(&library).get(app.cursor).cloned(),
                    _ => None,
                };
                if let Some(system) = system {
                    settings.focus(&system);
                }
                settings.open();
                Outcome::None
            } else if requested == Outcome::Updates {
                settings.open_updates();
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
            if let Outcome::Achievements { system, game } = &requested {
                cheevos = library
                    .games(system)
                    .get(*game)
                    .map(|rom| Lookup::start(paths.clone(), system.clone(), rom.path.clone()));
                outcome = Some(Outcome::None);
            } else if let Outcome::Rescrape { system, game } = &requested {
                if let Some(rom) = library
                    .games(system)
                    .get(*game)
                    .filter(|_| rescrape.is_none())
                {
                    rescrape = Some(Rescrape::start(
                        paths.clone(),
                        system.clone(),
                        rom.path.clone(),
                    ));
                    app.rescraping = true;
                }
                outcome = Some(Outcome::None);
            } else if let Outcome::Module(request) = &requested {
                match request {
                    archr_ui::modules::Request::Open { module, page } => {
                        settings.open();
                        settings.state.open_module_page(module, *page);
                    }
                    request => modules.request(request.clone()),
                }
                outcome = Some(Outcome::None);
            } else if requested == Outcome::ToggleWifi {
                if let Err(error) =
                    wifi::toggle(&paths, !ui.wifi_enabled(), ui.wifi_refresh_signal())
                {
                    log::warn!("Cannot start Wi-Fi worker: {error}");
                    ui.banner = Some(ui.catalog.text("error.operation"));
                    banner_until = Instant::now() + Duration::from_secs(8);
                }
                outcome = Some(Outcome::None);
            } else {
                let terminal = transition::is_terminal(&requested);
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
            let banner = ui.banner.clone();
            // No module runs while a game has the screen.
            let game = match &outcome {
                Some(Outcome::Launch { system, game }) => Some((system.clone(), *game)),
                Some(Outcome::Netplay(_) | Outcome::MoonlightStream(_)) => Some((String::new(), 0)),
                _ => None,
            };
            // No preview outlives the launcher's screen.
            if close_requested || outcome.as_ref().is_some_and(transition::is_terminal) {
                preview.stop();
            }
            if let Some((system, index)) = &game {
                let key = library.game_key(system, *index).unwrap_or_default();
                let name = library.games(system).get(*index).map(|g| g.name.clone());
                modules.hand_off(system, &key, &name.unwrap_or_default());
            }
            if transition::apply(
                &mut ui,
                &mut audio,
                &app,
                &mut library,
                &theme,
                outcome,
                &controllers.configuration(),
                selected.as_ref().and_then(|path| covers.get(path)),
            )? {
                return Ok(());
            }
            if game.is_some() {
                modules.resume();
            }
            dirty = true;
            banner_until = Instant::now() + Duration::from_secs(8);
            // Back from a failed hand-off (or a suspend): the output returns.
            audio.wake();
            if ui.banner != banner {
                audio.play(Sound::Error);
            }
        }
    }
}

/// The sounds of the theme in `dir`, over the installed default theme's.
fn theme_sounds(dir: Option<&std::path::Path>, paths: &Paths) -> Sounds {
    let default = paths.at(archr_ui::DEFAULT_THEME_DIR);
    let base = Sounds::load(&default);
    match dir {
        Some(dir) if dir != default => Sounds::load_over(dir, &base),
        _ => base,
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
/// Screen shot and logo of the game on the details screen, if any.
fn details_art(app: &App, data: &dyn DataSource) -> (Option<PathBuf>, Option<PathBuf>) {
    let Screen::Details { system, game } = app.backdrop().0 else {
        return (None, None);
    };
    data.games(system).get(*game).map_or((None, None), |game| {
        (game.details.screenshot.clone(), game.details.logo.clone())
    })
}

/// Badges of the achievements on the page the list shows.
fn badge_paths(app: &App) -> Vec<PathBuf> {
    if !matches!(app.screen, Screen::Achievements { .. }) {
        return Vec::new();
    }
    let page = app.sheet_page.max(1);
    let first = app.cursor / page * page;
    app.sheet
        .items
        .iter()
        .skip(first)
        .take(page)
        .filter_map(|item| item.badge.clone())
        .collect()
}

/// Pixel size of the games screen's cover in the active view.
fn cover_size(theme: &archr_ui::Theme, (width, height): (u32, u32)) -> (u32, u32) {
    let view = theme.view(&theme.active_view);
    let rect = view.games.art.map_or(theme.cover, |art| art.rect);
    (
        (rect.w * width as f32).round().max(1.) as u32,
        (rect.h * height as f32).round().max(1.) as u32,
    )
}
fn cover_paths(app: &App, data: &dyn DataSource) -> (Option<PathBuf>, Vec<PathBuf>) {
    let (screen, cursor) = app.backdrop();
    if let Screen::Details { system, game } = screen {
        let image = data.games(system).get(*game).and_then(|g| g.image.clone());
        return (image.clone(), image.into_iter().collect());
    }
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
    if transition::is_terminal(outcome)
        && script
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
