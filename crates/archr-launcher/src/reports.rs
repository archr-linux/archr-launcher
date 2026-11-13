//! Error reports (archr-error-report in the archr package): what the
//! settings page reads from the reporter's folders, the notice said once,
//! and the report of a module that crashed.
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_ui::settings::reports::{Facts, Last};
use std::{fs, path::Path};

const DIR: &str = "/storage/.cache/error-reports";
/// Written once the notice was answered (launcher store).
const NOTICE: &str = "error-reports-notice";
/// archr-error-report's own cap, with room.
const LIMIT: u64 = 300 * 1024;

/// The notice was never answered on this card, and the system has the
/// reporter (a launcher newer than its system says nothing).
pub fn notice_due(paths: &Paths) -> bool {
    !paths.store.join(NOTICE).exists() && paths.at("/usr/bin/archr-error-report").exists()
}
/// The notice's answer, off the draw thread: remembered, and reports
/// turned off when asked.
pub fn answer(paths: Paths, on: bool) {
    std::thread::spawn(move || {
        if !on {
            if let Err(error) =
                SystemCfg::set_and_save(&paths.config, &paths.lock, "system.error_reports", "0")
            {
                log::warn!("system.error_reports: {error}");
            }
            purge(&paths);
        }
        let _ = fs::create_dir_all(&paths.store);
        if let Err(error) = fs::write(paths.store.join(NOTICE), "1\n") {
            log::warn!("{NOTICE}: {error}");
        }
    });
}
/// Reports turned off: what waits is never sent.
pub fn purge(paths: &Paths) {
    for (path, _) in reports(&paths.at(DIR).join("queue")) {
        let _ = fs::remove_file(path);
    }
}
/// `<date>_<time>.txt` files of a folder, with their change time.
fn reports(dir: &Path) -> Vec<(std::path::PathBuf, std::time::SystemTime)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.len() == 23 && name.ends_with(".txt") && name.as_bytes()[10] == b'_'
        })
        .filter_map(|entry| {
            let meta = entry.metadata().ok().filter(fs::Metadata::is_file)?;
            Some((entry.path(), meta.modified().ok()?))
        })
        .collect()
}
/// The queue, today's count and the newest report (queued or sent).
pub fn facts(paths: &Paths) -> Facts {
    let dir = paths.at(DIR);
    let queue = reports(&dir.join("queue"));
    let today = today();
    let sent_today = fs::read_to_string(dir.join("today"))
        .ok()
        .and_then(|line| {
            let mut fields = line.split_whitespace();
            (fields.next()? == today).then_some(())?;
            fields.nth(1)?.parse().ok()
        })
        .unwrap_or(0);
    let newest = |list: Vec<_>, sent| {
        list.into_iter()
            .max_by(|(a, _): &(std::path::PathBuf, _), (b, _)| a.file_name().cmp(&b.file_name()))
            .map(|(path, at)| (path, at, sent))
    };
    let last = [
        newest(queue.clone(), false),
        newest(reports(&dir.join("sent")), true),
    ]
    .into_iter()
    .flatten()
    .max_by(|(a, ..), (b, ..)| a.file_name().cmp(&b.file_name()))
    .and_then(|(path, at, sent)| {
        let bytes = archr_io::read_bytes(&path, LIMIT, archr_io::Symlinks::Reject).ok()?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        Some(Last::new(
            name,
            sent,
            clock(at),
            &String::from_utf8_lossy(&bytes),
        ))
    });
    Facts {
        queued: queue.len(),
        sent_today,
        last,
        console_id: None,
    }
}
/// Today's date as archr-error-report writes it (`date +%F`, local time).
fn today() -> String {
    local(std::time::SystemTime::now(), "%F")
}
/// `HH:MM` of a moment, local time.
fn clock(at: std::time::SystemTime) -> String {
    local(at, "%H:%M")
}
fn local(at: std::time::SystemTime, format: &str) -> String {
    let seconds = at
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut out = [0u8; 32];
    let format = std::ffi::CString::new(format).expect("no NUL");
    // SAFETY: valid pointers; strftime writes at most out.len() bytes.
    let written = unsafe {
        libc::localtime_r(&seconds, &mut tm);
        libc::strftime(out.as_mut_ptr().cast(), out.len(), format.as_ptr(), &tm)
    };
    String::from_utf8_lossy(&out[..written]).into_owned()
}
/// A module process exited with `code` (128 + signal): archr-error-report
/// decides whether it is reported, in the background.
pub fn module_crashed(bin: &Path, id: &str, code: i32) {
    let mut command = std::process::Command::new(bin.join("archr-error-report"));
    command
        .args(["module", id, &code.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(error) => log::debug!("archr-error-report: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_come_from_the_queue_the_sent_folder_and_today() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        let reports = paths.at(DIR);
        fs::create_dir_all(reports.join("queue")).unwrap();
        fs::create_dir_all(reports.join("sent")).unwrap();
        assert_eq!(facts(&paths), Facts::default());
        fs::write(
            reports.join("sent/2026-11-10_12-31-05.txt"),
            "event: game\nwhat: x\n",
        )
        .unwrap();
        fs::write(reports.join("sent/2026-11-09_08-00-00.txt"), "old").unwrap();
        fs::write(reports.join("queue/notes.txt"), "not a report").unwrap();
        fs::write(reports.join("today"), format!("{} 3 2\n", today())).unwrap();
        let read = facts(&paths);
        assert_eq!((read.queued, read.sent_today), (0, 2));
        let last = read.last.unwrap();
        assert_eq!(
            (last.name.as_str(), last.sent),
            ("2026-11-10_12-31-05.txt", true)
        );
        assert_eq!(last.title(), "reports.failed_game|what=x");
        fs::write(
            reports.join("queue/2026-11-10_13-00-00.txt"),
            "event: kernel\n",
        )
        .unwrap();
        fs::write(reports.join("today"), "2000-01-01 3 2\n").unwrap();
        let read = facts(&paths);
        assert_eq!((read.queued, read.sent_today), (1, 0), "another day");
        let last = read.last.unwrap();
        assert_eq!(
            (last.name.as_str(), last.sent),
            ("2026-11-10_13-00-00.txt", false)
        );
        purge(&paths);
        assert_eq!(facts(&paths).queued, 0);
        assert!(reports.join("queue/notes.txt").exists(), "only reports go");
    }
    #[test]
    fn the_notice_is_due_until_answered_and_off_turns_reports_off() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, "system.error_reports=1\n").unwrap();
        assert!(!notice_due(&paths), "no reporter on this system");
        fs::create_dir_all(paths.at("/usr/bin")).unwrap();
        fs::write(paths.at("/usr/bin/archr-error-report"), "").unwrap();
        fs::create_dir_all(paths.at(DIR).join("queue")).unwrap();
        fs::write(paths.at(DIR).join("queue/2026-11-10_13-00-00.txt"), "x").unwrap();
        assert!(notice_due(&paths));
        answer(paths.clone(), false);
        for _ in 0..200 {
            if !notice_due(&paths) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!notice_due(&paths));
        let cfg = SystemCfg::load(&paths.config).unwrap();
        assert_eq!(
            cfg.get_setting("system.error_reports", None, None)
                .as_deref(),
            Some("0")
        );
        assert_eq!(facts(&paths).queued, 0);
    }
}
