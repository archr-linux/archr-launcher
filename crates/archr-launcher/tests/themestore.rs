//! Theme store against canned GitHub answers and hand-made tarballs: no test
//! ever reaches the network.
use archr_launcher::themestore::{extract, Shop, HOSTS};
use archr_scraper::{trusted_on, Http, NetError, Response};
use archr_ui::settings::store::{Action, Card, Detail, Reply, Tab};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

type Answers = BTreeMap<String, (u16, Vec<u8>)>;
/// GitHub as a table of URLs; anything else is a 404. `None` means offline.
#[derive(Clone, Default)]
struct Fake {
    answers: Arc<Mutex<Answers>>,
    asked: Arc<Mutex<Vec<String>>>,
    offline: Arc<Mutex<bool>>,
}
impl Fake {
    fn answer(&self, url: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.answers
            .lock()
            .unwrap()
            .insert(url.into(), (status, body.into()));
    }
    fn asked(&self, url: &str) -> usize {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|u| *u == url)
            .count()
    }
}
impl Http for Fake {
    fn get(&self, url: &str, limit: u64, _: &AtomicBool) -> Result<Response, NetError> {
        assert!(trusted_on(url, HOSTS), "left GitHub: {url}");
        self.asked.lock().unwrap().push(url.into());
        if *self.offline.lock().unwrap() {
            return Err(NetError::Failed);
        }
        let (status, body) = self
            .answers
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .unwrap_or((404, Vec::new()));
        if body.len() as u64 > limit {
            return Err(NetError::Failed);
        }
        Ok(Response { status, body })
    }
}

const SEARCH_STARS: &str = "https://api.github.com/search/repositories?q=topic:archr-launcher-theme+archived:false&sort=stars&order=desc&per_page=30";
const RAW: &str = "https://raw.githubusercontent.com/kaio-dev/neon";
const REPO: &str = "https://api.github.com/repos/kaio-dev/neon";
const THEME: &str = "name = \"Neon Arcade\"\nauthor = \"kaio\"\nversion = \"1.2.0\"\n";

fn shop(root: &Path, fake: &Fake) -> Shop {
    Shop::new(
        Box::new(fake.clone()),
        root.join("cache"),
        root.join("themes"),
    )
}
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(64, 48, image::Rgba([255, 46, 151, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}
fn card() -> Card {
    Card {
        owner: "kaio-dev".into(),
        repo: "neon".into(),
        stars: 214,
        branch: "main".into(),
        pushed: "2026-10-12".into(),
        ..Card::default()
    }
}

/// A gzip tarball under GitHub's top folder. Paths are written raw, so a
/// test can hold what `tar::Builder` itself refuses.
struct Tarball(tar::Builder<flate2::write::GzEncoder<Vec<u8>>>);
impl Tarball {
    fn new() -> Self {
        Self(tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        )))
    }
    fn entry(mut self, path: &str, kind: tar::EntryType, link: &str, data: &[u8]) -> Self {
        let mut header = tar::Header::new_gnu();
        // An absolute path is written as is, without the top folder.
        let name = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("kaio-dev-neon-abc123/{path}")
        };
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
        header.set_entry_type(kind);
        header.set_mode(0o644);
        header.set_size(data.len() as u64);
        header.set_cksum();
        self.0.append(&header, data).unwrap();
        self
    }
    fn file(self, path: &str, data: &[u8]) -> Self {
        self.entry(path, tar::EntryType::Regular, "", data)
    }
    fn theme() -> Self {
        Self::new()
            .file("theme.toml", THEME.as_bytes())
            .file("preview.png", &png())
            .file("fonts/Neon.ttf", b"font")
            .file("sounds/move.ogg", b"ogg")
            .file("install.sh", b"#!/bin/sh\nrm -rf /\n")
            .file("lib/evil.so", b"\x7fELF")
            .file(".github/workflows/x.md", b"hidden")
    }
    fn bytes(self) -> Vec<u8> {
        self.0.into_inner().unwrap().finish().unwrap()
    }
}

fn detail(root: &Path, fake: &Fake) -> Detail {
    fake.answer(
        &format!("{REPO}/releases/latest"),
        200,
        r#"{"tag_name":"v1.2.0","published_at":"2026-10-12T10:00:00Z"}"#,
    );
    fake.answer(
        &format!("{REPO}/git/trees/v1.2.0?recursive=1"),
        200,
        r#"{"sha":"tree1","truncated":false,"tree":[
            {"path":"theme.toml","type":"blob","size":60},
            {"path":"preview.png","type":"blob","size":3000},
            {"path":"screenshots","type":"tree"},
            {"path":"screenshots/1.png","type":"blob","size":3000},
            {"path":"fonts/Neon.ttf","type":"blob","size":1000000},
            {"path":"install.sh","type":"blob","size":99999}]}"#,
    );
    fake.answer(&format!("{RAW}/v1.2.0/theme.toml"), 200, THEME);
    fake.answer(&format!("{RAW}/v1.2.0/preview.png"), 200, png());
    fake.answer(&format!("{RAW}/v1.2.0/screenshots/1.png"), 200, png());
    shop(root, fake).detail(&card()).unwrap()
}

#[test]
fn the_search_is_saved_for_an_hour_and_rate_limits_keep_the_saved_list() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    fake.answer(
        SEARCH_STARS,
        200,
        r#"{"items":[
            {"name":"neon","owner":{"login":"kaio-dev"},"stargazers_count":214,
             "default_branch":"main","pushed_at":"2026-10-12T10:00:00Z"},
            {"name":"../escape","owner":{"login":"x"},"stargazers_count":1,"default_branch":"main"},
            {"name":"ok","owner":{"login":"bad/owner"},"stargazers_count":1,"default_branch":"main"}]}"#,
    );
    let store = shop(root.path(), &fake);
    let Reply::Listed {
        cards,
        problem,
        fetched,
        ..
    } = store.list(Tab::Popular, false)
    else {
        panic!("not a list");
    };
    assert_eq!(cards, [card()], "unsafe names are dropped");
    assert!(problem.is_none() && fetched.is_some());
    store.list(Tab::Popular, false);
    assert_eq!(
        fake.asked(SEARCH_STARS),
        1,
        "the second look is the saved one"
    );
    fake.answer(
        SEARCH_STARS,
        403,
        r#"{"message":"API rate limit exceeded"}"#,
    );
    let Reply::Listed { cards, problem, .. } = store.list(Tab::Popular, true) else {
        panic!("not a list");
    };
    assert_eq!(problem, Some("store.rate_limited"));
    assert_eq!(cards.len(), 1, "the saved list stays on screen");
    *fake.offline.lock().unwrap() = true;
    let Reply::Listed { cards, problem, .. } = store.list(Tab::Recent, false) else {
        panic!("not a list");
    };
    assert_eq!((cards.len(), problem), (0, Some("store.offline")));
}

#[test]
fn cards_get_the_theme_name_and_a_checked_cover() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    fake.answer(&format!("{RAW}/main/theme.toml"), 200, THEME);
    fake.answer(&format!("{RAW}/main/preview.png"), 200, png());
    let store = shop(root.path(), &fake);
    let decorated = store.decorate(&card()).unwrap();
    assert_eq!(decorated.title("pt_BR"), "Neon Arcade");
    let preview = decorated.preview.unwrap();
    assert!(preview.is_file());
    // Same push: nothing is fetched again, even offline.
    *fake.offline.lock().unwrap() = true;
    assert!(store.decorate(&card()).unwrap().preview.is_some());
    assert_eq!(fake.asked(&format!("{RAW}/main/theme.toml")), 1);
    // A "preview" that is not an image is never kept.
    *fake.offline.lock().unwrap() = false;
    fake.answer(&format!("{RAW}/main/preview.png"), 200, b"<html>".to_vec());
    let pushed = Card {
        pushed: "2026-10-13".into(),
        ..card()
    };
    assert_eq!(store.decorate(&pushed).unwrap().preview, None);
}

#[test]
fn details_follow_the_latest_release_and_install_only_theme_files() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let detail = detail(root.path(), &fake);
    assert_eq!(
        (detail.reference.as_str(), detail.tree.as_str()),
        ("v1.2.0", "tree1")
    );
    assert_eq!(detail.date, "2026-10-12");
    assert_eq!(detail.images.len(), 2, "preview and one screenshot");
    assert_eq!(
        detail.size,
        60 + 3000 + 3000 + 1_000_000,
        "install.sh is not counted"
    );
    assert_eq!(detail.includes, ["store.colors", "store.fonts"]);
    let store = shop(root.path(), &fake);
    fake.answer(
        "https://codeload.github.com/kaio-dev/neon/tar.gz/v1.2.0",
        200,
        Tarball::theme().bytes(),
    );
    let mut queue = VecDeque::new();
    let replies = store.handle(
        Action::Install(Box::new(detail.clone())),
        "default",
        &mut queue,
    );
    assert!(matches!(
        replies.last(),
        Some(Reply::Done(Ok("store.installed_ok")))
    ));
    let theme = root.path().join("themes/kaio-dev-neon");
    for kept in [
        "theme.toml",
        "preview.png",
        "fonts/Neon.ttf",
        "sounds/move.ogg",
    ] {
        assert!(theme.join(kept).is_file(), "{kept} missing");
    }
    for dropped in ["install.sh", "lib/evil.so", ".github"] {
        assert!(!theme.join(dropped).exists(), "{dropped} installed");
    }
    let Reply::Installed(map, cards) = store.installed() else {
        panic!("not the installed list");
    };
    assert_eq!(map.get("kaio-dev-neon").map(String::as_str), Some("tree1"));
    assert_eq!(cards[0].title("en"), "Neon Arcade");
    // No leftovers: the staging and old folders are gone.
    let names: Vec<_> = fs::read_dir(root.path().join("themes"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["kaio-dev-neon"]);
    // Offline, an installed theme still shows its details from the folder.
    *fake.offline.lock().unwrap() = true;
    fs::remove_dir_all(root.path().join("cache")).unwrap();
    let local = store.detail(&card()).unwrap();
    assert_eq!((local.tree.as_str(), local.images.len()), ("tree1", 1));
    // The theme in use stays; another one goes, and only store folders go.
    let removed = store.handle(
        Action::Remove("kaio-dev-neon".into()),
        "kaio-dev-neon",
        &mut queue,
    );
    assert!(matches!(
        removed[..],
        [Reply::Done(Err("store.active_theme"))]
    ));
    fs::create_dir_all(root.path().join("themes/mine")).unwrap();
    fs::write(
        root.path().join("themes/mine/theme.toml"),
        "name = \"Mine\"",
    )
    .unwrap();
    assert_eq!(store.remove("mine"), Err("store.failed"));
    assert_eq!(store.remove("../themes/kaio-dev-neon"), Err("store.failed"));
    assert_eq!(store.remove("kaio-dev-neon"), Ok(()));
    assert!(!theme.exists() && root.path().join("themes/mine").is_dir());
}

#[test]
fn an_update_replaces_the_folder_and_a_hand_made_one_is_never_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let mut detail = detail(root.path(), &fake);
    let tarball = "https://codeload.github.com/kaio-dev/neon/tar.gz/v1.2.0";
    fake.answer(tarball, 200, Tarball::theme().bytes());
    let store = shop(root.path(), &fake);
    store.install(&detail).unwrap();
    let theme = root.path().join("themes/kaio-dev-neon");
    fs::write(theme.join("stale.txt"), "old").unwrap();
    detail.tree = "tree2".into();
    fake.answer(
        tarball,
        200,
        Tarball::new().file("theme.toml", THEME.as_bytes()).bytes(),
    );
    store.install(&detail).unwrap();
    assert!(!theme.join("stale.txt").exists() && !theme.join("fonts").exists());
    let Reply::Installed(map, _) = store.installed() else {
        panic!()
    };
    assert_eq!(map["kaio-dev-neon"], "tree2");
    // A broken download leaves the installed version alone.
    fake.answer(tarball, 200, b"not gzip".to_vec());
    assert_eq!(store.install(&detail), Err("store.invalid"));
    fake.answer(
        tarball,
        200,
        Tarball::new().file("readme.md", b"no theme").bytes(),
    );
    assert_eq!(store.install(&detail), Err("store.invalid"));
    *fake.offline.lock().unwrap() = true;
    assert_eq!(store.install(&detail), Err("store.download_failed"));
    assert!(theme.join("theme.toml").is_file());
    // A folder of that name the store did not make is refused.
    fs::remove_dir_all(&theme).unwrap();
    fs::create_dir_all(&theme).unwrap();
    assert_eq!(store.install(&detail), Err("store.name_taken"));
}

#[test]
fn a_path_leaving_the_theme_refuses_the_whole_archive() {
    let root = tempfile::tempdir().unwrap();
    let into = root.path().join("into");
    fs::create_dir(&into).unwrap();
    for path in ["../../escape.txt", "fonts/../../escape.txt", "/abs.txt"] {
        let tarball = Tarball::new()
            .file("theme.toml", THEME.as_bytes())
            .file(path, b"x")
            .bytes();
        assert_eq!(extract(&tarball, &into), Err("store.invalid"), "{path}");
        assert!(!root.path().join("escape.txt").exists());
        fs::remove_dir_all(&into).unwrap();
        fs::create_dir(&into).unwrap();
    }
}

#[test]
fn links_and_special_files_are_never_created() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let into = root.path().join("into");
    fs::create_dir(&into).unwrap();
    let tarball = Tarball::new()
        .file("theme.toml", THEME.as_bytes())
        .entry(
            "fonts",
            tar::EntryType::Symlink,
            outside.to_str().unwrap(),
            b"",
        )
        .file("fonts/A.ttf", b"font")
        .entry("icons/x.svg", tar::EntryType::Symlink, "/etc/passwd", b"")
        .entry("sounds/hard.ogg", tar::EntryType::Link, "theme.toml", b"")
        .entry("sounds/fifo.ogg", tar::EntryType::Fifo, "", b"")
        .entry("sounds/dev.ogg", tar::EntryType::Char, "", b"")
        .bytes();
    extract(&tarball, &into).unwrap();
    assert!(fs::symlink_metadata(into.join("fonts")).unwrap().is_dir());
    assert!(into.join("fonts/A.ttf").is_file());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    for skipped in [
        "icons/x.svg",
        "sounds/hard.ogg",
        "sounds/fifo.ogg",
        "sounds/dev.ogg",
    ] {
        assert!(
            fs::symlink_metadata(into.join(skipped)).is_err(),
            "{skipped}"
        );
    }
}

#[test]
fn oversized_archives_stop_before_filling_the_card() {
    let root = tempfile::tempdir().unwrap();
    let into = root.path().join("into");
    fs::create_dir(&into).unwrap();
    // 61 MiB of zeros: a few kilobytes compressed, too much unpacked.
    let mut header = tar::Header::new_gnu();
    header
        .set_path("kaio-dev-neon-abc123/sounds/huge.wav")
        .unwrap();
    header.set_size(61 << 20);
    header.set_mode(0o644);
    header.set_cksum();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    builder
        .append(&header, std::io::Read::take(std::io::repeat(0), 61 << 20))
        .unwrap();
    let tarball = builder.into_inner().unwrap().finish().unwrap();
    assert!(tarball.len() < 1 << 20);
    assert_eq!(extract(&tarball, &into), Err("store.too_big"));
    assert!(!into.join("sounds/huge.wav").exists());
    // A download over the tarball limit never arrives (curl stops it).
    let fake = Fake::default();
    let big = vec![0u8; (25 << 20) + 1];
    fake.answer(
        "https://codeload.github.com/kaio-dev/neon/tar.gz/main",
        200,
        big,
    );
    let detail = Detail {
        card: card(),
        reference: "main".into(),
        tree: "t".into(),
        ..Detail::default()
    };
    assert_eq!(
        shop(root.path(), &fake).install(&detail),
        Err("store.download_failed")
    );
}

#[test]
fn only_github_hosts_are_trusted() {
    for url in [
        "https://api.github.com/search/repositories",
        "https://codeload.github.com/a/b/tar.gz/v1",
        "https://raw.githubusercontent.com/a/b/main/theme.toml",
        "https://objects.githubusercontent.com/x",
    ] {
        assert!(trusted_on(url, HOSTS), "{url}");
    }
    for url in [
        "http://api.github.com/x",
        "https://github.com/a/b",
        "https://api.github.com.evil.example/x",
        "https://evil.example/api.github.com",
    ] {
        assert!(!trusted_on(url, HOSTS), "{url}");
    }
}
