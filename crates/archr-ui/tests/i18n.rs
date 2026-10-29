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

fn keys(path: &Path) -> std::collections::BTreeSet<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| {
            line.strip_prefix('"')?
                .split_once("\" =")
                .map(|k| k.0.to_owned())
        })
        .collect()
}
fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Config entries and file names that look like catalog keys.
const CONFIG_KEYS: &[&str] = &[
    // Theme store hosts and file names.
    "api.github.com",
    "codeload.github.com",
    "objects.githubusercontent.com",
    "preview.png",
    "raw.githubusercontent.com",
    "archr.mangohud.alpha",
    "archr.mangohud.enabled",
    "archr.mangohud.font_size",
    "archr.mangohud.fps_limit",
    "archr.mangohud.items",
    "archr.mangohud.layout",
    "archr.mangohud.position",
    "archr.mangohud.state",
    "audio.device",
    "audio.volume",
    "controllers.bluetooth.enabled",
    "display.brightness",
    "fuse.mergerfs",
    "global.netplay",
    "global.netplay.customserver",
    "global.netplay.host",
    "global.netplay.nickname",
    "global.netplay.port",
    "global.netplay.relay",
    "global.retroachievements",
    "global.retroachievements.encore",
    "global.retroachievements.hardcore",
    "global.retroachievements.password",
    "global.retroachievements.richpresence",
    "global.retroachievements.screenshot",
    "global.retroachievements.sound",
    "global.retroachievements.token",
    "global.retroachievements.username",
    "global.retroachievements.verbose",
    "gpu.driver",
    "hotkey.brightness",
    "hotkey.brightness.enabled",
    "hotkey.exit",
    "hotkey.exit.enabled",
    "hotkey.mangohud",
    "hotkey.mangohud.enabled",
    "hotkey.screenshot",
    "hotkey.screenshot.enabled",
    "hotkey.volume",
    "hotkey.volume.enabled",
    "idle.timeout",
    "idle.timeout.game",
    "key.dpad.events",
    "key.function.a",
    "key.hotkey.a",
    "key.hotkey.b",
    "key.hotkey.c",
    "launcher.accent",
    "launcher.animations",
    "launcher.background",
    "launcher.buttons",
    "launcher.clock.format",
    "launcher.font",
    "launcher.scraper.media",
    "launcher.scraper.only_missing",
    "launcher.scraper.systems",
    "launcher.sounds",
    "launcher.sounds_volume",
    "launcher.text_size",
    "launcher.theme",
    "launcher.view",
    "moonlight.cfg",
    "moonlight.height",
    "moonlight.hosts",
    "moonlight.width",
    "netplay.lobby",
    "netplay.port",
    "netplay.relay_only",
    "netplay.show_missing",
    "root.password",
    "samba.enabled",
    "simplehttp.enabled",
    "ssh.enabled",
    "system.automount",
    "system.cpugovernor",
    "system.kblayout",
    "system.kbvariant",
    "system.language",
    "system.merged.storage",
    "system.suspendmode",
    "system.timezone",
    "wifi.adhoc.channel",
    "wifi.adhoc.enabled",
    "wifi.adhoc.id",
    "wifi.enabled",
    "wifi.key",
    "wifi.ssid",
    "a.rom",
    "b.rom",
    "body.ttf",
    "game.chd",
    "game.nes",
    "game.zip",
    "h.01",
    "legacy.size",
    "mario.nes",
    "missing.png",
    "nmbd.service",
    "notes.txt",
    "plain.sh",
    "runemu.sh",
    "power.png",
    "scraper.toml",
    "sfc.png",
    "simplehttp.conf",
    "smb.conf",
    "smbd.service",
    "snes.png",
    "sshd.conf",
    "sshd.service",
    "store.toml",
    "theme.toml",
    "twonlinesn.sh",
    "unlock.ogg",
    "v.01",
    "wifi.svg",
    "wrench.svg",
    "zelda.ogg",
];

/// Every string literal of a Rust source: comments skipped, char literals
/// (including '"'), escapes and raw strings handled.
fn literals(source: &str) -> Vec<String> {
    let c: Vec<char> = source.chars().collect();
    let (mut i, mut out) = (0, Vec::new());
    while i < c.len() {
        match c[i] {
            '/' if c.get(i + 1) == Some(&'/') => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                let mut depth = 0;
                while i < c.len() {
                    if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                        depth += 1;
                        i += 2;
                    } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            '\'' => {
                // A char literal ('x', '\n', '"') closes within a few chars;
                // anything else is a lifetime.
                let close = if c.get(i + 1) == Some(&'\\') {
                    c[i + 2..]
                        .iter()
                        .position(|&ch| ch == '\'')
                        .map(|n| i + 2 + n)
                } else {
                    (c.get(i + 2) == Some(&'\'')).then_some(i + 2)
                };
                i = close.map_or(i + 1, |end| end + 1);
            }
            'r' if matches!(c.get(i + 1), Some('"' | '#'))
                && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) =>
            {
                let hashes = c[i + 1..].iter().take_while(|&&ch| ch == '#').count();
                if c.get(i + 1 + hashes) != Some(&'"') {
                    i += 1;
                    continue;
                }
                let start = i + 2 + hashes;
                let end = (start..c.len())
                    .find(|&j| c[j] == '"' && c[j + 1..].iter().take(hashes).all(|&ch| ch == '#'))
                    .unwrap_or(c.len());
                out.push(c[start..end].iter().collect());
                i = end + 1 + hashes;
            }
            '"' => {
                let mut text = String::new();
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' {
                        i += 1;
                    }
                    text.push(c[i.min(c.len() - 1)]);
                    i += 1;
                }
                out.push(text);
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}
/// `[a-z_]+(\.[a-z0-9_]+)+`
fn key_shaped(text: &str) -> bool {
    let mut parts = text.split('.');
    let first = parts.next().unwrap_or("");
    let rest: Vec<_> = parts.collect();
    !first.is_empty()
        && first.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        && !rest.is_empty()
        && rest.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

#[test]
fn the_literal_tokenizer_skips_comments_and_char_literals() {
    let source = "let a = '\"'; // \"in.comment\"\nlet b = \"x.y\\\"z\"; let c: &'static str = r#\"raw.key\"#; /* \"no.pe\" */ let d = '\\'';";
    assert_eq!(literals(source), ["x.y\"z", "raw.key"]);
    assert!(key_shaped("settings.usb_busy") && key_shaped("a.b2.c"));
    assert!(!key_shaped("Cargo.toml") && !key_shaped("a.") && !key_shaped("a b.c"));
}

#[test]
fn every_key_used_in_code_exists_in_the_catalog() {
    let known = keys(&locales().join("en.toml"));
    let mut files = Vec::new();
    for krate in ["archr-ui", "archr-launcher"] {
        sources(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../")
                .join(krate)
                .join("src"),
            &mut files,
        );
    }
    let mut missing = Vec::new();
    for file in files {
        for literal in literals(&fs::read_to_string(&file).unwrap()) {
            let key = literal.split('|').next().unwrap();
            if key_shaped(key) && !CONFIG_KEYS.contains(&key) && !known.contains(key) {
                missing.push(key.to_owned());
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "literals that are not catalog keys: {missing:#?}"
    );
}

#[test]
fn any_catalog_key_is_recognised_but_plain_text_is_not() {
    let catalog = I18n::load(&locales(), Some("pt_BR"), None);
    assert!(catalog.has("hint.confirm") && catalog.has("settings.failed"));
    assert!(!catalog.has("Wi-Fi.guest") && !catalog.has("192.168.1.2"));
}
