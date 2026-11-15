//! The controller tester's lane: archr-joypad-cal in order, and the joypad
//! driver's `calibration` file read every `SAMPLE_EVERY` while a
//! calibration step measures. Its own thread, so the settings worker never
//! waits for it and a slow tool never delays a reading.
use super::*;
use archr_ui::settings::joypad::{self, Axes, Command, AXES, SAMPLE_EVERY};
use std::time::Instant;

const TOOL: &str = "archr-joypad-cal";

pub(super) fn lane(
    paths: Paths,
    requests: &mpsc::Receiver<Command>,
    results: &mpsc::Sender<Reply>,
) {
    let backend = Backend::new(paths);
    // The driver's file, once `path` named it.
    let mut file: Option<String> = None;
    let mut sampling = false;
    loop {
        let command = if sampling {
            match requests.recv_timeout(SAMPLE_EVERY) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match requests.recv() {
                Ok(command) => Some(command),
                Err(_) => return,
            }
        };
        let reply = match command {
            None => match file
                .as_deref()
                .and_then(|f| joypad::parse(&backend.read(f)))
            {
                Some(axes) => Reply::JoypadRead(Box::new(axes), Instant::now()),
                None => continue,
            },
            Some(Command::Sample(on)) => {
                sampling = on;
                continue;
            }
            Some(Command::Open) => {
                let (path, axes) = backend.joypad_open();
                file = path;
                Reply::JoypadOpened(axes.map(Box::new))
            }
            Some(command) => {
                let result = backend.joypad_run(&command);
                Reply::JoypadDone(command, result)
            }
        };
        if results.send(reply).is_err() {
            return;
        }
    }
}

impl Backend {
    /// The driver's file and its axes; none when this pad has no
    /// calibration (`path` exits 2) or the file cannot be read.
    fn joypad_open(&self) -> (Option<String>, Option<Axes>) {
        let path = match self.command(TOOL, &["path"]) {
            Ok(path) if !path.is_empty() => path,
            Ok(_) => return (None, None),
            Err(error) => {
                log::info!("No stick calibration: {error}");
                return (None, None);
            }
        };
        match joypad::parse(&self.read(&path)) {
            Some(axes) => (Some(path), Some(axes)),
            None => {
                log::warn!("{path}: not the calibration lines");
                (None, None)
            }
        }
    }
    fn joypad_tool(&self, args: &[String]) -> Result<(), String> {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.command(TOOL, &args).map(drop)
    }
    fn joypad_run(&self, command: &Command) -> Result<(), String> {
        match command {
            Command::Fit(fits) => fits
                .iter()
                .zip(AXES)
                .try_for_each(|(fit, axis)| self.joypad_tool(&fit.args(axis))),
            Command::Save => self.joypad_tool(&["save".into()]),
            Command::Reset => self.joypad_tool(&["reset".into()]),
            // Every axis, even after one failed: as much as possible back.
            Command::Restore(before) => before
                .iter()
                .zip(AXES)
                .map(|(axis, name)| self.joypad_tool(&axis.restore(name)))
                .fold(Ok(()), Result::and),
            Command::Open | Command::Sample(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archr_ui::settings::joypad::Fit;
    use std::os::unix::fs::PermissionsExt;

    const STATUS: &str = "\
x raw=1810 center=1790 fixed=0 neg=100 pos=100 deadzone=40 max=1800 invert=0
y raw=1700 center=1796 fixed=0 neg=100 pos=100 deadzone=40 max=1800 invert=0
rx raw=1802 center=1809 fixed=1 neg=104 pos=98 deadzone=40 max=1800 invert=0
ry raw=1798 center=1795 fixed=0 neg=100 pos=100 deadzone=40 max=1800 invert=0
";
    /// A root with the driver's file and an archr-joypad-cal that logs its
    /// arguments and exits `path_exit` for `path`.
    fn root(path_exit: u8) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(dir.path().join("sys/joypad")).unwrap();
        fs::write(dir.path().join("sys/joypad/calibration"), STATUS).unwrap();
        let tool = bin.join(TOOL);
        fs::write(
            &tool,
            format!(
                "#!/bin/sh\necho \"$*\" >> \"$ARCHR_LAUNCHER_ROOT/calls\"\n\
                 [ \"$1\" = path ] && echo /sys/joypad/calibration && exit {path_exit}\nexit 0\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        dir
    }
    fn start(dir: &tempfile::TempDir) -> (mpsc::Sender<Command>, mpsc::Receiver<Reply>) {
        let (tx, requests) = mpsc::channel();
        let (results, rx) = mpsc::channel();
        let paths = Paths::new(Some(dir.path().into()));
        thread::spawn(move || lane(paths, &requests, &results));
        (tx, rx)
    }
    /// The next reply that is not a reading.
    fn answer(rx: &mpsc::Receiver<Reply>) -> Reply {
        loop {
            match rx.recv_timeout(Duration::from_secs(5)).expect("an answer") {
                Reply::JoypadRead(..) => continue,
                reply => return reply,
            }
        }
    }

    #[test]
    fn the_lane_reads_the_driver_and_runs_the_tool_in_order() {
        let dir = root(0);
        let (tx, rx) = start(&dir);
        tx.send(Command::Open).unwrap();
        let Reply::JoypadOpened(Some(axes)) = answer(&rx) else {
            panic!("calibration offered");
        };
        assert_eq!((axes[0].raw, axes[2].neg), (1810, 104));
        tx.send(Command::Sample(true)).unwrap();
        let Ok(Reply::JoypadRead(read, _)) = rx.recv_timeout(Duration::from_secs(2)) else {
            panic!("a reading while sampling");
        };
        assert_eq!(read, axes);
        tx.send(Command::Sample(false)).unwrap();
        let fit = Fit {
            center: 1808,
            min: 100,
            max: 3500,
            deadzone: 5,
        };
        tx.send(Command::Fit([fit; 4])).unwrap();
        assert!(matches!(
            answer(&rx),
            Reply::JoypadDone(Command::Fit(_), Ok(()))
        ));
        tx.send(Command::Restore(axes.clone())).unwrap();
        assert!(matches!(
            answer(&rx),
            Reply::JoypadDone(Command::Restore(_), Ok(()))
        ));
        tx.send(Command::Save).unwrap();
        assert!(matches!(
            answer(&rx),
            Reply::JoypadDone(Command::Save, Ok(()))
        ));
        let calls = fs::read_to_string(dir.path().join("calls")).unwrap();
        assert_eq!(
            calls.lines().collect::<Vec<_>>(),
            [
                "path",
                "fit x 1808 100 3500 5",
                "fit y 1808 100 3500 5",
                "fit rx 1808 100 3500 5",
                "fit ry 1808 100 3500 5",
                "set x center=auto neg=100 pos=100 deadzone=40",
                "set y center=auto neg=100 pos=100 deadzone=40",
                "set rx center=1809 neg=104 pos=98 deadzone=40",
                "set ry center=auto neg=100 pos=100 deadzone=40",
                "save",
            ]
        );
    }

    #[test]
    fn a_pad_without_calibration_is_told_so() {
        let dir = root(2);
        let (tx, rx) = start(&dir);
        tx.send(Command::Open).unwrap();
        assert!(matches!(answer(&rx), Reply::JoypadOpened(None)));
        // No file to read: sampling sends nothing.
        tx.send(Command::Sample(true)).unwrap();
        assert!(rx.recv_timeout(SAMPLE_EVERY * 4).is_err());
    }
}
