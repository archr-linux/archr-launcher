//! Event-loop adapter. Disposable probes never hold the close gate.
use super::{
    updates::{Check, Install},
    Work, Worker,
};
use crate::{
    paths::Paths,
    state::{Button, Outcome, SessionAction},
};
use archr_ui::settings::{Effect, Games, Id, Page, Settings};
use std::time::{Duration, Instant};
/// Active radio scans are not free: refresh an open Wi-Fi page once a minute.
const SCAN_INTERVAL: Duration = Duration::from_secs(60);
/// A Bluetooth scan itself listens ~10 s; this is the whole period.
const BT_INTERVAL: Duration = Duration::from_secs(30);
pub struct Runtime {
    pub state: Settings,
    pub active: bool,
    paths: Paths,
    worker: Option<Worker>,
    next_info: Instant,
    next_scan: Instant,
    next_bt_scan: Instant,
    /// Seconds shown in the Wi-Fi footer, redrawn as they change.
    scan_age: u64,
    job: Option<(archr_scraper::Job, Vec<String>)>,
    /// Systems whose art or games changed, for the library to reload.
    scraped: Option<Vec<String>>,
    /// A Games page write finished: launch choices may differ now.
    games_saved: bool,
    closing: bool,
    check: Option<Check>,
    /// A system update runs: nothing closes or powers off meanwhile.
    install: Option<Install>,
    /// `install` follows a run found going, not one started here.
    attaching: bool,
    /// That run was found: the host opens its progress screen.
    reattached: bool,
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
            next_bt_scan: Instant::now(),
            scan_age: 0,
            job: None,
            scraped: None,
            games_saved: false,
            closing: false,
            check: None,
            install: None,
            attaching: false,
            reattached: false,
        }
    }
    fn send(&mut self, work: Work) {
        let result = (|| {
            if self.worker.is_none() {
                self.worker = Some(Worker::start(self.paths.clone())?);
            }
            self.worker.as_mut().expect("worker started").send(work)
        })();
        self.sync_scanning();
        if let Err(error) = result {
            log::warn!("Settings worker failed: {error}");
            self.state.fail(self.state.pending, error);
            self.state.pending = None;
            self.state.storage.working = None;
            self.state.raimport.working = false;
            self.state.raimport.scanning = false;
            self.state.loading = false;
        }
    }
    fn sync_scanning(&mut self) {
        self.state.scanning = self.worker.as_ref().is_some_and(Worker::scanning);
        self.state.bt_scanning = self.worker.as_ref().is_some_and(Worker::bt_scanning);
    }
    pub fn open(&mut self) {
        self.active = true;
        if !self.busy() {
            self.state.loading = true;
            self.send(Work::Load);
        }
    }
    /// The Games page starts on the system the launcher was showing.
    pub fn focus(&mut self, system: &str) {
        self.state.focus_system(system);
    }
    /// The library re-reads system.cfg after a Games page write.
    pub fn take_games_saved(&mut self) -> bool {
        std::mem::take(&mut self.games_saved)
    }
    pub fn busy(&self) -> bool {
        self.worker.as_ref().is_some_and(Worker::busy)
    }
    pub fn mutations_pending(&self) -> bool {
        self.state.pending.is_some()
            || self.state.connecting.is_some()
            || self.state.storage.working.is_some()
            || self.state.cheevos.verifying
            || self.state.store.working.is_some()
            || self.state.raimport.working
            || !self.state.slides.is_empty()
            || self.install.is_some()
    }
    /// archr-update check in the background (also the one at startup).
    pub fn check_updates(&mut self) {
        if self.check.is_none() && self.install.is_none() {
            self.state.updates.checking = true;
            self.check = Some(Check::start(self.paths.clone()));
        }
    }
    /// Follows an update already running (started before this launcher, or
    /// from a shell); `take_reattached` tells when one was found.
    pub fn watch_update(&mut self) {
        if self.install.is_none() {
            self.install = Some(Install::attach(&self.paths));
            self.attaching = true;
        }
    }
    pub fn take_reattached(&mut self) -> bool {
        std::mem::take(&mut self.reattached)
    }
    /// The START menu's update shortcut.
    pub fn open_updates(&mut self) {
        self.open();
        self.state.open_updates();
    }
    fn poll_updates(&mut self) -> bool {
        let mut changed = false;
        if let Some(result) = self.check.as_ref().and_then(Check::poll) {
            self.check = None;
            // "busy": a run is going (perhaps from a shell): follow it.
            if result.as_ref().err().is_some_and(|reason| reason == "busy") {
                self.watch_update();
            }
            super::updates::checked(&mut self.state.updates, result);
            changed = true;
        }
        if let Some(install) = &self.install {
            let (dirty, ended) = install.poll(&mut self.state.updates);
            changed |= dirty;
            if self.attaching && self.state.updates.run.is_some() {
                self.attaching = false;
                self.reattached = true;
            }
            if ended {
                self.install = None;
                self.attaching = false;
                // A cancelled download goes back to the page with a note.
                if matches!(
                    &self.state.updates.run,
                    Some(archr_ui::settings::updates::Run::Failed { reason, .. }) if reason == "cancelled"
                ) {
                    self.state.updates.run = None;
                    self.state.fail(None, "updates.cancelled".into());
                }
            }
        }
        // Nothing known yet about an open Updates page: look.
        if self.active
            && !self.closing
            && self.state.page() == Page::Updates
            && self.state.pending.is_none()
            && self.state.updates.wants_check()
        {
            self.check_updates();
            changed = true;
        }
        changed
    }
    pub fn close(&mut self) {
        self.closing = true;
    }
    /// The press an open "press a button" prompt waited for.
    pub fn capture(&mut self, pad: archr_ui::pad::Pad) {
        let effect = self.state.captured(pad);
        self.apply(effect);
    }
    pub fn handle(&mut self, button: Button) -> Outcome {
        let before = self.state.page();
        let effect = self.state.handle(button);
        let outcome = self.apply(effect);
        if before != Page::Wifi && self.state.page() == Page::Wifi {
            self.next_scan = Instant::now();
        }
        if before != Page::Bluetooth && self.state.page() == Page::Bluetooth {
            self.next_bt_scan = Instant::now();
        }
        if before != Page::Achievements
            && self.state.page() == Page::Achievements
            && !self.state.cheevos.user.is_empty()
        {
            self.send(Work::CheevosProfile);
        }
        outcome
    }
    fn apply(&mut self, effect: Effect) -> Outcome {
        match effect {
            Effect::Close => self.active = false,
            Effect::Set(id, value) if Games::ROWS.contains(&id) => {
                let system = self.state.games.name().to_owned();
                self.send(Work::Game(system, id, value));
            }
            Effect::Set(id, value) => self.send(Work::Set(id, value)),
            // Radio scans: on entry, on X, and every minute while open.
            Effect::Scan => {
                let now = Instant::now();
                (self.next_scan, self.next_bt_scan) = (now, now);
            }
            Effect::Bluetooth(action, addr) => self.send(Work::Bt(action, addr)),
            Effect::Connect(ssid, password) => self.send(Work::Connect(ssid, password)),
            Effect::Forget(ssid) => self.send(Work::Forget(ssid)),
            Effect::Login(user, password) => self.send(Work::Login(user, password)),
            Effect::CheevosLogin(user, password) => self.send(Work::CheevosLogin(user, password)),
            Effect::CheevosLogout => self.send(Work::CheevosLogout),
            Effect::CheevosKey(key) => self.send(Work::CheevosKey(key)),
            Effect::Scrape if self.job.is_some() => self.state.scrape.starting = false,
            Effect::Scrape => self.send(Work::Scrape),
            Effect::CancelScrape => {
                if let Some((job, _)) = &self.job {
                    job.cancel();
                }
            }
            Effect::Theme(name) => self.send(Work::Theme(name)),
            Effect::Appearance => {
                let look = self.state.appearance();
                self.send(Work::Appearance(vec![
                    (Id::Theme, look.theme),
                    (Id::View, look.view),
                    (Id::Accent, look.accent),
                    (Id::Background, look.background),
                    (Id::Font, look.font),
                    (Id::TextSize, look.text_size),
                    (Id::Animations, look.animations.code().into()),
                ]))
            }
            Effect::Storage(action) => self.send(Work::Storage(action)),
            Effect::Store(action) => self.send(Work::Store(action)),
            Effect::CheckUpdates => self.check_updates(),
            Effect::Update if self.install.is_none() && self.check.is_none() => {
                self.install = Some(Install::start(&self.paths));
            }
            // A check still running: the page shows it; press again after.
            Effect::Update => self.state.updates.run = None,
            Effect::CancelUpdate => {
                if let Some(install) = &self.install {
                    install.cancel();
                }
            }
            Effect::RaImport { backup, seen } => self.send(Work::RaImport(backup, seen)),
            Effect::Module(request) => return Outcome::Module(request),
            Effect::Reboot => return Outcome::Action(SessionAction::Reboot),
            Effect::None => {}
        }
        Outcome::None
    }
    /// Engine events arrive on the engine's own thread; this never blocks.
    fn poll_scrape(&mut self) -> bool {
        let Some((job, _)) = &self.job else {
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
                // Cancelled and failed runs may still have saved some art.
                self.scraped = self.job.take().map(|(_, systems)| systems);
                break;
            }
        }
        changed
    }

    /// Systems to reload once a scraping run has ended.
    pub fn take_scraped(&mut self) -> Option<Vec<String>> {
        self.scraped.take()
    }
    pub fn poll(&mut self) -> (bool, Option<String>) {
        let writing = self.state.pending.filter(|id| Games::ROWS.contains(id));
        let (mut dirty, language) = match self.worker.as_mut() {
            Some(worker) => worker.poll(&mut self.state, &mut self.job),
            None => (false, None),
        };
        self.games_saved |= writing.is_some() && self.state.pending.is_none();
        dirty |= self.poll_scrape();
        dirty |= self.poll_updates();
        // Nothing known yet about an open import page: look.
        if self.active
            && !self.closing
            && self.state.page() == Page::RaImport
            && self.state.raimport.wants_scan()
        {
            self.state.raimport.scanning = true;
            self.send(Work::RaScan);
            dirty = true;
        }
        let now = Instant::now();
        let wifi = self.active
            && self.state.page() == Page::Wifi
            && self.state.value(archr_ui::settings::Id::WifiSwitch) == "1";
        if wifi && !self.closing && !self.state.scanning && now >= self.next_scan {
            self.next_scan = now + SCAN_INTERVAL;
            self.send(Work::Scan);
            dirty = true;
        }
        let bluetooth = self.active
            && self.state.page() == Page::Bluetooth
            && self.state.value(archr_ui::settings::Id::BtSwitch) == "1";
        if bluetooth
            && !self.closing
            && !self.state.bt_scanning
            && self.state.connecting.is_none()
            && now >= self.next_bt_scan
        {
            self.next_bt_scan = now + BT_INTERVAL;
            self.send(Work::BtScan);
            dirty = true;
        }
        self.sync_scanning();
        let scanned = if bluetooth {
            self.state.bt_scanned
        } else {
            self.state.scanned
        };
        let age = scanned.map_or(0, |at| now.saturating_duration_since(at).as_secs());
        if (wifi || bluetooth) && age != self.scan_age {
            self.scan_age = age;
            dirty = true;
        }
        // Completion, not dispatch, starts the interval, even for a slow probe.
        if dirty {
            self.next_info = now + Duration::from_secs(2);
        }
        if !self.closing && !self.busy() && self.active && now >= self.next_info {
            match self.state.page() {
                Page::Info | Page::InfoDetail(_) => self.send(Work::Info),
                Page::Services if self.state.usb_network() => self.send(Work::Usb),
                _ => {}
            }
        }
        (dirty, language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archr_ui::settings::Id;
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
    #[test]
    fn a_slow_scan_never_delays_the_wifi_switch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "wifi.enabled=1\n").unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        let wifictl = bin.join("wifictl");
        fs::write(
            &wifictl,
            "#!/bin/sh\n[ \"$1\" = scanlist ] && /bin/sleep 3\nexit 0\n",
        )
        .unwrap();
        fs::set_permissions(&wifictl, fs::Permissions::from_mode(0o755)).unwrap();
        let mut runtime = Runtime::new(paths);
        runtime.active = true;
        runtime.state.open(Page::Wifi);
        runtime.state.values.insert(Id::WifiSwitch, "1".into());
        runtime.poll();
        assert!(runtime.state.scanning, "the scan started");
        runtime.send(Work::Set(Id::WifiSwitch, "0".into()));
        let began = Instant::now();
        while runtime.state.value(Id::WifiSwitch) != "0" {
            runtime.poll();
            assert!(
                began.elapsed() < Duration::from_millis(1500),
                "switch waited"
            );
            thread_yield();
        }
        assert!(runtime.state.scanning, "the scan is still running");
        // Its late, failed or successful reply neither raises an error nor lists networks.
        while runtime.state.scanning {
            runtime.poll();
            thread_yield();
        }
        assert!(runtime.state.error.is_none() && runtime.state.networks.is_empty());
        runtime.poll();
        assert!(!runtime.state.scanning, "no scan after switching off");
    }
    /// wifictl whose first scan is slow and lists "Stale"; later ones list "Fresh".
    fn scan_fixture() -> (tempfile::TempDir, Runtime) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "wifi.enabled=1\n").unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        let wifictl = bin.join("wifictl");
        fs::write(
            &wifictl,
            "#!/bin/sh\n[ \"$1\" = scanlist ] || exit 0\nn=$(/bin/cat \"$ARCHR_LAUNCHER_ROOT/n\" 2>/dev/null || echo 0)\n\
             echo $((n + 1)) > \"$ARCHR_LAUNCHER_ROOT/n\"\n\
             if [ $n = 0 ]; then /bin/sleep 1; printf 'Stale\\twpa2\\t50\\t0\\t0\\n'; else printf 'Fresh\\twpa2\\t50\\t0\\t0\\n'; fi\n",
        )
        .unwrap();
        fs::set_permissions(&wifictl, fs::Permissions::from_mode(0o755)).unwrap();
        let mut runtime = Runtime::new(paths);
        runtime.active = true;
        runtime.state.open(Page::Wifi);
        runtime.state.values.insert(Id::WifiSwitch, "1".into());
        runtime.poll();
        assert!(runtime.state.scanning, "the scan started");
        (dir, runtime)
    }
    fn ssids(runtime: &Runtime) -> Vec<&str> {
        runtime
            .state
            .networks
            .iter()
            .map(|n| n.ssid.as_str())
            .collect()
    }
    #[test]
    fn a_scan_requested_during_a_scan_runs_when_it_ends() {
        let (_dir, mut runtime) = scan_fixture();
        // Connect or forget finished meanwhile and asks for a fresh list.
        runtime.send(Work::Scan);
        let deadline = Instant::now() + Duration::from_secs(5);
        while ssids(&runtime) != ["Fresh"] {
            runtime.poll();
            assert!(
                Instant::now() < deadline,
                "no second scan: {:?}",
                ssids(&runtime)
            );
            thread_yield();
        }
    }
    #[test]
    fn a_scan_started_before_a_wifi_toggle_is_discarded() {
        let (_dir, mut runtime) = scan_fixture();
        runtime.send(Work::Set(Id::WifiSwitch, "0".into()));
        runtime.send(Work::Set(Id::WifiSwitch, "1".into()));
        let deadline = Instant::now() + Duration::from_secs(5);
        while ssids(&runtime) != ["Fresh"] {
            runtime.poll();
            assert!(!ssids(&runtime).contains(&"Stale"), "stale list shown");
            assert!(runtime.state.error.is_none());
            assert!(Instant::now() < deadline, "no fresh scan");
            thread_yield();
        }
    }
    #[test]
    fn games_page_writes_the_focused_system_and_flags_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        let catalog = paths.at("/usr/share/archr/systems.cfg");
        fs::create_dir_all(catalog.parent().unwrap()).unwrap();
        fs::write(
            &catalog,
            "<systemList>\
             <system><name>nes</name><fullname>NES</fullname><path>/r/nes</path><emulators>\
             <emulator name=\"retroarch\"><cores><core>fceumm</core></cores></emulator>\
             </emulators></system>\
             <system><name>snes</name><fullname>Super Nintendo</fullname><path>/r/snes</path>\
             <emulators><emulator name=\"retroarch\"><cores><core>snes9x</core>\
             <core>bsnes</core></cores></emulator></emulators></system></systemList>",
        )
        .unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "").unwrap();
        let mut runtime = Runtime::new(Paths::new(Some(dir.path().into())));
        runtime.focus("snes");
        runtime.open();
        let deadline = Instant::now() + Duration::from_secs(10);
        while runtime.state.loading || runtime.busy() {
            runtime.poll();
            assert!(Instant::now() < deadline);
            thread_yield();
        }
        runtime.state.open(Page::Games);
        assert_eq!(runtime.state.games.name(), "snes");
        runtime.state.cursor = runtime
            .state
            .visible_rows()
            .iter()
            .position(|row| row.id == Id::Core)
            .unwrap();
        runtime.handle(Button::R);
        while runtime.state.pending.is_some() {
            runtime.poll();
            assert!(Instant::now() < deadline);
            thread_yield();
        }
        assert_eq!(runtime.state.value(Id::Core), "bsnes");
        assert_eq!(
            fs::read_to_string(&paths.config).unwrap(),
            "snes.core=bsnes\n"
        );
        assert!(runtime.take_games_saved());
        assert!(!runtime.take_games_saved());
    }
    #[test]
    fn the_installed_tab_answers_from_the_store_lane_without_the_network() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        let theme = paths.at("/storage/.config/archr-launcher/themes/kaio-dev-neon");
        fs::create_dir_all(&theme).unwrap();
        fs::write(theme.join("theme.toml"), "name = \"Neon\"\n").unwrap();
        fs::write(
            theme.join(".archr-store.toml"),
            "owner = \"kaio-dev\"\nrepo = \"neon\"\nbranch = \"main\"\n\
             reference = \"v1\"\ntree = \"t1\"\ndate = \"2026-10-12\"\nstars = 3\n",
        )
        .unwrap();
        let mut runtime = Runtime::new(paths);
        runtime.active = true;
        runtime.state.loading = false;
        runtime.state.open(Page::Appearance);
        runtime.state.store.tab = archr_ui::settings::store::Tab::Installed;
        runtime.state.cursor = 1;
        runtime.handle(Button::A);
        assert_eq!(runtime.state.page(), Page::Store);
        let deadline = Instant::now() + Duration::from_secs(5);
        while runtime.state.store.loading {
            runtime.poll();
            assert!(Instant::now() < deadline, "no answer");
            thread_yield();
        }
        let cards = runtime.state.store.cards();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title("en"), "Neon");
        assert!(runtime.state.options[&Id::Theme].contains(&"kaio-dev-neon".to_owned()));
        assert!(
            !runtime.busy(),
            "the store lane never holds the settings worker"
        );
    }
    fn thread_yield() {
        std::thread::sleep(Duration::from_millis(5));
    }
}
