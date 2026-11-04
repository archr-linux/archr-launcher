//! archr-osd: the volume/brightness pill and the battery card, drawn over
//! everything, games included.
//!
//!   archr-osd                      run the overlay (from the sway session)
//!   archr-osd send volume 60 [muted]
//!   archr-osd send brightness 40   exit 1 when no overlay is listening
//!   archr-osd send battery-low 10
//!   archr-osd send battery 3 LEFT TOTAL game|menu
mod card;
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
    sync::mpsc,
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
    proto::parse_notice(text).ok_or_else(|| format!("malformed message {text:?}"))?;
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

/// Forward every level, before pill coalescing, without socket I/O on the UI thread.
fn relay(path: PathBuf) -> Result<mpsc::Sender<String>, String> {
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::Builder::new()
        .name("osd-levels".into())
        .spawn(move || {
            let Ok(socket) = UnixDatagram::unbound() else {
                return;
            };
            if socket.set_nonblocking(true).is_err() {
                return;
            }
            while let Ok(text) = rx.recv() {
                // No launcher (or a full queue) must never delay the OSD.
                let _ = socket.send_to(text.as_bytes(), &path);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(tx)
}

/// The latest well-formed pill and card messages waiting on the socket.
fn drain(
    socket: &UnixDatagram,
    levels: &mpsc::Sender<String>,
) -> (Option<proto::Message>, Option<proto::Alert>) {
    let mut buf = [0u8; 64];
    let (mut pill, mut card) = (None, None);
    while let Ok(n) = socket.recv(&mut buf) {
        let text = std::str::from_utf8(&buf[..n]).unwrap_or("");
        match proto::parse_notice(text) {
            Some(proto::Notice::Level(m)) => {
                let _ = levels.send(text.to_owned());
                pill = Some(m);
            }
            Some(proto::Notice::Alert(a)) => card = Some(a),
            None => {}
        }
    }
    (pill, card)
}

fn serve() -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
    let socket = bind()?;
    let levels = relay(socket_path().with_file_name("archr-launcher-levels.sock"))?;
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
            let (pill, card) = drain(&socket, &levels);
            if let Some(message) = pill {
                overlay.show(message, &qh);
            }
            if let Some(alert) = card {
                overlay.show_card(alert, &qh);
            }
        }
        sleep = overlay.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_delivers_levels_without_a_wayland_session() {
        let path = std::env::temp_dir().join(format!("osd-{}.sock", std::process::id()));
        let listener = UnixDatagram::bind(&path).unwrap();
        listener
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let levels = relay(path.clone()).unwrap();
        let (a, b) = UnixDatagram::pair().unwrap();
        b.set_nonblocking(true).unwrap();
        a.send(b"volume 23").unwrap();
        a.send(b"brightness 81").unwrap();
        drain(&b, &levels);
        let mut buf = [0; 64];
        for text in ["volume 23", "brightness 81"] {
            let n = listener.recv(&mut buf).unwrap();
            assert_eq!(&buf[..n], text.as_bytes());
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn drain_keeps_the_latest_valid_message() {
        let (a, b) = UnixDatagram::pair().unwrap();
        b.set_nonblocking(true).unwrap();
        let (levels, notices) = mpsc::channel();
        assert_eq!(drain(&b, &levels), (None, None));
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
        assert_eq!(drain(&b, &levels), (proto::parse("brightness 40"), None));
        assert_eq!(
            notices.try_iter().collect::<Vec<_>>(),
            ["volume 10", "brightness 40"]
        );
        assert_eq!(drain(&b, &levels), (None, None));
        a.send(b"battery 3 2 8 game").unwrap();
        assert!(matches!(
            drain(&b, &levels),
            (None, Some(proto::Alert::Battery(c))) if c.level == 3
        ));
    }
}
