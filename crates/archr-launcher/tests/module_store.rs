//! Module store against canned GitHub answers and hand-made tarballs: no
//! test ever reaches the network.
use archr_launcher::modules::{
    registry::{Dirs, Record, State},
    store::{Shop, HOSTS},
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
    fake.answer(
        &format!("{API}/archr-linux/archr-modules/releases/latest"),
        r#"{"tag_name":"v3","assets":[{"name":"reviewed.json","size":100}]}"#,
    );
    fake.answer(
        "https://github.com/archr-linux/archr-modules/releases/download/v3/reviewed.json",
        format!(
            r#"{{"modules":[{{"repo":"kaio-dev/archr-bateria","digest":"sha256:{reviewed}"}}]}}"#
        ),
    );
}
fn shop(root: &Path, fake: &Fake) -> Shop {
    Shop::new(Box::new(fake.clone()), root.join("cache"), Dirs::new(root))
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
    // A new release under the same version: the badge goes.
    let other = Tarball::module(MANIFEST)
        .file("bateria/x.txt", b"x")
        .bytes();
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    publish(&fake, MANIFEST, &other, &sha(&other), &sha(&tarball));
    assert!(!shop(root.path(), &fake).offer(&card).unwrap().reviewed);
}

#[test]
fn an_install_checks_digest_manifest_and_paths_before_anything_lands() {
    let root = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    let good = Tarball::module(MANIFEST).bytes();
    publish(&fake, MANIFEST, &good, &sha(&good), "");
    let store = shop(root.path(), &fake);
    let offer = store.offer(&card()).unwrap();
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
        assert_eq!(store.install(&offer), Err(problem));
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
