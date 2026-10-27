use archr_index::{
    cache::{build_overview, refresh_overview},
    SystemDef,
};
use std::{
    fs,
    time::{Duration, SystemTime},
};
fn system(path: std::path::PathBuf) -> SystemDef {
    SystemDef {
        name: "nes".into(),
        fullname: "Nintendo".into(),
        path,
        extensions: vec![".nes".into()],
        emulators: vec![],
    }
}
#[test]
fn refresh_reuses_unchanged_counts_and_tracks_changed_new_removed_systems() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("A.nes"), []).unwrap();
    let sys = system(dir.path().to_owned());
    let mut old = build_overview(std::slice::from_ref(&sys));
    old.systems[0].game_count = 999;
    let (same, changed) = refresh_overview(&old, std::slice::from_ref(&sys));
    assert!(!changed);
    assert_eq!(
        same.systems[0].game_count, 999,
        "unchanged input must not scan"
    );
    fs::write(dir.path().join("B.nes"), []).unwrap();
    fs::File::open(dir.path())
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(2))
        .unwrap();
    let (new, changed) = refresh_overview(&old, std::slice::from_ref(&sys));
    assert!(changed);
    assert_eq!(new.systems[0].game_count, 2);
    let mut renamed = sys.clone();
    renamed.name = "other".into();
    let (new, changed) = refresh_overview(&new, &[renamed]);
    assert!(changed);
    assert_eq!(new.systems[0].name, "other");
    let (empty, changed) = refresh_overview(&new, &[]);
    assert!(changed);
    assert!(empty.systems.is_empty());
}
#[test]
fn missing_unchanged_directory_does_not_invalidate_overview() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path().join("missing"));
    let old = build_overview(std::slice::from_ref(&sys));
    assert!(!refresh_overview(&old, &[sys]).1);
}
