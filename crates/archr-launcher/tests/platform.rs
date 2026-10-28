use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn sigterm_closes_fullscreen_and_windowed_without_handoff() {
    for args in [vec![], vec!["--windowed"]] {
        let root = tempfile::tempdir().unwrap();
        let log = root.path().join("stderr");
        let run = root.path().join("run/archr-launcher");
        fs::create_dir_all(&run).unwrap();
        fs::write(run.join("last-exit"), "0").unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
            .env("ARCHR_LAUNCHER_ROOT", root.path())
            .env("SDL_VIDEODRIVER", "offscreen")
            .args(args)
            .stderr(fs::File::create(&log).unwrap())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while run.join("last-exit").exists() {
            if child.try_wait().unwrap().is_some() || Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "launcher never became ready: {}",
                    fs::read_to_string(&log).unwrap()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Boot has been sent by the worker; let the event loop consume it.
        std::thread::sleep(Duration::from_millis(100));
        assert!(Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success());
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("SDL Quit did not stop launcher promptly");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(status.success(), "{}", fs::read_to_string(log).unwrap());
        assert!(!run.join("next").exists());
        assert!(root
            .path()
            .join("storage/.config/archr-launcher/store.toml")
            .exists());
    }
}
