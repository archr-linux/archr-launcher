//! Bounded Linux telemetry. Called only by the serial settings worker.
use super::Backend;
use archr_ui::{
    settings::{Id, InfoCard, InfoDetail, InfoMeter, Information},
    status::{find_battery, read_battery, wifi_on},
};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs,
    net::Ipv4Addr,
    os::unix::ffi::OsStrExt,
};

pub(super) fn percent(used: u64, total: u64) -> Option<u8> {
    used.saturating_mul(100)
        .checked_div(total)
        .map(|v| v.min(100) as u8)
}
pub(super) fn bytes(value: u64) -> String {
    if value >= 1024 * 1024 * 1024 {
        format!("@{:.1} GB", value as f64 / 1073741824.)
    } else {
        format!("@{} MB", value / 1048576)
    }
}
fn usage(used: u64, total: u64) -> String {
    format!(
        "settings.used_total|used={}|total={}",
        bytes(used),
        bytes(total)
    )
}
fn meter(label: impl Into<String>, value: String, percent: Option<u8>) -> InfoMeter {
    InfoMeter {
        label: label.into(),
        value,
        percent,
    }
}
fn card(
    main: String,
    secondary: String,
    percent: Option<u8>,
    rows: Vec<(&'static str, String)>,
    meters: Vec<InfoMeter>,
) -> InfoCard {
    InfoCard {
        lines: vec![main.clone(), secondary.clone()],
        percent,
        detail: InfoDetail {
            heading: main,
            subtitle: secondary,
            rows,
            meters,
        },
    }
}
/// The libc-owned list is freed on every return path. No command/path dependency.
pub(super) fn ipv4_interface(preferred: &str) -> Option<(String, String)> {
    struct Addresses(*mut libc::ifaddrs);
    impl Drop for Addresses {
        fn drop(&mut self) {
            // SAFETY: this list was allocated by a successful getifaddrs call.
            unsafe { libc::freeifaddrs(self.0) };
        }
    }
    let mut head = std::ptr::null_mut();
    // SAFETY: head points to writable storage for the output list pointer.
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return None;
    }
    let list = Addresses(head);
    let mut current = list.0;
    let mut first = None;
    while !current.is_null() {
        // SAFETY: the list remains owned by `list` until after traversal.
        let entry = unsafe { &*current };
        if !entry.ifa_addr.is_null() && !entry.ifa_name.is_null()
            && entry.ifa_flags & libc::IFF_LOOPBACK as u32 == 0
            // SAFETY: getifaddrs supplies a valid sockaddr and NUL-terminated name.
            && unsafe { (*entry.ifa_addr).sa_family } as i32 == libc::AF_INET
        {
            // SAFETY: AF_INET addresses are sockaddr_in, aligned by libc.
            let address = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_in>() };
            let ip = Ipv4Addr::from(address.sin_addr.s_addr.to_ne_bytes());
            if !ip.is_loopback() && !ip.is_unspecified() {
                // SAFETY: libc guarantees the interface name is NUL-terminated.
                let name = unsafe { CStr::from_ptr(entry.ifa_name) }
                    .to_string_lossy()
                    .into_owned();
                let entry = (name, ip.to_string());
                if entry.0 == preferred {
                    return Some(entry);
                }
                if first.is_none() {
                    first = Some(entry);
                }
            }
        }
        current = entry.ifa_next;
    }
    first
}
pub(super) fn filesystem_usage(path: &std::path::Path) -> Option<(u64, u64)> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: valid C path and writable output; initialized only on success.
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: successful statvfs initialized every field.
    let s = unsafe { stat.assume_init() };
    Some((
        s.f_blocks
            .saturating_sub(s.f_bfree)
            .saturating_mul(s.f_frsize),
        s.f_blocks.saturating_mul(s.f_frsize),
    ))
}
fn cpu_ticks(text: &str) -> BTreeMap<u32, (u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next()?.strip_prefix("cpu")?;
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            // guest/guest_nice are already included in user/nice: do not count twice.
            let ticks: Vec<u64> = words.take(8).map(|v| v.parse().unwrap_or(0)).collect();
            Some((
                name.parse().ok()?,
                (
                    ticks.iter().copied().fold(0u64, u64::saturating_add),
                    ticks
                        .get(3)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(ticks.get(4).copied().unwrap_or(0)),
                ),
            ))
        })
        .take(256)
        .collect()
}
fn gateway(routes: &str, interface: &str) -> String {
    routes
        .lines()
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 4 || fields[0] != interface || fields[1] != "00000000" {
                return None;
            }
            let flags = u32::from_str_radix(fields[3], 16).ok()?;
            if flags & 3 != 3 {
                return None;
            }
            Some(Ipv4Addr::from(u32::from_str_radix(fields[2], 16).ok()?.to_le_bytes()).to_string())
        })
        .unwrap_or_default()
}
fn wireless_signal(text: &str, interface: &str) -> Option<i32> {
    text.lines().find_map(|line| {
        let (name, rest) = line.split_once(':')?;
        if name.trim() != interface {
            return None;
        }
        let dbm = rest
            .split_whitespace()
            .nth(2)?
            .trim_end_matches('.')
            .parse::<i32>()
            .ok()?;
        (-127..=0).contains(&dbm).then_some(dbm)
    })
}
/// Wireless Extensions ESSID query, used only on the real system, never fixtures.
fn ssid(interface: &str) -> String {
    #[repr(C)]
    struct Point {
        pointer: *mut libc::c_void,
        length: u16,
        flags: u16,
    }
    #[repr(C)]
    union Data {
        point: std::mem::ManuallyDrop<Point>,
        padding: [u8; 16],
    }
    #[repr(C)]
    struct Request {
        name: [libc::c_char; libc::IFNAMSIZ],
        data: Data,
    }
    if interface.len() >= libc::IFNAMSIZ {
        return String::new();
    }
    // SAFETY: no pointers; creates a datagram control socket.
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return String::new();
    }
    let mut buffer = [0u8; 33];
    let mut request = Request {
        name: [0; libc::IFNAMSIZ],
        data: Data {
            point: std::mem::ManuallyDrop::new(Point {
                pointer: buffer.as_mut_ptr().cast(),
                length: 32,
                flags: 0,
            }),
        },
    };
    for (to, from) in request.name.iter_mut().zip(interface.bytes()) {
        *to = from as libc::c_char;
    }
    // SAFETY: iwreq has the Linux Wireless Extensions ABI; buffer holds 32 ESSID bytes.
    let result = unsafe { libc::ioctl(fd, 0x8B1B, &mut request) };
    // SAFETY: fd is owned by this function and no longer used.
    unsafe { libc::close(fd) };
    if result < 0 {
        return String::new();
    }
    // SAFETY: SIOCGIWESSID writes the point member, not another union alternative.
    let length = unsafe { request.data.point.length }.min(32) as usize;
    String::from_utf8_lossy(&buffer[..length])
        .trim_end_matches('\0')
        .into()
}

fn default_interface(routes: &str) -> &str {
    routes
        .lines()
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.len() > 3
                && fields[1] == "00000000"
                && u32::from_str_radix(fields[3], 16).is_ok_and(|flags| flags & 3 == 3))
            .then(|| fields[0])
        })
        .unwrap_or("")
}
fn uptime(seconds: u64, summary: bool) -> String {
    let (hours, minutes) = (seconds / 3600, seconds % 3600 / 60);
    match (hours > 0, summary) {
        (true, false) => format!("settings.duration_hours|hours={hours}|minutes={minutes}"),
        (true, true) => format!("settings.uptime_summary_hours|hours={hours}|minutes={minutes}"),
        (false, false) => format!("settings.duration_minutes|count={minutes}"),
        (false, true) => format!("settings.uptime_summary_minutes|count={minutes}"),
    }
}
fn mhz(value: Option<u64>, divisor: u64) -> String {
    value
        .map(|v| format!("@{} MHz", v / divisor))
        .unwrap_or_default()
}
/// Cached identity only; live cards are built once per refresh.
pub(super) struct Identity {
    version: String,
    device: String,
    kernel: String,
    cpu: String,
    core_type: String,
    cores: usize,
    gpu: String,
}
impl Backend {
    fn number_at(&self, path: &str) -> Option<u64> {
        self.read(path).parse().ok()
    }
    fn package_version(&self, package: &str) -> String {
        fs::read_dir(self.paths.at("/var/lib/pacman/local"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .find_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                name.strip_prefix(&format!("{package}-"))
                    .filter(|s| s.starts_with(|c: char| c.is_ascii_digit()))
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }

    fn image_version(&self) -> String {
        self.read("/etc/os-release")
            .lines()
            .find_map(|l| {
                l.strip_prefix("VERSION_ID=")
                    .map(|v| v.trim_matches('"').into())
            })
            .unwrap_or_else(|| self.read("/etc/version"))
    }
    fn identity(&self) -> Identity {
        let cpu = self.read("/proc/cpuinfo");
        let field = |name: &str| {
            cpu.lines()
                .filter_map(|l| l.split_once(':'))
                .find(|(key, _)| key.trim() == name)
                .map(|(_, v)| v.trim())
                .unwrap_or("")
        };
        let compatible = self.read("/sys/firmware/devicetree/base/compatible");
        let soc = compatible.split('\0').rev().find_map(|v| v.split_once(','));
        let hardware = [field("Hardware"), field("model name")]
            .into_iter()
            .find(|v| !v.is_empty());
        let model = if compatible.contains("rk3326")
            || hardware.is_some_and(|v| v.to_lowercase().contains("rk3326"))
        {
            "Rockchip RK3326".into()
        } else if let Some(model) = hardware {
            model.into()
        } else if let Some((vendor, chip)) = soc {
            format!("{vendor} {}", chip.to_uppercase())
        } else {
            "settings.unavailable".into()
        };
        let core_type = match field("CPU part") {
            "0xd03" => "ARM Cortex-A53",
            "0xd04" => "ARM Cortex-A35",
            "0xd05" => "ARM Cortex-A55",
            "0xd08" => "ARM Cortex-A72",
            "0xd0b" => "ARM Cortex-A76",
            other => other,
        }
        .to_owned();
        let rk3326 = model.contains("RK3326");
        Identity {
            // archr-update decides it (a newer recorded update, else the
            // image); its own fallback, the image's, when it cannot run.
            version: self
                .command("archr-update", &["version"])
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| self.image_version()),
            device: self
                .read("/sys/firmware/devicetree/base/model")
                .trim_end_matches('\0')
                .into(),
            kernel: self.read("/proc/sys/kernel/osrelease"),
            cpu: model,
            core_type: if core_type.is_empty() && rk3326 {
                "ARM Cortex-A35".into()
            } else {
                core_type
            },
            cores: cpu.lines().filter(|l| l.starts_with("processor")).count(),
            gpu: if rk3326 {
                "Mali-G31".into()
            } else {
                String::new()
            },
        }
    }
    pub(super) fn refresh_info(&self) -> Information {
        let identity = self.identity.get_or_init(|| self.identity());
        let cfg = self.cfg().ok();
        let seconds = self
            .read("/proc/uptime")
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.) as u64;
        Information {
            version: identity.version.clone(),
            device: identity.device.clone(),
            kernel: identity.kernel.clone(),
            cards: vec![
                self.cpu_info(identity, cfg.as_ref(), seconds),
                self.gpu_info(identity, cfg.as_ref()),
                self.memory_info(),
                self.storage_info(),
                self.network_info(),
                self.battery_info(seconds),
            ],
        }
    }
    fn cpu_info(
        &self,
        identity: &Identity,
        cfg: Option<&archr_config::SystemCfg>,
        seconds: u64,
    ) -> InfoCard {
        let base = "/sys/devices/system/cpu/cpufreq/policy0";
        let current = self.number_at(&format!("{base}/scaling_cur_freq"));
        let min = self.number_at(&format!("{base}/scaling_min_freq"));
        let max = self.number_at(&format!("{base}/scaling_max_freq"));
        let range = min
            .zip(max)
            .map(|(min, max)| format!("@{} · @{} MHz", min / 1000, max / 1000))
            .unwrap_or_default();
        let governor = self.read(&format!("{base}/scaling_governor"));
        let temp = self
            .number_at("/sys/class/thermal/thermal_zone0/temp")
            .filter(|v| *v <= 200_000);
        let temperature = temp
            .map(|v| format!("@{:.1} °C", v as f64 / 1000.))
            .unwrap_or_default();
        let turbo = cfg.map(|cfg| {
            cfg.get_setting(Id::Turbo.key(), None, None)
                .unwrap_or_else(|| Id::Turbo.default_value().into())
        });
        let summary = current
            .map(|v| format!("@{:.2} GHz", v as f64 / 1_000_000.))
            .unwrap_or_default();
        let secondary = [summary, governor.clone(), temperature.clone()]
            .into_iter()
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let ticks = cpu_ticks(&self.read("/proc/stat"));
        let mut previous = self.cpu_ticks.lock().unwrap_or_else(|e| e.into_inner());
        let mut meters: Vec<_> = ticks
            .iter()
            .map(|(&id, &(total, idle))| {
                let value = previous.get(&id).and_then(|&(old_total, old_idle)| {
                    if total < old_total || idle < old_idle {
                        return None;
                    }
                    percent(
                        (total - old_total).saturating_sub(idle - old_idle),
                        total - old_total,
                    )
                });
                meter(
                    format!("CPU{id}"),
                    value.map(|v| format!("{v}%")).unwrap_or_default(),
                    value,
                )
            })
            .collect();
        *previous = ticks;
        drop(previous);
        let critical = (0..16).find_map(|index| {
            let base = format!("/sys/class/thermal/thermal_zone0/trip_point_{index}");
            (self.read(&format!("{base}_type")) == "critical")
                .then(|| self.number_at(&format!("{base}_temp")))
                .flatten()
        });
        let caption = critical
            .map(|v| {
                format!(
                    "settings.temperature_limit|value={temperature}|limit=@{:.0} °C",
                    v as f64 / 1000.
                )
            })
            .unwrap_or_else(|| temperature.clone());
        meters.push(meter(
            "settings.temperature",
            caption,
            temp.and_then(|v| percent(v, critical.unwrap_or(100_000))),
        ));
        InfoCard {
            lines: vec![
                format!(
                    "settings.cpu_summary|model={}|count={}",
                    identity.cpu, identity.cores
                ),
                secondary,
            ],
            percent: None,
            detail: InfoDetail {
                heading: identity.cpu.clone(),
                subtitle: if identity.core_type.is_empty() {
                    format!(
                        "settings.cores_bits|count={}|bits={}",
                        identity.cores,
                        usize::BITS
                    )
                } else {
                    format!(
                        "settings.cores_type_bits|count={}|type={}|bits={}",
                        identity.cores,
                        identity.core_type,
                        usize::BITS
                    )
                },
                rows: vec![
                    ("settings.frequency", mhz(current, 1000)),
                    ("settings.min_max", range),
                    ("settings.frequency_governor", governor),
                    (
                        "settings.turbo",
                        turbo
                            .map(|v| {
                                if v == "1" {
                                    "settings.on"
                                } else {
                                    "settings.off"
                                }
                                .into()
                            })
                            .unwrap_or_default(),
                    ),
                    ("settings.temperature", temperature),
                    (
                        "settings.load_average",
                        self.read("/proc/loadavg")
                            .split_whitespace()
                            .take(3)
                            .filter(|v| v.parse::<f64>().is_ok())
                            .map(|v| format!("@{v}"))
                            .collect::<Vec<_>>()
                            .join(" · "),
                    ),
                    ("settings.uptime_label", uptime(seconds, false)),
                ],
                meters,
            },
        }
    }
    fn gpu_info(&self, identity: &Identity, cfg: Option<&archr_config::SystemCfg>) -> InfoCard {
        let active = [
            "/sys/class/drm/renderD128/device/driver",
            "/sys/class/drm/card0/device/driver",
            "/sys/class/misc/mali0/device/driver",
        ]
        .into_iter()
        .find_map(|p| {
            fs::read_link(self.paths.at(p))
                .ok()
                .and_then(|p| p.file_name().map(|v| v.to_string_lossy().into_owned()))
                .filter(|v| v.contains("mali") || v == "panfrost")
        })
        .or_else(|| {
            self.paths
                .at("/dev/mali0")
                .exists()
                .then(|| "libmali".into())
        })
        .map(|v| {
            if v.contains("mali") {
                "libmali".into()
            } else {
                v
            }
        });
        let configured = cfg
            .and_then(|c| c.get_setting(Id::Gpu.key(), None, None))
            .unwrap_or_else(|| Id::Gpu.default_value().into());
        let mesa = self.package_version("mesa");
        let mesa = mesa
            .rsplit_once('-')
            .map_or(mesa.as_str(), |(version, _)| version);
        let driver_version = if active.as_deref() == Some("panfrost") {
            if mesa.is_empty() {
                String::new()
            } else {
                format!("Mesa {mesa}")
            }
        } else {
            self.read("/sys/module/mali/version")
        };
        let gpu_model = identity.gpu.clone();
        let main = [
            gpu_model.clone(),
            active
                .as_deref()
                .map(|v| if v == "panfrost" { "Panfrost" } else { v })
                .unwrap_or_default()
                .into(),
        ]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        let devfreq = fs::read_dir(self.paths.at("/sys/class/devfreq"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().contains("gpu"))
            .map(|e| format!("/sys/class/devfreq/{}", e.file_name().to_string_lossy()));
        let resolution = self
            .read("/sys/class/graphics/fb0/virtual_size")
            .replace(',', " × ");
        // fb mode strings include refresh where exported (e.g. U:640x480p-60).
        let mode = self.read("/sys/class/graphics/fb0/mode");
        let refresh = mode
            .rsplit_once('-')
            .and_then(|(_, s)| s.parse::<f64>().ok())
            .map(|v| format!("@{v} Hz"))
            .unwrap_or_default();
        let current = devfreq
            .as_ref()
            .and_then(|p| self.number_at(&format!("{p}/cur_freq")));
        let frequency = mhz(current, 1_000_000);
        let ratio = devfreq
            .as_ref()
            .and_then(|p| percent(current?, self.number_at(&format!("{p}/max_freq"))?));
        let rows = vec![
            ("settings.gpu_model", gpu_model),
            ("settings.active_driver", active.unwrap_or_default()),
            ("settings.configured_driver", configured),
            ("settings.frequency", frequency.clone()),
            (
                "settings.frequency_governor",
                devfreq
                    .as_ref()
                    .map(|p| self.read(&format!("{p}/governor")))
                    .unwrap_or_default(),
            ),
            ("settings.driver_version", driver_version.clone()),
            ("settings.resolution", resolution.clone()),
            ("settings.refresh_rate", refresh),
        ];
        card(
            main,
            driver_version,
            None,
            rows,
            vec![meter("settings.frequency", frequency, ratio)],
        )
    }
    fn memory_info(&self) -> InfoCard {
        let memory = self.read("/proc/meminfo");
        let kb = |key: &str| {
            memory
                .lines()
                .find_map(|l| {
                    l.strip_prefix(key)?
                        .split_whitespace()
                        .next()?
                        .parse::<u64>()
                        .ok()
                })
                .unwrap_or(0)
                .saturating_mul(1024)
        };
        let total = kb("MemTotal:");
        let available = kb("MemAvailable:");
        let used = total.saturating_sub(available);
        if total == 0 {
            return card(String::new(), String::new(), None, vec![], vec![]);
        }
        let ratio = percent(used, total);
        let mut rows = vec![
            ("settings.total", bytes(total)),
            ("settings.used", bytes(used)),
            ("settings.available", bytes(available)),
            ("settings.cached", bytes(kb("Cached:"))),
        ];
        let mut meters = vec![meter("settings.memory", usage(used, total), ratio)];
        let zram_used = self
            .read("/sys/block/zram0/mm_stat")
            .split_whitespace()
            .next()
            .and_then(|v| v.parse().ok());
        let zram_size = self.number_at("/sys/block/zram0/disksize");
        let swap = kb("SwapTotal:");
        if swap > zram_size.unwrap_or(0) {
            let swap_used = swap.saturating_sub(kb("SwapFree:"));
            let value = usage(swap_used, swap);
            rows.push(("settings.swap", value.clone()));
            meters.push(meter("settings.swap", value, percent(swap_used, swap)));
        }
        if let Some((used, total)) = zram_used.zip(zram_size) {
            let value = usage(used, total);
            rows.push(("settings.zram", value.clone()));
            meters.push(meter("settings.zram", value, percent(used, total)));
        }
        card(
            usage(used, total),
            format!("settings.in_use|value={}", ratio.unwrap_or(0)),
            ratio,
            rows,
            meters,
        )
    }
    fn storage_info(&self) -> InfoCard {
        let mut main = String::new();
        let mut secondary = String::new();
        let mut ratio = None;
        let mut meters = vec![];
        let mounts = self.read("/proc/mounts");
        for line in mounts.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 3 {
                continue;
            }
            let mount = fields[1].replace("\\040", " ").replace("\\134", "\\");
            if !matches!(
                mount.as_str(),
                "/flash" | "/storage" | "/storage/roms" | "/roms"
            ) && !mount.starts_with("/var/media/")
            {
                continue;
            }
            if let Some((used, total)) = filesystem_usage(&self.paths.at(&mount)) {
                if mount == "/storage" || main.is_empty() {
                    main = usage(used, total);
                    ratio = percent(used, total);
                }
                if mount == "/flash" {
                    secondary = format!(
                        "settings.system_partition|used={}|total={}",
                        bytes(used),
                        bytes(total)
                    );
                }
                meters.push(meter(
                    format!("{mount} · {}", fields[2]),
                    usage(used, total),
                    percent(used, total),
                ));
            }
        }
        InfoCard {
            lines: vec![main, secondary],
            percent: ratio,
            detail: InfoDetail {
                heading: "settings.storage".into(),
                subtitle: "settings.mounted_partitions".into(),
                rows: vec![],
                meters,
            },
        }
    }
    fn network_info(&self) -> InfoCard {
        let routes = self.read("/proc/net/route");
        let preferred = default_interface(&routes);
        let (interface, ip) = if self.paths.root.is_none() {
            ipv4_interface(preferred).unwrap_or_default()
        } else {
            (String::new(), String::new())
        };
        let wireless = !interface.is_empty()
            && (self
                .paths
                .at(&format!("/sys/class/net/{interface}/wireless"))
                .exists()
                || self
                    .read("/proc/net/wireless")
                    .lines()
                    .any(|l| l.trim_start().starts_with(&format!("{interface}:"))));
        let connected = !ip.is_empty();
        let hostname = self.read("/proc/sys/kernel/hostname");
        let dns = self
            .read("/etc/resolv.conf")
            .lines()
            .filter_map(|l| {
                let mut words = l.split_whitespace();
                (words.next()? == "nameserver")
                    .then(|| words.next())
                    .flatten()
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let route_interface = if interface.is_empty() {
            preferred
        } else {
            &interface
        };
        let signal = wireless_signal(&self.read("/proc/net/wireless"), &interface);
        let ssid = if wireless && self.paths.root.is_none() {
            ssid(&interface)
        } else {
            String::new()
        };
        let signal_text = signal.map(|v| format!("@{v} dBm")).unwrap_or_default();
        card(
            if wireless && connected && wifi_on(&self.paths.at("/")) {
                "settings.wifi_connected"
            } else if connected {
                "settings.link_connected"
            } else {
                "settings.disconnected"
            }
            .into(),
            [ip.clone(), hostname.clone()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            None,
            vec![
                ("settings.interface", interface.clone()),
                ("settings.ip", ip),
                ("settings.gateway", gateway(&routes, route_interface)),
                ("settings.dns", dns),
                (
                    "settings.mac",
                    if interface.is_empty() {
                        String::new()
                    } else {
                        self.read(&format!("/sys/class/net/{interface}/address"))
                    },
                ),
                ("settings.ssid", ssid),
                ("settings.signal", signal_text.clone()),
                ("settings.hostname", hostname),
            ],
            vec![meter(
                "settings.signal",
                signal_text.clone(),
                signal.map(|v| ((v + 100) * 2).clamp(0, 100) as u8),
            )],
        )
    }
    fn battery_info(&self, seconds: u64) -> InfoCard {
        let battery = read_battery(&self.paths.at("/"));
        let battery_dir = find_battery(&self.paths.at("/"));
        let battery_value = |name: &str| {
            battery_dir
                .as_ref()
                .and_then(|e| {
                    archr_io::read_utf8(&e.join(name), 4096, archr_io::Symlinks::Follow).ok()
                })
                .unwrap_or_default()
                .trim()
                .to_owned()
        };
        let electrical = |name, divisor, unit, precision| {
            battery_value(name)
                .parse::<i64>()
                .ok()
                .map(|v| format!("@{:.*} {unit}", precision, v as f64 / divisor))
                .unwrap_or_default()
        };
        let battery_status = battery_value("status");
        let plugged = battery.is_some_and(|b| b.charging)
            || matches!(battery_status.as_str(), "Full" | "Not charging");
        let level = battery.map(|b| b.percent);
        let status = battery
            .map(|_| {
                if plugged {
                    "settings.plugged"
                } else {
                    "settings.discharging"
                }
            })
            .unwrap_or("settings.unavailable");
        card(
            level
                .map(|v| format!("settings.battery_summary|value={v}|status={status}"))
                .unwrap_or_default(),
            uptime(seconds, true),
            level,
            vec![
                (
                    "settings.level",
                    level.map(|v| format!("{v}%")).unwrap_or_default(),
                ),
                (
                    "settings.status",
                    match battery_status.as_str() {
                        "Charging" => "settings.charging",
                        "Discharging" => "settings.discharging",
                        "Full" => "settings.full",
                        "Not charging" => "settings.not_charging",
                        _ => "settings.unavailable",
                    }
                    .into(),
                ),
                (
                    "settings.voltage",
                    electrical("voltage_now", 1_000_000., "V", 2),
                ),
                (
                    "settings.current",
                    electrical("current_now", 1000., "mA", 0),
                ),
                (
                    "settings.health",
                    match battery_value("health").as_str() {
                        "Good" => "settings.health_good".into(),
                        "Unknown" => "settings.unavailable".into(),
                        v => v.into(),
                    },
                ),
                ("settings.design_capacity", {
                    let charge = electrical("charge_full_design", 1000., "mAh", 0);
                    if charge.is_empty() {
                        electrical("energy_full_design", 1_000_000., "Wh", 2)
                    } else {
                        charge
                    }
                }),
                ("settings.uptime_label", uptime(seconds, false)),
            ],
            vec![meter(
                "settings.battery",
                level.map(|v| format!("{v}%")).unwrap_or_default(),
                level,
            )],
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ticks_exclude_guest_and_gateway_is_little_endian() {
        assert_eq!(
            cpu_ticks("cpu0 10 0 20 70 0 0 0 0 100 100\n"),
            BTreeMap::from([(0, (100, 70))])
        );
        assert_eq!(
            gateway("wlan0 00000000 0103A8C0 0003", "wlan0"),
            "192.168.3.1"
        );
        assert_eq!(gateway("wlan0 00000000 0103A8C0 0000", "wlan0"), "");
    }
    #[test]
    fn signal_is_bounded_and_matches_the_selected_interface() {
        assert_eq!(
            wireless_signal(" wlan0: 0000 70. -49. 0", "wlan0"),
            Some(-49)
        );
        assert_eq!(wireless_signal(" wlan0: 0000 70. -49. 0", "eth0"), None);
        assert_eq!(
            wireless_signal(" wlan0: 0000 70. 2147483647 0", "wlan0"),
            None
        );
    }
    #[test]
    fn default_route_requires_up_and_gateway_flags() {
        assert_eq!(
            default_interface("usb0 00000000 0100007F 0000\nwlan0 00000000 0103A8C0 0003"),
            "wlan0"
        );
        assert_eq!(default_interface("eth0 00000000 0103A8C0 0003"), "eth0");
        assert_eq!(default_interface("eth0 00000000 0103A8C0 0001"), "");
        assert_eq!(
            uptime(7380, false),
            "settings.duration_hours|hours=2|minutes=3"
        );
        assert_eq!(
            uptime(10799, false),
            "settings.duration_hours|hours=2|minutes=59"
        );
        assert_eq!(
            uptime(3540, true),
            "settings.uptime_summary_minutes|count=59"
        );
        assert_eq!(
            uptime(7380, true),
            "settings.uptime_summary_hours|hours=2|minutes=3"
        );
    }
    #[test]
    fn ipv4_lookup_never_returns_loopback() {
        if let Some((name, address)) = ipv4_interface("") {
            assert!(!name.is_empty());
            assert!(!address.parse::<Ipv4Addr>().unwrap().is_loopback());
        }
    }
    #[test]
    fn version_comes_from_archr_update_else_the_image() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(Some(dir.path().into()));
        std::fs::create_dir_all(dir.path().join("etc")).unwrap();
        std::fs::write(dir.path().join("etc/os-release"), "VERSION_ID=\"2.1.1\"\n").unwrap();
        let backend = super::super::Backend::new(paths.clone());
        assert_eq!(backend.identity().version, "2.1.1");
        let bin = dir.path().join("usr/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("archr-update");
        std::fs::write(&script, "#!/bin/sh\n[ \"$1\" = version ] && echo 2.1.2\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let backend = super::super::Backend::new(paths);
        assert_eq!(backend.identity().version, "2.1.2");
    }
}
