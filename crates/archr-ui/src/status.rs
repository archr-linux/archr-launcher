//! Cached status: only the battery worker touches sysfs. It starts after
//! present; the render thread consumes its atomic snapshot without waiting.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU16, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const REFRESH: Duration = Duration::from_secs(30);
const READY: u16 = 1 << 9;
const PRESENT: u16 = 1 << 8;
const CHARGING: u16 = 1 << 7;

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
struct Worker {
    value: Arc<AtomicU16>,
    stop: mpsc::Sender<()>,
}
impl Worker {
    fn start(path: PathBuf) -> std::io::Result<Self> {
        let value = Arc::new(AtomicU16::new(0)); // no reading has arrived yet
        let published = value.clone();
        let (stop, stopped) = mpsc::channel();
        thread::Builder::new()
            .name("launcher-battery".into())
            .spawn(move || {
                let mut source = BatterySource {
                    path,
                    next_read: Instant::now(),
                };
                loop {
                    if let Some(battery) = source.poll(Instant::now()) {
                        let bits = READY
                            | battery.map_or(0, |b| {
                                PRESENT
                                    | u16::from(b.percent)
                                    | if b.charging { CHARGING } else { 0 }
                            });
                        published.store(bits, Ordering::Release);
                    }
                    if stopped.recv_timeout(REFRESH) != Err(mpsc::RecvTimeoutError::Timeout) {
                        break;
                    }
                }
            })?;
        Ok(Self { value, stop })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Wake a sleeping worker. Never join a potentially slow sysfs read on
        // the presentation thread; a read already in progress finishes there.
        let _ = self.stop.send(());
    }
}
pub struct Status {
    pub battery: Option<Battery>,
    pub clock: String,
    path: PathBuf,
    worker: Option<Worker>,
    started: bool,
    minute: u64,
}
impl Status {
    pub fn new(root: Option<&Path>, now: Instant) -> Self {
        let path = root
            .map_or_else(|| PathBuf::from("/"), Path::to_owned)
            .join("sys/class/power_supply/battery");
        let mut status = Self {
            battery: None,
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
            Err(error) => log::warn!("Cannot start battery worker: {error}"),
        }
    }
    pub fn poll(&mut self, _now: Instant) -> bool {
        let mut changed = false;
        if let Some(worker) = &self.worker {
            let bits = worker.value.load(Ordering::Acquire);
            if bits & READY != 0 {
                let battery = (bits & PRESENT != 0).then_some(Battery {
                    percent: (bits & 0x7f) as u8,
                    charging: bits & CHARGING != 0,
                });
                changed = self.battery != battery;
                self.battery = battery;
            }
        }
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if unix / 60 != self.minute {
            self.minute = unix / 60;
            self.clock = local_clock(unix);
            changed = true;
        }
        changed
    }
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
