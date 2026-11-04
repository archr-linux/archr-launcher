//! Direct daemon notices, never polling (even reads delay archr-levels' save).
use super::{Id, Paths, Reply};
use std::{
    fs, io,
    os::unix::{fs::PermissionsExt, net::UnixDatagram},
    sync::{mpsc, Arc, Weak},
    thread,
    time::Duration,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Reading {
    pub value: String,
    pub seq: u64,
}
impl Reading {
    pub fn parse(id: Id, text: &str) -> Option<Self> {
        Self::words(id, text.split_ascii_whitespace())
    }
    fn words<'a>(id: Id, mut words: impl Iterator<Item = &'a str>) -> Option<Self> {
        let value = words.next()?;
        let seq = words.next()?.parse().ok()?;
        if words.next().is_some() || value.parse::<u8>().ok()? > 100 {
            return None;
        }
        Some(Self {
            value: id.normalize(value),
            seq,
        })
    }
}

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
        // A full buffer may be a truncated datagram with a valid-looking prefix.
        if n == buf.len() {
            continue;
        }
        if let Some((id, value)) = std::str::from_utf8(&buf[..n]).ok().and_then(parse) {
            if results.send(Reply::LiveLevel(id, value)).is_err() {
                break;
            }
        }
    }
    Ok(())
}

fn parse(text: &str) -> Option<(Id, Reading)> {
    let mut words = text.split_ascii_whitespace();
    let id = match words.next()? {
        "volume" => Id::Volume,
        "brightness" => Id::Brightness,
        _ => return None,
    };
    Reading::words(id, words).map(|reading| (id, reading))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notices_require_a_sequence_and_normalize_brightness() {
        assert_eq!(
            parse("volume 70 9"),
            Some((
                Id::Volume,
                Reading {
                    value: "70".into(),
                    seq: 9
                }
            ))
        );
        assert_eq!(
            parse("brightness 0 10"),
            Some((
                Id::Brightness,
                Reading {
                    value: "5".into(),
                    seq: 10
                }
            ))
        );
        assert_eq!(
            Reading::parse(Id::Volume, "70 18446744073709551615")
                .unwrap()
                .seq,
            u64::MAX
        );
        for text in [
            "volume 101 1",
            "volume -1 1",
            "volume 6.5 1",
            "brightness 40 muted 1",
            "brightness2 40 1",
            "battery-low 10 1",
            "volume 70 extra",
            "volume 70",
            "volume 70 muted",
            "volume",
            "volume 70 -1",
            "volume 70 18446744073709551616",
            "volume 70 1 extra",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
    }

    #[test]
    fn full_and_truncated_datagrams_cannot_publish_their_valid_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notices");
        let socket = UnixDatagram::bind(&path).unwrap();
        let sender = UnixDatagram::unbound().unwrap();
        sender.connect(&path).unwrap();
        let alive = Arc::new(());
        let weak = Arc::downgrade(&alive);
        let (tx, rx) = mpsc::channel();
        let listener = thread::spawn(move || receive(&socket, &path, &weak, &tx));
        let mut full = [b' '; 64];
        full[..11].copy_from_slice(b"volume 70 9");
        sender.send(&full).unwrap();
        let mut truncated = full.to_vec();
        truncated.extend_from_slice(b"invalid suffix");
        sender.send(&truncated).unwrap();
        sender.send(&[0xff]).unwrap();
        sender.send(b"volume 80 10").unwrap();
        assert!(matches!(rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            Reply::LiveLevel(Id::Volume, Reading { ref value, seq: 10 }) if value == "80"));
        drop(alive);
        sender.send(b"").unwrap();
        listener.join().unwrap().unwrap();
        assert!(rx.try_recv().is_err());
    }
}
