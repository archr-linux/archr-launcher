//! archr-update on worker threads: the check's JSON report, and an update's
//! JSON lines read as they arrive. SDL's thread only drains the channels.
use super::Backend;
use crate::paths::Paths;
use archr_ui::settings::updates::{Package, Progress, Reboot, Report, Run, Stage, Updates};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    os::fd::AsRawFd,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
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

/// What a run leaves for its followers (archr-update's own paths).
const PROGRESS: &str = "/run/archr-update.progress";
const CANCEL: &str = "/run/archr-update.cancel";
const LOCK: &str = "/run/archr-update.lock";
/// A started run takes the lock and opens its file within this.
const START_TIMEOUT: Duration = Duration::from_secs(20);

enum Line {
    Event(String),
    /// The run ended; nothing more comes.
    Exit,
    /// There was no run to follow.
    Idle,
}
/// A run of `update --progress` in its own systemd unit, followed through
/// its progress file: stopping the launcher never stops pacman, and a new
/// launcher follows a run already going. Dropping this leaves the run alone.
pub(super) struct Install {
    rx: mpsc::Receiver<Line>,
    cancel: Arc<AtomicBool>,
}
impl Install {
    /// Starts the update outside the launcher's session and follows it.
    pub(super) fn start(paths: &Paths) -> Self {
        Self::spawn(paths, |paths, tx| {
            let _ = fs::remove_file(paths.at(PROGRESS));
            let _ = fs::remove_file(paths.at(CANCEL));
            let script = paths.at("/usr/bin/archr-update");
            let script = script.to_string_lossy();
            let backend = Backend::new(paths.clone());
            let args = [
                "--unit=archr-update",
                "--collect",
                "--quiet",
                "--",
                &script,
                "update",
                "--progress",
            ];
            if let Err(error) = backend.command("systemd-run", &args) {
                log::warn!("archr-update not started: {error}");
                let event = serde_json::json!({
                    "phase": "error", "reason": "unknown", "line": error, "changed": false
                });
                let _ = tx.send(Line::Event(event.to_string()));
                let _ = tx.send(Line::Exit);
                return false;
            }
            true
        })
    }
    /// Follows a run that is already going, if any (Idle otherwise).
    pub(super) fn attach(paths: &Paths) -> Self {
        Self::spawn(paths, |paths, tx| {
            let running = locked(&paths.at(LOCK))
                && fs::read_to_string(paths.at(PROGRESS))
                    .is_ok_and(|text| text.lines().last().is_some_and(|l| !terminal(l)));
            if !running {
                let _ = tx.send(Line::Idle);
            }
            running
        })
    }
    fn spawn(paths: &Paths, begin: fn(&Paths, &mpsc::Sender<Line>) -> bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (paths, asked) = (paths.clone(), cancel.clone());
        let spawned = thread::Builder::new()
            .name("launcher-update".into())
            .spawn(move || {
                if begin(&paths, &tx) {
                    follow(&paths, &tx, &asked);
                }
            });
        if let Err(error) = spawned {
            log::warn!("Update follower not started: {error}");
        }
        Self { rx, cancel }
    }
    /// Only the download stops: the run watches for the cancel file.
    pub(super) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    /// Applies what arrived: (anything changed, the follow ended).
    pub(super) fn poll(&self, updates: &mut Updates) -> (bool, bool) {
        let mut changed = false;
        while let Ok(line) = self.rx.try_recv() {
            match line {
                Line::Event(line) => {
                    apply(updates, &line);
                    changed = true;
                }
                Line::Idle => return (changed, true),
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
/// The last line of a run.
fn terminal(line: &str) -> bool {
    serde_json::from_str::<Value>(line)
        .is_ok_and(|v| matches!(v["phase"].as_str(), Some("done" | "error")))
}
/// Someone holds archr-update's lock (flock, like the script).
fn locked(path: &std::path::Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    // SAFETY: flock(2) on a descriptor this function owns; the lock, when
    // taken, goes with the file at the end of the scope.
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) != 0 }
}
/// Reads the progress file as it grows until the run's last line, or until
/// the run is gone (its lock free) or never showed up.
fn follow(paths: &Paths, tx: &mpsc::Sender<Line>, cancel: &AtomicBool) {
    let began = Instant::now();
    let (mut offset, mut partial, mut cancelled) = (0, String::new(), false);
    loop {
        if cancel.load(Ordering::Relaxed) && !cancelled {
            cancelled = fs::write(paths.at(CANCEL), "").is_ok();
        }
        let mut grew = false;
        match fs::File::open(paths.at(PROGRESS)) {
            Ok(mut file) => {
                let mut chunk = String::new();
                if file.seek(SeekFrom::Start(offset)).is_ok() {
                    offset += file.read_to_string(&mut chunk).unwrap_or(0) as u64;
                }
                grew = !chunk.is_empty();
                partial.push_str(&chunk);
                while let Some(end) = partial.find('\n') {
                    let line: String = partial.drain(..=end).collect();
                    let last = terminal(line.trim());
                    if tx.send(Line::Event(line)).is_err() || last {
                        let _ = tx.send(Line::Exit);
                        return;
                    }
                }
                if !grew && !locked(&paths.at(LOCK)) {
                    let _ = tx.send(Line::Exit);
                    return;
                }
            }
            Err(_) if began.elapsed() > START_TIMEOUT => {
                // Most likely another run or check held the lock.
                let event = r#"{"phase":"error","reason":"busy","line":"","changed":false}"#;
                let _ = tx.send(Line::Event(event.into()));
                let _ = tx.send(Line::Exit);
                return;
            }
            Err(_) => {}
        }
        if !grew {
            thread::sleep(Duration::from_millis(100));
        }
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

    /// A fake archr-update: `check --json` prints a report; `update
    /// --progress` takes the lock, then plays the scenario named in
    /// $ROOT/mode into the progress file, one event per 250 ms (the follower
    /// reads every 100 ms, so each shows on its own). Tools run with
    /// only the fixture's /usr/bin on PATH: system ones by full path.
    const FAKE: &str = r#"#!/bin/sh
R="$ARCHR_LAUNCHER_ROOT"
if [ "$1" = check ]; then
  echo '{"os_version":"2.1.1","channel":"stable","total":2,"download_size":3145728,"reboot_required":true,"reboot_reason":"kernel","new_version":"2.1.2","news":"Kernel novo.","packages":[{"name":"linux","old":"6.12.94-3","new":"6.12.95-1","size":2097152},{"name":"libnew","old":"","new":"1.0-1","size":1048576}]}'
  exit 0
fi
[ "$1" = version ] && { echo 2.1.2; exit 0; }
exec 9>>"$R/run/archr-update.lock"
/usr/bin/flock -n 9 || exit 1
: >"$R/run/archr-update.progress"
say() { echo "$1" >>"$R/run/archr-update.progress"; /bin/sleep 0.25; }
mode=$(/bin/cat "$R/mode")
say '{"phase":"sync"}'
say '{"phase":"download","package":"linux","bytes":1048576,"total":3145728,"percent":33,"speed":524288}'
case "$mode" in
  download) say '{"phase":"error","reason":"download","message":"Download failed.","line":"error: failed retrieving file '"'"'linux-6.12.95-1-aarch64.pkg.tar.zst'"'"'","changed":false}'; exit 1 ;;
  cancel)
    while [ ! -e "$R/run/archr-update.cancel" ]; do /bin/sleep 0.05; done
    say '{"phase":"error","reason":"cancelled","message":"","line":"","changed":false}'
    exit 1 ;;
  crash) exit 3 ;;
esac
say '{"phase":"download","package":"","bytes":3145728,"total":3145728,"percent":100,"speed":0}'
say '{"phase":"install","package":"linux","current":1,"count":2,"percent":50}'
if [ "$mode" = install ]; then
  say '{"phase":"error","reason":"install","message":"Install failed.","line":"error: failed to commit transaction (conflicting files)","changed":true}'
  exit 1
fi
[ "$mode" = slow ] && /bin/sleep 1
say '{"phase":"install","package":"libnew","current":2,"count":2,"percent":100}'
say '{"phase":"hooks","message":"(1/1) Arming ConditionNeedsUpdate..."}'
say '{"phase":"done","count":2,"reboot_required":true,"reboot_reason":"kernel","old_version":"2.1.1","new_version":"2.1.2"}'
"#;
    /// systemd-run: records its arguments and starts the command in its own
    /// session, like a unit outside the launcher's.
    const SYSTEMD_RUN: &str = r#"#!/bin/sh
echo "$*" >"$ARCHR_LAUNCHER_ROOT/systemd-run.args"
while [ "${1#-}" != "$1" ]; do shift; done
/usr/bin/setsid "$@" </dev/null >/dev/null 2>&1 &
"#;

    fn fixture(mode: &str) -> (tempfile::TempDir, Runtime) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "").unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        for (name, script) in [("archr-update", FAKE), ("systemd-run", SYSTEMD_RUN)] {
            fs::write(bin.join(name), script).unwrap();
            fs::set_permissions(bin.join(name), fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::create_dir_all(dir.path().join("run")).unwrap();
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
    #[test]
    fn the_run_is_its_own_unit_followed_through_its_file() {
        let (dir, mut runtime) = checked("ok");
        runtime.handle(Button::A);
        until(&mut runtime, |r| !r.mutations_pending());
        let args = fs::read_to_string(dir.path().join("systemd-run.args")).unwrap();
        let script = dir.path().join("usr/bin/archr-update");
        assert_eq!(
            args.trim(),
            format!(
                "--unit=archr-update --collect --quiet -- {} update --progress",
                script.display()
            )
        );
        assert!(matches!(runtime.state.updates.run, Some(Run::Done { .. })));
    }
    #[test]
    fn a_new_launcher_follows_a_run_already_going() {
        let (dir, mut runtime) = fixture("slow");
        // A run left by the launcher before this one.
        let script = dir.path().join("usr/bin/archr-update");
        let mut run = std::process::Command::new(&script);
        run.args(["update", "--progress"])
            .env("ARCHR_LAUNCHER_ROOT", dir.path());
        let mut run = run.spawn().unwrap();
        let progress = dir.path().join("run/archr-update.progress");
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read_to_string(&progress).map_or(true, |t| !t.contains("install")) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        runtime.state.updates = Default::default();
        runtime.watch_update();
        until(&mut runtime, |r| r.state.updates.working());
        assert!(
            runtime.take_reattached(),
            "the host opens the progress screen"
        );
        assert!(runtime.mutations_pending());
        until(&mut runtime, |r| !r.mutations_pending());
        assert!(matches!(runtime.state.updates.run, Some(Run::Done { .. })));
        run.wait().unwrap();
        // Nothing going: the watch ends quietly.
        runtime.state.updates.run = None;
        runtime.watch_update();
        until(&mut runtime, |r| !r.mutations_pending());
        assert!(!runtime.take_reattached());
        assert!(runtime.state.updates.run.is_none());
    }
}
