//! Cached status: only the status worker touches sysfs. It starts after
//! present; the render thread consumes a typed snapshot without waiting.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const REFRESH: Duration = Duration::from_secs(30);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}
struct BatterySource {
    path: PathBuf,
    next_read: Instant,
}
impl BatterySource {
    fn poll(&mut self, now: Instant) -> Option<Option<Battery>> {
        if now < self.next_read {
            return None;
        }
        self.next_read = now + REFRESH;
        Some((|| {
            let percent = fs::read_to_string(self.path.join("capacity"))
                .ok()?
                .trim()
                .parse::<u8>()
                .ok()?;
            if percent > 100 {
                return None;
            }
            let charging = fs::read_to_string(self.path.join("status"))
                .ok()
                .is_some_and(|s| s.trim() == "Charging");
            Some(Battery { percent, charging })
        })())
    }
}
#[derive(Clone, Copy, Default)]
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
}
impl Worker {
    fn start(path: PathBuf) -> std::io::Result<Self> {
        let value = Arc::new(Mutex::new(Snapshot::default()));
        let published = value.clone();
        let (stop, stopped) = mpsc::channel();
        thread::Builder::new()
            .name("launcher-status".into())
            .spawn(move || {
                let mut source = BatterySource {
                    path: path.join("sys/class/power_supply/battery"),
                    next_read: Instant::now(),
                };
                loop {
                    let battery = source
                        .poll(Instant::now())
                        .unwrap_or_else(|| published.lock().expect("status snapshot").battery);
                    *published.lock().expect("status snapshot") = Snapshot {
                        battery,
                        wifi: wifi_on(&path),
                    };
                    match stopped.recv_timeout(REFRESH) {
                        Ok(Signal::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Ok(Signal::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self { value, stop })
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
        };
        status.poll(now);
        status
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
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.update(unix, snapshot)
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
            let clock = local_clock(unix);
            changed |= self.clock != clock;
            self.clock = clock;
        }
        changed
    }
}
/// Radio state is the wlan rfkill soft block, which `wifictl` flips at once; link
/// flags would lag behind it and depend on the interface name.
fn wifi_on(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root.join("sys/class/rfkill")) else {
        return false;
    };
    let read = |dir: &Path, name: &str| fs::read_to_string(dir.join(name)).unwrap_or_default();
    entries
        .flatten()
        .any(|e| read(&e.path(), "type").trim() == "wlan" && read(&e.path(), "soft").trim() == "0")
}
fn local_clock(unix: u64) -> String {
    let seconds = unix as libc::time_t;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers reference correctly aligned live allocations;
    // localtime_r writes a complete tm on success and retains neither pointer.
    let result = unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) };
    if result.is_null() {
        return "--:--".to_owned();
    }
    // SAFETY: a non-null return confirms the output was initialized.
    let local = unsafe { local.assume_init() };
    format!("{:02}:{:02}", local.tm_hour, local.tm_min)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn worker_source_is_optional_and_throttled_for_thirty_seconds() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_owned();
        fs::write(path.join("capacity"), "87").unwrap();
        fs::write(path.join("status"), "Charging").unwrap();
        let now = Instant::now();
        let mut source = BatterySource {
            path: path.clone(),
            next_read: now,
        };
        assert_eq!(
            source.poll(now),
            Some(Some(Battery {
                percent: 87,
                charging: true
            }))
        );
        fs::write(path.join("capacity"), "12").unwrap();
        assert_eq!(source.poll(now + Duration::from_secs(29)), None);
        assert_eq!(source.poll(now + REFRESH).unwrap().unwrap().percent, 12);
        fs::write(path.join("capacity"), "999").unwrap();
        assert_eq!(source.poll(now + REFRESH * 2), Some(None));
        fs::remove_file(path.join("capacity")).unwrap();
        assert_eq!(source.poll(now + REFRESH * 3), Some(None));
    }
}
