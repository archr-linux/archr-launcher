//! `dorequest.php`, the API rcheevos (and so RetroArch) speaks. Every call is a
//! form POST: user names, passwords and tokens never sit in a URL.
use archr_scraper::{encode, image_ext, Curl, Http};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, sync::atomic::AtomicBool};

/// The service and its image server; every hop must stay on them.
pub const HOSTS: &[&str] = &["retroachievements.org"];
const DOREQUEST: &str = "https://retroachievements.org/dorequest.php";
const BADGES: &str = "https://media.retroachievements.org/Badge/";
const REPLY_LIMIT: u64 = 4 << 20;
const BADGE_LIMIT: u64 = 256 << 10;
/// Achievements kept per game; real sets stay far below.
const MAX_ACHIEVEMENTS: usize = 1000;

/// Optional developer web API login from the build environment, in ES's own
/// format (`z=<user>&y=<key>`). Only the profile card's extras need it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Wrong user or password, or a token the service no longer accepts.
    Auth,
    /// The service does not know this game.
    NotFound,
    /// No answer: offline, DNS, timeout.
    Network,
    /// An answer we cannot use.
    Service,
}
impl Error {
    /// Locale key describing the failure.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auth => "cheevos.bad_login",
            Self::NotFound => "cheevos.not_found",
            Self::Network => "cheevos.network",
            Self::Service => "cheevos.service",
        }
    }
}

/// A signed-in account. Never derive Debug: the token must not reach a log.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Account {
    pub user: String,
    pub token: String,
    /// Hardcore and softcore points.
    pub points: u64,
    pub softcore: u64,
}
impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Account({}, <hidden>)", self.user)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub id: u64,
    pub title: String,
    pub hardcore: bool,
    pub achievements: Vec<Achievement>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Achievement {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub points: u64,
    /// Badge image name on the media server (digits).
    pub badge: String,
    pub unlocked: bool,
}

pub struct Client {
    http: Box<dyn Http>,
}

/// Printable text of reasonable length; control characters become spaces.
fn text(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(256)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn number(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str()?.trim().parse().ok())
        .unwrap_or(0)
}

/// Badge names are short digit strings; anything else never becomes a URL or a file.
pub(crate) fn badge_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_digit())
}

impl Client {
    pub fn new(http: Box<dyn Http>) -> Self {
        Self { http }
    }

    /// The system `curl`, hardened as for the scraper; temporaries in `tmp`.
    pub fn production(tmp: PathBuf) -> Self {
        Self::new(Box::new(Curl {
            tmp,
            user_agent: format!("archr-launcher/{}", env!("CARGO_PKG_VERSION")),
            hosts: HOSTS,
        }))
    }

    fn call(&self, form: &str, cancel: &AtomicBool) -> Result<Value, Error> {
        let reply = self
            .http
            .post(DOREQUEST, form, REPLY_LIMIT, cancel)
            .map_err(|_| Error::Network)?;
        let value: Value = serde_json::from_slice(&reply.body).map_err(|_| Error::Service)?;
        if value["Success"].as_bool() == Some(true) && reply.status == 200 {
            return Ok(value);
        }
        Err(match (reply.status, value["Code"].as_str().unwrap_or("")) {
            (401 | 403, _) | (_, "invalid_credentials" | "expired_token" | "access_denied") => {
                Error::Auth
            }
            (404, _) | (_, "not_found") => Error::NotFound,
            _ => Error::Service,
        })
    }

    fn account(value: &Value, user: &str) -> Result<Account, Error> {
        let account = Account {
            user: text(&value["User"]),
            token: text(&value["Token"]),
            points: number(&value["Score"]),
            softcore: number(&value["SoftcoreScore"]),
        };
        // The service answers with the canonical spelling of the same user.
        if account.token.is_empty() || !account.user.eq_ignore_ascii_case(user.trim()) {
            return Err(Error::Service);
        }
        Ok(account)
    }

    /// Exchanges the password for the token RetroArch signs in with.
    pub fn login(&self, user: &str, password: &str, cancel: &AtomicBool) -> Result<Account, Error> {
        let form = format!("r=login2&u={}&p={}", encode(user.trim()), encode(password));
        Self::account(&self.call(&form, cancel)?, user)
    }

    /// The account behind a saved token, with current points.
    pub fn refresh(&self, user: &str, token: &str, cancel: &AtomicBool) -> Result<Account, Error> {
        let form = format!("r=login2&u={}&t={}", encode(user), encode(token));
        Self::account(&self.call(&form, cancel)?, user)
    }

    /// The set for a game hash, with the user's unlocks (hardcore ones in
    /// hardcore mode). Unofficial achievements are left out.
    pub fn game(
        &self,
        (user, token): (&str, &str),
        hash: &str,
        hardcore: bool,
        cancel: &AtomicBool,
    ) -> Result<Game, Error> {
        let auth = format!("u={}&t={}", encode(user), encode(token));
        let patch = self.call(&format!("r=patch&{auth}&m={}", encode(hash)), cancel)?;
        let data = &patch["PatchData"];
        let id = number(&data["ID"]);
        if id == 0 {
            return Err(Error::NotFound);
        }
        let unlocks = self.call(
            &format!("r=unlocks&{auth}&g={id}&h={}", u8::from(hardcore)),
            cancel,
        )?;
        let unlocked: Vec<u64> = unlocks["UserUnlocks"]
            .as_array()
            .map(|ids| ids.iter().map(number).collect())
            .unwrap_or_default();
        let achievements = data["Achievements"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter(|a| number(&a["Flags"]) == 3)
            .take(MAX_ACHIEVEMENTS)
            .map(|a| {
                let id = number(&a["ID"]);
                let badge = text(&a["BadgeName"]);
                Achievement {
                    id,
                    title: text(&a["Title"]),
                    description: text(&a["Description"]),
                    points: number(&a["Points"]),
                    badge: if badge_name(&badge) {
                        badge
                    } else {
                        String::new()
                    },
                    unlocked: unlocked.contains(&id),
                }
            })
            .collect();
        Ok(Game {
            id,
            title: text(&data["Title"]),
            hardcore,
            achievements,
        })
    }

    /// A badge image (the grey `_lock` one for locked achievements), only
    /// when it is a PNG or JPEG that decodes within the scraper's limits.
    pub fn badge(&self, name: &str, locked: bool, cancel: &AtomicBool) -> Result<Vec<u8>, Error> {
        if !badge_name(name) {
            return Err(Error::Service);
        }
        let url = format!("{BADGES}{name}{}.png", if locked { "_lock" } else { "" });
        let reply = self
            .http
            .get(&url, BADGE_LIMIT, cancel)
            .map_err(|_| Error::Network)?;
        if reply.status != 200 || image_ext(&reply.body).is_none() {
            return Err(Error::Service);
        }
        Ok(reply.body)
    }
}
