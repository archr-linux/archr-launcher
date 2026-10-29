//! Starting module processes. `Systemd` sandboxes each one in its own unit
//! (docs/modulos.md, "Segurança"); `Direct` runs it plainly, for tests and
//! for development on a PC only.
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant},
};

/// A protocol line is at most this long.
pub const LINE: usize = 64 << 10;
/// stderr lines kept in the launcher's log per process.
const STDERR_LINES: usize = 8;

/// Everything needed to start one module process.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// systemd unit name (`archr-mod-<id>`, with a suffix for one-shots).
    pub unit: String,
    pub id: String,
    /// The module's code, read only.
    pub dir: PathBuf,
    /// Program and arguments.
    pub argv: Vec<OsString>,
    /// The module's writable folder.
    pub data: PathBuf,
    /// Its settings file, read only.
    pub settings: PathBuf,
    /// Its secret values, read only; absent without `secret` rows.
    pub secrets: PathBuf,
    /// The module's own uid, also its gid.
    pub uid: u32,
    pub env: Vec<(String, String)>,
    pub net: bool,
    /// Read-only folders granted by `read`.
    pub read: Vec<PathBuf>,
    pub memory_mb: u32,
}

pub trait Executor: Send + Sync {
    /// The command that runs `plan` with piped stdio.
    fn command(&self, plan: &Plan) -> Command;
    /// Kills one unit, or every module unit with `None`.
    fn kill(&self, unit: Option<&str>);
}

/// `systemd-run --pipe` as the module's own user, one transient unit per
/// process.
pub struct Systemd {
    pub run: PathBuf,
    pub ctl: PathBuf,
}
impl Executor for Systemd {
    fn command(&self, plan: &Plan) -> Command {
        let mut command = Command::new(&self.run);
        command.args(sandbox(plan)).arg("--").args(&plan.argv);
        command.env_clear();
        command
    }
    fn kill(&self, unit: Option<&str>) {
        let pattern = unit.map_or_else(|| "archr-mod-*".to_owned(), |u| format!("{u}.service"));
        let child = Command::new(&self.ctl)
            .args(["kill", "--signal=SIGKILL", "--", &pattern])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(child) = child {
            wait_for(child, Duration::from_secs(2));
        }
    }
}

/// The `systemd-run` arguments for `plan`, before `--`.
pub fn sandbox(plan: &Plan) -> Vec<String> {
    let mut args: Vec<String> = [
        "--pipe",
        "--quiet",
        "--collect",
        "--service-type=exec",
        "-p NoNewPrivileges=yes",
        "-p CapabilityBoundingSet=",
        "-p AmbientCapabilities=",
        "-p ProtectSystem=strict",
        "-p ProtectHome=yes",
        "-p PrivateTmp=yes",
        "-p PrivateDevices=yes",
        "-p ProtectKernelTunables=yes",
        "-p ProtectKernelModules=yes",
        "-p ProtectKernelLogs=yes",
        "-p ProtectControlGroups=yes",
        "-p ProtectClock=yes",
        "-p ProtectHostname=yes",
        // Other uids' processes are hidden: with a uid per module, the
        // other modules' too. systemd 255 has no PrivatePIDs= (257).
        "-p ProtectProc=invisible",
        // Every uid but root and the module's own is `nobody` inside.
        "-p PrivateUsers=yes",
        "-p RestrictSUIDSGID=yes",
        "-p LockPersonality=yes",
        "-p RestrictNamespaces=yes",
        "-p SystemCallFilter=@system-service",
        "-p SystemCallArchitectures=native",
        "-p TemporaryFileSystem=/storage:ro",
        "-p MemorySwapMax=0",
        "-p TasksMax=16",
        "-p CPUWeight=20",
        "-p Nice=10",
        "-p IOWeight=20",
    ]
    .iter()
    .flat_map(|a| a.split_once(' ').map_or(vec![*a], |(f, v)| vec![f, v]))
    .map(str::to_owned)
    .collect();
    args.push(format!("--unit={}", plan.unit));
    args.extend(["-p".into(), format!("User={}", plan.uid)]);
    args.extend(["-p".into(), format!("Group={}", plan.uid)]);
    args.extend(plan.env.iter().map(|(k, v)| format!("--setenv={k}={v}")));
    let mut property = |value: String| args.extend(["-p".to_owned(), value]);
    property(format!("BindReadOnlyPaths={}", plan.dir.display()));
    property(format!("BindPaths={}", plan.data.display()));
    property(format!("BindReadOnlyPaths={}", plan.settings.display()));
    // "-": a path that does not exist is skipped, not an error.
    property(format!("BindReadOnlyPaths=-{}", plan.secrets.display()));
    for path in &plan.read {
        property(format!("BindReadOnlyPaths=-{}", path.display()));
    }
    if !plan.net {
        property("PrivateNetwork=yes".into());
        property("RestrictAddressFamilies=AF_UNIX".into());
    }
    property(format!("MemoryMax={}M", plan.memory_mb));
    property(format!("WorkingDirectory={}", plan.dir.display()));
    args
}

/// The module's user and group as nss-systemd drop-ins (`/run/userdb`):
/// systemd 255 refuses a numeric `User=` that no user database knows.
pub fn user(userdb: &Path, uid: u32) -> io::Result<()> {
    let name = format!("archr-mod-{uid}");
    let user = serde_json::json!({"userName": name, "uid": uid, "gid": uid,
        "disposition": "system", "homeDirectory": "/", "shell": "/usr/bin/nologin", "locked": true});
    let group = serde_json::json!({"groupName": name, "gid": uid, "disposition": "system"});
    for (file, record) in [
        (format!("{uid}.user"), &user),
        (format!("{name}.user"), &user),
        (format!("{uid}.group"), &group),
        (format!("{name}.group"), &group),
    ] {
        let path = userdb.join(file);
        if !path.is_file() {
            archr_io::atomic_publish(&path, false, |file| {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(std::fs::Permissions::from_mode(0o644))?;
                file.write_all(record.to_string().as_bytes())
            })?;
        }
    }
    Ok(())
}

/// No sandbox: the plain process, in its own process group. Only for tests
/// and `--modules-dir` on a development PC.
#[derive(Default)]
pub struct Direct {
    groups: Mutex<Vec<(String, u32)>>,
}
impl Executor for Direct {
    fn command(&self, plan: &Plan) -> Command {
        let mut command = Command::new(&plan.argv[0]);
        command
            .args(&plan.argv[1..])
            .current_dir(&plan.dir)
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .envs(plan.env.iter().map(|(k, v)| (k, v)));
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command
    }
    fn kill(&self, unit: Option<&str>) {
        let mut groups = self.groups.lock().unwrap_or_else(|e| e.into_inner());
        groups.retain(|(name, pid)| {
            let hit = unit.is_none_or(|u| u == name);
            if hit {
                // SAFETY: signalling a process group we created.
                unsafe { libc::kill(-(*pid as i32), libc::SIGKILL) };
            }
            !hit
        });
    }
}
impl Direct {
    fn track(&self, unit: &str, pid: u32) {
        let mut groups = self.groups.lock().unwrap_or_else(|e| e.into_inner());
        groups.retain(|(name, _)| name != unit);
        groups.push((unit.to_owned(), pid));
    }
}

/// Starts `plan`: the child, its stdin, and a thread that hands each stdout
/// line to `line` (`None` once at the end; a line too long ends the reading).
pub fn spawn(
    exec: &dyn Executor,
    direct: Option<&Direct>,
    plan: &Plan,
    line: impl Fn(Option<Result<String, ()>>) + Send + 'static,
) -> std::io::Result<(Child, ChildStdin)> {
    let mut child = exec
        .command(plan)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(direct) = direct {
        direct.track(&plan.unit, child.id());
    }
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let id = plan.id.clone();
    thread::spawn(move || {
        // A bounded share of stderr reaches the launcher's log.
        for text in BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .take(STDERR_LINES)
        {
            log::warn!(
                "Module {id}: {}",
                text.chars().take(200).collect::<String>()
            );
        }
    });
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut bytes = Vec::new();
            match (&mut reader)
                .take(LINE as u64 + 1)
                .read_until(b'\n', &mut bytes)
            {
                Ok(0) | Err(_) => break,
                Ok(_) if bytes.len() > LINE => {
                    line(Some(Err(())));
                    break;
                }
                Ok(_) => line(Some(String::from_utf8(bytes).map_err(|_| ()))),
            }
        }
        line(None);
    });
    Ok((child, stdin))
}

/// A writer thread per process: a module that stops reading never blocks
/// the host. `false` from `send` means the queue is full or closed.
pub fn writer(mut stdin: ChildStdin) -> mpsc::SyncSender<String> {
    let (tx, rx) = mpsc::sync_channel::<String>(64);
    thread::spawn(move || {
        for message in rx {
            if stdin
                .write_all(message.as_bytes())
                .and_then(|()| stdin.write_all(b"\n"))
                .and_then(|()| stdin.flush())
                .is_err()
            {
                break;
            }
        }
    });
    tx
}

/// Reaps `child`, killing it after `limit`; its exit code if it exited.
pub fn wait_for(mut child: Child, limit: Duration) -> Option<i32> {
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sandbox_hides_storage_and_binds_only_what_was_granted() {
        let plan = Plan {
            unit: "archr-mod-x".into(),
            id: "x".into(),
            dir: "/storage/.config/archr-launcher/modules/x".into(),
            data: "/storage/.config/archr-launcher/module-data/x".into(),
            settings: "/storage/.config/archr-launcher/module-settings/x.json".into(),
            secrets: "/storage/.config/archr-launcher/module-settings/x.secret.json".into(),
            uid: 60642,
            read: vec!["/storage/roms".into()],
            memory_mb: 24,
            env: vec![("HOME".into(), "/data".into())],
            ..Plan::default()
        };
        let args = sandbox(&plan).join(" ");
        for expected in [
            "--pipe",
            "--unit=archr-mod-x",
            "-p User=60642",
            "-p Group=60642",
            "-p PrivateUsers=yes",
            "-p ProtectProc=invisible",
            "-p TemporaryFileSystem=/storage:ro",
            "-p BindReadOnlyPaths=/storage/.config/archr-launcher/modules/x",
            "-p BindPaths=/storage/.config/archr-launcher/module-data/x",
            "-p BindReadOnlyPaths=-/storage/roms",
            "-p BindReadOnlyPaths=-/storage/.config/archr-launcher/module-settings/x.secret.json",
            "-p PrivateNetwork=yes",
            "-p MemoryMax=24M",
            "--setenv=HOME=/data",
        ] {
            assert!(args.contains(expected), "{expected} in {args}");
        }
        assert!(!args.contains("archr-mod "), "no shared user: {args}");
        let online = sandbox(&Plan { net: true, ..plan }).join(" ");
        assert!(!online.contains("PrivateNetwork") && !online.contains("AF_UNIX"));
    }

    #[test]
    fn user_records_name_the_uid_for_systemd() {
        let dir = tempfile::tempdir().unwrap();
        user(dir.path(), 60642).unwrap();
        for file in ["60642.user", "archr-mod-60642.user"] {
            let record: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dir.path().join(file)).unwrap()).unwrap();
            assert_eq!(record["uid"], 60642);
            assert_eq!(record["userName"], "archr-mod-60642");
        }
        let group = std::fs::read_to_string(dir.path().join("60642.group")).unwrap();
        assert!(group.contains("\"gid\":60642"), "{group}");
    }
}
