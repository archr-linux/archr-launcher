//! Module store against canned GitHub answers and hand-made tarballs: no
//! test ever reaches the network.
use archr_launcher::modules::{
    registry::{Dirs, Record, State},
    store::{same_digest, Shop, HOSTS},
};
use archr_scraper::{trusted_on, Http, NetError, Response};
use archr_ui::modules::{Card, StoreReply, Tab};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

/// GitHub as a table of URLs; anything else is a 404.
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);
impl Fake {
    fn answer(&self, url: &str, body: impl Into<Vec<u8>>) {
        self.0.lock().unwrap().insert(url.into(), body.into());
    }
}
impl Http for Fake {
    fn get(&self, url: &str, limit: u64, _: &AtomicBool) -> Result<Response, NetError> {
        assert!(trusted_on(url, HOSTS), "left GitHub: {url}");
        match self.0.lock().unwrap().get(url) {
            Some(body) if body.len() as u64 > limit => Err(NetError::Failed),
            Some(body) => Ok(Response {
                status: 200,
                body: body.clone(),
            }),
            None => Ok(Response {
                status: 404,
                body: Vec::new(),
            }),
        }
    }
}

const SEARCH: &str = "https://api.github.com/search/repositories?q=topic:archr-launcher-module+archived:false&sort=stars&order=desc&per_page=30";
const API: &str = "https://api.github.com/repos";
const DOWNLOAD: &str = "https://github.com/kaio-dev/archr-bateria/releases/download/v1.0.0";
const MANIFEST: &str = "id = \"bateria\"\nversion = \"1.0.0\"\napi_version = 1\n\
    name = { en = \"Battery\", pt_BR = \"Bateria\" }\nicon = \"battery-full\"\n\
    [run]\nruntime = \"bash\"\nentry = \"main.sh\"\nmode = \"daemon\"\n\
    [[start.tiles]]\nid = \"b\"\naction = \"refresh\"\n";

/// A gzip tarball; paths are written raw so a test can hold what
/// `tar::Builder` itself refuses.
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
        header.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
        header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
        header.set_entry_type(kind);
        header.set_mode(0o777);
        header.set_size(data.len() as u64);
        header.set_cksum();
        self.0.append(&header, data).unwrap();
        self
    }
    fn file(self, path: &str, data: &[u8]) -> Self {
        self.entry(path, tar::EntryType::Regular, "", data)
    }
    fn module(manifest: &str) -> Self {
        let icons = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/default/icons");
        Self::new()
            .file("bateria/module.toml", manifest.as_bytes())
            .file("bateria/main.sh", b"#!/bin/bash\n")
            .file("bateria/locales/en.toml", b"\"x\" = \"y\"\n")
            .file(
                "bateria/icons/battery-full.svg",
                &fs::read(icons.join("battery-full.svg")).unwrap(),
            )
    }
    fn bytes(self) -> Vec<u8> {
        self.0.into_inner().unwrap().finish().unwrap()
    }
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A release of kaio-dev/archr-bateria holding `tarball`, its digest as
/// GitHub publishes it, and the reviewed list naming `reviewed`.
fn publish(fake: &Fake, manifest: &str, tarball: &[u8], digest: &str, reviewed: &str) {
    fake.answer(
        SEARCH,
        r#"{"items":[
            {"name":"archr-tempo","owner":{"login":"ana"},"stargazers_count":87,"default_branch":"main"},
            {"name":"archr-bateria","owner":{"login":"kaio-dev"},"stargazers_count":12,"default_branch":"main"}]}"#,
    );
    fake.answer(
        &format!("{API}/kaio-dev/archr-bateria/releases/latest"),
        format!(
            r#"{{"tag_name":"v1.0.0","assets":[
            {{"name":"module.toml","size":{},"digest":"sha256:{}"}},
            {{"name":"bateria-1.0.0.tar.gz","size":{},"digest":"sha256:{digest}"}}]}}"#,
            manifest.len(),
            sha(manifest.as_bytes()),
            tarball.len()
        ),
    );
    fake.answer(&format!("{DOWNLOAD}/module.toml"), manifest);
    fake.answer(
        &format!("{DOWNLOAD}/bateria-1.0.0.tar.gz"),
        tarball.to_vec(),
    );
    let list = format!(
        r#"{{"serial":3,"modules":[{{"repo":"kaio-dev/archr-bateria","digest":"sha256:{reviewed}"}}]}}"#
    );
    review(fake, list.as_bytes(), &sign(list.as_bytes()));
}
/// The test key's private half; `shop` trusts its public half.
const TEST_KEY: [u8; 32] = [7; 32];
fn sign(body: &[u8]) -> String {
    use ed25519_dalek::Signer;
    let key = ed25519_dalek::SigningKey::from_bytes(&TEST_KEY);
    key.sign(body)
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// archr-linux/archr-modules' latest release: the list and its signature.
fn review(fake: &Fake, list: &[u8], signature: &str) {
    fake.answer(
        &format!("{API}/archr-linux/archr-modules/releases/latest"),
        r#"{"tag_name":"v3","assets":[{"name":"reviewed.json","size":100},
            {"name":"reviewed.json.sig","size":129}]}"#,
    );
    let download = "https://github.com/archr-linux/archr-modules/releases/download/v3";
    fake.answer(&format!("{download}/reviewed.json"), list.to_vec());
    fake.answer(
        &format!("{download}/reviewed.json.sig"),
        format!("{signature}\n"),
    );
}
fn shop(root: &Path, fake: &Fake) -> Shop {
    let public = ed25519_dalek::SigningKey::from_bytes(&TEST_KEY).verifying_key();
    Shop::new(Box::new(fake.clone()), root.join("cache"), Dirs::new(root))
        .with_review_key(public.to_bytes())
}
fn card() -> Card {
    Card {
        owner: "kaio-dev".into(),
        repo: "archr-bateria".into(),
        ..Card::default()
    }
}

#[test]
fn reviewed_modules_come_first_and_the_badge_follows_the_digest() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let tarball = Tarball::module(MANIFEST).bytes();
    publish(&fake, MANIFEST, &tarball, &sha(&tarball), &sha(&tarball));
    let store = shop(root.path(), &fake);
    let StoreReply::Listed { cards, .. } = store.list(Tab::Reviewed, false) else {
        panic!("not a list");
    };
    assert_eq!(cards.len(), 1);
    assert!(cards[0].reviewed);
    // The digests travel with the card: the host checks the installed one.
    assert_eq!(cards[0].digests, [format!("sha256:{}", sha(&tarball))]);
    assert!(same_digest(
        &cards[0].digests[0],
        &sha(&tarball).to_uppercase()
    ));
    assert!(!same_digest("sha256:", ""), "nothing is not a match");
    let StoreReply::Listed { cards, .. } = store.list(Tab::All, false) else {
        panic!("not a list");
    };
    assert_eq!(cards[0].repo, "archr-bateria", "reviewed first");
    assert_eq!(cards.len(), 2);
    fake.answer(
        "https://github.com/kaio-dev/archr-bateria/releases/latest/download/module.toml",
        MANIFEST,
    );
    let card = store.decorate(&card()).unwrap();
    assert_eq!(
        (card.id.as_str(), card.name.as_str()),
        ("bateria", "Battery")
    );
    let offer = store.offer(&card).unwrap();
    assert!(offer.reviewed);
    assert_eq!(offer.version, "1.0.0");
    assert!(offer.blocked.is_none() && offer.added.is_empty());
    assert!(offer
        .permissions
        .iter()
        .any(|p| p.key == "modules.perm_tile"));
    // A new release under the same version: the badge goes, in the list too.
    let other = Tarball::module(MANIFEST)
        .file("bateria/x.txt", b"x")
        .bytes();
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    publish(&fake, MANIFEST, &other, &sha(&other), &sha(&tarball));
    let store = shop(root.path(), &fake);
    assert!(!store.offer(&card).unwrap().reviewed);
    let StoreReply::Listed { cards, .. } = store.list(Tab::All, false) else {
        panic!("not a list");
    };
    let bateria = cards.iter().find(|c| c.repo == "archr-bateria").unwrap();
    assert!(
        !bateria.reviewed,
        "the current release is not the reviewed one"
    );
    assert_eq!(
        bateria.digests.len(),
        1,
        "an installed reviewed package keeps its badge"
    );
    let StoreReply::Listed { cards, .. } = store.list(Tab::Reviewed, false) else {
        panic!("not a list");
    };
    assert!(cards.is_empty());
}

#[test]
fn an_install_checks_digest_manifest_and_paths_before_anything_lands() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let good = Tarball::module(MANIFEST).bytes();
    publish(&fake, MANIFEST, &good, &sha(&good), "");
    let store = shop(root.path(), &fake);
    let offer = store.offer(&card()).unwrap();
    assert_eq!(offer.digest, format!("sha256:{}", sha(&good)));
    // Another package published after the page was shown, even a valid one.
    let other = Tarball::module(MANIFEST)
        .file("bateria/x.txt", b"x")
        .bytes();
    fs::remove_dir_all(root.path().join("cache")).unwrap();
    let swapped_release = Fake::default();
    publish(&swapped_release, MANIFEST, &other, &sha(&other), "");
    assert_eq!(
        shop(root.path(), &swapped_release).install(&offer),
        Err("modules.digest_mismatch")
    );
    assert!(!root.path().join("modules/bateria").exists());
    // A package that is not the one GitHub describes (an hour later).
    fs::remove_dir_all(root.path().join("cache")).unwrap();
    publish(&fake, MANIFEST, &good, &sha(b"other"), "");
    let store = shop(root.path(), &fake);
    assert_eq!(store.install(&offer), Err("modules.digest_mismatch"));
    // Each hostile package is refused whole, nothing installed.
    let swapped = MANIFEST.replace(
        "[[start.tiles]]",
        "[permissions]\nnet = true\n[[start.tiles]]",
    );
    for (tarball, problem) in [
        (
            Tarball::module(&swapped).bytes(),
            "modules.manifest_mismatch",
        ),
        (
            Tarball::module(MANIFEST)
                .file("bateria/../../escape", b"x")
                .bytes(),
            "modules.invalid",
        ),
        (
            Tarball::module(MANIFEST)
                .entry("bateria/link", tar::EntryType::Symlink, "/etc/shadow", b"")
                .bytes(),
            "modules.invalid",
        ),
        (
            Tarball::module(MANIFEST)
                .file("outro/main.sh", b"x")
                .bytes(),
            "modules.invalid",
        ),
        (
            Tarball::new()
                .file("bateria/module.toml", MANIFEST.as_bytes())
                .bytes(),
            "modules.invalid",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let fake = Fake::default();
        publish(&fake, MANIFEST, &tarball, &sha(&tarball), "");
        let store = shop(root.path(), &fake);
        // As if shown with this package: the unpacking itself refuses it.
        let shown = archr_ui::modules::Offer {
            digest: format!("sha256:{}", sha(&tarball)),
            ..offer.clone()
        };
        assert_eq!(store.install(&shown), Err(problem));
        let left: Vec<_> = fs::read_dir(root.path().join("modules"))
            .map(|d| d.flatten().map(|e| e.file_name()).collect())
            .unwrap_or_default();
        assert!(left.is_empty(), "{problem}: {left:?}");
    }
    // The good one: code with normalized modes, entry executable.
    fs::remove_dir_all(root.path().join("cache")).unwrap();
    publish(&fake, MANIFEST, &good, &sha(&good), "");
    let installed = shop(root.path(), &fake).install(&offer).unwrap();
    assert_eq!(installed.source, "kaio-dev/archr-bateria");
    assert_eq!(installed.digest, format!("sha256:{}", sha(&good)));
    let dir = root.path().join("modules/bateria");
    let mode = |name: &str| fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!((mode("main.sh"), mode("module.toml")), (0o755, 0o644));
}

#[test]
fn updates_show_only_new_permissions_and_keep_the_previous_version() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let good = Tarball::module(MANIFEST).bytes();
    publish(&fake, MANIFEST, &good, &sha(&good), "");
    let installed = shop(root.path(), &fake)
        .install(&shop(root.path(), &fake).offer(&card()).unwrap())
        .unwrap();
    let mut state = State::default();
    state.modules.insert(
        "bateria".into(),
        Record {
            source: installed.source,
            version: installed.version,
            consent: true,
            enabled: true,
            ..Record::default()
        },
    );
    state.save(&root.path().join("modules.toml")).unwrap();
    let newer = MANIFEST.replace("1.0.0", "1.1.0").replace(
        "[[start.tiles]]",
        "[permissions]\nnet = true\nread = [\"roms\"]\n[[start.tiles]]",
    );
    let tarball = Tarball::module(&newer).bytes();
    fs::remove_dir_all(root.path().join("cache")).unwrap();
    let fake = Fake::default();
    publish(&fake, &newer, &tarball, &sha(&tarball), "");
    let download = "https://github.com/kaio-dev/archr-bateria/releases/download/v1.1.0";
    fake.answer(
        &format!("{API}/kaio-dev/archr-bateria/releases/latest"),
        format!(
            r#"{{"tag_name":"v1.1.0","assets":[{{"name":"module.toml","size":1}},
            {{"name":"bateria-1.1.0.tar.gz","size":{},"digest":"sha256:{}"}}]}}"#,
            tarball.len(),
            sha(&tarball)
        ),
    );
    fake.answer(&format!("{download}/module.toml"), newer.clone());
    fake.answer(&format!("{download}/bateria-1.1.0.tar.gz"), tarball);
    let store = shop(root.path(), &fake);
    let offer = store.offer(&card()).unwrap();
    assert_eq!(offer.installed.as_deref(), Some("1.0.0"));
    let added: Vec<&str> = offer.added.iter().map(|p| p.key).collect();
    assert_eq!(added, ["modules.perm_net", "modules.perm_read_roms"]);
    assert!(
        offer.added.iter().all(|p| p.sensitive),
        "internet and ROMs together"
    );
    store.install(&offer).unwrap();
    let previous = root.path().join("modules/.previous-bateria/module.toml");
    assert!(fs::read_to_string(previous).unwrap().contains("1.0.0"));
    // Another repository may not take over the id.
    let other = Card {
        owner: "outro".into(),
        ..card()
    };
    let fake = Fake::default();
    let tarball = Tarball::module(MANIFEST).bytes();
    fake.answer(
        &format!("{API}/outro/archr-bateria/releases/latest"),
        format!(
            r#"{{"tag_name":"v1.0.0","assets":[{{"name":"module.toml","size":1}},
            {{"name":"bateria-1.0.0.tar.gz","size":1,"digest":"sha256:{}"}}]}}"#,
            sha(&tarball)
        ),
    );
    fake.answer(
        "https://github.com/outro/archr-bateria/releases/download/v1.0.0/module.toml",
        MANIFEST,
    );
    let offer = shop(root.path(), &fake).offer(&other).unwrap();
    assert_eq!(offer.blocked, Some("modules.other_source"));
}

/// kaio-dev/archr-bateria 1.0.0 installed from the store, as the host
/// records it.
fn installed_bateria(root: &Path) {
    let fake = Fake::default();
    let good = Tarball::module(MANIFEST).bytes();
    publish(&fake, MANIFEST, &good, &sha(&good), "");
    let store = shop(root, &fake);
    let installed = store.install(&store.offer(&card()).unwrap()).unwrap();
    let mut state = State::default();
    state.modules.insert(
        "bateria".into(),
        Record {
            source: installed.source,
            version: installed.version,
            digest: installed.digest,
            granted: installed.granted,
            consent: true,
            enabled: true,
            ..Record::default()
        },
    );
    state.save(&root.join("modules.toml")).unwrap();
    fs::remove_dir_all(root.join("cache")).unwrap();
}
/// Release 1.1.0 of `manifest`, reviewed when `reviewed`.
fn release_1_1(manifest: &str, reviewed: bool) -> Fake {
    let tarball = Tarball::module(manifest).bytes();
    let fake = Fake::default();
    let digest = sha(&tarball);
    publish(
        &fake,
        manifest,
        &tarball,
        &digest,
        if reviewed { &digest } else { "" },
    );
    let download = "https://github.com/kaio-dev/archr-bateria/releases/download/v1.1.0";
    fake.answer(
        &format!("{API}/kaio-dev/archr-bateria/releases/latest"),
        format!(
            r#"{{"tag_name":"v1.1.0","assets":[{{"name":"module.toml","size":1}},
            {{"name":"bateria-1.1.0.tar.gz","size":{},"digest":"sha256:{digest}"}}]}}"#,
            tarball.len(),
        ),
    );
    fake.answer(&format!("{download}/module.toml"), manifest.to_owned());
    fake.answer(&format!("{download}/bateria-1.1.0.tar.gz"), tarball);
    fake
}

#[test]
fn automatic_updates_leave_switched_off_and_failed_modules_alone() {
    let newer = MANIFEST.replace("1.0.0", "1.1.0");
    for change in [
        (|r: &mut Record| r.enabled = false) as fn(&mut Record),
        |r: &mut Record| r.failed = true,
        |r: &mut Record| r.consent = false,
    ] {
        let root = tempfile::tempdir().unwrap();
        installed_bateria(root.path());
        let path = root.path().join("modules.toml");
        let mut state = State::load(&path);
        change(state.modules.get_mut("bateria").unwrap());
        state.save(&path).unwrap();
        assert!(shop(root.path(), &release_1_1(&newer, true))
            .auto("bateria")
            .is_none());
        let on_disk = fs::read_to_string(root.path().join("modules/bateria/module.toml")).unwrap();
        assert!(on_disk.contains("1.0.0"));
    }
}

#[test]
fn a_new_surface_or_a_process_that_stays_open_is_asked_for() {
    let oneshot = MANIFEST.replace("mode = \"daemon\"", "mode = \"oneshot\"");
    let root = tempfile::tempdir().unwrap();
    installed_bateria(root.path());
    // As if 1.0.0 had been a one-shot.
    fs::write(root.path().join("modules/bateria/module.toml"), &oneshot).unwrap();
    let newer = MANIFEST.replace("1.0.0", "1.1.0").replace(
        "[[start.tiles]]",
        "[[start.widgets]]\nid = \"w\"\naction = \"refresh\"\n[[start.tiles]]",
    );
    let offer = shop(root.path(), &release_1_1(&newer, true))
        .offer(&card())
        .unwrap();
    let added: Vec<&str> = offer.added.iter().map(|p| p.key).collect();
    assert_eq!(added, ["modules.perm_widget", "modules.perm_daemon"]);
}

#[test]
fn a_rolled_back_package_is_never_taken_again_on_its_own() {
    let newer = MANIFEST.replace("1.0.0", "1.1.0");
    let root = tempfile::tempdir().unwrap();
    installed_bateria(root.path());
    let fake = release_1_1(&newer, true);
    let tarball = Tarball::module(&newer).bytes();
    let path = root.path().join("modules.toml");
    let mut state = State::load(&path);
    state.modules.get_mut("bateria").unwrap().bad_digest = format!("sha256:{}", sha(&tarball));
    state.save(&path).unwrap();
    assert!(
        shop(root.path(), &fake).auto("bateria").is_none(),
        "no daily loop"
    );
}

#[test]
fn automatic_updates_take_only_reviewed_versions_that_ask_for_nothing_new() {
    let newer = MANIFEST.replace("1.0.0", "1.1.0");
    let asks = newer.replace(
        "[[start.tiles]]",
        "[permissions]\nnet = true\n[[start.tiles]]",
    );
    // A new place on the screen, or staying open, is new as well.
    let tile = format!("{newer}[[start.tiles]]\nid = \"c\"\naction = \"refresh\"\n");
    for (manifest, reviewed, updated) in [
        (&newer, false, false),
        (&asks, true, false),
        (&tile, true, false),
        (&newer, true, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        installed_bateria(root.path());
        let done = shop(root.path(), &release_1_1(manifest, reviewed)).auto("bateria");
        assert_eq!(done.is_some(), updated, "reviewed {reviewed}: {manifest}");
        let on_disk = fs::read_to_string(root.path().join("modules/bateria/module.toml")).unwrap();
        assert_eq!(on_disk.contains("1.1.0"), updated);
        if updated {
            assert_eq!(done.unwrap().version, "1.1.0");
            assert!(
                root.path().join("modules/.previous-bateria").is_dir(),
                "the old version waits for the new one's first good run"
            );
        }
    }
}

/// The module host with this store, as on a device with automatic updates
/// on; daemons run plainly.
fn host(
    root: &Path,
    fake: &Fake,
    auto_delay: std::time::Duration,
) -> archr_launcher::modules::Modules {
    use archr_launcher::modules::{process::Direct, Config, Modules};
    fs::write(root.join("system.cfg"), "launcher.modules.autoupdate=1\n").unwrap();
    let direct = Arc::new(Direct::default());
    Modules::start(Config {
        dirs: Dirs::new(root),
        run: root.join("run"),
        system_cfg: root.join("system.cfg"),
        lock: root.join("system.cfg.lock"),
        executor: direct.clone(),
        direct: Some(direct),
        userdb: None,
        language: "en".into(),
        device: "test".into(),
        delay: std::time::Duration::ZERO,
        auto_delay,
        firewall: None,
        shop: Some(shop(root, fake)),
    })
}
/// Store answers and notices until `done` holds, at most 10 s.
fn until_store(
    modules: &archr_launcher::modules::Modules,
    done: impl Fn(&[StoreReply], &[archr_launcher::modules::Notice]) -> bool,
) -> (Vec<StoreReply>, Vec<archr_launcher::modules::Notice>) {
    use archr_launcher::modules::Event;
    let (mut replies, mut notices) = (Vec::new(), Vec::new());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done(&replies, &notices) {
        assert!(
            std::time::Instant::now() < deadline,
            "{replies:?} {notices:?}"
        );
        for event in modules.poll() {
            match event {
                Event::Store(reply) => replies.push(reply),
                Event::Notice(notice) => notices.push(notice),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    (replies, notices)
}
fn version_on_disk(root: &Path) -> String {
    let toml = fs::read_to_string(root.join("modules/bateria/module.toml")).unwrap();
    toml.lines()
        .find(|l| l.starts_with("version"))
        .unwrap_or("")
        .to_owned()
}

#[test]
fn the_host_updates_on_its_own_only_outside_games_and_keeps_the_switch() {
    use archr_launcher::modules::Notice;
    let newer = MANIFEST.replace("1.0.0", "1.1.0");
    let root = tempfile::tempdir().unwrap();
    installed_bateria(root.path());
    let modules = host(
        root.path(),
        &release_1_1(&newer, true),
        std::time::Duration::from_millis(400),
    );
    // A game takes the screen before the first look: nothing happens.
    modules.hand_off("", "", "");
    std::thread::sleep(std::time::Duration::from_millis(1200));
    assert!(version_on_disk(root.path()).contains("1.0.0"));
    modules.resume();
    let updated =
        |_: &[StoreReply], n: &[Notice]| n.iter().any(|n| matches!(n, Notice::Updated(..)));
    until_store(&modules, updated);
    assert!(version_on_disk(root.path()).contains("1.1.0"));
    let record = State::load(&root.path().join("modules.toml")).modules["bateria"].clone();
    assert!(record.enabled && record.consent && record.auto_updated > 0);
    assert!(record.previous_digest.starts_with("sha256:"));
}

#[test]
fn the_host_leaves_switched_off_and_rolled_back_modules_alone() {
    let newer = MANIFEST.replace("1.0.0", "1.1.0");
    let tarball = Tarball::module(&newer).bytes();
    for off in [true, false] {
        let root = tempfile::tempdir().unwrap();
        installed_bateria(root.path());
        let path = root.path().join("modules.toml");
        let mut state = State::load(&path);
        let record = state.modules.get_mut("bateria").unwrap();
        if off {
            record.enabled = false;
        } else {
            record.bad_digest = format!("sha256:{}", sha(&tarball));
        }
        state.save(&path).unwrap();
        let modules = host(
            root.path(),
            &release_1_1(&newer, true),
            std::time::Duration::ZERO,
        );
        if off {
            // Not even asked about: give the host its first look.
            std::thread::sleep(std::time::Duration::from_millis(800));
            let (replies, notices) = until_store(&modules, |_, _| true);
            assert!(
                replies.is_empty() && notices.is_empty(),
                "{replies:?} {notices:?}"
            );
        } else {
            let (_, notices) = until_store(&modules, |r, _| {
                r.iter()
                    .any(|r| matches!(r, StoreReply::Checked(id) if id == "bateria"))
            });
            assert!(notices.is_empty(), "{notices:?}");
        }
        assert!(version_on_disk(root.path()).contains("1.0.0"));
        assert_eq!(State::load(&path).modules["bateria"].enabled, !off);
    }
}

#[test]
fn only_a_list_signed_by_the_archr_key_and_not_older_gives_badges() {
    use archr_launcher::modules::store::{signed, REVIEW_KEY};
    // The list tools/sign-reviewed signed with the real key.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let list = fs::read(fixtures.join("reviewed.json")).unwrap();
    let signature = fs::read(fixtures.join("reviewed.json.sig")).unwrap();
    assert!(signed(&list, &signature, &REVIEW_KEY));
    let mut changed = list.clone();
    changed[2] ^= 1;
    assert!(!signed(&changed, &signature, &REVIEW_KEY));
    assert!(!signed(&list, b"", &REVIEW_KEY));
    assert!(!signed(&list, &signature[..100], &REVIEW_KEY));
    // Through the store: a list signed by another key, or none, reviews
    // nothing and says why.
    let tarball = Tarball::module(MANIFEST).bytes();
    let digest = sha(&tarball);
    let body = format!(
        r#"{{"serial":3,"modules":[{{"repo":"kaio-dev/archr-bateria","digest":"sha256:{digest}"}}]}}"#
    );
    let stranger = {
        use ed25519_dalek::Signer;
        let key = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
        let bytes = key.sign(body.as_bytes()).to_bytes();
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    for (signature, problem) in [
        (sign(body.as_bytes()), None),
        (stranger, Some("modules.review_unsigned")),
        (String::new(), Some("modules.review_unsigned")),
    ] {
        let root = tempfile::tempdir().unwrap();
        let fake = Fake::default();
        publish(&fake, MANIFEST, &tarball, &digest, &digest);
        review(&fake, body.as_bytes(), &signature);
        let store = shop(root.path(), &fake);
        let StoreReply::Listed {
            cards,
            problem: said,
            ..
        } = store.list(Tab::All, false)
        else {
            panic!("not a list");
        };
        assert_eq!(said, problem);
        let bateria = cards.iter().find(|c| c.repo == "archr-bateria").unwrap();
        assert_eq!(bateria.reviewed, problem.is_none());
        assert_eq!(store.offer(&card()).unwrap().reviewed, problem.is_none());
    }
    // An older list than one already seen: refused, nothing reviewed.
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    publish(&fake, MANIFEST, &tarball, &digest, &digest);
    assert!(shop(root.path(), &fake).offer(&card()).unwrap().reviewed);
    fs::remove_dir_all(root.path().join("cache")).unwrap();
    let old = body.replace("\"serial\":3", "\"serial\":2");
    review(&fake, old.as_bytes(), &sign(old.as_bytes()));
    let StoreReply::Listed { problem, .. } = shop(root.path(), &fake).list(Tab::All, false) else {
        panic!("not a list");
    };
    assert_eq!(problem, Some("modules.review_old"));
}
