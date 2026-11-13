//! Diagnostics and BIOS: the slow scripts behind both pages, on their own
//! lane so the settings worker never waits for a report or a checksum.
//! archr-debug owns system.loglevel, archr-report writes and sends the zip,
//! archr-console-id names the console, archr-systems checks the BIOS and archr-bios-livres installs a free one.
use super::*;
use archr_ui::settings::{bios, diag, reports};

/// The folder archr-report writes to, and its own time limit with room.
const REPORTS: &str = "/storage/roms/relatorios";
const REPORT_TIME: Duration = Duration::from_secs(180);
/// A few MB over a slow Wi-Fi.
const SEND_TIME: Duration = Duration::from_secs(300);
/// Every BIOS file is read for its MD5; a big Neo Geo zip takes seconds.
const BIOS_TIME: Duration = Duration::from_secs(120);
/// pacman may download the package first.
const INSTALL_TIME: Duration = Duration::from_secs(300);

pub(super) enum Slow {
    Diag,
    Report,
    /// Send this zip of REPORTS, by name.
    Send(String),
    Bios,
    BiosInstall(String),
}

impl Backend {
    pub(super) fn slow(&self, work: Slow) -> Reply {
        match work {
            Slow::Diag => Reply::Diag(Box::new(self.diag_facts())),
            Slow::Report => Reply::Report(self.report()),
            Slow::Send(name) => Reply::ReportSent(self.send_report(&name)),
            Slow::Bios => Reply::Bios(
                self.command_with_timeout("archr-systems", &["--json"], BIOS_TIME)
                    .and_then(|json| bios::parse(&json)),
            ),
            Slow::BiosInstall(free) => Reply::BiosInstall(
                self.command_with_timeout("archr-bios-livres", &["install", &free], INSTALL_TIME)
                    .map(drop),
            ),
        }
    }
    /// `archr-debug status` and the newest report.
    fn diag_facts(&self) -> diag::Facts {
        let status = self
            .command("archr-debug", &["status"])
            .map_err(|e| log::warn!("{e}"))
            .unwrap_or_default();
        diag::Facts::parse(&status, self.last_report())
    }
    fn last_report(&self) -> Option<diag::Report> {
        fs::read_dir(self.paths.at(REPORTS))
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let meta = entry.metadata().ok()?;
                (name.starts_with("archr-relatorio-") && name.ends_with(".zip") && meta.is_file())
                    .then(|| diag::Report {
                        name,
                        bytes: meta.len(),
                        at: meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                    })
            })
            .max_by(|a, b| a.at.cmp(&b.at).then_with(|| a.name.cmp(&b.name)))
    }
    fn report(&self) -> Result<diag::Report, String> {
        let out = self.command_with_timeout("archr-report", &[], REPORT_TIME)?;
        let path = out
            .lines()
            .find_map(|line| line.strip_prefix("REPORT="))
            .ok_or("archr-report: no report written")?;
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let meta = fs::metadata(self.paths.at(path)).map_err(|e| format!("{path}: {e}"))?;
        Ok(diag::Report {
            name,
            bytes: meta.len(),
            at: meta
                .modified()
                .unwrap_or_else(|_| std::time::SystemTime::now()),
        })
    }
    /// `archr-report send <zip>`: its exit code says what happened
    /// (diag::sent_message). Only a report of its own folder goes.
    fn send_report(&self, name: &str) -> Result<i32, String> {
        if !name.starts_with("archr-relatorio-") || !name.ends_with(".zip") || name.contains('/') {
            return Err(format!("not a report: {name}"));
        }
        let zip = self.paths.at(REPORTS).join(name);
        let mut command = self.tool("archr-report");
        command.arg("send").arg(&zip);
        let output = crate::process::capture_with_input(
            &mut command,
            None,
            SEND_TIME,
            &AtomicBool::new(false),
            64 * 1024,
        )
        .map_err(|e| format!("archr-report send: {e}"))?;
        if !output.status.success() {
            log::warn!(
                "archr-report send: {}; {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        output
            .status
            .code()
            .ok_or_else(|| format!("archr-report send: {}", output.status))
    }
    /// This console's id, None without archr-console-id or a valid answer.
    pub(super) fn console_id(&self) -> Option<String> {
        self.command("archr-console-id", &[])
            .map_err(|e| log::debug!("{e}"))
            .ok()
            .and_then(|out| reports::console_id(&out))
    }
    /// The debug switch: archr-debug writes the key; the reply is what it saved.
    pub(super) fn apply_debug(&self, value: &str) -> Result<String, String> {
        let argument = match value {
            "1" => "on",
            "0" => "off",
            _ => return Err("Invalid setting value".into()),
        };
        self.command("archr-debug", &[argument])?;
        Ok(debug_value(self.cfg()?.get_setting(
            "system.loglevel",
            None,
            None,
        )))
        .and_then(|saved| {
            if saved == value {
                Ok(saved)
            } else {
                Err(format!("archr-debug {argument}: system.loglevel not saved"))
            }
        })
    }
}
/// "1" while system.loglevel is verbose.
pub(super) fn debug_value(loglevel: Option<String>) -> String {
    if loglevel.as_deref() == Some("verbose") {
        "1"
    } else {
        "0"
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (tempfile::TempDir, Backend) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("usr/bin")).unwrap();
        fs::create_dir_all(root.join("storage/.config/system/configs")).unwrap();
        fs::write(
            root.join("storage/.config/system/configs/system.cfg"),
            "system.loglevel=none\n",
        )
        .unwrap();
        let backend = Backend::new(Paths::new(Some(root.into())));
        (dir, backend)
    }
    fn script(dir: &tempfile::TempDir, name: &str, body: &str) {
        let path = dir.path().join("usr/bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn the_switch_runs_archr_debug_and_reads_back_the_key() {
        let (dir, backend) = fixture();
        let cfg = "$ARCHR_LAUNCHER_ROOT/storage/.config/system/configs/system.cfg";
        script(
            &dir,
            "archr-debug",
            &format!("[ \"$1\" = on ] && echo system.loglevel=verbose >\"{cfg}\"; exit 0"),
        );
        assert_eq!(backend.apply_debug("1"), Ok("1".into()));
        assert!(
            backend.apply_debug("0").is_err(),
            "the script did not save it"
        );
        assert!(backend.apply_debug("yes").is_err());
        assert_eq!(debug_value(None), "0");
    }
    #[test]
    fn facts_and_reports_come_from_the_scripts_and_the_folder() {
        let (dir, backend) = fixture();
        script(&dir, "archr-debug", "echo PSTORE=ramoops; echo CRASHES=1");
        let folder = dir.path().join("storage/roms/relatorios");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("archr-relatorio-20261030-2236.zip"), [0; 10]).unwrap();
        fs::write(folder.join("notes.txt"), "x").unwrap();
        let facts = backend.diag_facts();
        assert_eq!(
            (facts.backend.as_deref(), facts.crashes),
            (Some("ramoops"), 1)
        );
        assert_eq!(
            facts.last.unwrap().name,
            "archr-relatorio-20261030-2236.zip"
        );
        let zip = folder.join("archr-relatorio-20261031-0102.zip");
        script(
            &dir,
            "archr-report",
            &format!("printf 1234 >'{}'; echo REPORT=/storage/roms/relatorios/archr-relatorio-20261031-0102.zip; echo SIZE=4", zip.display()),
        );
        let report = backend.report().unwrap();
        assert_eq!(
            (report.name.as_str(), report.bytes),
            ("archr-relatorio-20261031-0102.zip", 4)
        );
        script(&dir, "archr-report", "exit 1");
        assert!(backend.report().is_err());
    }
    #[test]
    fn a_send_reports_archr_reports_exit_code() {
        let (dir, backend) = fixture();
        let zip = "archr-relatorio-20261113-0915.zip";
        // The script sees the zip's full path and answers by its exit code.
        script(
            &dir,
            "archr-report",
            &format!(
                "[ \"$1\" = send ] && [ \"$2\" = \"$ARCHR_LAUNCHER_ROOT{REPORTS}/{zip}\" ] || exit 9\n\
                 read code <\"$ARCHR_LAUNCHER_ROOT/code\"; exit $code"
            ),
        );
        for code in [0, 2, 3, 4, 5] {
            fs::write(dir.path().join("code"), code.to_string()).unwrap();
            assert!(
                matches!(backend.slow(Slow::Send(zip.into())), Reply::ReportSent(Ok(c)) if c == code),
                "exit {code}"
            );
        }
        assert!(backend.send_report("../etc/passwd.zip").is_err());
        assert!(backend.send_report("photo-1.zip").is_err());
    }
    #[test]
    fn the_console_id_is_read_from_archr_console_id() {
        let (dir, backend) = fixture();
        assert_eq!(backend.console_id(), None, "no command, no row");
        script(&dir, "archr-console-id", "echo 3f9a0c2b7d14");
        assert_eq!(backend.console_id().as_deref(), Some("3f9a0c2b7d14"));
        script(&dir, "archr-console-id", "echo nope");
        assert_eq!(backend.console_id(), None);
        script(&dir, "archr-console-id", "echo 3f9a0c2b7d14; exit 1");
        assert_eq!(backend.console_id(), None, "a failure is no id");
    }
    #[test]
    fn bios_check_and_install_go_through_the_scripts() {
        let (dir, backend) = fixture();
        script(
            &dir,
            "archr-systems",
            r#"echo '{"systems":[{"id":"psx","name":"PlayStation","cores":"x","verdict":"builtin","free":"psx","free_installed":false,"files":[]}]}'"#,
        );
        let Reply::Bios(Ok(systems)) = backend.slow(Slow::Bios) else {
            panic!("a list");
        };
        assert_eq!(systems[0].name, "PlayStation");
        script(&dir, "archr-bios-livres", "[ \"$1 $2\" = \"install psx\" ]");
        assert!(matches!(
            backend.slow(Slow::BiosInstall("psx".into())),
            Reply::BiosInstall(Ok(()))
        ));
        assert!(matches!(
            backend.slow(Slow::BiosInstall("msx".into())),
            Reply::BiosInstall(Err(_))
        ));
    }
}
