//! Achievements page effects: sign-in and sign-out through the
//! RetroAchievements API, the account card, and the options setsettings.sh
//! hands to RetroArch. system.cfg keeps the user name and the token only.
use super::*;
use archr_cheevos::{Account, Client};
use archr_ui::settings::cheevos::Profile;

pub const USER: &str = "global.retroachievements.username";
pub const TOKEN: &str = "global.retroachievements.token";
const PASSWORD: &str = "global.retroachievements.password";
/// Where RetroArch looks for unlock sounds (retroarch patch 0011).
const SOUNDS: [&str; 2] = [
    "/usr/share/libretro/sounds",
    "/storage/roms/music/retroachievements",
];

fn sound_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_ ".contains(&b))
}

fn profile(client: &Client, account: &Account, cancel: &AtomicBool) -> Profile {
    let summary = client.summary(&account.user, cancel).unwrap_or_default();
    Profile {
        points: account.points,
        softcore: account.softcore,
        since: summary.since,
        last_game: summary.last_game,
    }
}

impl Backend {
    fn cheevos_client(&self) -> Client {
        Client::production(self.paths.cache.join("cheevos-tmp"))
    }
    /// Unlock sounds installed for RetroArch, by name, after "none".
    fn sounds(&self) -> Vec<String> {
        let mut sounds: Vec<String> = SOUNDS
            .iter()
            .filter_map(|dir| fs::read_dir(self.paths.at(dir)).ok())
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension()? == "ogg").then_some(())?;
                Some(path.file_stem()?.to_str()?.to_owned())
            })
            .filter(|name| sound_name(name))
            .collect();
        sounds.sort();
        sounds.dedup();
        sounds.insert(0, "none".into());
        sounds
    }
    pub(super) fn load_cheevos(&self, cfg: &SystemCfg, state: &mut Settings) {
        state.cheevos.user = cfg
            .get_setting(TOKEN, None, None)
            .and_then(|_| cfg.get_setting(USER, None, None))
            .unwrap_or_default();
        let mut sounds = self.sounds();
        let current = state.value(Id::CheevosSound).to_owned();
        if !sounds.contains(&current) {
            sounds.push(current);
        }
        state.options.insert(Id::CheevosSound, sounds);
    }
    pub(super) fn apply_cheevos(&self, id: Id, value: &str) -> Option<Result<String, String>> {
        let valid = match id {
            Id::Cheevos
            | Id::CheevosHardcore
            | Id::CheevosVerbose
            | Id::CheevosEncore
            | Id::CheevosRichPresence
            | Id::CheevosScreenshot => id.choices().contains(&value),
            Id::CheevosSound => self.sounds().iter().any(|s| s == value),
            _ => return None,
        };
        Some(if valid {
            self.save(id, value).map(|()| value.into())
        } else {
            Err("Invalid option".into())
        })
    }
    /// Trades the password for a token; only the token and user are saved.
    pub(super) fn cheevos_login(
        &self,
        user: &str,
        password: &str,
    ) -> Result<(String, Profile), String> {
        self.sign_in(&self.cheevos_client(), user, password)
    }
    fn sign_in(
        &self,
        client: &Client,
        user: &str,
        password: &str,
    ) -> Result<(String, Profile), String> {
        let user = user.trim();
        if user.is_empty() || password.is_empty() || user.len() > 32 {
            return Err("cheevos.bad_login".into());
        }
        let never = AtomicBool::new(false);
        let account = client
            .login(user, password, &never)
            .map_err(|e| e.key().to_owned())?;
        // A system.cfg value is one line of plain text.
        let plain = |text: &str| text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if !plain(&account.user) || !plain(&account.token) {
            return Err("cheevos.service".into());
        }
        SystemCfg::set_many_and_save(
            &self.paths.config,
            &self.paths.lock,
            &[
                (USER, &account.user),
                (TOKEN, &account.token),
                (PASSWORD, ""),
                (Id::Cheevos.key(), "1"),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok((account.user.clone(), profile(client, &account, &never)))
    }
    /// Forgets the token (and any password an older setup saved).
    pub(super) fn cheevos_logout(&self) -> Result<(), String> {
        SystemCfg::set_many_and_save(
            &self.paths.config,
            &self.paths.lock,
            &[(TOKEN, ""), (PASSWORD, "")],
        )
        .map_err(|e| e.to_string())
    }
    /// Current points of the saved account; `None` offline or signed out.
    pub(super) fn cheevos_profile(&self) -> Option<Profile> {
        let cfg = self.cfg().ok()?;
        let (user, token) = (
            cfg.get_setting(USER, None, None)?,
            cfg.get_setting(TOKEN, None, None)?,
        );
        let client = self.cheevos_client();
        let never = AtomicBool::new(false);
        match client.refresh(&user, &token, &never) {
            Ok(account) => Some(profile(&client, &account, &never)),
            Err(error) => {
                log::warn!("RetroAchievements profile: {}", error.key());
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archr_scraper::{Http, NetError, Response};
    struct Fake(&'static str);
    impl Http for Fake {
        fn get(&self, _: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
            Err(NetError::Failed)
        }
        fn post(&self, _: &str, _: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
            Ok(Response {
                status: 200,
                body: self.0.as_bytes().to_vec(),
            })
        }
    }
    fn backend() -> (tempfile::TempDir, Backend) {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
        fs::write(
            &paths.config,
            "global.retroachievements=0\nglobal.retroachievements.password=old\n",
        )
        .unwrap();
        (dir, Backend::new(paths))
    }
    #[test]
    fn signing_in_saves_the_token_never_the_password_and_signing_out_forgets_it() {
        let (_dir, backend) = backend();
        let client = Client::new(Box::new(Fake(
            r#"{"Success":true,"User":"Douglas","Token":"abc123","Score":7,"SoftcoreScore":2}"#,
        )));
        let (user, profile) = backend.sign_in(&client, "douglas", "s3cret!").unwrap();
        assert_eq!(
            (user.as_str(), profile.points, profile.softcore),
            ("Douglas", 7, 2)
        );
        let saved = fs::read_to_string(&backend.paths.config).unwrap();
        assert!(saved.contains("global.retroachievements=1\n"), "{saved}");
        assert!(saved.contains("global.retroachievements.username=Douglas\n"));
        assert!(saved.contains("global.retroachievements.token=abc123\n"));
        assert!(
            !saved.contains("s3cret") && !saved.contains("=old"),
            "{saved}"
        );
        let mut state = Settings::default();
        backend.load_cheevos(&backend.cfg().unwrap(), &mut state);
        assert_eq!(state.cheevos.user, "Douglas");
        backend.cheevos_logout().unwrap();
        let saved = fs::read_to_string(&backend.paths.config).unwrap();
        assert!(!saved.contains("abc123"), "{saved}");
        let mut state = Settings::default();
        backend.load_cheevos(&backend.cfg().unwrap(), &mut state);
        assert_eq!(state.cheevos.user, "");
    }
    #[test]
    fn a_refused_sign_in_saves_nothing() {
        let (_dir, backend) = backend();
        let client = Client::new(Box::new(Fake(
            r#"{"Success":false,"Code":"invalid_credentials","Error":"Invalid"}"#,
        )));
        assert_eq!(
            backend.sign_in(&client, "douglas", "wrong"),
            Err("cheevos.bad_login".into())
        );
        assert_eq!(
            backend.sign_in(&client, " ", "x"),
            Err("cheevos.bad_login".into())
        );
        // A token that is not plain text never reaches system.cfg.
        let client = Client::new(Box::new(Fake(
            r#"{"Success":true,"User":"douglas","Token":"a\nb=c"}"#,
        )));
        assert_eq!(
            backend.sign_in(&client, "douglas", "x"),
            Err("cheevos.service".into())
        );
        let saved = fs::read_to_string(&backend.paths.config).unwrap();
        assert!(saved.contains("global.retroachievements=0\n"));
    }
    #[test]
    fn unlock_sounds_come_from_retroarchs_folders_and_are_validated() {
        let (dir, backend) = backend();
        let sounds = dir.path().join("usr/share/libretro/sounds");
        fs::create_dir_all(&sounds).unwrap();
        for name in ["unlock.ogg", "zelda.ogg", "notes.txt", "bad;name.ogg"] {
            fs::write(sounds.join(name), b"").unwrap();
        }
        assert_eq!(backend.sounds(), ["none", "unlock", "zelda"]);
        assert_eq!(
            backend.apply_cheevos(Id::CheevosSound, "zelda"),
            Some(Ok("zelda".into()))
        );
        assert!(matches!(
            backend.apply_cheevos(Id::CheevosSound, "../x"),
            Some(Err(_))
        ));
        assert!(matches!(
            backend.apply_cheevos(Id::CheevosHardcore, "2"),
            Some(Err(_))
        ));
        assert_eq!(
            backend.apply_cheevos(Id::CheevosHardcore, "1"),
            Some(Ok("1".into()))
        );
        assert_eq!(backend.apply_cheevos(Id::Volume, "1"), None);
        let saved = fs::read_to_string(&backend.paths.config).unwrap();
        assert!(saved.contains("global.retroachievements.sound=zelda\n"));
        assert!(saved.contains("global.retroachievements.hardcore=1\n"));
    }
}
