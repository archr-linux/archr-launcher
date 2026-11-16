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

/// Once per boot for each news (the launcher starts again after every
/// game): what was last said under `topic` is kept in /run, in
/// `toast-<topic>`.
pub fn first_this_boot(paths: &Paths, topic: &str, news: &str) -> bool {
    let file = paths.run.join(format!("toast-{topic}"));
    if fs::read_to_string(&file).is_ok_and(|said| said == news) {
        return false;
    }
    if let Err(error) = fs::create_dir_all(&paths.run).and_then(|()| fs::write(&file, news)) {
        log::warn!("{}: {error}", file.display());
    }
    true
}

/// A store whose updates the START menu marks: themes or modules.
#[derive(Clone, Copy)]
pub enum Shop {
    Themes,
    Modules,
}
/// The news of one store's updates, told like the system's: once per boot
/// and set of versions, so the periodic look does not repeat it, and never
/// on the store's own page (seen there, it counts as said).
pub struct StoreNews {
    shop: Shop,
    last: String,
}
impl StoreNews {
    pub fn new(shop: Shop) -> Self {
        Self {
            shop,
            last: String::new(),
        }
    }
    /// `updates`: id and newer version of each item the store has, and the
    /// name to show for an id.
    pub fn check(
        &mut self,
        paths: &Paths,
        catalog: &I18n,
        updates: &[(String, String)],
        name: impl Fn(&str) -> String,
        on_page: bool,
    ) -> Option<Toast> {
        let news: Vec<_> = updates.iter().map(|(n, v)| format!("{n} {v}")).collect();
        let news = news.join("\n");
        if news == self.last {
            return None;
        }
        self.last.clone_from(&news);
        let (topic, one, many) = match self.shop {
            Shop::Themes => ("themes", "toast.theme_update", "toast.theme_updates"),
            Shop::Modules => ("modules", "toast.module_update", "toast.module_updates"),
        };
        if updates.is_empty() || !first_this_boot(paths, topic, &news) || on_page {
            return None;
        }
        let title = match updates {
            [(id, _)] => catalog.format(one, &[("name", &name(id))]),
            _ => catalog.plural(
                many,
                updates.len() as f64,
                &[("count", &updates.len().to_string())],
            ),
        };
        Some(Toast::new(Kind::Info, Category::Updates, title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_updates_are_told_once_per_boot_and_version() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let catalog = I18n::load(std::path::Path::new("/nonexistent"), Some("en"), None);
        let named = |id: &str| format!("Praia ({id})");
        let one = [("praia".to_owned(), "t1".to_owned())];
        let mut themes = StoreNews::new(Shop::Themes);
        let check = |news: &mut StoreNews, updates: &[(String, String)], on_page| {
            news.check(&paths, &catalog, updates, named, on_page)
                .map(|t| t.title)
        };
        assert_eq!(check(&mut themes, &[], false), None, "nothing to tell");
        assert_eq!(
            check(&mut themes, &one, false).as_deref(),
            Some("Theme update: Praia (praia)")
        );
        // The periodic look finds the same: said already, and after the
        // launcher comes back from a game too.
        assert_eq!(check(&mut themes, &one, false), None);
        assert_eq!(check(&mut StoreNews::new(Shop::Themes), &one, false), None);
        // A newer version is news again; each store keeps its own.
        let two = [
            ("a".to_owned(), "1.1".to_owned()),
            ("b".to_owned(), "2.0".to_owned()),
        ];
        let mut modules = StoreNews::new(Shop::Modules);
        assert_eq!(
            check(&mut modules, &two, false).as_deref(),
            Some("2 module updates")
        );
        let newer = [("praia".to_owned(), "t2".to_owned())];
        assert!(check(&mut themes, &newer, false).is_some());
        // Seen on the store's own page: no card, and none after leaving it.
        let three = [("c".to_owned(), "3.0".to_owned())];
        assert_eq!(check(&mut modules, &three, true), None);
        assert_eq!(check(&mut modules, &three, false), None);
        assert_eq!(
            check(&mut StoreNews::new(Shop::Modules), &three, false),
            None
        );
        // The system's news keeps its own file.
        assert!(first_this_boot(&paths, "updates", "3.0.4 2"));
        assert!(root
            .path()
            .join("run/archr-launcher/toast-updates")
            .is_file());
    }

    #[test]
    fn store_updates_follow_the_updates_category() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let catalog = I18n::load(std::path::Path::new("/nonexistent"), Some("pt_BR"), None);
        let updates = [("bateria".to_owned(), "1.1.0".to_owned())];
        let toast = StoreNews::new(Shop::Modules)
            .check(&paths, &catalog, &updates, str::to_owned, false)
            .unwrap();
        assert_eq!(toast.title, "Atualização de módulo: bateria");
        assert_eq!(
            (toast.kind, toast.category),
            (Kind::Info, Category::Updates)
        );
        let now = Instant::now();
        let mut cards = archr_ui::toast::Toasts::default();
        cards.prefs.show[Category::Updates as usize] = false;
        cards.push(toast.clone(), now);
        assert!(cards.shown().is_empty(), "Notifications › Updates off");
        cards.prefs.show[Category::Updates as usize] = true;
        cards.push(toast, now);
        assert_eq!(cards.shown().len(), 1);
    }

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
