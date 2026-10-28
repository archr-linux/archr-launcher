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
    let mut unavailable = ready();
    unavailable.open(Page::Games);
    unavailable.open(Page::Scraper);
    vec![
        ("wifi", wifi),
        ("password", password),
        ("forget", forget),
        ("connecting", connecting),
        ("services", services),
        ("region", region),
        ("netplay", netplay),
        ("scraper", scraper),
        ("scraper-running", running),
        ("scraper-unavailable", unavailable),
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
    for language in ["pt_BR", "ru"] {
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
