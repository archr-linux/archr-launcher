//! Games › Resume and switch (Config-retomar) and System › Power
//! (Config-energia): the rows and how their values read.
use super::{described, hotkeys::key_name, row, Id, Kind, Row};
use crate::i18n::I18n;

/// The switcher's shortcut, a tap on one button: FN, which no emulator
/// binds (every Select combination is a RetroArch, mupen64plus or
/// DuckStation hotkey; L3 and R3 are PPSSPP's pause, melonDS's screens and
/// the DS touch).
pub const SHORTCUTS: [&str; 1] = ["BTN_MODE"];

pub const RESUME: &[Row] = &[
    described(Id::ResumeBoot, "resume.boot", "resume.boot_detail").under("resume.on_boot"),
    described(Id::ResumeWait, "resume.wait", "resume.wait_detail"),
    described(Id::SwitcherOn, "resume.switcher", "resume.switcher_detail")
        .kind(Kind::Toggle)
        .under("resume.switcher_section"),
    described(Id::SwitcherKey, "resume.shortcut", "resume.shortcut_detail"),
    row(Id::SwitcherCount, "resume.count", Kind::Value),
    described(
        Id::ResumeStandalone,
        "resume.standalone",
        "resume.standalone_detail",
    )
    .under("resume.no_state_section"),
];

pub const POWER: &[Row] = &[
    described(
        Id::BatteryCritical,
        "power.critical",
        "power.critical_detail",
    )
    .under("power.battery"),
    described(Id::BatteryLow, "power.low", "power.low_detail"),
    described(Id::Idle, "power.idle", "power.idle_detail").under("power.unused"),
    described(Id::IdleGame, "power.idle_game", "power.idle_game_detail"),
    described(Id::ShutdownDelay, "power.shutdown", "power.shutdown_detail"),
    described(Id::Suspend, "power.mode", "power.mode_detail").under("power.suspend"),
];

/// The shown value of a Resume or Power row.
pub(super) fn value(catalog: &I18n, id: Id, value: &str) -> String {
    let count = |key: &str| catalog.format(key, &[("count", value)]);
    match (id, value) {
        (Id::ResumeBoot, "off") => catalog.text("settings.off"),
        (Id::ResumeBoot, _) => catalog.text("resume.last_game"),
        (Id::ResumeWait, _) => count("resume.seconds"),
        (Id::SwitcherKey, key) => key_name(key).to_owned(),
        (Id::ResumeStandalone, "skip") => catalog.text("resume.skip"),
        (Id::ResumeStandalone, _) => catalog.text("resume.start"),
        (Id::BatteryCritical | Id::BatteryLow, "0") => catalog.text("settings.off"),
        (Id::BatteryCritical | Id::BatteryLow, _) => count("power.percent"),
        (Id::ShutdownDelay, seconds) => {
            let minutes = seconds.parse::<u32>().unwrap_or(900) / 60;
            catalog.format("settings.minutes", &[("count", &minutes.to_string())])
        }
        _ => value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Page, Settings};

    #[test]
    fn pages_hold_their_rows_and_parents() {
        assert_eq!(Page::Resume.rows().len(), 6);
        assert_eq!(Page::Power.rows().len(), 6);
        assert_eq!(Page::Resume.parent(), &["settings.games"]);
        assert_eq!(Page::Power.parent(), &["settings.system"]);
        // Inactivity and suspension moved here from Sound and System.
        for id in [Id::Idle, Id::IdleGame, Id::Suspend] {
            assert!(Page::Power.rows().iter().any(|r| r.id == id));
            assert!(!Page::Sound.rows().iter().any(|r| r.id == id));
            assert!(!Page::System.rows().iter().any(|r| r.id == id));
        }
    }

    #[test]
    fn defaults_are_valid_choices() {
        for row in RESUME.iter().chain(POWER) {
            let choices = row.id.choices();
            if !choices.is_empty() && !matches!(row.id, Id::Idle | Id::IdleGame | Id::Suspend) {
                assert!(choices.contains(&row.id.default_value()), "{:?}", row.id);
            }
        }
    }

    #[test]
    fn values_read_as_in_the_mockups() {
        let catalog = I18n::load(
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales")),
            Some("pt_BR"),
            None,
        );
        let settings = Settings::default();
        let shown = |id, v: &str| {
            let mut s = settings.clone();
            s.values.insert(id, v.into());
            s.display_value(id, &catalog)
        };
        assert_eq!(shown(Id::ResumeBoot, "last"), "Último jogo");
        assert_eq!(shown(Id::ResumeWait, "3"), "3 s");
        assert_eq!(shown(Id::SwitcherKey, "BTN_MODE"), "FN");
        assert_eq!(shown(Id::SwitcherCount, "8"), "8");
        assert_eq!(shown(Id::ResumeStandalone, "start"), "Abrir do início");
        assert_eq!(shown(Id::BatteryCritical, "3"), "3%");
        assert_eq!(shown(Id::BatteryLow, "10"), "10%");
        assert_eq!(shown(Id::ShutdownDelay, "900"), "15 min");
        assert_eq!(shown(Id::BatteryCritical, "0"), "Desligado");
    }
}
