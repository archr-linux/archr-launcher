use std::{fs, process::Command};

#[test]
fn cli_bootstrap_is_english_only_and_does_not_read_config_or_catalogs() {
    use std::{
        ffi::CString,
        os::unix::ffi::OsStrExt,
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir().unwrap();
    let config = root
        .path()
        .join("storage/.config/system/configs/system.cfg");
    let catalogs = root.path().join("usr/share/archr-launcher/locales");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::create_dir_all(&catalogs).unwrap();
    // Either config or catalog I/O would block the bootstrap on these FIFOs.
    for path in [&config, &catalogs.join("fr.toml")] {
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a live NUL-terminated path in our private fixture.
        assert_eq!(unsafe { libc_mkfifo(name.as_ptr(), 0o600) }, 0);
    }
    for (args, want, success) in [
        (vec!["--help"], "archr-launcher [--windowed]", true),
        (vec!["--invalid"], "Unknown argument: --invalid", false),
        (
            vec!["--script"],
            "--script requires a button sequence",
            false,
        ),
        (
            vec!["--script", "QUIT"],
            "--script requires --windowed",
            false,
        ),
        (
            vec!["--windowed", "--script", "INVALID"],
            "Invalid script button: INVALID",
            false,
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
            .env("ARCHR_LAUNCHER_ROOT", root.path())
            .env("LANG", "fr_FR.UTF-8")
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("CLI bootstrap blocked on fixture I/O");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.success(), success);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(text.contains(want), "{text}");
        assert!(!text.contains("Cannot read catalog"), "{text}");
    }
}

// Avoid a new launcher dependency just for this POSIX fixture.
unsafe extern "C" {
    #[link_name = "mkfifo"]
    fn libc_mkfifo(path: *const std::os::raw::c_char, mode: u32) -> std::os::raw::c_int;
}
