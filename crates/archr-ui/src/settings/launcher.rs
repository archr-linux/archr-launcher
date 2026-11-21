//! Settings › Launcher (Config-launcher): the library, what opens at power
//! on and the launcher's own upkeep. What it reads (counts, cache size, the
//! version) and every action belong to the host: it sees the library and the
//! files; this page only asks and shows.
use super::{described, Effect, Id, Kind, Page, Row, Settings};
use crate::{state::Button, HintButton};
use std::time::SystemTime;

pub const ROWS: &[Row] = &[
    described(Id::Resync, "launcher.resync", "launcher.resync_detail")
        .kind(Kind::Action)
        .under("launcher.library"),
    described(Id::PruneMissing, "launcher.prune", "launcher.prune_detail").kind(Kind::Action),
    described(
        Id::ShowEmpty,
        "launcher.show_empty",
        "launcher.show_empty_detail",
    )
    .kind(Kind::Toggle),
    described(Id::ClearCovers, "launcher.covers", "launcher.covers_detail").kind(Kind::Action),
    described(Id::OpenAt, "launcher.open_at", "launcher.open_at_detail").under("launcher.opening"),
    described(
        Id::HiddenSystems,
        "launcher.hidden",
        "launcher.hidden_detail",
    )
    .kind(Kind::Action),
    described(Id::ClearRecent, "launcher.recent", "launcher.recent_detail").kind(Kind::Action),
    described(
        Id::RestartLauncher,
        "launcher.restart",
        "launcher.restart_detail",
    )
    .kind(Kind::Action)
    .under("launcher.maintenance"),
    described(
        Id::RestoreLauncher,
        "launcher.restore",
        "launcher.restore_detail",
    )
    .kind(Kind::Action),
    described(Id::LauncherLog, "launcher.log", "launcher.log_detail").kind(Kind::Action),
    described(
        Id::LauncherVersion,
        "launcher.version",
        "launcher.version_detail",
    )
    .kind(Kind::Action),
];

/// Every row of the page.
pub const IDS: [Id; 11] = [
    Id::Resync,
    Id::PruneMissing,
    Id::ShowEmpty,
    Id::ClearCovers,
    Id::OpenAt,
    Id::HiddenSystems,
    Id::ClearRecent,
    Id::RestartLauncher,
    Id::RestoreLauncher,
    Id::LauncherLog,
    Id::LauncherVersion,
];

/// What the host does for the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Count the games, the missing ones, the cover cache and the rest.
    Look,
    /// Read every system's folder again, the cached lists dropped.
    Resync,
    /// Take the games no longer on the card off favorites and recents.
    Prune,
    /// Delete the cover thumbnails; they are made again as lists show.
    ClearCovers,
    /// Bring every hidden system back.
    ShowHidden,
    /// Empty the recent games.
    ClearRecent,
    /// Quit: the session starts the launcher again.
    Restart,
    /// Theme, system order, hidden systems, folders and the Start shortcuts
    /// back to their defaults; games, favorites and saves stay.
    Restore,
    /// Read the end of the launcher's log.
    ReadLog,
}

/// The page's readings, from the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub games: usize,
    /// Favorites and recents whose file is gone.
    pub missing: usize,
    pub cover_bytes: u64,
    pub hidden: usize,
    pub recent: usize,
    /// When the game lists were last read from the folders.
    pub synced: Option<SystemTime>,
    /// The package's version, short ("1241a58").
    pub version: String,
}

#[derive(Clone, Debug, Default)]
pub struct LauncherPage {
    pub facts: Option<Facts>,
    pub looking: bool,
    /// A question open before an action: its row.
    pub confirm: Option<Id>,
    /// The action under way: its row reads "…".
    pub busy: Option<Id>,
    /// The log's last lines, and where the viewer is.
    pub log: Vec<String>,
    pub scroll: usize,
}

pub const CONFIRM_HINTS: &[(HintButton, &str)] = &[
    (HintButton::Key('A'), "hint.confirm"),
    (HintButton::Key('B'), "hint.cancel"),
];
/// Lines one L/R jump moves in the log.
pub const PAGE_LINES: usize = 14;

/// "142 MB", "640 KB": the cover cache, the number localized.
pub fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("@{} MB", bytes.div_ceil(1024 * 1024))
    } else {
        format!("@{} KB", bytes.div_ceil(1024))
    }
}

/// "agora", "há 2 h": how long ago, as a catalog reference.
pub fn ago(at: SystemTime, now: SystemTime) -> String {
    let minutes = now.duration_since(at).unwrap_or_default().as_secs() / 60;
    match minutes {
        0 => "time.now".into(),
        m if m < 60 => format!("time.minutes|count={m}"),
        m if m < 48 * 60 => format!("time.hours|count={}", m / 60),
        m => format!("time.days|count={}", m / (24 * 60)),
    }
}

impl Settings {
    /// Nothing known yet about the open page: the host looks.
    pub fn launcher_wants_look(&self) -> bool {
        self.page() == Page::Launcher && self.launcher.facts.is_none() && !self.launcher.looking
    }
    /// The host's readings; an action that was under way has ended.
    pub fn launcher_facts(&mut self, facts: Facts) {
        self.launcher.facts = Some(facts);
        self.launcher.looking = false;
        self.launcher.busy = None;
    }
    /// The log's lines, newest last; the viewer opens at its end.
    pub fn launcher_log(&mut self, lines: Vec<String>) {
        self.launcher.scroll = lines.len().saturating_sub(PAGE_LINES);
        self.launcher.log = lines;
    }
    fn launcher_count(&self, read: impl Fn(&Facts) -> usize) -> usize {
        self.launcher.facts.as_ref().map_or(0, read)
    }
    pub(super) fn launcher_disabled(&self, id: Id) -> bool {
        if self.launcher.busy.is_some()
            && matches!(
                id,
                Id::Resync | Id::PruneMissing | Id::ClearCovers | Id::RestoreLauncher
            )
        {
            return true;
        }
        match id {
            Id::PruneMissing => self.launcher_count(|f| f.missing) == 0,
            Id::HiddenSystems => self.launcher_count(|f| f.hidden) == 0,
            Id::ClearRecent => self.launcher_count(|f| f.recent) == 0,
            Id::ClearCovers => self
                .launcher
                .facts
                .as_ref()
                .is_none_or(|f| f.cover_bytes == 0),
            _ => false,
        }
    }
    /// The reading on the right of a row, as a catalog reference.
    pub(super) fn launcher_value(&self, id: Id) -> String {
        if self.launcher.busy == Some(id) {
            return "launcher.working".into();
        }
        let Some(facts) = &self.launcher.facts else {
            return match id {
                Id::RestartLauncher | Id::RestoreLauncher | Id::LauncherLog => "›".into(),
                _ => String::new(),
            };
        };
        match id {
            Id::Resync => format!("launcher.games|count={}|n=@{}", facts.games, facts.games),
            Id::PruneMissing if facts.missing == 0 => "launcher.none".into(),
            Id::PruneMissing => format!("@{}", facts.missing),
            Id::ClearCovers => size(facts.cover_bytes),
            Id::HiddenSystems if facts.hidden == 0 => "launcher.none".into(),
            Id::HiddenSystems => format!("@{}", facts.hidden),
            Id::ClearRecent if facts.recent == 0 => "launcher.none".into(),
            Id::ClearRecent => "launcher.clear".into(),
            Id::RestartLauncher | Id::RestoreLauncher | Id::LauncherLog => "›".into(),
            Id::LauncherVersion => format!("{} · {}", facts.version, self.value(Id::Theme)),
            _ => String::new(),
        }
    }
    /// The footer: how long ago the lists were last read, as a catalog
    /// reference for `launcher.synced`'s {time}.
    pub fn launcher_note(&self, now: SystemTime) -> Option<String> {
        if self.page() != Page::Launcher {
            return None;
        }
        let at = self.launcher.facts.as_ref()?.synced?;
        Some(ago(at, now))
    }
    /// A reading followed by "›": the row acts on what it counts.
    pub(super) fn launcher_arrow(&self, id: Id) -> bool {
        matches!(
            id,
            Id::Resync | Id::PruneMissing | Id::ClearCovers | Id::HiddenSystems | Id::ClearRecent
        ) && self.launcher.facts.is_some()
            && self.launcher.busy != Some(id)
            && !self.launcher_disabled(id)
    }
    /// What A does on the selected row, said in the footer.
    pub(super) fn launcher_verb(&self) -> Option<&'static str> {
        let row = self.visible_rows().get(self.cursor).copied()?;
        if self.launcher_disabled(row.id) || self.launcher.busy.is_some() {
            return None;
        }
        Some(match row.id {
            Id::Resync => "launcher.do_resync",
            Id::PruneMissing => "launcher.do_prune",
            Id::ClearCovers | Id::ClearRecent => "launcher.clear",
            Id::HiddenSystems => "launcher.do_show",
            Id::RestartLauncher => "launcher.do_restart",
            Id::RestoreLauncher => "launcher.do_restore",
            Id::LauncherLog => "launcher.do_view",
            _ => return None,
        })
    }
    pub(super) fn launcher_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::Key;
        const BACK: (HintButton, &str) = (Key('B'), "hint.back");
        match self.launcher_verb() {
            Some("launcher.do_resync") => &[(Key('A'), "launcher.do_resync"), BACK],
            Some("launcher.do_prune") => &[(Key('A'), "launcher.do_prune"), BACK],
            Some("launcher.clear") => &[(Key('A'), "launcher.clear"), BACK],
            Some("launcher.do_show") => &[(Key('A'), "launcher.do_show"), BACK],
            Some("launcher.do_restart") => &[(Key('A'), "launcher.do_restart"), BACK],
            Some("launcher.do_restore") => &[(Key('A'), "launcher.do_restore"), BACK],
            Some("launcher.do_view") => &[(Key('A'), "launcher.do_view"), BACK],
            _ => &[BACK],
        }
    }
    pub(super) fn launcher_activate(&mut self, id: Id) -> Option<Effect> {
        if !IDS.contains(&id) {
            return None;
        }
        if self.launcher_disabled(id) || self.launcher.busy.is_some() {
            return Some(Effect::None);
        }
        Some(match id {
            Id::Resync => self.launcher_run(id, Action::Resync),
            Id::ClearCovers => self.launcher_run(id, Action::ClearCovers),
            Id::HiddenSystems => self.launcher_run(id, Action::ShowHidden),
            Id::ClearRecent => self.launcher_run(id, Action::ClearRecent),
            // These cannot be undone, or take the screen away: asked first.
            Id::PruneMissing | Id::RestoreLauncher | Id::RestartLauncher => {
                self.launcher.confirm = Some(id);
                Effect::None
            }
            Id::LauncherLog => {
                self.launcher.log.clear();
                self.launcher.scroll = 0;
                self.open(Page::LauncherLog);
                Effect::Launcher(Action::ReadLog)
            }
            _ => Effect::None,
        })
    }
    fn launcher_run(&mut self, id: Id, action: Action) -> Effect {
        self.launcher.busy = Some(id);
        Effect::Launcher(action)
    }
    /// The question before an action: A goes ahead, any other button cancels.
    pub(super) fn launcher_input(&mut self, button: Button) -> Option<Effect> {
        let id = self.launcher.confirm.take()?;
        if button != Button::A {
            return Some(Effect::None);
        }
        Some(match id {
            Id::PruneMissing => self.launcher_run(id, Action::Prune),
            Id::RestartLauncher => self.launcher_run(id, Action::Restart),
            Id::RestoreLauncher => {
                // The look is this state's own: the draft goes back at once
                // and the host persists it.
                self.restore_look();
                self.values
                    .insert(Id::Theme, Id::Theme.default_value().into());
                self.launcher_run(id, Action::Restore)
            }
            _ => Effect::None,
        })
    }
    /// The open question, for the dialog: title, detail.
    pub fn launcher_question(&self) -> Option<(String, &'static str)> {
        let id = self.launcher.confirm?;
        Some(match id {
            Id::PruneMissing => (
                format!(
                    "launcher.prune_question|count={}",
                    self.launcher_count(|f| f.missing)
                ),
                "launcher.prune_what",
            ),
            Id::RestartLauncher => ("launcher.restart_question".into(), "launcher.restart_what"),
            _ => ("launcher.restore_question".into(), "launcher.restore_what"),
        })
    }
    /// The log viewer scrolls by a line (↑ ↓) or a page (L R).
    pub(super) fn launcher_log_input(&mut self, button: Button) -> Option<Effect> {
        let last = self.launcher.log.len().saturating_sub(PAGE_LINES);
        let scroll = &mut self.launcher.scroll;
        match button {
            Button::Up => *scroll = scroll.saturating_sub(1),
            Button::Down => *scroll = (*scroll + 1).min(last),
            Button::L => *scroll = scroll.saturating_sub(PAGE_LINES),
            Button::R => *scroll = (*scroll + PAGE_LINES).min(last),
            _ => return None,
        }
        Some(Effect::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Settings {
        let mut state = Settings::default();
        state.open(Page::Launcher);
        state
    }
    fn facts() -> Facts {
        Facts {
            games: 1284,
            missing: 12,
            cover_bytes: 142 * 1024 * 1024,
            hidden: 2,
            recent: 5,
            synced: None,
            version: "1241a58".into(),
        }
    }

    #[test]
    fn the_page_asks_the_host_to_look_once() {
        let mut state = page();
        assert!(state.launcher_wants_look());
        state.launcher.looking = true;
        assert!(!state.launcher_wants_look());
        state.launcher_facts(facts());
        assert!(!state.launcher_wants_look());
        assert!(!state.launcher.looking);
    }

    #[test]
    fn readings_follow_the_facts() {
        let mut state = page();
        assert_eq!(state.launcher_value(Id::Resync), "");
        state.launcher_facts(facts());
        assert_eq!(
            state.launcher_value(Id::Resync),
            "launcher.games|count=1284|n=@1284"
        );
        assert_eq!(state.launcher_value(Id::PruneMissing), "@12");
        assert_eq!(state.launcher_value(Id::ClearCovers), "@142 MB");
        assert_eq!(state.launcher_value(Id::HiddenSystems), "@2");
        assert_eq!(state.launcher_value(Id::ClearRecent), "launcher.clear");
        assert!(state
            .launcher_value(Id::LauncherVersion)
            .starts_with("1241a58 · "));
    }

    #[test]
    fn nothing_to_do_disables_the_row() {
        let mut state = page();
        state.launcher_facts(Facts {
            missing: 0,
            hidden: 0,
            recent: 0,
            cover_bytes: 0,
            ..facts()
        });
        for id in [
            Id::PruneMissing,
            Id::HiddenSystems,
            Id::ClearRecent,
            Id::ClearCovers,
        ] {
            assert!(state.launcher_disabled(id), "{id:?}");
            assert_eq!(state.launcher_activate(id), Some(Effect::None));
        }
        assert_eq!(state.launcher_value(Id::PruneMissing), "launcher.none");
        assert!(!state.launcher_disabled(Id::Resync));
    }

    #[test]
    fn quick_actions_run_at_once_and_read_busy() {
        let mut state = page();
        state.launcher_facts(facts());
        assert_eq!(
            state.launcher_activate(Id::Resync),
            Some(Effect::Launcher(Action::Resync))
        );
        assert_eq!(state.launcher_value(Id::Resync), "launcher.working");
        // One at a time.
        assert_eq!(state.launcher_activate(Id::ClearCovers), Some(Effect::None));
        state.launcher_facts(facts());
        assert_eq!(
            state.launcher_activate(Id::HiddenSystems),
            Some(Effect::Launcher(Action::ShowHidden))
        );
    }

    #[test]
    fn lasting_actions_are_asked_first() {
        let mut state = page();
        state.launcher_facts(facts());
        for (id, action) in [
            (Id::PruneMissing, Action::Prune),
            (Id::RestartLauncher, Action::Restart),
            (Id::RestoreLauncher, Action::Restore),
        ] {
            assert_eq!(state.launcher_activate(id), Some(Effect::None));
            assert!(state.launcher_question().is_some());
            assert_eq!(state.launcher_input(Button::B), Some(Effect::None));
            assert_eq!(state.launcher.confirm, None);
            assert_eq!(state.launcher.busy, None);
            state.launcher_activate(id);
            assert_eq!(
                state.launcher_input(Button::A),
                Some(Effect::Launcher(action))
            );
            state.launcher_facts(facts());
        }
        assert_eq!(state.launcher_input(Button::A), None);
    }

    #[test]
    fn restore_puts_the_look_back_at_once() {
        let mut state = page();
        state.launcher_facts(facts());
        state.values.insert(Id::Theme, "ellegance".into());
        state.values.insert(Id::TextSize, "large".into());
        state.launcher_activate(Id::RestoreLauncher);
        state.launcher_input(Button::A);
        assert_eq!(state.value(Id::Theme), Id::Theme.default_value());
        assert_eq!(state.value(Id::TextSize), Id::TextSize.default_value());
    }

    #[test]
    fn the_log_opens_at_its_end_and_scrolls() {
        let mut state = page();
        assert_eq!(
            state.launcher_activate(Id::LauncherLog),
            Some(Effect::Launcher(Action::ReadLog))
        );
        assert_eq!(state.page(), Page::LauncherLog);
        state.launcher_log((0..40).map(|n| n.to_string()).collect());
        assert_eq!(state.launcher.scroll, 40 - PAGE_LINES);
        state.launcher_log_input(Button::Down);
        assert_eq!(state.launcher.scroll, 40 - PAGE_LINES);
        state.launcher_log_input(Button::L);
        assert_eq!(state.launcher.scroll, 40 - 2 * PAGE_LINES);
        state.launcher_log_input(Button::Up);
        assert_eq!(state.launcher.scroll, 40 - 2 * PAGE_LINES - 1);
    }

    #[test]
    fn sizes_read_in_megabytes_or_kilobytes() {
        assert_eq!(size(142 * 1024 * 1024), "@142 MB");
        assert_eq!(size(1), "@1 KB");
        assert_eq!(size(0), "@0 KB");
    }
}
