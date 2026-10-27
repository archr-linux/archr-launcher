//! Ordered `system.cfg` access using the shell helpers' PID-file lock protocol.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(100);

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
    /// A live lock holder is retried for up to ten seconds. Invalid or dead PID
    /// locks are removed. The lock is released on both success and I/O errors.
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

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
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

struct PidLock {
    path: PathBuf,
}

impl PidLock {
    fn acquire(path: &Path, timeout: Duration) -> io::Result<Self> {
        let started = Instant::now();
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
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut file) => {
                    let guard = Self {
                        path: path.to_path_buf(),
                    };
                    writeln!(file, "{}", std::process::id())?;
                    return Ok(guard);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let contents = match fs::read(path) {
                        Ok(contents) => contents,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                        Err(error) => return Err(error),
                    };
                    let pid = std::str::from_utf8(&contents)
                        .ok()
                        .and_then(|contents| contents.trim().parse::<u32>().ok());
                    let live = match pid {
                        Some(pid) => Path::new("/proc").join(pid.to_string()).try_exists()?,
                        None => false,
                    };
                    if !live {
                        match fs::remove_file(path) {
                            Ok(()) => {}
                            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                            Err(error) => return Err(error),
                        }
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
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
