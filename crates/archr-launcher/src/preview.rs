//! The details screen's muted preview video. ffmpeg (already in the image)
//! decodes, scales, crops and paces the clip in its own process; a reader
//! thread keeps only the newest frame, and the draw thread uploads it.
use crate::process;
use sdl2::{
    pixels::PixelFormatEnum,
    render::{Texture, TextureCreator},
    video::WindowContext,
};
use std::{
    io::Read,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

/// Resting on the game this long starts the video.
const DELAY: Duration = Duration::from_secs(1);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const FPS: u32 = 25;

/// The clip and the box it fills, in pixels.
pub type Want = (PathBuf, (u32, u32));

#[derive(Default)]
struct Slot {
    frame: Vec<u8>,
    /// Frames delivered so far; 0 before the first.
    seq: u64,
    progress: f32,
    /// The reader stopped: ffmpeg failed or ended.
    done: bool,
}

/// ffmpeg's process group while it runs unreaped; 0 before and after.
/// The reader clears it, under the lock, before it reaps the child, so a
/// kill through it never reaches a reused PID.
type Group = Arc<Mutex<i32>>;

/// One running clip. Dropping it kills ffmpeg's group at once, wherever the
/// reader is (even blocked before the first frame); the reader then reaps.
struct Player {
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
    group: Group,
}
impl Player {
    fn start(path: PathBuf, size: (u32, u32)) -> Self {
        let slot = Arc::new(Mutex::new(Slot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let group = Group::default();
        let (shared, cancel, pgid) = (slot.clone(), stop.clone(), group.clone());
        let spawned = thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                if let Err(error) = play(&path, size, &shared, &cancel, &pgid) {
                    log::warn!("Preview {}: {error}", path.display());
                }
                shared.lock().unwrap_or_else(|e| e.into_inner()).done = true;
            });
        if spawned.is_err() {
            slot.lock().unwrap_or_else(|e| e.into_inner()).done = true;
        }
        Self { slot, stop, group }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let group = *self.group.lock().unwrap_or_else(|e| e.into_inner());
        if group > 0 {
            // SAFETY: plain syscall; the group is live and unreaped (see Group).
            unsafe { libc::kill(-group, libc::SIGKILL) };
        }
    }
}

/// ffmpeg and ffprobe die with the thread that started them, so a launcher
/// that exits (a game starting) never leaves one behind.
fn die_with_parent(command: &mut Command) {
    // SAFETY: prctl is async-signal-safe; nothing is allocated after fork.
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
}

/// Clip length in seconds, for the progress line.
fn duration(path: &Path, stop: &AtomicBool) -> Option<f32> {
    let mut command = Command::new("ffprobe");
    die_with_parent(&mut command);
    command
        .args(["-v", "error", "-protocol_whitelist", "file", "-f", "mp4"])
        .args(["-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(path);
    let output = process::capture(&mut command, PROBE_TIMEOUT, stop, 256).ok()?;
    let seconds: f32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    (seconds > 0.).then_some(seconds)
}

fn play(
    path: &Path,
    (w, h): (u32, u32),
    slot: &Mutex<Slot>,
    stop: &AtomicBool,
    group: &Mutex<i32>,
) -> Result<(), String> {
    let length = duration(path, stop);
    if stop.load(Ordering::SeqCst) {
        return Ok(());
    }
    let filter = format!(
        "fps={FPS},scale={w}:{h}:force_original_aspect_ratio=increase:flags=bilinear,crop={w}:{h}"
    );
    let mut command = Command::new("ffmpeg");
    die_with_parent(&mut command);
    command
        .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
        // Only a local MP4 file: no network or other protocol, no demuxer guessing.
        .args(["-protocol_whitelist", "file", "-f", "mp4"])
        // Real time, in a loop, one decoding thread: the menus stay first.
        .args(["-re", "-stream_loop", "-1", "-threads", "1", "-i"])
        .arg(path)
        .args(["-an", "-sn", "-dn", "-vf", &filter])
        .args(["-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"])
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let child = process::spawn(&mut command).map_err(|e| format!("ffmpeg: {e}"))?;
    let mut child = process::Running(child);
    {
        let mut pgid = group.lock().unwrap_or_else(|e| e.into_inner());
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        *pgid = child.0.id() as i32;
    }
    // Cleared before any return reaps the child (Running's drop).
    struct Clear<'a>(&'a Mutex<i32>);
    impl Drop for Clear<'_> {
        fn drop(&mut self) {
            *self.0.lock().unwrap_or_else(|e| e.into_inner()) = 0;
        }
    }
    let clear = Clear(group);
    let mut out = child.0.stdout.take().ok_or("ffmpeg: no output")?;
    let mut frame = vec![0; w as usize * h as usize * 3];
    let mut seq = 0u64;
    while !stop.load(Ordering::Relaxed) {
        out.read_exact(&mut frame)
            .map_err(|e| format!("ffmpeg: {e}"))?;
        seq += 1;
        let at = seq as f32 / FPS as f32;
        let mut shared = slot.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::swap(&mut shared.frame, &mut frame);
        if frame.len() != shared.frame.len() {
            frame = vec![0; shared.frame.len()];
        }
        shared.seq = seq;
        shared.progress = length.map_or(0., |l| (at % l) / l);
    }
    drop(clear);
    drop(child);
    Ok(())
}

/// What the details screen shows of the preview, kept across frames.
pub struct Preview<'t> {
    creator: &'t TextureCreator<WindowContext>,
    texture: Option<Texture<'t>>,
    want: Option<Want>,
    since: Instant,
    player: Option<Player>,
    seen: u64,
    progress: f32,
    /// A clip ffmpeg could not play: its still stays, it is not retried.
    failed: Option<PathBuf>,
}
impl<'t> Preview<'t> {
    pub fn new(creator: &'t TextureCreator<WindowContext>) -> Self {
        Self {
            creator,
            texture: None,
            want: None,
            since: Instant::now(),
            player: None,
            seen: 0,
            progress: 0.,
            failed: None,
        }
    }
    /// Follow what should play now; true when the picture changed.
    pub fn poll(&mut self, want: Option<Want>, now: Instant) -> bool {
        let mut changed = false;
        if want != self.want {
            changed = self.seen > 0;
            self.player = None;
            self.seen = 0;
            self.want = want;
            self.since = now;
        }
        let Some((path, (w, h))) = &self.want else {
            return changed;
        };
        if self.player.is_none() {
            if self.failed.as_ref() == Some(path) || now < self.since + DELAY {
                return changed;
            }
            self.player = Some(Player::start(path.clone(), (*w, *h)));
        }
        let Some(player) = &self.player else {
            return changed;
        };
        let slot = player.slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot.done {
            drop(slot);
            self.failed = Some(path.clone());
            self.player = None;
            return std::mem::take(&mut self.seen) > 0 || changed;
        }
        if slot.seq == self.seen {
            return changed;
        }
        let fits = self.texture.as_ref().is_some_and(|t| {
            let q = t.query();
            (q.width, q.height) == (*w, *h)
        });
        if !fits {
            self.texture = self
                .creator
                .create_texture_streaming(PixelFormatEnum::RGB24, *w, *h)
                .map_err(|e| log::warn!("Preview texture: {e}"))
                .ok();
        }
        let Some(texture) = self.texture.as_mut() else {
            return changed;
        };
        if let Err(error) = texture.update(None, &slot.frame, *w as usize * 3) {
            log::warn!("Preview frame: {error}");
            return changed;
        }
        self.seen = slot.seq;
        self.progress = slot.progress;
        true
    }
    /// Stop now, before the launcher hands the screen to a game.
    pub fn stop(&mut self) {
        self.poll(None, Instant::now());
    }
    /// The newest frame and the clip's progress, once one arrived.
    pub fn frame(&self) -> Option<(&Texture<'t>, f32)> {
        (self.seen > 0)
            .then_some(self.texture.as_ref())
            .flatten()
            .map(|t| (t, self.progress))
    }
    /// How long the loop may sleep before the preview needs it again.
    pub fn wait_ms(&self, now: Instant) -> u32 {
        match (&self.want, &self.player) {
            (Some(_), Some(_)) => 1000 / FPS / 2,
            (Some((path, _)), None) if self.failed.as_ref() != Some(path) => {
                (self.since + DELAY)
                    .saturating_duration_since(now)
                    .as_millis() as u32
                    + 1
            }
            _ => u32::MAX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-second clip, when this machine has ffmpeg (the device does).
    fn clip(dir: &Path) -> Option<PathBuf> {
        let path = dir.join("Clip.mp4");
        let made = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-f", "lavfi", "-i"])
            .args(["testsrc=size=160x120:rate=25", "-t", "1", "-c:v", "mpeg4"])
            .arg(&path)
            .status()
            .is_ok_and(|status| status.success());
        made.then_some(path)
    }

    #[test]
    fn frames_arrive_at_the_box_size_with_progress_and_stop_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let Some(path) = clip(dir.path()) else {
            eprintln!("ffmpeg missing: preview test skipped");
            return;
        };
        let player = Player::start(path, (40, 30));
        let deadline = Instant::now() + Duration::from_secs(10);
        let (frame, progress) = loop {
            let slot = player.slot.lock().unwrap();
            assert!(!slot.done, "ffmpeg stopped before a frame");
            if slot.seq >= 3 {
                break (slot.frame.len(), slot.progress);
            }
            drop(slot);
            assert!(Instant::now() < deadline, "no frames");
            thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(frame, 40 * 30 * 3);
        assert!(progress > 0. && progress < 1., "{progress}");
        let slot = player.slot.clone();
        drop(player);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !slot.lock().unwrap().done {
            assert!(Instant::now() < deadline, "reader kept running");
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn dropping_the_player_kills_ffmpeg_blocked_before_its_first_frame() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("ffmpeg missing: preview test skipped");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("Waiting.mp4");
        let name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        // No writer ever comes: ffmpeg waits in open(), no frame arrives.
        let player = Player::start(fifo, (40, 30));
        let deadline = Instant::now() + Duration::from_secs(10);
        let pid = loop {
            let pid = *player.group.lock().unwrap();
            if pid > 0 {
                break pid;
            }
            assert!(Instant::now() < deadline, "ffmpeg never started");
            thread::sleep(Duration::from_millis(10));
        };
        let slot = player.slot.clone();
        drop(player);
        let deadline = Instant::now() + Duration::from_secs(5);
        // SAFETY: signal 0 only checks that the process exists.
        while unsafe { libc::kill(pid, 0) } == 0 {
            assert!(
                Instant::now() < deadline,
                "ffmpeg {pid} outlived its player"
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(slot.lock().unwrap().seq, 0);
    }

    #[test]
    fn a_clip_ffmpeg_cannot_read_ends_without_frames() {
        let dir = tempfile::tempdir().unwrap();
        // A playlist named .mp4 would make ffmpeg fetch its URL: it is read
        // as MP4 only, and refused.
        let playlist = b"#EXTM3U\n#EXTINF:1,\nhttp://127.0.0.1:9/clip.ts\n";
        for (name, bytes) in [("Broken.mp4", &b"not a video"[..]), ("List.mp4", playlist)] {
            check_no_frames(dir.path(), name, bytes);
        }
    }

    fn check_no_frames(dir: &Path, name: &str, bytes: &[u8]) {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        let player = Player::start(path, (40, 30));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !player.slot.lock().unwrap().done {
            assert!(Instant::now() < deadline, "reader kept running");
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(player.slot.lock().unwrap().seq, 0);
    }
}
