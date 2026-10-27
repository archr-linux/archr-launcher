//! Cached status. Sysfs is consulted at most once per thirty seconds, never
//! from draw; the event-loop timer also invalidates the minute-resolution clock.
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}
pub struct Status {
    pub battery: Option<Battery>,
    pub clock: String,
    path: PathBuf,
    next_battery: Instant,
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
            next_battery: now,
            minute: u64::MAX,
        };
        status.poll(now);
        status
    }
    pub fn refresh_battery(&mut self, now: Instant) -> bool {
        if now < self.next_battery {
            return false;
        }
        self.next_battery = now + Duration::from_secs(30);
        let battery = (|| {
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
        })();
        let changed = self.battery != battery;
        self.battery = battery;
        changed
    }
    pub fn poll(&mut self, now: Instant) -> bool {
        let mut changed = self.refresh_battery(now);
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
