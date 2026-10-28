use archr_ui::i18n::{plural_category, I18n};
use std::{fs, path::Path};

fn locales() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales")
}

#[test]
fn launcher_plural_rules() {
    for (n, want) in [
        (1., "one"),
        (2., "few"),
        (5., "many"),
        (11., "many"),
        (12., "many"),
        (21., "one"),
        (22., "few"),
        (25., "many"),
        (111., "many"),
        (0., "many"),
        (1.5, "other"),
    ] {
        assert_eq!(plural_category("ru", n), want, "ru {n}");
    }
    for lang in ["en", "es"] {
        assert_eq!(plural_category(lang, 0.), "other");
        assert_eq!(plural_category(lang, 1.), "one");
        assert_eq!(plural_category(lang, 2.), "other");
        assert_eq!(plural_category(lang, 1.5), "other");
    }
    for (n, want) in [
        (0., "other"),
        (1., "one"),
        (2., "other"),
        (0.5, "other"),
        (1.5, "other"),
    ] {
        assert_eq!(plural_category("pt_BR", n), want);
    }
}

#[test]
fn portuguese_and_english_game_and_favorite_counts() {
    for (lang, games, favorites) in [
        (
            "pt_BR",
            ["0 jogos", "1 jogo", "2 jogos"],
            ["0 favoritos", "1 favorito", "2 favoritos"],
        ),
        (
            "en",
            ["0 games", "1 game", "2 games"],
            ["0 favorites", "1 favorite", "2 favorites"],
        ),
    ] {
        let catalog = I18n::load(&locales(), Some(lang), None);
        for n in 0..=2 {
            let count = n.to_string();
            for (key, want) in [("games.count", games[n]), ("favorites.count", favorites[n])] {
                assert_eq!(catalog.plural(key, n as f64, &[("count", &count)]), want);
            }
        }
    }
}

#[test]
fn resolution_full_language_environment_and_english() {
    for (config, env, want) in [
        (Some("pt_BR"), None, "pt_BR"),
        (Some("pt-BR.UTF-8"), None, "pt_BR"),
        (Some("es_MX"), None, "es"),
        (Some("ru_RU"), None, "ru"),
        (Some("en_US"), None, "en"),
        (None, Some("ru_RU.UTF-8"), "ru"),
        (Some("xx"), Some("ru"), "en"),
        (None, None, "en"),
        (Some("../../ru"), None, "en"),
    ] {
        let i = I18n::load(&locales(), config, env);
        assert_eq!(i.language(), want);
    }
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("fr.toml"), "\"hint.open\" = \"Base\"").unwrap();
    fs::write(
        dir.path().join("fr_CA.toml"),
        "\"hint.open\" = \"Regional\"",
    )
    .unwrap();
    let i = I18n::load(dir.path(), Some("fr_CA"), None);
    assert_eq!(i.text("hint.open"), "Regional");
    fs::remove_file(dir.path().join("fr_CA.toml")).unwrap();
    assert_eq!(
        I18n::load(dir.path(), Some("fr_CA"), None).text("hint.open"),
        "Base"
    );
    // New catalogs are discoverable without a hardcoded language allowlist.
    fs::write(dir.path().join("fr.toml"), "\"hint.open\" = \"Ouvrir\"").unwrap();
    assert_eq!(
        I18n::load(dir.path(), Some("fr_FR"), None).text("hint.open"),
        "Ouvrir"
    );
}

#[test]
fn interpolation_fallback_and_missing_keys() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("xx.toml"),
        "\"notice.opening\" = \"Abriendo {game}…\"",
    )
    .unwrap();
    let i = I18n::load(dir.path(), Some("xx"), None);
    assert_eq!(
        i.format("notice.opening", &[("game", "A {game} {other}")]),
        "Abriendo A {game} {other}…"
    );
    assert_eq!(i.format("notice.opening", &[]), "Abriendo {game}…");
    assert_eq!(i.text("hint.back"), "Back");
    assert_eq!(i.text("unknown.key"), "unknown.key");
    assert_eq!(i.text("unknown.key"), "unknown.key");
    assert_eq!(i.plural("games.count", 2., &[("count", "2")]), "2 games");
}

#[test]
fn malformed_and_unsafe_catalogs_fall_back_to_english() {
    let dir = tempfile::tempdir().unwrap();
    for source in [
        "[invalid",
        "\"hint.open\" = 12",
        "\"games.count\" = { one = 12, other = 'x' }",
        "\"games.count\" = { one = 'x' }",
    ] {
        fs::write(dir.path().join("xx.toml"), source).unwrap();
        assert_eq!(
            I18n::load(dir.path(), Some("xx"), None).text("hint.open"),
            "Open"
        );
    }
    fs::write(dir.path().join("xx.toml"), "x".repeat(65537)).unwrap();
    assert_eq!(I18n::load(dir.path(), Some("xx"), None).language(), "en");
    fs::remove_file(dir.path().join("xx.toml")).unwrap();
    std::os::unix::fs::symlink(locales().join("ru.toml"), dir.path().join("xx.toml")).unwrap();
    assert_eq!(I18n::load(dir.path(), Some("xx"), None).language(), "en");
}

#[test]
fn every_catalog_has_exactly_the_english_keys_categories_and_placeholders() {
    let english: toml::Table = toml::from_str(include_str!("../../../locales/en.toml")).unwrap();
    assert!(english.keys().all(|key| !key.starts_with("cli.")));
    for key in ["menu.settings", "notice.settings"] {
        assert!(!english.contains_key(key));
    }
    for code in ["pt_BR", "es", "ru"] {
        let source = fs::read_to_string(locales().join(format!("{code}.toml"))).unwrap();
        let other: toml::Table = toml::from_str(&source).unwrap();
        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            other.keys().collect::<Vec<_>>(),
            "{code}"
        );
        for (key, value) in &english {
            let strings = |v: &toml::Value| -> Vec<String> {
                if let Some(s) = v.as_str() {
                    vec![s.to_owned()]
                } else {
                    v.as_table()
                        .unwrap()
                        .values()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect()
                }
            };
            let placeholders = |s: &str| -> std::collections::BTreeSet<String> {
                s.split('{')
                    .skip(1)
                    .map(|p| p.split('}').next().unwrap().to_owned())
                    .collect()
            };
            let expected = placeholders(&strings(value)[0]);
            for translated in strings(&other[key]) {
                assert_eq!(placeholders(&translated), expected, "{code}: {key}");
            }
            if value.is_table() {
                let table = other[key].as_table().unwrap();
                for category in if code == "ru" {
                    &["one", "few", "many", "other"][..]
                } else {
                    &["one", "other"][..]
                } {
                    assert!(table.contains_key(*category), "{code}: {key}.{category}");
                }
            }
        }
    }
}

#[test]
fn relative_time_and_player_ranges_use_the_catalog() {
    let i = I18n::load(&locales(), Some("ru"), None);
    for (elapsed, want) in [
        (0, "сейчас"),
        (60, "1 мин. назад"),
        (120, "2 мин. назад"),
        (3600, "1 ч. назад"),
        (86400, "вчера"),
        (3 * 86400, "3 дн. назад"),
        (30 * 86400, "1 мес. назад"),
        (365 * 86400, "1 год назад"),
        (2 * 365 * 86400, "2 года назад"),
        (5 * 365 * 86400, "5 лет назад"),
    ] {
        assert_eq!(i.relative_time(100, 100 + elapsed), want);
    }
    assert_eq!(i.relative_time(200, 100), "сейчас");
    assert_eq!(i.players("1"), "1 игрок");
    assert_eq!(i.players("2"), "2 игрока");
    assert_eq!(i.players("1-2"), "1-2 игрока");
    assert_eq!(i.players("1-5"), "1-5 игроков");
}

#[test]
fn missing_translation_logs_one_warning_per_key() {
    use std::sync::Mutex;
    struct Logger(Mutex<Vec<String>>);
    impl log::Log for Logger {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
        fn flush(&self) {}
    }
    static LOGGER: Logger = Logger(Mutex::new(Vec::new()));
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("xx.toml"), "\"hint.open\" = 'Открыть'").unwrap();
    let i = I18n::load(dir.path(), Some("xx"), None);
    for _ in 0..3 {
        assert_eq!(i.text("test.unique_missing_key"), "test.unique_missing_key");
        assert_eq!(i.text("menu.shutdown"), "Power off");
    }
    let lines = LOGGER.0.lock().unwrap();
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("test.unique_missing_key"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("menu.shutdown"))
            .count(),
        1
    );
}

#[test]
fn nonregular_and_invalid_utf8_catalogs_never_block_or_leak() {
    use std::{
        ffi::CString,
        os::unix::ffi::OsStrExt,
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("xx.toml");
    fs::create_dir(&path).unwrap();
    assert_eq!(
        I18n::load(dir.path(), Some("xx"), None).text("hint.open"),
        "Open"
    );
    fs::remove_dir(&path).unwrap();
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert_eq!(
        I18n::load(dir.path(), Some("xx"), None).text("hint.open"),
        "Open"
    );
    fs::remove_file(&path).unwrap();
    let cpath = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: cpath is a live NUL-terminated path in our private test directory.
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let began = Instant::now();
    assert_eq!(
        I18n::load(dir.path(), Some("xx"), None).text("hint.open"),
        "Open"
    );
    assert!(began.elapsed() < Duration::from_millis(100));
}

#[test]
fn shipped_languages_are_embedded_and_ignore_external_overrides() {
    let dir = tempfile::tempdir().unwrap();
    for (code, want) in [
        ("en_US", "Open"),
        ("pt_BR", "Abrir"),
        ("es_MX", "Abrir"),
        ("ru_RU", "Открыть"),
    ] {
        // No locale directory exists at all on the fast path.
        let i = I18n::load(&dir.path().join("absent"), Some(code), None);
        assert_eq!(i.text("hint.open"), want, "{code}");
        for name in [code, code.split('_').next().unwrap()] {
            fs::write(
                dir.path().join(format!("{name}.toml")),
                "\"hint.open\" = 'Unexpected override'",
            )
            .unwrap();
        }
        assert_eq!(
            I18n::load(dir.path(), Some(code), None).text("hint.open"),
            want
        );
    }
}

#[test]
fn portuguese_language_alias_serves_all_regions_from_brazilian_catalog() {
    let dir = tempfile::tempdir().unwrap();
    for code in ["pt", "pt_PT", "pt_AO", "pt_BR", "pt-PT.UTF-8"] {
        let i = I18n::load(dir.path(), Some(code), None);
        assert_eq!(i.language(), "pt_BR", "{code}");
        assert_eq!(i.text("hint.play"), "Jogar");
    }
}
