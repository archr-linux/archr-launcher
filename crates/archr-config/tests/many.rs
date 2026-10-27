use archr_config::SystemCfg;
use std::fs;
#[test]
fn batch_replaces_both_keys_in_one_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let lock = dir.path().join("lock");
    fs::write(
        &path,
        "# keep\nnes.core=old\nnes.emulator=old\nother=value\n",
    )
    .unwrap();
    let old = fs::File::open(&path).unwrap();
    SystemCfg::set_many_and_save(
        &path,
        &lock,
        &[("nes.core", "nestopia"), ("nes.emulator", "retroarch")],
    )
    .unwrap();
    let cfg = SystemCfg::load(&path).unwrap();
    assert_eq!(cfg.get("nes", None, "core").as_deref(), Some("nestopia"));
    assert_eq!(
        cfg.get("nes", None, "emulator").as_deref(),
        Some("retroarch")
    );
    assert!(fs::read_to_string(&path).unwrap().contains("# keep\n"));
    use std::io::Read;
    let mut source = String::new();
    (&old).read_to_string(&mut source).unwrap();
    assert!(source.contains("nes.core=old\nnes.emulator=old"));
    assert!(!lock.exists());
}
