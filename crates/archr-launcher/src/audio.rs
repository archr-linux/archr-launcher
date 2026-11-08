//! Theme sounds. The device is opened after the first frame, the files are
//! decoded on a worker, and playing never blocks: a sound that is not ready
//! is skipped. The device is released before any hand-off (game, tool,
//! suspend, power) so it never holds the output the next program needs.
//!
//! A quiet output is parked: an open SDL stream feeds silence to PipeWire
//! without end, which on the RK3326 keeps a core busy and the clock up. The
//! decoded files stay; the next press reopens the output before it is handled.
use archr_config::SystemCfg;
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
/// Quiet this long, the output is parked.
const PARK_AFTER: Duration = Duration::from_secs(5);
/// How often the loop looks for an output being opened.
const OPENING_POLL_MS: u32 = 4;
/// What the output is asked for; the chunks are converted to what it gave.
const FREQUENCY: i32 = 44100;
const CHUNK_SIZE: i32 = 1024;
/// Repeated moves cut the previous one; system sounds and the rest have their own.
const NAVIGATION: Channel = Channel(0);
const SYSTEM: Channel = Channel(1);
const OTHER: Channel = Channel(2);

/// A decoded chunk is plain memory, handed from the loader to the owner once.
struct Loaded(Chunk);
// SAFETY: the loader thread never touches a chunk after returning it.
unsafe impl Send for Loaded {}
type Chunks = HashMap<PathBuf, Loaded>;
/// Frequency, format and channels the output was opened with.
type Spec = (i32, u16, i32);

enum Device {
    Closed,
    Loading(JoinHandle<Chunks>),
    Ready(Chunks),
    /// Quiet for a while: the output is closed, the decoded files kept for
    /// the spec they were converted to.
    Parked(Chunks, Spec),
    /// Reopening a parked output on a worker: the sound server takes over
    /// 100 ms to answer, and the press that asked must not wait for it.
    Reopening(JoinHandle<Result<Spec, String>>, Chunks, Spec),
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
    /// A notice-like sound asked for while the files were still loading,
    /// or any sound asked for while the output reopens.
    queued: Option<Event>,
    dwell: Dwell,
    /// The last sound started, or the output opened.
    last: Instant,
}

impl Audio {
    pub fn new(sounds: Sounds, enabled: bool, volume: u8) -> Self {
        let mut audio = Self {
            sounds,
            enabled,
            volume,
            device: Device::Closed,
            released: false,
            queued: None,
            dwell: Dwell::default(),
            last: Instant::now(),
        };
        audio.open();
        audio
    }
    /// The output is open (or opening); parked counts as closed.
    pub fn is_open(&self) -> bool {
        matches!(
            self.device,
            Device::Loading(_) | Device::Reopening(..) | Device::Ready(_)
        )
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
        if let Err(error) = open_output() {
            log::warn!("Theme sounds are off: {error}");
            self.device = Device::Mute;
            return;
        }
        self.last = Instant::now();
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
            Device::Mute | Device::Parked(..) => {}
            Device::Reopening(opener, chunks, _) => {
                if matches!(opener.join(), Ok(Ok(_))) {
                    mixer::close_audio();
                }
                drop(chunks);
            }
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
        self.queued = None;
    }
    /// Before handing the screen and sound to another program: finish a
    /// closing sound (briefly), then free the output.
    pub fn release(&mut self) {
        // A press that reopened the output (a launch from idle) still gets
        // its sound.
        self.reopened(true);
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
    /// Any press: a parked output starts reopening before the press is
    /// handled, and the press's own sound plays once it is back. After a
    /// release (waking from suspend, or a cancelled hand-off) it opens again.
    pub fn wake(&mut self) {
        self.unpark();
        // Pressing on keeps it open, sound or not.
        if matches!(self.device, Device::Ready(_)) {
            self.last = Instant::now();
        }
        if std::mem::take(&mut self.released) {
            self.open();
        }
    }
    pub fn play(&mut self, event: Event) {
        self.unpark();
        self.poll();
        if !self.enabled {
            return;
        }
        match (event, &self.device) {
            (Event::Boot | Event::Error | Event::Notice, Device::Loading(_))
            | (_, Device::Reopening(..)) => self.queued = Some(event),
            (_, Device::Ready(chunks)) => {
                if let Some(chunk) = self.sounds.event(event).and_then(|file| chunks.get(file)) {
                    let channel = match event {
                        Event::Move | Event::Page => NAVIGATION,
                        _ => OTHER,
                    };
                    start(channel, &chunk.0, self.volume);
                    self.last = Instant::now();
                }
            }
            _ => {}
        }
    }
    /// The cursor rests on this system (None: not browsing systems).
    pub fn focus(&mut self, system: Option<&str>, now: Instant) {
        if self.dwell.focus(system, now) && matches!(self.device, Device::Ready(_)) {
            SYSTEM.halt();
        }
    }
    /// Collects the loaded files, plays the boot and system sounds due, and
    /// parks an output left quiet.
    pub fn poll(&mut self) {
        self.poll_at(Instant::now());
    }
    fn poll_at(&mut self, now: Instant) {
        // A system sound on its way (the focus moved without a press).
        if self.dwell.due.is_some() {
            self.unpark();
        }
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
            if let Some(event) = self.queued.take() {
                self.play(event);
            }
        }
        self.reopened(false);
        if let (Device::Ready(chunks), Some(system)) = (&self.device, self.dwell.due(now)) {
            if let Some(chunk) = self.sounds.system(&system).and_then(|f| chunks.get(f)) {
                start(SYSTEM, &chunk.0, self.volume);
                self.last = now;
            }
        }
        self.park(now);
    }
    /// Closes an output quiet for PARK_AFTER, keeping the decoded files.
    fn park(&mut self, now: Instant) {
        if !matches!(self.device, Device::Ready(_))
            || now.saturating_duration_since(self.last) < PARK_AFTER
            || self.dwell.due.is_some()
            || Channel::all().is_playing()
        {
            return;
        }
        // The spec the chunks were converted to, read before closing.
        let Ok(spec) = mixer::query_spec() else {
            return;
        };
        if let Device::Ready(chunks) = std::mem::replace(&mut self.device, Device::Closed) {
            mixer::close_audio();
            self.device = Device::Parked(chunks, spec);
        }
    }
    /// Starts reopening a parked output on a worker.
    fn unpark(&mut self) {
        if !matches!(self.device, Device::Parked(..)) {
            return;
        }
        if let Device::Parked(chunks, spec) = std::mem::replace(&mut self.device, Device::Closed) {
            let opener = std::thread::spawn(|| {
                open_output()?;
                mixer::query_spec()
            });
            self.device = Device::Reopening(opener, chunks, spec);
        }
    }
    /// Takes a finished reopen (or waits for it): the chunks are kept only
    /// when the output came back with their spec, otherwise the files load
    /// again. A failure (the sound server restarting) stays parked and the
    /// next press tries again.
    fn reopened(&mut self, wait: bool) {
        if !matches!(&self.device, Device::Reopening(opener, ..) if wait || opener.is_finished()) {
            return;
        }
        let Device::Reopening(opener, chunks, spec) =
            std::mem::replace(&mut self.device, Device::Closed)
        else {
            return;
        };
        match opener.join() {
            Ok(Ok(now)) if now == spec => {
                self.last = Instant::now();
                self.device = Device::Ready(chunks);
                if let Some(event) = self.queued.take() {
                    self.play(event);
                }
            }
            Ok(Ok(_)) => {
                drop(chunks);
                mixer::close_audio();
                self.open();
            }
            Ok(Err(error)) => {
                log::warn!("Cannot reopen the sound output: {error}");
                self.device = Device::Parked(chunks, spec);
                self.queued = None;
            }
            Err(_) => {
                self.device = Device::Parked(chunks, spec);
                self.queued = None;
            }
        }
    }
    /// Longest the event loop may sleep without missing a system sound or
    /// leaving an opening output (and the sound it holds) waiting.
    /// (Parking needs no wake-up of its own: the idle loop turns every 100 ms.)
    pub fn wait_ms(&self, now: Instant) -> u32 {
        match self.device {
            Device::Loading(_) | Device::Reopening(..) => {
                self.dwell.wait_ms(now).min(OPENING_POLL_MS)
            }
            _ => self.dwell.wait_ms(now),
        }
    }
}

/// Opens the output with the launcher's request and its three channels.
fn open_output() -> Result<(), String> {
    mixer::open_audio(FREQUENCY, mixer::AUDIO_S16LSB, 2, CHUNK_SIZE)?;
    mixer::allocate_channels(3);
    Ok(())
}

fn start(channel: Channel, chunk: &Chunk, volume: u8) {
    channel.set_volume(i32::from(volume) * mixer::MAX_VOLUME / 100);
    if let Err(error) = channel.play(chunk, 0) {
        log::warn!("Cannot play sound: {error}");
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
        Outcome::Launch { .. }
        | Outcome::PlayFrom { .. }
        | Outcome::Netplay(_)
        | Outcome::MoonlightStream(_) => return Some(Event::Launch),
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

/// The saved switch and 0-100 volume, each falling back to its default.
pub fn saved(cfg: Option<&SystemCfg>) -> (bool, u8) {
    let get = |id: Id| {
        cfg.and_then(|cfg| cfg.get_setting(id.key(), None, None))
            .unwrap_or_else(|| id.default_value().into())
    };
    (
        get(Id::Sounds) == "1",
        get(Id::SoundsVolume).parse().unwrap_or(60),
    )
}

/// What a settings press changed: place, cursor, a value in flight or a value.
#[derive(PartialEq)]
pub struct Mark {
    open: bool,
    page: Page,
    cursor: usize,
    pending: Option<Id>,
    values: BTreeMap<Id, String>,
}

impl Mark {
    pub fn of(state: &Settings, open: bool) -> Self {
        Self {
            open,
            page: state.page(),
            cursor: state.cursor,
            pending: state.pending,
            values: state.values.clone(),
        }
    }
}

pub fn settings(button: Button, before: &Mark, after: &Mark) -> Option<Event> {
    if before.open && !after.open {
        return Some(Event::Back);
    }
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
        let before = Mark::of(&state, true);
        state.cursor = 2;
        assert_eq!(
            settings(Button::Down, &before, &Mark::of(&state, true)),
            Some(Event::Move)
        );
        let moved = Mark::of(&state, true);
        state.pending = Some(Id::Turbo);
        assert_eq!(
            settings(Button::A, &moved, &Mark::of(&state, true)),
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
    fn tiny_wav(path: &std::path::Path) {
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

    fn theme_with_sounds() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sounds")).unwrap();
        for event in ["move", "launch"] {
            tiny_wav(&dir.path().join(format!("sounds/{event}.wav")));
        }
        dir
    }

    /// The mixer is one per process; its tests take turns.
    fn mixer_lock() -> std::sync::MutexGuard<'static, ()> {
        static MIXER: std::sync::Mutex<()> = std::sync::Mutex::new(());
        std::env::set_var("SDL_AUDIODRIVER", "dummy");
        MIXER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn wait_quiet() {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Channel::all().is_playing() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!Channel::all().is_playing());
    }

    /// Lets the output go quiet, then polls as if PARK_AFTER had passed.
    fn park(audio: &mut Audio) {
        wait_quiet();
        audio.poll_at(audio.last.max(Instant::now()) + PARK_AFTER);
        assert!(matches!(audio.device, Device::Parked(..)));
        assert!(!audio.is_open());
        assert!(mixer::query_spec().is_err(), "a parked output is closed");
    }

    /// Polls until a reopen in flight is taken.
    fn reopen(audio: &mut Audio) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while matches!(audio.device, Device::Reopening(..)) && Instant::now() < deadline {
            audio.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!matches!(audio.device, Device::Reopening(..)));
    }

    fn wait_ready(audio: &mut Audio) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(audio.device, Device::Ready(_)) && Instant::now() < deadline {
            audio.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(audio.device, Device::Ready(_)));
    }

    #[test]
    fn device_is_released_for_the_hand_off_and_reopened_on_input() {
        let _mixer = mixer_lock();
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

    #[test]
    fn a_quiet_output_parks_and_the_next_press_reopens_it() {
        let _mixer = mixer_lock();
        let dir = theme_with_sounds();
        let mut audio = Audio::new(Sounds::load(dir.path()), true, 80);
        wait_ready(&mut audio);
        audio.play(Event::Move);
        wait_quiet();
        let played = Instant::now();
        audio.poll_at(played + PARK_AFTER - Duration::from_secs(1));
        assert!(audio.is_open(), "not quiet long enough yet");
        audio.poll_at(played + PARK_AFTER);
        assert!(matches!(audio.device, Device::Parked(..)));
        assert!(mixer::query_spec().is_err());
        // A press reopens it, without waiting, with the files it had.
        audio.wake();
        assert!(matches!(audio.device, Device::Reopening(..)));
        assert!(audio.is_open());
        assert!(audio.wait_ms(Instant::now()) <= OPENING_POLL_MS);
        // Pressing on never opens it a second time: one close (the mixer
        // counts opens) frees it. The press's sound plays once it is back.
        audio.wake();
        audio.play(Event::Move);
        reopen(&mut audio);
        assert!(matches!(audio.device, Device::Ready(_)));
        assert!(NAVIGATION.is_playing(), "the press's sound waited for it");
        audio.wake();
        audio.play(Event::Move);
        park(&mut audio);
        // A sound asked for by itself (a notice) reopens it too, and plays.
        audio.play(Event::Launch);
        reopen(&mut audio);
        assert!(matches!(audio.device, Device::Ready(_)));
        assert!(OTHER.is_playing());
        // A launch from idle: the hand-off waits for the reopen and plays
        // the launch sound before freeing the output.
        park(&mut audio);
        audio.wake();
        audio.play(Event::Launch);
        audio.reopened(true); // release() starts with this
        assert!(OTHER.is_playing(), "the launch sound plays");
        audio.release();
        assert!(mixer::query_spec().is_err());
        audio.wake();
        wait_ready(&mut audio);
        // Presses without a sound keep it open.
        wait_quiet();
        audio.last = Instant::now() - PARK_AFTER;
        audio.wake();
        audio.poll();
        assert!(audio.is_open());
        // The hand-off from a parked output: closed, files freed, and only
        // the next input opens it again.
        park(&mut audio);
        audio.release();
        assert!(matches!(audio.device, Device::Closed));
        assert!(mixer::query_spec().is_err());
        audio.play(Event::Move);
        assert!(!audio.is_open());
        audio.poll_at(Instant::now() + PARK_AFTER);
        assert!(!audio.is_open());
        audio.wake();
        wait_ready(&mut audio);
        audio.close();
        assert!(mixer::query_spec().is_err(), "every open was closed");
    }

    #[test]
    fn a_parked_output_follows_the_settings_the_theme_and_the_cursor() {
        let _mixer = mixer_lock();
        let dir = theme_with_sounds();
        let mut audio = Audio::new(Sounds::load(dir.path()), true, 80);
        wait_ready(&mut audio);
        park(&mut audio);
        // The volume alone changes nothing about the output.
        audio.configure(true, 30);
        assert!(matches!(audio.device, Device::Parked(..)));
        assert_eq!(audio.volume, 30);
        // Switched off: the files go; on again: the output opens and loads.
        audio.configure(false, 30);
        assert!(matches!(audio.device, Device::Closed));
        audio.wake();
        assert!(!audio.is_open(), "off stays off");
        audio.configure(true, 30);
        wait_ready(&mut audio);
        // Another theme loads its own files.
        park(&mut audio);
        let other = theme_with_sounds();
        std::fs::create_dir(other.path().join("sounds/systems")).unwrap();
        tiny_wav(&other.path().join("sounds/systems/snes.wav"));
        audio.set_sounds(Sounds::load(other.path()));
        assert!(matches!(audio.device, Device::Loading(_)));
        wait_ready(&mut audio);
        // A system sound on its way reopens the output before it is due.
        let now = Instant::now();
        audio.focus(Some("nes"), now);
        park(&mut audio);
        audio.focus(Some("snes"), now);
        audio.poll_at(now);
        assert!(matches!(audio.device, Device::Reopening(..)));
        reopen(&mut audio);
        assert!(matches!(audio.device, Device::Ready(_)));
        audio.last = now - PARK_AFTER * 2;
        audio.poll_at(now + SYSTEM_DELAY / 2);
        assert!(audio.is_open(), "the pending system sound holds it");
        audio.poll_at(now + SYSTEM_DELAY);
        assert!(SYSTEM.is_playing());
        park(&mut audio);
        audio.close();
        assert!(mixer::query_spec().is_err());
    }

    #[test]
    fn a_reopened_output_with_another_spec_loads_the_files_again() {
        let _mixer = mixer_lock();
        let dir = theme_with_sounds();
        let mut audio = Audio::new(Sounds::load(dir.path()), true, 80);
        wait_ready(&mut audio);
        park(&mut audio);
        // Someone else holds the output at another rate: the reopen shares it.
        mixer::open_audio(22050, mixer::AUDIO_S16LSB, 2, CHUNK_SIZE).unwrap();
        audio.wake();
        reopen(&mut audio);
        assert!(matches!(audio.device, Device::Loading(_)));
        wait_ready(&mut audio);
        audio.close();
        mixer::close_audio();
        assert!(mixer::query_spec().is_err());
    }
}
