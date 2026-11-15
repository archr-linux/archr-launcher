//! System › Test the controller and the four calibration steps draw in
//! every language without a text the layout cannot hold.
//! ARCHR_JOYPAD_CAPTURE_DIR keeps the pictures to compare with the mockups
//! (Sistema-testar-controle, Calibrar-centro, -curso, -zona-morta, -resumo).
mod common;
use archr_ui::{
    i18n::I18n,
    motion::Motion,
    pad::Pad,
    render::Ui,
    settings::{
        joypad::{self, Input, HOLD, SAMPLE_EVERY},
        Id, Page, Settings,
    },
    state::{App, Button},
    Theme,
};
use common::FakeData;
use std::time::{Duration, Instant};

struct Warnings;
static WARNINGS: Warnings = Warnings;
static SEEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
impl log::Log for Warnings {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        let text = record.args().to_string();
        if text.contains("unavailable in this layout") {
            SEEN.lock().unwrap().push(text);
        }
    }
    fn flush(&self) {}
}

const STATUS: &str = "\
x raw=1810 center=1790 fixed=0 neg=100 pos=100 deadzone=60 max=1800 invert=0
y raw=1700 center=1796 fixed=0 neg=100 pos=100 deadzone=60 max=1800 invert=0
rx raw=1802 center=1809 fixed=0 neg=100 pos=100 deadzone=60 max=1800 invert=0
ry raw=1798 center=1795 fixed=0 neg=100 pos=100 deadzone=60 max=1800 invert=0
";
fn at(raw: [i32; 4]) -> joypad::Axes {
    let mut axes = joypad::parse(STATUS).unwrap();
    for (axis, raw) in axes.iter_mut().zip(raw) {
        axis.raw = raw;
    }
    axes
}
fn press(state: &mut Settings, pad: Pad, now: Instant) {
    state.joypad_input(Input::Button(pad, true), now);
    state.joypad_input(Input::Button(pad, false), now);
}
/// The tester as in the mockup: A held, the left stick to the right,
/// twelve controls tested.
fn tester() -> Settings {
    let mut state = Settings::default();
    state.loading = false;
    state.open(Page::System);
    state.cursor = state
        .visible_rows()
        .iter()
        .position(|r| r.id == Id::PadTestLink)
        .unwrap();
    state.handle(Button::A);
    state.joypad.device = Some(("r36s_Gamepad".into(), true));
    state.joypad_opened(joypad::parse(STATUS));
    let now = Instant::now();
    for pad in [
        Pad::B,
        Pad::X,
        Pad::Y,
        Pad::L1,
        Pad::R1,
        Pad::Start,
        Pad::Fn,
        Pad::Up,
        Pad::Down,
        Pad::Left,
        Pad::Right,
    ] {
        press(&mut state, pad, now);
    }
    state.joypad_input(Input::Button(Pad::A, true), now);
    state.joypad_input(Input::Stick(0, 25800), now);
    state.joypad_input(Input::Stick(1, -2000), now);
    state.joypad_input(Input::Stick(3, 220), now);
    state
}
/// The calibration on its first step, 1.8 s into the rest.
fn center() -> Settings {
    let mut state = tester();
    let now = Instant::now();
    state.joypad_input(Input::Button(Pad::A, false), now);
    state.joypad_input(Input::Button(Pad::Y, true), now - 2 * HOLD);
    state.joypad_tick(now - HOLD);
    state.joypad_input(Input::Button(Pad::Y, false), now);
    let mut t = now - Duration::from_millis(1800);
    for n in 0..54 {
        let wobble = [0, 9, -6, 3][n % 4];
        state.joypad_read(at([1808 + wobble, 1700 - wobble, 1803, 1797]), t);
        t += SAMPLE_EVERY;
    }
    state
}
/// The rest measured, the left stick turned three times, the right one half.
fn travel(turns: [f32; 2]) -> Settings {
    let mut state = center();
    let now = Instant::now();
    state.joypad_read(at([1808, 1700, 1803, 1797]), now + Duration::from_secs(2));
    let steps = 96;
    for step in 0..=(turns[0].max(turns[1]) * steps as f32) as usize {
        let angle = step as f32 * std::f32::consts::TAU / steps as f32;
        let (cos, sin) = (angle.cos(), angle.sin());
        let turned = |n: usize| step as f32 <= turns[n] * steps as f32;
        let left = if turned(0) {
            [
                1808 + (1640. * cos * if cos < 0. { 0.94 } else { 1. }) as i32,
                1700 + (1720. * sin) as i32,
            ]
        } else {
            [1808, 1700]
        };
        let right = if turned(1) {
            [1803 + (1770. * cos) as i32, 1797 + (1750. * sin) as i32]
        } else {
            [1803, 1797]
        };
        state.joypad_read(at([left[0], left[1], right[0], right[1]]), now);
    }
    state
}

#[test]
fn tester_and_calibration_draw_in_every_language() {
    log::set_logger(&WARNINGS).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    let temp = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("ARCHR_JOYPAD_CAPTURE_DIR")
        .map_or_else(|| temp.path().to_path_buf(), Into::into);
    let ttf;
    let creator;
    let mut ui = Ui::new(640, 480, true).unwrap();
    ttf = sdl2::ttf::init().unwrap();
    creator = ui.canvas.texture_creator();
    let theme = Theme::builtin();
    ui.configure(&ttf, &creator, &theme, None).unwrap();
    ui.set_motion(Motion::Off);
    let now = Instant::now();
    let curso = travel([3.2, 0.5]);
    assert_eq!(
        curso.joypad.calibration.as_ref().unwrap().step,
        joypad::Step::Travel
    );
    let mut zone = travel([3.2, 3.2]);
    assert_eq!(
        zone.joypad.calibration.as_ref().unwrap().step,
        joypad::Step::Deadzone
    );
    for _ in 0..4 {
        press(&mut zone, Pad::Right, now);
    }
    zone.joypad_read(at([2150, 1650, 1803, 1797]), now);
    zone.joypad_input(Input::Stick(0, 6200), now);
    zone.joypad_input(Input::Stick(1, -2900), now);
    let mut summary = zone.clone();
    press(&mut summary, Pad::A, now);
    for language in ["en", "pt_BR", "es", "ru"] {
        ui.catalog = I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
        for (name, state) in [
            ("testar-controle", tester()),
            ("calibrar-centro", center()),
            ("calibrar-curso", curso.clone()),
            ("calibrar-zona-morta", zone.clone()),
            ("calibrar-resumo", summary.clone()),
        ] {
            ui.screenshot = Some(dir.join(format!("{name}-{language}.png")));
            ui.draw_with_settings(
                &App::new(9),
                &FakeData::default(),
                &theme,
                Default::default(),
                Some(&state),
            )
            .unwrap();
        }
    }
    let seen = SEEN.lock().unwrap();
    assert!(seen.is_empty(), "{seen:#?}");
}
