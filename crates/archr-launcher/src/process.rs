//! Bounded worker-side helpers; callers never run these on SDL's thread.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        // SAFETY: callers create a new child process group, whose id is its PID.
        // try_wait may already have reaped its leader: signal the group anyway
        // to remove descendants. This leaves a small, accepted PID-reuse window.
        unsafe { libc::kill(-(self.0.id() as i32), libc::SIGKILL) };
        let _ = self.0.wait();
    }
}

/// The child must lead its own process group. Reap the group on every exit path.
pub(crate) fn wait(
    child: Child,
    timeout: Duration,
    cancel: &AtomicBool,
    mut check: impl FnMut() -> io::Result<()>,
) -> io::Result<ExitStatus> {
    let mut child = Running(child);
    let end = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        if Instant::now() >= end {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
        }
        check()?;
        if let Some(status) = child.0.try_wait()? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Separate anonymous captures avoid pipe backpressure. The worker checks their
/// sizes every poll and bounds the final read (including output after leader exit).
pub(crate) fn capture(
    command: &mut Command,
    timeout: Duration,
    cancel: &AtomicBool,
    limit: u64,
) -> io::Result<Output> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
    }
    let child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()?;
    let overflow = || io::Error::other("command output limit exceeded");
    let status = wait(child, timeout, cancel, || {
        if stdout.metadata()?.len() > limit || stderr.metadata()?.len() > limit {
            return Err(overflow());
        }
        Ok(())
    })?;
    let read = |file: &mut File| -> io::Result<Vec<u8>> {
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(overflow());
        }
        Ok(bytes)
    };
    Ok(Output {
        status,
        stdout: read(&mut stdout)?,
        stderr: read(&mut stderr)?,
    })
}
