use archr_config::SystemCfg;
#[test]
fn local_core_lookup_does_not_replace_es_default_with_global() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    std::fs::write(
        &path,
        "global.core=unrelated\nnes.core=fceumm\nnes[\"Game.nes\"].core=nestopia\n",
    )
    .unwrap();
    let cfg = SystemCfg::load(&path).unwrap();
    assert_eq!(cfg.get_local("amiga", None, "core"), None);
    assert_eq!(cfg.get_local("nes", None, "core"), Some("fceumm".into()));
    assert_eq!(
        cfg.get_local("nes", Some("Game.nes"), "core"),
        Some("nestopia".into())
    );
    assert_eq!(cfg.get("amiga", None, "core"), Some("unrelated".into()));
}
