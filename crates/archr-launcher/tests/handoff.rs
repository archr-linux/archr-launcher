use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use archr_launcher::{
    handoff::{launch_command, read_last_exit, write_next, NextCmd},
    state::SessionAction,
};
use tempfile::tempdir;

#[test]
fn launch_command_quotes_every_argument_and_preserves_es_flags() {
    assert_eq!(
        launch_command(Path::new("/rom dir/Tom's $Game (USA).zip"), "snes", "snes9x", "retroarch", "player 1 \"pad\""),
        "/usr/bin/runemu.sh '/rom dir/Tom'\\''s $Game (USA).zip' '-Psnes' '--core=snes9x' '--emulator=retroarch' '--controllers=player 1 \"pad\"'"
    );
    assert_eq!(
        launch_command(Path::new(""), "", "", "", ""),
        "/usr/bin/runemu.sh '' '-P' '--core=' '--emulator=' '--controllers='"
    );
}

#[test]
fn next_file_format_game_and_action() {
    let root = tempdir().unwrap();
    let dir = root.path().join("run/launcher");
    write_next(&dir, &NextCmd::Game("/usr/bin/runemu.sh 'game.zip'".into())).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("next")).unwrap(),
        "game /usr/bin/runemu.sh 'game.zip'\n"
    );
    for (action, line) in [
        (SessionAction::Shutdown, "action shutdown\n"),
        (SessionAction::Reboot, "action reboot\n"),
        (SessionAction::Suspend, "action suspend\n"),
        (SessionAction::OpenSettings, "action settings\n"),
    ] {
        write_next(&dir, &NextCmd::Action(action)).unwrap();
        assert_eq!(fs::read_to_string(dir.join("next")).unwrap(), line);
    }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn next_rejects_newline_or_nul_in_every_game_argument() {
    let root = tempdir().unwrap();
    let dir = root.path().join("not-created");
    for bad in ["bad\nvalue", "bad\0value"] {
        for index in 0..5 {
            let mut args = ["game.zip", "snes", "snes9x", "retroarch", "pad"];
            args[index] = bad;
            let command = launch_command(Path::new(args[0]), args[1], args[2], args[3], args[4]);
            assert_eq!(
                write_next(&dir, &NextCmd::Game(command))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(!dir.exists());
        }
    }
    write_next(&dir, &NextCmd::Action(SessionAction::Reboot)).unwrap();
    for bad in ["game\naction shutdown", "game\0ignored"] {
        assert_eq!(
            write_next(&dir, &NextCmd::Game(bad.into()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            fs::read_to_string(dir.join("next")).unwrap(),
            "action reboot\n"
        );
    }
}

#[test]
fn next_replaces_instead_of_truncating_and_cleans_up_on_failure() {
    let dir = tempdir().unwrap();
    write_next(dir.path(), &NextCmd::Action(SessionAction::Reboot)).unwrap();
    let mut old = fs::File::open(dir.path().join("next")).unwrap();
    write_next(dir.path(), &NextCmd::Action(SessionAction::Suspend)).unwrap();
    let mut text = String::new();
    old.read_to_string(&mut text).unwrap();
    assert_eq!(text, "action reboot\n");
    assert_eq!(
        fs::read_to_string(dir.path().join("next")).unwrap(),
        "action suspend\n"
    );
    fs::remove_file(dir.path().join("next")).unwrap();
    fs::create_dir(dir.path().join("next")).unwrap();
    assert!(write_next(dir.path(), &NextCmd::Action(SessionAction::Shutdown)).is_err());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn last_exit_parses_trimmed_i32_and_ignores_other_input() {
    let dir = tempdir().unwrap();
    assert_eq!(read_last_exit(dir.path()), None);
    for (source, expected) in [
        ("0\n", Some(0)),
        ("  -12\n\t", Some(-12)),
        ("2147483647", Some(i32::MAX)),
        ("-2147483648", Some(i32::MIN)),
        ("2147483648", None),
        ("", None),
        ("1\n2", None),
        ("error", None),
        ("7 trailing", None),
    ] {
        fs::write(dir.path().join("last-exit"), source).unwrap();
        assert_eq!(read_last_exit(dir.path()), expected, "{source:?}");
    }
    for source in [vec![0xff], [b"0".as_slice(), &vec![b' '; 4096]].concat()] {
        fs::write(dir.path().join("last-exit"), source).unwrap();
        assert_eq!(read_last_exit(dir.path()), None);
    }
    fs::remove_file(dir.path().join("last-exit")).unwrap();
    fs::create_dir(dir.path().join("last-exit")).unwrap();
    assert_eq!(read_last_exit(dir.path()), None);
}

#[cfg(unix)]
#[test]
fn last_exit_fifo_is_rejected_without_opening() {
    let dir = tempdir().unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(dir.path().join("last-exit"))
        .status()
        .unwrap()
        .success());
    assert_eq!(read_last_exit(dir.path()), None);
}
