use super::*;
use std::{os::unix::fs::PermissionsExt, time::Instant};

fn fixture(script: &str) -> (tempfile::TempDir, Paths) {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(Some(root.path().to_owned()));
    let binary = paths.at("/usr/bin/moonlight");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    (root, paths)
}
fn wait(runtime: &mut Runtime, until: impl Fn(&Runtime) -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        runtime.poll();
        if until(runtime) {
            break;
        }
        assert!(Instant::now() < end, "worker did not finish");
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn generated_pin_is_passed_as_argv_and_visible_before_cli_finishes() {
    let (_root, paths) = fixture(
        r#"
case "$1" in
list) echo 'You must pair with the PC first' >&2; exit 1;;
pair) printf '%s' "$7" > "$(dirname "$0")/pin"; [ "$6" = -pin ] && [ "$8" = 127.0.0.1 ] || exit 9; exec sleep 30;;
esac
"#,
    );
    let mut runtime = Runtime::new(paths.clone(), (640, 480));
    wait(&mut runtime, |r| !r.state.busy);
    runtime.state.hosts = vec!["127.0.0.1".into()];
    runtime.open();
    wait(&mut runtime, |r| !r.state.busy);
    runtime.handle(Button::A);
    wait(&mut runtime, |r| {
        r.state.pin.is_some() && paths.at("/usr/bin/pin").exists()
    });
    let pin = runtime.state.pin.as_ref().unwrap();
    assert_eq!(pin.len(), 4);
    assert!((1..=9999).contains(&pin.parse::<u16>().unwrap()));
    assert_eq!(fs::read_to_string(paths.at("/usr/bin/pin")).unwrap(), *pin);
    assert!(runtime.state.busy);
    runtime.close();
    wait(&mut runtime, |r| !r.state.busy);
}
#[test]
fn reopening_a_cancelled_query_refreshes_instead_of_showing_offline() {
    let (_root, paths) = fixture("printf '1. Desktop\\n'");
    let mut runtime = Runtime::new(paths, (640, 480));
    wait(&mut runtime, |r| !r.state.busy);
    runtime.state.hosts = vec!["127.0.0.1".into()];
    runtime.state.busy = true;
    runtime.close();
    runtime.open();
    runtime
        .tx
        .send(Event::Done(Err("cancelled".into())))
        .unwrap();
    wait(&mut runtime, |r| !r.state.busy);
    assert_eq!(runtime.state.status, Status::Online);
    assert_eq!(runtime.state.apps, ["Desktop"]);
    assert_eq!(runtime.state.error, None);
}
#[test]
fn unpair_forgets_and_persists_even_when_the_computer_is_offline() {
    for success in [true, false] {
        let (_root, paths) = fixture(if success {
            "echo 'Succesfully unpaired'"
        } else {
            "echo 'Cannot connect' >&2; exit 1"
        });
        let mut runtime = Runtime::new(paths.clone(), (640, 480));
        wait(&mut runtime, |r| !r.state.busy);
        runtime.state.hosts = vec!["127.0.0.1".into()];
        runtime.state.confirm_unpair = true;
        runtime.handle(Button::A);
        wait(&mut runtime, |r| !r.state.busy);
        assert!(runtime.state.hosts.is_empty());
        assert!(load(&paths, (640, 480)).unwrap().0.is_empty());
        assert_eq!(
            runtime.state.error,
            (!success).then_some("moonlight.remote_unpair_failed")
        );
    }
}
#[test]
fn deadline_kills_the_process_group_and_output_is_bounded() {
    let (root, paths) = fixture("(sleep 0.3; touch \"$(dirname \"$0\")/leaked\") & wait");
    let cancel = AtomicBool::new(false);
    let result = execute(
        &paths,
        "list",
        "127.0.0.1",
        Duration::from_millis(60),
        &cancel,
        None,
    );
    assert!(result.unwrap_err().contains("timed out"));
    thread::sleep(Duration::from_millis(400));
    assert!(!root.path().join("usr/bin/leaked").exists());
    fs::write(
        paths.at("/usr/bin/moonlight"),
        "#!/bin/sh\nhead -c 300000 /dev/zero\n",
    )
    .unwrap();
    assert!(execute(
        &paths,
        "list",
        "127.0.0.1",
        Duration::from_secs(1),
        &cancel,
        None
    )
    .unwrap_err()
    .contains("output limit"));
}
#[test]
fn worker_uses_real_argv_and_checks_pair_and_unpair_success_markers() {
    let (_root, paths) = fixture(
        r#"
[ "$2" = -config ] && [ "$4" = -keydir ] && [ "$6" = 127.0.0.1 ] || exit 9
case "$1" in
list) printf 'You must pair with the PC first\n' >&2; exit 1;;
pair) printf 'Failed to pair to server: rejected\n' >&2;;
unpair) printf 'Failed to unpair to server: rejected\n' >&2;;
esac
"#,
    );
    let cancel = AtomicBool::new(false);
    assert_eq!(
        execute(
            &paths,
            "list",
            "127.0.0.1",
            Duration::from_secs(1),
            &cancel,
            None
        )
        .unwrap(),
        Reply::NeedsPair
    );
    for action in ["pair", "unpair"] {
        assert!(execute(
            &paths,
            action,
            "127.0.0.1",
            Duration::from_secs(1),
            &cancel,
            None
        )
        .is_err());
    }
}
#[test]
fn add_ip_and_options_round_trip_through_launcher_owned_store() {
    let (_root, paths) = fixture("printf 'Connecting to 127.0.0.1...\\n1. Desktop\\n'");
    let mut runtime = Runtime::new(paths.clone(), (640, 480));
    wait(&mut runtime, |r| !r.state.busy);
    runtime.open();
    runtime.handle(Button::A);
    for key in "127.0.0.1".chars() {
        while Keyboard::KEYS[runtime.state.keyboard.as_ref().unwrap().cursor()] != key.to_string() {
            runtime.handle(Button::R);
        }
        runtime.handle(Button::A);
    }
    while runtime.state.keyboard.as_ref().unwrap().cursor() != 12 {
        runtime.handle(Button::R);
    }
    runtime.handle(Button::A);
    wait(&mut runtime, |r| !r.state.busy);
    assert_eq!(runtime.state.status, Status::Online);
    assert_eq!(runtime.state.apps, ["Desktop"]);
    runtime.handle(Button::Y);
    runtime.handle(Button::R);
    runtime.handle(Button::Down);
    runtime.handle(Button::R);
    runtime.handle(Button::Down);
    runtime.handle(Button::R);
    runtime.handle(Button::Down);
    runtime.handle(Button::R);
    runtime.handle(Button::B);
    wait(&mut runtime, |r| !r.state.busy);
    let (hosts, options) = load(&paths, (640, 480)).unwrap();
    assert_eq!(hosts, ["127.0.0.1"]);
    assert_eq!(
        (
            options.width,
            options.height,
            options.fps,
            options.bitrate,
            options.codec
        ),
        (1280, 720, 30, 10000, Codec::H265)
    );
    assert_eq!(
        runtime.handle(Button::A),
        Outcome::MoonlightStream(Stream {
            host: "127.0.0.1".into(),
            app: "Desktop".into(),
            options
        })
    );
}
#[test]
fn stale_hosts_and_invalid_options_are_not_executed() {
    let (_root, paths) = fixture("exit 99");
    fs::create_dir_all(&paths.store).unwrap();
    fs::write(
        paths.store.join("moonlight.cfg"),
        "moonlight.hosts=-help,127.0.0.1,::1,127.0.0.1,; touch BAD\nmoonlight.fps=0\n",
    )
    .unwrap();
    let (hosts, options) = load(&paths, (800, 480)).unwrap();
    assert_eq!(hosts, ["127.0.0.1"]);
    assert_eq!(options, Options::new(800, 480));
}

#[test]
fn dropping_runtime_after_fatal_ui_error_still_reaps_pairing() {
    let (_root, paths) = fixture(
        r#"
case "$1" in
list) echo 'You must pair with the PC first' >&2; exit 1;;
pair) echo $$ > "$(dirname "$0")/pid"; echo 'Please enter the following PIN on the target PC: 1234'; exec sleep 30;;
esac
"#,
    );
    let mut runtime = Runtime::new(paths.clone(), (640, 480));
    wait(&mut runtime, |r| !r.state.busy);
    runtime.state.hosts = vec!["127.0.0.1".into()];
    runtime.open();
    wait(&mut runtime, |r| !r.state.busy);
    runtime.handle(Button::A);
    wait(&mut runtime, |r| {
        r.state.pin.is_some() && paths.at("/usr/bin/pid").exists()
    });
    let pid: i32 = fs::read_to_string(paths.at("/usr/bin/pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    drop(runtime);
    // SAFETY: signal zero only probes the test child's existence.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "runtime teardown must not orphan the CLI"
    );
}
#[test]
fn closing_does_not_start_another_query_after_queued_pair_success() {
    let (_root, paths) = fixture("exec sleep 30");
    let mut runtime = Runtime::new(paths, (640, 480));
    wait(&mut runtime, |r| !r.state.busy);
    runtime.state.hosts = vec!["127.0.0.1".into()];
    runtime.close();
    runtime
        .tx
        .send(Event::Done(Ok(Some(Reply::Paired))))
        .unwrap();
    runtime.poll();
    assert!(
        !runtime.state.busy,
        "closed screen must not start a list query"
    );
}

#[test]
fn successful_helper_also_reaps_background_descendants() {
    let (root, paths) =
        fixture("(sleep 0.3; touch \"$(dirname \"$0\")/leaked\") & printf '1. Desktop\\n'");
    assert_eq!(
        execute(
            &paths,
            "list",
            "127.0.0.1",
            Duration::from_secs(1),
            &AtomicBool::new(false),
            None
        )
        .unwrap(),
        Reply::Apps(vec!["Desktop".into()])
    );
    thread::sleep(Duration::from_millis(400));
    assert!(!root.path().join("usr/bin/leaked").exists());
}
