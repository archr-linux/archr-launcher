//! SDL integration. Disk scanning and locked config updates run in workers.
mod script;
use archr_config::SystemCfg;
use archr_launcher::{
    library::Library,
    paths::Paths,
    state::{App, DataSource, Outcome, Screen},
};
use archr_ui::{
    controllers::Controllers,
    covers::CoverCache,
    input::{button_event, Repeat},
    render::Ui,
    resolve_theme,
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() {
    init_logging();
    if let Err(error) = run() {
        eprintln!("archr-launcher: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let began = Instant::now();
    let mut windowed = false;
    let mut script_source = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--windowed" => windowed = true,
            "--script" => {
                script_source = Some(
                    args.next()
                        .ok_or("--script exige uma sequência de botões")?,
                )
            }
            "--help" => {
                println!("archr-launcher [--windowed]");
                return Ok(());
            }
            _ => return Err(format!("Argumento desconhecido: {arg}")),
        }
    }
    if script_source.is_some() && !windowed {
        return Err("--script exige --windowed".to_owned());
    }
    let mut script = script_source
        .as_deref()
        .map(script::Script::parse)
        .transpose()?;
    let paths = Paths::new(std::env::var_os("ARCHR_LAUNCHER_ROOT").map(PathBuf::from));
    // No per-system index, controller enumeration or cover work before present.
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, windowed)?;
    ttf = sdl2::ttf::init().map_err(|error| error.to_string())?;
    creator = ui.canvas.texture_creator();
    let config = SystemCfg::load(&paths.config).ok();
    let name = config
        .as_ref()
        .and_then(|cfg| cfg.get("launcher", None, "theme"))
        .unwrap_or_else(|| "default".to_owned());
    let mut theme = resolve_theme(
        &paths.at("/storage/.config/archr-launcher/themes"),
        &paths.at("/usr/share/archr-launcher/themes"),
        &name,
    );
    theme.font = theme.font.as_deref().map(|font| paths.remap(font));
    ui.configure(&ttf, &creator, &theme, paths.root.as_deref())?;
    let mut library = Library::initial(paths.clone());
    let mut app = App::new(ui.page_size(&theme));
    ui.banner = library.banner.take();
    ui.draw(&app, &library, &theme, None)?;
    eprintln!(
        "archr-launcher: first frame {} ms ({} system rows)",
        began.elapsed().as_millis(),
        app.system_rows(&library).len()
    );
    library.start();
    let (width, height) = ui.canvas.output_size()?;
    let mut covers = CoverCache::new(
        &creator,
        paths.cache.join("thumbs"),
        (theme.cover.w * width as f32).round().max(1.) as u32,
        (theme.cover.h * height as f32).round().max(1.) as u32,
    );
    let mut controllers = Controllers::new(&ui.sdl)?;
    controllers.load_mappings(paths.at("/storage/.config/emulationstation/es_input.cfg"));
    let mut events = ui.sdl.event_pump()?;
    let mut repeat = Repeat::default();
    let mut dirty = false;
    let mut deferred = None;
    let mut banner_until = Instant::now() + Duration::from_secs(8);
    loop {
        controllers.poll_mappings();
        let selected_system = if app.screen == Screen::Systems {
            app.system_rows(&library).get(app.cursor).cloned()
        } else {
            None
        };
        if library.poll() {
            if let Some(name) = selected_system {
                if let Some(index) = app
                    .system_rows(&library)
                    .iter()
                    .position(|system| *system == name)
                {
                    app.cursor = index;
                }
            }
            dirty = true;
        }
        if let Some(banner) = library.banner.take() {
            ui.banner = Some(banner);
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
            let cover = selected.as_ref().and_then(|path| covers.get(path));
            ui.draw(&app, &library, &theme, cover)?;
            dirty = false;
        }
        let mut button = None;
        if let Some(driver) = &mut script {
            if let Some(step) =
                driver.poll(library.ready && !ui.loading && library.pending_writes == 0)?
            {
                match step {
                    script::Step::Button(b) => button = Some(b),
                    script::Step::Quit => return Ok(()),
                    script::Step::Snapshot => {
                        ui.screenshot = Some(
                            std::env::var_os("ARCHR_LAUNCHER_SCREENSHOT")
                                .map(PathBuf::from)
                                .ok_or("SNAPSHOT exige ARCHR_LAUNCHER_SCREENSHOT")?,
                        );
                        dirty = true;
                    }
                    script::Step::Wait => {}
                }
            }
        }
        if let Some(event) = events.wait_event_timeout(if button.is_some() {
            0
        } else {
            repeat.wait_ms(Instant::now())
        }) {
            match event {
                Event::Quit { .. } if windowed => return Ok(()),
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } if windowed => return Ok(()),
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
        button = button.or_else(|| repeat.poll(Instant::now()));
        if let Some(button) = button {
            let outcome = app.handle(button, &library);
            if matches!(outcome, Outcome::Launch { .. } | Outcome::Action(_))
                && library.pending_writes > 0
            {
                deferred = Some(outcome);
            } else {
                match library.handle(outcome, &controllers.configuration()) {
                    Ok(true) => return Ok(()),
                    Ok(false) => {}
                    Err(error) => {
                        ui.banner = Some(error);
                        banner_until = Instant::now() + Duration::from_secs(8);
                    }
                }
            }
            dirty = true;
        }
        if library.pending_writes == 0 {
            if let Some(outcome) = deferred.take() {
                if !library.write_failed && library.handle(outcome, &controllers.configuration())? {
                    return Ok(());
                }
            }
        }
    }
}
fn cover_paths(app: &App, data: &dyn DataSource) -> (Option<PathBuf>, Vec<PathBuf>) {
    let Screen::Games { system } = &app.screen else {
        return (None, Vec::new());
    };
    let games = data.games(system);
    let selected = games.get(app.cursor).and_then(|game| game.image.clone());
    let mut paths = Vec::new();
    if let Some(path) = &selected {
        paths.push(path.clone());
    }
    for index in app.cursor.saturating_sub(2)..=app.cursor.saturating_add(2) {
        if let Some(path) = games.get(index).and_then(|game| game.image.clone()) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    (selected, paths)
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
