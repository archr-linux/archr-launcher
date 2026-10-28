//! Event-loop adapter: immediate input, lazy feedback and timed off-thread work.
use super::{Work, Worker};
use crate::{
    paths::Paths,
    state::{Button, Outcome, SessionAction},
};
use archr_config::SystemCfg;
use archr_ui::settings::{Effect, Id, Page, Settings};
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use std::time::{Duration, Instant};
pub struct Runtime {
    pub state: Settings,
    pub active: bool,
    paths: Paths,
    worker: Option<Worker>,
    next_info: Instant,
    sound: Option<AudioQueue<i16>>,
    sound_attempted: bool,
}
impl Runtime {
    pub fn new(paths: Paths, config: Option<&SystemCfg>) -> Self {
        let mut state = Settings::default();
        if let Some(value) = config.and_then(|c| c.get_setting(Id::UiSound.key(), None, None)) {
            state.values.insert(Id::UiSound, value);
        }
        Self {
            state,
            active: false,
            paths,
            worker: None,
            next_info: Instant::now(),
            sound: None,
            sound_attempted: false,
        }
    }
    fn send(&mut self, work: Work) {
        let result = (|| {
            if self.worker.is_none() {
                self.worker = Some(Worker::start(self.paths.clone())?);
            }
            self.worker.as_mut().expect("worker started").send(work)
        })();
        if let Err(error) = result {
            self.state.error = Some((self.state.pending, error));
            self.state.pending = None;
            self.state.loading = false;
        }
    }
    pub fn open(&mut self) {
        self.active = true;
        if !self.busy() {
            self.state.loading = true;
            self.send(Work::Load);
        }
    }
    pub fn busy(&self) -> bool {
        self.worker.as_ref().is_some_and(Worker::busy)
    }
    pub fn feedback(&mut self, sdl: &sdl2::Sdl) {
        if self.state.value(Id::UiSound) == "1" {
            if !self.sound_attempted {
                self.sound_attempted = true;
                self.sound = sdl.audio().ok().and_then(|a| {
                    a.open_queue(
                        None,
                        &AudioSpecDesired {
                            freq: Some(22050),
                            channels: Some(1),
                            samples: Some(512),
                        },
                    )
                    .ok()
                });
                if let Some(queue) = &self.sound {
                    queue.resume();
                }
            }
            if let Some(queue) = &self.sound {
                queue.clear();
                let pcm: Vec<i16> = (0..440)
                    .map(|i| {
                        ((i as f32 * std::f32::consts::TAU * 660. / 22050.).sin()
                            * 1400.
                            * (1. - i as f32 / 440.)) as i16
                    })
                    .collect();
                if let Err(error) = queue.queue_audio(&pcm) {
                    self.state.error = Some((Some(Id::UiSound), error));
                }
            } else {
                self.state.error = Some((Some(Id::UiSound), "settings.sound_failed".into()));
            }
        }
    }
    pub fn handle(&mut self, button: Button) -> Outcome {
        match self.state.handle(button) {
            Effect::Close => self.active = false,
            Effect::Set(id, value) => self.send(Work::Set(id, value)),
            Effect::Reboot => return Outcome::Action(SessionAction::Reboot),
            Effect::None => {}
        }
        Outcome::None
    }
    pub fn poll(&mut self) -> (bool, Option<String>) {
        let previous = self.state.pending;
        let (dirty, language) = self
            .worker
            .as_mut()
            .map(|w| w.poll(&mut self.state))
            .unwrap_or_default();
        if dirty && matches!(previous, Some(Id::Audio | Id::UiSound)) && self.state.error.is_none()
        {
            self.sound = None;
            self.sound_attempted = false;
        }
        let now = Instant::now();
        if !self.busy() && self.active && self.state.page() == Page::Info && now >= self.next_info {
            self.next_info = now + Duration::from_secs(4);
            self.send(Work::Info);
        }
        (dirty, language)
    }
}
