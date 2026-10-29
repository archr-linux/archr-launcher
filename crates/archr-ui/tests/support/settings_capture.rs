use super::*;
use crate::{appearance::Appearance, settings::network::Network, state::Button};

fn network(ssid: &str, security: &str, signal: u8, connected: bool) -> Network {
    Network {
        ssid: ssid.into(),
        security: security.into(),
        signal,
        connected,
        known: connected,
    }
}
fn ready() -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.access_ip = "192.168.3.10".into();
    state.values.insert(Id::WifiSwitch, "1".into());
    state
}
/// Every page this branch draws, in the shipped theme fonts.
fn pages() -> Vec<(&'static str, Settings)> {
    let now = Instant::now();
    let mut wifi = ready();
    wifi.open(Page::Wifi);
    wifi.networks = vec![
        network("Casa_5G", "psk", 92, true),
        network("Casa", "psk", 70, false),
        network("Vizinho", "psk", 40, false),
        network("Café", "open", 12, false),
    ];
    wifi.scanned = now.checked_sub(Duration::from_secs(5));
    wifi.cursor = 1;
    let mut wifi_searching = wifi.clone();
    wifi_searching.scanning = true;
    let mut wifi_empty = wifi_searching.clone();
    wifi_empty.networks.truncate(1);
    wifi_empty.cursor = 0;
    let mut password = wifi.clone();
    password.networks[0].connected = false;
    password.handle(Button::A);
    for button in [Button::A; 8] {
        password.handle(button);
    }
    for button in [Button::Down, Button::R, Button::R, Button::R, Button::R] {
        password.handle(button);
    }
    let mut forget = wifi.clone();
    forget.confirm_forget = Some("Casa_5G".into());
    let mut connecting = wifi.clone();
    connecting.connecting = Some("Casa".into());
    let mut bluetooth = ready();
    bluetooth.values.insert(Id::BtSwitch, "1".into());
    bluetooth.bt = crate::settings::bluetooth::parse_devices(
        "<device id=\"00:1A:2B:3C:4D:5E\" name=\"JBL Go 3\" type=\"\" connected=\"no\" />\n\
         <device id=\"11:22:33:44:55:66\" name=\"8BitDo SN30 Pro\" type=\"joystick\" connected=\"yes\" />",
        "<device id=\"AA:BB:CC:DD:EE:01\" name=\"Xbox Wireless Controller\" status=\"added\" type=\"joystick\" />",
    );
    bluetooth.open(Page::Wifi);
    bluetooth.open(Page::Bluetooth);
    bluetooth.bt_scanned = now.checked_sub(Duration::from_secs(5));
    bluetooth.cursor = 1;
    let mut bt_searching = bluetooth.clone();
    bt_searching.bt_scanning = true;
    let mut bt_forget = bluetooth.clone();
    bt_forget.cursor = 2;
    bt_forget.confirm_forget = Some("00:1A:2B:3C:4D:5E".into());
    let mut bt_pairing = bluetooth.clone();
    bt_pairing.connecting = Some("Xbox Wireless Controller".into());
    let mut bt_off = ready();
    bt_off.open(Page::Wifi);
    bt_off.open(Page::Bluetooth);
    let mut services = ready();
    services.open(Page::Wifi);
    services.open(Page::Services);
    for (id, value) in [
        (Id::Ssh, "1"),
        (Id::Samba, "1"),
        (Id::Player, "2"),
        (Id::UsbMode, "network"),
    ] {
        services.values.insert(id, value.into());
    }
    services.cursor = 1;
    let mut region = ready();
    region.open(Page::Region);
    region.values.insert(Id::Language, "pt_BR".into());
    region.values.insert(Id::KeyboardLayout, "br".into());
    region.values.insert(Id::KeyboardVariant, "abnt2".into());
    region.labels.insert(
        (Id::KeyboardLayout, "br".into()),
        "Portuguese (Brazil)".into(),
    );
    region
        .labels
        .insert((Id::KeyboardVariant, "abnt2".into()), "ABNT2".into());
    region.cursor = 2;
    let mut netplay = ready();
    netplay.open(Page::Games);
    netplay.open(Page::Netplay);
    netplay.values.insert(Id::Nickname, "douglas".into());
    netplay
        .values
        .insert(Id::NetplayHost, "192.168.3.20".into());
    netplay.values.insert(Id::Relay, "saopaulo".into());
    netplay.cursor = 4;
    let mut scraper = ready();
    scraper.open(Page::Games);
    scraper.open(Page::Scraper);
    scraper.scrape = crate::settings::Scrape {
        available: true,
        user: "douglas".into(),
        threads: 1,
        done: 612,
        total: 612,
        scraped: 563,
        missing: 49,
        outcome: Some("settings.scrape_done"),
        ..Default::default()
    };
    scraper.cursor = 5;
    let mut running = scraper.clone();
    running.scrape.running = true;
    running.scrape.outcome = None;
    running.scrape.done = 140;
    running.scrape.system = "Super Nintendo".into();
    let mut media = scraper.clone();
    media.open(Page::ScraperMedia);
    media.cursor = 2;
    let mut media_all = media.clone();
    media_all.values.insert(
        Id::ScraperMedia,
        "box,box3d,mix,screenshot,title,wheel,video,manual".into(),
    );
    media_all.cursor = 6;
    let mut scraper_all = scraper.clone();
    scraper_all.values = media_all.values.clone();
    scraper_all.cursor = 3;
    use crate::settings::storage::{Card, Confirm};
    let mut storage = ready();
    storage.open(Page::Storage);
    storage.values.insert(Id::Automount, "1".into());
    storage.storage.card = Some(Card {
        external: true,
        name: "settings.card_external|label=ROMS".into(),
        detail: "settings.card_detail|fs=ext4|used=@41.2 GB|total=@119.0 GB".into(),
        percent: 35,
    });
    storage.storage.backup = Some("settings.backup_today|time=14:42".into());
    storage.storage.merge_supported = true;
    storage.cursor = 3;
    let mut storage_end = storage.clone();
    storage_end.cursor = 6;
    storage_end.storage.password = "Pl4yR00m".into();
    let mut storage_wifi = storage.clone();
    storage_wifi.storage.confirm = Some(Confirm::BackupWifi);
    let mut storage_backup = storage.clone();
    storage_backup.storage.confirm = Some(Confirm::Backup);
    let mut storage_reset = storage_end.clone();
    storage_reset.storage.confirm = Some(Confirm::ResetFinal);
    let mut storage_working = storage.clone();
    storage_working.storage.working = Some("settings.backing_up");
    let mut storage_password = storage.clone();
    storage_password.cursor = 5;
    storage_password.handle(Button::A);
    for button in [Button::A; 6] {
        storage_password.handle(button);
    }
    let mut storage_internal = storage.clone();
    storage_internal.storage.card = Some(Card {
        external: false,
        name: "settings.card_internal".into(),
        detail: "settings.card_detail|fs=ext4|used=@4.1 GB|total=@25.3 GB".into(),
        percent: 16,
    });
    storage_internal.storage.backup = None;
    storage_internal.storage.merge_supported = false;
    storage_internal.cursor = 0;
    let mut unavailable = ready();
    unavailable.open(Page::Games);
    unavailable.open(Page::Scraper);
    let mut games = ready();
    let pair = |emulator: &str, core: &str, options: &[Id]| crate::settings::GameCore {
        emulator: emulator.into(),
        core: core.into(),
        default: false,
        options: options.to_vec(),
    };
    let options = [Id::Ratio, Id::Filter, Id::Autosave, Id::StateSlot];
    games.games.replace(vec![crate::settings::GameSystem {
        name: "snes".into(),
        fullname: "Super Nintendo".into(),
        cores: vec![
            pair("retroarch", "snes9x", &options),
            pair("retroarch", "bsnes", &options),
            pair("snes9x-sa", "snes9x-sa", &[]),
        ],
        saved: [(Id::Autosave, "1".to_owned()), (Id::StateSlot, "1".into())].into(),
    }]);
    games.sync_games();
    games.open(Page::Games);
    let mut standalone = games.clone();
    standalone.complete(Id::Emulator, Ok("snes9x-sa".into()));
    standalone.cursor = 3;
    let mut sound = ready();
    sound.open(Page::Sound);
    let sink = "Built-in Audio Internal Speaker".to_owned();
    sound.options.insert(Id::Audio, vec![sink.clone()]);
    sound.values.insert(Id::Audio, sink);
    sound.handle(Button::R);
    sound.handle(Button::R);
    assert!(!sound.slides.is_empty() && sound.pending.is_none());
    let mut appearance = ready();
    appearance.values.insert(Id::Accent, "#FF6B4A".into());
    appearance.open(Page::Appearance);
    appearance.cursor = 3;
    let mut custom = appearance.clone();
    custom.handle(Button::A);
    assert_eq!(custom.page(), Page::AccentColor);
    let mut custom_value = custom.clone();
    custom_value.cursor = 2;
    let mut custom_hex = custom.clone();
    custom_hex.cursor = 3;
    let mut hex_keyboard = custom_hex.clone();
    hex_keyboard.handle(Button::A);
    assert_eq!(hex_keyboard.page(), Page::Keyboard);
    // The whole interface in a very light accent, as after saving it.
    let mut applied_light = ready();
    applied_light.values.insert(Id::Accent, "#FFE14D".into());
    applied_light.open(Page::Appearance);
    applied_light.cursor = 2;
    let mut applied_light_custom = applied_light.clone();
    applied_light_custom.handle(Button::A);
    applied_light_custom.cursor = 1;
    let mut mangohud = ready();
    mangohud.mangohud_supported = true;
    mangohud.values.insert(Id::Mangohud, "1".into());
    mangohud.open(Page::Games);
    mangohud.open(Page::Mangohud);
    mangohud.cursor = 2;
    let mut mangohud_items = mangohud.clone();
    mangohud_items
        .values
        .insert(Id::MangohudLayout, "detailed".into());
    mangohud_items.open(Page::MangohudItems);
    mangohud_items.cursor = 6;
    let mut mangohud_wide = mangohud.clone();
    for (id, value) in [
        (Id::MangohudLayout, "horizontal"),
        (Id::MangohudPosition, "bottom-center"),
        (Id::MangohudSize, "30"),
        (Id::MangohudAlpha, "100"),
        (Id::MangohudFps, "60"),
    ] {
        mangohud_wide.values.insert(id, value.into());
    }
    mangohud_wide.cursor = 4;
    let mut mangohud_custom = mangohud.clone();
    for (id, value) in [
        (Id::MangohudLayout, "custom"),
        (Id::MangohudItems, "fps,frametime,ram,battery,time,game"),
        (Id::MangohudPosition, "top-right"),
        (Id::MangohudSize, "18"),
        (Id::MangohudAlpha, "0"),
    ] {
        mangohud_custom.values.insert(id, value.into());
    }
    mangohud_custom.cursor = 3;
    let mut mangohud_off = mangohud.clone();
    mangohud_off.mangohud_supported = false;
    mangohud_off.values.insert(Id::Mangohud, "0".into());
    mangohud_off.cursor = 0;
    let mut games_mangohud = games.clone();
    games_mangohud.mangohud_supported = true;
    games_mangohud.cursor = 6;
    let mut controls = ready();
    controls.open(Page::Controls);
    let mut launcher_controls = controls.clone();
    launcher_controls.open(Page::LauncherControls);
    launcher_controls
        .values
        .insert(Id::Buttons, "B,A,X,Y,L1,R1,START".into());
    launcher_controls.cursor = 2;
    let mut launcher_press = launcher_controls.clone();
    launcher_press.cursor = 3;
    launcher_press.handle(Button::A);
    let mut hotkeys = controls.clone();
    hotkeys
        .values
        .extend(crate::settings::hotkeys::hotkey_values(|key| {
            (key == "key.dpad.events").then(|| "1".into())
        }));
    hotkeys.values.insert(Id::HotkeyOn(2), "0".into());
    hotkeys.open(Page::Hotkeys);
    hotkeys.cursor = 2;
    let mut hotkey_press = hotkeys.clone();
    hotkey_press.cursor = 4;
    hotkey_press.handle(Button::A);
    hotkey_press.captured(crate::pad::Pad::R1);
    let mut hotkey_conflict = hotkeys.clone();
    hotkey_conflict.handle(Button::A);
    hotkey_conflict.captured(crate::pad::Pad::A);
    let mut hotkey_modifier = hotkeys.clone();
    hotkey_modifier.cursor = 0;
    vec![
        // First, so the list snaps to its scrolled end.
        ("storage-end", storage_end),
        ("sound-sliding", sound),
        ("wifi", wifi),
        ("wifi-searching-a", wifi_searching.clone()),
        ("wifi-searching-b", wifi_searching),
        ("wifi-empty-a", wifi_empty.clone()),
        ("wifi-empty-b", wifi_empty),
        ("bluetooth-searching-a", bt_searching.clone()),
        ("bluetooth-searching-b", bt_searching),
        ("password", password),
        ("forget", forget),
        ("connecting", connecting),
        ("bluetooth", bluetooth),
        ("bluetooth-forget", bt_forget),
        ("bluetooth-pairing", bt_pairing),
        ("bluetooth-off", bt_off),
        ("services", services),
        ("region", region),
        ("games", games),
        ("games-standalone", standalone),
        ("netplay", netplay),
        ("scraper", scraper),
        ("scraper-running", running),
        ("scraper-unavailable", unavailable),
        ("scraper-media", media),
        ("scraper-media-all", media_all),
        ("scraper-all-media", scraper_all),
        ("storage", storage),
        ("storage-internal", storage_internal),
        ("storage-backup", storage_backup),
        ("storage-backup-wifi", storage_wifi),
        ("storage-reset-final", storage_reset),
        ("storage-working", storage_working),
        ("storage-password", storage_password),
        ("appearance-custom", appearance),
        ("custom", custom),
        ("custom-value", custom_value),
        ("custom-hex", custom_hex),
        ("custom-keyboard", hex_keyboard),
        ("applied-appearance", applied_light),
        ("applied-custom", applied_light_custom),
        ("mangohud", mangohud),
        ("mangohud-items", mangohud_items),
        ("mangohud-wide", mangohud_wide),
        ("mangohud-custom", mangohud_custom),
        ("mangohud-unsupported", mangohud_off),
        ("games-mangohud", games_mangohud),
        ("controls", controls),
        ("controls-launcher", launcher_controls),
        ("controls-launcher-press", launcher_press),
        ("controls-hotkeys", hotkeys),
        ("controls-hotkeys-modifier", hotkey_modifier),
        ("controls-hotkeys-press", hotkey_press),
        ("controls-hotkeys-conflict", hotkey_conflict),
    ]
}
#[test]
#[ignore = "manual capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_network_and_games_pages() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_SETTINGS_CAPTURE_DIR").expect("capture directory"),
    );
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::load(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"),
    );
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let save = |ui: &mut Ui, name: String| {
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        image::save_buffer(
            output.join(name),
            &pixels,
            640,
            480,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let languages =
        std::env::var("ARCHR_SETTINGS_CAPTURE_LANGUAGES").unwrap_or_else(|_| "pt_BR,ru".into());
    for language in languages.split(',') {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in pages() {
            ui.canvas.set_draw_color(color(theme.background));
            ui.canvas.clear();
            let look = if name.starts_with("applied-") {
                state.appearance().apply(&theme)
            } else {
                theme.clone()
            };
            // The search dots are drawn at two points of their loop.
            ui.frame.now =
                ui.born + Duration::from_millis(if name.ends_with("-b") { 840 } else { 480 });
            // Twice: dialogs fade in, so the second frame shows them settled.
            for _ in 0..2 {
                ui.canvas.set_draw_color(color(look.background));
                ui.canvas.clear();
                ui.settings_page(&state, &look).unwrap();
                ui.snap_animations();
            }
            save(&mut ui, format!("{name}-{language}.png"));
        }
        struct Home(Vec<archr_index::cache::SystemEntry>);
        impl crate::state::DataSource for Home {
            fn systems(&self) -> &[archr_index::cache::SystemEntry] {
                &self.0
            }
            fn games(&self, _: &str) -> &[archr_index::Game] {
                &[]
            }
            fn origin(&self, _: &str, _: usize) -> Option<(String, usize)> {
                None
            }
            fn cores(&self, _: &str) -> Vec<(String, String)> {
                Vec::new()
            }
            fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
                None
            }
        }
        let systems = [
            ("gba", "Game Boy Advance", 146),
            ("snes", "Super Nintendo", 213),
            ("megadrive", "Mega Drive", 98),
            ("n64", "Nintendo 64", 41),
        ]
        .map(
            |(name, fullname, game_count)| archr_index::cache::SystemEntry {
                name: name.into(),
                fullname: fullname.into(),
                game_count,
                dir_mtime: 0,
                source_fingerprint: 0,
            },
        );
        let home = Home(systems.to_vec());
        let app = crate::state::App::new(10);
        for (name, accent) in [
            ("light", "#FFE14D"),
            ("dark", "#1B2A6B"),
            ("coral", "#FF6B4A"),
        ] {
            let look = Appearance {
                accent: accent.into(),
                ..Appearance::default()
            }
            .apply(&theme);
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw(&app, &home, &look, None).unwrap();
            }
            save(&mut ui, format!("home-{name}-{language}.png"));
        }
        let mut moonlight = crate::moonlight::State::new((640, 480));
        moonlight.busy = false;
        moonlight.keyboard = Some(crate::keyboard::Keyboard::new(
            crate::keyboard::Mode::Ipv4,
            "192.168.3.2".into(),
        ));
        ui.canvas.set_draw_color(color(theme.background));
        ui.canvas.clear();
        ui.moonlight_page(&moonlight, &theme).unwrap();
        save(&mut ui, format!("moonlight-keyboard-{language}.png"));
    }
}

/// A small library for the browsing screens.
struct Shelf {
    systems: Vec<archr_index::cache::SystemEntry>,
    games: Vec<archr_index::Game>,
}
impl Shelf {
    fn new() -> Self {
        let systems = [
            ("gba", "Game Boy Advance", 146),
            ("snes", "Super Nintendo", 213),
            ("megadrive", "Mega Drive", 98),
            ("n64", "Nintendo 64", 41),
            ("psx", "PlayStation", 64),
            ("nes", "Nintendo Entertainment System", 120),
        ]
        .map(
            |(name, fullname, game_count)| archr_index::cache::SystemEntry {
                name: name.into(),
                fullname: fullname.into(),
                game_count,
                dir_mtime: 0,
                source_fingerprint: 0,
            },
        )
        .to_vec();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let games = [
            "Chrono Trigger",
            "Donkey Kong Country",
            "F-Zero",
            "Kirby Super Star",
            "Mega Man X",
            "Super Mario World",
            "Super Metroid",
            "The Legend of Zelda: A Link to the Past",
            "Yoshi's Island",
        ]
        .iter()
        .enumerate()
        .map(|(n, name)| archr_index::Game {
            path: format!("{name}.sfc").into(),
            name: (*name).into(),
            image: None,
            video: None,
            favorite: n == 5,
            last_played: (n == 3).then(|| now - 3 * 86_400),
            year: Some(1990 + n as u16),
            players: Some("1-2".into()),
            details: archr_index::Details {
                description: "Ataque os oito Mavericks e enfrente Sigma. \
                              Атакуйте восемь Мавериков и сразитесь с Сигмой. "
                    .repeat(4),
                genre: "Platform".into(),
                developer: "Capcom".into(),
                publisher: "Capcom".into(),
                rating: Some(80),
                ..Default::default()
            },
        })
        .collect();
        Self { systems, games }
    }
}
impl crate::state::DataSource for Shelf {
    fn systems(&self) -> &[archr_index::cache::SystemEntry] {
        &self.systems
    }
    fn games(&self, system: &str) -> &[archr_index::Game] {
        if system == "snes" {
            &self.games
        } else {
            &[]
        }
    }
    fn origin(&self, system: &str, game: usize) -> Option<(String, usize)> {
        Some((system.into(), game))
    }
    fn manufacturer(&self, system: &str) -> Option<(&str, &str)> {
        (system == "snes").then_some(("Nintendo", "1990"))
    }
    fn cores(&self, _: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
        None
    }
}
/// A stand-in box art: a dusk gradient with a sun and a horizon.
fn box_art(
    creator: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
) -> sdl2::render::Texture<'_> {
    let (w, h) = (300u32, 420u32);
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = y as f32 / h as f32;
            let (dx, dy) = (x as f32 - 150., y as f32 - 190.);
            let sun = dx.hypot(dy) < 70.;
            let ground = y > 300;
            let [r, g, b] = if sun {
                [250, 192, 74]
            } else if ground {
                [27, 42, 107]
            } else {
                [
                    (122. + 120. * t) as u8,
                    (59. + 60. * t) as u8,
                    (47. + 20. * t) as u8,
                ]
            };
            pixels.extend([r, g, b, 255]);
        }
    }
    let mut texture = creator
        .create_texture_static(sdl2::pixels::PixelFormatEnum::RGBA32, w, h)
        .unwrap();
    texture.update(None, &pixels, (w * 4) as usize).unwrap();
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    texture
}

#[test]
#[ignore = "manual capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_views() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_SETTINGS_CAPTURE_DIR").expect("capture directory"),
    );
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let themes = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let cover = box_art(&creator);
    let shelf = Shelf::new();
    let save = |ui: &mut Ui, name: String| {
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        image::save_buffer(
            output.join(name),
            &pixels,
            640,
            480,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let languages =
        std::env::var("ARCHR_SETTINGS_CAPTURE_LANGUAGES").unwrap_or_else(|_| "pt_BR,ru".into());
    let views = std::env::var("ARCHR_CAPTURE_VIEWS")
        .unwrap_or_else(|_| "default:lista,default:vitrine".into());
    for spec in views.split(',') {
        // folder:view, or folder:view:noart for a library without pictures.
        let mut parts = spec.split(':');
        let (folder, view) = (parts.next().unwrap(), parts.next().unwrap());
        let noart = parts.next() == Some("noart");
        // A folder may also be a theme outside the repository (absolute path).
        let label = std::path::Path::new(folder)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(folder);
        let default = themes.join("default");
        let mut theme = Theme::load_over(&themes.join(folder), &Theme::load(&default));
        theme.active_view = view.into();
        let empty = output.join("no-art");
        ui.theme_dir = Some(if noart {
            empty.clone()
        } else {
            themes.join(folder)
        });
        ui.default_theme_dir = Some(if noart { empty } else { default });
        ui.configure(&ttf, &creator, &theme, None).unwrap();
        let view_id = view;
        let view = &*format!("{view}{}", if noart { "-noart" } else { "" });
        for language in languages.split(',') {
            ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
            let mut app = crate::state::App::new(10);
            app.set_view(&theme.view(view_id));
            app.cursor = 1;
            // System art loads on the worker after the first frame.
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                ui.tick();
                ui.snap_animations();
                ui.draw(&app, &shelf, &theme, None).unwrap();
                std::thread::sleep(Duration::from_millis(20));
            }
            save(&mut ui, format!("{label}-{view}-systems-{language}.png"));
            // Frames in the middle of each motion, to prove they exist.
            let midway = |ui: &mut Ui, app: &crate::state::App, art, wait: u64, name: &str| {
                ui.draw(app, &shelf, &theme, art).unwrap();
                std::thread::sleep(Duration::from_millis(wait));
                ui.draw(app, &shelf, &theme, art).unwrap();
                assert!(ui.animating(), "{name} is still moving");
                save(ui, format!("{label}-{view}-{name}-{language}.png"));
            };
            ui.start_opening();
            midway(&mut ui, &app, None, 110, "opening-mid");
            ui.snap_animations();
            app.cursor = 1;
            app.handle(crate::state::Button::A, &shelf);
            app.cursor = 3;
            midway(&mut ui, &app, Some(&cover), 80, "slide-mid");
            for (name, art) in [("games", Some(&cover)), ("games-nocover", None)] {
                for _ in 0..2 {
                    ui.snap_animations();
                    ui.draw(&app, &shelf, &theme, art).unwrap();
                }
                save(&mut ui, format!("{label}-{view}-{name}-{language}.png"));
            }
            ui.snap_animations();
            ui.draw(&app, &shelf, &theme, Some(&cover)).unwrap();
            app.cursor = 4;
            midway(&mut ui, &app, None, 90, "crossfade-mid");
            ui.snap_animations();
            app.cursor = 3;
            ui.draw(&app, &shelf, &theme, Some(&cover)).unwrap();
            app.handle(crate::state::Button::Y, &shelf);
            midway(&mut ui, &app, Some(&cover), 60, "dialog-mid");
            ui.snap_animations();
            ui.draw(&app, &shelf, &theme, Some(&cover)).unwrap();
            save(&mut ui, format!("{label}-{view}-dialog-{language}.png"));
            // The details screen of the game under the cursor.
            let browsing = std::mem::replace(
                &mut app.screen,
                crate::state::Screen::Details {
                    system: "snes".into(),
                    game: 4,
                },
            );
            let art = crate::render::Art {
                cover: Some(&cover),
                ..Default::default()
            };
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw_with_settings(&app, &shelf, &theme, art, None)
                    .unwrap();
            }
            save(&mut ui, format!("{label}-{view}-details-{language}.png"));
            app.screen = browsing;
            // Appearance with this theme installed and active.
            let mut look = ready();
            look.base_theme = theme.clone();
            look.values.insert(Id::Theme, folder.into());
            look.values.insert(Id::View, view_id.into());
            look.theme_names.insert(folder.into(), theme.name.clone());
            look.open(Page::Appearance);
            // On the View row, so the accent dots show unhighlighted.
            look.cursor = 2;
            ui.canvas.set_draw_color(color(theme.background));
            ui.canvas.clear();
            ui.settings_page(&look, &look.appearance().apply(&theme))
                .unwrap();
            save(&mut ui, format!("{label}-{view}-appearance-{language}.png"));
        }
    }
}

/// Achievements page states (Config-ra) and its sign-in keyboard.
fn cheevos_pages() -> Vec<(&'static str, Settings)> {
    let mut base = ready();
    base.values.insert(Id::Cheevos, "1".into());
    base.options.insert(
        Id::CheevosSound,
        vec!["none".into(), "unlock".into(), "zelda".into()],
    );
    base.open(Page::Achievements);
    let signed_out = base.clone();
    let mut off = base.clone();
    off.values.insert(Id::Cheevos, "0".into());
    let mut signed_in = base.clone();
    signed_in.cheevos.user = "douglas".into();
    signed_in.cheevos.profile = Some(crate::settings::cheevos::Profile {
        points: 1240,
        softcore: 87,
    });
    for id in [Id::CheevosHardcore, Id::CheevosVerbose] {
        signed_in.values.insert(id, "1".into());
    }
    let mut scrolled = signed_in.clone();
    scrolled.cursor = scrolled.visible_rows().len() - 1;
    let mut user = base.clone();
    user.cursor = 1;
    user.handle(Button::A);
    user.keyboard = Some(crate::keyboard::Keyboard::new(
        crate::keyboard::Mode::Text,
        "douglas".into(),
    ));
    let mut password = user.clone();
    password.handle(Button::Start);
    for button in [Button::A; 6] {
        password.handle(button);
    }
    let mut verifying = base.clone();
    verifying.cheevos.verifying = true;
    verifying.cursor = 1;
    vec![
        ("conquistas-deslogado", signed_out),
        ("conquistas-desligado", off),
        ("conquistas-logado", signed_in),
        ("conquistas-logado-fim", scrolled),
        ("conquistas-teclado-usuario", user),
        ("conquistas-teclado-senha", password),
        ("conquistas-verificando", verifying),
    ]
}
#[test]
#[ignore = "manual capture, ARCHR_CHEEVOS_CAPTURE_DIR required"]
fn capture_achievements() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_CHEEVOS_CAPTURE_DIR").expect("capture directory"),
    );
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let themes = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let theme = Theme::load(&themes.join("default"));
    ui.theme_dir = Some(themes.join("default"));
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let save = |ui: &mut Ui, name: String| {
        let pixels = ui
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        image::save_buffer(
            output.join(name),
            &pixels,
            640,
            480,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    // Badges as the lookup leaves them on disk: a few coloured squares.
    let badge_dir = tempfile::tempdir().unwrap();
    let badge = |n: u8, gray: bool| {
        let path = badge_dir.path().join(format!("{n}.png"));
        let rgb = if gray {
            [90, 96, 104]
        } else {
            [40 + n * 30, 160, 255 - n * 25]
        };
        image::RgbaImage::from_fn(64, 64, |x, y| {
            let edge = x < 6 || y < 6 || x > 57 || y > 57;
            let c = if edge { rgb.map(|v| v / 2) } else { rgb };
            image::Rgba([c[0], c[1], c[2], 255])
        })
        .save(&path)
        .unwrap();
        path
    };
    let item =
        |title: &str, description: &str, points: u64, unlocked: bool, badge| crate::cheevos::Item {
            title: title.into(),
            description: description.into(),
            points,
            unlocked,
            badge,
        };
    let sheet = crate::cheevos::Sheet {
        fetch: crate::cheevos::Fetch::Fresh,
        hardcore: true,
        items: vec![
            item("Começo de jornada", "Termine a primeira fase", 5, true, Some(badge(1, false))),
            item("Caça às moedas", "Junte 100 moedas numa fase", 10, true, Some(badge(2, false))),
            item("Mundo das estrelas", "Abra a Estrada Estelar", 25, false, Some(badge(3, true))),
            item("Sem uma vida perdida", "Termine o mundo 1 sem morrer", 25, false, None),
            item(
                "Segredo do castelo",
                "Ache a saída secreta do castelo de Bowser sem usar a capa nem o Yoshi em nenhum momento",
                10,
                false,
                None,
            ),
            item("Все звёзды", "Соберите все звёзды в Мире звёзд", 50, false, None),
            item("Último", "Página dois", 1, false, None),
        ],
    };
    let mut badges =
        crate::covers::CoverCache::new(&creator, badge_dir.path().join("thumbs"), 36, 36);
    let wanted: Vec<_> = sheet.items.iter().filter_map(|i| i.badge.clone()).collect();
    let deadline = Instant::now() + Duration::from_secs(3);
    while wanted.iter().any(|p| badges.peek(p).is_none()) && Instant::now() < deadline {
        badges.request(&wanted);
        badges.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    let shelf = Shelf::new();
    for language in ["pt_BR", "ru", "es", "en"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in cheevos_pages() {
            // Each state is a fresh page, not a switch flipped on the last one.
            ui.switches.clear();
            ui.snap_animations();
            ui.canvas.set_draw_color(color(theme.background));
            ui.canvas.clear();
            ui.settings_page(&state, &theme).unwrap();
            save(&mut ui, format!("{name}-{language}.png"));
        }
        // The game screen (RA-jogo) and the game menu that opens it.
        let mut app = crate::state::App::new(1);
        app.set_view(&theme.view(&theme.active_view));
        app.cheevos = true;
        app.screen = crate::state::Screen::Games {
            system: "snes".into(),
        };
        app.cursor = 5;
        app.handle(Button::Y, &shelf);
        app.cursor = 1;
        let art = crate::render::Art {
            badges: Some(&badges),
            ..Default::default()
        };
        let frame = |ui: &mut Ui, app: &crate::state::App, name: &str| {
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw_with_settings(app, &shelf, &theme, art, None)
                    .unwrap();
            }
            save(ui, format!("{name}-{language}.png"));
        };
        frame(&mut ui, &app, "jogo-menu");
        app.handle(Button::A, &shelf);
        frame(&mut ui, &app, "jogo-carregando");
        app.sheet = sheet.clone();
        app.cursor = 1;
        frame(&mut ui, &app, "jogo-conquistas");
        app.sheet_page = ui.achievements_page;
        app.handle(Button::R, &shelf);
        app.sheet.fetch = crate::cheevos::Fetch::Saved;
        frame(&mut ui, &app, "jogo-conquistas-pagina2");
        app.sheet = crate::cheevos::Sheet {
            fetch: crate::cheevos::Fetch::Failed("cheevos.unsupported"),
            ..Default::default()
        };
        frame(&mut ui, &app, "jogo-sem-formato");
    }
}
/// A theme's preview as GitHub would serve it: its own colours, a list and a cover.
fn store_preview(
    dir: &std::path::Path,
    name: &str,
    colors: ([u8; 3], [u8; 3], [u8; 3]),
) -> std::path::PathBuf {
    let (back, ink, accent) = colors;
    let path = dir.join(format!("{name}.png"));
    image::RgbaImage::from_fn(640, 480, |x, y| {
        let row = (y as i32 - 70) / 44;
        let c = if (412..600).contains(&x) && (80..380).contains(&y) {
            ink.map(|v| v / 2 + back[0] / 4)
        } else if (30..380).contains(&x)
            && (70..334).contains(&(y as i32))
            && (y as i32 - 70) % 44 < 30
        {
            if row == 1 {
                accent
            } else {
                ink.map(|v| (u16::from(v) * 2 / 5 + u16::from(back[0]) * 3 / 5) as u8)
            }
        } else if !(44..432).contains(&y) {
            back.map(|v| v.saturating_add(12))
        } else {
            back
        };
        image::Rgba([c[0], c[1], c[2], 255])
    })
    .save(&path)
    .unwrap();
    path
}
fn store_pages(dir: &std::path::Path) -> Vec<(&'static str, Settings)> {
    use crate::settings::store::{Card, Detail, Reply, Tab};
    let card =
        |owner: &str, repo: &str, name: &str, stars: u64, preview: Option<std::path::PathBuf>| {
            Card {
                owner: owner.into(),
                repo: repo.into(),
                name: crate::layout::Names::plain(name),
                stars,
                branch: "main".into(),
                pushed: "2026-10-12".into(),
                preview,
            }
        };
    let cards = vec![
        card(
            "archr-linux",
            "grafite",
            "Grafite",
            312,
            Some(store_preview(
                dir,
                "grafite",
                ([11, 15, 20], [230, 237, 243], [30, 136, 255]),
            )),
        ),
        card(
            "archr-linux",
            "praia",
            "Praia",
            260,
            Some(store_preview(
                dir,
                "praia",
                ([10, 37, 64], [243, 246, 250], [255, 201, 60]),
            )),
        ),
        card(
            "kaio-dev",
            "neon-arcade",
            "Neon Arcade",
            214,
            Some(store_preview(
                dir,
                "neon",
                ([18, 0, 34], [245, 230, 255], [255, 46, 151]),
            )),
        ),
        card(
            "marina-s",
            "papel",
            "Papel",
            98,
            Some(store_preview(
                dir,
                "papel",
                ([244, 241, 234], [31, 27, 22], [210, 69, 43]),
            )),
        ),
        card(
            "retrofolks",
            "game-boy",
            "Game Boy",
            77,
            Some(store_preview(
                dir,
                "gb",
                ([155, 188, 15], [15, 56, 15], [15, 56, 15]),
            )),
        ),
        card("sw-themes", "mono", "Mono", 41, None),
        card(
            "ana",
            "aurora-boreal-extremamente-longa",
            "Aurora Boreal Extremamente Longa",
            9,
            None,
        ),
    ];
    let mut grid = ready();
    grid.open(Page::Appearance);
    grid.open(Page::Store);
    grid.store.fetched = Some(std::time::SystemTime::now());
    grid.store_reply(Reply::Installed(
        [
            ("archr-linux-grafite".to_owned(), "a".to_owned()),
            ("archr-linux-praia".to_owned(), "b".to_owned()),
        ]
        .into_iter()
        .collect(),
        cards[..2].to_vec(),
    ));
    grid.store.tab = Tab::Popular;
    grid.store_reply(Reply::Listed {
        tab: Tab::Popular,
        cards: cards.clone(),
        problem: None,
        fetched: Some(std::time::SystemTime::now()),
    });
    grid.cursor = 2;
    let mut lower = grid.clone();
    lower.cursor = 6;
    let mut detail = grid.clone();
    detail.handle(Button::A);
    let shots: Vec<_> = (1..=3)
        .map(|n| {
            store_preview(
                dir,
                &format!("neon-{n}"),
                ([18 + n * 10, 0, 34], [245, 230, 255], [255, 46, 151]),
            )
        })
        .collect();
    let mut facts = Detail::describe(
        cards[2].clone(),
        "name = \"Neon Arcade\"\ndescription = { en = \"Neon pink over deep purple, a glowing showcase and arcade sounds.\", \
         pt_BR = \"Rosa neon sobre roxo profundo, vitrine com brilho e sons de fliperama.\", \
         ru = \"Неоново-розовый на глубоком фиолетовом, витрина с подсветкой и звуки игровых автоматов.\" }\n\
         author = \"kaio-dev\"\nversion = \"1.2.0\"\nlicense = \"CC BY 4.0\"\n",
        &[
            ("theme.toml".into(), 2_000),
            ("fonts/Neon.ttf".into(), 2_400_000),
            ("systems/snes.png".into(), 900_000),
            ("sounds/move.ogg".into(), 300_000),
        ],
        "0.1.0",
    )
    .unwrap();
    facts.date = "2026-10-12".into();
    facts.reference = "v1.2.0".into();
    facts.tree = "t2".into();
    facts.images = [cards[2].preview.clone().unwrap()]
        .into_iter()
        .chain(shots)
        .collect();
    detail.store_reply(Reply::Detail(
        "kaio-dev-neon-arcade".into(),
        Ok(Box::new(facts)),
    ));
    let mut loading = grid.clone();
    loading.handle(Button::A);
    let mut installing = detail.clone();
    installing.handle(Button::A);
    let mut installed = installing.clone();
    let mut map = installed.store.installed.clone();
    map.insert("kaio-dev-neon-arcade".into(), "t2".into());
    installed.store_reply(Reply::Installed(map, cards[..3].to_vec()));
    installed.store_reply(Reply::Done(Ok("store.installed_ok")));
    let mut remove = installed.clone();
    remove.error = None;
    remove.handle(Button::X);
    let mut offline = ready();
    offline.open(Page::Appearance);
    offline.open(Page::Store);
    offline.store_reply(Reply::Listed {
        tab: Tab::Popular,
        cards: Vec::new(),
        problem: Some("store.offline"),
        fetched: None,
    });
    let mut limited = offline.clone();
    limited.store_reply(Reply::Listed {
        tab: Tab::Popular,
        cards: Vec::new(),
        problem: Some("store.rate_limited"),
        fetched: None,
    });
    let mut saved = grid.clone();
    saved.store.problem = Some("store.rate_limited");
    let mut mine = installed.clone();
    mine.handle(Button::B);
    mine.error = None;
    mine.handle(Button::L);
    mine.store.loading = false;
    vec![
        ("loja-grade", grid),
        ("loja-grade-rolada", lower),
        ("loja-detalhes-carregando", loading),
        ("loja-detalhes", detail),
        ("loja-instalando", installing),
        ("loja-instalado-usar-agora", installed),
        ("loja-remover", remove),
        ("loja-instalados", mine),
        ("loja-sem-rede", offline),
        ("loja-limite", limited),
        ("loja-limite-lista-salva", saved),
    ]
}
#[test]
#[ignore = "manual capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_store() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_SETTINGS_CAPTURE_DIR").expect("capture directory"),
    );
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let themes = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let theme = Theme::load(&themes.join("default"));
    let images = tempfile::tempdir().unwrap();
    ui.theme_dir = Some(themes.join("default"));
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    ui.root = Some(images.path().to_path_buf());
    let pages = store_pages(images.path());
    for language in ["pt_BR", "ru", "es", "en"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in &pages {
            // Draw, let the loader decode what the page asked for, draw again.
            let deadline = Instant::now() + Duration::from_secs(5);
            ui.switches.clear();
            for _ in 0..2 {
                ui.snap_animations();
                ui.settings_page(state, &theme).unwrap();
            }
            loop {
                ui.snap_animations();
                ui.canvas.set_draw_color(color(theme.background));
                ui.canvas.clear();
                ui.settings_page(state, &theme).unwrap();
                let wanted = ui.store_wanted.clone();
                let ready = wanted
                    .iter()
                    .all(|p| ui.store_art.as_ref().is_some_and(|a| a.peek(p).is_some()));
                if ready || Instant::now() > deadline {
                    break;
                }
                ui.tick();
                std::thread::sleep(Duration::from_millis(20));
            }
            let pixels = ui
                .canvas
                .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
            image::save_buffer(
                output.join(format!("{name}-{language}.png")),
                &pixels,
                640,
                480,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}
