//! Event-loop adapter. Disposable probes never hold the close gate.
use super::{Work, Worker};
use crate::{
    paths::Paths,
    state::{Button, Outcome, SessionAction},
};
use archr_ui::settings::{Effect, Page, Settings};
use std::time::{Duration, Instant};
pub struct Runtime {
    pub state: Settings,
    pub active: bool,
    paths: Paths,
    worker: Option<Worker>,
    next_info: Instant,
    closing: bool,
}
impl Runtime {
    pub fn new(paths: Paths) -> Self {
        Self {
            state: Settings::default(),
            active: false,
            paths,
            worker: None,
            next_info: Instant::now(),
            closing: false,
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
    pub fn mutations_pending(&self) -> bool {
        self.state.pending.is_some()
    }
    pub fn close(&mut self) {
        self.closing = true;
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
        // Completion, not dispatch, starts the interval, even for a slow probe.
        if dirty {
            self.next_info = now + Duration::from_secs(4);
        }
        if !self.closing
            && !self.busy()
            && self.active
            && self.state.page() == Page::Info
            && now >= self.next_info
        {
            self.send(Work::Info);
        }
        (dirty, language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    #[test]
    fn slow_refresh_is_disposable_and_next_interval_starts_after_completion() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        let ip = bin.join("ip");
        fs::write(&ip, "#!/bin/sh\n/bin/sleep 4.2\n").unwrap();
        fs::set_permissions(&ip, fs::Permissions::from_mode(0o755)).unwrap();
        let mut runtime = Runtime::new(Paths::new(Some(dir.path().into())));
        runtime.active = true;
        runtime.state.open(Page::Info);
        runtime.next_info = Instant::now() - Duration::from_secs(8);
        runtime.poll();
        assert!(runtime.busy());
        assert!(!runtime.mutations_pending());
        let deadline = Instant::now() + Duration::from_secs(6);
        while runtime.busy() {
            runtime.poll();
            assert!(Instant::now() < deadline);
            thread_yield();
        }
        assert!(runtime.next_info > Instant::now() + Duration::from_secs(3));
        runtime.close();
        runtime.next_info = Instant::now() - Duration::from_secs(1);
        runtime.poll();
        assert!(!runtime.busy());
    }
    fn thread_yield() {
        std::thread::sleep(Duration::from_millis(5));
    }
}
