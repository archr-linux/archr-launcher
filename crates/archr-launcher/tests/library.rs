use archr_index::{cache, load_systems};
use archr_launcher::{
    library::Library,
    paths::Paths,
    state::{App, DataSource, Outcome},
    store::Store,
};
use std::{
    fs,
    time::{Duration, Instant},
};

fn fixture() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("storage/.config/emulationstation");
    fs::create_dir_all(&config).unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    let roms = paths.at("/storage/roms/nes");
    fs::create_dir_all(&roms).unwrap();
    fs::write(roms.join("A.nes"), []).unwrap();
    fs::write(config.join("es_systems.cfg"), "<systemList><system><name>nes</name><fullname>Nintendo</fullname><path>/storage/roms/nes</path><extension>.nes</extension><emulators><emulator name=\"retroarch\"><cores><core default=\"true\">fceumm</core><core>nestopia</core></cores></emulator></emulators></system></systemList>").unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    let mut definitions = load_systems(&paths.systems).unwrap();
    definitions[0].path = roms.clone();
    cache::save_overview(&paths.cache, &cache::build_overview(&definitions)).unwrap();
    let mut store = Store::default();
    store.toggle_favorite(&format!("nes/{}", roms.join("A.nes").display()));
    store.save(&paths.store).unwrap();
    fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    fs::write(&paths.config, "global.core=wrong\nnes.emulator=retroarch\nnes.core=fceumm\nnes[\"A.nes\"].core=nestopia\n").unwrap();
    (dir, paths)
}
fn wait(library: &mut Library, condition: impl Fn(&Library) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        library.poll();
        if condition(library) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background library did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn warm_overview_precedes_worker_and_collection_origins_are_real() {
    let (_dir, paths) = fixture();
    let mut library = Library::initial(paths.clone());
    assert_eq!(library.systems()[0].fullname, "Nintendo");
    assert!(library.games("nes").is_empty());
    assert!(library.games("@favorites").is_empty());
    assert!(!paths.cache.join("systems").exists());
    library.start();
    wait(&mut library, |library| library.ready);
    assert!(
        library.games("nes").is_empty(),
        "overview must not load Store-referenced systems"
    );
    assert!(!paths.cache.join("systems").exists());
    assert_eq!(App::new(10).system_rows(&library), ["@favorites", "nes"]);
    library.ensure("@favorites");
    wait(&mut library, |library| {
        library.games("@favorites").len() == 1
    });
    assert_eq!(library.origin("@favorites", 0), Some(("nes".into(), 0)));
    assert_eq!(
        library.current_core("nes", 0),
        Some(("retroarch".into(), "nestopia".into()))
    );
    assert_eq!(App::new(10).system_rows(&library), ["@favorites", "nes"]);
    assert!(library
        .handle(
            Outcome::Launch {
                system: "nes".into(),
                game: 0
            },
            " -p1index 2 -p1guid abc "
        )
        .unwrap());
    let next = fs::read_to_string(paths.run.join("next")).unwrap();
    assert!(next.contains("--core=nestopia"));
    assert!(next.contains("--controllers= -p1index 2 -p1guid abc "));
    assert_eq!(Store::load(&paths.store).recent().len(), 1);
}
#[test]
fn core_lock_is_waited_in_worker_not_in_ui_call() {
    let (_dir, paths) = fixture();
    let mut library = Library::initial(paths.clone());
    library.start();
    wait(&mut library, |library| library.ready);
    library.ensure("nes");
    wait(&mut library, |library| !library.games("nes").is_empty());
    fs::create_dir_all(paths.lock.parent().unwrap()).unwrap();
    fs::write(&paths.lock, std::process::id().to_string()).unwrap();
    library
        .handle(
            Outcome::SetCore {
                system: "nes".into(),
                game: 0,
                emulator: "retroarch".into(),
                core: "fceumm".into(),
            },
            "",
        )
        .unwrap();
    assert_eq!(library.pending_writes, 1);
    assert!(paths.lock.exists());
    // A synchronous implementation times out on our live PID instead of queuing.
    fs::remove_file(&paths.lock).unwrap();
    wait(&mut library, |library| library.pending_writes == 0);
    assert!(!library.write_failed);
    assert!(fs::read_to_string(paths.config)
        .unwrap()
        .contains("nes[\"A.nes\"].core=fceumm"));
}

#[test]
fn action_flushes_latest_favorites_before_publishing_next() {
    use archr_launcher::state::SessionAction;
    let (_dir, paths) = fixture();
    let mut library = Library::initial(paths.clone());
    library.start();
    wait(&mut library, |library| library.ready);
    library.ensure("nes");
    wait(&mut library, |library| !library.games("nes").is_empty());
    library
        .handle(
            Outcome::ToggleFavorite {
                system: "nes".into(),
                game: 0,
            },
            "",
        )
        .unwrap();
    assert!(library
        .handle(Outcome::Action(SessionAction::OpenSettings), "")
        .unwrap());
    assert!(paths.run.join("next").exists());
    assert!(Store::load(&paths.store).favorites().is_empty());
}
#[test]
fn failed_final_store_flush_does_not_publish_next() {
    use archr_launcher::state::SessionAction;
    let (_dir, paths) = fixture();
    let mut library = Library::initial(paths.clone());
    library.start();
    wait(&mut library, |library| library.ready);
    // No game import/save has been queued. Force every store rename to fail.
    fs::remove_file(paths.store.join("store.toml")).unwrap();
    fs::create_dir(paths.store.join("store.toml")).unwrap();
    assert!(library
        .handle(Outcome::Action(SessionAction::OpenSettings), "")
        .is_err());
    assert!(!paths.run.join("next").exists());
}

#[test]
#[ignore = "step 4 / item 8: keys must be relative to each system directory"]
fn new_favorites_and_recents_use_relative_identity() {
    let (_dir, paths) = fixture();
    Store::default().save(&paths.store).unwrap();
    let mut library = Library::initial(paths.clone());
    library.start();
    wait(&mut library, |l| l.ready);
    library.ensure("nes");
    wait(&mut library, |l| !l.games("nes").is_empty());
    library
        .handle(
            Outcome::ToggleFavorite {
                system: "nes".into(),
                game: 0,
            },
            "",
        )
        .unwrap();
    library
        .handle(
            Outcome::Launch {
                system: "nes".into(),
                game: 0,
            },
            "",
        )
        .unwrap();
    let store = Store::load(&paths.store);
    assert_eq!(store.favorites(), ["nes/A.nes"]);
    assert_eq!(store.recent(), ["nes/A.nes"]);
}

#[test]
#[ignore = "step 4 / item 8: migrate legacy absolute identities idempotently"]
fn legacy_absolute_favorites_and_recents_migrate_once() {
    let (_dir, paths) = fixture();
    let mut store = Store::load(&paths.store);
    store.push_recent(&format!(
        "nes/{}",
        paths.at("/storage/roms/nes/A.nes").display()
    ));
    store.save(&paths.store).unwrap();
    for _ in 0..2 {
        let mut library = Library::initial(paths.clone());
        library.start();
        wait(&mut library, |l| l.ready);
        for collection in ["@favorites", "@recent"] {
            library.ensure(collection);
        }
        wait(&mut library, |l| {
            l.games("@favorites").len() == 1 && l.games("@recent").len() == 1
        });
        assert_eq!(library.origin("@favorites", 0), Some(("nes".into(), 0)));
        library.finish().unwrap();
        let store = Store::load(&paths.store);
        assert_eq!(store.favorites(), ["nes/A.nes"]);
        assert_eq!(store.recent(), ["nes/A.nes"]);
    }
}
