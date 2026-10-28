use super::Backend;
use archr_ui::{
    settings::{Id, InfoCard, Information},
    status::read_battery,
};
use std::{ffi::CString, fs, os::unix::ffi::OsStrExt};
fn percent(used: u64, total: u64) -> Option<u8> {
    used.saturating_mul(100)
        .checked_div(total)
        .map(|v| v.min(100) as u8)
}
fn card(title: &'static str, lines: Vec<String>, percent: Option<u8>) -> InfoCard {
    InfoCard {
        title,
        lines,
        percent,
    }
}
impl Backend {
    pub(super) fn refresh_info(&self) -> Information {
        let mut info = self
            .identity
            .get_or_init(|| {
                let cpu = self.read("/proc/cpuinfo");
                let hardware = cpu
                    .lines()
                    .filter_map(|l| l.split_once(':'))
                    .find(|(k, _)| matches!(k.trim(), "Hardware" | "model name"))
                    .map_or("", |(_, v)| v.trim());
                let device = self
                    .read("/sys/firmware/devicetree/base/model")
                    .trim_end_matches('\0')
                    .into();
                let rk3326 = hardware.to_lowercase().contains("rk3326")
                    || self
                        .read("/sys/firmware/devicetree/base/compatible")
                        .contains("rk3326");
                let cores = cpu.lines().filter(|l| l.starts_with("processor")).count();
                Information {
                    version: self
                        .read("/etc/os-release")
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("VERSION_ID=")
                                .map(|v| v.trim_matches('"').into())
                        })
                        .unwrap_or_else(|| self.read("/etc/version")),
                    device,
                    kernel: self.read("/proc/sys/kernel/osrelease"),
                    uptime: String::new(),
                    cards: vec![
                        card(
                            "settings.processor",
                            vec![if rk3326 {
                                format!("{cores} × ARM Cortex-A35")
                            } else {
                                hardware.into()
                            }],
                            None,
                        ),
                        card(
                            "settings.graphics",
                            if rk3326 {
                                vec!["Mali-G31".into()]
                            } else {
                                vec![]
                            },
                            None,
                        ),
                    ],
                }
            })
            .clone();
        let seconds = self
            .read("/proc/uptime")
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.) as u64;
        info.uptime = format!("{}:{:02}", seconds / 3600, seconds / 60 % 60);
        let freq = self
            .read("/sys/devices/system/cpu/cpufreq/policy0/scaling_cur_freq")
            .parse::<u64>()
            .ok()
            .map(|v| format!("{} MHz", v / 1000));
        let temp = self
            .read("/sys/class/thermal/thermal_zone0/temp")
            .parse::<i64>()
            .ok()
            .filter(|v| (0..=200_000).contains(v))
            .map(|v| format!("{} °C", v / 1000));
        info.cards[0].lines.push(
            [freq, temp]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
        );
        info.cards[0]
            .lines
            .push(self.read("/sys/devices/system/cpu/cpufreq/policy0/scaling_governor"));
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
        });
        let configured = self
            .cfg()
            .ok()
            .and_then(|c| c.get_setting(Id::Gpu.key(), None, None))
            .unwrap_or_else(|| Id::Gpu.default_value().into());
        let active = active.map(|v| {
            if v.contains("mali") {
                "libmali".into()
            } else {
                v
            }
        });
        info.cards[1].lines.push(format!(
            "settings.active|{}",
            active.as_deref().unwrap_or("?")
        ));
        if active.as_deref() != Some(&configured) {
            info.cards[1]
                .lines
                .push(format!("settings.after_reboot|{configured}"));
        }
        let resolution = self
            .read("/sys/class/graphics/fb0/virtual_size")
            .replace(',', " × ");
        info.cards[1].lines.push(resolution);
        let memory = self.read("/proc/meminfo");
        let kb = |key| {
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
        };
        let total = kb("MemTotal:");
        let used = total.saturating_sub(kb("MemAvailable:"));
        info.cards.push(card(
            "settings.memory",
            if total == 0 {
                vec![]
            } else {
                vec![format!("{} / {} MiB", used / 1024, total / 1024)]
            },
            percent(used, total),
        ));
        let mut storage = card("settings.storage", vec![], None);
        if let Ok(path) = CString::new(self.paths.at("/storage/roms").as_os_str().as_bytes()) {
            let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: valid NUL-terminated path and writable statvfs output.
            if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } == 0 {
                // SAFETY: successful statvfs initialized every field.
                let s = unsafe { stat.assume_init() };
                let total = s.f_blocks.saturating_mul(s.f_frsize);
                let used = s
                    .f_blocks
                    .saturating_sub(s.f_bfree)
                    .saturating_mul(s.f_frsize);
                storage.lines.push(format!(
                    "{:.1} / {:.1} GiB",
                    used as f64 / 1073741824.,
                    total as f64 / 1073741824.
                ));
                storage.percent = percent(used, total);
            }
        }
        info.cards.push(storage);
        let network = self
            .command("ip", &["-o", "-4", "addr", "show", "scope", "global"])
            .ok()
            .and_then(|s| {
                s.lines().find_map(|l| {
                    let w: Vec<_> = l.split_whitespace().collect();
                    (w.len() >= 4)
                        .then(|| vec![w[1].into(), w[3].split('/').next().unwrap_or("").into()])
                })
            })
            .unwrap_or_default();
        info.cards.push(card("settings.network", network, None));
        let battery = read_battery(&self.paths.at("/"));
        info.cards.push(card(
            "settings.battery",
            battery
                .map(|b| {
                    vec![
                        format!("{}%", b.percent),
                        if b.charging {
                            "settings.charging"
                        } else {
                            "settings.discharging"
                        }
                        .into(),
                    ]
                })
                .unwrap_or_default(),
            battery.map(|b| b.percent),
        ));
        info
    }
}
