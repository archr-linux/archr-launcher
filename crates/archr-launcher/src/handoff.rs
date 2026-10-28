use std::{
    io::{self, Write},
    path::Path,
};

use archr_io::{atomic_publish, read_utf8, Symlinks::Follow};

/// Build the runemu.sh launch contract; every argument is a POSIX shell word.
/// Returns `InvalidInput` if the ROM path is not valid UTF-8.
pub fn launch_command(
    rom: &Path,
    system: &str,
    core: &str,
    emulator: &str,
    controllers: &str,
) -> io::Result<String> {
    launch_command_with(
        Path::new("/usr/bin/runemu.sh"),
        rom,
        system,
        core,
        emulator,
        controllers,
    )
}

/// Build the same command with an explicit executable (root-isolated PC runs).
pub fn launch_command_with(
    bin: &Path,
    rom: &Path,
    system: &str,
    core: &str,
    emulator: &str,
    controllers: &str,
) -> io::Result<String> {
    let rom = rom.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "ROM path must be valid UTF-8")
    })?;
    let bin = bin.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable path must be valid UTF-8",
        )
    })?;
    // Keep the fixed production executable literal; quote test executable paths.
    let bin = if bin == "/usr/bin/runemu.sh" {
        bin.to_owned()
    } else {
        quote(bin)
    };
    Ok(format!(
        "{} {} {} {} {} {}",
        bin,
        quote(rom),
        quote(&format!("-P{system}")),
        quote(&format!("--core={core}")),
        quote(&format!("--emulator={emulator}")),
        quote(&format!("--controllers={controllers}")),
    ))
}

/// Expand a catalog `<command>` for a system without emulators, as ES did.
/// `%ROM%`, `%BASENAME%` and `%SYSTEM%` become single quoted shell words; any
/// other placeholder, or one inside quotes, is rejected rather than guessed.
pub fn catalog_command(template: &str, rom: &Path, system: &str) -> io::Result<String> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    let rom_text = rom
        .to_str()
        .ok_or_else(|| invalid("ROM path must be valid UTF-8".to_owned()))?;
    let basename = rom.file_stem().and_then(|stem| stem.to_str()).unwrap_or("");
    let mut command = String::with_capacity(template.len() + rom_text.len());
    let (mut single, mut double) = (false, false);
    let mut rest = template;
    while let Some(c) = rest.chars().next() {
        let placeholder = rest
            .strip_prefix('%')
            .and_then(|tail| tail.find('%').map(|end| &tail[..end]))
            .filter(|name| {
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
            });
        if let Some(name) = placeholder {
            let value = match name {
                "ROM" => rom_text,
                "BASENAME" => basename,
                "SYSTEM" => system,
                _ => return Err(invalid(format!("unsupported placeholder %{name}%"))),
            };
            if single || double {
                return Err(invalid(format!("quoted placeholder %{name}%")));
            }
            command.push_str(&quote(value));
            rest = &rest[name.len() + 2..];
            continue;
        }
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ => {}
        }
        command.push(c);
        rest = &rest[c.len_utf8()..];
    }
    Ok(command)
}

fn quote(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "'\\''"))
}

/// Publish exactly one supervisor instruction, rejecting multiline or NUL data
/// before creating or replacing anything on disk.
///
/// The supervisor must read the line with `IFS= read -r line` to preserve shell
/// escapes. For a game, strip the `game ` prefix and run `sh -c "$command"`.
pub fn write_next(run_dir: &Path, command: &str) -> io::Result<()> {
    if command.contains(['\n', '\0']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "game command contains newline or NUL",
        ));
    }
    atomic_publish(&run_dir.join("next"), false, |file| {
        writeln!(file, "game {command}")
    })
}

pub fn read_last_exit(run_dir: &Path) -> Option<i32> {
    read_utf8(&run_dir.join("last-exit"), 128, Follow)
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
    fn launch_command_rejects_non_utf8_binary_path() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let bin = Path::new(OsStr::from_bytes(b"/bin/runemu\xff.sh"));
        assert_eq!(
            launch_command_with(
                bin,
                Path::new("game.zip"),
                "snes",
                "snes9x",
                "retroarch",
                "pad"
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidInput
        );
    }

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
        )
        .unwrap();
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

    #[test]
    fn catalog_command_quotes_each_placeholder_as_one_word() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("args");
        fs::write(&bin, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o700)).unwrap();
        let rom = Path::new("/storage/.config/modules/Tom's $(x) \"Mount\" NFS.sh");
        let template = format!("{} %ROM% --base=%BASENAME% -P%SYSTEM% 100%", bin.display());
        let command = catalog_command(&template, rom, "tools").unwrap();
        let result = Command::new("sh").arg("-c").arg(command).output().unwrap();
        assert!(result.status.success());
        assert_eq!(
            String::from_utf8(result.stdout)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec![
                rom.to_str().unwrap(),
                "--base=Tom's $(x) \"Mount\" NFS",
                "-Ptools",
                "100%"
            ]
        );
        assert_eq!(
            catalog_command("/usr/bin/true", rom, "imageviewer").unwrap(),
            "/usr/bin/true"
        );
    }

    #[test]
    fn catalog_command_rejects_what_it_cannot_expand_safely() {
        let rom = Path::new("/roms/ok.sh");
        for template in [
            "/usr/bin/runemu.sh %ROM% --core=%CORE%",
            "foot \"%ROM%\"",
            "foot '%ROM%'",
        ] {
            assert_eq!(
                catalog_command(template, rom, "tools").unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{template}"
            );
        }
    }
}
