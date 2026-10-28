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

/// An address the lobby may legitimately hand out; also keeps arguments that
/// runemu.sh splits by substring (`--`, `-P`, ...) out of the join line.
pub fn is_ip(text: &str) -> bool {
    text.parse::<std::net::IpAddr>().is_ok()
}

/// RetroArch nickname rule, shared with the settings page that saves it.
pub(crate) const NICK_MAX: usize = 32;
pub(crate) fn nick_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || " _.".contains(c)
}
/// Lobby join: the runemu contract plus the ES netplay arguments
/// `--connect IP --port PORT --nick NICK`, `--spectate` to watch instead of play and,
/// for relay (MITM) rooms, `--mitm-session ID`.
///
/// Contract expected of runemu.sh/setsettings.sh: host and port of the session come
/// from `--connect`/`--port` (never from the saved `global.netplay.host`/`port`);
/// the mode is client, or spectator with `--spectate`; with `--mitm-session` the
/// address is the relay and the id selects the room on it; the value after `--nick`
/// is informational. runemu.sh parses its arguments by substring, so everything
/// from the network is restricted: the address must be an IP literal, the session
/// alphanumeric, and the nickname `[A-Za-z0-9 _.]` (at most 32 characters, no `-`).
/// Values are exported and only referenced, never spliced into the shell line.
pub fn netplay_command(
    bin: &Path,
    rom: &Path,
    game: (&str, &str, &str, &str),
    peer: (&str, u16, &str, &str),
    spectate: bool,
) -> io::Result<String> {
    let (system, core, emulator, controllers) = game;
    let (ip, port, nick, session) = peer;
    let session_ok = session.len() <= 64 && session.bytes().all(|b| b.is_ascii_alphanumeric());
    if !is_ip(ip) || port == 0 || !session_ok {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid netplay address",
        ));
    }
    let nick: String = nick
        .chars()
        .filter(|c| nick_char(*c))
        .take(NICK_MAX)
        .collect();
    let nick = if nick.trim().is_empty() {
        "ArchR Player".to_owned()
    } else {
        nick
    };
    let base = launch_command_with(bin, rom, system, core, emulator, controllers)?;
    let mut command = String::new();
    for (name, value) in [
        ("ARCHR_NP_IP", ip),
        ("ARCHR_NP_PORT", &port.to_string()),
        ("ARCHR_NP_NICK", &nick),
        ("ARCHR_NP_SESSION", session),
    ] {
        command.push_str(&assignment(name, value));
    }
    command.push_str("export ARCHR_NP_IP ARCHR_NP_PORT ARCHR_NP_NICK ARCHR_NP_SESSION; ");
    command.push_str(&base);
    command.push_str(
        " --connect \"$ARCHR_NP_IP\" --port \"$ARCHR_NP_PORT\" --nick \"$ARCHR_NP_NICK\"",
    );
    if !session.is_empty() {
        command.push_str(" --mitm-session \"$ARCHR_NP_SESSION\"");
    }
    if spectate {
        command.push_str(" --spectate");
    }
    Ok(command)
}

/// Expand a catalog `<command>` for a system without emulators.
///
/// File names never enter the template: the values are exported as
/// `ARCHR_ROM`, `ARCHR_BASENAME` and `ARCHR_SYSTEM`, and each placeholder
/// becomes a reference to its variable. Fail-closed grammar; a placeholder is
/// accepted only as:
/// - a whole unquoted word: `foot %ROM%` -> `foot "$ARCHR_ROM"`;
/// - a whole double-quoted word: `"%ROM%"` -> `"$ARCHR_ROM"`;
/// - `--command="%ROM%"` (weston-terminal runs it through another `sh -c`):
///   `--command='"$ARCHR_ROM"'`, expanded by that inner shell from the
///   inherited environment.
///
/// Templates with backslashes, backticks, `$(` or `((`, unknown placeholders,
/// a placeholder that follows `-c`, `eval` or `let` (its value would be run as
/// code) and any other placeholder context are refused.
pub fn catalog_command(template: &str, rom: &Path, system: &str) -> io::Result<String> {
    let refuse = |reason: &str| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{reason} in catalog command {template:?}"),
        )
    };
    let rom_text = rom.to_str().ok_or_else(|| refuse("non-UTF-8 ROM path"))?;
    if template.contains(['\\', '`']) || template.contains("$(") || template.contains("((") {
        return Err(refuse("shell escape or substitution"));
    }
    let basename = rom.file_stem().and_then(|stem| stem.to_str()).unwrap_or("");
    let mut command = String::new();
    for (name, value) in [
        ("ARCHR_ROM", rom_text),
        ("ARCHR_BASENAME", basename),
        ("ARCHR_SYSTEM", system),
    ] {
        command.push_str(&assignment(name, value));
    }
    command.push_str("export ARCHR_ROM ARCHR_BASENAME ARCHR_SYSTEM; ");
    let blank = |c: char| matches!(c, ' ' | '\t');
    let word_start = |text: &str| text.is_empty() || text.ends_with(blank);
    let word_end = |text: &str| text.is_empty() || text.starts_with(blank);
    // `text` ends where the placeholder's word starts.
    let evaluated = |text: &str| {
        let previous = text.trim_end_matches(blank).rsplit(blank).next();
        matches!(previous, Some("-c" | "eval" | "let"))
    };
    let (mut single, mut double) = (false, false);
    let mut index = 0;
    while let Some(c) = template[index..].chars().next() {
        let name = template[index..]
            .strip_prefix('%')
            .and_then(|tail| tail.find('%').map(|end| &tail[..end]))
            .filter(|name| {
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
            });
        let Some(name) = name else {
            match c {
                '\'' if !double => single = !single,
                '"' if !single => double = !double,
                _ => {}
            }
            command.push(c);
            index += c.len_utf8();
            continue;
        };
        if !matches!(name, "ROM" | "BASENAME" | "SYSTEM") {
            return Err(refuse(&format!("unsupported placeholder %{name}%")));
        }
        let reference = format!("\"$ARCHR_{name}\"");
        let (before, after) = (&template[..index], &template[index + name.len() + 2..]);
        let closed = after.strip_prefix('"').filter(|rest| word_end(rest));
        let word = before
            .strip_suffix("--command=\"")
            .or_else(|| before.strip_suffix('"'))
            .filter(|_| double)
            .unwrap_or(before);
        if evaluated(word) {
            return Err(refuse(&format!("placeholder %{name}% run as code")));
        }
        if !single && !double && word_start(before) && word_end(after) {
            command.push_str(&reference);
            index += name.len() + 2;
        } else if double && closed.is_some() {
            // The opening quote is already in the output; replace it.
            command.pop();
            if before.strip_suffix('"').is_some_and(word_start) {
                command.push_str(&reference);
            } else if before.strip_suffix("--command=\"").is_some_and(word_start) {
                command.push_str(&format!("'{reference}'"));
            } else {
                return Err(refuse(&format!(
                    "placeholder %{name}% inside a quoted word"
                )));
            }
            double = false;
            index += name.len() + 3;
        } else {
            return Err(refuse(&format!(
                "placeholder %{name}% inside a word or quotes"
            )));
        }
    }
    if single || double {
        return Err(refuse("unbalanced quotes"));
    }
    Ok(command)
}

/// One `NAME=value; ` assignment on a single line. `printf %b` restores
/// newlines (the handoff is one line); the `x` keeps trailing ones.
pub(crate) fn assignment(name: &str, value: &str) -> String {
    if value.contains('\n') {
        let escaped = value.replace('\\', "\\\\").replace('\n', "\\n");
        format!(
            "{name}=$(printf '%bx' {}); {name}=${{{name}%x}}; ",
            quote(&escaped)
        )
    } else {
        format!("{name}={}; ", quote(value))
    }
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

    const NAMES: [&str; 6] = [
        "plain.sh",
        "Tom's \"Mount\" NFS.sh",
        "cost $HOME and ${x}.sh",
        "tick `id` and $(id).sh",
        "back\\slash 100%ROM%.sh",
        "two\nlines\n.sh",
    ];

    /// Write `#!/bin/sh` scripts that print their $0 and arguments to `out`.
    fn tool(dir: &Path, name: &str, out: &Path) -> std::path::PathBuf {
        let path = dir.join(name);
        let body = format!(
            "#!/bin/sh\nprintf '%s\\0' \"$0\" \"$@\" >>{}\n",
            quote(out.to_str().unwrap())
        );
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn run(command: &str) {
        let result = Command::new("sh").arg("-c").arg(command).output().unwrap();
        assert!(
            result.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn catalog_command_passes_hostile_names_through_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let bin = tool(dir.path(), "args", &out);
        for name in NAMES {
            let rom = dir.path().join(name);
            let template = format!("{} %ROM% \"%BASENAME%\" -P %SYSTEM% 100%", bin.display());
            let command = catalog_command(&template, &rom, "tools").unwrap();
            assert!(!command.contains('\n'), "handoff must stay one line");
            let _ = fs::remove_file(&out);
            run(&command);
            let stem = rom.file_stem().unwrap().to_str().unwrap();
            let expected = [
                bin.to_str().unwrap(),
                rom.to_str().unwrap(),
                stem,
                "-P",
                "tools",
                "100%",
            ];
            assert_eq!(
                fs::read_to_string(&out).unwrap(),
                expected.map(|arg| format!("{arg}\0")).concat(),
                "{name:?}"
            );
        }
    }

    #[test]
    fn catalog_command_reaches_the_inner_shell_of_weston_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        // Like the patched weston-terminal: run --command with `$SHELL -c`.
        let weston = dir.path().join("weston-terminal");
        fs::write(
            &weston,
            "#!/bin/sh\nfor arg; do case $arg in --command=*) exec sh -c \"${arg#--command=}\";; esac; done\nexit 9\n",
        )
        .unwrap();
        fs::set_permissions(&weston, fs::Permissions::from_mode(0o700)).unwrap();
        for name in NAMES {
            let rom = tool(dir.path(), name, &out);
            let template = format!("{} --command=\"%ROM%\"", weston.display());
            let _ = fs::remove_file(&out);
            run(&catalog_command(&template, &rom, "tools").unwrap());
            assert_eq!(
                fs::read_to_string(&out).unwrap(),
                format!("{}\0", rom.display()),
                "{name:?}"
            );
        }
    }

    #[test]
    fn catalog_command_refuses_every_other_placeholder_context() {
        let rom = Path::new("/roms/ok.sh");
        for template in [
            "/usr/bin/runemu.sh %ROM% --core=%CORE%",
            "foot \\\"%ROM%\\\"",
            "foot \"\\\"%ROM%\\\"\"",
            "foot '%ROM%'",
            "foot `%ROM%`",
            "foot \"$(cat %ROM%)\"",
            "foot --file=%ROM%",
            "foot x%ROM%",
            "foot \"a %ROM%\"",
            "foot \"%ROM%\"x",
            "sh -c \"%ROM%; true\"",
            "weston-terminal x--command=\"%ROM%\"",
            "foot \"%ROM%",
            "sh -c %ROM%",
            "bash -c \"%ROM%\"",
            "eval %ROM%",
            "let %ROM%",
            "/bin/sh -c\t%ROM%",
            "foot \"%ROM%\" -c %SYSTEM%",
            "(( %ROM% ))",
            "foot\u{a0}%ROM%",
        ] {
            assert_eq!(
                catalog_command(template, rom, "tools").unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{template}"
            );
        }
        assert!(catalog_command("/usr/bin/true", rom, "imageviewer")
            .unwrap()
            .ends_with("; /usr/bin/true"));
    }
}
