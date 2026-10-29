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
const FPS: u32 = 25;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

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

/// One running clip. Dropping it asks the reader to kill ffmpeg, which it
/// does after the next frame (a frame comes every 1/FPS s).
struct Player {
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
}
impl Player {
    fn start(path: PathBuf, size: (u32, u32)) -> Self {
        let slot = Arc::new(Mutex::new(Slot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, cancel) = (slot.clone(), stop.clone());
        let spawned = thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                if let Err(error) = play(&path, size, &shared, &cancel) {
                    log::warn!("Preview {}: {error}", path.display());
                }
                shared.lock().unwrap_or_else(|e| e.into_inner()).done = true;
            });
        if spawned.is_err() {
            slot.lock().unwrap_or_else(|e| e.into_inner()).done = true;
        }
        Self { slot, stop }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Clip length in seconds, for the progress line.
fn duration(path: &Path, stop: &AtomicBool) -> Option<f32> {
    let mut command = Command::new("ffprobe");
    command
        .args(["-v", "error", "-show_entries", "format=duration", "-of"])
        .args(["csv=p=0"])
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
) -> Result<(), String> {
    let length = duration(path, stop);
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    let filter = format!(
        "fps={FPS},scale={w}:{h}:force_original_aspect_ratio=increase:flags=bilinear,crop={w}:{h}"
    );
    let mut command = Command::new("ffmpeg");
    command
        .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
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
    fn a_clip_ffmpeg_cannot_read_ends_without_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Broken.mp4");
        std::fs::write(&path, b"not a video").unwrap();
        let player = Player::start(path, (40, 30));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !player.slot.lock().unwrap().done {
            assert!(Instant::now() < deadline, "reader kept running");
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(player.slot.lock().unwrap().seq, 0);
    }
}
