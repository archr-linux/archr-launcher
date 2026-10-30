//! Storage page: both cards, mount options, backup, root password and reset.
//! Every action that changes data asks first; the host runs it off SDL's thread.
use super::{described, Effect, Id, Kind, Row, Settings};
use crate::{state::Button, HintButton};

pub const STORAGE: &[Row] = &[
    // Shown only while unified storage joins the two cards.
    described(Id::UnifiedCard, "settings.merged", "")
        .kind(Kind::Action)
        .under("settings.games_card"),
    described(Id::StorageCard, "settings.card_internal", "")
        .kind(Kind::Action)
        .under("settings.games_card"),
    // Shown only while a second card is mounted.
    described(Id::SecondCard, "settings.card_microsd", "").kind(Kind::Action),
    // Shown only where the card can have, or has, the ROMs partition.
    described(Id::RomsLink, "roms.title", "").kind(Kind::Action),
    described(
        Id::Automount,
        "settings.automount",
        "settings.automount_detail",
    )
    .kind(Kind::Toggle)
    .under("settings.options"),
    described(
        Id::MergedStorage,
        "settings.merged",
        "settings.merged_detail",
    )
    .kind(Kind::Toggle),
    described(Id::Backup, "settings.backup", "settings.backup_detail")
        .kind(Kind::Action)
        .under("settings.data"),
    described(
        Id::RestoreBackup,
        "settings.restore_backup",
        "settings.restore_none",
    )
    .kind(Kind::Action),
    described(
        Id::FactoryReset,
        "settings.factory_reset",
        "settings.factory_detail",
    )
    .kind(Kind::Action),
];

/// A card as the host found it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    /// Catalog reference for the row title ("microSD (ROMS)").
    pub name: String,
    /// Catalog reference: filesystem and usage.
    pub detail: String,
    pub percent: u8,
}
#[derive(Clone, Debug, Default)]
pub struct Storage {
    /// The system card (/storage).
    pub card: Option<Card>,
    /// The second card (/storage/games-external); only it can be ejected.
    pub second: Option<Card>,
    /// Both cards as one /storage/roms: their sizes and usage summed.
    pub unified: Option<Card>,
    /// Subtitle reference naming the newest backup's time, if one exists.
    pub backup: Option<String>,
    /// root.password as setrootpass saved it; shown so it can be used.
    pub password: String,
    /// The ROMs partition: moved to it, being moved, or not yet.
    pub roms: super::roms::Status,
    pub confirm: Option<Confirm>,
    /// Locale key of the running action; input waits for the host's reply.
    pub working: Option<&'static str>,
}
/// Work the host runs for this page. `Debug` never shows the password.
#[derive(Clone, PartialEq, Eq)]
pub enum Action {
    Eject,
    Backup {
        wifi: bool,
    },
    Restore {
        wifi: bool,
    },
    RootPassword(String),
    FactoryReset,
    /// archr-roms-migrate: arm the move for the next boot, disarm it, or
    /// copy again after it stalled.
    /// `files`: the previewed count, for the notice after the move.
    RomsStart {
        files: u64,
    },
    RomsCancel,
    RomsRetry,
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eject => write!(f, "Eject"),
            Self::Backup { wifi } => write!(f, "Backup {{ wifi: {wifi} }}"),
            Self::Restore { wifi } => write!(f, "Restore {{ wifi: {wifi} }}"),
            Self::RootPassword(_) => write!(f, "RootPassword(<hidden>)"),
            Self::FactoryReset => write!(f, "FactoryReset"),
            Self::RomsStart { files } => write!(f, "RomsStart {{ files: {files} }}"),
            Self::RomsCancel => write!(f, "RomsCancel"),
            Self::RomsRetry => write!(f, "RomsRetry"),
        }
    }
}
impl Action {
    /// Locale key shown while the host works.
    pub fn progress(&self) -> &'static str {
        match self {
            Self::Eject => "settings.ejecting",
            Self::Backup { .. } => "settings.backing_up",
            Self::Restore { .. } => "settings.restoring",
            Self::RootPassword(_) => "settings.saving_password",
            Self::FactoryReset => "settings.resetting",
            Self::RomsStart { .. } | Self::RomsRetry => "roms.arming",
            Self::RomsCancel => "roms.cancelling",
        }
    }
}
/// A question on screen. The typed password lives here until confirmed.
#[derive(Clone, PartialEq, Eq)]
pub enum Confirm {
    Eject,
    /// Turning unified storage off.
    Separate,
    Backup,
    BackupWifi,
    Restore,
    RootPassword(String),
    Reset,
    ResetFinal,
    /// Moving the ROMs to the exFAT partition: restarts the device.
    RomsMove {
        files: u64,
    },
}
impl std::fmt::Debug for Confirm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootPassword(_) => write!(f, "RootPassword(<hidden>)"),
            Self::Eject => write!(f, "Eject"),
            Self::Separate => write!(f, "Separate"),
            Self::Backup => write!(f, "Backup"),
            Self::BackupWifi => write!(f, "BackupWifi"),
            Self::Restore => write!(f, "Restore"),
            Self::Reset => write!(f, "Reset"),
            Self::ResetFinal => write!(f, "ResetFinal"),
            Self::RomsMove { files } => write!(f, "RomsMove {{ files: {files} }}"),
        }
    }
}
type Hints = &'static [(HintButton, &'static str)];
impl Confirm {
    /// Question, explanation and the buttons that answer it.
    pub fn dialog(&self) -> (&'static str, &'static str, Hints) {
        use HintButton::Key;
        const CANCEL: (HintButton, &str) = (Key('B'), "hint.cancel");
        match self {
            Self::Eject => (
                "settings.eject_question",
                "settings.eject_explain",
                &[(Key('A'), "settings.eject"), CANCEL],
            ),
            Self::Separate => (
                "settings.separate_question",
                "settings.separate_explain",
                &[(Key('A'), "settings.separate"), CANCEL],
            ),
            Self::Backup => (
                "settings.backup_question",
                "settings.backup_explain",
                &[
                    (Key('A'), "settings.backup"),
                    (Key('X'), "settings.include_wifi"),
                    CANCEL,
                ],
            ),
            Self::BackupWifi => (
                "settings.wifi_backup_question",
                "settings.wifi_backup_explain",
                &[(Key('A'), "settings.include_wifi"), CANCEL],
            ),
            Self::Restore => (
                "settings.restore_question",
                "settings.restore_explain",
                &[
                    (Key('A'), "settings.restore_short"),
                    (Key('X'), "settings.restore_wifi"),
                    CANCEL,
                ],
            ),
            Self::RootPassword(_) => (
                "settings.password_question",
                "settings.password_explain",
                &[(Key('A'), "settings.change"), CANCEL],
            ),
            Self::Reset => (
                "settings.reset_question",
                "settings.reset_explain",
                &[(Key('A'), "settings.continue"), CANCEL],
            ),
            // A different button: a doubled A press never erases anything.
            Self::ResetFinal => (
                "settings.reset_final_question",
                "settings.reset_final_explain",
                &[(Key('X'), "settings.erase"), CANCEL],
            ),
            Self::RomsMove { .. } => (
                "roms.move_question",
                "roms.move_explain",
                &[(Key('A'), "roms.move_short"), CANCEL],
            ),
        }
    }
    /// The answer to a button; None leaves the question up.
    fn answer(self, button: Button) -> Option<Answer> {
        Some(match (self, button) {
            (_, Button::B) => Answer::Cancel,
            (Self::Eject, Button::A) => Answer::Run(Action::Eject),
            (Self::Separate, Button::A) => Answer::Set(Id::MergedStorage, "0"),
            (Self::Backup, Button::A) => Answer::Run(Action::Backup { wifi: false }),
            (Self::Backup, Button::X) => Answer::Ask(Self::BackupWifi),
            (Self::BackupWifi, Button::A) => Answer::Run(Action::Backup { wifi: true }),
            (Self::Restore, Button::A) => Answer::Run(Action::Restore { wifi: false }),
            (Self::Restore, Button::X) => Answer::Run(Action::Restore { wifi: true }),
            (Self::RootPassword(password), Button::A) => {
                Answer::Run(Action::RootPassword(password))
            }
            (Self::Reset, Button::A) => Answer::Ask(Self::ResetFinal),
            (Self::ResetFinal, Button::X) => Answer::Run(Action::FactoryReset),
            (Self::RomsMove { files }, Button::A) => Answer::Run(Action::RomsStart { files }),
            _ => return None,
        })
    }
}
enum Answer {
    Cancel,
    Ask(Confirm),
    Run(Action),
    Set(Id, &'static str),
}
/// Characters that survive setrootpass, get_setting's unquoted echo, Samba's
/// `echo -e` and simple-http-server's `-a root:<password>` argument.
pub fn root_password_valid(password: &str) -> bool {
    (4..=64).contains(&password.len())
        && password != "default"
        && password
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.@#%+=,:!^~/".contains(&b))
}

impl Settings {
    pub(super) fn storage_input(&mut self, button: Button) -> Option<Effect> {
        if self.storage.working.is_some() {
            return Some(Effect::None);
        }
        let confirm = self.storage.confirm.take()?;
        Some(match confirm.clone().answer(button) {
            None => {
                self.storage.confirm = Some(confirm);
                Effect::None
            }
            Some(Answer::Cancel) => Effect::None,
            Some(Answer::Ask(next)) => {
                self.storage.confirm = Some(next);
                Effect::None
            }
            Some(Answer::Run(action)) => {
                self.storage.working = Some(action.progress());
                Effect::Storage(action)
            }
            Some(Answer::Set(id, value)) => {
                self.pending = Some(id);
                Effect::Set(id, value.into())
            }
        })
    }
    /// Turning unified storage off asks first; turning it on does not.
    pub(super) fn storage_switch(&mut self, id: Id) -> Option<Effect> {
        (id == Id::MergedStorage && self.value(id) == "1").then(|| {
            self.storage.confirm = Some(Confirm::Separate);
            Effect::None
        })
    }
    /// A on a storage row: every action asks before it runs.
    pub(super) fn storage_activate(&mut self, id: Id) -> Option<Effect> {
        if id.roms() {
            return self.roms_activate(id);
        }
        self.storage.confirm = Some(match id {
            Id::SecondCard if self.storage.second.is_some() => Confirm::Eject,
            Id::Backup => Confirm::Backup,
            Id::RestoreBackup => Confirm::Restore,
            Id::FactoryReset => Confirm::Reset,
            _ => return None,
        });
        Some(Effect::None)
    }
    /// The typed password is only held for the confirmation question.
    pub(super) fn root_password_typed(&mut self, password: String) {
        self.storage.confirm = Some(Confirm::RootPassword(password));
    }
    /// Settles a storage action. Ok carries a message key or a reboot question.
    pub fn storage_done(&mut self, result: Result<&'static str, String>) {
        self.storage.working = None;
        // ROMs on the PC looks again at what the action changed.
        self.roms.scan = None;
        match result {
            Ok(key) if key.ends_with("_reboot_question") => self.reboot_prompt = Some(key),
            Ok(key) => self.fail(None, key.into()),
            Err(error) => self.fail(None, error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Page;
    fn storage() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Storage);
        state.storage.card = Some(Card::default());
        state.storage.second = Some(Card::default());
        state.storage.backup = Some("settings.backup_today|time=14:42".into());
        state
    }
    fn at(state: &mut Settings, id: Id) {
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == id)
            .unwrap();
    }
    #[test]
    fn wifi_goes_into_a_backup_only_after_the_unencrypted_warning() {
        let mut state = storage();
        at(&mut state, Id::Backup);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.storage.confirm, Some(Confirm::Backup));
        assert_eq!(state.handle(Button::X), Effect::None);
        assert_eq!(state.storage.confirm, Some(Confirm::BackupWifi));
        assert_eq!(
            state.storage.confirm.as_ref().unwrap().dialog().1,
            "settings.wifi_backup_explain"
        );
        assert_eq!(
            state.handle(Button::A),
            Effect::Storage(Action::Backup { wifi: true })
        );
        assert_eq!(state.storage.working, Some("settings.backing_up"));
        // Input waits for the reply.
        assert_eq!(state.handle(Button::B), Effect::None);
        assert_eq!(state.page(), Page::Storage);
        state.storage_done(Ok("settings.backup_done"));
        assert!(state.storage.working.is_none());
        at(&mut state, Id::Backup);
        state.handle(Button::A);
        assert_eq!(
            state.handle(Button::A),
            Effect::Storage(Action::Backup { wifi: false })
        );
    }
    #[test]
    fn factory_reset_needs_two_different_confirmations() {
        let mut state = storage();
        at(&mut state, Id::FactoryReset);
        state.handle(Button::A);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.storage.confirm, Some(Confirm::ResetFinal));
        // A doubled A press does not erase; B cancels.
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.handle(Button::B), Effect::None);
        assert!(state.storage.confirm.is_none());
        state.handle(Button::A);
        state.handle(Button::A);
        assert_eq!(
            state.handle(Button::X),
            Effect::Storage(Action::FactoryReset)
        );
    }
    #[test]
    fn root_password_charset_is_checked() {
        for bad in [
            "abc",
            "has space",
            "back\\slash",
            "star*",
            "default",
            "pé12",
        ] {
            assert!(!root_password_valid(bad), "{bad}");
        }
        assert!(root_password_valid("Archr-2026#ok"));
    }
    #[test]
    fn eject_only_for_the_second_card_and_restore_only_with_a_backup() {
        let mut state = storage();
        at(&mut state, Id::SecondCard);
        state.handle(Button::A);
        assert_eq!(state.storage.confirm, Some(Confirm::Eject));
        assert_eq!(state.handle(Button::A), Effect::Storage(Action::Eject));
        state.storage_done(Err("settings.eject_busy".into()));
        // Ejected: its row goes away.
        state.storage.second = None;
        assert!(!state.visible_rows().iter().any(|r| r.id == Id::SecondCard));
        state.storage.backup = None;
        for id in [Id::StorageCard, Id::RestoreBackup] {
            at(&mut state, id);
            assert_eq!(state.handle(Button::A), Effect::None);
            assert!(state.storage.confirm.is_none());
            assert_eq!(state.hints().len(), 1);
        }
    }
    #[test]
    fn unified_storage_asks_before_separating_and_applies_at_once() {
        let mut state = storage();
        at(&mut state, Id::MergedStorage);
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::MergedStorage, "1".into())
        );
        state.complete(Id::MergedStorage, Ok("1".into()));
        assert_eq!(state.reboot_prompt, None, "applied without a reboot");
        // Off: a question first, for every button that flips it.
        for button in [Button::A, Button::L, Button::R] {
            assert_eq!(state.handle(button), Effect::None);
            assert_eq!(state.storage.confirm, Some(Confirm::Separate));
            assert_eq!(state.handle(Button::B), Effect::None);
            assert_eq!(state.value(Id::MergedStorage), "1");
        }
        state.handle(Button::A);
        assert_eq!(
            state.handle(Button::A),
            Effect::Set(Id::MergedStorage, "0".into())
        );
        assert_eq!(state.pending, Some(Id::MergedStorage));
        // The union's row is there only while the cards are joined.
        assert!(!state.visible_rows().iter().any(|r| r.id == Id::UnifiedCard));
        state.storage.unified = Some(Card::default());
        assert_eq!(state.visible_rows()[0].id, Id::UnifiedCard);
        assert!(state.fixed(Id::UnifiedCard));
    }
    #[test]
    fn restore_and_mount_options_ask_to_reboot() {
        let mut state = storage();
        state.storage_done(Ok("settings.restore_reboot_question"));
        assert_eq!(
            state.reboot_prompt,
            Some("settings.restore_reboot_question")
        );
        state.handle(Button::B);
        at(&mut state, Id::Automount);
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::Automount, "0".into())
        );
        state.complete(Id::Automount, Ok("0".into()));
        assert_eq!(
            state.reboot_prompt,
            Some("settings.storage_reboot_question")
        );
    }
}
