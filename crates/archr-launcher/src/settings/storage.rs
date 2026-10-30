//! Storage page effects. The system scripts own every change: backuptool,
//! setrootpass, factoryreset and automount (which also ejects the card).
use super::info::{bytes, filesystem_usage, percent};
use super::*;
use archr_ui::settings::storage::{root_password_valid, Action, Card, Storage};
use std::time::UNIX_EPOCH;

/// backuptool zips settings and saves; factoryreset deletes all of /storage.
const LONG: Duration = Duration::from_secs(15 * 60);
const EXTERNAL: &str = "/storage/games-external";
const ROMS: &str = "/storage/roms";
/// automount binds the system card's roms here under unified storage.
const SD1_ROMS: &str = "/run/archr/sd1-roms";
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
    /// The ROMs partition (exFAT) of the system card: on /storage/roms, or
    /// under unified storage the branch bound from it.
    fn roms_partition(&self, mounts: &[(String, String, String)]) -> Option<(u64, u64)> {
        mounts
            .iter()
            .find(|(_, mount, fs)| fs == "exfat" && (mount == ROMS || mount == SD1_ROMS))
            .and_then(|(_, mount, _)| filesystem_usage(&self.paths.at(mount)))
    }
    /// The card mounted at `path` (the system card at /storage, the second
    /// card at games-external), with its usage. The system card with a ROMs
    /// partition shows what is free on each: neither can use the other's.
    fn card(&self, mounts: &[(String, String, String)], path: &str) -> Option<Card> {
        let (device, _, fs) = mounts.iter().rev().find(|(_, mount, _)| mount == path)?;
        let (used, total) = filesystem_usage(&self.paths.at(path))?;
        if let Some((roms_used, roms_total)) =
            self.roms_partition(mounts).filter(|_| path == "/storage")
        {
            return Some(Card {
                name: "settings.card_internal".into(),
                detail: format!(
                    "settings.card_parts|system={}|roms={}",
                    bytes(total.saturating_sub(used)),
                    bytes(roms_total.saturating_sub(roms_used))
                ),
                percent: percent(used + roms_used, total + roms_total).unwrap_or(0),
            });
        }
        let name = match (path == EXTERNAL, self.label(device)) {
            (true, Some(label)) => format!("settings.card_external|label={label}"),
            (true, None) => "settings.card_microsd".into(),
            (false, _) => "settings.card_internal".into(),
        };
        // automount found errors on the second card: say so instead of usage.
        let errors = format!("{}.errors", crate::worker::SECOND_CARD);
        let detail = if path == EXTERNAL && self.paths.at(&errors).exists() {
            "settings.card_errors".into()
        } else {
            format!(
                "settings.card_detail|fs={fs}|used={}|total={}",
                bytes(used),
                bytes(total)
            )
        };
        Some(Card {
            name,
            detail,
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
    /// A card's printed size in whole GB (64, 128): the disk's sectors
    /// rounded up to a power of two, as cards are sold.
    fn nominal_gb(&self, mounts: &[(String, String, String)], path: &str) -> Option<u64> {
        let (device, _, _) = mounts.iter().rev().find(|(_, mount, _)| mount == path)?;
        let name = std::path::Path::new(device).file_name()?.to_str()?;
        let disk = match name.rsplit_once('p') {
            Some((disk, part)) if disk.starts_with("mmcblk") && part.parse::<u8>().is_ok() => disk,
            _ => name,
        };
        let sectors: u64 = self
            .read(&format!("/sys/class/block/{disk}/size"))
            .parse()
            .ok()?;
        let gb = (sectors * 512).div_ceil(1_000_000_000);
        (gb > 0).then(|| gb.next_power_of_two())
    }
    /// Both cards behind one /storage/roms: printed sizes and usage summed.
    fn unified(&self, mounts: &[(String, String, String)]) -> Option<Card> {
        mounts
            .iter()
            .rev()
            .find(|(_, mount, _)| mount == ROMS)
            .filter(|(_, _, fs)| fs == "fuse.mergerfs")?;
        let (first, second) = (
            self.nominal_gb(mounts, "/storage")?,
            self.nominal_gb(mounts, EXTERNAL)?,
        );
        let (used_first, total_first) = filesystem_usage(&self.paths.at("/storage"))?;
        let (used_roms, total_roms) = self.roms_partition(mounts).unwrap_or_default();
        let (used_first, total_first) = (used_first + used_roms, total_first + total_roms);
        let (used_second, total_second) = filesystem_usage(&self.paths.at(EXTERNAL))?;
        let (used, total) = (used_first + used_second, total_first + total_second);
        Some(Card {
            name: format!(
                "settings.card_unified|first=@{first}|second=@{second} GB|total=@{} GB",
                first + second
            ),
            detail: format!(
                "settings.used_total|used={}|total={}",
                bytes(used),
                bytes(total)
            ),
            percent: percent(used, total).unwrap_or(0),
        })
    }
    pub(super) fn storage(&self) -> Storage {
        let cfg = self.cfg().ok();
        let setting = |key| cfg.as_ref().and_then(|c| c.get_setting(key, None, None));
        let mounts = self.mounts();
        Storage {
            card: self.card(&mounts, "/storage"),
            second: self.card(&mounts, EXTERNAL),
            unified: self.unified(&mounts),
            backup: self.latest_backup(setting(Id::ClockFormat.key()).as_deref() == Some("12")),
            password: setting("root.password").unwrap_or_else(|| "archr".into()),
            roms: self.roms_status(),
            ..Storage::default()
        }
    }
    /// automount owns the switch: it saves it and joins or separates the
    /// cards at once. Separating never deletes or moves a file.
    pub(super) fn unify(&self, value: &str) -> Result<String, String> {
        self.command_with_timeout("automount", &["unify", value], LONG)
            .map(|_| value.into())
            .map_err(|error| {
                log::warn!("Unified storage: {error}");
                "settings.merged_failed".into()
            })
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
    /// success. The library follows an ejected card by itself.
    pub(super) fn run_storage(&self, action: &Action) -> Result<&'static str, String> {
        let flag = |wifi: bool| if wifi { &["--with-wifi"][..] } else { &[][..] };
        let script = |args: &[&str], failure: &'static str| {
            self.command_with_timeout("backuptool", args, LONG)
                .map_err(|error| {
                    log::warn!("{error}");
                    failure.to_owned()
                })
        };
        match action {
            Action::Eject => self.eject().map(|()| "settings.eject_done"),
            Action::RomsStart { .. } | Action::RomsCancel | Action::RomsRetry => {
                self.run_roms(action)
            }
            Action::Backup { wifi } => {
                script(
                    &[&["backup"], flag(*wifi)].concat(),
                    "settings.backup_failed",
                )?;
                Ok("settings.backup_done")
            }
            Action::Restore { wifi } => {
                script(
                    &[&["restore"], flag(*wifi)].concat(),
                    "settings.restore_failed",
                )?;
                Ok("settings.restore_reboot_question")
            }
            Action::RootPassword(password) => {
                self.root_password(password)?;
                Ok("settings.password_done")
            }
            // The script reboots when it is done.
            Action::FactoryReset => self
                .command_with_timeout("factoryreset", &["ALL"], LONG)
                .map(|_| "settings.resetting")
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
    /// automount syncs and unmounts the second card (and unified storage's
    /// overlay); a busy card stays mounted and listed.
    fn eject(&self) -> Result<(), String> {
        if !self.mounts().iter().any(|(_, mount, _)| mount == EXTERNAL) {
            return Err("settings.eject_none".into());
        }
        self.command_with_timeout("automount", &["eject"], LONG)
            .map(drop)
            .map_err(|error| {
                log::warn!("Eject: {error}");
                "settings.eject_busy".to_owned()
            })
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
    const CARD: &str = "/dev/mmcblk1p3 /storage ext4 rw 0 0\n\
        /dev/mmcblk0p1 /storage/games-external exfat rw 0 0\n";
    #[test]
    fn eject_asks_automount_and_a_busy_card_stays() {
        let (root, paths) = fixture("", CARD);
        script(&root, &paths, "automount", "");
        let backend = Backend::new(paths.clone());
        assert_eq!(
            backend.run_storage(&Action::Eject),
            Ok("settings.eject_done")
        );
        assert_eq!(calls(&root), "automount eject\n");
        script(&root, &paths, "automount", "exit 1");
        assert_eq!(
            backend.run_storage(&Action::Eject),
            Err("settings.eject_busy".into())
        );
        // Without a second card there is nothing to eject.
        fs::write(
            paths.at("/proc/mounts"),
            "/dev/mmcblk1p3 /storage ext4 rw 0 0\n",
        )
        .unwrap();
        assert_eq!(
            backend.run_storage(&Action::Eject),
            Err("settings.eject_none".into())
        );
        assert_eq!(backend.storage().second, None);
    }
    #[test]
    fn the_system_card_shows_what_is_free_on_each_partition() {
        let mounts = format!("{CARD}/dev/mmcblk1p4 /storage/roms exfat rw 0 0\n");
        let (_root, paths) = fixture("", &mounts);
        fs::create_dir_all(paths.at(ROMS)).unwrap();
        let card = Backend::new(paths.clone()).storage().card.unwrap();
        assert_eq!(card.name, "settings.card_internal");
        assert!(card.detail.starts_with("settings.card_parts|system=@"));
        assert!(card.detail.contains("|roms=@"));
        // Under unified storage the partition is the union's branch.
        let union = format!(
            "{CARD}/dev/mmcblk1p4 /run/archr/sd1-roms exfat rw 0 0\n\
             archr-roms /storage/roms fuse.mergerfs rw 0 0\n"
        );
        fs::write(paths.at("/proc/mounts"), union).unwrap();
        fs::create_dir_all(paths.at(SD1_ROMS)).unwrap();
        let storage = Backend::new(paths).storage();
        assert!(storage.card.unwrap().detail.contains("|roms=@"));
    }
    #[test]
    fn a_card_with_errors_shows_the_warning() {
        let (_root, paths) = fixture("", CARD);
        let backend = Backend::new(paths.clone());
        assert!(backend
            .storage()
            .second
            .unwrap()
            .detail
            .starts_with("settings.card_detail|"));
        let errors = paths.at("/run/archr/roms2.errors");
        fs::create_dir_all(errors.parent().unwrap()).unwrap();
        fs::write(&errors, "").unwrap();
        let storage = backend.storage();
        assert_eq!(storage.second.unwrap().detail, "settings.card_errors");
        assert!(storage
            .card
            .unwrap()
            .detail
            .starts_with("settings.card_detail|"));
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
            Ok("settings.restore_reboot_question")
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
            Ok("settings.password_done")
        );
        assert_eq!(backend.storage().password, SECRET);
        assert_eq!(
            backend.run_storage(&Action::RootPassword("a b".into())),
            Err("settings.root_invalid".into())
        );
    }
    #[test]
    fn both_cards_report_their_name_usage_and_the_newest_backup() {
        let (_root, paths) = fixture("", CARD);
        fs::create_dir_all(paths.at("/dev/disk/by-label")).unwrap();
        std::os::unix::fs::symlink("../../mmcblk0p1", paths.at("/dev/disk/by-label/ROMS\\x20A"))
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
        assert_eq!(card.name, "settings.card_internal");
        assert!(card
            .detail
            .starts_with("settings.card_detail|fs=ext4|used=@"));
        let second = storage.second.unwrap();
        assert_eq!(second.name, "settings.card_external|label=ROMS A");
        assert!(second
            .detail
            .starts_with("settings.card_detail|fs=exfat|used=@"));
        assert!(storage
            .backup
            .is_some_and(|b| b.starts_with("settings.backup_today|time=")));
        assert!(storage.password == "archr" && storage.unified.is_none());
    }
    #[test]
    fn unified_storage_sums_both_cards_and_asks_automount() {
        let union = format!("{CARD}archr-roms /storage/roms fuse.mergerfs rw 0 0\n");
        let (root, paths) = fixture("", &union);
        fs::create_dir_all(paths.at("/storage/roms")).unwrap();
        for (disk, sectors) in [("mmcblk1", 124_735_488u64), ("mmcblk0", 249_737_216)] {
            fs::create_dir_all(paths.at(&format!("/sys/class/block/{disk}"))).unwrap();
            fs::write(
                paths.at(&format!("/sys/class/block/{disk}/size")),
                sectors.to_string(),
            )
            .unwrap();
        }
        let backend = Backend::new(paths.clone());
        let storage = backend.storage();
        let unified = storage.unified.unwrap();
        assert_eq!(
            unified.name,
            "settings.card_unified|first=@64|second=@128 GB|total=@192 GB"
        );
        assert!(unified.detail.starts_with("settings.used_total|used=@"));
        // Each card is still listed under it.
        assert!(storage.card.is_some() && storage.second.is_some());
        // Not joined: no union row.
        fs::write(paths.at("/proc/mounts"), CARD).unwrap();
        assert_eq!(backend.storage().unified, None);
        script(&root, &paths, "automount", "");
        assert_eq!(backend.unify("0"), Ok("0".into()));
        script(&root, &paths, "automount", "exit 1");
        assert_eq!(backend.unify("1"), Err("settings.merged_failed".into()));
        assert_eq!(calls(&root), "automount unify 0\nautomount unify 1\n");
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
