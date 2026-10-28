use archr_launcher::{
    moonlight::{parse_output, stream_command, Reply},
    paths::Paths,
};
use archr_ui::moonlight::{Options, Stream};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn parses_pinned_embedded_output_without_inventing_app_ids() {
    let reply = parse_output(
        "Connecting to 192.168.1.5...\r\n1. Steam Big Picture\r\n2. Desktop\r\n3. 12. A game\r\n",
        "list",
        true,
    )
    .unwrap();
    assert_eq!(
        reply,
        Reply::Apps(vec![
            "Steam Big Picture".into(),
            "Desktop".into(),
            "12. A game".into()
        ])
    );
    assert_eq!(
        parse_output("You must pair with the PC first\n", "list", false).unwrap(),
        Reply::NeedsPair
    );
    assert_eq!(
        parse_output("Succesfully paired\n", "pair", true).unwrap(),
        Reply::Paired
    );
    assert_eq!(
        parse_output("Succesfully unpaired\n", "unpair", true).unwrap(),
        Reply::Unpaired
    );
    assert!(parse_output("Failed to pair to server: rejected\n", "pair", true).is_err());
    assert!(parse_output("Failed to unpair to server: rejected\n", "unpair", true).is_err());
    assert!(parse_output("Can't get app list\n", "list", false).is_err());
    assert!(parse_output("1. Desktop\n", "list", false).is_err());
}

#[test]
fn stream_handoff_passes_names_as_data_and_options_as_exact_argv() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(Some(root.path().to_owned()));
    let binary = paths.at("/usr/bin/moonlight");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, "#!/bin/sh\nprintf '%s\\0' \"$@\"\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let stream = Stream {
        host: "192.168.1.5".into(),
        app: "Game ' $(touch BAD) `echo bad` ; \"quoted\"\nsecond line".into(),
        options: Options::new(640, 480),
    };
    let line = stream_command(&paths, &stream).unwrap();
    assert!(!line.contains('\n'));
    let output = Command::new("sh")
        .arg("-c")
        .arg(&line)
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args: Vec<_> = output
        .stdout
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
        .map(|b| String::from_utf8(b.to_vec()).unwrap())
        .collect();
    assert_eq!(
        &args[..3],
        &[
            "stream",
            "-config",
            paths
                .at("/storage/.config/moonlight/moonlight.conf")
                .to_str()
                .unwrap()
        ]
    );
    assert!(args.windows(2).any(|a| a == ["-app", &stream.app]));
    for pair in [
        ["-width", "640"],
        ["-height", "480"],
        ["-fps", "60"],
        ["-bitrate", "5000"],
        ["-codec", "h264"],
    ] {
        assert!(args.windows(2).any(|a| a == pair));
    }
    assert_eq!(args.last().unwrap(), &stream.host);
    assert!(
        !args.iter().any(|arg| arg == "--"),
        "upstream getopt stops before consuming a host after --"
    );
    assert!(!root.path().join("BAD").exists());
    let mut invalid = stream.clone();
    invalid.host = "-help".into();
    assert!(stream_command(&paths, &invalid).is_err());
    invalid.host = "1.2.3.4; touch BAD".into();
    assert!(stream_command(&paths, &invalid).is_err());
}

fn fixture() -> (tempfile::TempDir, Paths) {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(Some(root.path().to_owned()));
    fs::create_dir_all(paths.systems.parent().unwrap()).unwrap();
    fs::write(&paths.systems, "<systemList></systemList>").unwrap();
    fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    fs::write(&paths.config, "system.language=pt_BR\n").unwrap();
    fs::create_dir_all(&paths.store).unwrap();
    fs::write(
        paths.store.join("moonlight.cfg"),
        "moonlight.hosts=192.168.1.5\n",
    )
    .unwrap();
    let binary = paths.at("/usr/bin/moonlight");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, "#!/bin/sh\ncase \"$1\" in list) printf 'Connecting to 192.168.1.5...\\n1. Steam Big Picture\\n2. Desktop\\n';; *) exit 3;; esac\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    (root, paths)
}

#[test]
fn virtual_collection_requires_binary_and_survives_overview_refresh() {
    use archr_launcher::{library::Library, state::App};
    let (_root, paths) = fixture();
    let mut library = Library::initial(paths.clone());
    assert_eq!(App::system_rows(&library), ["@moonlight"]);
    library.start();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !library.counts_known || !library.ready {
        library.poll();
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(App::system_rows(&library), ["@moonlight"]);
    fs::remove_file(paths.at("/usr/bin/moonlight")).unwrap();
    assert!(App::system_rows(&Library::initial(paths)).is_empty());
}

#[test]
fn actual_app_opens_moonlight_and_snapshots_terminal_handoff() {
    let (root, paths) = fixture();
    let frame = root.path().join("stream.png");
    let output = Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
        .env("ARCHR_LAUNCHER_ROOT", root.path())
        .env("SDL_VIDEODRIVER", "offscreen")
        .env("ARCHR_LAUNCHER_SCREENSHOT", &frame)
        .args(["--windowed", "--script", "A,A,SNAPSHOT"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let next = fs::read_to_string(paths.run.join("next")).unwrap();
    assert!(next.starts_with("game ARCHR_MOONLIGHT_BIN="), "{next}");
    assert!(next.contains("ARCHR_MOONLIGHT_APP='Steam Big Picture'"));
    assert!(
        frame.is_file(),
        "stream launch must render the opening notice before exiting"
    );
}
