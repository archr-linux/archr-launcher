//! Achievements page (Config-ra): the RetroAchievements account and the
//! options setsettings.sh hands to RetroArch. Only the user name and the
//! token are ever stored; the password goes straight to the sign-in.
use super::{described, row, Effect, Id, Kind, Row, Settings};
use crate::keyboard::Mode;

pub const CHEEVOS: &[Row] = &[
    row(Id::Cheevos, "settings.cheevos", Kind::Toggle),
    described(
        Id::CheevosAccount,
        "settings.cheevos_account",
        "settings.cheevos_account_detail",
    )
    .kind(Kind::Action),
    described(
        Id::CheevosHardcore,
        "settings.cheevos_hardcore",
        "settings.cheevos_hardcore_detail",
    )
    .kind(Kind::Toggle),
    row(Id::CheevosVerbose, "settings.cheevos_verbose", Kind::Toggle),
    described(
        Id::CheevosEncore,
        "settings.cheevos_encore",
        "settings.cheevos_encore_detail",
    )
    .kind(Kind::Toggle),
    described(
        Id::CheevosRichPresence,
        "settings.cheevos_presence",
        "settings.cheevos_presence_detail",
    )
    .kind(Kind::Toggle),
    row(
        Id::CheevosScreenshot,
        "settings.cheevos_screenshot",
        Kind::Toggle,
    ),
    row(Id::CheevosSound, "settings.cheevos_sound", Kind::Value),
    row(Id::CheevosLogout, "settings.cheevos_logout", Kind::Action),
];

/// Account facts for the profile card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub points: u64,
    pub softcore: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Cheevos {
    /// The signed-in user (a token is saved); empty when signed out.
    pub user: String,
    /// User name typed for a sign-in that has not been answered yet.
    pub draft: String,
    /// A sign-in or sign-out is running; the page waits for it.
    pub verifying: bool,
    pub profile: Option<Profile>,
}

impl Settings {
    pub(super) fn cheevos_visible(&self, id: Id) -> bool {
        let signed_in = !self.cheevos.user.is_empty();
        match id {
            Id::CheevosAccount => !signed_in,
            Id::CheevosLogout => signed_in,
            _ => true,
        }
    }
    pub(super) fn cheevos_activate(&mut self, id: Id) -> Option<Effect> {
        if self.cheevos.verifying {
            return Some(Effect::None);
        }
        match id {
            Id::CheevosAccount => {
                let user = std::mem::take(&mut self.cheevos.draft);
                self.open_keyboard(Id::CheevosAccount, Mode::Text, user);
                Some(Effect::None)
            }
            Id::CheevosLogout => {
                self.cheevos.verifying = true;
                Some(Effect::CheevosLogout)
            }
            _ => None,
        }
    }
    /// A typed user name asks for the password; the password signs in.
    pub(super) fn cheevos_typed(&mut self, id: Id, value: String) -> Effect {
        if id == Id::CheevosAccount {
            self.cheevos.draft = value.trim().to_owned();
            self.open_keyboard(Id::CheevosPassword, Mode::Secret, String::new());
            return Effect::None;
        }
        self.cheevos.verifying = true;
        Effect::CheevosLogin(self.cheevos.draft.clone(), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{settings::Page, state::Button};
    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Achievements);
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }
    #[test]
    fn signing_in_asks_the_user_then_a_secret_password() {
        let mut state = page();
        assert!(ids(&state).contains(&Id::CheevosAccount));
        assert!(!ids(&state).contains(&Id::CheevosLogout));
        state.cursor = 1;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::Keyboard);
        state.handle(Button::A);
        state.handle(Button::Start);
        assert_eq!(state.keyboard.as_ref().unwrap().mode, Mode::Secret);
        assert_eq!(state.keyboard_target, Some(Id::CheevosPassword));
        state.handle(Button::A);
        let effect = state.handle(Button::Start);
        assert_eq!(effect, Effect::CheevosLogin("q".into(), "q".into()));
        assert!(!format!("{effect:?}").contains("\"q\")"));
        assert!(state.cheevos.verifying);
        assert_eq!(state.page(), Page::Achievements);
        // Nothing else starts while the service answers.
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(
            state.subtitle(state.visible_rows()[1]).as_deref(),
            Some("settings.verifying")
        );
    }
    #[test]
    fn a_signed_in_page_offers_signing_out_instead() {
        let mut state = page();
        state.cheevos.user = "douglas".into();
        assert!(!ids(&state).contains(&Id::CheevosAccount));
        state.cursor = ids(&state).len() - 1;
        assert_eq!(state.handle(Button::A), Effect::CheevosLogout);
        assert!(state.cheevos.verifying);
        state.cursor = 1;
        assert_eq!(state.visible_rows()[1].id, Id::CheevosHardcore);
        assert_eq!(
            state.handle(Button::R),
            Effect::None,
            "busy until the reply"
        );
        state.cheevos.verifying = false;
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::CheevosHardcore, "1".into())
        );
    }
    #[test]
    fn options_use_the_keys_setsettings_reads() {
        assert_eq!(Id::Cheevos.key(), "global.retroachievements");
        assert_eq!(
            Id::CheevosHardcore.key(),
            "global.retroachievements.hardcore"
        );
        assert_eq!(Id::CheevosEncore.key(), "global.retroachievements.encore");
        assert_eq!(Id::CheevosSound.default_value(), "none");
    }
}
