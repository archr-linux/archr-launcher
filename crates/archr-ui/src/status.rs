//! Cached status: only the status worker touches sysfs. It starts after
//! present; the render thread consumes a typed snapshot without waiting.
//! Battery is re-read every 2 s, the radio every 30 s; only changes publish.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Charger plug/unplug must show promptly; two small sysfs reads every 2 s.
const BATTERY_REFRESH: Duration = Duration::from_secs(2);
/// Everything else (the Wi-Fi radio) is refreshed slowly or on an explicit signal.
const REFRESH: Duration = Duration::from_secs(30);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}
/// Off-thread acquisition shared by the status pill and information cards.
pub fn read_battery(root: &Path) -> Option<Battery> {
    battery_at(&find_battery(root)?)
}
/// The first power supply named `battery` or of type `Battery`, by name order.
pub fn find_battery(root: &Path) -> Option<PathBuf> {
    let base = root.join("sys/class/power_supply");
    let mut sources: Vec<_> = fs::read_dir(base).ok()?.filter_map(Result::ok).collect();
    sources.sort_by_key(|e| e.file_name());
    sources.into_iter().map(|e| e.path()).find(|p| {
        p.file_name().is_some_and(|v| v == "battery")
            || archr_io::read_utf8(&p.join("type"), 4096, archr_io::Symlinks::Follow)
                .is_ok_and(|s| s.trim() == "Battery")
    })
}
fn battery_at(path: &Path) -> Option<Battery> {
    let percent = archr_io::read_utf8(&path.join("capacity"), 4096, archr_io::Symlinks::Follow)
        .ok()?
        .trim()
        .parse::<u8>()
        .ok()?;
    let charging = archr_io::read_utf8(&path.join("status"), 4096, archr_io::Symlinks::Follow)
        .is_ok_and(|s| s.trim() == "Charging");
    (percent <= 100).then_some(Battery { percent, charging })
}
/// Polled every 2 s: the directory is found once and searched for again only
/// after a failed read, so a steady tick costs exactly two small reads.
struct BatterySource {
    root: PathBuf,
    dir: Option<PathBuf>,
    next_read: Instant,
}
impl BatterySource {
    fn new(root: PathBuf, now: Instant) -> Self {
        Self {
            root,
            dir: None,
            next_read: now,
        }
    }
    fn poll(&mut self, now: Instant) -> Option<Option<Battery>> {
        if now < self.next_read {
            return None;
        }
        self.next_read = now + BATTERY_REFRESH;
        if let Some(battery) = self.dir.as_deref().and_then(battery_at) {
            return Some(Some(battery));
        }
        self.dir = find_battery(&self.root);
        Some(self.dir.as_deref().and_then(battery_at))
    }
}
#[derive(Clone, Copy, Default, PartialEq)]
struct Snapshot {
    battery: Option<Battery>,
    wifi: bool,
}
enum Signal {
    Refresh,
    Stop,
}
struct Worker {
    value: Arc<Mutex<Snapshot>>,
    stop: mpsc::Sender<Signal>,
    clock_12: Arc<AtomicBool>,
}
impl Worker {
    fn start(path: PathBuf) -> std::io::Result<Self> {
        let value = Arc::new(Mutex::new(Snapshot::default()));
        let published = value.clone();
        let (stop, stopped) = mpsc::channel();
        let clock_12 = Arc::new(AtomicBool::new(false));
        let worker_clock = clock_12.clone();
        thread::Builder::new()
            .name("launcher-status".into())
            .spawn(move || {
                let config = archr_io::read_utf8(
                    &path.join("storage/.config/system/configs/system.cfg"),
                    1024 * 1024,
                    archr_io::Symlinks::Follow,
                )
                .unwrap_or_default();
                worker_clock.store(
                    config
                        .lines()
                        .any(|line| line.trim() == "launcher.clock.format=12"),
                    Ordering::Release,
                );
                let mut source = BatterySource::new(path.clone(), Instant::now());
                let mut snapshot = Snapshot::default();
                let mut next_wifi = Instant::now();
                loop {
                    let now = Instant::now();
                    let mut next = snapshot;
                    if let Some(battery) = source.poll(now) {
                        next.battery = battery;
                    }
                    if now >= next_wifi {
                        next.wifi = wifi_on(&path);
                        next_wifi = now + REFRESH;
                    }
                    // Publish only real changes; the render thread redraws on them.
                    if next != snapshot {
                        snapshot = next;
                        *published.lock().expect("status snapshot") = snapshot;
                    }
                    let wake = source.next_read.min(next_wifi);
                    match stopped.recv_timeout(wake.saturating_duration_since(Instant::now())) {
                        Ok(Signal::Refresh) => next_wifi = Instant::now(),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Ok(Signal::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            value,
            stop,
            clock_12,
        })
    }
    fn refresh_signal(&self) -> impl FnOnce() + Send + 'static {
        let sender = self.stop.clone();
        move || {
            let _ = sender.send(Signal::Refresh);
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Wake a sleeping worker. Never join a potentially slow sysfs read on
        // the presentation thread; a read already in progress finishes there.
        let _ = self.stop.send(Signal::Stop);
    }
}
pub struct Status {
    pub battery: Option<Battery>,
    pub wifi: bool,
    pub clock: String,
    path: PathBuf,
    worker: Option<Worker>,
    started: bool,
    minute: u64,
    clock_12: bool,
}
impl Status {
    pub fn new(root: Option<&Path>, now: Instant) -> Self {
        let path = root.map_or_else(|| PathBuf::from("/"), Path::to_owned);
        let mut status = Self {
            battery: None,
            wifi: false,
            clock: String::new(),
            path,
            worker: None,
            started: false,
            minute: u64::MAX,
            clock_12: false,
        };
        status.poll(now);
        status
    }
    /// The clock reads 12-hour (AM/PM).
    pub fn clock_12(&self) -> bool {
        self.clock_12
    }
    /// Call only after the first frame has been presented. Starting is idempotent.
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        match Worker::start(self.path.clone()) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => log::warn!("Cannot start status worker: {error}"),
        }
    }
    pub fn refresh_signal(&self) -> Option<impl FnOnce() + Send + 'static> {
        self.worker.as_ref().map(Worker::refresh_signal)
    }
    pub fn poll(&mut self, _now: Instant) -> bool {
        let snapshot = self
            .worker
            .as_ref()
            .and_then(|w| w.value.try_lock().ok().map(|s| *s));
        if let Some(worker) = &self.worker {
            let mode = worker.clock_12.load(Ordering::Acquire);
            if mode != self.clock_12 {
                self.clock_12 = mode;
                self.invalidate_clock();
            }
        }
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.update(unix, snapshot)
    }
    /// A successful settings publish changes the live pill on the next frame.
    pub fn set_clock_format(&mut self, format: &str) -> bool {
        let next = format == "12";
        if self.clock_12 == next {
            return false;
        }
        self.clock_12 = next;
        if let Some(worker) = &self.worker {
            worker.clock_12.store(next, Ordering::Release);
        }
        self.invalidate_clock();
        true
    }
    /// Recompute even within the same minute after a timezone change.
    pub fn invalidate_clock(&mut self) {
        self.minute = u64::MAX;
    }
    fn update(&mut self, unix: u64, snapshot: Option<Snapshot>) -> bool {
        let mut changed = false;
        if let Some(Snapshot { battery, wifi }) = snapshot {
            changed = self.battery != battery || self.wifi != wifi;
            self.battery = battery;
            self.wifi = wifi;
        }
        if unix / 60 != self.minute {
            self.minute = unix / 60;
            let clock = local_clock(unix, self.clock_12);
            changed |= self.clock != clock;
            self.clock = clock;
        }
        changed
    }
}
/// Radio state is the wlan rfkill soft block, which `wifictl` flips at once; link
/// flags would lag behind it and depend on the interface name.
pub fn wifi_on(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root.join("sys/class/rfkill")) else {
        return false;
    };
    let read = |dir: &Path, name: &str| {
        archr_io::read_utf8(&dir.join(name), 4096, archr_io::Symlinks::Follow).unwrap_or_default()
    };
    entries
        .flatten()
        .any(|e| read(&e.path(), "type").trim() == "wlan" && read(&e.path(), "soft").trim() == "0")
}
/// Broken-down local time of a Unix timestamp.
pub fn local_time(unix: u64) -> Option<libc::tm> {
    let seconds = unix as libc::time_t;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers reference correctly aligned live allocations;
    // localtime_r writes a complete tm on success and retains neither pointer.
    let result = unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) };
    // SAFETY: a non-null return confirms the output was initialized.
    (!result.is_null()).then(|| unsafe { local.assume_init() })
}
pub fn local_clock(unix: u64, clock_12: bool) -> String {
    let Some(local) = local_time(unix) else {
        return "--:--".to_owned();
    };
    if clock_12 {
        let hour = (local.tm_hour + 11) % 12 + 1;
        format!(
            "{hour:02}:{:02} {}",
            local.tm_min,
            if local.tm_hour < 12 { "AM" } else { "PM" }
        )
    } else {
        format!("{:02}:{:02}", local.tm_hour, local.tm_min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_format_changes_cached_pill_without_waiting_a_minute() {
        let mut status = Status::new(None, Instant::now());
        let unix = 1_700_000_040;
        status.update(unix, None);
        assert!(status.set_clock_format("12"));
        assert!(status.update(unix, None));
        assert!(status.clock.contains("AM") || status.clock.contains("PM"));
        assert!(!status.set_clock_format("12"));
    }

    #[test]
    fn timezone_change_invalidates_a_clock_cached_within_the_minute() {
        let mut status = Status::new(None, Instant::now());
        let unix = 1_700_000_040;
        status.minute = unix / 60;
        status.clock = "stale timezone".into();
        assert!(!status.update(unix, None));
        status.invalidate_clock();
        assert!(status.update(unix, None));
        assert_ne!(status.clock, "stale timezone");
    }

    #[test]
    fn redraw_only_when_displayed_clock_or_snapshot_changes() {
        let mut status = Status::new(None, Instant::now());
        let unix = 1_700_000_040;
        let empty = Some(Snapshot::default());
        status.update(unix, empty);
        assert!(!status.update(unix + 59, empty));
        assert!(status.update(unix + 60, empty));
        assert!(!status.update(unix + 60, empty));
        for (percent, charging, wifi, changed) in [
            (Some(87), false, false, true),
            (Some(87), false, false, false),
            (Some(86), false, false, true),
            (Some(86), true, false, true),
            (Some(86), true, true, true),
            (Some(86), true, true, false),
            (Some(86), false, true, true),
            (Some(86), false, false, true),
            (None, false, false, true),
            (None, false, false, false),
        ] {
            let snapshot = Snapshot {
                battery: percent.map(|percent| Battery { percent, charging }),
                wifi,
            };
            assert_eq!(status.update(unix + 60, Some(snapshot)), changed);
            assert!(
                !status.update(unix + 60, None),
                "an unavailable snapshot retains displayed status"
            );
        }
    }

    fn radio(root: &Path, name: &str, kind: &str, soft: &str) {
        let dir = root.join("sys/class/rfkill").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("type"), format!("{kind}\n")).unwrap();
        fs::write(dir.join("soft"), format!("{soft}\n")).unwrap();
    }

    #[test]
    fn wifi_follows_the_wlan_soft_block_under_the_root() {
        let root = tempfile::tempdir().unwrap();
        assert!(!wifi_on(root.path()));
        radio(root.path(), "rfkill0", "bluetooth", "0");
        assert!(!wifi_on(root.path()));
        radio(root.path(), "rfkill1", "wlan", "1");
        assert!(!wifi_on(root.path()));
        radio(root.path(), "rfkill1", "wlan", "0");
        assert!(wifi_on(root.path()));
    }

    #[test]
    fn refresh_signal_reads_wifi_without_waiting_for_periodic_poll() {
        let root = tempfile::tempdir().unwrap();
        radio(root.path(), "rfkill0", "wlan", "1");
        let battery = root.path().join("sys/class/power_supply/battery");
        fs::create_dir_all(&battery).unwrap();
        fs::write(battery.join("capacity"), "67").unwrap();
        let mut status = Status::new(Some(root.path()), Instant::now());
        status.start();
        let deadline = Instant::now() + Duration::from_secs(2);
        // The first read is complete once the battery is published; the next
        // change is then attributable to the explicit signal.
        while status.battery.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
            status.poll(Instant::now());
        }
        assert!(status.battery.is_some() && !status.wifi);
        radio(root.path(), "rfkill0", "wlan", "0");
        status.refresh_signal().unwrap()();
        while !status.wifi && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
            status.poll(Instant::now());
        }
        assert!(status.wifi);
    }

    #[test]
    fn charger_changes_reach_the_pill_within_one_battery_period() {
        let root = tempfile::tempdir().unwrap();
        let battery = root.path().join("sys/class/power_supply/battery");
        fs::create_dir_all(&battery).unwrap();
        fs::write(battery.join("capacity"), "50").unwrap();
        fs::write(battery.join("status"), "Discharging").unwrap();
        let mut status = Status::new(Some(root.path()), Instant::now());
        status.start();
        // Redraw requests not explained by a minute change of the clock.
        let poll = |status: &mut Status| {
            let clock = status.clock.clone();
            usize::from(status.poll(Instant::now()) && status.clock == clock)
        };
        let wait = |status: &mut Status, charging: bool| {
            let deadline = Instant::now() + BATTERY_REFRESH + Duration::from_secs(1);
            let mut redraws = 0;
            while status.battery.map(|b| b.charging) != Some(charging) {
                assert!(Instant::now() < deadline, "charger state not picked up");
                std::thread::sleep(Duration::from_millis(5));
                redraws += poll(status);
            }
            redraws
        };
        assert_eq!(wait(&mut status, false), 1);
        fs::write(battery.join("status"), "Charging").unwrap();
        // Exactly one redraw per real change; idle polls in between request none.
        assert_eq!(wait(&mut status, true), 1);
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(5));
            assert_eq!(poll(&mut status), 0);
        }
        fs::write(battery.join("status"), "Discharging").unwrap();
        assert_eq!(wait(&mut status, false), 1);
    }

    #[test]
    fn battery_source_is_optional_and_throttled_to_two_seconds() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("sys/class/power_supply/battery");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("capacity"), "87").unwrap();
        fs::write(path.join("status"), "Charging").unwrap();
        let now = Instant::now();
        let mut source = BatterySource::new(root.path().into(), now);
        assert_eq!(
            source.poll(now),
            Some(Some(Battery {
                percent: 87,
                charging: true
            }))
        );
        fs::write(path.join("capacity"), "12").unwrap();
        assert_eq!(source.poll(now + Duration::from_millis(1999)), None);
        assert_eq!(
            source.poll(now + BATTERY_REFRESH).unwrap().unwrap().percent,
            12
        );
        fs::write(path.join("capacity"), "999").unwrap();
        assert_eq!(source.poll(now + BATTERY_REFRESH * 2), Some(None));
        fs::remove_file(path.join("capacity")).unwrap();
        assert_eq!(source.poll(now + BATTERY_REFRESH * 3), Some(None));
    }

    #[test]
    fn battery_directory_is_cached_and_searched_again_only_after_a_failed_read() {
        let root = tempfile::tempdir().unwrap();
        let supplies = root.path().join("sys/class/power_supply");
        let supply = |name: &str, percent: &str| {
            let dir = supplies.join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("type"), "Battery\n").unwrap();
            fs::write(dir.join("capacity"), percent).unwrap();
        };
        supply("rk817-battery", "40");
        let now = Instant::now();
        let mut source = BatterySource::new(root.path().into(), now);
        let percent = |source: &mut BatterySource, tick: u32| {
            source
                .poll(now + BATTERY_REFRESH * tick)
                .unwrap()
                .map(|b| b.percent)
        };
        assert_eq!(percent(&mut source, 0), Some(40));
        // An earlier-sorting supply appears: the cached directory keeps winning.
        supply("axp-battery", "90");
        assert_eq!(percent(&mut source, 1), Some(40));
        // Only a failed read triggers a new search.
        fs::remove_dir_all(supplies.join("rk817-battery")).unwrap();
        assert_eq!(percent(&mut source, 2), Some(90));
        assert_eq!(source.dir, Some(supplies.join("axp-battery")));
    }
}
