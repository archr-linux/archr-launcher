//! The toast spool (docs/avisos.md): any process drops `<name>.toast` in
//! /run/archr/notify; the launcher shows it, subject to Settings ›
//! Appearance › Notifications, and deletes it. Malformed files are deleted
//! unshown. A tmpfs folder read once a second, on the main loop's tick.
//! Also the words of the update news the launcher sees itself.
use crate::paths::Paths;
use archr_ui::{
    i18n::I18n,
    settings::updates::{Reboot, Report, Run},
    toast::{Category, Kind, Toast, SPOOL},
};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

const EVERY: Duration = Duration::from_secs(1);
/// Three short lines and a detail never need more.
const LIMIT: u64 = 4096;

pub struct Spool {
    dir: PathBuf,
    next: Instant,
}
impl Spool {
    pub fn new(paths: &Paths) -> Self {
        let dir = paths.at(SPOOL);
        // Writers create it too; the launcher's copy only saves them the step.
        if let Err(error) = fs::create_dir_all(&dir) {
            log::warn!("{}: {error}", dir.display());
        }
        Self {
            dir,
            next: Instant::now(),
        }
    }
    /// The toasts dropped since the last look, oldest first.
    pub fn poll(&mut self, now: Instant) -> Vec<Toast> {
        if now < self.next {
            return Vec::new();
        }
        self.next = now + EVERY;
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<_> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".toast"))
            .map(|e| {
                let at = e.metadata().and_then(|m| m.modified()).ok();
                (at, e.path())
            })
            .collect();
        files.sort();
        files
            .into_iter()
            .filter_map(|(_, path)| {
                let text = archr_io::read_utf8(&path, LIMIT, archr_io::Symlinks::Reject);
                if let Err(error) = fs::remove_file(&path) {
                    log::warn!("{}: {error}", path.display());
                }
                let toast = text.ok().as_deref().and_then(Toast::parse);
                if toast.is_none() {
                    log::warn!("{}: not a toast, deleted", path.display());
                }
                toast
            })
            .collect()
    }
}

/// A check found updates: how many, and the release they bring.
pub fn found(catalog: &I18n, report: &Report) -> Toast {
    let count = report.packages.len();
    let title = catalog.plural(
        "updates.available",
        count as f64,
        &[("count", &count.to_string())],
    );
    let toast = Toast::new(Kind::Info, Category::Updates, title);
    if report.new_version.is_empty() {
        return toast;
    }
    let channel = catalog.text(&format!("updates.{}", report.channel));
    toast.detail(catalog.format(
        "updates.card",
        &[("version", &report.new_version), ("channel", &channel)],
    ))
}
/// An update that ended: done (and whether to restart) or why it failed.
pub fn ended(catalog: &I18n, run: &Run) -> Option<Toast> {
    Some(match run {
        Run::Working(_) => return None,
        Run::Done { reboot, .. } => {
            let detail = match reboot {
                Some(Reboot::Kernel) => "updates.restart_kernel",
                Some(Reboot::Launcher) => "updates.restart_launcher",
                None => "updates.ready",
            };
            Toast::new(Kind::Done, Category::Updates, catalog.text("updates.done"))
                .detail(catalog.text(detail))
        }
        Run::Failed { reason, .. } => {
            let title = catalog.text("toast.update_failed");
            Toast::new(Kind::Error, Category::Updates, title)
                .detail(catalog.text(&format!("updates.failed_{reason}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toasts_are_read_in_order_and_every_file_is_deleted() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let mut spool = Spool::new(&paths);
        let dir = root.path().join("run/archr/notify");
        assert!(dir.is_dir(), "created for the writers");
        fs::write(dir.join("First.toast"), "info\nnetwork\nFirst\n").unwrap();
        // Different times, whatever the file system's resolution.
        std::thread::sleep(Duration::from_millis(20));
        fs::write(dir.join("Second.toast"), "error\nerrors\nSecond\nWhy\n").unwrap();
        fs::write(dir.join("Bad.toast"), "loud\nnetwork\nNo\n").unwrap();
        fs::write(dir.join("Half.tmp"), "info\nnetwork\nBeing written\n").unwrap();
        let now = Instant::now();
        assert_eq!(
            spool.poll(now),
            [
                Toast::new(Kind::Info, Category::Network, "First"),
                Toast::new(Kind::Error, Category::Errors, "Second").detail("Why"),
            ]
        );
        let left: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["Half.tmp"], "shown and malformed ones are gone");
        fs::write(dir.join("Later.toast"), "done\nstorage\nLater\n").unwrap();
        assert!(spool.poll(now).is_empty(), "once a second");
        assert_eq!(spool.poll(now + EVERY).len(), 1);
    }
}
