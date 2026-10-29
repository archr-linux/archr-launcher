use archr_launcher::{handoff::launch_command_with, paths::Paths};
use std::path::Path;

#[test]
fn root_prefix_includes_user_config_lock_run_and_executable() {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::new(Some(root.path().to_owned()));
    assert_eq!(paths.lock, root.path().join("tmp/.system.cfg.lock"));
    assert_eq!(paths.run, root.path().join("run/archr-launcher"));
    assert_eq!(
        paths.remap(Path::new("/storage/roms/nes")),
        root.path().join("storage/roms/nes")
    );
    let executable = paths.at("/usr/bin/runemu.sh");
    let command = launch_command_with(
        &executable,
        Path::new("/rom/Game.nes"),
        "nes",
        "nestopia",
        "retroarch",
        "",
    )
    .unwrap();
    assert!(command.starts_with(&format!("'{}' ", executable.display())));
}

#[test]
fn systems_catalog_prefers_the_user_copy_and_never_reads_es_catalogs() {
    use archr_launcher::paths::CATALOGS;
    let root = tempfile::tempdir().unwrap();
    let at = |path: &str| root.path().join(path.trim_start_matches('/'));
    let install = |path: &str| {
        std::fs::create_dir_all(at(path).parent().unwrap()).unwrap();
        std::fs::write(at(path), "<systemList/>").unwrap();
    };
    // Only ES catalogs installed: report the packaged ArchR catalog as missing.
    for es in [
        "/storage/.config/emulationstation/es_systems.cfg",
        "/usr/config/emulationstation/es_systems.cfg",
    ] {
        install(es);
    }
    assert_eq!(
        Paths::new(Some(root.path().to_owned())).systems,
        at("/usr/share/archr/systems.cfg")
    );
    // Install lowest precedence first; each newer file must win.
    for path in CATALOGS.iter().rev() {
        install(path);
        assert_eq!(Paths::new(Some(root.path().to_owned())).systems, at(path));
    }
    // A directory never shadows a real catalog.
    std::fs::remove_file(at(CATALOGS[0])).unwrap();
    std::fs::create_dir(at(CATALOGS[0])).unwrap();
    assert_eq!(
        Paths::new(Some(root.path().to_owned())).systems,
        at(CATALOGS[1])
    );
}
