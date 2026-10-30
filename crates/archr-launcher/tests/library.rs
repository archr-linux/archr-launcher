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
    let config = dir.path().join("storage/.config/archr");
    fs::create_dir_all(&config).unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    let roms = paths.at("/storage/roms/nes");
    fs::create_dir_all(&roms).unwrap();
    fs::write(roms.join("A.nes"), []).unwrap();
    fs::write(config.join("systems.cfg"), "<systemList><system><name>nes</name><fullname>Nintendo</fullname><path>/storage/roms/nes</path><extension>.nes</extension><emulators><emulator name=\"retroarch\"><cores><core default=\"true\">fceumm</core><core>nestopia</core></cores></emulator></emulators></system></systemList>").unwrap();
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
fn ready(paths: &Paths, system: Option<&str>) -> Library {
    let mut library = Library::initial(paths.clone());
    library.start();
    wait(&mut library, |l| l.ready);
    if let Some(system) = system {
        library.ensure(system);
        wait(&mut library, |l| !l.loading(system));
    }
    library
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
    assert_eq!(App::system_rows(&library), ["@favorites", "nes"]);
    library.ensure("@favorites");
    wait(&mut library, |library| {
        library.games("@favorites").len() == 1
    });
    assert_eq!(library.origin("@favorites", 0), Some(("nes".into(), 0)));
    assert_eq!(
        library.current_core("nes", 0),
        Some(("retroarch".into(), "nestopia".into()))
    );
    assert_eq!(App::system_rows(&library), ["@favorites", "nes"]);
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
    let mut library = ready(&paths, Some("nes"));
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
fn mangohud_per_game_is_saved_under_the_rom_and_removed_to_follow_the_system() {
    let (_dir, paths) = fixture();
    fs::write(
        &paths.config,
        "nes.archr.mangohud.enabled=1\narchr.mangohud.enabled=0\n",
    )
    .unwrap();
    let mut library = ready(&paths, Some("nes"));
    assert_eq!(library.mangohud("nes", 0), (String::new(), "1".into()));
    for (value, line) in [
        ("0", Some("nes[\"A.nes\"].archr.mangohud.enabled=0")),
        ("", None),
    ] {
        library
            .handle(
                Outcome::SetMangohud {
                    system: "nes".into(),
                    game: 0,
                    value: value.into(),
                },
                "",
            )
            .unwrap();
        assert_eq!(library.mangohud("nes", 0).0, value);
        wait(&mut library, |library| library.pending_writes == 0);
        assert!(!library.write_failed);
        let saved = fs::read_to_string(&paths.config).unwrap();
        assert_eq!(saved.contains("nes[\"A.nes\"]"), line.is_some(), "{saved}");
        if let Some(line) = line {
            assert!(saved.contains(line), "{saved}");
        }
    }
}

#[test]
fn launch_requires_final_store_flush_before_publishing_next() {
    let (_dir, paths) = fixture();
    let mut library = ready(&paths, Some("nes"));
    library
        .handle(
            Outcome::ToggleFavorite {
                system: "nes".into(),
                game: 0,
            },
            "",
        )
        .unwrap();
    let launch = Outcome::Launch {
        system: "nes".into(),
        game: 0,
    };
    assert!(library.handle(launch.clone(), "").unwrap());
    assert!(Store::load(&paths.store).favorites().is_empty());
    fs::remove_file(paths.run.join("next")).unwrap();
    // A failed final flush must not publish a new instruction.
    fs::remove_file(paths.store.join("store.toml")).unwrap();
    fs::create_dir(paths.store.join("store.toml")).unwrap();
    assert!(library.handle(launch, "").is_err());
    assert!(!paths.run.join("next").exists());
}

#[test]
fn legacy_absolute_favorites_and_recents_migrate_once() {
    let (_dir, paths) = fixture();
    let mut store = Store::load(&paths.store);
    store.push_recent(&format!(
        "nes/{}",
        paths.at("/storage/roms/nes/A.nes").display()
    ));
    store.save(&paths.store).unwrap();
    for _ in 0..2 {
        let mut library = ready(&paths, None);
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

#[test]
fn new_favorites_and_recents_use_relative_identity() {
    use std::os::unix::fs::symlink;
    for keys in [
        &["nes/A.nes"][..],
        &["nes/one/A.nes", "nes/two/A.nes"],
        &["nes/one/A.nes", "nes/Link.nes"],
    ] {
        for legacy in [false, true] {
            let (_dir, paths) = fixture();
            let roms = paths.at("/storage/roms/nes");
            fs::remove_file(roms.join("A.nes")).unwrap();
            let mut xml = String::from("<gameList>");
            let mut store = Store::default();
            for (index, key) in keys.iter().enumerate() {
                let relative = key.strip_prefix("nes/").unwrap();
                let path = roms.join(relative);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                if relative == "Link.nes" {
                    symlink("one/A.nes", &path).unwrap();
                } else {
                    fs::write(&path, []).unwrap();
                }
                if legacy {
                    let old = format!("nes/{}", path.display());
                    store.toggle_favorite(&old);
                    store.push_recent(&old);
                }
                xml.push_str(&format!(
                    "<game><path>{relative}</path><name>{index}</name></game>"
                ));
            }
            fs::write(roms.join("gamelist.xml"), format!("{xml}</gameList>")).unwrap();
            if legacy {
                // Mixed generations collapse without changing first-seen history order.
                store.toggle_favorite(keys[0]);
            }
            store.save(&paths.store).unwrap();
            let mut library = ready(&paths, Some("nes"));
            if !legacy {
                for game in 0..keys.len() {
                    for outcome in [
                        Outcome::ToggleFavorite {
                            system: "nes".into(),
                            game,
                        },
                        Outcome::Launch {
                            system: "nes".into(),
                            game,
                        },
                    ] {
                        library.handle(outcome, "").unwrap();
                    }
                }
            }
            library.finish().unwrap();
            let store = Store::load(&paths.store);
            assert_eq!(store.favorites(), keys);
            let recent: Vec<_> = keys.iter().rev().copied().collect();
            assert_eq!(store.recent(), recent);
        }
    }
}

#[test]
fn collections_update_only_changed_members_and_keep_order() {
    let (_dir, paths) = fixture();
    let roms = paths.at("/storage/roms/nes");
    for name in ["B.nes", "C.nes"] {
        fs::write(roms.join(name), []).unwrap();
    }
    let mut library = ready(&paths, Some("nes"));
    for collection in ["@favorites", "@recent"] {
        library.ensure(collection);
    }
    for (game, favorite, favorites, recent) in [
        (1, true, vec![0, 1], vec![]),
        (0, false, vec![0, 1], vec![0]),
        (1, false, vec![0, 1], vec![1, 0]),
        (0, true, vec![1], vec![1, 0]),
        (1, false, vec![1], vec![1, 0]),
        (0, true, vec![1, 0], vec![1, 0]),
    ] {
        // Unchanged entries must retain their owned payloads, not be cloned.
        let unchanged = library
            .games("@favorites")
            .iter()
            .find(|entry| entry.path == roms.join("B.nes"))
            .map(|entry| entry.name.as_ptr());
        let outcome = if favorite {
            Outcome::ToggleFavorite {
                system: "nes".into(),
                game,
            }
        } else {
            Outcome::Launch {
                system: "nes".into(),
                game,
            }
        };
        library.handle(outcome, "").unwrap();
        for (name, expected) in [("@favorites", favorites), ("@recent", recent)] {
            let actual: Vec<_> = (0..library.games(name).len())
                .map(|i| library.origin(name, i).unwrap().1)
                .collect();
            assert_eq!(actual, expected, "{name}");
            for entry in library.games(name) {
                let source = library
                    .games("nes")
                    .iter()
                    .find(|game| game.path == entry.path)
                    .unwrap();
                assert_eq!(entry.favorite, source.favorite);
            }
        }
        if let Some(ptr) = unchanged {
            assert_eq!(
                library
                    .games("@favorites")
                    .iter()
                    .find(|entry| entry.path == roms.join("B.nes"))
                    .unwrap()
                    .name
                    .as_ptr(),
                ptr
            );
        }
    }
}

#[test]
fn session_actions_cannot_publish_next() {
    use archr_launcher::state::SessionAction::{Reboot, Shutdown, Suspend};
    let (_root, paths) = fixture();
    let mut library = ready(&paths, None);
    for action in [Shutdown, Reboot, Suspend] {
        assert!(library.handle(Outcome::Action(action), "").is_err());
        assert!(!paths.run.join("next").exists());
    }
}

#[test]
fn invalid_store_is_visible_and_never_replaced_on_exit() {
    let (_dir, paths) = fixture();
    let path = paths.store.join("store.toml");
    fs::write(&path, "broken [").unwrap();
    let library = ready(&paths, None);
    assert_eq!(
        library.banner.as_ref().map(|banner| banner.key),
        Some("error.store_load")
    );
    library.finish().unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "broken [");
}

#[test]
fn system_order_is_saved_off_thread_reloaded_and_default_order_clears_it() {
    let (_dir, paths) = fixture();
    let mut library = ready(&paths, None);
    let custom = ["nes", "@favorites"].map(String::from).to_vec();
    assert!(!library
        .handle(Outcome::SetOrder(custom.clone()), "")
        .unwrap());
    assert_eq!(App::system_rows(&library), ["nes", "@favorites"]);
    library.finish().unwrap();
    assert_eq!(Store::load(&paths.store).order(), custom);
    let mut reopened = ready(&paths, None);
    assert_eq!(App::system_rows(&reopened), ["nes", "@favorites"]);
    // Saving the default arrangement is the same as having no custom order.
    let default = ["@favorites", "nes"].map(String::from).to_vec();
    reopened.handle(Outcome::SetOrder(default), "").unwrap();
    assert!(reopened.system_order().is_empty());
    reopened.finish().unwrap();
    assert!(Store::load(&paths.store).order().is_empty());
}

#[test]
fn saved_order_is_known_before_the_worker_starts() {
    let (_dir, paths) = fixture();
    let mut store = Store::load(&paths.store);
    store.set_order(["nes", "@favorites"].map(String::from).into(), |_| true);
    store.save(&paths.store).unwrap();
    let library = Library::initial(paths);
    assert!(!library.ready);
    assert_eq!(library.system_order(), ["nes", "@favorites"]);
}

#[test]
fn restore_from_the_menu_clears_the_persisted_order() {
    use archr_launcher::state::{Button, QuickItem};
    let (_dir, paths) = fixture();
    let mut library = ready(&paths, None);
    library
        .handle(
            Outcome::SetOrder(["nes", "@favorites"].map(String::from).into()),
            "",
        )
        .unwrap();
    let mut app = App::new(5);
    app.handle(Button::Y, &library);
    assert_eq!(app.quick_menu_items(&library)[2], QuickItem::RestoreOrder);
    app.handle(Button::Down, &library);
    app.handle(Button::Down, &library);
    let outcome = app.handle(Button::A, &library);
    library.handle(outcome, "").unwrap();
    library.finish().unwrap();
    assert_eq!(App::system_rows(&library), ["@favorites", "nes"]);
    assert!(Store::load(&paths.store).order().is_empty());
}

#[test]
fn confirming_an_order_before_boot_keeps_favorites_and_survives_boot() {
    let (_dir, paths) = fixture();
    let mut library = Library::initial(paths.clone());
    library.start();
    assert!(!library.ready);
    let custom = ["nes", "@favorites"].map(String::from).to_vec();
    library
        .handle(Outcome::SetOrder(custom.clone()), "")
        .unwrap();
    wait(&mut library, |l| l.ready);
    assert_eq!(
        library.system_order(),
        custom,
        "Boot must not undo the move"
    );
    assert_eq!(library.favorite_count("nes"), 1);
    library.finish().unwrap();
    let saved = Store::load(&paths.store);
    assert_eq!(saved.order(), custom);
    assert_eq!(saved.favorites().len(), 1);
}

#[test]
fn invalid_store_banner_is_shown_from_the_initial_library() {
    let (_dir, paths) = fixture();
    fs::write(paths.store.join("store.toml"), "favorites = [").unwrap();
    let library = Library::initial(paths);
    assert!(library.banner.is_some());
}

#[test]
fn systems_without_emulators_launch_their_catalog_command() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    let tools = paths.at("/storage/.config/modules");
    fs::create_dir_all(&tools).unwrap();
    fs::write(tools.join("Mount NFS.sh"), []).unwrap();
    let music = paths.at("/storage/roms/music");
    fs::create_dir_all(&music).unwrap();
    fs::write(music.join("Song.rom"), []).unwrap();
    let catalog = paths.at("/storage/.config/archr/systems.cfg");
    fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    fs::write(&catalog, "<systemList><system><name>tools</name><fullname>Tools</fullname><path>/storage/.config/modules</path><extension>.sh</extension><command>/usr/bin/foot %ROM%</command></system><system><name>music</name><fullname>Music</fullname><path>/storage/roms/music</path><extension>.rom</extension></system></systemList>").unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    let mut old = Store::default();
    old.push_recent("tools/Old.sh");
    old.push_recent("music/Song.rom");
    old.save(&paths.store).unwrap();
    let mut library = ready(&paths, Some("tools"));
    assert_eq!(
        library.collection_len("@recent"),
        1,
        "old tool recents dropped"
    );
    let launch = |system: &str| Outcome::Launch {
        system: system.into(),
        game: 0,
    };
    assert!(library.is_tools("tools") && !library.is_tools("music"));
    assert!(library.handle(launch("tools"), "pad").unwrap());
    assert_eq!(
        library.collection_len("@recent"),
        1,
        "tools are not recents"
    );
    let rom = tools.join("Mount NFS.sh");
    assert_eq!(
        fs::read_to_string(paths.run.join("next")).unwrap(),
        format!(
            "game ARCHR_ROM='{}'; ARCHR_BASENAME='Mount NFS'; ARCHR_SYSTEM='tools'; \
             export ARCHR_ROM ARCHR_BASENAME ARCHR_SYSTEM; /usr/bin/foot \"$ARCHR_ROM\"\n",
            rom.display()
        )
    );
    fs::remove_file(paths.run.join("next")).unwrap();
    // Neither emulators nor a command: nothing is published.
    library.ensure("music");
    wait(&mut library, |l| !l.loading("music"));
    assert_eq!(
        library.handle(launch("music"), "").unwrap_err(),
        "error.no_core"
    );
    assert!(!paths.run.join("next").exists());
}

#[test]
fn a_command_that_wraps_an_emulator_is_not_tools() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(Some(dir.path().to_owned()));
    for name in ["ports", "other"] {
        let roms = paths.at(&format!("/storage/roms/{name}"));
        fs::create_dir_all(&roms).unwrap();
        fs::write(roms.join("G.sh"), []).unwrap();
    }
    let catalog = paths.at("/storage/.config/archr/systems.cfg");
    fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    fs::write(&catalog, "<systemList><system><name>ports</name><fullname>Ports</fullname><path>/storage/roms/ports</path><extension>.sh</extension><command>/usr/bin/runemu.sh %ROM% -P%SYSTEM%</command></system><system><name>other</name><fullname>Other</fullname><path>/storage/roms/other</path><extension>.sh</extension><command>/usr/bin/x --emu %EMULATOR% %ROM%</command></system></systemList>").unwrap();
    let library = ready(&paths, None);
    assert!(!library.is_tools("ports") && !library.is_tools("other"));
}
#[test]
fn a_system_core_saved_by_the_settings_applies_to_the_next_launch() {
    let (_dir, paths) = fixture();
    fs::write(&paths.config, "nes.core=fceumm\n").unwrap();
    let mut library = ready(&paths, Some("nes"));
    assert_eq!(
        library.current_core("nes", 0),
        Some(("retroarch".into(), "fceumm".into()))
    );
    fs::write(&paths.config, "nes.core=nestopia\n").unwrap();
    library.reload_config();
    wait(&mut library, |l| {
        l.current_core("nes", 0) == Some(("retroarch".into(), "nestopia".into()))
    });
}

#[test]
fn a_launch_from_a_state_backs_up_the_cartridge_save_then_names_the_slot() {
    use archr_launcher::saves::{scan, Places};
    let (_dir, paths) = fixture();
    let mut library = ready(&paths, Some("nes"));
    let roms = paths.at("/storage/roms/nes");
    let states = paths.at("/storage/roms/savestates/nes");
    fs::create_dir_all(&states).unwrap();
    fs::write(roms.join("A.srm"), "progress").unwrap();
    fs::write(roms.join("A.srm.bak"), "older copy").unwrap();
    fs::write(states.join("A.state3"), "state").unwrap();
    fs::write(states.join("A.state3.png"), "picture").unwrap();
    let game = library.progress_game("nes", 0).unwrap();
    let progress = scan(&Places::new(&paths), &game);
    let save = &progress.saves[0];
    assert_eq!((save.slot, save.playable), (Some(3), true));
    assert_eq!(save.emulator, "RetroArch · nestopia", "the game's own core");
    library.entry = archr_launcher::saves::Entry::of(save);
    let launch = Outcome::Launch {
        system: "nes".into(),
        game: 0,
    };
    assert!(library.handle(launch.clone(), "").unwrap());
    let next = fs::read_to_string(paths.run.join("next")).unwrap();
    assert!(next.trim_end().ends_with(" '--entryslot=3'"), "{next}");
    assert_eq!(
        fs::read_to_string(roms.join("A.srm.bak")).unwrap(),
        "progress"
    );
    // The entry is for one launch only.
    fs::remove_file(paths.run.join("next")).unwrap();
    assert!(library.handle(launch.clone(), "").unwrap());
    let next = fs::read_to_string(paths.run.join("next")).unwrap();
    assert!(!next.contains("--entry"), "{next}");
    // No copy, no game: a cartridge save that cannot be read stops the launch.
    fs::remove_file(paths.run.join("next")).unwrap();
    let mut entry = archr_launcher::saves::Entry::of(save).unwrap();
    entry.backup = vec![roms.join("missing.srm")];
    library.entry = Some(entry);
    assert_eq!(library.handle(launch, "").unwrap_err(), "error.save_backup");
    assert!(!paths.run.join("next").exists());
}
