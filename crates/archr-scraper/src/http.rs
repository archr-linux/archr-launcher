//! HTTP seam: the engine only sees `Http`, so tests never touch the network.
use archr_io::read_bounded;
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering::Relaxed},
    thread,
    time::Duration,
};

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NetError {
    Cancelled,
    Timeout,
    Failed,
}

pub trait Http: Send + Sync {
    /// GET `url`; the body is capped at `limit` bytes (larger is `Failed`).
    /// Must return promptly with `Cancelled` once `cancel` is raised.
    /// Implementations must never log or return the URL: it carries credentials.
    fn get(&self, url: &str, limit: u64, cancel: &AtomicBool) -> Result<Response, NetError>;
}

/// The system `curl` (already in the image for ES; it brings TLS and CA
/// certificates). The URL travels on stdin, never in argv.
pub struct Curl {
    pub tmp: PathBuf,
    pub user_agent: String,
}

fn fail<E>(_: E) -> NetError {
    NetError::Failed
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Http for Curl {
    fn get(&self, url: &str, limit: u64, cancel: &AtomicBool) -> Result<Response, NetError> {
        std::fs::create_dir_all(&self.tmp).map_err(fail)?;
        let out = tempfile::NamedTempFile::new_in(&self.tmp).map_err(fail)?;
        let timeout = if limit > 16 << 20 { 240 } else { 30 };
        let mut child = Command::new("curl")
            .args(["-s", "-L", "--max-redirs", "3", "--proto", "=https"])
            .args(["--proto-redir", "=https", "--connect-timeout", "10"])
            .args(["--max-time", &timeout.to_string()])
            .args(["--max-filesize", &limit.to_string(), "-K", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(fail)?;
        let config = format!(
            "user-agent = {}\nurl = {}\noutput = {}\nwrite-out = \"%{{http_code}}\"\n",
            quoted(&self.user_agent),
            quoted(url),
            quoted(&out.path().to_string_lossy()),
        );
        if child
            .stdin
            .take()
            .is_none_or(|mut stdin| stdin.write_all(config.as_bytes()).is_err())
        {
            let _ = child.kill();
            return Err(NetError::Failed);
        }
        let status = loop {
            if cancel.load(Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(NetError::Cancelled);
            }
            match child.try_wait() {
                Ok(Some(_)) => break child.wait_with_output().map_err(fail)?,
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(_) => return Err(NetError::Failed),
            }
        };
        match status.status.code() {
            Some(0) => {}
            Some(28) => return Err(NetError::Timeout),
            _ => return Err(NetError::Failed),
        }
        let code = String::from_utf8_lossy(&status.stdout).trim().parse();
        let body = read_bounded(out.as_file(), limit).map_err(fail)?;
        Ok(Response {
            status: code.map_err(fail)?,
            body,
        })
    }
}
