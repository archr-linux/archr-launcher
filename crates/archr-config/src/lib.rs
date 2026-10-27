//! Ordered `system.cfg` access using the shell helpers' PID-file lock protocol.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(100);
const PID_PUBLICATION_GRACE: Duration = Duration::from_secs(2);

/// A snapshot of the configuration, including comments and original line endings.
pub struct SystemCfg {
    lines: Vec<String>,
}

impl SystemCfg {
    /// Read a snapshot without changing the configuration.
    pub fn load(path: &Path) -> io::Result<SystemCfg> {
        let contents = fs::read_to_string(path)?;
        Ok(Self {
            lines: contents.split_inclusive('\n').map(String::from).collect(),
        })
    }

    /// Look up an exact key in game, system, then global scope.
    pub fn get(&self, system: &str, rom: Option<&str>, key: &str) -> Option<String> {
        rom.and_then(|name| self.entry(&rom_key(system, name, key)))
            .or_else(|| self.entry(&format!("{system}.{key}")))
            .or_else(|| self.entry(&format!("global.{key}")))
            .map(String::from)
    }

    /// Game then system scope only, for keys whose fallback is es_systems.cfg.
    pub fn get_local(&self, system: &str, rom: Option<&str>, key: &str) -> Option<String> {
        rom.and_then(|name| self.entry(&rom_key(system, name, key)))
            .or_else(|| self.entry(&format!("{system}.{key}")))
            .map(String::from)
    }

    /// Re-read under the shared shell lock and atomically replace one entry.
    ///
    /// A live lock holder is retried for up to ten seconds. Dead PID locks are
    /// reclaimed; invalid contents receive a two-second grace period for shell
    /// PID publication. The lock is released on success and I/O errors, provided
    /// it still contains our PID.
    pub fn set_and_save(path: &Path, lock: &Path, entry_key: &str, value: &str) -> io::Result<()> {
        Self::set_many_and_save(path, lock, &[(entry_key, value)])
    }

    /// Apply all entries under one lock and publish them with one atomic rename.
    pub fn set_many_and_save(path: &Path, lock: &Path, entries: &[(&str, &str)]) -> io::Result<()> {
        Self::set_many_with_timeout(path, lock, entries, LOCK_TIMEOUT)
    }

    fn entry(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|line| entry_value(line, key))
    }

    #[cfg(test)]
    fn set_and_save_with_timeout(
        path: &Path,
        lock: &Path,
        entry_key: &str,
        value: &str,
        timeout: Duration,
    ) -> io::Result<()> {
        Self::set_many_with_timeout(path, lock, &[(entry_key, value)], timeout)
    }

    fn set_many_with_timeout(
        path: &Path,
        lock: &Path,
        entries: &[(&str, &str)],
        timeout: Duration,
    ) -> io::Result<()> {
        let _guard = PidLock::acquire(lock, timeout)?;
        let mut cfg = Self::load(path)?;
        for &(entry_key, value) in entries {
            if let Some(line) = cfg
                .lines
                .iter_mut()
                .find(|line| entry_value(line, entry_key).is_some())
            {
                let (_, ending) = split_line_ending(line);
                *line = format!("{entry_key}={value}{ending}");
            } else {
                if let Some(last) = cfg.lines.last_mut() {
                    if !last.ends_with('\n') {
                        last.push('\n');
                    }
                }
                cfg.lines.push(format!("{entry_key}={value}\n"));
            }
        }
        let mut temporary = tempfile::NamedTempFile::new_in(containing_directory(path))?;
        for line in &cfg.lines {
            temporary.write_all(line.as_bytes())?;
        }
        temporary
            .as_file()
            .set_permissions(fs::metadata(path)?.permissions())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// Build the literal key format understood by the existing shell helpers.
pub fn rom_key(system: &str, rom_file_name: &str, key: &str) -> String {
    format!("{system}[\"{rom_file_name}\"].{key}")
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = line.strip_suffix('\n') {
        (content, "\n")
    } else {
        (line, "")
    }
}

fn entry_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    split_line_ending(line)
        .0
        .strip_prefix(key)?
        .strip_prefix('=')
}

fn containing_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn lock_pid(contents: &[u8]) -> Option<u32> {
    std::str::from_utf8(contents).ok()?.trim().parse().ok()
}

struct LockSnapshot {
    contents: Vec<u8>,
    modified: SystemTime,
}

impl LockSnapshot {
    fn read(path: &Path) -> io::Result<Self> {
        let mut file = fs::File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        Ok(Self {
            contents,
            modified: file.metadata()?.modified()?,
        })
    }

    fn is_stale(&self) -> io::Result<bool> {
        if let Some(pid) = lock_pid(&self.contents) {
            return Ok(!Path::new("/proc").join(pid.to_string()).try_exists()?);
        }
        // Shell writers create the file before writing their PID.
        Ok(self
            .modified
            .elapsed()
            .is_ok_and(|age| age > PID_PUBLICATION_GRACE))
    }
}

struct PidLock {
    path: PathBuf,
    pid: u32,
}

impl PidLock {
    fn check_deadline(path: &Path, deadline: Instant) -> io::Result<()> {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "timed out waiting for configuration lock {}",
                    path.display()
                ),
            ));
        }
        Ok(())
    }

    fn reclaim_if_stale(path: &Path, deadline: Instant) -> io::Result<()> {
        let mut helper_path = path.as_os_str().to_os_string();
        helper_path.push(".reclaim");
        let helper = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(Path::new(&helper_path))?;
        // Keep this file permanently so all reclaimers lock the same inode.
        // Closing the handle releases the advisory lock on every return path.
        loop {
            Self::check_deadline(path, deadline)?;
            match helper.try_lock() {
                Ok(()) => break,
                Err(fs::TryLockError::WouldBlock) => thread::sleep(
                    LOCK_RETRY_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                ),
                Err(fs::TryLockError::Error(error)) => return Err(error),
            }
        }
        Self::check_deadline(path, deadline)?;
        let snapshot = match LockSnapshot::read(path) {
            Ok(snapshot) => snapshot,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        // Discard any earlier stale judgment: another reclaimer may have
        // removed the old lock and a live owner may already have replaced it.
        if snapshot.is_stale()? {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn acquire(path: &Path, timeout: Duration) -> io::Result<Self> {
        let deadline = Instant::now() + timeout;
        let pid = std::process::id();
        let mut published = tempfile::NamedTempFile::new_in(containing_directory(path))?;
        writeln!(published, "{pid}")?;
        loop {
            Self::check_deadline(path, deadline)?;
            // Publish a complete PID in one no-clobber filesystem operation.
            match fs::hard_link(published.path(), path) {
                Ok(()) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                        pid,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let snapshot = match LockSnapshot::read(path) {
                        Ok(snapshot) => snapshot,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                        Err(error) => return Err(error),
                    };
                    if snapshot.is_stale()? {
                        Self::reclaim_if_stale(path, deadline)?;
                        continue;
                    }
                }
                Err(error) => return Err(error),
            }
            thread::sleep(
                LOCK_RETRY_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        if fs::read(&self.path).is_ok_and(|contents| lock_pid(&contents) == Some(self.pid)) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fresh_unpublished_or_invalid_pid_is_not_reclaimed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("system.cfg");
        let lock = dir.path().join(".system.cfg.lock");
        fs::write(&path, "snes.core=old\n").unwrap();

        for contents in [b"".as_slice(), b"not-a-pid\n", b"\xff"] {
            fs::write(&lock, contents).unwrap();
            let timeout = Duration::from_millis(30);
            let started = Instant::now();

            let result =
                SystemCfg::set_and_save_with_timeout(&path, &lock, "snes.core", "new", timeout);

            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
            assert!(started.elapsed() >= timeout);
            assert_eq!(fs::read(&lock).unwrap(), contents);
            assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=old\n");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        }
    }

    #[test]
    fn old_empty_lock_is_reclaimed_without_waiting_for_public_timeout() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("system.cfg");
        let lock = dir.path().join(".system.cfg.lock");
        fs::write(&path, "snes.core=old\n").unwrap();
        fs::File::create(&lock)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(5))
            .unwrap();

        SystemCfg::set_and_save_with_timeout(
            &path,
            &lock,
            "snes.core",
            "new",
            Duration::from_millis(100),
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=new\n");
        assert!(!lock.exists());
        assert!(dir.path().join(".system.cfg.lock.reclaim").exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn guard_does_not_remove_a_successors_pid() {
        let dir = tempdir().unwrap();
        let lock = dir.path().join(".system.cfg.lock");
        let guard = PidLock::acquire(&lock, Duration::from_millis(100)).unwrap();
        assert_eq!(
            fs::read_to_string(&lock).unwrap(),
            format!("{}\n", std::process::id())
        );
        let successor = format!("{}\n", std::process::id() + 1);
        fs::write(&lock, &successor).unwrap();

        drop(guard);

        assert_eq!(fs::read_to_string(&lock).unwrap(), successor);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn competing_reclaimers_rejudge_after_the_first_acquires() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{mpsc, Barrier};

        let dir = tempdir().unwrap();
        let lock = dir.path().join(".system.cfg.lock");
        fs::write(&lock, "4294967295\n").unwrap();
        let both_judged_stale = Barrier::new(2);
        let holders = AtomicUsize::new(0);
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let (checked_tx, checked_rx) = mpsc::channel();

        thread::scope(|scope| {
            let lock = &lock;
            let both_judged_stale = &both_judged_stale;
            let holders = &holders;
            let first = scope.spawn(move || {
                assert!(LockSnapshot::read(lock).unwrap().is_stale().unwrap());
                both_judged_stale.wait();
                PidLock::reclaim_if_stale(lock, Instant::now() + Duration::from_millis(100))
                    .unwrap();
                let guard = PidLock::acquire(lock, Duration::from_millis(100)).unwrap();
                assert_eq!(holders.fetch_add(1, Ordering::SeqCst), 0);
                acquired_tx.send(()).unwrap();
                checked_rx.recv().unwrap();
                assert_eq!(holders.fetch_sub(1, Ordering::SeqCst), 1);
                drop(guard);
            });
            let second = scope.spawn(move || {
                assert!(LockSnapshot::read(lock).unwrap().is_stale().unwrap());
                both_judged_stale.wait();
                acquired_rx.recv().unwrap();

                PidLock::reclaim_if_stale(lock, Instant::now() + Duration::from_millis(100))
                    .unwrap();
                let result = PidLock::acquire(lock, Duration::from_millis(30));
                if result.is_ok() {
                    assert_eq!(
                        holders.fetch_add(1, Ordering::SeqCst),
                        0,
                        "overlapping owners"
                    );
                }
                assert_eq!(result.err().unwrap().kind(), io::ErrorKind::TimedOut);
                assert_eq!(holders.load(Ordering::SeqCst), 1);
                assert_eq!(
                    fs::read_to_string(lock).unwrap(),
                    format!("{}\n", std::process::id())
                );
                checked_tx.send(()).unwrap();
            });
            first.join().unwrap();
            second.join().unwrap();
        });

        assert_eq!(holders.load(Ordering::SeqCst), 0);
        assert!(!lock.exists());
        let helper = fs::File::open(dir.path().join(".system.cfg.lock.reclaim")).unwrap();
        helper.try_lock().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn held_reclaim_helper_obeys_the_original_acquisition_deadline() {
        use std::sync::mpsc;

        let dir = tempdir().unwrap();
        let path = dir.path().join("system.cfg");
        let lock = dir.path().join(".system.cfg.lock");
        fs::write(&path, "snes.core=old\n").unwrap();
        fs::write(&lock, "4294967295\n").unwrap();
        let blocker = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.path().join(".system.cfg.lock.reclaim"))
            .unwrap();
        blocker.lock().unwrap();
        let timeout = Duration::from_millis(30);
        let (completed_tx, completed_rx) = mpsc::channel();

        thread::scope(|scope| {
            let started = Instant::now();
            let worker = scope.spawn(|| {
                let result =
                    SystemCfg::set_and_save_with_timeout(&path, &lock, "snes.core", "new", timeout);
                completed_tx.send(result).unwrap();
            });
            let while_held = completed_rx.recv_timeout(Duration::from_millis(250));
            let elapsed = started.elapsed();
            let contents_while_held = fs::read_to_string(&lock).unwrap();
            // Unblock even the old implementation so a regression fails, not hangs.
            blocker.unlock().unwrap();
            worker.join().unwrap();

            let error = while_held
                .expect("acquisition must finish while the helper is still held")
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            assert!(elapsed >= timeout);
            assert_eq!(contents_while_held, "4294967295\n");
        });

        assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=old\n");
        assert_eq!(fs::read_to_string(&lock).unwrap(), "4294967295\n");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[test]
    fn reclaim_helper_is_kept_and_unlocked_when_main_lock_is_gone() {
        let dir = tempdir().unwrap();
        let lock = dir.path().join(".system.cfg.lock");

        PidLock::reclaim_if_stale(&lock, Instant::now() + Duration::from_millis(100)).unwrap();

        assert!(!lock.exists());
        let helper = fs::File::open(dir.path().join(".system.cfg.lock.reclaim")).unwrap();
        helper.try_lock().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn reclaim_helper_unlocks_on_read_error_without_removing_main_lock() {
        let dir = tempdir().unwrap();
        let lock = dir.path().join(".system.cfg.lock");
        fs::create_dir(&lock).unwrap();

        assert!(
            PidLock::reclaim_if_stale(&lock, Instant::now() + Duration::from_millis(100)).is_err()
        );

        assert!(lock.is_dir());
        let helper = fs::File::open(dir.path().join(".system.cfg.lock.reclaim")).unwrap();
        helper.try_lock().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn live_pid_lock_waits_then_times_out_without_removing_lock() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("system.cfg");
        let lock = dir.path().join(".system.cfg.lock");
        let pid = format!("{}\n", std::process::id());
        fs::write(&path, "snes.core=old\n").unwrap();
        fs::write(&lock, &pid).unwrap();
        let timeout = Duration::from_millis(30);
        let started = Instant::now();

        let error = SystemCfg::set_and_save_with_timeout(&path, &lock, "snes.core", "new", timeout)
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() >= timeout);
        assert_eq!(fs::read_to_string(&lock).unwrap(), pid);
        assert_eq!(fs::read_to_string(&path).unwrap(), "snes.core=old\n");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
