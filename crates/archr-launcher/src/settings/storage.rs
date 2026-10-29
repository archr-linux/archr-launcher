//! Storage page effects. The system scripts own every change: backuptool,
//! setrootpass, factoryreset and automount; the launcher only unmounts to eject.
use super::info::{bytes, filesystem_usage, percent};
use super::*;
use archr_ui::settings::storage::{root_password_valid, Action, Card, Storage};
use std::time::UNIX_EPOCH;

/// backuptool zips settings and saves; factoryreset deletes all of /storage.
const LONG: Duration = Duration::from_secs(15 * 60);
const EXTERNAL: &str = "/storage/games-external";
const ROMS: &str = "/storage/roms";
/// Where automount binds internal ROMs while unified storage overlays them.
const INTERNAL_BIND: &str = "/storage/.games-internal-bind/roms";
const BACKUPS: &str = "/storage/roms/backup";

/// The newest backup's time as a catalog reference: "today, 14:42" or a date.
fn when(then: &libc::tm, today: &libc::tm, clock: String) -> String {
    if (then.tm_year, then.tm_yday) == (today.tm_year, today.tm_yday) {
        format!("settings.backup_today|time={clock}")
    } else {
        format!(
            "settings.backup_date|day={:02}|month={:02}|year={}|time={clock}",
            then.tm_mday,
            then.tm_mon + 1,
            then.tm_year + 1900
        )
    }
}

impl Backend {
    /// (device, mount point, filesystem) of every mount, octal escapes decoded.
    fn mounts(&self) -> Vec<(String, String, String)> {
        let unescape = |field: &str| field.replace("\\040", " ").replace("\\134", "\\");
        self.read("/proc/mounts")
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let device = unescape(fields.next()?);
                let mount = unescape(fields.next()?);
                Some((device, mount, fields.next()?.to_owned()))
            })
            .collect()
    }
    /// The partition label udev published for a device, if any.
    fn label(&self, device: &str) -> Option<String> {
        let name = std::path::Path::new(device).file_name()?.to_owned();
        fs::read_dir(self.paths.at("/dev/disk/by-label"))
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                fs::read_link(entry.path()).is_ok_and(|target| target.file_name() == Some(&name))
            })
            .map(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .replace("\\x20", " ")
                    .replace('|', "")
            })
    }
    /// The card holding the games: a mounted second card, else /storage.
    fn card(&self) -> Option<Card> {
        let mounts = self.mounts();
        let external = mounts.iter().any(|(_, mount, _)| mount == EXTERNAL);
        let path = if external { EXTERNAL } else { "/storage" };
        let (device, _, fs) = mounts.iter().rev().find(|(_, mount, _)| mount == path)?;
        let (used, total) = filesystem_usage(&self.paths.at(path))?;
        let name = match (external, self.label(device)) {
            (true, Some(label)) => format!("settings.card_external|label={label}"),
            (true, None) => "settings.card_microsd".into(),
            (false, _) => "settings.card_internal".into(),
        };
        Some(Card {
            external,
            name,
            detail: format!(
                "settings.card_detail|fs={fs}|used={}|total={}",
                bytes(used),
                bytes(total)
            ),
            percent: percent(used, total).unwrap_or(0),
        })
    }
    /// backuptool's current file is `<OS>_BACKUP.zip`; older ones are ARCHIVED_*.
    fn latest_backup(&self, clock_12: bool) -> Option<String> {
        let newest = fs::read_dir(self.paths.at(BACKUPS))
            .ok()?
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.ends_with("_BACKUP.zip")
                    && !name.starts_with('.')
                    && !name.starts_with("ARCHIVED_")
            })
            .filter_map(|entry| entry.metadata().ok()?.modified().ok())
            .max()?;
        let unix = |time: std::time::SystemTime| {
            time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
        };
        let then = archr_ui::status::local_time(unix(newest))?;
        let today = archr_ui::status::local_time(unix(std::time::SystemTime::now()))?;
        let clock = archr_ui::status::local_clock(unix(newest), clock_12);
        Some(when(&then, &today, clock))
    }
    pub(super) fn storage(&self) -> Storage {
        let cfg = self.cfg().ok();
        let setting = |key| cfg.as_ref().and_then(|c| c.get_setting(key, None, None));
        Storage {
            card: self.card(),
            backup: self.latest_backup(setting(Id::ClockFormat.key()).as_deref() == Some("12")),
            password: setting("root.password").unwrap_or_else(|| "archr".into()),
            // automount's own verdict on the card it last inspected.
            merge_supported: !self.paths.at("/storage/.ms_unsupported").exists(),
            ..Storage::default()
        }
    }
    pub(super) fn load_storage(&self, cfg: &SystemCfg, state: &mut Settings) {
        for row in Page::Storage.rows() {
            if let Some(value) = cfg.get_setting(row.id.key(), None, None) {
                state.values.insert(row.id, value);
            }
        }
        state.storage = self.storage();
    }
    /// Runs one confirmed action: a message key (or reboot question) on
    /// success and, after an eject, every system to reload.
    pub(super) fn run_storage(
        &self,
        action: &Action,
    ) -> Result<(&'static str, Option<Vec<String>>), String> {
        let flag = |wifi: bool| if wifi { &["--with-wifi"][..] } else { &[][..] };
        let script = |args: &[&str], failure: &'static str| {
            self.command_with_timeout("backuptool", args, LONG)
                .map_err(|error| {
                    log::warn!("{error}");
                    failure.to_owned()
                })
        };
        match action {
            Action::Eject => self
                .eject()
                .map(|systems| ("settings.eject_done", Some(systems))),
            Action::Backup { wifi } => {
                script(
                    &[&["backup"], flag(*wifi)].concat(),
                    "settings.backup_failed",
                )?;
                Ok(("settings.backup_done", None))
            }
            Action::Restore { wifi } => {
                script(
                    &[&["restore"], flag(*wifi)].concat(),
                    "settings.restore_failed",
                )?;
                Ok(("settings.restore_reboot_question", None))
            }
            Action::RootPassword(password) => {
                self.root_password(password)?;
                Ok(("settings.password_done", None))
            }
            // The script reboots when it is done.
            Action::FactoryReset => self
                .command_with_timeout("factoryreset", &["ALL"], LONG)
                .map(|_| ("settings.resetting", None))
                .map_err(|error| {
                    log::warn!("{error}");
                    "settings.reset_failed".into()
                }),
        }
    }
    /// setrootpass reads the password on stdin, out of every process's argv.
    /// It never reaches a log: errors are scrubbed, and success is read back
    /// from system.cfg.
    fn root_password(&self, password: &str) -> Result<(), String> {
        if !root_password_valid(password) {
            return Err("settings.root_invalid".into());
        }
        let failed = |error: String| {
            log::warn!("{}", error.replace(password, "<hidden>"));
            "settings.password_failed".to_owned()
        };
        let line = format!("{password}\n");
        self.command_with_input(
            "setrootpass",
            &["-"],
            Some(line.as_bytes()),
            Duration::from_secs(60),
        )
        .map_err(failed)?;
        let stored = self
            .cfg()
            .map_err(failed)?
            .get_setting("root.password", None, None);
        if stored.as_deref() == Some(password) {
            Ok(())
        } else {
            Err(failed(
                "setrootpass: exited without saving root.password".into(),
            ))
        }
    }
    /// Unmounts what automount stacked on the card, top first. A card that
    /// stays busy halfway is remounted by automount, so ROMs never vanish.
    fn eject(&self) -> Result<Vec<String>, String> {
        let mounted = |path: &str| self.mounts().iter().any(|(_, mount, _)| mount == path);
        if !mounted(EXTERNAL) {
            return Err("settings.eject_none".into());
        }
        let busy = |error: String| {
            log::warn!("Eject: {error}");
            "settings.eject_busy".to_owned()
        };
        self.command("sync", &[]).map_err(busy)?;
        if mounted(ROMS) {
            self.command("umount", &[ROMS]).map_err(busy)?;
        }
        if mounted(INTERNAL_BIND) {
            let _ = self.command("umount", &[INTERNAL_BIND]);
        }
        if let Err(error) = self.command("umount", &[EXTERNAL]) {
            if let Err(again) = self.command_with_timeout("automount", &[], LONG) {
                log::warn!("Eject: remount failed: {again}");
            }
            return Err(busy(error));
        }
        Ok(crate::worker::definitions(&self.paths)
            .unwrap_or_default()
            .into_iter()
            .map(|system| system.name)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    /// A root with recording scripts and a given /proc/mounts.
    fn fixture(config: &str, mounts: &str) -> (tempfile::TempDir, Paths) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        for dir in ["/usr/bin", "/proc", "/storage/games-external"] {
            fs::create_dir_all(paths.at(dir)).unwrap();
        }
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(&paths.config, config).unwrap();
        fs::write(paths.at("/proc/mounts"), mounts).unwrap();
        (root, paths)
    }
    fn script(root: &tempfile::TempDir, paths: &Paths, name: &str, body: &str) {
        let file = paths.at(&format!("/usr/bin/{name}"));
        fs::write(
            &file,
            format!(
                "#!/bin/sh\necho \"{name} $*\" >> {}\n{body}\n",
                root.path().join("calls").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn calls(root: &tempfile::TempDir) -> String {
        fs::read_to_string(root.path().join("calls")).unwrap_or_default()
    }
    const CARD: &str = "/dev/mmcblk0p3 /storage ext4 rw 0 0\n\
        /dev/mmcblk1p1 /storage/games-external ext4 rw 0 0\n\
        /dev/mmcblk1p1 /storage/roms ext4 rw 0 0\n";
    #[test]
    fn eject_unmounts_roms_then_the_card_and_reloads_every_system() {
        let (root, paths) = fixture("", CARD);
        script(&root, &paths, "sync", "");
        script(&root, &paths, "umount", "");
        let backend = Backend::new(paths);
        assert_eq!(
            backend.run_storage(&Action::Eject).map(|r| r.0),
            Ok("settings.eject_done")
        );
        assert_eq!(
            calls(&root),
            "sync \numount /storage/roms\numount /storage/games-external\n"
        );
    }
    #[test]
    fn a_card_still_busy_halfway_is_remounted_by_automount() {
        let (root, paths) = fixture("", CARD);
        script(&root, &paths, "sync", "");
        script(&root, &paths, "automount", "");
        script(
            &root,
            &paths,
            "umount",
            "[ \"$1\" = /storage/games-external ] && exit 32\nexit 0",
        );
        let backend = Backend::new(paths.clone());
        assert_eq!(
            backend.run_storage(&Action::Eject),
            Err("settings.eject_busy".into())
        );
        assert!(calls(&root).ends_with("umount /storage/games-external\nautomount \n"));
        // Without a second card there is nothing to eject.
        fs::write(
            paths.at("/proc/mounts"),
            "/dev/mmcblk0p3 /storage ext4 rw 0 0\n",
        )
        .unwrap();
        assert_eq!(
            backend.run_storage(&Action::Eject),
            Err("settings.eject_none".into())
        );
    }
    #[test]
    fn backup_passes_with_wifi_only_when_chosen() {
        let (root, paths) = fixture("", CARD);
        script(&root, &paths, "backuptool", "");
        let backend = Backend::new(paths);
        backend
            .run_storage(&Action::Backup { wifi: false })
            .unwrap();
        backend.run_storage(&Action::Backup { wifi: true }).unwrap();
        assert_eq!(
            backend.run_storage(&Action::Restore { wifi: false }),
            Ok(("settings.restore_reboot_question", None))
        );
        assert_eq!(
            calls(&root),
            "backuptool backup\nbackuptool backup --with-wifi\nbackuptool restore\n"
        );
    }
    #[test]
    fn root_password_counts_only_once_system_cfg_holds_it() {
        const SECRET: &str = "S3cret-pass";
        let (root, paths) = fixture("root.password=archr\n", CARD);
        // A failing script that echoes its argument to stderr.
        let file = paths.at("/usr/bin/setrootpass");
        fs::write(&file, "#!/bin/sh\necho \"bad $1\" >&2\nexit 1\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        let backend = Backend::new(paths.clone());
        let error = backend
            .run_storage(&Action::RootPassword(SECRET.into()))
            .unwrap_err();
        assert_eq!(error, "settings.password_failed");
        // A script that exits 0 without saving is not a success either.
        script(&root, &paths, "setrootpass", "");
        assert_eq!(
            backend.run_storage(&Action::RootPassword(SECRET.into())),
            Err("settings.password_failed".into())
        );
        let save = format!(
            "[ \"$1\" = - ] && read -r pw && printf 'root.password=%s\\n' \"$pw\" > {}",
            paths.config.display()
        );
        script(&root, &paths, "setrootpass", &save);
        assert_eq!(
            backend.run_storage(&Action::RootPassword(SECRET.into())),
            Ok(("settings.password_done", None))
        );
        assert_eq!(backend.storage().password, SECRET);
        assert_eq!(
            backend.run_storage(&Action::RootPassword("a b".into())),
            Err("settings.root_invalid".into())
        );
    }
    #[test]
    fn card_reports_the_external_label_usage_and_newest_backup() {
        let (_root, paths) = fixture("", CARD);
        fs::create_dir_all(paths.at("/dev/disk/by-label")).unwrap();
        std::os::unix::fs::symlink("../../mmcblk1p1", paths.at("/dev/disk/by-label/ROMS\\x20A"))
            .unwrap();
        fs::create_dir_all(paths.at(BACKUPS)).unwrap();
        fs::write(
            paths.at(&format!("{BACKUPS}/ARCHIVED_ArchR_BACKUP-1.zip")),
            "",
        )
        .unwrap();
        let backend = Backend::new(paths.clone());
        assert_eq!(backend.storage().backup, None);
        fs::write(paths.at(&format!("{BACKUPS}/ArchR_BACKUP.zip")), "").unwrap();
        let storage = backend.storage();
        let card = storage.card.unwrap();
        assert!(card.external);
        assert_eq!(card.name, "settings.card_external|label=ROMS A");
        assert!(card
            .detail
            .starts_with("settings.card_detail|fs=ext4|used=@"));
        assert!(storage
            .backup
            .is_some_and(|b| b.starts_with("settings.backup_today|time=")));
        assert!(storage.password == "archr" && storage.merge_supported);
    }
    #[test]
    fn older_backups_show_their_date() {
        let mut then: libc::tm = unsafe { std::mem::zeroed() };
        let mut today = then;
        (then.tm_year, then.tm_yday, then.tm_mon, then.tm_mday) = (126, 290, 9, 18);
        (today.tm_year, today.tm_yday) = (126, 300);
        assert_eq!(
            when(&then, &today, "09:05".into()),
            "settings.backup_date|day=18|month=10|year=2026|time=09:05"
        );
    }
}
