//! Bounded worker-side helpers; callers never run these on SDL's thread.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// A child in its own process group, killed with its group and reaped on drop.
pub(crate) struct Running(pub(crate) Child);
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

/// Spawn, retrying ETXTBSY: a thread that forks while another thread still
/// holds a just-written executable open (until its exec closes the O_CLOEXEC
/// descriptor) makes exec of that file fail briefly.
pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    let mut attempt = 0;
    loop {
        match command.spawn() {
            Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) && attempt < 5 => {
                attempt += 1;
                thread::sleep(Duration::from_millis(10 * attempt));
            }
            result => return result,
        }
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
    capture_with_input(command, None, timeout, cancel, limit)
}

/// `capture` with `input` on stdin, for secrets that must stay out of argv.
/// The input fits the pipe buffer, so writing it never waits on the child.
pub(crate) fn capture_with_input(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
    cancel: &AtomicBool,
    limit: u64,
) -> io::Result<Output> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
    }
    let stdin = match input {
        Some(input) if input.len() <= 4096 => {
            let (reader, mut writer) = io::pipe()?;
            writer.write_all(input)?;
            Stdio::from(reader)
        }
        Some(_) => return Err(io::Error::other("stdin input too large")),
        None => Stdio::null(),
    };
    command
        .process_group(0)
        .stdin(stdin)
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    let child = spawn(command)?;
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
