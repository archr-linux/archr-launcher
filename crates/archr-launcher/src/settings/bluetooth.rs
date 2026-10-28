//! Settings effects owned by the Bluetooth page. `archr-bluetooth` does the work;
//! discovery is a stream, so a scan listens for a short window and then stops it.
use super::*;
use archr_ui::settings::bluetooth::{parse_devices, valid_address, Action, Device};
use std::{
    io::{Read, Seek, SeekFrom},
    os::unix::process::CommandExt,
    process::Stdio,
    time::Instant,
};

/// How long a scan listens for devices to show up.
const LISTEN: Duration = Duration::from_secs(10);
/// Pairing waits for the device itself: the script gives up after 60 s.
const PAIR_TIMEOUT: Duration = Duration::from_secs(90);

/// Known failures become locale keys; the raw text goes to the log.
fn bt_error(error: String, fallback: &str) -> String {
    log::warn!("Bluetooth: {error}");
    if error.contains("agent is not running") {
        "settings.bt_unavailable".into()
    } else {
        fallback.into()
    }
}

impl Backend {
    pub(super) fn bt_scan(&self) -> Result<Vec<Device>, String> {
        self.bt_scan_for(LISTEN)
    }
    fn bt_scan_for(&self, listen: Duration) -> Result<Vec<Device>, String> {
        let paired = self
            .command("archr-bluetooth", &["list"])
            .map_err(|e| bt_error(e, "settings.bt_failed"))?;
        let live = self.bt_listen(listen)?;
        Ok(parse_devices(&paired, &live))
    }
    /// Runs `start_live_devices` for `listen`, then ends it the way its own
    /// trap expects (SIGTERM) and returns what it printed.
    fn bt_listen(&self, listen: Duration) -> Result<String, String> {
        let mut output = tempfile::tempfile().map_err(|e| e.to_string())?;
        let mut command = self.tool("archr-bluetooth");
        command
            .arg("start_live_devices")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(output.try_clone().map_err(|e| e.to_string())?)
            .stderr(Stdio::null());
        let mut child = crate::process::spawn(&mut command).map_err(|e| e.to_string())?;
        let end = Instant::now() + listen;
        let mut exited = None;
        while exited.is_none() && Instant::now() < end {
            exited = child.try_wait().map_err(|e| e.to_string())?;
            thread::sleep(Duration::from_millis(50));
        }
        // SAFETY: the child leads its own process group, whose id is its PID.
        let group = -(child.id() as i32);
        if exited.is_none() {
            unsafe { libc::kill(group, libc::SIGTERM) };
            let grace = Instant::now() + Duration::from_secs(3);
            while child.try_wait().map_err(|e| e.to_string())?.is_none() && Instant::now() < grace {
                thread::sleep(Duration::from_millis(50));
            }
        }
        unsafe { libc::kill(group, libc::SIGKILL) };
        let _ = child.wait();
        // Discovery must not outlive the page, even when the trap never ran.
        let _ = self.command_with_timeout(
            "archr-bluetooth",
            &["stop_live_devices"],
            Duration::from_secs(5),
        );
        if exited.is_some_and(|status| !status.success()) {
            return Err(bt_error(
                "start_live_devices failed".into(),
                "settings.bt_unavailable",
            ));
        }
        let mut text = Vec::new();
        output
            .seek(SeekFrom::Start(0))
            .and_then(|_| output.take(256 * 1024).read_to_end(&mut text))
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&text).into_owned())
    }
    pub(super) fn bt_switch(&self, on: bool) -> Result<(), String> {
        self.command("archr-bluetooth", &[if on { "enable" } else { "disable" }])
            .map(drop)
            .map_err(|e| bt_error(e, "settings.bt_failed"))
    }
    pub(super) fn bt_act(&self, action: Action, addr: &str) -> Result<(), String> {
        if !valid_address(addr) {
            return Err("settings.bt_failed".into());
        }
        let run = |verb: &str, timeout, fallback| {
            self.command_with_timeout("archr-bluetooth", &[verb, addr], timeout)
                .map(drop)
                .map_err(|e| bt_error(e, fallback))
        };
        let short = Duration::from_secs(30);
        match action {
            Action::Pair => {
                run("trust", PAIR_TIMEOUT, "settings.bt_pair_failed")?;
                run("connect", short, "settings.bt_failed")
            }
            Action::Connect => run("connect", short, "settings.bt_failed"),
            Action::Disconnect => run("disconnect", short, "settings.bt_failed"),
            Action::Forget => run("remove", short, "settings.bt_failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(script: &str) -> (tempfile::TempDir, Backend) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let tool = paths.at("/usr/bin/archr-bluetooth");
        fs::create_dir_all(tool.parent().unwrap()).unwrap();
        fs::write(&tool, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        (root, Backend::new(paths))
    }
    const SCRIPT: &str = r#"
echo "$@" >> "$ARCHR_LAUNCHER_ROOT/calls"
case "$1" in
list) echo '<device id="00:1A:2B:3C:4D:5E" name="JBL Go 3" type="" connected="no" />' ;;
start_live_devices)
  trap 'echo stopped >> "$ARCHR_LAUNCHER_ROOT/calls"; exit 0' TERM
  echo '<device id="AA:BB:CC:DD:EE:01" name="Xbox" status="added" type="joystick" />'
  /bin/sleep 30 & wait ;;
esac"#;

    #[test]
    fn a_scan_listens_for_the_window_then_stops_discovery_through_the_trap() {
        let (root, backend) = fixture(SCRIPT);
        let began = Instant::now();
        let devices = backend.bt_scan_for(Duration::from_millis(400)).unwrap();
        assert!(began.elapsed() < Duration::from_secs(5));
        let names: Vec<_> = devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["JBL Go 3", "Xbox"]);
        let calls = fs::read_to_string(root.path().join("calls")).unwrap();
        assert!(calls.contains("stopped"), "{calls}");
        assert!(calls.trim_end().ends_with("stop_live_devices"), "{calls}");
    }
    #[test]
    fn a_missing_agent_is_reported_not_listed_empty() {
        let (_root, backend) = fixture("[ \"$1\" = list ] || exit 1");
        assert_eq!(
            backend.bt_scan_for(Duration::from_secs(1)),
            Err("settings.bt_unavailable".to_owned())
        );
    }
    #[test]
    fn pairing_trusts_then_connects_and_bad_addresses_never_reach_the_script() {
        let (root, backend) = fixture("echo \"$@\" >> \"$ARCHR_LAUNCHER_ROOT/calls\"");
        assert!(backend.bt_act(Action::Pair, "AA:BB:CC:DD:EE:01").is_ok());
        assert!(backend.bt_act(Action::Forget, "--help").is_err());
        let calls = fs::read_to_string(root.path().join("calls")).unwrap();
        assert_eq!(
            calls,
            "trust AA:BB:CC:DD:EE:01\nconnect AA:BB:CC:DD:EE:01\n"
        );
    }
    #[test]
    fn a_failed_trust_is_a_pairing_failure() {
        let (_root, backend) = fixture("echo 'Failed to trust device' >&2; exit 1");
        assert_eq!(
            backend.bt_act(Action::Pair, "AA:BB:CC:DD:EE:01"),
            Err("settings.bt_pair_failed".to_owned())
        );
    }
}
