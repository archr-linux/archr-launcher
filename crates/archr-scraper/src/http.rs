//! HTTP seam: the engine only sees `Http`, so tests never touch the network.
use archr_io::read_bounded;
use std::{
    io::{Seek, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
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
    /// POST `form` (already URL-encoded) to `url`; no redirects. The form may
    /// carry passwords: the same never-log rule applies.
    fn post(
        &self,
        _url: &str,
        _form: &str,
        _limit: u64,
        _cancel: &AtomicBool,
    ) -> Result<Response, NetError> {
        Err(NetError::Failed)
    }
}

/// ScreenScraper and its media servers.
pub const SCREENSCRAPER: &[&str] = &["screenscraper.fr"];

/// An https URL on screenscraper.fr or a subdomain. Every URL we fetch
/// there carries our login, so this gates all hops.
pub fn trusted(url: &str) -> bool {
    trusted_on(url, SCREENSCRAPER)
}

/// An https URL on one of `hosts` or a subdomain, nothing else: plain
/// lowercase hostname (no userinfo, port, fragment, backslash or control
/// characters).
pub fn trusted_on(url: &str, hosts: &[&str]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let clean = url
        .bytes()
        .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'\\' | b'#'));
    let host = rest.split(['/', '?']).next().unwrap_or("");
    let plain = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
    clean
        && plain
        && hosts
            .iter()
            .any(|h| host == *h || host.strip_suffix(h).is_some_and(|s| s.ends_with('.')))
}

/// Percent-encoding for query strings and form bodies.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn fail<E>(_: E) -> NetError {
    NetError::Failed
}

/// curl config value; control characters would end the line, so they are refused.
fn quoted(text: &str) -> Result<String, NetError> {
    if text.chars().any(char::is_control) {
        return Err(NetError::Failed);
    }
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Kills and reaps the child on every path out of a request.
struct Reaped(Child);
impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The system `curl` (already in the image for ES; it brings TLS and CA
/// certificates). The URL travels on stdin, never in argv; no user curlrc, no
/// globbing, no inherited environment (the developer login may be in it), and
/// redirects are followed here, one validated hop at a time.
pub struct Curl {
    pub tmp: PathBuf,
    pub user_agent: String,
    /// Hosts every hop must stay on (`trusted_on`).
    pub hosts: &'static [&'static str],
}

impl Curl {
    fn once(
        &self,
        url: &str,
        form: Option<&str>,
        limit: u64,
        cancel: &AtomicBool,
    ) -> Result<(u16, String, Vec<u8>), NetError> {
        if !trusted_on(url, self.hosts) {
            return Err(NetError::Failed);
        }
        std::fs::create_dir_all(&self.tmp).map_err(fail)?;
        let out = tempfile::NamedTempFile::new_in(&self.tmp).map_err(fail)?;
        let mut config = format!(
            "user-agent = {}\nurl = {}\noutput = {}\nwrite-out = \"%{{http_code}} %{{redirect_url}}\"\n",
            quoted(&self.user_agent)?,
            quoted(url)?,
            quoted(&out.path().to_string_lossy())?,
        );
        if let Some(form) = form {
            config += &format!("data = {}\n", quoted(form)?);
        }
        // Redirect metadata goes to a file, not a pipe: a server-chosen Location
        // can be arbitrarily long and must never be able to stall curl.
        let mut meta = tempfile::tempfile_in(&self.tmp).map_err(fail)?;
        // Videos and manuals get longer than a page of art.
        let timeout = if limit > 8 << 20 { 240 } else { 30 };
        let mut command = Command::new("curl");
        command
            .args([
                "-q",
                "-s",
                "--globoff",
                "--max-redirs",
                "0",
                "--proto",
                "=https",
            ])
            .args([
                "--connect-timeout",
                "10",
                "--max-time",
                &timeout.to_string(),
            ])
            .args(["--max-filesize", &limit.to_string(), "-K", "-"])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::from(meta.try_clone().map_err(fail)?))
            .stderr(Stdio::null());
        for key in ["PATH", "SSL_CERT_FILE", "SSL_CERT_DIR"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let mut child = Reaped(command.spawn().map_err(fail)?);
        let mut stdin = child.0.stdin.take().ok_or(NetError::Failed)?;
        stdin.write_all(config.as_bytes()).map_err(fail)?;
        drop(stdin);
        let code = loop {
            if cancel.load(Relaxed) {
                return Err(NetError::Cancelled);
            }
            match child.0.try_wait().map_err(fail)? {
                Some(status) => break status.code(),
                None => thread::sleep(Duration::from_millis(50)),
            }
        };
        match code {
            Some(0) => {}
            Some(28) => return Err(NetError::Timeout),
            _ => return Err(NetError::Failed),
        }
        meta.rewind().map_err(fail)?;
        let line = String::from_utf8(read_bounded(&meta, 2048).map_err(fail)?).map_err(fail)?;
        let (status, location) = line.split_once(' ').unwrap_or((&line, ""));
        let body = read_bounded(out.as_file(), limit).map_err(fail)?;
        Ok((
            status.parse().map_err(fail)?,
            location.trim().to_owned(),
            body,
        ))
    }
}

impl Http for Curl {
    fn get(&self, url: &str, limit: u64, cancel: &AtomicBool) -> Result<Response, NetError> {
        let mut url = url.to_owned();
        for _ in 0..4 {
            let (status, location, body) = self.once(&url, None, limit, cancel)?;
            if !matches!(status, 301 | 302 | 303 | 307 | 308) {
                return Ok(Response { status, body });
            }
            url = location;
        }
        Err(NetError::Failed)
    }
    fn post(
        &self,
        url: &str,
        form: &str,
        limit: u64,
        cancel: &AtomicBool,
    ) -> Result<Response, NetError> {
        let (status, _, body) = self.once(url, Some(form), limit, cancel)?;
        Ok(Response { status, body })
    }
}
