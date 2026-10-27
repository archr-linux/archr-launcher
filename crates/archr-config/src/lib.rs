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

    /// Re-read under the shared shell lock and atomically replace one entry.
    ///
    /// A live lock holder is retried for up to ten seconds. Dead PID locks are
    /// reclaimed; invalid contents receive a two-second grace period for shell
    /// PID publication. The lock is released on success and I/O errors, provided
    /// it still contains our PID.
    pub fn set_and_save(path: &Path, lock: &Path, entry_key: &str, value: &str) -> io::Result<()> {
        Self::set_and_save_with_timeout(path, lock, entry_key, value, LOCK_TIMEOUT)
    }

    fn entry(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|line| entry_value(line, key))
    }

    fn set_and_save_with_timeout(
        path: &Path,
        lock: &Path,
        entry_key: &str,
        value: &str,
        timeout: Duration,
    ) -> io::Result<()> {
        let _guard = PidLock::acquire(lock, timeout)?;
        let mut cfg = Self::load(path)?;
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
    fn reclaim(path: &Path, stale_contents: &[u8]) -> io::Result<()> {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut prefix = path.file_name().unwrap_or_default().to_os_string();
        prefix.push(format!(".stale.{}.{nanos}.", std::process::id()));
        // Reserve a unique quarantine name; rename replaces only our placeholder.
        let renamed = tempfile::Builder::new()
            .prefix(&prefix)
            .tempfile_in(containing_directory(path))?
            .into_temp_path();
        match fs::rename(path, &renamed) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
        let still_stale = LockSnapshot::read(&renamed)
            .and_then(|snapshot| Ok(snapshot.contents == stale_contents && snapshot.is_stale()?));
        if !matches!(still_stale, Ok(true)) {
            // Another reclaimer may already have replaced the judged stale lock.
            // Never overwrite a newer owner when restoring a displaced successor.
            match fs::hard_link(&renamed, path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    // Preserve the displaced file if restoration itself fails.
                    let _ = renamed.keep();
                    return Err(error);
                }
            }
        }
        renamed.close()?;
        still_stale.map(|_| ())
    }

    fn acquire(path: &Path, timeout: Duration) -> io::Result<Self> {
        let started = Instant::now();
        let pid = std::process::id();
        let mut published = tempfile::NamedTempFile::new_in(containing_directory(path))?;
        writeln!(published, "{pid}")?;
        loop {
            if started.elapsed() >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "timed out waiting for configuration lock {}",
                        path.display()
                    ),
                ));
            }
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
                        Self::reclaim(path, &snapshot.contents)?;
                        continue;
                    }
                }
                Err(error) => return Err(error),
            }
            thread::sleep(LOCK_RETRY_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
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
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
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
    fn reclaimer_restores_a_live_successor_of_the_judged_stale_lock() {
        let dir = tempdir().unwrap();
        let lock = dir.path().join(".system.cfg.lock");
        let stale = b"4294967295\n";
        fs::write(&lock, stale).unwrap();
        let judged_contents = fs::read(&lock).unwrap();
        let successor = format!("{}\n", std::process::id());
        fs::write(&lock, &successor).unwrap();

        PidLock::reclaim(&lock, &judged_contents).unwrap();

        assert_eq!(fs::read_to_string(&lock).unwrap(), successor);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
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
