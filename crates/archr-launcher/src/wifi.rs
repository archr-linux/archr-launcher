//! Wi-Fi radio switch. `wifictl` runs off the render thread under a deadline,
//! one request at a time; a press while one is running is ignored.
use crate::paths::Paths;
use archr_config::SystemCfg;
use std::{
    io,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(10);
static BUSY: AtomicBool = AtomicBool::new(false);

fn start(program: &std::path::Path, arg: &str) -> io::Result<std::process::Child> {
    Command::new(program)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// True when the child exits successfully within `deadline`; a hung one is killed.
fn wait(mut child: std::process::Child, deadline: Duration) -> bool {
    let end = Instant::now() + deadline;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Switches the radio and records it in `wifi.enabled` so it survives a reboot.
/// `refreshed` runs once the radio state may have changed; enabling then asks
/// `wifictl` to connect without holding the switch.
pub fn toggle(
    paths: &Paths,
    enable: bool,
    refreshed: Option<impl FnOnce() + Send + 'static>,
) -> io::Result<()> {
    if BUSY.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let program = paths.at("/usr/bin/wifictl");
    let (config, lock) = (paths.config.clone(), paths.lock.clone());
    thread::Builder::new()
        .name("launcher-wifi".into())
        .spawn(move || {
            let arg = if enable { "enable" } else { "disable" };
            let done = start(&program, arg).is_ok_and(|child| wait(child, DEADLINE));
            if done {
                let value = if enable { "1" } else { "0" };
                if let Err(error) = SystemCfg::set_and_save(&config, &lock, "wifi.enabled", value) {
                    log::warn!("Cannot save wifi.enabled: {error}");
                }
            } else {
                log::warn!("wifictl {arg} failed");
            }
            if let Some(refreshed) = refreshed {
                refreshed();
            }
            BUSY.store(false, Ordering::Release);
            if done && enable {
                if let Ok(mut connect) = start(&program, "connect") {
                    let _ = connect.wait();
                }
            }
        })
        .map(drop)
        .inspect_err(|_| BUSY.store(false, Ordering::Release))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, sync::mpsc};

    #[test]
    fn hung_helper_is_killed_at_the_deadline() {
        let began = Instant::now();
        let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        assert!(!wait(child, Duration::from_millis(100)));
        assert!(began.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn enable_saves_the_key_then_connects_and_overlapping_requests_are_ignored() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let log = root.path().join("log");
        let script = paths.at("/usr/bin/wifictl");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nsleep 0.3\n[ \"$1\" = probe ] || echo \"$1\" >> {}\n",
                log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        // Another test's fork can briefly hold the fresh file open for writing.
        while Command::new(&script).arg("probe").status().is_err() {
            thread::sleep(Duration::from_millis(5));
        }
        fs::write(&paths.config, "audio.volume=60\n").unwrap();
        let (sender, refreshed) = mpsc::channel();
        toggle(&paths, true, Some(move || sender.send(()).unwrap())).unwrap();
        // Still running until the refresh and BUSY release: this one is dropped.
        toggle(&paths, false, None::<fn()>).unwrap();
        refreshed.recv_timeout(Duration::from_secs(5)).unwrap();
        let config = SystemCfg::load(&paths.config).unwrap();
        assert_eq!(config.get("wifi", None, "enabled").as_deref(), Some("1"));
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read_to_string(&log).unwrap_or_default() != "enable\nconnect\n"
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(fs::read_to_string(&log).unwrap(), "enable\nconnect\n");
    }
}
