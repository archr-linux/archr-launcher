//! Event-loop adapter: immediate input and timed off-thread work.
use super::{Work, Worker};
use crate::{
    paths::Paths,
    state::{Button, Outcome, SessionAction},
};
use archr_ui::settings::{Effect, Id, Page, Settings};
use std::time::{Duration, Instant};
pub struct Runtime {
    pub state: Settings,
    pub active: bool,
    paths: Paths,
    worker: Option<Worker>,
    next_info: Instant,
}
impl Runtime {
    pub fn new(paths: Paths) -> Self {
        Self {
            state: Settings::default(),
            active: false,
            paths,
            worker: None,
            next_info: Instant::now(),
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
        let (dirty, language) = self
            .worker
            .as_mut()
            .map(|w| w.poll(&mut self.state))
            .unwrap_or_default();
        let now = Instant::now();
        if !self.busy() && self.active && self.state.page() == Page::Info && now >= self.next_info {
            self.next_info = now + Duration::from_secs(4);
            self.send(Work::Info);
        }
        (dirty, language)
    }
}
