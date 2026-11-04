//! OSD level notices, never requests to archr-levels (even reads delay its save).
use super::{Id, Paths, Reply};
use std::{
    fs, io,
    os::unix::{fs::PermissionsExt, net::UnixDatagram},
    sync::{mpsc, Arc, Weak},
    thread,
    time::Duration,
};

pub(super) fn start(paths: &Paths, results: mpsc::Sender<Reply>) -> Result<Arc<()>, String> {
    let alive = Arc::new(());
    let weak = Arc::downgrade(&alive);
    let dir = if paths.root.is_some() {
        paths.at("/run")
    } else {
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(Into::into)
            .unwrap_or_else(|| "/run".into())
    };
    thread::Builder::new()
        .name("launcher-levels".into())
        .spawn(move || {
            let path = dir.join("archr-launcher-levels.sock");
            let listen = || -> io::Result<()> {
                // Do not steal a live listener's pathname; stale sockets are safe to replace.
                if UnixDatagram::unbound()?.connect(&path).is_ok() {
                    return Err(io::Error::new(io::ErrorKind::AddrInUse, "level listener"));
                }
                let _ = fs::remove_file(&path);
                let socket = UnixDatagram::bind(&path)?;
                let result = receive(&socket, &path, &weak, &results);
                let _ = fs::remove_file(&path);
                result
            };
            if let Err(error) = listen() {
                log::warn!("Level notices: {}: {error}", path.display());
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(alive)
}

fn receive(
    socket: &UnixDatagram,
    path: &std::path::Path,
    alive: &Weak<()>,
    results: &mpsc::Sender<Reply>,
) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    // Only a shutdown check: idle listeners do not read files or run processes.
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut buf = [0; 64];
    while alive.strong_count() > 0 {
        let n = match socket.recv(&mut buf) {
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue
            }
            Err(e) => return Err(e),
        };
        if let Some((id, value)) = std::str::from_utf8(&buf[..n]).ok().and_then(parse) {
            if results.send(Reply::LiveLevel(id, value)).is_err() {
                break;
            }
        }
    }
    Ok(())
}

fn parse(text: &str) -> Option<(Id, String)> {
    let words: Vec<_> = text.split_ascii_whitespace().collect();
    let (id, level) = match words[..] {
        ["volume", level] | ["volume", level, "muted"] => (Id::Volume, level),
        ["brightness", level] => (Id::Brightness, level),
        _ => return None,
    };
    let n = level.parse::<u8>().ok()?;
    (n <= 100).then(|| (id, id.normalize(level)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notices_accept_only_levels_and_normalize_brightness() {
        assert_eq!(parse("volume 70 muted"), Some((Id::Volume, "70".into())));
        assert_eq!(parse("brightness 0"), Some((Id::Brightness, "5".into())));
        for text in [
            "volume 101",
            "volume -1",
            "volume 6.5",
            "brightness 40 muted",
            "battery-low 10",
            "volume 70 extra",
            "volume",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
    }
}
