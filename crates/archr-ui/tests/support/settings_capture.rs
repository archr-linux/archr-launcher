use super::*;
use crate::{settings::network::Network, state::Button};

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
    storage_end.storage.custom_password = true;
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
    vec![
        // First, so the list snaps to its scrolled end.
        ("storage-end", storage_end),
        ("sound-sliding", sound),
        ("wifi", wifi),
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
        ("storage", storage),
        ("storage-internal", storage_internal),
        ("storage-backup", storage_backup),
        ("storage-backup-wifi", storage_wifi),
        ("storage-reset-final", storage_reset),
        ("storage-working", storage_working),
        ("storage-password", storage_password),
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
            ui.settings_page(&state, &theme).unwrap();
            save(&mut ui, format!("{name}-{language}.png"));
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
