//! The achievements screen's data, off the draw thread: the game hash
//! (cached per file), the list saved last time at once, then the service's
//! answer and the badge images. Every update is a whole sheet.
use crate::{
    paths::Paths,
    settings::{CHEEVOS_TOKEN, CHEEVOS_USER},
};
use archr_cheevos::{rom_hash, Client, Error, Game};
use archr_config::SystemCfg;
use archr_io::{atomic_publish, read_utf8, Symlinks::Follow};
use archr_ui::{
    cheevos::{Fetch, Item, Sheet},
    settings::Id,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
    time::UNIX_EPOCH,
};

pub struct Lookup {
    updates: Receiver<Sheet>,
    cancel: Arc<AtomicBool>,
}

impl Lookup {
    pub fn start(paths: Paths, system: String, rom: PathBuf) -> Self {
        let (send, updates) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        thread::spawn(move || {
            let client = Client::production(paths.cache.join("cheevos-tmp"));
            run(&paths, &client, (&system, &rom), &stop, &send);
        });
        Self { updates, cancel }
    }
    /// The newest sheet since the last call; never blocks.
    pub fn poll(&mut self) -> Option<Sheet> {
        self.updates.try_iter().last()
    }
}

/// Leaving the screen stops the downloads.
impl Drop for Lookup {
    fn drop(&mut self) {
        self.cancel.store(true, Relaxed);
    }
}

/// A file name part made of the user name's plain characters.
fn plain(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .take(32)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// The game hash, remembered per path, size and modification time.
pub(crate) fn cached_hash(dir: &Path, system: &str, rom: &Path) -> Option<String> {
    let meta = fs::metadata(rom).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut fnv = 0xcbf29ce484222325u64;
    for byte in rom.as_os_str().as_encoded_bytes() {
        fnv = (fnv ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    let path = dir
        .join("hashes")
        .join(format!("{system}-{fnv:016x}-{mtime}-{}", meta.len()));
    let valid = |h: &str| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit());
    if let Some(hash) = read_utf8(&path, 64, Follow).ok().filter(|h| valid(h)) {
        return Some(hash);
    }
    let hash = rom_hash(system, rom)
        .map_err(|e| log::warn!("Cannot hash {}: {e}", rom.display()))
        .ok()??;
    let _ = fs::create_dir_all(dir.join("hashes"));
    let _ = atomic_publish(&path, false, |file| file.write_all(hash.as_bytes()));
    Some(hash)
}

fn badge_path(dir: &Path, name: &str, unlocked: bool) -> PathBuf {
    let suffix = if unlocked { "" } else { "_lock" };
    dir.join("badges").join(format!("{name}{suffix}.png"))
}

/// Unlocked first; badges that are on disk.
fn sheet(game: &Game, fetch: Fetch, dir: &Path) -> Sheet {
    let mut items: Vec<Item> = game
        .achievements
        .iter()
        .map(|a| Item {
            title: a.title.clone(),
            description: a.description.clone(),
            points: a.points,
            unlocked: a.unlocked,
            badge: Some(badge_path(dir, &a.badge, a.unlocked))
                .filter(|path| !a.badge.is_empty() && path.is_file()),
        })
        .collect();
    items.sort_by_key(|item| !item.unlocked);
    Sheet {
        fetch,
        hardcore: game.hardcore,
        items,
    }
}

fn run(
    paths: &Paths,
    client: &Client,
    (system, rom): (&str, &Path),
    cancel: &AtomicBool,
    send: &Sender<Sheet>,
) {
    let fail = |key| {
        let _ = send.send(Sheet {
            fetch: Fetch::Failed(key),
            ..Sheet::default()
        });
    };
    let cfg = SystemCfg::load(&paths.config).ok();
    let get = |key: &str| {
        cfg.as_ref()
            .and_then(|cfg| cfg.get_setting(key, None, None))
            .unwrap_or_default()
    };
    let (user, token) = (get(CHEEVOS_USER), get(CHEEVOS_TOKEN));
    if get(Id::Cheevos.key()) != "1" || user.is_empty() || token.is_empty() {
        return fail("cheevos.sign_in_first");
    }
    let hardcore = get(Id::CheevosHardcore.key()) == "1";
    let dir = paths.cache.join("cheevos");
    let Some(hash) = cached_hash(&dir, system, rom) else {
        return fail("cheevos.unsupported");
    };
    let saved_path = dir.join("games").join(format!(
        "{}-{hash}-{}.json",
        plain(&user),
        u8::from(hardcore)
    ));
    let saved: Option<Game> = read_utf8(&saved_path, 4 << 20, Follow)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    if let Some(game) = &saved {
        let _ = send.send(sheet(game, Fetch::Loading, &dir));
    }
    let (game, fetch) = match client.game((&user, &token), &hash, hardcore, cancel) {
        Ok(game) => {
            let _ = fs::create_dir_all(dir.join("games"));
            if let Ok(text) = serde_json::to_string(&game) {
                let _ = atomic_publish(&saved_path, false, |file| file.write_all(text.as_bytes()));
            }
            (game, Fetch::Fresh)
        }
        Err(Error::Network | Error::Service) if saved.is_some() => {
            (saved.expect("saved list"), Fetch::Saved)
        }
        Err(error) => return fail(error.key()),
    };
    let shown = sheet(&game, fetch, &dir);
    if send.send(shown).is_err() {
        return;
    }
    // Missing badges, in the order the list shows them.
    let _ = fs::create_dir_all(dir.join("badges"));
    let mut wanted: Vec<_> = game.achievements.iter().collect();
    wanted.sort_by_key(|a| !a.unlocked);
    for achievement in wanted {
        let path = badge_path(&dir, &achievement.badge, achievement.unlocked);
        if cancel.load(Relaxed) {
            return;
        }
        if achievement.badge.is_empty() || path.is_file() {
            continue;
        }
        match client.badge(&achievement.badge, !achievement.unlocked, cancel) {
            Ok(bytes) => {
                if atomic_publish(&path, false, |file| file.write_all(&bytes)).is_ok()
                    && send.send(sheet(&game, fetch, &dir)).is_err()
                {
                    return;
                }
            }
            Err(Error::Network) => return,
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use archr_scraper::{Http, NetError, Response};
    use std::sync::Mutex;

    struct Fake {
        online: bool,
        seen: Arc<Mutex<Vec<String>>>,
    }
    fn png() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(2, 2)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    impl Http for Fake {
        fn get(&self, url: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
            self.seen.lock().unwrap().push(url.into());
            if !self.online {
                return Err(NetError::Failed);
            }
            Ok(Response {
                status: 200,
                body: png(),
            })
        }
        fn post(&self, _: &str, form: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
            self.seen.lock().unwrap().push(form.into());
            if !self.online {
                return Err(NetError::Timeout);
            }
            let body = if form.starts_with("r=patch") {
                r#"{"Success":true,"PatchData":{"ID":9,"Title":"Mario","Achievements":[
                {"ID":1,"Title":"First","Description":"a","Points":5,"BadgeName":"111","Flags":3},
                {"ID":2,"Title":"Second","Description":"b","Points":10,"BadgeName":"222","Flags":3}]}}"#
            } else {
                r#"{"Success":true,"UserUnlocks":[2]}"#
            };
            Ok(Response {
                status: 200,
                body: body.as_bytes().to_vec(),
            })
        }
    }
    fn fixture(config: &str) -> (tempfile::TempDir, Paths, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(dir.path().into()));
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::write(&paths.config, config).unwrap();
        let rom = dir.path().join("mario.nes");
        fs::write(&rom, [b"NES\x1a".as_slice(), &[0; 12], b"game"].concat()).unwrap();
        (dir, paths, rom)
    }
    fn sheets(paths: &Paths, rom: &Path, online: bool) -> (Vec<Sheet>, Vec<String>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let client = Client::new(Box::new(Fake {
            online,
            seen: seen.clone(),
        }));
        let (send, receive) = channel();
        run(paths, &client, ("nes", rom), &AtomicBool::new(false), &send);
        drop(send);
        let seen = seen.lock().unwrap().clone();
        (receive.iter().collect(), seen)
    }
    const SIGNED_IN: &str =
        "global.retroachievements=1\nglobal.retroachievements.username=Douglas\n\
        global.retroachievements.token=tok\nglobal.retroachievements.hardcore=1\n";

    #[test]
    fn a_game_is_fetched_with_its_badges_then_served_from_the_cache_offline() {
        let (_dir, paths, rom) = fixture(SIGNED_IN);
        let (updates, seen) = sheets(&paths, &rom, true);
        let hash = rom_hash("nes", &rom).unwrap().unwrap();
        assert!(seen[0].starts_with("r=patch&u=Douglas&t=tok&m=") && seen[0].ends_with(&hash));
        assert!(seen[1].ends_with("&h=1"), "{seen:?}");
        assert!(seen[2].ends_with("/Badge/222.png") && seen[3].ends_with("/Badge/111_lock.png"));
        let last = updates.last().unwrap();
        assert_eq!(last.fetch, Fetch::Fresh);
        assert!(last.hardcore);
        let titles: Vec<_> = last
            .items
            .iter()
            .map(|i| (i.title.as_str(), i.unlocked))
            .collect();
        assert_eq!(titles, [("Second", true), ("First", false)]);
        assert!(last
            .items
            .iter()
            .all(|i| i.badge.as_ref().is_some_and(|p| p.is_file())));
        // Offline: the saved list, then nothing new to download.
        let (updates, seen) = sheets(&paths, &rom, false);
        assert_eq!(updates.first().unwrap().fetch, Fetch::Loading);
        assert_eq!(updates.last().unwrap().fetch, Fetch::Saved);
        assert_eq!(updates.last().unwrap().items.len(), 2);
        assert_eq!(seen.len(), 1, "only the list was asked for: {seen:?}");
    }

    #[test]
    fn signed_out_unsupported_and_unreachable_explain_themselves() {
        let (_dir, paths, rom) = fixture("global.retroachievements=1\n");
        let (updates, seen) = sheets(&paths, &rom, true);
        assert_eq!(updates[0].fetch, Fetch::Failed("cheevos.sign_in_first"));
        assert!(seen.is_empty());
        let (_dir, paths, _) = fixture(SIGNED_IN);
        let chd = paths.cache.join("game.chd");
        fs::create_dir_all(&paths.cache).unwrap();
        fs::write(&chd, b"x").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let client = Client::new(Box::new(Fake { online: true, seen }));
        let (send, receive) = channel();
        run(
            &paths,
            &client,
            ("psx", &chd),
            &AtomicBool::new(false),
            &send,
        );
        assert_eq!(
            receive.recv().unwrap().fetch,
            Fetch::Failed("cheevos.unsupported")
        );
        let (_dir, paths, rom) = fixture(SIGNED_IN);
        let (updates, _) = sheets(&paths, &rom, false);
        assert_eq!(updates[0].fetch, Fetch::Failed("cheevos.network"));
    }
}
