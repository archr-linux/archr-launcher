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
    next_scan: Instant,
    /// Seconds shown in the Wi-Fi footer, redrawn as they change.
    scan_age: u64,
    job: Option<archr_scraper::Job>,
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
            next_scan: Instant::now(),
            scan_age: 0,
            job: None,
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
            log::warn!("Settings worker failed: {error}");
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
        self.state.pending.is_some() || self.state.connecting.is_some()
    }
    pub fn close(&mut self) {
        self.closing = true;
    }
    pub fn handle(&mut self, button: Button) -> Outcome {
        let before = self.state.page();
        match self.state.handle(button) {
            Effect::Close => self.active = false,
            Effect::Set(id, value) => self.send(Work::Set(id, value)),
            Effect::Scan => self.send(Work::Scan),
            Effect::Connect(ssid, password) => self.send(Work::Connect(ssid, password)),
            Effect::Forget(ssid) => self.send(Work::Forget(ssid)),
            Effect::Login(user, password) => self.send(Work::Login(user, password)),
            Effect::Scrape => self.send(Work::Scrape),
            Effect::CancelScrape => {
                if let Some(job) = &self.job {
                    job.cancel();
                }
            }
            Effect::Reboot => return Outcome::Action(SessionAction::Reboot),
            Effect::None => {}
        }
        if before != Page::Wifi && self.state.page() == Page::Wifi {
            self.next_scan = Instant::now();
        }
        Outcome::None
    }
    /// Engine events arrive on the engine's own thread; this never blocks.
    fn poll_scrape(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let mut changed = false;
        while let Some(event) = job.poll() {
            changed = true;
            let (progress, stop) = match event {
                archr_scraper::Event::Progress(progress) => (progress, None),
                archr_scraper::Event::Done(report) => (report.progress, Some(report.stop)),
            };
            let scrape = &mut self.state.scrape;
            scrape.system = progress.system;
            scrape.done = progress.done;
            scrape.total = progress.total;
            scrape.scraped = progress.scraped;
            scrape.missing = progress.not_found + progress.failed;
            if let Some(stop) = stop {
                scrape.running = false;
                scrape.outcome = Some(super::pages::scrape_outcome(stop));
                self.job = None;
                break;
            }
        }
        changed
    }

    pub fn poll(&mut self) -> (bool, Option<String>) {
        let (mut dirty, language) = match self.worker.as_mut() {
            Some(worker) => worker.poll(&mut self.state, &mut self.job),
            None => (false, None),
        };
        dirty |= self.poll_scrape();
        let now = Instant::now();
        let wifi = self.active
            && self.state.page() == Page::Wifi
            && self.state.value(archr_ui::settings::Id::WifiSwitch) == "1";
        if wifi && dirty && self.state.scanned.is_some() {
            self.next_scan = now + Duration::from_secs(10);
        }
        if wifi && !self.closing && !self.busy() && now >= self.next_scan {
            self.next_scan = now + Duration::from_secs(10);
            self.send(Work::Scan);
        }
        let age = self
            .state
            .scanned
            .map_or(0, |at| now.saturating_duration_since(at).as_secs());
        if wifi && age != self.scan_age {
            self.scan_age = age;
            dirty = true;
        }
        // Completion, not dispatch, starts the interval, even for a slow probe.
        if dirty {
            self.next_info = now + Duration::from_secs(2);
        }
        if !self.closing
            && !self.busy()
            && self.active
            && matches!(self.state.page(), Page::Info | Page::InfoDetail(_))
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
    fn refresh_is_disposable_uses_two_second_interval_and_never_runs_ip() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        let ip = bin.join("ip");
        fs::write(
            &ip,
            "#!/bin/sh\n/usr/bin/touch \"$ARCHR_LAUNCHER_ROOT/ip-was-run\"\n/bin/sleep 4.2\n",
        )
        .unwrap();
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
        assert!(runtime.next_info > Instant::now() + Duration::from_secs(1));
        assert!(!dir.path().join("ip-was-run").exists());
        runtime.close();
        runtime.next_info = Instant::now() - Duration::from_secs(1);
        runtime.poll();
        assert!(!runtime.busy());
    }
    fn thread_yield() {
        std::thread::sleep(Duration::from_millis(5));
    }
}
