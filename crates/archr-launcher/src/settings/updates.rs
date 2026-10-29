//! archr-update on worker threads: the check's JSON report, and an update's
//! JSON lines read as they arrive. SDL's thread only drains the channels.
use super::Backend;
use crate::paths::Paths;
use archr_ui::settings::updates::{Package, Progress, Reboot, Report, Run, Stage, Updates};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::Stdio,
    sync::{atomic::AtomicBool, mpsc},
    thread,
    time::{Duration, Instant},
};

/// The sync and the package list on a slow mirror.
const CHECK_TIMEOUT: Duration = Duration::from_secs(180);

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
fn number(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or_default()
}
/// `check --json`: the report, or the reason code of a failure.
pub(super) fn report(json: &str) -> Result<Report, String> {
    let value: Value = serde_json::from_str(json.trim()).map_err(|_| "unknown".to_owned())?;
    if let Some(reason) = value["error"].as_str() {
        log::warn!("Update check failed: {}", text(&value, "message"));
        return Err(reason.to_owned());
    }
    let packages = value["packages"]
        .as_array()
        .ok_or("unknown")?
        .iter()
        .map(|p| Package {
            name: text(p, "name"),
            old: text(p, "old"),
            new: text(p, "new"),
            size: number(p, "size"),
        })
        .collect();
    Ok(Report {
        version: text(&value, "os_version"),
        channel: text(&value, "channel"),
        packages,
        download: number(&value, "download_size"),
        reboot: Reboot::parse(&text(&value, "reboot_reason")),
        new_version: text(&value, "new_version"),
        news: text(&value, "news"),
    })
}
/// One `update --progress` line into the run on screen.
pub(super) fn apply(updates: &mut Updates, line: &str) {
    let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
        return;
    };
    let mut progress = match &updates.run {
        Some(Run::Working(progress)) => progress.clone(),
        _ => Progress::default(),
    };
    let percent = number(&event, "percent").min(100) as u8;
    progress.stage = match event["phase"].as_str().unwrap_or_default() {
        "sync" => Stage::Sync,
        "download" => {
            progress.package = text(&event, "package");
            progress.bytes = number(&event, "bytes");
            progress.total = number(&event, "total");
            progress.speed = number(&event, "speed");
            progress.percent = percent;
            Stage::Download
        }
        "install" => {
            progress.package = text(&event, "package");
            progress.current = number(&event, "current") as usize;
            progress.count = number(&event, "count") as usize;
            progress.percent = percent;
            Stage::Install
        }
        "hooks" => Stage::Hooks,
        "done" => {
            updates.run = Some(Run::Done {
                reboot: Reboot::parse(&text(&event, "reboot_reason")),
                from: text(&event, "old_version"),
                to: text(&event, "new_version"),
                count: number(&event, "count") as usize,
            });
            return;
        }
        "error" => {
            updates.run = Some(Run::Failed {
                reason: text(&event, "reason"),
                line: text(&event, "line"),
                changed: event["changed"].as_bool().unwrap_or(false),
            });
            return;
        }
        _ => return,
    };
    updates.run = Some(Run::Working(progress));
}

pub(super) struct Check(mpsc::Receiver<Result<Report, String>>);
impl Check {
    pub(super) fn start(paths: Paths) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(Backend::new(paths).update_check());
        });
        Self(rx)
    }
    /// The finished check, once.
    pub(super) fn poll(&self) -> Option<Result<Report, String>> {
        self.0.try_recv().ok()
    }
}

enum Line {
    Event(String),
    /// The script ended; nothing more comes.
    Exit,
}
/// A running `update --progress`. Dropping it leaves the script running:
/// past the download nothing may stop it.
pub(super) struct Install {
    rx: mpsc::Receiver<Line>,
    pid: i32,
}
impl Install {
    pub(super) fn start(paths: &Paths) -> Result<Self, String> {
        let mut command = Backend::new(paths.clone()).tool("archr-update");
        command
            .args(["update", "--progress"])
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = crate::process::spawn(&mut command).map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().ok_or("no output")?;
        let pid = child.id() as i32;
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("launcher-update".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(Line::Event(line)).is_err() {
                        break;
                    }
                }
                let _ = child.wait();
                let _ = tx.send(Line::Exit);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { rx, pid })
    }
    /// Only the download stops: the script hands SIGTERM to pacman as SIGINT.
    pub(super) fn cancel(&self) {
        // SAFETY: kill(2) of our own child, by pid. The reader thread reaps it
        // after its last line; a cancel only comes while it still downloads.
        unsafe { libc::kill(self.pid, libc::SIGTERM) };
    }
    /// Applies what arrived; true once the script has ended.
    pub(super) fn poll(&self, updates: &mut Updates) -> (bool, bool) {
        let mut changed = false;
        while let Ok(line) = self.rx.try_recv() {
            changed = true;
            match line {
                Line::Event(line) => apply(updates, &line),
                Line::Exit => {
                    // Gone without a last word: a crash, never a success.
                    if updates.working() {
                        updates.run = Some(Run::Failed {
                            reason: "unknown".into(),
                            line: String::new(),
                            changed: true,
                        });
                    }
                    return (true, true);
                }
            }
        }
        (changed, false)
    }
}

impl Backend {
    fn update_check(&self) -> Result<Report, String> {
        let mut command = self.tool("archr-update");
        command.args(["check", "--json"]);
        let output = crate::process::capture(
            &mut command,
            CHECK_TIMEOUT,
            &AtomicBool::new(false),
            256 * 1024,
        )
        .map_err(|e| {
            log::warn!("archr-update check: {e}");
            "unknown".to_owned()
        })?;
        report(&String::from_utf8_lossy(&output.stdout))
    }
}

/// A finished check into the page: the report or why it failed, and when.
pub(super) fn checked(updates: &mut Updates, result: Result<Report, String>) {
    updates.checking = false;
    updates.checked = Some(Instant::now());
    match result {
        Ok(report) => {
            updates.report = Some(report);
            updates.failed = None;
        }
        Err(reason) => updates.failed = Some(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Runtime;
    use crate::{
        paths::Paths,
        state::{Button, Outcome, SessionAction},
    };
    use archr_ui::settings::{
        updates::{Reboot, Run, Stage},
        Page,
    };
    use std::{fs, os::unix::fs::PermissionsExt, time::Duration, time::Instant};

    /// A fake archr-update: `check --json` prints a report, `update --progress`
    /// plays the scenario named in $ROOT/mode, one event per 50 ms. Tools run
    /// with only the fixture's /usr/bin on PATH: system ones by full path.
    const FAKE: &str = r#"#!/bin/sh
say() { echo "$1"; /bin/sleep 0.05; }
if [ "$1" = check ]; then
  echo '{"os_version":"2.1.1","channel":"stable","total":2,"download_size":3145728,"reboot_required":true,"reboot_reason":"kernel","new_version":"2.1.2","news":"Kernel novo.","packages":[{"name":"linux","old":"6.12.94-3","new":"6.12.95-1","size":2097152},{"name":"libnew","old":"","new":"1.0-1","size":1048576}]}'
  exit 0
fi
mode=$(/bin/cat "$ARCHR_LAUNCHER_ROOT/mode")
[ "$mode" = cancel ] && trap 'echo "{\"phase\":\"error\",\"reason\":\"cancelled\",\"message\":\"\",\"line\":\"\",\"changed\":false}"; exit 1' TERM
say '{"phase":"sync"}'
say '{"phase":"download","package":"linux","bytes":1048576,"total":3145728,"percent":33,"speed":524288}'
case "$mode" in
  download) say '{"phase":"error","reason":"download","message":"Download failed.","line":"error: failed retrieving file '"'"'linux-6.12.95-1-aarch64.pkg.tar.zst'"'"'","changed":false}'; exit 1 ;;
  cancel) while :; do /bin/sleep 0.05; done ;;
  crash) exit 3 ;;
esac
say '{"phase":"download","package":"","bytes":3145728,"total":3145728,"percent":100,"speed":0}'
say '{"phase":"install","package":"linux","current":1,"count":2,"percent":50}'
if [ "$mode" = install ]; then
  say '{"phase":"error","reason":"install","message":"Install failed.","line":"error: failed to commit transaction (conflicting files)","changed":true}'
  exit 1
fi
say '{"phase":"install","package":"libnew","current":2,"count":2,"percent":100}'
say '{"phase":"hooks","message":"(1/1) Arming ConditionNeedsUpdate..."}'
echo '{"phase":"done","count":2,"reboot_required":true,"reboot_reason":"kernel","old_version":"2.1.1","new_version":"2.1.2"}'
"#;

    fn fixture(mode: &str) -> (tempfile::TempDir, Runtime) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "").unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("archr-update"), FAKE).unwrap();
        fs::set_permissions(bin.join("archr-update"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.path().join("mode"), mode).unwrap();
        let mut runtime = Runtime::new(paths);
        runtime.active = true;
        runtime.state.loading = false;
        runtime.state.open_updates();
        (dir, runtime)
    }
    /// Polls until `done` holds, returning every stage seen on the way.
    fn until(runtime: &mut Runtime, done: impl Fn(&Runtime) -> bool) -> Vec<Stage> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stages = Vec::new();
        while !done(runtime) {
            runtime.poll();
            if let Some(Run::Working(progress)) = &runtime.state.updates.run {
                if stages.last() != Some(&progress.stage) {
                    stages.push(progress.stage);
                }
            }
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        stages
    }
    fn checked(mode: &str) -> (tempfile::TempDir, Runtime) {
        let (dir, mut runtime) = fixture(mode);
        // The open page checks by itself.
        until(&mut runtime, |r| r.state.updates.report.is_some());
        (dir, runtime)
    }
    #[test]
    fn the_open_page_checks_and_reads_the_report() {
        let (_dir, runtime) = checked("ok");
        let updates = &runtime.state.updates;
        let report = updates.report.as_ref().unwrap();
        assert!(!updates.checking && updates.checked.is_some());
        assert_eq!(report.version, "2.1.1");
        assert_eq!(report.reboot, Some(Reboot::Kernel));
        assert_eq!(report.download, 3_145_728);
        assert_eq!(report.packages[1].old, "");
        assert_eq!(report.news, "Kernel novo.");
        assert_eq!(updates.count(), 2);
    }
    #[test]
    fn progress_events_drive_the_run_to_a_restart() {
        let (_dir, mut runtime) = checked("ok");
        assert_eq!(runtime.handle(Button::A), Outcome::None);
        assert!(runtime.mutations_pending(), "nothing closes meanwhile");
        let stages = until(&mut runtime, |r| {
            matches!(r.state.updates.run, Some(Run::Done { .. }))
        });
        assert_eq!(
            stages,
            [Stage::Sync, Stage::Download, Stage::Install, Stage::Hooks]
        );
        assert_eq!(
            runtime.state.updates.run,
            Some(Run::Done {
                reboot: Some(Reboot::Kernel),
                from: "2.1.1".into(),
                to: "2.1.2".into(),
                count: 2
            })
        );
        until(&mut runtime, |r| !r.mutations_pending());
        assert_eq!(
            runtime.handle(Button::A),
            Outcome::Action(SessionAction::Reboot)
        );
    }
    #[test]
    fn a_failed_download_and_a_failed_install_say_what_happened() {
        for (mode, reason, line, changed) in [
            (
                "download",
                "download",
                "error: failed retrieving file 'linux-6.12.95-1-aarch64.pkg.tar.zst'",
                false,
            ),
            (
                "install",
                "install",
                "error: failed to commit transaction (conflicting files)",
                true,
            ),
            ("crash", "unknown", "", true),
        ] {
            let (_dir, mut runtime) = checked(mode);
            runtime.handle(Button::A);
            until(&mut runtime, |r| !r.mutations_pending());
            assert_eq!(
                runtime.state.updates.run,
                Some(Run::Failed {
                    reason: reason.into(),
                    line: line.into(),
                    changed
                }),
                "{mode}"
            );
            // B goes back to the page; A from the failure tries again.
            runtime.handle(Button::B);
            assert_eq!(runtime.state.page(), Page::Updates);
            assert!(runtime.state.updates.run.is_none());
        }
    }
    #[test]
    fn b_cancels_only_while_downloading() {
        let (_dir, mut runtime) = checked("cancel");
        runtime.handle(Button::A);
        until(
            &mut runtime,
            |r| matches!(&r.state.updates.run, Some(Run::Working(p)) if p.stage == Stage::Download),
        );
        runtime.handle(Button::B);
        until(&mut runtime, |r| !r.mutations_pending());
        assert!(runtime.state.updates.run.is_none());
        assert_eq!(
            runtime.state.error.as_ref().map(|e| e.1.as_str()),
            Some("updates.cancelled")
        );
        assert_eq!(runtime.state.page(), Page::Updates);
    }
}
