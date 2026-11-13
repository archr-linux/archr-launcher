//! System updates (Atualizacoes*): archr-update's last report and one run,
//! from the download to the restart. The host fills both; this navigates.
use super::{described, row, Effect, Id, Kind, Page, Row, Settings};
use crate::{state::Button, HintButton};
use std::time::Instant;

pub const OPTIONS: &str = "updates.options";
pub const UPDATES: &[Row] = &[
    described(
        Id::UpdateInstall,
        "updates.install",
        "updates.reboot_kernel",
    )
    .kind(Kind::Action),
    row(Id::UpdateCheck, "updates.check", Kind::Action),
    described(
        Id::UpdateCard,
        "updates.from_card",
        "updates.from_card_detail",
    )
    .kind(Kind::Action),
    // The channel picker returns when repo-next / repo-dev are published:
    // until then archr-update reads every channel as stable.
    row(Id::UpdateBoot, "updates.at_startup", Kind::Toggle).under(OPTIONS),
    row(Id::UpdateNotify, "updates.notify", Kind::Toggle),
];
/// Where archr-update writes everything, pacman included.
pub const LOG: &str = "/storage/.config/archr-update.log";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    /// Empty for a new dependency.
    pub old: String,
    pub new: String,
    /// Download size in bytes.
    pub size: u64,
}
/// The package a progress line names: archr-update reports the cached file
/// ("name-version-pkgrel-arch.pkg.tar.zst", the name may hold hyphens) while
/// downloading and the bare name while installing.
pub fn package_name(file: &str) -> &str {
    let Some(at) = file.rfind(".pkg.tar") else {
        return file;
    };
    let mut parts = file[..at].rsplitn(4, '-');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(_), Some(_), Some(_), Some(name)) => name,
        _ => file,
    }
}
/// The catalog suffix of a failure's title and reason
/// (`updates.failed_*`, `updates.why_*`): archr-update's reason code, or
/// "unknown" for a code this launcher does not know.
pub fn failure(reason: &str) -> &'static str {
    match reason {
        "network" => "network",
        "battery" => "battery",
        "space" => "space",
        "busy" => "busy",
        "seed" => "seed",
        "sync" => "sync",
        "download" => "download",
        "install" => "install",
        "offline-invalid" => "offline_invalid",
        "offline-old" => "offline_old",
        "offline-incomplete" => "offline_incomplete",
        _ => "unknown",
    }
}
/// What only takes effect after a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reboot {
    Kernel,
    Launcher,
}
impl Reboot {
    pub fn parse(reason: &str) -> Option<Self> {
        match reason {
            "kernel" => Some(Self::Kernel),
            "launcher" => Some(Self::Launcher),
            _ => None,
        }
    }
}
/// `archr-update check --json`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub version: String,
    pub channel: String,
    pub packages: Vec<Package>,
    pub download: u64,
    pub reboot: Option<Reboot>,
    /// The release's version and notes, when it publishes them.
    pub new_version: String,
    pub news: String,
}
/// `archr-update offline --json`: an update file the user copied to
/// roms/update, installed without the network.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub file: String,
    pub id: String,
    /// Newer than the installed system: only then it installs.
    pub newer: bool,
    pub changelog: Vec<String>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    #[default]
    Sync,
    Download,
    /// An update from the card unpacks and checks its file first.
    Extract,
    Install,
    Hooks,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    /// The package being downloaded or installed.
    pub package: String,
    pub bytes: u64,
    pub total: u64,
    /// Bytes per second.
    pub speed: u64,
    pub current: usize,
    pub count: usize,
    pub percent: u8,
    /// B was pressed; the download stops at the script's next tick.
    pub cancelling: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    Working(Progress),
    Done {
        reboot: Option<Reboot>,
        from: String,
        to: String,
        count: usize,
    },
    /// A reason code of archr-update, pacman's line and whether packages
    /// were already being replaced.
    Failed {
        reason: String,
        line: String,
        changed: bool,
    },
}
#[derive(Clone, Debug, Default)]
pub struct Updates {
    pub report: Option<Report>,
    pub checking: bool,
    pub checked: Option<Instant>,
    /// Reason code of the last check that failed.
    pub failed: Option<String>,
    pub run: Option<Run>,
    /// The update file on the card, once looked for.
    pub card: Option<Card>,
    pub card_looked: bool,
    pub card_looking: bool,
    /// The question before installing the card's file is open.
    pub confirm_card: bool,
    /// The file the run on screen installs; None for the network.
    pub from_card: Option<String>,
}
impl Updates {
    pub fn count(&self) -> usize {
        self.report.as_ref().map_or(0, |r| r.packages.len())
    }
    pub fn working(&self) -> bool {
        matches!(self.run, Some(Run::Working(_)))
    }
    /// The download can still stop; the install cannot.
    pub fn cancellable(&self) -> bool {
        matches!(&self.run, Some(Run::Working(p)) if p.stage <= Stage::Download && !p.cancelling)
    }
    /// Nothing is known yet: the host checks while the page is open.
    pub fn wants_check(&self) -> bool {
        self.report.is_none() && self.failed.is_none() && !self.checking
    }
    /// The card's file itself failed (bad, old, or for a newer image):
    /// trying it again cannot help, so the screen only goes back.
    pub fn card_refused(&self) -> bool {
        matches!(&self.run, Some(Run::Failed { reason, .. }) if reason.starts_with("offline-"))
    }
    /// The page is open and the card was not looked at yet: the host looks.
    pub fn wants_card_scan(&self) -> bool {
        !self.card_looked && !self.card_looking && self.run.is_none()
    }
    /// The card's file installs only when newer, and one thing at a time.
    pub fn card_disabled(&self) -> bool {
        self.card.as_ref().is_none_or(|c| !c.newer) || self.checking
    }
    pub fn card_found(&mut self, card: Option<Card>) {
        self.card_looking = false;
        self.card_looked = true;
        self.card = card;
    }
    /// Subtitle of "Download and install": what restarts at the end.
    pub fn install_note(&self) -> Option<&'static str> {
        let report = self.report.as_ref().filter(|r| !r.packages.is_empty())?;
        Some(match report.reboot {
            Some(Reboot::Kernel) => "updates.reboot_kernel",
            Some(Reboot::Launcher) => "updates.reboot_launcher",
            None => "updates.no_reboot",
        })
    }
}

impl Settings {
    /// Opens the page from anywhere (the START menu), over System.
    pub fn open_updates(&mut self) {
        self.home();
        self.cursor = super::CATEGORIES
            .iter()
            .position(|c| c.0 == Page::System)
            .unwrap_or(0);
        self.open(Page::System);
        self.cursor = Page::System
            .rows()
            .iter()
            .position(|r| r.id == Id::Update)
            .unwrap_or(0);
        self.updates.card_looked = false;
        self.open(Page::Updates);
    }
    pub(super) fn start_update(&mut self) -> Effect {
        self.updates.from_card = None;
        self.updates.run = Some(Run::Working(Progress::default()));
        Effect::Update
    }
    /// The card's row asks first; A on the question installs.
    pub(super) fn ask_card_update(&mut self) -> Effect {
        self.updates.confirm_card = !self.updates.card_disabled();
        Effect::None
    }
    fn start_card_update(&mut self, file: String) -> Effect {
        self.updates.from_card = Some(file.clone());
        self.updates.run = Some(Run::Working(Progress {
            stage: Stage::Extract,
            ..Default::default()
        }));
        Effect::UpdateFromCard(file)
    }
    /// Back on the page after a run: the next check and look tell what is left.
    fn leave_run(&mut self, done: bool) {
        self.updates.run = None;
        self.updates.from_card = None;
        self.updates.card_looked = false;
        if done {
            self.updates.report = None;
            self.updates.failed = None;
        }
    }
    /// Subtitle of the card's row: where it comes from, or why it waits.
    pub(super) fn card_subtitle(&self) -> Option<&'static str> {
        let card = self.updates.card.as_ref()?;
        Some(if card.newer {
            "updates.from_card_detail"
        } else {
            "updates.card_old"
        })
    }
    /// The run screens take every press; the list falls through to the rows.
    pub(super) fn updates_input(&mut self, button: Button) -> Option<Effect> {
        if self.page() == Page::UpdatePackages {
            let last = self.updates.count();
            match button {
                Button::Up => self.cursor = self.cursor.saturating_sub(1),
                Button::Down => self.cursor = (self.cursor + 1).min(last),
                Button::B => return None,
                _ => {}
            }
            return Some(Effect::None);
        }
        if std::mem::take(&mut self.updates.confirm_card) {
            let file = self.updates.card.as_ref().map(|c| c.file.clone());
            return Some(match file {
                Some(file) if button == Button::A && !self.updates.card_disabled() => {
                    self.start_card_update(file)
                }
                _ => Effect::None,
            });
        }
        let cancellable = self.updates.cancellable();
        let listed = self.updates.count() > 0;
        let refused = self.updates.card_refused();
        let effect = match (&mut self.updates.run, button) {
            (Some(Run::Working(progress)), Button::B) if cancellable => {
                progress.cancelling = true;
                Effect::CancelUpdate
            }
            (
                Some(Run::Done {
                    reboot: Some(_), ..
                }),
                Button::A,
            ) => Effect::Reboot,
            (Some(Run::Done { .. }), Button::A | Button::B) => {
                self.leave_run(true);
                Effect::None
            }
            (Some(Run::Failed { .. }), Button::A | Button::B) if refused => {
                self.leave_run(false);
                Effect::None
            }
            (Some(Run::Failed { .. }), Button::A) => match self.updates.from_card.clone() {
                None => self.start_update(),
                Some(file) if !file.is_empty() => self.start_card_update(file),
                // A run followed from before this launcher: choose again.
                Some(_) => {
                    self.leave_run(false);
                    Effect::None
                }
            },
            (Some(Run::Failed { .. }), Button::B) => {
                self.leave_run(false);
                Effect::None
            }
            (Some(_), _) => Effect::None,
            (None, Button::Y) if listed => {
                self.open(Page::UpdatePackages);
                Effect::None
            }
            (None, _) => return None,
        };
        Some(effect)
    }
    pub(super) fn updates_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::Key;
        if self.updates.confirm_card {
            return &[(Key('A'), "hint.confirm"), (Key('B'), "hint.cancel")];
        }
        match &self.updates.run {
            Some(Run::Working(_)) if self.updates.cancellable() => {
                &[(Key('B'), "updates.cancel_download")]
            }
            Some(Run::Working(_)) => &[],
            Some(Run::Done {
                reboot: Some(_), ..
            }) => &[(Key('A'), "menu.reboot"), (Key('B'), "settings.later")],
            Some(Run::Done { .. }) => &[(Key('A'), "updates.close")],
            Some(Run::Failed { .. }) if self.updates.card_refused() => &[(Key('B'), "hint.back")],
            Some(Run::Failed { .. }) => &[(Key('A'), "updates.retry"), (Key('B'), "hint.back")],
            None if self.page() == Page::UpdatePackages => &[(Key('B'), "hint.back")],
            None if self.updates.count() > 0 => &[
                (Key('A'), "hint.choose"),
                (Key('Y'), "updates.packages"),
                (Key('B'), "hint.back"),
            ],
            None => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(packages: usize) -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open_updates();
        state.updates.report = Some(Report {
            packages: vec![Package::default(); packages],
            reboot: Some(Reboot::Kernel),
            ..Default::default()
        });
        state
    }
    #[test]
    fn package_files_name_only_their_package() {
        let hash = "0123456789abcdef".repeat(4);
        for name in ["deadbeefcafe0-tools", "deadbeefcafe1-tools", "AIC8800"] {
            assert_eq!(
                package_name(&format!("{name}-{hash}-1-aarch64.pkg.tar.zst.part")),
                name
            );
            assert_eq!(package_name(name), name);
        }
        // A file without version, pkgrel and architecture stays whole.
        let odd = format!("{}.pkg.tar.zst", "odd");
        assert_eq!(package_name(&odd), odd);
        assert_eq!(package_name(""), "");
    }
    #[test]
    fn opens_over_system_and_installs_only_with_packages() {
        let mut state = page(0);
        assert_eq!(state.page(), Page::Updates);
        assert_eq!(state.parent_page(), Page::System);
        assert!(state.disabled(Id::UpdateInstall));
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.handle(Button::Y), Effect::None);
        assert_eq!(state.page(), Page::Updates);
        state = page(2);
        assert_eq!(state.updates.install_note(), Some("updates.reboot_kernel"));
        assert_eq!(state.handle(Button::A), Effect::Update);
        assert!(state.updates.working());
        assert_eq!(state.handle(Button::B), Effect::CancelUpdate);
        // A second press, and every press of the install, does nothing.
        assert_eq!(state.handle(Button::B), Effect::None);
        if let Some(Run::Working(progress)) = &mut state.updates.run {
            progress.stage = Stage::Install;
            progress.cancelling = false;
        }
        for button in [Button::B, Button::A, Button::Start, Button::Up] {
            assert_eq!(state.handle(button), Effect::None);
        }
        assert_eq!(state.page(), Page::Updates);
        assert!(state.hints().is_empty());
    }
    #[test]
    fn done_restarts_or_checks_again_and_failure_retries() {
        let mut state = page(2);
        state.updates.run = Some(Run::Done {
            reboot: Some(Reboot::Kernel),
            from: "2.1.1".into(),
            to: "2.1.2".into(),
            count: 2,
        });
        assert_eq!(state.handle(Button::A), Effect::Reboot);
        assert_eq!(state.handle(Button::B), Effect::None);
        assert!(state.updates.run.is_none() && state.updates.wants_check());
        state = page(2);
        state.updates.run = Some(Run::Failed {
            reason: "download".into(),
            line: String::new(),
            changed: false,
        });
        assert_eq!(state.handle(Button::A), Effect::Update);
        state.updates.run = Some(Run::Failed {
            reason: "download".into(),
            line: String::new(),
            changed: false,
        });
        assert_eq!(state.handle(Button::B), Effect::None);
        assert_eq!(
            (state.page(), state.updates.run.clone()),
            (Page::Updates, None)
        );
    }
    #[test]
    fn y_lists_the_packages() {
        let mut state = page(3);
        assert_eq!(state.hints()[1], (HintButton::Key('Y'), "updates.packages"));
        state.handle(Button::Y);
        assert_eq!(state.page(), Page::UpdatePackages);
        for _ in 0..5 {
            state.handle(Button::Down);
        }
        assert_eq!(state.cursor, 3, "three packages and the notes");
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Updates);
    }
    #[test]
    fn only_published_channels_are_offered() {
        // archr-update reads any unpublished channel as stable; the page
        // must neither offer one nor show it as the current channel.
        assert!(!UPDATES.iter().any(|row| row.id == Id::UpdateChannel));
        assert_eq!(Id::UpdateChannel.choices(), &["stable"]);
    }
    fn card(newer: bool) -> Card {
        Card {
            file: "/storage/roms/update/archr-update-20261113.1.tar".into(),
            id: "20261113.1".into(),
            newer,
            changelog: vec!["- [system] fix the clock".into()],
        }
    }
    /// The page with this card found, the cursor on its row.
    fn on_card(card: Option<Card>) -> Settings {
        let mut state = page(0);
        state.updates.card_found(card);
        if let Some(at) = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::UpdateCard)
        {
            state.cursor = at;
        }
        state
    }
    #[test]
    fn the_card_row_shows_only_a_found_file_and_installs_only_a_newer_one() {
        let catalog = crate::i18n::I18n::load(std::path::Path::new("/nonexistent"), None, None);
        let shown = |state: &Settings| state.visible_rows().iter().any(|r| r.id == Id::UpdateCard);
        let row = |state: &Settings| {
            let row = state
                .visible_rows()
                .into_iter()
                .find(|r| r.id == Id::UpdateCard);
            state.subtitle(&row.expect("the card's row"))
        };
        let mut state = page(0);
        assert!(state.updates.wants_card_scan(), "the open page looks");
        assert!(!shown(&state));
        state = on_card(None);
        assert!(!state.updates.wants_card_scan() && !shown(&state));
        state = on_card(Some(card(true)));
        assert!(!state.disabled(Id::UpdateCard));
        assert_eq!(state.display_value(Id::UpdateCard, &catalog), "20261113.1");
        assert_eq!(row(&state).as_deref(), Some("updates.from_card_detail"));
        // A network check running holds the lock: the row waits for it.
        state.updates.checking = true;
        assert!(state.disabled(Id::UpdateCard));
        // No network: the check failed, the card still offers its file.
        state.updates.checking = false;
        state.updates.report = None;
        state.updates.failed = Some("network".into());
        assert!(shown(&state) && !state.disabled(Id::UpdateCard));
        state = on_card(Some(card(false)));
        assert!(state.disabled(Id::UpdateCard));
        assert_eq!(state.display_value(Id::UpdateCard, &catalog), "20261113.1");
        assert_eq!(row(&state).as_deref(), Some("updates.card_old"));
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(!state.updates.confirm_card && state.updates.run.is_none());
    }
    #[test]
    fn the_card_asks_before_it_runs_and_runs_its_own_file() {
        let mut state = on_card(Some(card(true)));
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.updates.confirm_card && state.updates.run.is_none());
        assert_eq!(
            state.hints(),
            [
                (HintButton::Key('A'), "hint.confirm"),
                (HintButton::Key('B'), "hint.cancel")
            ]
        );
        // B, or any other button, answers no.
        assert_eq!(state.handle(Button::B), Effect::None);
        assert!(!state.updates.confirm_card && state.updates.run.is_none());
        assert_eq!(state.page(), Page::Updates);
        state.handle(Button::A);
        let file = card(true).file;
        assert_eq!(
            state.handle(Button::A),
            Effect::UpdateFromCard(file.clone())
        );
        assert_eq!(state.updates.from_card.as_deref(), Some(file.as_str()));
        assert!(matches!(&state.updates.run, Some(Run::Working(p)) if p.stage == Stage::Extract));
        assert!(
            !state.updates.cancellable(),
            "the card's update does not stop"
        );
        assert_eq!(state.handle(Button::B), Effect::None);
        // A failure on the way tries the same file again; one of the file
        // itself only goes back, and the page looks at the card again.
        let fail = |reason: &str| {
            Some(Run::Failed {
                reason: reason.into(),
                line: String::new(),
                changed: false,
            })
        };
        state.updates.run = fail("battery");
        assert_eq!(state.handle(Button::A), Effect::UpdateFromCard(file));
        state.updates.run = fail("offline-invalid");
        assert_eq!(state.hints(), [(HintButton::Key('B'), "hint.back")]);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.updates.run.is_none() && state.updates.from_card.is_none());
        assert!(state.updates.wants_card_scan());
        // The network's button installs from the network again.
        state = page(2);
        state.updates.from_card = Some("old".into());
        assert_eq!(state.handle(Button::A), Effect::Update);
        assert!(state.updates.from_card.is_none());
    }
    #[test]
    fn done_and_every_visit_look_at_the_card_again() {
        let mut state = on_card(Some(card(true)));
        state.updates.from_card = Some(card(true).file);
        state.updates.run = Some(Run::Done {
            reboot: None,
            from: "20261112".into(),
            to: "20261113.1".into(),
            count: 3,
        });
        state.handle(Button::A);
        assert!(state.updates.wants_card_scan() && state.updates.wants_check());
        state = on_card(None);
        state.open_updates();
        assert!(state.updates.wants_card_scan());
        state = on_card(None);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::UpdateCheck)
            .unwrap();
        assert_eq!(state.handle(Button::A), Effect::CheckUpdates);
        assert!(state.updates.wants_card_scan());
    }
    #[test]
    fn failure_reasons_have_their_own_words_in_every_language() {
        for (reason, key) in [
            ("offline-invalid", "offline_invalid"),
            ("offline-old", "offline_old"),
            ("offline-incomplete", "offline_incomplete"),
            ("battery", "battery"),
            ("cancelled", "unknown"),
            ("something-new", "unknown"),
        ] {
            assert_eq!(failure(reason), key, "{reason}");
        }
        for language in ["en", "pt_BR", "es", "ru"] {
            let catalog =
                crate::i18n::I18n::load(std::path::Path::new("/nonexistent"), Some(language), None);
            for key in ["offline_invalid", "offline_old", "offline_incomplete"] {
                for kind in ["failed", "why"] {
                    let key = format!("updates.{kind}_{key}");
                    assert_ne!(catalog.text(&key), key, "{language}");
                }
            }
            let why = catalog.text("updates.why_offline_incomplete");
            assert!(why.contains("arch-r.io"), "{language}: {why}");
        }
    }
}
