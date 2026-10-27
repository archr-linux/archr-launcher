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
    let user = root
        .path()
        .join("storage/.config/emulationstation/es_systems.cfg");
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    std::fs::write(&user, "<systemList/>").unwrap();
    assert_eq!(Paths::new(Some(root.path().to_owned())).systems, user);
}
