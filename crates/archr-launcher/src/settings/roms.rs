//! ROMs on the PC: archr-roms-migrate's state, preview and actions, the names
//! exFAT refuses, and the notice once a move is over. The script owns the
//! move; it runs at boot, before the launcher (archr-roms-migrate.target).
use super::*;
use archr_ui::i18n::Message;
use archr_ui::settings::roms::{renames, Check, Scan, Status};

const SCRIPT: &str = "archr-roms-migrate";
/// The script's own marker (ext4) and log.
const MARK: &str = "/storage/.roms-migrate";
const LOG: &str = "/storage/.cache/roms-migrate.log";
/// The launcher's: a move it armed, and how many files it previewed.
const ARMED: &str = "roms-migrate";
/// The check walks every ROM.
const CHECK_TIME: Duration = Duration::from_secs(300);
/// Names listed at most, as the script lists them.
const LIST: usize = 500;

impl Backend {
    /// The script's stdout, also when it exits non-zero: check and start
    /// print why a move cannot run.
    fn migrate(&self, args: &[&str], timeout: Duration) -> Result<(bool, String), String> {
        let mut command = self.tool(SCRIPT);
        command.args(args);
        let output = crate::process::capture_with_input(
            &mut command,
            None,
            timeout,
            &AtomicBool::new(false),
            64 * 1024,
        )
        .map_err(|e| format!("{SCRIPT} {}: {e}", args.join(" ")))?;
        Ok((
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ))
    }
    /// Empty where the system has no such script (no ROMs partition then).
    pub(super) fn roms_status(&self) -> Status {
        if !self.paths.at(&format!("/usr/bin/{SCRIPT}")).is_file() {
            return Status::default();
        }
        self.migrate(&["status"], Duration::from_secs(20))
            .map(|(_, out)| Status::parse(&out))
            .unwrap_or_else(|error| {
                log::warn!("{error}");
                Status::default()
            })
    }
    /// The page's facts: the preview and the names to change on an old card,
    /// the names changed (from the log) on a moved one, and the battery.
    pub(super) fn roms_scan(&self) -> (Status, Scan) {
        let status = self.roms_status();
        // The script's own preview, with the renames it would make.
        let out = status
            .ext4()
            .then(|| self.migrate(&["check"], CHECK_TIME))
            .and_then(|result| result.map_err(|e| log::warn!("{e}")).ok())
            .map(|(_, out)| out);
        let (check, names) = match out {
            Some(out) => (Check::parse(&out), renames(&out)),
            None => (None, renamed(&self.read_log())),
        };
        let root = self.paths.root.clone().unwrap_or_else(|| "/".into());
        let battery = archr_ui::status::read_battery(&root).map(|b| (b.percent, b.charging));
        let scan = Scan {
            check,
            names,
            battery,
        };
        (status, scan)
    }
    fn read_log(&self) -> String {
        archr_io::read_bytes(&self.paths.at(LOG), 4 << 20, archr_io::Symlinks::Follow)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    }
    /// Arms, cancels or restarts the move: a message key or a reboot question.
    pub(super) fn run_roms(&self, action: &Action) -> Result<&'static str, String> {
        let armed = self.paths.cache.join(ARMED);
        let run = |args: &[&str], time| {
            self.migrate(args, time).map_err(|error| {
                log::warn!("{error}");
                String::new()
            })
        };
        match action {
            Action::RomsStart { files } => match run(&["start"], CHECK_TIME) {
                Ok((true, _)) => {
                    let marker = format!("FILES={files}\n");
                    if let Err(error) =
                        archr_io::atomic_publish(&armed, true, |f| f.write_all(marker.as_bytes()))
                    {
                        log::warn!("{}: {error}", armed.display());
                    }
                    Ok("roms.start_reboot_question")
                }
                Ok((false, out)) => Err(match Check::parse(&out).map(|c| c.result) {
                    Some(r) if r == "no-space" => "roms.no_space",
                    Some(r) if r == "low-power" => "roms.low_power",
                    _ => "roms.start_failed",
                }
                .into()),
                Err(_) => Err("roms.start_failed".into()),
            },
            Action::RomsCancel => match run(&["cancel"], Duration::from_secs(20)) {
                Ok((true, _)) => {
                    let _ = fs::remove_file(&armed);
                    Ok("roms.cancelled")
                }
                _ => Err("roms.cancel_failed".into()),
            },
            Action::RomsRetry => match run(&["retry"], Duration::from_secs(20)) {
                Ok((true, _)) => Ok("roms.retry_reboot_question"),
                _ => Err("roms.retry_failed".into()),
            },
            _ => unreachable!("storage actions run in storage.rs"),
        }
    }
}
use archr_ui::settings::storage::Action;
use std::io::Write;

/// The log of the last move: from its last "Armed:" line.
fn last_run(log: &str) -> &str {
    log.rfind(" Armed: ").map_or(log, |at| &log[at..])
}
/// "Renamed: snes/A:B.sfc -> A_B.sfc" lines of the last move.
fn renamed(log: &str) -> Vec<(String, String)> {
    last_run(log)
        .lines()
        .filter_map(|line| line.split_once(" Renamed: ")?.1.rsplit_once(" -> "))
        .take(LIST)
        .map(|(path, name)| (path.to_owned(), name.to_owned()))
        .collect()
}

/// Once a move the launcher armed is over, what to tell at startup: done
/// (files and renamed names, or folders kept back), given up, or stalled.
/// Read on the library's worker; the launcher's marker goes with the news.
pub fn migration_notice(paths: &Paths) -> Option<Message> {
    let armed = paths.cache.join(ARMED);
    let marker = archr_io::read_utf8(&armed, 4096, archr_io::Symlinks::Follow).ok()?;
    let script = archr_io::read_utf8(&paths.at(MARK), 64 * 1024, archr_io::Symlinks::Follow);
    let notice = match script {
        Ok(mark) if mark.lines().any(|l| l == "STATE=stalled") => {
            Message::new("error.roms_stalled")
        }
        // Armed and not restarted yet, or copying.
        Ok(_) => return None,
        Err(_) => {
            let log = archr_io::read_bytes(&paths.at(LOG), 4 << 20, archr_io::Symlinks::Follow)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default();
            notice(last_run(&log), &marker)
        }
    };
    let _ = fs::remove_file(&armed);
    Some(notice)
}
fn notice(run: &str, marker: &str) -> Message {
    let done = run
        .lines()
        .rev()
        .find_map(|line| line.split_once(" Done: ")?.1.split_once(" entries, "));
    let Some((_, kept)) = done else {
        return Message::new("error.roms_not_moved");
    };
    let kept = kept.split(' ').next().unwrap_or("0");
    if kept != "0" {
        return Message::new("roms.moved_left").with("kept", kept);
    }
    let files = marker
        .lines()
        .find_map(|l| l.strip_prefix("FILES="))
        .unwrap_or("0");
    Message::new("roms.moved")
        .with("files", files)
        .with("renamed", renamed(run).len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const LOG_DONE: &str = "\
2026-10-29 10:00:00 Armed: STORAGE 9000 MiB, ARCHR_ROMS 20000 MiB, ROMs 7000 MiB.
2026-10-29 10:00:05 Not migrating: no-space
2026-10-30 09:00:00 Armed: STORAGE 9000 MiB, ARCHR_ROMS 20000 MiB, ROMs 7000 MiB.
2026-10-30 09:00:10 Moving the ROMs to ARCHR_ROMS. Do not turn the device off.
2026-10-30 09:00:11 Renamed: snes/Zelda: Link.sfc -> Zelda_ Link.sfc
2026-10-30 09:00:11 Renamed: gba/MARIO.gba -> MARIO (2).gba
2026-10-30 09:00:12 [1/2] gba
2026-10-30 09:08:00 Done: 2 entries, 0 kept on STORAGE.
";

    fn fixture(script: &str) -> (tempfile::TempDir, Paths) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        for dir in ["/usr/bin", "/storage/.cache", "/storage/roms"] {
            fs::create_dir_all(paths.at(dir)).unwrap();
        }
        fs::create_dir_all(&paths.cache).unwrap();
        let file = paths.at(&format!("/usr/bin/{SCRIPT}"));
        fs::write(
            &file,
            format!(
                "#!/bin/sh\necho \"$*\" >> {}\n{script}\n",
                root.path().join("calls").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        (root, paths)
    }
    fn calls(root: &tempfile::TempDir) -> String {
        fs::read_to_string(root.path().join("calls")).unwrap_or_default()
    }

    #[test]
    fn an_old_card_gets_the_scripts_preview_and_renames() {
        let (_root, paths) = fixture(
            "case $1 in status) echo LAYOUT=ext4 ;; \
             check) printf 'LAYOUT=ext4\\nROMS_FILES=3\\nROMS_MIB=900\\nRENAMES=2\\nPOWER=low\\n\
             RENAME=snes/Zelda: Link.sfc\\tZelda_ Link.sfc\\nRENAME=gba/Pokémon.gba\\tPokémon (2).gba\\n\
             RESULT=low-power\\n'; exit 1 ;; esac",
        );
        let (status, scan) = Backend::new(paths).roms_scan();
        assert!(status.ext4());
        let check = scan.check.unwrap();
        assert_eq!(
            (check.files, check.power, check.result.as_str()),
            (3, false, "low-power")
        );
        assert_eq!(
            scan.names,
            [
                (
                    "snes/Zelda: Link.sfc".to_owned(),
                    "Zelda_ Link.sfc".to_owned()
                ),
                ("gba/Pokémon.gba".into(), "Pokémon (2).gba".into()),
            ]
        );
    }
    #[test]
    fn a_moved_card_lists_the_names_the_move_changed() {
        let (_root, paths) = fixture("echo LAYOUT=exfat; echo ROMS_FS=clean");
        fs::write(paths.at(LOG), LOG_DONE).unwrap();
        let (status, scan) = Backend::new(paths).roms_scan();
        assert!(status.exfat() && scan.check.is_none());
        assert_eq!(
            scan.names,
            [
                (
                    "snes/Zelda: Link.sfc".to_owned(),
                    "Zelda_ Link.sfc".to_owned()
                ),
                ("gba/MARIO.gba".into(), "MARIO (2).gba".into()),
            ]
        );
    }
    #[test]
    fn start_remembers_the_files_cancel_forgets_and_refusals_say_why() {
        let (root, paths) = fixture("");
        let backend = Backend::new(paths.clone());
        assert_eq!(
            backend.run_roms(&Action::RomsStart { files: 1507 }),
            Ok("roms.start_reboot_question")
        );
        let armed = paths.cache.join(ARMED);
        assert_eq!(fs::read_to_string(&armed).unwrap(), "FILES=1507\n");
        // Armed, not restarted yet: no news.
        fs::write(paths.at(MARK), "STATE=armed\n").unwrap();
        assert!(migration_notice(&paths).is_none() && armed.exists());
        assert_eq!(backend.run_roms(&Action::RomsCancel), Ok("roms.cancelled"));
        assert!(!armed.exists());
        assert_eq!(calls(&root), "start\ncancel\n");
        let (_root, paths) = fixture("echo RESULT=no-space; exit 1");
        assert_eq!(
            Backend::new(paths.clone()).run_roms(&Action::RomsStart { files: 1 }),
            Err("roms.no_space".into())
        );
        assert!(!paths.cache.join(ARMED).exists());
        let (_root, paths) = fixture("exit 1");
        assert_eq!(
            Backend::new(paths).run_roms(&Action::RomsRetry),
            Err("roms.retry_failed".into())
        );
    }
    #[test]
    fn the_notice_after_the_move_tells_once_how_it_went() {
        let (_root, paths) = fixture("");
        let armed = paths.cache.join(ARMED);
        let render = |m: Message| {
            let mut text = m.key.to_owned();
            for (k, v) in m.args {
                text.push_str(&format!("|{k}={v}"));
            }
            text
        };
        fs::write(&armed, "FILES=1507\n").unwrap();
        fs::write(paths.at(LOG), LOG_DONE).unwrap();
        assert_eq!(
            migration_notice(&paths).map(render).as_deref(),
            Some("roms.moved|files=1507|renamed=2")
        );
        assert!(migration_notice(&paths).is_none(), "once");
        fs::write(&armed, "FILES=9\n").unwrap();
        fs::write(paths.at(LOG), LOG_DONE.replace("0 kept", "2 kept")).unwrap();
        assert_eq!(
            migration_notice(&paths).map(render).as_deref(),
            Some("roms.moved_left|kept=2")
        );
        fs::write(&armed, "FILES=9\n").unwrap();
        fs::write(
            paths.at(LOG),
            &LOG_DONE[..LOG_DONE.find("2026-10-30 09:00:00").unwrap()],
        )
        .unwrap();
        assert_eq!(
            migration_notice(&paths).map(render).as_deref(),
            Some("error.roms_not_moved")
        );
        fs::write(&armed, "FILES=9\n").unwrap();
        fs::write(paths.at(MARK), "STATE=stalled\nTRIES=3\n").unwrap();
        assert_eq!(
            migration_notice(&paths).map(render).as_deref(),
            Some("error.roms_stalled")
        );
    }
}
