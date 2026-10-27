use std::{io, path::Path};

use crate::{
    file_io::{atomic_write, read_bounded},
    state::SessionAction,
};

#[derive(Debug)]
pub enum NextCmd {
    Game(String),
    Action(SessionAction),
}

/// Build the existing ES runemu contract; every argument is a POSIX shell word.
pub fn launch_command(
    rom: &Path,
    system: &str,
    core: &str,
    emulator: &str,
    controllers: &str,
) -> String {
    launch_command_with(
        Path::new("/usr/bin/runemu.sh"),
        rom,
        system,
        core,
        emulator,
        controllers,
    )
}

fn launch_command_with(
    bin: &Path,
    rom: &Path,
    system: &str,
    core: &str,
    emulator: &str,
    controllers: &str,
) -> String {
    let quote = |argument: &str| format!("'{}'", argument.replace('\'', "'\\''"));
    // Keep the fixed production executable literal; quote test executable paths.
    let bin = if bin == Path::new("/usr/bin/runemu.sh") {
        "/usr/bin/runemu.sh".to_owned()
    } else {
        quote(&bin.to_string_lossy())
    };
    format!(
        "{} {} {} {} {} {}",
        bin,
        quote(&rom.to_string_lossy()),
        quote(&format!("-P{system}")),
        quote(&format!("--core={core}")),
        quote(&format!("--emulator={emulator}")),
        quote(&format!("--controllers={controllers}")),
    )
}

/// Publish exactly one supervisor instruction, rejecting multiline or NUL data
/// before creating or replacing anything on disk.
pub fn write_next(run_dir: &Path, cmd: &NextCmd) -> io::Result<()> {
    let line = match cmd {
        NextCmd::Game(command) => {
            if command.contains(['\n', '\0']) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "game command contains newline or NUL",
                ));
            }
            format!("game {command}\n")
        }
        NextCmd::Action(action) => {
            let name = match action {
                SessionAction::Shutdown => "shutdown",
                SessionAction::Reboot => "reboot",
                SessionAction::Suspend => "suspend",
                SessionAction::OpenSettings => "settings",
            };
            format!("action {name}\n")
        }
    };
    atomic_write(run_dir, "next", line.as_bytes())
}

pub fn read_last_exit(run_dir: &Path) -> Option<i32> {
    read_bounded(&run_dir.join("last-exit"), 128)
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};

    #[test]
    fn launch_command_survives_quotes_spaces_dollar_parens() {
        let dir = tempfile::Builder::new()
            .prefix("launcher ' $ (test) ")
            .tempdir()
            .unwrap();
        let bin = dir.path().join("fake runemu.sh");
        fs::write(&bin, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o700)).unwrap();
        let rom = dir.path().join("rom dir/Tom's $Game (USA).zip");
        let command = launch_command_with(
            &bin,
            &rom,
            "snes",
            "core's $value (x)",
            "retro arch",
            "player 1 \"pad\"; $(echo injected) 'two'",
        );
        let result = Command::new("sh").arg("-c").arg(command).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = String::from_utf8(result.stdout).unwrap();
        assert_eq!(
            output.lines().collect::<Vec<_>>(),
            vec![
                rom.to_str().unwrap(),
                "-Psnes",
                "--core=core's $value (x)",
                "--emulator=retro arch",
                "--controllers=player 1 \"pad\"; $(echo injected) 'two'"
            ]
        );
    }
}
