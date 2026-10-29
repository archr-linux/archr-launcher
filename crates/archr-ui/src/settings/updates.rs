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
    described(Id::UpdateChannel, "updates.channel", "updates.channels").under(OPTIONS),
    row(Id::UpdateBoot, "updates.at_startup", Kind::Toggle),
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    #[default]
    Sync,
    Download,
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
        self.stack.truncate(1);
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
        self.open(Page::Updates);
    }
    pub(super) fn start_update(&mut self) -> Effect {
        self.updates.run = Some(Run::Working(Progress::default()));
        Effect::Update
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
        let cancellable = self.updates.cancellable();
        let listed = self.updates.count() > 0;
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
                // The next check tells what is left.
                self.updates.run = None;
                self.updates.report = None;
                self.updates.failed = None;
                Effect::None
            }
            (Some(Run::Failed { .. }), Button::A) => self.start_update(),
            (Some(Run::Failed { .. }), Button::B) => {
                self.updates.run = None;
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
        match &self.updates.run {
            Some(Run::Working(_)) if self.updates.cancellable() => {
                &[(Key('B'), "updates.cancel_download")]
            }
            Some(Run::Working(_)) => &[],
            Some(Run::Done {
                reboot: Some(_), ..
            }) => &[(Key('A'), "menu.reboot"), (Key('B'), "settings.later")],
            Some(Run::Done { .. }) => &[(Key('A'), "updates.close")],
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
    fn y_lists_the_packages_and_a_new_channel_checks_again() {
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
        state.cursor = 2;
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::UpdateChannel, "next".into())
        );
        state.complete(Id::UpdateChannel, Ok("next".into()));
        assert!(state.updates.wants_check());
    }
}
