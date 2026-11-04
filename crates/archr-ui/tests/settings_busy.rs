mod common;
use archr_ui::{
    motion::Motion,
    render::Ui,
    settings::{Id, Page, Settings},
    state::App,
    Theme,
};
use common::FakeData;
use std::{thread, time::Duration};

/// A control applying a change animates in place (no "Applying" text): the
/// switch's knob waits halfway and breathes, other controls show three
/// dots; it holds still and asks for no frames when motion is off.
#[test]
fn applying_controls_animate_in_place_and_hold_still_without_motion() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    let frame = |ui: &mut Ui, state: &Settings| {
        ui.draw_with_settings(
            &App::new(9),
            &FakeData::default(),
            &theme,
            Default::default(),
            Some(state),
        )
        .unwrap();
        ui.canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap()
    };
    let saved = std::env::var_os("ARCHR_BUSY_SHOTS").map(std::path::PathBuf::from);
    for (page, id) in [
        (Page::Services, Id::Ssh),
        (Page::Services, Id::UsbMode),
        (Page::Wifi, Id::WifiSwitch),
        (Page::Bluetooth, Id::BtSwitch),
    ] {
        ui.set_motion(Motion::On);
        let mut state = Settings::default();
        state.open(page);
        state.loading = false;
        let idle = frame(&mut ui, &state);
        state.pending = Some(id);
        let mut shots = Vec::new();
        for n in 0..3 {
            if let Some(dir) = &saved {
                ui.screenshot = Some(dir.join(format!("busy-{id:?}-{n}.png")));
            }
            shots.push(frame(&mut ui, &state));
            assert!(
                ui.animating(),
                "{id:?}: frames keep coming while it applies"
            );
            thread::sleep(Duration::from_millis(200));
        }
        assert!(shots[0] != idle, "{id:?}: the control shows it is working");
        assert!(
            shots[0] != shots[1] || shots[1] != shots[2],
            "{id:?}: the indicator moves"
        );
        ui.set_motion(Motion::Off);
        let still = frame(&mut ui, &state);
        assert!(!ui.animating(), "{id:?}: no frames without motion");
        thread::sleep(Duration::from_millis(200));
        assert!(still == frame(&mut ui, &state), "{id:?}: held still");
        state.pending = None;
        frame(&mut ui, &state);
        assert!(!ui.animating(), "{id:?}: done, nothing left running");
    }
}
