//! archr-osd: the volume/brightness pill drawn over everything, games included.
//!
//!   archr-osd                      run the overlay (from the sway session)
//!   archr-osd send volume 60 [muted]
//!   archr-osd send brightness 40   exit 1 when no overlay is listening
mod overlay;
mod pill;
mod proto;

use smithay_client_toolkit::reexports::client::{
    backend::WaylandError, globals::registry_queue_init, Connection,
};
use std::{
    os::{fd::AsRawFd, unix::fs::PermissionsExt, unix::net::UnixDatagram},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/run".into());
    PathBuf::from(dir).join("archr-osd.sock")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None => serve(),
        Some("send") => send(&args[1..].join(" ")),
        Some(_) => Err("usage: archr-osd [send volume N [muted] | send brightness N]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("archr-osd: {e}");
            ExitCode::FAILURE
        }
    }
}

fn send(text: &str) -> Result<(), String> {
    proto::parse(text).ok_or_else(|| format!("malformed message {text:?}"))?;
    let path = socket_path();
    let socket = UnixDatagram::unbound().map_err(|e| e.to_string())?;
    socket
        .send_to(text.as_bytes(), &path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

fn bind() -> Result<UnixDatagram, String> {
    let path = socket_path();
    if UnixDatagram::unbound()
        .and_then(|s| s.connect(&path))
        .is_ok()
    {
        return Err(format!("already running on {}", path.display()));
    }
    let _ = std::fs::remove_file(&path);
    let socket = UnixDatagram::bind(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .and_then(|()| socket.set_nonblocking(true))
        .map_err(|e| e.to_string())?;
    Ok(socket)
}

/// The latest well-formed message waiting on the socket, if any.
fn drain(socket: &UnixDatagram) -> Option<proto::Message> {
    let mut buf = [0u8; 64];
    let mut latest = None;
    while let Ok(n) = socket.recv(&mut buf) {
        let text = std::str::from_utf8(&buf[..n]).unwrap_or("");
        latest = proto::parse(text).or(latest);
    }
    latest
}

fn serve() -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
    let socket = bind()?;
    let (globals, mut queue) = registry_queue_init(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let mut overlay = overlay::Overlay::new(&globals, &qh)?;
    let mut sleep: Option<Duration> = None;
    loop {
        queue
            .dispatch_pending(&mut overlay)
            .map_err(|e| e.to_string())?;
        queue.flush().map_err(|e| e.to_string())?;
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let mut fds =
            [guard.connection_fd().as_raw_fd(), socket.as_raw_fd()].map(|fd| libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            });
        let timeout = sleep.map_or(-1, |d| d.as_millis().clamp(1, i32::MAX as u128) as i32);
        // SAFETY: two valid pollfd entries, both descriptors outlive the call.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout) };
        if ready > 0 && fds[0].revents != 0 {
            if let Err(e) = guard.read() {
                if !matches!(&e, WaylandError::Io(io) if io.kind() == std::io::ErrorKind::WouldBlock)
                {
                    return Err(format!("wayland: {e}"));
                }
            }
        } else {
            drop(guard);
        }
        queue
            .dispatch_pending(&mut overlay)
            .map_err(|e| e.to_string())?;
        if ready > 0 && fds[1].revents != 0 {
            if let Some(message) = drain(&socket) {
                overlay.show(message, &qh);
            }
        }
        sleep = overlay.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_keeps_the_latest_valid_message() {
        let (a, b) = UnixDatagram::pair().unwrap();
        b.set_nonblocking(true).unwrap();
        assert_eq!(drain(&b), None);
        for text in [
            "volume 10",
            "garbage",
            "brightness 40",
            "volume 6.5",
            "\u{ff}",
        ] {
            a.send(text.as_bytes()).unwrap();
        }
        a.send(&[0xff, 0xfe]).unwrap();
        assert_eq!(drain(&b), proto::parse("brightness 40"));
        assert_eq!(drain(&b), None);
    }
}
