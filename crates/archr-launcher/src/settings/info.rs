use super::Backend;
use archr_ui::settings::{Id, InfoCard, Settings};
use std::{ffi::CString, fs, os::unix::ffi::OsStrExt};
fn field(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        line.split_once('=')
            .filter(|(k, _)| *k == key)
            .map(|(_, value)| value.trim_matches('"').to_owned())
    })
}
fn percent(used: u64, total: u64) -> Option<u8> {
    (total > 0).then(|| (used.saturating_mul(100) / total).min(100) as u8)
}
impl Backend {
    pub(super) fn refresh_info(&self, state: &mut Settings) {
        let release = self.read("/etc/os-release");
        state.version = field(&release, "VERSION_ID").unwrap_or_else(|| self.read("/etc/version"));
        state.kernel = self.read("/proc/sys/kernel/osrelease");
        state.device = self
            .read("/sys/firmware/devicetree/base/model")
            .trim_end_matches('\0')
            .to_owned();
        let cpu = self.read("/proc/cpuinfo");
        let hardware = cpu
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| matches!(key.trim(), "Hardware" | "model name"))
            .map(|(_, v)| v.trim().to_owned())
            .unwrap_or_default();
        let cores = cpu
            .lines()
            .filter(|line| line.starts_with("processor"))
            .count();
        let freq = self
            .read("/sys/devices/system/cpu/cpufreq/policy0/scaling_cur_freq")
            .parse::<u64>()
            .ok()
            .map(|v| format!("{} MHz", v / 1000))
            .unwrap_or_default();
        let memory = self.read("/proc/meminfo");
        let kb = |name: &str| {
            memory
                .lines()
                .find_map(|line| {
                    line.strip_prefix(name)?
                        .split_whitespace()
                        .next()?
                        .parse::<u64>()
                        .ok()
                })
                .unwrap_or(0)
        };
        let total = kb("MemTotal:");
        let used = total.saturating_sub(kb("MemAvailable:"));
        let mut storage_lines = Vec::new();
        let mut storage_percent = None;
        if let Ok(path) = CString::new(self.paths.at("/storage/roms").as_os_str().as_bytes()) {
            let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: NUL-terminated path and valid writable statvfs output.
            if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } == 0 {
                // SAFETY: statvfs initialized every field after successful return.
                let stat = unsafe { stat.assume_init() };
                let total = stat.f_blocks.saturating_mul(stat.f_frsize);
                let used = stat
                    .f_blocks
                    .saturating_sub(stat.f_bfree)
                    .saturating_mul(stat.f_frsize);
                storage_lines.push(format!(
                    "{:.1} / {:.1} GiB",
                    used as f64 / 1073741824.,
                    total as f64 / 1073741824.
                ));
                storage_percent = percent(used, total);
            }
        }
        let mut network = Vec::new();
        if let Ok(output) = self.command("ip", &["-o", "-4", "addr", "show", "scope", "global"]) {
            for line in output.lines().take(2) {
                let words: Vec<_> = line.split_whitespace().collect();
                if words.len() >= 4 {
                    network.extend([
                        words[1].to_owned(),
                        words[3].split('/').next().unwrap_or("").to_owned(),
                    ]);
                    break;
                }
            }
        }
        let battery = fs::read_dir(self.paths.at("/sys/class/power_supply"))
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                fs::read_to_string(path.join("type")).is_ok_and(|s| s.trim() == "Battery")
                    || path.file_name().is_some_and(|s| s == "battery")
            });
        let charge = battery
            .as_ref()
            .and_then(|path| {
                fs::read_to_string(path.join("capacity"))
                    .ok()?
                    .trim()
                    .parse::<u8>()
                    .ok()
            })
            .filter(|v| *v <= 100);
        let charging = battery
            .as_ref()
            .and_then(|path| fs::read_to_string(path.join("status")).ok())
            .is_some_and(|v| v.trim() == "Charging");
        let driver = self
            .cfg()
            .ok()
            .and_then(|cfg| cfg.get_setting(Id::Gpu.key(), None, None))
            .unwrap_or_else(|| Id::Gpu.default_value().into());
        state.cards = vec![
            InfoCard {
                title: "settings.processor",
                lines: vec![hardware, format!("{cores} × ARM Cortex-A35"), freq],
                percent: None,
            },
            InfoCard {
                title: "settings.graphics",
                lines: vec!["Mali-G31".into(), driver],
                percent: None,
            },
            InfoCard {
                title: "settings.memory",
                lines: if total == 0 {
                    vec![]
                } else {
                    vec![format!("{} / {} MiB", used / 1024, total / 1024)]
                },
                percent: percent(used, total),
            },
            InfoCard {
                title: "settings.storage",
                lines: storage_lines,
                percent: storage_percent,
            },
            InfoCard {
                title: "settings.network",
                lines: network,
                percent: None,
            },
            InfoCard {
                title: "settings.battery",
                lines: charge
                    .map(|v| {
                        vec![
                            format!("{v}%"),
                            if charging {
                                "settings.charging"
                            } else {
                                "settings.discharging"
                            }
                            .into(),
                        ]
                    })
                    .unwrap_or_default(),
                percent: charge,
            },
        ];
    }
}
