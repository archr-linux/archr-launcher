//! System › Save sync (Saves-sync, Saves-destino, Saves-conflito): the saves
//! in a folder every console of the user shares, a PC share or an rclone
//! cloud remote. `archr-savesync` does the work; each visit reads its
//! `status`, and the host runs the test, the sync and the conflict choices
//! off SDL's thread. The share password goes to archr-savesync on its
//! stdin and never comes back: the page only knows whether one is saved.
use super::{described, row, Effect, Id, Kind, Page, Row, Settings};
use crate::keyboard::Mode;
use crate::state::Button;
use serde::Deserialize;

pub const SAVESYNC: &[Row] = &[
    described(Id::SaveSync, "savesync.enabled", "savesync.enabled_detail")
        .kind(Kind::Toggle)
        .under("savesync.section_sync"),
    described(
        Id::SaveSyncTargetLink,
        "savesync.target",
        "savesync.target_none",
    )
    .kind(Kind::Action),
    described(
        Id::SaveSyncBoot,
        "savesync.on_boot",
        "savesync.on_boot_detail",
    )
    .kind(Kind::Toggle)
    .under("savesync.section_when"),
    described(
        Id::SaveSyncExit,
        "savesync.on_exit",
        "savesync.on_exit_detail",
    )
    .kind(Kind::Toggle),
    described(
        Id::SaveSyncStates,
        "savesync.states",
        "savesync.states_detail",
    )
    .kind(Kind::Toggle),
    described(Id::SaveSyncNow, "savesync.now", "savesync.never").kind(Kind::Action),
    described(
        Id::SaveSyncConflicts,
        "savesync.conflicts",
        "savesync.conflicts_detail",
    )
    .kind(Kind::Action),
];
/// Destination (Saves-destino): the two places as a choice, then the PC's
/// share; a cloud remote is set up with rclone on a PC (the card says how).
pub const TARGET: &[Row] = &[
    described(Id::SaveSyncPc, "savesync.pc", "savesync.pc_detail")
        .kind(Kind::Action)
        .under("savesync.section_where"),
    described(Id::SaveSyncCloud, "savesync.cloud", "savesync.cloud_detail").kind(Kind::Action),
    row(Id::SaveSyncAddress, "savesync.address", Kind::Action).under("savesync.section_pc"),
    row(Id::SaveSyncUser, "savesync.user", Kind::Action),
    described(
        Id::SaveSyncPassword,
        "savesync.password",
        "savesync.password_detail",
    )
    .kind(Kind::Action),
    described(Id::SaveSyncTest, "savesync.test", "savesync.test_detail").kind(Kind::Action),
];

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Last {
    pub ok: bool,
    pub at: u64,
    pub up: u32,
    pub down: u32,
    pub systems: Vec<String>,
    pub conflicts: u32,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Side {
    pub at: u64,
    pub size: u64,
    /// The console that sent the folder's copy (empty on this side).
    pub by: String,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Conflict {
    /// saves/<system>/<path>, savestates/... or psp/SAVEDATA/...
    pub key: String,
    pub local: Side,
    pub remote: Side,
}
impl Conflict {
    pub fn game(&self) -> &str {
        let name = self.key.rsplit('/').next().unwrap_or(&self.key);
        name.rsplit_once('.').map_or(name, |(stem, _)| stem)
    }
    pub fn system(&self) -> &str {
        match self.key.split('/').collect::<Vec<_>>()[..] {
            ["psp", ..] => "psp",
            [_, system, ..] => system,
            _ => "",
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Status {
    pub enabled: bool,
    pub last: Option<Last>,
    pub conflicts: Vec<Conflict>,
    pub password: bool,
}
/// What archr-savesync answered a test, a sync or a choice: the error
/// code it names (unreachable, auth, no_share, ...) when not ok.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Answer {
    pub ok: bool,
    pub error: Option<String>,
    pub consoles: u32,
    pub conflicts: u32,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Action {
    Status,
    Test,
    Sync,
    Password(String),
    /// The conflict's key; true keeps this console's copy.
    Resolve(String, bool),
}
/// The password never reaches a log.
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status => write!(f, "Status"),
            Self::Test => write!(f, "Test"),
            Self::Sync => write!(f, "Sync"),
            Self::Password(_) => write!(f, "Password(<hidden>)"),
            Self::Resolve(key, local) => write!(f, "Resolve({key:?}, {local})"),
        }
    }
}
impl Action {
    /// archr-savesync's arguments, and what goes to its stdin.
    pub fn command(&self) -> (Vec<String>, Option<String>) {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
        match self {
            Self::Status => (args(&["status"]), None),
            Self::Test => (args(&["test"]), None),
            Self::Sync => (args(&["sync"]), None),
            Self::Password(secret) => (args(&["password"]), Some(secret.clone())),
            Self::Resolve(key, local) => (
                args(&["resolve", key, if *local { "local" } else { "remote" }]),
                None,
            ),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct SaveSync {
    pub status: Option<Status>,
    pub loading: bool,
    /// Read the status again, keeping the page on screen meanwhile.
    pub stale: bool,
    /// The action running; the page waits for it.
    pub busy: Option<Action>,
    /// The last test: consoles using the folder, or the error code.
    pub test: Option<Result<u32, String>>,
    /// The sync the user asked found conflicts: show the first.
    pub show_conflict: bool,
    /// The conflict page's card in focus: 0 this console's, 1 the folder's.
    pub choice: u8,
}
const PAGES: [Page; 3] = [Page::SaveSync, Page::SaveSyncTarget, Page::SaveSyncConflict];
pub(super) const IDS: &[Id] = &[
    Id::SaveSync,
    Id::SaveSyncTargetLink,
    Id::SaveSyncBoot,
    Id::SaveSyncExit,
    Id::SaveSyncStates,
    Id::SaveSyncNow,
    Id::SaveSyncConflicts,
    Id::SaveSyncPc,
    Id::SaveSyncCloud,
    Id::SaveSyncAddress,
    Id::SaveSyncUser,
    Id::SaveSyncPassword,
    Id::SaveSyncTest,
];

/// A time as the pages say it: `today` with "14:20", or `day` with
/// "18/11" and "14:20".
fn when((today, day): (&str, &str), at: u64, now: u64, clock_12: bool) -> String {
    let clock = crate::status::local_clock(at, clock_12);
    let date = |t: u64| {
        crate::status::local_time(t).map(|tm| (tm.tm_year, tm.tm_yday, tm.tm_mday, tm.tm_mon))
    };
    match (date(at), date(now)) {
        (Some(a), Some(n)) if (a.0, a.1) != (n.0, n.1) => {
            format!("{day}|date={:02}/{:02}|time={clock}", a.2, a.3 + 1)
        }
        _ => format!("{today}|time={clock}"),
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Settings {
    /// An open save-sync page with nothing known yet: the host looks.
    pub fn savesync_wants_status(&self) -> bool {
        PAGES.contains(&self.page())
            && (self.savesync.status.is_none() || self.savesync.stale)
            && !self.savesync.loading
            && self.savesync.busy.is_none()
    }
    /// The conflict the page shows: the first one left.
    pub fn savesync_conflict(&self) -> Option<&Conflict> {
        self.savesync.status.as_ref()?.conflicts.first()
    }
    fn conflicts(&self) -> usize {
        self.savesync
            .status
            .as_ref()
            .map_or(0, |s| s.conflicts.len())
    }
    fn clock_12(&self) -> bool {
        self.value(Id::ClockFormat) == "12"
    }
    /// A time on the pages, a catalog reference.
    pub fn savesync_when(&self, at: u64) -> String {
        when(
            ("savesync.at_today", "savesync.at_day"),
            at,
            now(),
            self.clock_12(),
        )
    }
    pub(super) fn savesync_visible(&self, id: Id) -> bool {
        let pc = self.value(Id::SaveSyncTarget) != "cloud";
        match id {
            Id::SaveSyncConflicts => self.conflicts() > 0,
            Id::SaveSyncAddress | Id::SaveSyncUser | Id::SaveSyncPassword => pc,
            _ => true,
        }
    }
    fn savesync_run(&mut self, action: Action) -> Effect {
        self.savesync.busy = Some(action.clone());
        Effect::SaveSync(action)
    }
    pub(super) fn savesync_activate(&mut self, id: Id) -> Option<Effect> {
        if self.savesync.busy.is_some() {
            return Some(Effect::None);
        }
        match id {
            Id::SaveSyncTargetLink => self.open(Page::SaveSyncTarget),
            Id::SaveSyncConflicts => {
                self.savesync.choice = 0;
                self.open(Page::SaveSyncConflict);
            }
            Id::SaveSyncNow => {
                self.savesync.show_conflict = true;
                return Some(self.savesync_run(Action::Sync));
            }
            Id::SaveSyncTest => {
                self.savesync.test = None;
                return Some(self.savesync_run(Action::Test));
            }
            Id::SaveSyncPc | Id::SaveSyncCloud => {
                let value = if id == Id::SaveSyncPc { "smb" } else { "cloud" };
                if self.value(Id::SaveSyncTarget) == value {
                    return Some(Effect::None);
                }
                self.savesync.test = None;
                self.pending = Some(Id::SaveSyncTarget);
                return Some(Effect::Set(Id::SaveSyncTarget, value.into()));
            }
            // Never prefilled: the saved one is not shown, only replaced.
            Id::SaveSyncPassword => {
                self.open_keyboard(Id::SaveSyncPassword, Mode::Secret, String::new())
            }
            _ => return None,
        }
        Some(Effect::None)
    }
    pub(super) fn savesync_typed(&mut self, value: String) -> Effect {
        self.savesync.test = None;
        self.savesync_run(Action::Password(value))
    }
    /// Destination: Y tests the connection from any row.
    pub(super) fn savesync_target_input(&mut self, button: Button) -> Option<Effect> {
        (button == Button::Y).then(|| self.savesync_activate(Id::SaveSyncTest))?
    }
    /// The conflict page (Saves-conflito): ‹ › moves between the two
    /// copies, A keeps the one in focus, X takes the folder's, B leaves it
    /// for later.
    pub(super) fn savesync_conflict_input(&mut self, button: Button) -> Option<Effect> {
        if self.savesync.busy.is_some() {
            return Some(Effect::None);
        }
        let keep = |state: &mut Self, local: bool| {
            let key = state.savesync_conflict().map(|c| c.key.clone());
            Some(key.map_or(Effect::None, |key| {
                state.savesync_run(Action::Resolve(key, local))
            }))
        };
        match button {
            Button::Left | Button::Right | Button::L | Button::R | Button::Up | Button::Down => {
                self.savesync.choice = 1 - self.savesync.choice.min(1);
                Some(Effect::None)
            }
            Button::A => keep(self, self.savesync.choice == 0),
            Button::X => keep(self, false),
            Button::B => Some(self.back()),
            _ => Some(Effect::None),
        }
    }
    /// The host's answer: archr-savesync's JSON, or why it could not run.
    pub fn savesync_done(&mut self, action: &Action, result: Result<String, String>) {
        self.savesync.busy = None;
        if let Action::Status = action {
            self.savesync.loading = false;
            self.savesync.stale = false;
            let status = result
                .and_then(|json| serde_json::from_str::<Status>(&json).map_err(|e| e.to_string()));
            match status {
                Ok(status) => {
                    let open = self.savesync.show_conflict && !status.conflicts.is_empty();
                    self.savesync.status = Some(status);
                    self.savesync.show_conflict = false;
                    if open && self.page() == Page::SaveSync {
                        self.savesync.choice = 0;
                        self.open(Page::SaveSyncConflict);
                    }
                }
                Err(error) => {
                    log::warn!("archr-savesync status: {error}");
                    self.savesync.status.get_or_insert_with(Status::default);
                }
            }
            return;
        }
        let answer = result
            .and_then(|json| serde_json::from_str::<Answer>(&json).map_err(|e| e.to_string()))
            .unwrap_or_else(|error| {
                log::warn!("archr-savesync {action:?}: {error}");
                Answer {
                    error: Some("failed".into()),
                    ..Answer::default()
                }
            });
        let error = || answer.error.clone().unwrap_or_else(|| "failed".into());
        match action {
            Action::Test => {
                self.savesync.test = Some(if answer.ok {
                    Ok(answer.consoles)
                } else {
                    Err(error())
                });
            }
            Action::Password(_) if !answer.ok => self.fail(None, "savesync.password_failed".into()),
            Action::Resolve(..) if !answer.ok => {
                self.fail(None, format!("savesync.error_{}", error()))
            }
            // The conflict page shows the next one, or leaves when none is left.
            Action::Resolve(..) if self.page() == Page::SaveSyncConflict => {
                self.savesync.choice = 0;
                if answer.conflicts == 0 {
                    self.back();
                }
            }
            _ => {}
        }
        self.savesync.stale = true;
    }
    pub(super) fn savesync_subtitle(&self, row: &Row) -> Option<String> {
        let ss = &self.savesync;
        Some(match row.id {
            Id::SaveSyncTargetLink if self.value(Id::SaveSyncTarget) == "cloud" => {
                "savesync.target_cloud".into()
            }
            Id::SaveSyncTargetLink if self.value(Id::SaveSyncAddress).is_empty() => {
                "savesync.target_none".into()
            }
            Id::SaveSyncTargetLink => format!(
                "savesync.target_pc|address={}",
                self.value(Id::SaveSyncAddress).replace('|', "")
            ),
            Id::SaveSyncNow if matches!(ss.busy, Some(Action::Sync)) => "savesync.syncing".into(),
            Id::SaveSyncNow => {
                let Some(last) = ss.status.as_ref().and_then(|s| s.last.as_ref()) else {
                    return row.subtitle.map(Into::into);
                };
                match &last.error {
                    Some(code) => format!("savesync.last_{code}"),
                    None if last.up + last.down == 0 => {
                        let keys = ("savesync.last_quiet_today", "savesync.last_quiet_day");
                        when(keys, last.at, now(), self.clock_12())
                    }
                    None => format!(
                        "{}|count={}|systems={}",
                        when(
                            if last.up + last.down == 1 {
                                (
                                    "savesync.last_moved_one_today",
                                    "savesync.last_moved_one_day",
                                )
                            } else {
                                ("savesync.last_moved_today", "savesync.last_moved_day")
                            },
                            last.at,
                            now(),
                            self.clock_12()
                        ),
                        last.up + last.down,
                        last.systems.join(", ").to_uppercase()
                    ),
                }
            }
            Id::SaveSyncTest if matches!(ss.busy, Some(Action::Test)) => "savesync.testing".into(),
            Id::SaveSyncTest => match &ss.test {
                Some(Ok(1)) => "savesync.test_ok_one".into(),
                Some(Ok(count)) => format!("savesync.test_ok|count={count}"),
                Some(Err(code)) => format!("savesync.error_{code}"),
                None => row.subtitle?.into(),
            },
            _ => row.subtitle?.into(),
        })
    }
    /// The value column: a catalog reference, or the text itself.
    pub(super) fn savesync_value(&self, id: Id) -> String {
        match id {
            Id::SaveSyncTest if self.savesync.busy == Some(Action::Test) => String::new(),
            Id::SaveSyncTest => match &self.savesync.test {
                Some(Ok(_)) => "savesync.ok".into(),
                Some(Err(_)) => "savesync.failed".into(),
                None => "›".into(),
            },
            Id::SaveSyncTargetLink | Id::SaveSyncNow => "›".into(),
            Id::SaveSyncConflicts => self.conflicts().to_string(),
            Id::SaveSyncPassword if self.savesync.status.as_ref().is_some_and(|s| s.password) => {
                "••••••••".into()
            }
            Id::SaveSyncPassword => "settings.define".into(),
            _ => String::new(),
        }
    }
    /// The test's verdict colours its "ok" or "failed".
    pub fn savesync_ink(&self, id: Id) -> Option<bool> {
        match (id, &self.savesync.test) {
            (Id::SaveSyncTest, Some(result)) => Some(result.is_ok()),
            _ => None,
        }
    }
    /// The radio mark of a destination row: Some(chosen).
    pub fn savesync_radio(&self, id: Id) -> Option<bool> {
        let cloud = self.value(Id::SaveSyncTarget) == "cloud";
        match id {
            Id::SaveSyncPc => Some(!cloud),
            Id::SaveSyncCloud => Some(cloud),
            _ => None,
        }
    }
    /// The card beside each page: (title, text) blocks and a foot line.
    pub fn savesync_aside(
        &self,
    ) -> Option<(&'static [(&'static str, &'static str)], &'static str)> {
        match self.page() {
            Page::SaveSync => Some((
                &[
                    ("savesync.aside_how", "savesync.aside_how_detail"),
                    ("savesync.aside_account", "savesync.aside_account_detail"),
                ],
                "savesync.aside_offline",
            )),
            Page::SaveSyncTarget if self.value(Id::SaveSyncTarget) == "cloud" => Some((
                &[
                    ("savesync.aside_cloud", "savesync.aside_cloud_detail"),
                    ("savesync.aside_many", "savesync.aside_many_detail"),
                ],
                "savesync.aside_cloud_foot",
            )),
            Page::SaveSyncTarget => Some((
                &[
                    ("savesync.aside_pc", "savesync.aside_pc_detail"),
                    ("savesync.aside_many", "savesync.aside_many_detail"),
                ],
                "savesync.aside_windows",
            )),
            _ => None,
        }
    }
    /// The footer's left text: "1 of N conflicts" on the conflict page.
    pub fn savesync_note(&self) -> Option<String> {
        (self.page() == Page::SaveSyncConflict).then(|| match self.conflicts() {
            1 => "savesync.count_one".into(),
            n => format!("savesync.count|count={n}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(at: Page) -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::SaveSync);
        if at != Page::SaveSync {
            state.open(at);
        }
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }
    fn row(state: &Settings, id: Id) -> Row {
        state
            .visible_rows()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
    }
    const STATUS: &str = r#"{"enabled": true, "password": true,
        "last": {"ok": true, "at": 1795035617, "up": 2, "down": 0, "systems": ["gba"], "conflicts": 1},
        "conflicts": [{"key": "saves/gba/Pokemon Emerald.srm",
            "local": {"at": 1795035682, "size": 131072},
            "remote": {"at": 1795035600, "size": 131072, "by": "R36S Soy Sauce"}}]}"#;

    #[test]
    fn the_page_reads_the_status_once_per_visit_and_names_the_last_sync() {
        let mut state = page(Page::SaveSync);
        assert!(state.savesync_wants_status());
        state.savesync.loading = true;
        assert!(!state.savesync_wants_status());
        assert!(!ids(&state).contains(&Id::SaveSyncConflicts));
        state.savesync_done(&Action::Status, Ok(STATUS.into()));
        assert!(!state.savesync_wants_status());
        assert!(ids(&state).contains(&Id::SaveSyncConflicts));
        let subtitle = state.subtitle(&row(&state, Id::SaveSyncNow)).unwrap();
        assert!(
            matches!(
                subtitle.split('|').next(),
                Some("savesync.last_moved_today" | "savesync.last_moved_day")
            ),
            "{subtitle}"
        );
        assert!(subtitle.contains("|time="), "{subtitle}");
        assert!(subtitle.ends_with("|count=2|systems=GBA"), "{subtitle}");
        assert!(state.savesync_aside().is_some());
    }
    #[test]
    fn syncing_now_waits_for_the_reply_then_opens_the_conflict() {
        let mut state = page(Page::SaveSync);
        state.savesync_done(&Action::Status, Ok(r#"{"enabled": true}"#.into()));
        state.cursor = ids(&state)
            .iter()
            .position(|&id| id == Id::SaveSyncNow)
            .unwrap();
        assert_eq!(state.handle(Button::A), Effect::SaveSync(Action::Sync));
        assert_eq!(
            state.handle(Button::A),
            Effect::None,
            "busy until the reply"
        );
        state.savesync_done(&Action::Sync, Ok(r#"{"ok": true, "conflicts": 1}"#.into()));
        assert!(state.savesync_wants_status(), "reads the status again");
        state.savesync_done(&Action::Status, Ok(STATUS.into()));
        assert_eq!(state.page(), Page::SaveSyncConflict);
        assert_eq!(state.savesync_conflict().unwrap().game(), "Pokemon Emerald");
        assert_eq!(state.savesync_note().unwrap(), "savesync.count_one");
        let key = "saves/gba/Pokemon Emerald.srm".to_owned();
        // ‹ › moves to the folder's copy; A keeps the one in focus.
        state.handle(Button::Right);
        assert_eq!(
            state.handle(Button::A),
            Effect::SaveSync(Action::Resolve(key.clone(), false))
        );
        state.savesync_done(
            &Action::Resolve(key, false),
            Ok(r#"{"ok": true, "conflicts": 0}"#.into()),
        );
        assert_eq!(state.page(), Page::SaveSync, "none left: back");
    }
    #[test]
    fn x_takes_the_folders_copy_and_b_leaves_it_for_later() {
        let mut state = page(Page::SaveSync);
        state.savesync_done(&Action::Status, Ok(STATUS.into()));
        state.savesync_activate(Id::SaveSyncConflicts);
        assert_eq!(state.page(), Page::SaveSyncConflict);
        let effect = state.handle(Button::X);
        assert_eq!(
            effect,
            Effect::SaveSync(Action::Resolve(
                "saves/gba/Pokemon Emerald.srm".into(),
                false
            ))
        );
        state.savesync.busy = None;
        state.handle(Button::B);
        assert_eq!(state.page(), Page::SaveSync);
    }
    #[test]
    fn the_destination_is_a_choice_and_the_cloud_hides_the_share_rows() {
        let mut state = page(Page::SaveSyncTarget);
        assert_eq!(state.savesync_radio(Id::SaveSyncPc), Some(true));
        assert!(ids(&state).contains(&Id::SaveSyncPassword));
        state.cursor = 1;
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::SaveSyncTarget, "cloud".into())
        );
        state.pending = None;
        state.values.insert(Id::SaveSyncTarget, "cloud".into());
        assert_eq!(
            ids(&state),
            vec![Id::SaveSyncPc, Id::SaveSyncCloud, Id::SaveSyncTest]
        );
        assert_eq!(state.savesync_radio(Id::SaveSyncCloud), Some(true));
        // Y tests from any row.
        state.cursor = 0;
        assert_eq!(state.handle(Button::Y), Effect::SaveSync(Action::Test));
        state.savesync_done(
            &Action::Test,
            Ok(r#"{"ok": false, "error": "auth"}"#.into()),
        );
        let test = row(&state, Id::SaveSyncTest);
        assert_eq!(state.subtitle(&test).unwrap(), "savesync.error_auth");
        assert_eq!(state.savesync_value(Id::SaveSyncTest), "savesync.failed");
        assert_eq!(state.savesync_ink(Id::SaveSyncTest), Some(false));
        state.savesync_done(&Action::Test, Ok(r#"{"ok": true, "consoles": 3}"#.into()));
        assert_eq!(state.subtitle(&test).unwrap(), "savesync.test_ok|count=3");
        assert_eq!(state.savesync_value(Id::SaveSyncTest), "savesync.ok");
    }
    #[test]
    fn the_password_is_typed_in_secret_and_never_logged() {
        let mut state = page(Page::SaveSyncTarget);
        state.cursor = ids(&state)
            .iter()
            .position(|&id| id == Id::SaveSyncPassword)
            .unwrap();
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Keyboard);
        assert_eq!(state.keyboard.as_ref().unwrap().mode, Mode::Secret);
        state.handle(Button::A);
        let effect = state.handle(Button::Start);
        assert_eq!(effect, Effect::SaveSync(Action::Password("q".into())));
        assert!(!format!("{effect:?}").contains("\"q\""));
        assert_eq!(
            Action::Password("q".into()).command(),
            (vec!["password".to_owned()], Some("q".to_owned()))
        );
    }
    #[test]
    fn a_conflict_names_its_game_and_system() {
        let c = Conflict {
            key: "saves/gba/sub/Emerald.srm".into(),
            ..Conflict::default()
        };
        assert_eq!((c.game(), c.system()), ("Emerald", "gba"));
        let c = Conflict {
            key: "psp/SAVEDATA/ULUS10041/DATA.BIN".into(),
            ..Conflict::default()
        };
        assert_eq!(c.system(), "psp");
    }
}
