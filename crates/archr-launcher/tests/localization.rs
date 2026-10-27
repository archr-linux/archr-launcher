use std::{fs, process::Command};

#[test]
fn startup_uses_system_language_and_root_catalog_before_lang_environment() {
    let root = tempfile::tempdir().unwrap();
    let catalogs = root.path().join("usr/share/archr-launcher/locales");
    let config = root
        .path()
        .join("storage/.config/system/configs/system.cfg");
    fs::create_dir_all(&catalogs).unwrap();
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    for (code, label) in [
        ("en", "English"),
        ("es", "Español"),
        ("ru", "Русский"),
        ("pt_BR", "Português"),
    ] {
        fs::write(
            catalogs.join(format!("{code}.toml")),
            format!("\"cli.usage\" = \"{label}\"\n"),
        )
        .unwrap();
    }
    let run = |lang: &str, argument: &str| {
        Command::new(env!("CARGO_BIN_EXE_archr-launcher"))
            .env("ARCHR_LAUNCHER_ROOT", root.path())
            .env("LANG", lang)
            .arg(argument)
            .output()
            .unwrap()
    };
    for (value, want) in [
        ("ru_RU", "Русский\n"),
        ("es_MX", "Español\n"),
        ("pt_BR", "Português\n"),
        ("en_US", "English\n"),
        ("xx", "English\n"),
    ] {
        // global.language is deliberately different: system.language is exact.
        let source = format!("# fixture\nglobal.language=es\nsystem.language={value}\n");
        fs::write(&config, &source).unwrap();
        let result = run("es_MX.UTF-8", "--help");
        assert!(result.status.success(), "{value}");
        assert_eq!(String::from_utf8(result.stdout).unwrap(), want);
        assert_eq!(
            fs::read_to_string(&config).unwrap(),
            source,
            "startup is read-only"
        );
    }
    fs::write(&config, "global.language=es\n").unwrap();
    assert_eq!(
        String::from_utf8(run("ru_RU.UTF-8", "--help").stdout).unwrap(),
        "Русский\n"
    );
    fs::write(&config, "system.language=ru_RU\n").unwrap();
    fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales/ru.toml"),
        catalogs.join("ru.toml"),
    )
    .unwrap();
    let error = run("en_US.UTF-8", "--invalid");
    assert!(!error.status.success());
    assert!(String::from_utf8(error.stderr)
        .unwrap()
        .contains("Неизвестный аргумент: --invalid"));
}
