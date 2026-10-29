//! Achievements page effects: sign-in and sign-out through the
//! RetroAchievements API, the account card, and the options setsettings.sh
//! hands to RetroArch. system.cfg keeps the user name and the token only.
//!
//! Nothing here forgets the account on a failure: offline, timeouts and
//! service errors leave every file as it is, and even a refused token only
//! asks for the password again (the page's `expired`). Only "Sign out"
//! clears the token and the web API key.
use super::*;
use archr_cheevos::{Account, Client, Error, Summary};
use archr_io::{publish_private, read_utf8, remove_durable, Symlinks::Reject};
use archr_ui::settings::cheevos::Profile;

pub const USER: &str = "global.retroachievements.username";
pub const TOKEN: &str = "global.retroachievements.token";
const PASSWORD: &str = "global.retroachievements.password";
/// The user's web API key, in its own 0600 file in the launcher store and
/// not in system.cfg: backuptool zips system.cfg onto the ROMs share, and
/// modules may be granted system.cfg keys. Neither sees this file.
const WEB_KEY: &str = "cheevos-web-key";
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

fn profile(account: &Account, summary: Summary) -> Profile {
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
    /// The saved user and token; read from disk only, never the network.
    fn cheevos_account(cfg: &SystemCfg) -> Option<(String, String)> {
        Some((
            cfg.get_setting(USER, None, None)?,
            cfg.get_setting(TOKEN, None, None)?,
        ))
    }
    /// A symbolic link in its place is not followed: the key is ours alone.
    fn web_key(&self) -> Option<String> {
        read_utf8(&self.paths.store.join(WEB_KEY), 256, Reject)
            .ok()
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
    }
    /// Member-since and last game when a web API key is saved; whether the
    /// service refused the key. Failures never touch the saved key.
    fn summary(&self, client: &Client, user: &str) -> (Summary, bool) {
        let Some(key) = self.web_key() else {
            return (Summary::default(), false);
        };
        match client.summary(user, &key, &AtomicBool::new(false)) {
            Ok(summary) => (summary, false),
            Err(error) => {
                log::warn!("RetroAchievements summary: {}", error.key());
                (Summary::default(), error == Error::Auth)
            }
        }
    }
    pub(super) fn load_cheevos(&self, cfg: &SystemCfg, state: &mut Settings) {
        state.cheevos.user = Self::cheevos_account(cfg)
            .map(|(user, _)| user)
            .unwrap_or_default();
        state.cheevos.web_key = self.web_key().is_some();
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
        let (summary, _) = self.summary(client, &account.user);
        Ok((account.user.clone(), profile(&account, summary)))
    }
    /// Forgets the token, the web API key (and any password an older setup
    /// saved). The user name stays for the next sign-in. The key goes first:
    /// a failure then leaves the account fully signed in, never half.
    pub(super) fn cheevos_logout(&self) -> Result<(), String> {
        remove_durable(&self.paths.store.join(WEB_KEY))
            .and_then(|()| {
                SystemCfg::set_many_and_save(
                    &self.paths.config,
                    &self.paths.lock,
                    &[(TOKEN, ""), (PASSWORD, "")],
                )
            })
            .map_err(|e| e.to_string())
    }
    /// Checks a web API key with the service and saves it only once it is
    /// accepted (`Some`: the card's extras); an empty key removes it (`None`).
    pub(super) fn cheevos_key(&self, key: &str) -> Result<Option<Summary>, String> {
        self.save_key(&self.cheevos_client(), key)
    }
    fn save_key(&self, client: &Client, key: &str) -> Result<Option<Summary>, String> {
        let path = self.paths.store.join(WEB_KEY);
        if key.is_empty() {
            return remove_durable(&path)
                .map(|()| None)
                .map_err(|e| e.to_string());
        }
        if key.len() > 64 || !key.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("cheevos.bad_web_key".into());
        }
        let cfg = self.cfg().map_err(|e| e.to_string())?;
        let (user, _) = Self::cheevos_account(&cfg).ok_or("cheevos.sign_in_first")?;
        let summary = client
            .summary(&user, key, &AtomicBool::new(false))
            .map_err(|e| match e {
                Error::Auth => "cheevos.bad_web_key".to_owned(),
                e => e.key().to_owned(),
            })?;
        publish_private(&path, key.as_bytes()).map_err(|e| e.to_string())?;
        Ok(Some(summary))
    }
    /// The saved account's card, and whether its web API key was refused.
    /// `Error::Auth` means the token itself was refused: the page asks for
    /// the password again; nothing is cleared.
    pub(super) fn cheevos_profile(&self) -> Result<(Profile, bool), Error> {
        self.fetch_profile(&self.cheevos_client())
    }
    fn fetch_profile(&self, client: &Client) -> Result<(Profile, bool), Error> {
        let cfg = self.cfg().map_err(|_| Error::Service)?;
        let (user, token) = Self::cheevos_account(&cfg).ok_or(Error::Service)?;
        let account = client
            .refresh(&user, &token, &AtomicBool::new(false))
            .inspect_err(|e| log::warn!("RetroAchievements profile: {}", e.key()))?;
        let (summary, refused) = self.summary(client, &account.user);
        Ok((profile(&account, summary), refused))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archr_scraper::{Http, NetError, Response};
    use std::sync::{Arc, Mutex};
    /// One canned answer per method (`None`: no network), recording URLs.
    #[derive(Clone, Default)]
    struct Service {
        post: Option<(u16, &'static str)>,
        get: Option<(u16, &'static str)>,
        urls: Arc<Mutex<Vec<String>>>,
    }
    impl Http for Service {
        fn get(&self, url: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
            self.urls.lock().unwrap().push(url.into());
            let (status, body) = self.get.ok_or(NetError::Timeout)?;
            Ok(Response {
                status,
                body: body.into(),
            })
        }
        fn post(&self, _: &str, _: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
            let (status, body) = self.post.ok_or(NetError::Timeout)?;
            Ok(Response {
                status,
                body: body.into(),
            })
        }
    }
    const SIGNED: &str = r#"{"Success":true,"User":"Douglas","Token":"abc123","Score":7}"#;
    const SUMMARY: &str =
        r#"{"User":"Douglas","MemberSince":"2021-03-04","RecentlyPlayed":[{"Title":"Zelda"}]}"#;
    /// The sign-in answers `post`; the web API is offline.
    fn answer(post: &'static str) -> Client {
        client(&Service {
            post: Some((200, post)),
            ..Default::default()
        })
    }
    fn client(service: &Service) -> Client {
        Client::new(Box::new(service.clone()))
    }
    /// A fresh backend over the same files: what a reboot sees.
    fn reboot(dir: &tempfile::TempDir) -> (Backend, Settings) {
        let backend = Backend::new(crate::paths::Paths::new(Some(dir.path().into())));
        let mut state = Settings::default();
        backend.load_cheevos(&backend.cfg().unwrap(), &mut state);
        (backend, state)
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
        let client = answer(
            r#"{"Success":true,"User":"Douglas","Token":"abc123","Score":7,"SoftcoreScore":2}"#,
        );
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
        let client = answer(r#"{"Success":false,"Code":"invalid_credentials","Error":"Invalid"}"#);
        assert_eq!(
            backend.sign_in(&client, "douglas", "wrong"),
            Err("cheevos.bad_login".into())
        );
        assert_eq!(
            backend.sign_in(&client, " ", "x"),
            Err("cheevos.bad_login".into())
        );
        // A token that is not plain text never reaches system.cfg.
        let client = answer(r#"{"Success":true,"User":"douglas","Token":"a\nb=c"}"#);
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
    #[test]
    fn the_account_and_key_survive_a_reboot_and_every_kind_of_failure() {
        let (dir, backend) = backend();
        let online = Service {
            post: Some((200, SIGNED)),
            get: Some((200, SUMMARY)),
            ..Default::default()
        };
        backend.sign_in(&client(&online), "douglas", "pw").unwrap();
        let key = backend.save_key(&client(&online), "k3y").unwrap().unwrap();
        assert_eq!(
            (key.since.as_str(), key.last_game.as_str()),
            ("2021", "Zelda")
        );
        let file = dir
            .path()
            .join("storage/.config/archr-launcher/cheevos-web-key");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Neither system.cfg (zipped by backuptool) nor a log holds the key.
        assert!(!fs::read_to_string(&backend.paths.config)
            .unwrap()
            .contains("k3y"));
        let before = (
            fs::read(&backend.paths.config).unwrap(),
            fs::read(&file).unwrap(),
        );
        let failures = [
            Service::default(),
            Service {
                post: Some((503, "<html>down</html>")),
                ..Default::default()
            },
            Service {
                post: Some((200, r#"{"Success":false,"Code":"invalid_credentials"}"#)),
                ..Default::default()
            },
            Service {
                post: Some((200, SIGNED)),
                get: Some((401, "{}")),
                ..Default::default()
            },
            Service {
                post: Some((200, SIGNED)),
                ..Default::default()
            },
        ];
        let mut outcomes = Vec::new();
        for failure in &failures {
            let (backend, state) = reboot(&dir);
            assert_eq!(
                (state.cheevos.user.as_str(), state.cheevos.web_key),
                ("Douglas", true)
            );
            outcomes.push(
                backend
                    .fetch_profile(&client(failure))
                    .map(|(p, refused)| (p.since, refused)),
            );
            // A new key typed during the failure is not saved over the old one.
            let _ = backend.save_key(&client(failure), "other");
        }
        assert_eq!(
            outcomes,
            [
                Err(Error::Network),
                Err(Error::Service),
                Err(Error::Auth),
                Ok((String::new(), true)),
                Ok((String::new(), false)),
            ]
        );
        assert_eq!(
            (
                fs::read(&backend.paths.config).unwrap(),
                fs::read(&file).unwrap()
            ),
            before,
            "no failure changed a saved file"
        );
        // Only signing out forgets the token and the key.
        backend.cheevos_logout().unwrap();
        let (_, state) = reboot(&dir);
        assert_eq!(
            (state.cheevos.user.as_str(), state.cheevos.web_key),
            ("", false)
        );
        assert!(!file.exists());
    }
    #[test]
    fn a_web_key_is_checked_before_saving_and_can_be_removed() {
        let (dir, backend) = backend();
        let file = dir
            .path()
            .join("storage/.config/archr-launcher/cheevos-web-key");
        let refused = Service {
            get: Some((401, r#"{"message":"Unauthenticated."}"#)),
            ..Default::default()
        };
        assert_eq!(
            backend.save_key(&client(&refused), "k"),
            Err("cheevos.sign_in_first".into())
        );
        let online = Service {
            post: Some((200, SIGNED)),
            get: Some((200, SUMMARY)),
            ..Default::default()
        };
        backend.sign_in(&client(&online), "douglas", "pw").unwrap();
        assert_eq!(
            backend.save_key(&client(&refused), "bad"),
            Err("cheevos.bad_web_key".into())
        );
        assert_eq!(
            backend.save_key(&client(&online), "a\nb"),
            Err("cheevos.bad_web_key".into())
        );
        assert_eq!(
            backend.save_key(&client(&Service::default()), "k"),
            Err("cheevos.network".into())
        );
        assert!(!file.exists());
        backend.save_key(&client(&online), "a&z=b").unwrap();
        assert!(online.urls.lock().unwrap()[0].contains("&y=a%26z%3Db&"));
        assert_eq!(fs::read_to_string(&file).unwrap(), "a&z=b");
        assert_eq!(backend.save_key(&client(&online), ""), Ok(None));
        assert!(!file.exists());
        let elsewhere = dir.path().join("elsewhere");
        fs::write(&elsewhere, "planted").unwrap();
        std::os::unix::fs::symlink(&elsewhere, &file).unwrap();
        assert_eq!(backend.web_key(), None);
    }
}
