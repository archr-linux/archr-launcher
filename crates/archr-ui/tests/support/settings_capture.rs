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
    services.cursor = 2;
    // Config-usb: the console on the PC's internet, Wi-Fi held off.
    let mut usb_internet = services.clone();
    usb_internet.values.insert(Id::UsbMode, "internet".into());
    usb_internet.usb =
        crate::settings::network::Usb::parse("link=online\naddress=192.168.137.42\nwifi=held\n");
    usb_internet.cursor = 5;
    let mut usb_fallback = usb_internet.clone();
    usb_fallback.usb =
        crate::settings::network::Usb::parse("link=fallback\naddress=169.254.170.2\nwifi=held\n");
    usb_fallback.cursor = 8;
    let mut usb_rndis = services.clone();
    usb_rndis.values.insert(Id::UsbCompat, "1".into());
    usb_rndis.usb = crate::settings::network::Usb::parse("link=local\naddress=169.254.170.2\n");
    usb_rndis.cursor = 6;
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
        name: "settings.card_internal".into(),
        detail: "settings.card_detail|fs=ext4|used=@4.1 GB|total=@25.3 GB".into(),
        percent: 16,
    });
    storage.storage.second = Some(Card {
        name: "settings.card_external|label=EASYROMS".into(),
        detail: "settings.card_detail|fs=exfat|used=@41.2 GB|total=@119.0 GB".into(),
        percent: 35,
    });
    storage.storage.backup = Some("settings.backup_today|time=14:42".into());
    storage.cursor = 1;
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
    let mut services_password = services.clone();
    services_password.cursor = 1;
    services_password.handle(Button::A);
    for button in [Button::A; 6] {
        services_password.handle(button);
    }
    let mut services_confirm = services_password.clone();
    services_confirm.handle(Button::Start);
    let mut storage_errors = storage.clone();
    storage_errors.storage.second.as_mut().unwrap().detail = "settings.card_errors".into();
    let mut storage_internal = storage.clone();
    storage_internal.storage.second = None;
    storage_internal.storage.backup = None;
    storage_internal.cursor = 0;
    // Unified storage off (cursor on its switch), on, and the question
    // before separating the cards.
    let mut storage_separate_cards = storage.clone();
    storage_separate_cards.cursor = 3;
    let mut storage_unified = storage.clone();
    storage_unified.values.insert(Id::MergedStorage, "1".into());
    storage_unified.storage.unified = Some(Card {
        name: "settings.card_unified|first=@64|second=@128 GB|total=@192 GB".into(),
        detail: "settings.used_total|used=@45.3 GB|total=@144.3 GB".into(),
        percent: 31,
    });
    storage_unified.cursor = 4;
    let mut storage_unified_top = storage_unified.clone();
    storage_unified_top.cursor = 0;
    let mut storage_separate = storage_unified.clone();
    storage_separate.storage.confirm = Some(Confirm::Separate);
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
    let mut raimport = ready();
    raimport.open(Page::System);
    raimport.open(Page::RaImport);
    let pairs = |items: &[(&str, &str)]| -> Vec<(String, String)> {
        items
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    };
    raimport.raimport.found = Some(crate::settings::raimport::Found {
        settings: 14,
        skipped: 9,
        cores: vec!["mGBA".into(), "Snes9x".into(), "FCEUmm".into()],
        overrides: 3,
        presets: 2,
        remaps: 1,
        apply: pairs(&[
            ("video_scale_integer", "true"),
            ("video_scale_integer_overscale", "true"),
            ("aspect_ratio_index", "23"),
            ("audio_latency", "64"),
            ("input_menu_toggle_gamepad_combo", "2"),
            ("config/mGBA/mGBA.cfg", "raimport.file_settings|count=3"),
            ("shaders/presets/mGBA/Game.slangp", "raimport.file_preset"),
        ]),
        ignored: pairs(&[
            ("video_driver", "raimport.why_blocked"),
            ("libretro_directory", "raimport.why_blocked"),
            ("audio_out_rate", "raimport.why_unknown"),
            ("config/Mupen64Plus-Next", "raimport.why_core"),
            ("shaders/crt-royale.slangp", "raimport.why_shader"),
        ]),
    });
    raimport.cursor = 5;
    use crate::settings::roms::{Check, Scan, Status};
    let mut storage_roms = ready();
    storage_roms.open(Page::Storage);
    storage_roms.storage.card = Some(crate::settings::storage::Card {
        name: "settings.card_internal".into(),
        detail: "settings.card_parts|system=@6.4 GB|roms=@13.2 GB".into(),
        percent: 32,
    });
    storage_roms.storage.roms = Status::parse("LAYOUT=exfat\nROMS_FS=errors\n");
    storage_roms.cursor = 1;
    let mut roms = ready();
    roms.open(Page::Storage);
    roms.open(Page::Roms);
    roms.storage.roms = Status::parse("LAYOUT=ext4\n");
    roms.roms.scan = Some(Scan {
        check: Check::parse(
            "ROMS_FILES=1507\nROMS_MIB=7987\nSTORAGE_AFTER_MIB=8192\nROMS_PART_MIB=21504\n\
             RENAMES=3\nPOWER=ok\nRESULT=ok\n",
        ),
        names: vec![
            (
                "snes/Zelda: A Link to the Past.sfc".into(),
                "Zelda_ A Link to the Past.sfc".into(),
            ),
            ("gba/Mario & Luigi?.gba".into(), "Mario & Luigi_.gba".into()),
            (
                "n64/mario kart 64.z64".into(),
                "mario kart 64 (2).z64".into(),
            ),
        ],
        battery: Some((87, true)),
    });
    roms.cursor = 3;
    let mut roms_list = roms.clone();
    roms_list.cursor = 1;
    roms_list.handle(Button::A);
    assert_eq!(roms_list.page(), Page::RomsList);
    let mut roms_confirm = roms.clone();
    roms_confirm.handle(Button::A);
    let mut roms_armed = roms.clone();
    roms_armed.storage.roms = Status::parse("LAYOUT=migrating\nSTATE=armed\n");
    roms_armed.cursor = 0;
    let mut roms_exfat = roms.clone();
    roms_exfat.storage.roms =
        Status::parse("LAYOUT=exfat\nROMS_FS=repaired\nNOT_MIGRATED=/storage/roms-not-migrated\n");
    roms_exfat.cursor = 0;
    let mut raimport_list = raimport.clone();
    raimport_list.cursor = 3;
    raimport_list.handle(Button::A);
    assert_eq!(raimport_list.page(), Page::RaImportList);
    let mut sound = ready();
    sound.open(Page::Sound);
    let sink = "Built-in Audio Internal Speaker".to_owned();
    sound.options.insert(Id::Audio, vec![sink.clone()]);
    sound.values.insert(Id::Audio, sink);
    // Volume and brightness move through archr-levels; the sounds slider
    // still slides here.
    sound.values.insert(Id::Volume, "64".into());
    sound.cursor = 4;
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
        ("services_usb_internet", usb_internet),
        ("services_usb_fallback", usb_fallback),
        ("services_usb_rndis", usb_rndis),
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
        ("storage-unified-off", storage_separate_cards),
        ("storage-unified", storage_unified),
        ("storage-unified-top", storage_unified_top),
        ("storage-separate", storage_separate),
        ("storage-errors", storage_errors),
        ("storage-backup", storage_backup),
        ("storage-backup-wifi", storage_wifi),
        ("storage-reset-final", storage_reset),
        ("storage-working", storage_working),
        ("services-password", services_password),
        ("services-password-confirm", services_confirm),
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
        ("raimport", raimport),
        ("raimport-list", raimport_list),
        ("storage-roms", storage_roms),
        ("roms", roms),
        ("roms-list", roms_list),
        ("roms-confirm", roms_confirm),
        ("roms-armed", roms_armed),
        ("roms-exfat", roms_exfat),
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
            // The ROMs partition went read-only: the footer says so.
            if let Some(status) = &mut ui.status {
                status.roms = Some("roms.alert_readonly");
            }
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw(&app, &shelf, &theme, Some(&cover)).unwrap();
            }
            save(
                &mut ui,
                format!("{label}-{view}-games-alert-{language}.png"),
            );
            if let Some(status) = &mut ui.status {
                status.roms = None;
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
            // The Start menu with a module's tile (and its live value) and
            // shortcut: a tile, then each shortcut, selected.
            let entry = |id: &str, label: &str| crate::modules::Entry {
                id: id.into(),
                label: label.into(),
                icon: None,
                target: crate::modules::Target::Action("a".into()),
            };
            let mut modules = crate::modules::View {
                modules: vec![crate::modules::Module {
                    id: "clima".into(),
                    enabled: true,
                    granted: true,
                    tiles: vec![entry("t", "Clima")],
                    shortcuts: vec![entry("s", "Clima")],
                    ..Default::default()
                }],
                ..Default::default()
            };
            modules
                .values
                .insert(("clima".into(), "tile:t".into()), "24 °C".into());
            app.modules = std::sync::Arc::new(modules);
            let hub = |row, tile, shortcut| crate::state::Screen::StartHub {
                row,
                tile,
                shortcut,
            };
            for (name, screen) in [("hub-tile", hub(0, 2, 0))]
                .into_iter()
                .chain((0..7).map(|n| ("hub-shortcut", hub(1, 0, n))))
                .chain([("hub-module", hub(0, 3, 0))])
                .enumerate()
                .map(|(n, (name, screen))| (format!("{name}{}", n.saturating_sub(1)), screen))
            {
                app.screen = screen;
                for _ in 0..2 {
                    ui.snap_animations();
                    ui.draw(&app, &shelf, &theme, None).unwrap();
                }
                save(&mut ui, format!("{label}-{view}-{name}-{language}.png"));
            }
            app.modules = Default::default();
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
        ..Default::default()
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
    // The web API key row: missing, saved (the card gains its extras),
    // refused, its keyboard, and a session the service no longer accepts.
    let mut keyed = signed_in.clone();
    keyed.cheevos.web_key = true;
    if let Some(profile) = keyed.cheevos.profile.as_mut() {
        profile.since = "2021".into();
        profile.last_game = "Super Mario World".into();
    }
    keyed.cursor = 1;
    let mut refused = signed_in.clone();
    refused.cheevos.web_key = true;
    refused.cheevos.key_refused = true;
    refused.cursor = 1;
    let mut key_keyboard = signed_in.clone();
    key_keyboard.cursor = 1;
    key_keyboard.handle(Button::A);
    for button in [Button::A; 8] {
        key_keyboard.handle(button);
    }
    let mut expired = keyed.clone();
    expired.cheevos.expired = true;
    vec![
        ("conquistas-chave-ausente", {
            let mut s = signed_in.clone();
            s.cursor = 1;
            s
        }),
        ("conquistas-chave-informada", keyed),
        ("conquistas-chave-recusada", refused),
        ("conquistas-teclado-chave", key_keyboard),
        ("conquistas-sessao-expirada", expired),
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

/// Updates (Atualizacoes*): the page, its run screens and the START mark.
fn updates_pages() -> Vec<(&'static str, Settings)> {
    use crate::settings::updates::{Package, Progress, Reboot, Report, Run, Stage};
    let package = |name: &str, old: &str, new: &str, size: u64| Package {
        name: name.into(),
        old: old.into(),
        new: new.into(),
        size,
    };
    let mut page = ready();
    page.info.version = "2.1.1".into();
    page.open_updates();
    page.updates.checked = Instant::now().checked_sub(Duration::from_secs(125));
    page.updates.report = Some(Report {
        version: "2.1.1".into(),
        channel: "stable".into(),
        packages: vec![
            package("archr-launcher", "1.4.2", "1.5.0", 2_202_009),
            package("linux-rk3326", "6.18.55-2", "6.18.57-1", 32_925_696),
            package("retroarch", "1.21.0-3", "1.22.0-1", 13_421_772),
            package("mesa", "26.2.1", "26.2.2", 1_992_294),
        ],
        download: 50_541_772,
        reboot: Some(Reboot::Kernel),
        new_version: "2.1.2".into(),
        news: "Launcher 1.5: loja de temas e sons. Kernel com carregamento mais rápido. RetroArch 1.22."
            .into(),
    });
    let mut current = page.clone();
    current.updates.report = Some(Report {
        version: "2.1.1".into(),
        ..Default::default()
    });
    current.cursor = 3;
    let mut checking = page.clone();
    checking.updates.report = None;
    checking.updates.checking = true;
    let mut offline = checking.clone();
    offline.updates.checking = false;
    offline.updates.failed = Some("network".into());
    let mut many = page.clone();
    if let Some(report) = &mut many.updates.report {
        for n in 0..14 {
            report.packages.push(package(
                &format!("lib-extra-{n}"),
                "1.0-1",
                "1.1-1",
                400_000,
            ));
        }
    }
    let mut packages = many.clone();
    packages.open(Page::UpdatePackages);
    packages.cursor = 2;
    let mut progress = page.clone();
    progress.updates.run = Some(Run::Working(Progress {
        stage: Stage::Download,
        package: "retroarch-1.22.0-1".into(),
        bytes: 33_449_574,
        total: 50_541_772,
        speed: 1_887_436,
        percent: 66,
        ..Default::default()
    }));
    let mut installing = page.clone();
    installing.updates.run = Some(Run::Working(Progress {
        stage: Stage::Install,
        package: "linux-rk3326".into(),
        current: 2,
        count: 4,
        percent: 50,
        bytes: 50_541_772,
        total: 50_541_772,
        ..Default::default()
    }));
    let mut done = page.clone();
    done.updates.run = Some(Run::Done {
        reboot: Some(Reboot::Kernel),
        from: "2.1.1".into(),
        to: "2.1.2".into(),
        count: 4,
    });
    let mut failed = page.clone();
    failed.updates.run = Some(Run::Failed {
        reason: "download".into(),
        line: "error: failed retrieving file 'retroarch-1.22.0-1-aarch64.pkg.tar.zst'".into(),
        changed: false,
    });
    // Versions pinned to a commit (AIC8800 on the device), two that share
    // their first 7 characters, two 64-character hashes that differ only in
    // the last one, a long plain version and a new dependency: no line
    // shows any of them.
    let absurd = "0123456789abcdef".repeat(4);
    let other = format!("{}e", &absurd[..63]);
    let mut hashed = page.clone();
    if let Some(report) = &mut hashed.updates.report {
        report.packages = vec![
            package(
                "AIC8800",
                "b6ba4cbacf9657caecce26c2426880c5bb485266-1",
                "1018f17a629c638acc1a01df19e3f2146e7b4f5c-1",
                368_204,
            ),
            package(
                "colisao",
                &format!("abc1234{}-1", "a".repeat(33)),
                &format!("abc1234{}-1", "b".repeat(33)),
                2_202_009,
            ),
            package(
                "firmware-absurdo",
                &format!("{absurd}-1"),
                &format!("{other}-1"),
                1_992_294,
            ),
            package("versao-longa", &"x".repeat(64), &"y".repeat(64), 400_000),
            package("dependencia-nova", "", &format!("{absurd}-1"), 52_000),
        ];
    }
    let mut hashed_list = hashed.clone();
    hashed_list.open(Page::UpdatePackages);
    let mut hashed_progress = hashed.clone();
    hashed_progress.updates.run = Some(Run::Working(Progress {
        stage: Stage::Download,
        package:
            "deadbeefcafe0-tools-1018f17a629c638acc1a01df19e3f2146e7b4f5c-1-aarch64.pkg.tar.zst"
                .into(),
        bytes: 368_204,
        total: 2_570_408,
        speed: 180_000,
        percent: 14,
        ..Default::default()
    }));
    vec![
        ("atualizacoes", page),
        ("atualizacoes-hash", hashed),
        ("atualizacoes-hash-pacotes", hashed_list),
        ("atualizacoes-hash-progresso", hashed_progress),
        ("atualizacoes-em-dia", current),
        ("atualizacoes-verificando", checking),
        ("atualizacoes-sem-rede", offline),
        ("atualizacoes-muitos", many),
        ("atualizacoes-pacotes", packages),
        ("atualizacoes-progresso", progress),
        ("atualizacoes-instalando", installing),
        ("atualizacoes-concluida", done),
        ("atualizacoes-erro", failed),
    ]
}
#[test]
#[ignore = "manual capture, ARCHR_UPDATES_CAPTURE_DIR required"]
fn capture_updates() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_UPDATES_CAPTURE_DIR").expect("capture directory"),
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
    let shelf = Shelf::new();
    for language in ["pt_BR", "ru", "en", "es"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in updates_pages() {
            ui.switches.clear();
            ui.frame.now = ui.born + Duration::from_millis(480);
            for _ in 0..2 {
                ui.canvas.set_draw_color(color(theme.background));
                ui.canvas.clear();
                ui.settings_page(&state, &theme).unwrap();
                ui.snap_animations();
            }
            save(&mut ui, format!("{name}-{language}.png"));
        }
        // The START menu with an update waiting, on the update shortcut and off it.
        let mut app = crate::state::App::new(1);
        app.updates = 4;
        for (name, shortcut) in [("start-atualizacao", 1), ("start-marca", 0)] {
            app.screen = crate::state::Screen::StartHub {
                row: 1,
                tile: 1,
                shortcut,
            };
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw_with_settings(&app, &shelf, &theme, crate::render::Art::default(), None)
                    .unwrap();
            }
            save(&mut ui, format!("{name}-{language}.png"));
        }
    }
}

/// Long versions (a 64-character hash among them) stay on their own line:
/// nothing reaches the gap between the rows and the packages panel, nor the
/// screen's right margin; each line shows the name, update or new and the
/// size, never a version, and the progress names the package, not its file.
#[test]
fn update_versions_stay_inside_their_line() {
    use crate::settings::updates::{Run, Stage};
    let _sdl = crate::SDL_TEST_LOCK.lock().unwrap();
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::load(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"),
    );
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let pages: std::collections::HashMap<_, _> = updates_pages().into_iter().collect();
    let draw = |ui: &mut Ui, state: &Settings, area: sdl2::rect::Rect| {
        ui.switches.clear();
        ui.frame.now = ui.born + Duration::from_millis(480);
        for _ in 0..2 {
            ui.canvas.set_draw_color(color(theme.background));
            ui.canvas.clear();
            ui.settings_page(state, &theme).unwrap();
            ui.snap_animations();
        }
        ui.canvas
            .read_pixels(Some(area), sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap()
    };
    let mut empty = pages["atualizacoes"].clone();
    if let Some(report) = &mut empty.updates.report {
        report.packages.clear();
    }
    for language in ["pt_BR", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        // The gap column and the right margin, beside the package lines.
        for area in [
            sdl2::rect::Rect::new(370, 70, 12, 140),
            sdl2::rect::Rect::new(626, 70, 14, 140),
        ] {
            let clean = draw(&mut ui, &empty, area);
            let hashed = draw(&mut ui, &pages["atualizacoes-hash"], area);
            assert!(
                clean == hashed,
                "a version left its line ({language}, {area:?})"
            );
        }
        let edge = sdl2::rect::Rect::new(626, 50, 14, 140);
        let clean = draw(&mut ui, &empty, edge);
        let list = draw(&mut ui, &pages["atualizacoes-hash-pacotes"], edge);
        assert!(clean == list, "the list passed the margin ({language})");
    }
    // What each line says: its name, update or new, and the size; never a
    // version, which is a 40- or 64-character hash for some packages.
    let mut view = View {
        ui: &mut ui,
        theme: &theme,
    };
    view.ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some("pt_BR"), None);
    let report = pages["atualizacoes-hash"].updates.report.clone().unwrap();
    for package in &report.packages {
        let (kind, size) = view.package_note(package);
        let line = format!("{}{kind}{size}", package.name);
        assert!(line.contains(&view.ui.catalog.size(package.size)), "{line}");
        let expected = if package.old.is_empty() {
            "novo"
        } else {
            "atualização"
        };
        assert_eq!(kind, expected);
        for version in [&package.old, &package.new] {
            let start: String = version.chars().take(7).collect();
            assert!(
                start.is_empty() || !line.contains(&start),
                "{line} shows {version}"
            );
        }
    }
    // The progress screen names the package, not its file.
    let state = &pages["atualizacoes-hash-progresso"];
    let Some(Run::Working(mut progress)) = state.updates.run.clone() else {
        panic!("a run in progress");
    };
    let (now, _, _) = view.progress_text(state, &progress);
    assert!(now.contains("deadbeefcafe0-tools"), "{now}");
    assert!(
        !now.contains("1018f17") && !now.contains("aarch64"),
        "{now}"
    );
    progress.stage = Stage::Install;
    progress.package = "deadbeefcafe1-tools".into();
    let (now, _, install) = view.progress_text(state, &progress);
    for text in [&now, &install] {
        assert!(text.contains("deadbeefcafe1-tools"), "{text}");
    }
}

/// System › Modules, the module store, the permission page, a module page
/// and a module tile on the Start menu, as installed on a device.
fn module_view() -> crate::modules::View {
    use crate::modules::{
        Context, Entry, Module, Page as ModPage, Permission, Quick, Row as ModRow, RowKind, Status,
        Target,
    };
    let icons =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/icons");
    let svg = |name: &str| {
        Some(
            std::fs::read_to_string(icons.join(format!("{name}.svg")))
                .unwrap()
                .into(),
        )
    };
    let permission = |key, detail: &str, sensitive| Permission {
        key,
        detail: detail.into(),
        sensitive,
    };
    let row = |label: &str, section: Option<&str>, kind, default: &str| ModRow {
        label: label.into(),
        subtitle: None,
        section: section.map(Into::into),
        kind,
        default: default.into(),
    };
    let battery = Module {
        id: "bateria-detalhada".into(),
        name: "Bateria detalhada".into(),
        description: "Carga, potência e saúde da bateria no menu Start.".into(),
        author: "archr-linux".into(),
        version: "1.0.2".into(),
        license: "MIT".into(),
        icon: svg("battery-full"),
        enabled: true,
        granted: true,
        source: "archr-linux/archr-bateria".into(),
        reviewed: true,
        permissions: vec![
            permission("modules.perm_none", "modules.perm_none_detail", false),
            permission("modules.perm_tile", "", false),
            permission("modules.perm_page", "", false),
            permission("modules.perm_daemon", "", false),
        ],
        tiles: vec![Entry {
            id: "bateria".into(),
            label: "Bateria".into(),
            icon: svg("battery-full"),
            target: Target::Page(0),
        }],
        widgets: vec![Entry {
            id: "carga".into(),
            label: "Bateria detalhada".into(),
            icon: svg("battery-full"),
            target: Target::Page(0),
        }],
        pages: vec![ModPage {
            id: "main".into(),
            title: "Bateria detalhada".into(),
            detail: false,
            hint: None,
            rows: vec![
                row(
                    "Agora",
                    Some("Estado"),
                    RowKind::Text { id: "agora".into() },
                    "",
                ),
                row("Saúde", None, RowKind::Text { id: "saude".into() }, ""),
                row(
                    "Mostrar potência",
                    Some("Opções"),
                    RowKind::Toggle {
                        key: "watts".into(),
                    },
                    "1",
                ),
                row(
                    "Atualizar a cada",
                    None,
                    RowKind::Value {
                        key: "intervalo".into(),
                        choices: vec!["5".into(), "15".into(), "30".into()],
                        labels: vec![
                            "5 segundos".into(),
                            "15 segundos".into(),
                            "30 segundos".into(),
                        ],
                    },
                    "15",
                ),
                row(
                    "Atualizar agora",
                    None,
                    RowKind::Action {
                        action: "atualizar".into(),
                        confirm: None,
                        icon: None,
                    },
                    "",
                ),
            ],
        }],
        settings: [
            ("watts".to_owned(), "1".to_owned()),
            ("intervalo".to_owned(), "15".to_owned()),
        ]
        .into(),
        ..Module::default()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cpu = Module {
        id: "perfil-cpu".into(),
        name: "Perfil de CPU".into(),
        version: "2.0.0".into(),
        license: "MIT".into(),
        icon: svg("cpu"),
        enabled: true,
        granted: true,
        source: "archr-linux/archr-module-perfil-cpu".into(),
        reviewed: true,
        digest: format!("sha256:3f9a{}c21e", "0".repeat(56)),
        used: now - 600,
        auto_updated: now - 86400,
        permissions: vec![
            permission("modules.perm_config_write", "system.cpugovernor", true),
            permission(
                "modules.perm_hooks_launch",
                "modules.perm_hooks_launch_detail",
                false,
            ),
            permission(
                "modules.perm_cpu_profile",
                "modules.perm_cpu_profile_detail",
                false,
            ),
            permission("modules.perm_launch", "modules.perm_launch_detail", false),
            permission("modules.perm_quick", "", false),
            permission("modules.perm_daemon", "", false),
        ],
        quick: vec![Quick {
            id: "perfil".into(),
            label: "Perfil de CPU sugerido".into(),
            context: Context::Game,
            systems: Vec::new(),
            action: "sugerir".into(),
        }],
        ..Module::default()
    };
    let playtime = Module {
        id: "tempo-de-jogo".into(),
        name: "Tempo de jogo".into(),
        version: "0.3.0".into(),
        enabled: true,
        granted: true,
        status: Status::Failed,
        source: "kaio-dev/archr-tempo".into(),
        ..Module::default()
    };
    let backup = Module {
        id: "backup-nuvem".into(),
        name: "Backup na nuvem".into(),
        version: "0.4.1".into(),
        enabled: true,
        granted: true,
        source: "nuvem-dev/archr-backup".into(),
        pages: vec![ModPage {
            id: "main".into(),
            title: "Backup na nuvem".into(),
            detail: false,
            hint: None,
            rows: vec![
                row(
                    "Enviando saves",
                    Some("Agora"),
                    RowKind::Progress { id: "envio".into() },
                    "",
                ),
                row(
                    "",
                    Some("Últimos jogos"),
                    RowKind::List {
                        id: "jogos".into(),
                        open: None,
                        alt: None,
                    },
                    "",
                ),
            ],
        }],
        ..Module::default()
    };
    let mut view = crate::modules::View {
        modules: vec![battery, cpu, playtime, backup],
        ..Default::default()
    };
    let key = |m: &str, t: &str| (m.to_owned(), t.to_owned());
    view.values
        .insert(key("bateria-detalhada", "widget:carga"), "87 %".into());
    view.details.insert(
        key("bateria-detalhada", "widget:carga"),
        "1,9 W · cerca de 3 h 40 min".into(),
    );
    view.progress
        .insert(key("bateria-detalhada", "widget:carga"), 87);
    view.values
        .insert(key("backup-nuvem", "row:envio"), "12 de 31".into());
    view.progress.insert(key("backup-nuvem", "row:envio"), 39);
    let item = |title: &str, subtitle: &str, value: &str| crate::modules::Item {
        id: title.into(),
        title: title.into(),
        subtitle: subtitle.into(),
        value: value.into(),
        action: Some("abrir".into()),
        ..Default::default()
    };
    view.items.insert(
        key("backup-nuvem", "row:jogos"),
        vec![
            item("Super Mario World", "SNES · enviado", "há 2 h"),
            item("The Legend of Zelda", "NES · enviado", "ontem"),
            item("Castlevania: SotN", "PS1 · aguardando rede", "ontem"),
        ],
    );
    for (target, value) in [
        ("tile:bateria", "87 % · 1,9 W"),
        ("row:agora", "87 % · 1,9 W"),
        ("row:saude", "94 %"),
    ] {
        view.values
            .insert(("bateria-detalhada".into(), target.into()), value.into());
    }
    view
}
fn module_pages(dir: &std::path::Path) -> Vec<(&'static str, Settings)> {
    use crate::modules::{Card, Offer, Permission, StoreReply, Tab as ModTab};
    let view = std::sync::Arc::new(module_view());
    let mut list = ready();
    list.set_modules(view.clone());
    list.open(Page::System);
    let system = list.clone();
    list.open(Page::Modules);
    let mut none = ready();
    none.open(Page::System);
    none.open(Page::Modules);
    let mut page = list.clone();
    page.open_module_page("bateria-detalhada", 0);
    page.cursor = 2;
    let mut rich = list.clone();
    rich.open_module_page("backup-nuvem", 0);
    rich.cursor = 1;
    let mut auto = list.clone();
    auto.values.insert(Id::ModulesAuto, "1".into());
    auto.cursor = 4;
    let mut details = list.clone();
    details.cursor = 1;
    details.handle(Button::Y);
    let mut store = list.clone();
    // The store link comes after the modules and the automatic updates.
    store.cursor = view.modules.len() + 1;
    store.handle(Button::A);
    let card =
        |owner: &str, repo: &str, name: &str, description: &str, icon: &str, stars, reviewed| {
            Card {
                owner: owner.into(),
                repo: repo.into(),
                id: repo.trim_start_matches("archr-").into(),
                name: name.into(),
                description: description.into(),
                icon: icon.into(),
                version: "1.0.2".into(),
                stars,
                reviewed,
                digests: Vec::new(),
            }
        };
    let cards = vec![
        card(
            "archr-linux",
            "archr-bateria",
            "Bateria detalhada",
            "Bloco no Start com consumo e tempo restante",
            "battery-full",
            0,
            true,
        ),
        Card {
            version: "1.2.0".into(),
            ..card(
                "archr-linux",
                "archr-perfil-cpu",
                "Perfil de CPU",
                "Perfil de desempenho por jogo",
                "cpu",
                0,
                true,
            )
        },
        card(
            "nuvem-dev",
            "archr-backup",
            "Backup na nuvem",
            "Envia os saves para o seu WebDAV",
            "cloud",
            132,
            false,
        ),
        card(
            "kaio-dev",
            "archr-tempo",
            "Tempo de jogo",
            "Horas por jogo e por semana",
            "clock",
            87,
            false,
        ),
    ];
    store.module_store_reply(StoreReply::Listed {
        tab: ModTab::All,
        cards: cards.clone(),
        problem: None,
    });
    store.modules.tab = ModTab::All;
    store.module_store_reply(StoreReply::Listed {
        tab: ModTab::All,
        cards: cards.clone(),
        problem: None,
    });
    store.cursor = 2;
    let preview = dir.join("preview.png");
    image::RgbaImage::from_fn(640, 480, |x, y| {
        image::Rgba([(x / 3) as u8, 40 + (y / 4) as u8, 120, 255])
    })
    .save(&preview)
    .unwrap();
    let mut offer = store.clone();
    offer.handle(Button::A);
    let permission = |key, detail: &str, sensitive| Permission {
        key,
        detail: detail.into(),
        sensitive,
    };
    offer.module_store_reply(StoreReply::Offer(
        "nuvem-dev/archr-backup".into(),
        Ok(Box::new(Offer {
            card: cards[2].clone(),
            version: "0.4.1".into(),
            author: "nuvem-dev".into(),
            license: "MIT".into(),
            size: 180 * 1024,
            permissions: vec![
                permission("modules.perm_net", "modules.perm_net_detail", true),
                permission(
                    "modules.perm_read_screenshots",
                    "/storage/roms/screenshots",
                    true,
                ),
                permission("modules.perm_tile", "", false),
                permission("modules.perm_page", "", false),
            ],
            preview: Some(preview),
            ..Offer::default()
        })),
    ));
    let mut api2 = offer.clone();
    api2.modules.offer = None;
    api2.module_store_reply(StoreReply::Offer(
        "nuvem-dev/archr-backup".into(),
        Ok(Box::new(Offer {
            card: Card {
                name: "Perfil de CPU".into(),
                description: "Escolhe sozinho o perfil de CPU de cada jogo na hora de abrir: \
                    desempenho nos sistemas pesados, economia nos leves."
                    .into(),
                icon: "cpu".into(),
                ..cards[1].clone()
            },
            version: "2.0.0".into(),
            // A long author gives way; version, size and license stay.
            author: "ArchR Linux Project <contato@archr.dev>".into(),
            license: "MIT".into(),
            size: 24 * 1024,
            reviewed: true,
            permissions: vec![
                permission("modules.perm_config_write", "system.cpugovernor", true),
                permission(
                    "modules.perm_hooks_launch",
                    "modules.perm_hooks_launch_detail",
                    false,
                ),
                permission(
                    "modules.perm_cpu_profile",
                    "modules.perm_cpu_profile_detail",
                    false,
                ),
                permission("modules.perm_launch", "modules.perm_launch_detail", false),
                permission("modules.perm_quick", "", false),
                permission("modules.perm_daemon", "", false),
            ],
            preview: None,
            ..Offer::default()
        })),
    ));
    let mut actions = api2.clone();
    actions.modules.offer = None;
    actions.module_store_reply(StoreReply::Offer(
        "nuvem-dev/archr-backup".into(),
        Ok(Box::new(Offer {
            card: Card {
                name: "Painel rápido".into(),
                description: "Brilho, volume, Wi-Fi e captura de tela num bloco do Start.".into(),
                icon: "sun".into(),
                ..cards[1].clone()
            },
            version: "1.0.0".into(),
            author: "archr-linux".into(),
            license: "MIT".into(),
            size: 12 * 1024,
            reviewed: true,
            permissions: vec![
                permission("modules.perm_wifi", "modules.perm_wifi_detail", true),
                permission(
                    "modules.perm_brightness",
                    "modules.perm_brightness_detail",
                    false,
                ),
                permission("modules.perm_volume", "modules.perm_volume_detail", false),
                permission(
                    "modules.perm_screenshot",
                    "modules.perm_screenshot_detail",
                    false,
                ),
                permission("modules.perm_tile", "", false),
            ],
            ..Offer::default()
        })),
    ));
    let mut update = api2.clone();
    update.modules.offer = None;
    update.module_store_reply(StoreReply::Offer(
        "nuvem-dev/archr-backup".into(),
        Ok(Box::new(Offer {
            card: Card {
                name: "Perfil de CPU".into(),
                description: "Novidades: lembra o perfil que você escolheu para cada jogo e \
                    abre o jogo direto do menu Y."
                    .into(),
                icon: "cpu".into(),
                ..cards[1].clone()
            },
            version: "2.0.0".into(),
            installed: Some("1.2.0".into()),
            author: "archr-linux".into(),
            license: "MIT".into(),
            size: 24 * 1024,
            reviewed: true,
            added: vec![
                permission("modules.perm_config_write", "system.cpugovernor", true),
                permission("modules.perm_launch", "modules.perm_launch_detail", false),
            ],
            kept: vec![
                permission("modules.perm_hooks_launch", "", false),
                permission("modules.perm_cpu_profile", "", false),
            ],
            ..Offer::default()
        })),
    ));
    vec![
        ("sistema", system),
        ("loja-modulo-fase2", api2),
        ("loja-modulo-sistema", actions),
        ("loja-modulo-atualizacao", update),
        ("modulos-auto", auto),
        ("modulo-detalhes", details),
        ("pagina-rica", rich),
        ("modulos", list),
        ("modulos-vazio", none),
        ("pagina-modulo", page),
        ("loja-modulos", store),
        ("permissoes", offer),
    ]
}
#[test]
#[ignore = "manual capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_modules() {
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
    let pages = module_pages(images.path());
    struct Data;
    impl crate::state::DataSource for Data {
        fn systems(&self) -> &[archr_index::cache::SystemEntry] {
            &[]
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
    let mut hub = crate::state::App::new(8);
    hub.modules = std::sync::Arc::new(module_view());
    hub.updates = 4;
    hub.screen = crate::state::Screen::StartHub {
        row: 0,
        tile: 3,
        shortcut: 0,
    };
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
    for language in ["pt_BR", "ru", "es", "en"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in &pages {
            let deadline = Instant::now() + Duration::from_secs(5);
            ui.switches.clear();
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
            save(&mut ui, format!("{name}-{language}.png"));
        }
        ui.snap_animations();
        ui.draw(&hub, &Data, &theme, None).unwrap();
        ui.snap_animations();
        ui.draw(&hub, &Data, &theme, None).unwrap();
        save(&mut ui, format!("start-bloco-{language}.png"));
        let screen = std::mem::replace(
            &mut hub.screen,
            crate::state::Screen::StartHub {
                row: 2,
                tile: 1,
                shortcut: 0,
            },
        );
        ui.snap_animations();
        ui.draw(&hub, &Data, &theme, None).unwrap();
        ui.snap_animations();
        ui.draw(&hub, &Data, &theme, None).unwrap();
        save(&mut ui, format!("start-widget-{language}.png"));
        hub.screen = screen;
    }
}

/// Network › USB adapters, the announcement card and the save conversion
/// dialog (Rede-adaptadores, Dongle-aviso, Converter-save).
#[test]
#[ignore = "manual capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_dongle_and_saves() {
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
    let advice = "seq\t3\nadded\t0bda:b733\n\
        dev\t0bda:b733\tRealtek RTL8733BU\twifi+bt\t2.4+5\t-\t-\t-\t-\tno_driver\n";
    let seen = "dev\t2604:0013\tAIC8800DC (Tenda AX300)\twifi\t2.4\taic8800_fdrv_usb\taic8800_fdrv_usb\twlan0\t-\tworks\n\
        dev\t0e8d:7663\tMediaTek MT7663U\twifi\t2.4+5\tmt7663u\tmt7663u\t-\tmediatek/mt7663pr2h.bin\tfirmware\n\
        dev\t1286:2043\tMarvell 88W8797\twifi\t2.4+5\tmwifiex_usb\tmwifiex_usb\t-\tmrvl/usb8797_uapsta.bin\tfirmware\n";
    let table =
        "date\t2026-10-30\nchip\tRTL8821CU\twifi\t2.4+5\nchip\tRTL8812BU / 8822BU\twifi\t2.4+5\n\
        chip\tRTL8188FTV / 8188EUS\twifi\t2.4\nchip\tMT7601U\twifi\t2.4\nchip\tRTL8761B\tbt\tbt\n";
    let mut usb = ready();
    let _ = usb.open_usb();
    usb.adapters = Some(crate::usb::Adapters::new(advice, seen, table));
    let mut usb_seen = usb.clone();
    usb_seen.cursor = 2;
    let mut usb_details = usb.clone();
    usb_details.usb_details = true;
    let mut wifi = ready();
    wifi.open(Page::Wifi);
    wifi.networks = vec![network("Casa_5G", "psk", 92, true)];
    wifi.cursor = wifi.wifi_rows() - 1;
    let mut games = ready();
    games.games.replace(vec![crate::settings::GameSystem {
        name: "nds".into(),
        fullname: "Nintendo DS".into(),
        cores: vec![
            crate::settings::GameCore {
                emulator: "drastic".into(),
                core: "drastic-arch".into(),
                default: true,
                options: Vec::new(),
            },
            crate::settings::GameCore {
                emulator: "melonds".into(),
                core: "melonds-sa".into(),
                default: false,
                options: Vec::new(),
            },
        ],
        saved: [(Id::Emulator, "melonds".to_owned())].into_iter().collect(),
    }]);
    games.sync_games();
    games.open(Page::Games);
    games.cursor = 1;
    let offer = crate::convert::Offer {
        system: "nds".into(),
        from_name: "DraStic".into(),
        to_name: "melonDS".into(),
        from_ext: ".dsv".into(),
        to_ext: ".sav".into(),
        count: 12,
        keep: "roms/nds/.antes-melonds".into(),
        ..Default::default()
    };
    games.convert = Some(offer.clone());
    let mut n64 = games.clone();
    if let Some(offer) = n64.convert.as_mut() {
        offer.from_name = "Mupen64Plus-Next".into();
        offer.to_name = "Mupen64Plus".into();
        (offer.from_ext, offer.to_ext) = (".srm".into(), ".eep".into());
        offer.keep = "roms/n64/.antes-mupen64plus".into();
        (offer.count, offer.up_to, offer.replace) = (7, true, 2);
    }
    let mut later = games.clone();
    later.convert.as_mut().unwrap().on_convert = false;
    let mut converting = ready();
    converting.open(Page::Games);
    converting.converting = true;
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
    let shelf = Shelf::new();
    let languages =
        std::env::var("ARCHR_SETTINGS_CAPTURE_LANGUAGES").unwrap_or_else(|_| "pt_BR,ru".into());
    for language in languages.split(',') {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in [
            ("usb-adapters", &usb),
            ("usb-adapters-seen", &usb_seen),
            ("usb-details", &usb_details),
            ("wifi-usb-row", &wifi),
            ("convert-settings", &games),
            ("convert-later", &later),
            ("convert-n64", &n64),
            ("convert-working", &converting),
        ] {
            for _ in 0..2 {
                ui.canvas.set_draw_color(color(theme.background));
                ui.canvas.clear();
                ui.settings_page(state, &theme).unwrap();
                ui.snap_animations();
            }
            save(&mut ui, format!("{name}-{language}.png"));
        }
        let advice = crate::usb::Advice::parse(advice);
        let mut app = crate::state::App::new(10);
        app.cursor = 1;
        for (name, adapter) in [
            ("dongle-card", advice.now[0].clone()),
            ("dongle-card-works", crate::usb::parse_seen(seen)[0].clone()),
        ] {
            ui.dongle = Some(crate::usb::Card {
                adapter,
                chips: crate::usb::Table::parse(table).chips,
                since: Instant::now(),
            });
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw(&app, &shelf, &theme, None).unwrap();
            }
            save(&mut ui, format!("{name}-{language}.png"));
        }
        ui.dongle = None;
        app.convert = Some(crate::convert::Offer {
            rom: Some("Mario Kart DS.nds".into()),
            count: 1,
            ..offer.clone()
        });
        for _ in 0..2 {
            ui.snap_animations();
            ui.draw(&app, &shelf, &theme, None).unwrap();
        }
        save(&mut ui, format!("convert-game-{language}.png"));
    }
}

/// A game screen like the mockups' (sky, sun, hills, ground), to a file.
fn scene(
    path: &std::path::Path,
    (top, bottom, hill, ground): ([u8; 3], [u8; 3], [u8; 3], [u8; 3]),
) {
    image::RgbaImage::from_fn(640, 480, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let t = fy / 480.;
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
        let c = if (fx - 448.).hypot(fy - 106.) < 38. {
            [245, 240, 192]
        } else if fy > 365. {
            if (x / 62) % 2 == 0 {
                ground
            } else {
                ground.map(|v| v.saturating_sub(18))
            }
        } else if (fx - 96.).hypot(fy - 330.) < 150. || (fx - 344.).hypot(fy - 330.) < 100. {
            hill
        } else {
            [
                mix(top[0], bottom[0]),
                mix(top[1], bottom[1]),
                mix(top[2], bottom[2]),
            ]
        };
        image::Rgba([c[0], c[1], c[2], 255])
    })
    .save(path)
    .unwrap();
}

#[test]
#[ignore = "manual capture, ARCHR_RESUME_CAPTURE_DIR required"]
fn capture_resume() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_RESUME_CAPTURE_DIR").expect("capture directory"),
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
    let mut status =
        crate::status::Status::new(Some(std::path::Path::new("/nonexistent")), Instant::now());
    status.clock = "14:32".into();
    status.wifi = true;
    status.battery = Some(crate::status::Battery {
        percent: 87,
        charging: false,
    });
    ui.status = Some(status);
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
    let dir = tempfile::tempdir().unwrap();
    let pictures: Vec<_> = [
        (
            [92, 148, 252],
            [156, 196, 255],
            [63, 163, 77],
            [200, 116, 58],
        ),
        (
            [29, 59, 110],
            [77, 138, 208],
            [46, 110, 142],
            [216, 192, 112],
        ),
        ([43, 26, 74], [208, 96, 106], [90, 58, 106], [58, 42, 64]),
        ([14, 42, 26], [46, 122, 74], [30, 90, 52], [74, 58, 34]),
    ]
    .iter()
    .enumerate()
    .map(|(n, colors)| {
        let path = dir.path().join(format!("{n}.png"));
        scene(&path, *colors);
        path
    })
    .collect();
    let mut cache = crate::covers::CoverCache::new(&creator, dir.path().join("thumbs"), 640, 480);
    let deadline = Instant::now() + Duration::from_secs(5);
    while pictures.iter().any(|p| cache.peek(p).is_none()) && Instant::now() < deadline {
        cache.request(&pictures);
        cache.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let shelf = Shelf::new();
    let recent = |n: usize, picture: Option<usize>, entry: bool, ago: u64| crate::resume::Recent {
        system: "snes".into(),
        game: n,
        name: shelf.games[n].name.clone(),
        picture: picture.map(|p| pictures[p].clone()),
        left: Some(now - ago),
        entry: entry.then(|| "--entryslot=auto".into()),
    };
    let mut resume = Settings::default();
    resume.loading = false;
    resume.open(Page::Resume);
    let mut power = Settings::default();
    power.loading = false;
    power.open(Page::Power);
    power.values.insert(Id::Idle, "5".into());
    power.values.insert(Id::IdleGame, "15".into());
    power.values.insert(Id::Suspend, "off".into());
    for language in ["pt_BR", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in [("config-retomar", &resume), ("config-energia", &power)] {
            ui.switches.clear();
            for _ in 0..2 {
                ui.snap_animations();
                ui.canvas.set_draw_color(color(theme.background));
                ui.canvas.clear();
                ui.settings_page(state, &theme).unwrap();
            }
            save(&mut ui, format!("{name}-{language}.png"));
        }
        let art = crate::render::Art {
            pictures: Some(&cache),
            ..Default::default()
        };
        let mut app = crate::state::App::new(1);
        app.screen = crate::state::Screen::Switcher;
        app.switcher = vec![
            recent(4, Some(0), true, 7200),
            recent(0, Some(1), true, 20),
            recent(7, Some(2), false, 86_400 * 3),
            recent(2, None, false, 86_400 * 9),
            recent(5, Some(3), true, 86_400 * 12),
            recent(8, None, false, 86_400 * 20),
        ];
        app.cursor = 1;
        let frame = |ui: &mut Ui, app: &crate::state::App, name: &str| {
            for _ in 0..2 {
                ui.snap_animations();
                ui.draw_with_settings(app, &shelf, &theme, art, None)
                    .unwrap();
            }
            save(ui, format!("{name}-{language}.png"));
        };
        frame(&mut ui, &app, "alternador");
        app.cursor = 2;
        frame(&mut ui, &app, "alternador-sem-retomar");
        app.switcher.clear();
        frame(&mut ui, &app, "alternador-vazio");
        app.screen = crate::state::Screen::Resume;
        app.boot = Some(crate::resume::Boot {
            game: recent(0, Some(1), true, 86_400),
            ready: true,
            power_off: true,
            progress: 0.6,
        });
        frame(&mut ui, &app, "retomar-boot");
        app.boot = Some(crate::resume::Boot {
            game: recent(7, None, false, 600),
            ready: true,
            power_off: true,
            progress: 0.2,
        });
        frame(&mut ui, &app, "retomar-boot-sem-estado");
    }
}

/// The automatic CPU tuning module (examples/modules/ajuste-cpu) as it
/// shows on a device after some games, in `language`: its texts come from
/// the module's own locales (Modulo-ajuste-auto, Modulo-ajuste-jogo).
fn tune_pages(language: &str, picture: &std::path::Path) -> Vec<(&'static str, Settings)> {
    use crate::modules::{
        Item, Module, Page as ModPage, Permission, Row as ModRow, RowKind, Tone, View as ModView,
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/modules/ajuste-cpu");
    let table = |lang: &str| -> toml::Table {
        std::fs::read_to_string(root.join(format!("locales/{lang}.toml")))
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    };
    let (own, en) = (table(language), table("en"));
    let tr = |text: &str, args: &[(&str, &str)]| -> String {
        let Some(key) = text.strip_prefix('@') else {
            return text.to_owned();
        };
        let mut text = own
            .get(key)
            .or_else(|| en.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or(key)
            .to_owned();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    };
    let svg = |name: &str| {
        std::fs::read_to_string(root.join(format!("icons/{name}.svg")))
            .ok()
            .map(Into::into)
    };
    let row = |kind, label: &str, section: Option<&str>| ModRow {
        label: tr(label, &[]),
        subtitle: None,
        section: section.map(|s| tr(s, &[])),
        kind,
        default: String::new(),
    };
    let action = |name: &str, label: &str, icon: &str| {
        row(
            RowKind::Action {
                action: name.into(),
                confirm: None,
                icon: svg(icon),
            },
            label,
            None,
        )
    };
    let module = Module {
        id: "ajuste-cpu".into(),
        name: tr("@title", &[]),
        version: "1.0.0".into(),
        license: "MIT".into(),
        icon: svg("cpu"),
        enabled: true,
        granted: true,
        source: "archr-linux/archr-ajuste-cpu".into(),
        reviewed: true,
        permissions: vec![
            Permission {
                key: "modules.perm_hooks_launch",
                detail: "modules.perm_hooks_launch_detail".into(),
                sensitive: false,
            },
            Permission {
                key: "modules.perm_cpu_tune",
                detail: "modules.perm_cpu_tune_detail".into(),
                sensitive: false,
            },
            Permission {
                key: "modules.perm_page",
                detail: String::new(),
                sensitive: false,
            },
        ],
        pages: vec![
            ModPage {
                id: "main".into(),
                title: tr("@title", &[]),
                detail: false,
                hint: Some(tr("@hint.page", &[])),
                rows: vec![
                    row(
                        RowKind::Stats {
                            id: "resumo".into(),
                        },
                        "",
                        None,
                    ),
                    row(
                        RowKind::List {
                            id: "jogos".into(),
                            open: Some(1),
                            alt: Some(("reaprender".into(), tr("@hint.relearn", &[]))),
                        },
                        "",
                        Some("@section.games"),
                    ),
                ],
            },
            ModPage {
                id: "jogo".into(),
                title: tr("@title", &[]),
                detail: true,
                hint: Some(tr("@hint.detail", &[])),
                rows: vec![
                    row(
                        RowKind::Header {
                            id: "cabeca".into(),
                        },
                        "",
                        None,
                    ),
                    row(
                        RowKind::Bars {
                            id: "barras".into(),
                        },
                        "@bars.title",
                        None,
                    ),
                    ModRow {
                        default: tr("@note", &[]),
                        ..row(RowKind::Text { id: "nota".into() }, "", None)
                    },
                    row(RowKind::Facts { id: "fatos".into() }, "", None),
                    action("reaprender", "@button.relearn", "refresh-cw"),
                    action("maximo", "@button.max", "zap"),
                ],
            },
        ],
        ..Module::default()
    };
    let item = |title: &str, sub: String, value: &str, detail: String, badge: &str, tone| Item {
        id: title.to_lowercase().replace(' ', "-"),
        title: title.into(),
        subtitle: sub,
        value: value.into(),
        action: Some("detalhes".into()),
        detail,
        badge: badge.into(),
        tone,
        ..Item::default()
    };
    let learned = |system: &str, n: &str| tr("@sub.learned", &[("system", system), ("n", n)]);
    let saving = |pct: &str| tr("@detail.saving", &[("pct", pct)]);
    let games = vec![
        item(
            "Chrono Trigger",
            tr(
                "@sub.testing",
                &[("system", "Super Nintendo"), ("mhz", "816 MHz")],
            ),
            "",
            tr("@detail.session", &[("n", "2"), ("of", "3")]),
            &tr("@badge.learning", &[]),
            Tone::Info,
        ),
        item(
            "Super Mario 64",
            tr("@sub.out", &[("system", "Nintendo 64")]),
            "",
            tr("@detail.always", &[("mhz", "1416 MHz")]),
            &tr("@badge.out", &[]),
            Tone::Muted,
        ),
        item(
            "Super Mario World",
            learned("Super Nintendo", "6"),
            "816 MHz",
            saving("38"),
            "",
            Tone::Plain,
        ),
        item(
            "Pokémon Emerald",
            learned("Game Boy Advance", "4"),
            "1008 MHz",
            saving("27"),
            "",
            Tone::Plain,
        ),
        item(
            "Castlevania: SotN",
            learned("PlayStation", "3"),
            "1008 MHz",
            saving("16"),
            "",
            Tone::Plain,
        ),
    ];
    let stat = |title: &str, value: &str, subtitle: String, tone| Item {
        title: tr(title, &[]),
        value: value.into(),
        subtitle,
        tone,
        ..Item::default()
    };
    let bar = |mhz: &str, speed: u8, tone| Item {
        title: mhz.into(),
        value: format!("{speed}%"),
        progress: Some(speed),
        tone,
        ..Item::default()
    };
    let fact = |title: &str, value: &str, tone| Item {
        title: tr(title, &[]),
        value: value.into(),
        tone,
        ..Item::default()
    };
    let mut view = ModView {
        modules: vec![module],
        ..ModView::default()
    };
    let target = |t: &str| ("ajuste-cpu".to_owned(), t.to_owned());
    view.items.insert(
        target("row:resumo"),
        vec![
            stat(
                "@stat.saving",
                "−22%",
                tr("@stat.saving_sub", &[]),
                Tone::Good,
            ),
            stat(
                "@stat.learned",
                "14",
                tr("@stat.learned_sub", &[]),
                Tone::Plain,
            ),
            stat(
                "@stat.learning",
                "3",
                tr("@stat.learning_sub", &[("n", "3")]),
                Tone::Plain,
            ),
        ],
    );
    view.items.insert(target("row:jogos"), games);
    let game = ("snes".to_owned(), "snes/Super Mario World.sfc".to_owned());
    view.items.insert(
        target("row:cabeca"),
        vec![Item {
            id: "super-mario-world".into(),
            title: "Super Mario World".into(),
            subtitle: tr(
                "@sub.core",
                &[("system", "Super Nintendo"), ("core", "snes9x")],
            ),
            badge: tr("@badge.learned", &[]),
            tone: Tone::Good,
            game: Some(game.clone()),
            ..Item::default()
        }],
    );
    view.items.insert(
        target("row:barras"),
        vec![
            bar("408 MHz", 96, Tone::Bad),
            bar("816 MHz", 68, Tone::Info),
            bar("1008 MHz", 57, Tone::Plain),
            bar("1416 MHz", 44, Tone::Plain),
        ],
    );
    view.items.insert(
        target("row:fatos"),
        vec![
            fact("@fact.cap", "816 MHz", Tone::Plain),
            fact("@fact.sessions", "6", Tone::Plain),
            fact("@fact.limit", "0%", Tone::Plain),
            fact("@fact.saving", "−38%", Tone::Good),
            fact(
                "@fact.battery",
                &tr("@fact.minutes", &[("n", "40")]),
                Tone::Plain,
            ),
        ],
    );
    let view = std::sync::Arc::new(view);
    let mut list = ready();
    list.set_modules(view.clone());
    list.open(Page::System);
    list.open(Page::Modules);
    let mut auto = list.clone();
    auto.open_module_page("ajuste-cpu", 0);
    auto.cursor = 2;
    let mut learning = auto.clone();
    learning.cursor = 0;
    let mut detail = auto.clone();
    detail.modules.images.insert(game, picture.to_path_buf());
    detail.handle(Button::A);
    let mut details = list.clone();
    details.handle(Button::Y);
    vec![
        ("ajuste-auto", auto),
        ("ajuste-aprendendo", learning),
        ("ajuste-jogo", detail),
        ("ajuste-detalhes", details),
    ]
}
#[test]
#[ignore = "manual capture, ARCHR_SETTINGS_CAPTURE_DIR required"]
fn capture_tune() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let output = std::path::PathBuf::from(
        std::env::var_os("ARCHR_SETTINGS_CAPTURE_DIR").expect("capture directory"),
    );
    let (ttf, creator);
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let themes = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let theme = Theme::load(&themes.join("default"));
    let images = tempfile::tempdir().unwrap();
    ui.theme_dir = Some(themes.join("default"));
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    ui.root = Some(images.path().to_path_buf());
    // A game picture: sky, hills and ground.
    let picture = images.path().join("smw.png");
    image::RgbaImage::from_fn(256, 224, |_, y| match y {
        0..=140 => image::Rgba([92 + (y / 4) as u8, 148 + (y / 4) as u8, 252, 255]),
        141..=180 => image::Rgba([63, 163, 77, 255]),
        _ => image::Rgba([200, 116, 58, 255]),
    })
    .save(&picture)
    .unwrap();
    for language in ["pt_BR", "ru", "es", "en"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in tune_pages(language, &picture) {
            let deadline = Instant::now() + Duration::from_secs(5);
            ui.switches.clear();
            loop {
                ui.snap_animations();
                ui.canvas.set_draw_color(color(theme.background));
                ui.canvas.clear();
                ui.settings_page(&state, &theme).unwrap();
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
