//! Theme sounds. The device is opened after the first frame, the files are
//! decoded on a worker, and playing never blocks: a sound that is not ready
//! is skipped. The device is released before any hand-off (game, tool,
//! suspend, power) so it never holds the output the next program needs.
use archr_ui::{
    settings::{Id, Page, Settings},
    sounds::{Event, Sounds},
    state::{Button, Outcome, Screen},
};
use sdl2::mixer::{self, Channel, Chunk};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// A system sound plays only after the cursor rests on it this long.
const SYSTEM_DELAY: Duration = Duration::from_millis(350);
/// Longest wait for a closing sound (opening a game) before releasing the device.
const RELEASE_WAIT: Duration = Duration::from_millis(450);
/// Repeated moves cut the previous one; system sounds and the rest have their own.
const NAVIGATION: Channel = Channel(0);
const SYSTEM: Channel = Channel(1);
const OTHER: Channel = Channel(2);

/// A decoded chunk is plain memory, handed from the loader to the owner once.
struct Loaded(Chunk);
// SAFETY: the loader thread never touches a chunk after returning it.
unsafe impl Send for Loaded {}
type Chunks = HashMap<PathBuf, Loaded>;

enum Device {
    Closed,
    Loading(JoinHandle<Chunks>),
    Ready(Chunks),
    /// The output could not be opened; the launcher stays silent.
    Mute,
}

pub struct Audio {
    sounds: Sounds,
    enabled: bool,
    volume: u8,
    device: Device,
    /// Handed off to another program: stays closed until the next input.
    released: bool,
    boot_pending: bool,
    dwell: Dwell,
}

impl Audio {
    pub fn new(sounds: Sounds, enabled: bool, volume: u8) -> Self {
        let mut audio = Self {
            sounds,
            enabled,
            volume,
            device: Device::Closed,
            released: false,
            boot_pending: false,
            dwell: Dwell::default(),
        };
        audio.open();
        audio
    }
    pub fn is_open(&self) -> bool {
        !matches!(self.device, Device::Closed | Device::Mute)
    }
    /// The settings changed (enabled switch and 0-100 volume).
    pub fn configure(&mut self, enabled: bool, volume: u8) {
        self.volume = volume.min(100);
        if enabled != self.enabled {
            self.enabled = enabled;
            if enabled {
                self.open();
            } else {
                self.close();
            }
        }
    }
    /// A different theme: its files replace the loaded ones.
    pub fn set_sounds(&mut self, sounds: Sounds) {
        if sounds != self.sounds {
            self.close();
            self.sounds = sounds;
            self.open();
        }
    }
    fn open(&mut self) {
        if !self.enabled || self.released || !matches!(self.device, Device::Closed) {
            return;
        }
        let files = self.sounds.files();
        if files.is_empty() {
            return;
        }
        if let Err(error) = mixer::open_audio(44100, mixer::AUDIO_S16LSB, 2, 1024) {
            log::warn!("Theme sounds are off: {error}");
            self.device = Device::Mute;
            return;
        }
        mixer::allocate_channels(3);
        self.device = Device::Loading(std::thread::spawn(move || {
            files
                .into_iter()
                .filter_map(|file| match Chunk::from_file(&file) {
                    Ok(chunk) => Some((file, Loaded(chunk))),
                    Err(error) => {
                        log::warn!("Cannot load sound {}: {error}", file.display());
                        None
                    }
                })
                .collect()
        }));
    }
    fn close(&mut self) {
        match std::mem::replace(&mut self.device, Device::Closed) {
            Device::Closed => return,
            Device::Mute => {}
            Device::Loading(loader) => {
                let _ = loader.join();
                mixer::close_audio();
            }
            Device::Ready(chunks) => {
                Channel::all().halt();
                drop(chunks);
                mixer::close_audio();
            }
        }
        self.boot_pending = false;
    }
    /// Before handing the screen and sound to another program: finish a
    /// closing sound (briefly), then free the output.
    pub fn release(&mut self) {
        let deadline = Instant::now() + RELEASE_WAIT;
        while matches!(self.device, Device::Ready(_))
            && OTHER.is_playing()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.close();
        self.released = true;
    }
    /// Input after a release (waking from suspend, or a cancelled hand-off).
    pub fn wake(&mut self) {
        if std::mem::take(&mut self.released) {
            self.open();
        }
    }
    pub fn play(&mut self, event: Event) {
        self.poll();
        if !self.enabled {
            return;
        }
        match (event, &self.device) {
            (Event::Boot, Device::Loading(_)) => self.boot_pending = true,
            (_, Device::Ready(chunks)) => {
                if let Some(chunk) = self.sounds.event(event).and_then(|file| chunks.get(file)) {
                    let channel = match event {
                        Event::Move | Event::Page => NAVIGATION,
                        _ => OTHER,
                    };
                    self.start(channel, &chunk.0);
                }
            }
            _ => {}
        }
    }
    fn start(&self, channel: Channel, chunk: &Chunk) {
        channel.set_volume(i32::from(self.volume) * mixer::MAX_VOLUME / 100);
        if let Err(error) = channel.play(chunk, 0) {
            log::warn!("Cannot play sound: {error}");
        }
    }
    /// The cursor rests on this system (None: not browsing systems).
    pub fn focus(&mut self, system: Option<&str>, now: Instant) {
        if self.dwell.focus(system, now) && matches!(self.device, Device::Ready(_)) {
            SYSTEM.halt();
        }
    }
    /// Collects the loaded files, then plays the boot and system sounds due.
    pub fn poll(&mut self) {
        if matches!(&self.device, Device::Loading(loader) if loader.is_finished()) {
            if let Device::Loading(loader) = std::mem::replace(&mut self.device, Device::Closed) {
                self.device = match loader.join() {
                    Ok(chunks) => Device::Ready(chunks),
                    Err(_) => {
                        mixer::close_audio();
                        Device::Mute
                    }
                };
            }
            if std::mem::take(&mut self.boot_pending) {
                self.play(Event::Boot);
            }
        }
        if let (Device::Ready(chunks), Some(system)) =
            (&self.device, self.dwell.due(Instant::now()))
        {
            if let Some(chunk) = self.sounds.system(&system).and_then(|f| chunks.get(f)) {
                self.start(SYSTEM, &chunk.0);
            }
        }
    }
    /// Longest the event loop may sleep without missing a system sound.
    pub fn wait_ms(&self, now: Instant) -> u32 {
        self.dwell.wait_ms(now)
    }
}

/// Which system the cursor rests on, and when its sound is due.
#[derive(Default)]
struct Dwell {
    /// None until the first call: the system shown at start is not announced.
    focused: Option<Option<String>>,
    due: Option<(String, Instant)>,
}

impl Dwell {
    /// True when the focus moved (a playing system sound should be cut).
    fn focus(&mut self, system: Option<&str>, now: Instant) -> bool {
        if self
            .focused
            .as_ref()
            .is_some_and(|f| f.as_deref() == system)
        {
            return false;
        }
        let first = self.focused.is_none();
        self.focused = Some(system.map(str::to_owned));
        self.due = system
            .filter(|_| !first)
            .map(|name| (name.to_owned(), now + SYSTEM_DELAY));
        true
    }
    fn due(&mut self, now: Instant) -> Option<String> {
        if self.due.as_ref().is_some_and(|(_, at)| *at <= now) {
            return self.due.take().map(|(name, _)| name);
        }
        None
    }
    fn wait_ms(&self, now: Instant) -> u32 {
        self.due.as_ref().map_or(u32::MAX, |(_, at)| {
            at.saturating_duration_since(now).as_millis().clamp(1, 1000) as u32
        })
    }
}

/// Moving between places or within one; None when the press changed nothing.
fn navigation<P: PartialEq>(
    button: Button,
    before: (&P, usize),
    after: (&P, usize),
) -> Option<Event> {
    if before.0 != after.0 {
        Some(if button == Button::B {
            Event::Back
        } else {
            Event::Select
        })
    } else if before.1 != after.1 {
        Some(if matches!(button, Button::L | Button::R) {
            Event::Page
        } else {
            Event::Move
        })
    } else {
        None
    }
}

/// The event for a press on the systems, games and quick menu screens.
pub fn browse(
    button: Button,
    before: (&Screen, usize),
    after: (&Screen, usize),
    outcome: &Outcome,
) -> Option<Event> {
    match outcome {
        Outcome::None => {}
        Outcome::Launch { .. } | Outcome::Netplay(_) | Outcome::MoonlightStream(_) => {
            return Some(Event::Launch)
        }
        Outcome::ToggleFavorite { .. } => return Some(Event::Favorite),
        _ => return Some(Event::Select),
    }
    let quick = |(screen, _): (&Screen, usize)| matches!(screen, Screen::QuickMenu { .. });
    match (before.0, after.0) {
        _ if !quick(before) && quick(after) => Some(Event::MenuOpen),
        _ if quick(before) && !quick(after) => Some(Event::MenuClose),
        (Screen::StartHub { .. }, Screen::StartHub { .. }) => {
            (before != after).then_some(Event::Move)
        }
        _ => navigation(button, before, after),
    }
}

/// What a settings press changed: place, cursor, a value in flight or a value.
#[derive(PartialEq)]
pub struct Mark {
    page: Page,
    cursor: usize,
    pending: Option<Id>,
    values: BTreeMap<Id, String>,
}

impl Mark {
    pub fn of(state: &Settings) -> Self {
        Self {
            page: state.page(),
            cursor: state.cursor,
            pending: state.pending,
            values: state.values.clone(),
        }
    }
}

pub fn settings(button: Button, before: &Mark, after: &Mark) -> Option<Event> {
    if before.pending != after.pending || before.values != after.values {
        return Some(Event::Toggle);
    }
    navigation(
        button,
        (&before.page, before.cursor),
        (&after.page, after.cursor),
    )
}

/// Screens without a model to compare (lobby, Moonlight): by button alone.
pub fn by_button(button: Button) -> Option<Event> {
    match button {
        Button::Up | Button::Down => Some(Event::Move),
        Button::L | Button::R => Some(Event::Page),
        Button::A => Some(Event::Select),
        Button::B => Some(Event::Back),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(system: &str) -> Screen {
        Screen::Games {
            system: system.into(),
        }
    }

    #[test]
    fn cues_follow_what_the_press_changed() {
        let (systems, snes) = (Screen::Systems, screen("snes"));
        let quick = Screen::QuickMenu { game: None };
        let none = Outcome::None;
        let cue = |b, from, to, o: &Outcome| browse(b, from, to, o);
        assert_eq!(
            cue(Button::Down, (&systems, 0), (&systems, 1), &none),
            Some(Event::Move)
        );
        assert_eq!(
            cue(Button::R, (&snes, 0), (&snes, 10), &none),
            Some(Event::Page)
        );
        assert_eq!(cue(Button::Down, (&systems, 3), (&systems, 3), &none), None);
        assert_eq!(
            cue(Button::A, (&systems, 0), (&snes, 0), &none),
            Some(Event::Select)
        );
        assert_eq!(
            cue(Button::B, (&snes, 4), (&systems, 0), &none),
            Some(Event::Back)
        );
        assert_eq!(
            cue(Button::Y, (&snes, 4), (&quick, 0), &none),
            Some(Event::MenuOpen)
        );
        assert_eq!(
            cue(Button::B, (&quick, 0), (&snes, 4), &none),
            Some(Event::MenuClose)
        );
        let launch = Outcome::Launch {
            system: "snes".into(),
            game: 0,
        };
        assert_eq!(
            cue(Button::A, (&snes, 4), (&snes, 4), &launch),
            Some(Event::Launch)
        );
        let favorite = Outcome::ToggleFavorite {
            system: "snes".into(),
            game: 0,
        };
        assert_eq!(
            cue(Button::X, (&snes, 4), (&snes, 4), &favorite),
            Some(Event::Favorite)
        );
    }

    #[test]
    fn settings_toggle_beats_navigation() {
        let mut state = Settings::default();
        let before = Mark::of(&state);
        state.cursor = 2;
        assert_eq!(
            settings(Button::Down, &before, &Mark::of(&state)),
            Some(Event::Move)
        );
        let moved = Mark::of(&state);
        state.pending = Some(Id::Turbo);
        assert_eq!(
            settings(Button::A, &moved, &Mark::of(&state)),
            Some(Event::Toggle)
        );
        assert_eq!(settings(Button::A, &moved, &moved), None);
    }

    #[test]
    fn system_sound_waits_for_the_cursor_to_rest() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut dwell = Dwell::default();
        // The system shown at start is not announced.
        assert!(dwell.focus(Some("nes"), start));
        assert_eq!(dwell.due(at(1000)), None);
        // Scrolling fast re-arms the delay each time: only the last one plays.
        dwell.focus(Some("snes"), at(2000));
        dwell.focus(Some("gba"), at(2200));
        assert!(!dwell.focus(Some("gba"), at(2250)));
        assert_eq!(dwell.due(at(2400)), None);
        assert_eq!(dwell.wait_ms(at(2400)), 150);
        assert_eq!(dwell.due(at(2200 + 350)), Some("gba".into()));
        assert_eq!(dwell.due(at(5000)), None);
        // Leaving the list cancels a pending sound; coming back arms it again.
        dwell.focus(Some("psx"), at(6000));
        dwell.focus(None, at(6100));
        assert_eq!(dwell.due(at(7000)), None);
        dwell.focus(Some("psx"), at(8000));
        assert_eq!(dwell.due(at(8350)), Some("psx".into()));
    }

    /// A one-sample WAV is enough for the mixer to accept the file.
    pub(crate) fn tiny_wav(path: &std::path::Path) {
        let mut wav = b"RIFF".to_vec();
        wav.extend(40u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        for field in [1u16, 1] {
            wav.extend(field.to_le_bytes());
        }
        wav.extend(22050u32.to_le_bytes());
        wav.extend(44100u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(4u32.to_le_bytes());
        wav.extend([0u8; 4]);
        std::fs::write(path, wav).unwrap();
    }

    pub(crate) fn theme_with_sounds() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sounds")).unwrap();
        for event in ["move", "launch"] {
            tiny_wav(&dir.path().join(format!("sounds/{event}.wav")));
        }
        dir
    }

    pub(crate) fn wait_ready(audio: &mut Audio) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(audio.device, Device::Ready(_)) && Instant::now() < deadline {
            audio.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(audio.device, Device::Ready(_)));
    }

    #[test]
    fn device_is_released_for_the_hand_off_and_reopened_on_input() {
        std::env::set_var("SDL_AUDIODRIVER", "dummy");
        // The shipped theme decodes completely.
        let shipped = Sounds::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default"),
        );
        let mut audio = Audio::new(shipped, true, 100);
        wait_ready(&mut audio);
        let Device::Ready(chunks) = &audio.device else {
            unreachable!()
        };
        assert_eq!(chunks.len(), Event::ALL.len());
        audio.close();
        let dir = theme_with_sounds();
        let mut audio = Audio::new(Sounds::load(dir.path()), true, 80);
        wait_ready(&mut audio);
        audio.play(Event::Move);
        assert!(NAVIGATION.is_playing());
        audio.release();
        assert!(!audio.is_open());
        assert!(mixer::query_spec().is_err(), "the output must be closed");
        audio.play(Event::Move);
        assert!(!audio.is_open(), "nothing reopens it before the next input");
        audio.wake();
        wait_ready(&mut audio);
        // Switching it off in the settings frees the output too.
        audio.configure(false, 80);
        assert!(mixer::query_spec().is_err());
        audio.configure(true, 80);
        assert!(audio.is_open());
        audio.close();
    }

    #[test]
    fn a_theme_without_sounds_never_opens_the_device() {
        let audio = Audio::new(Sounds::default(), true, 100);
        assert!(!audio.is_open());
    }
}
