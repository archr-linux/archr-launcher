//! System › During a game (Config-durante-jogo): what stops while a game
//! runs, and the memory a heavy one gets. archr-game-services and
//! archr-memory-guard read the game rows when a game starts;
//! archr-memory-manager reads the memory ones at boot.
use super::{described, Id, Kind, Row};
use crate::i18n::I18n;

pub const ROWS: &[Row] = &[
    described(Id::GameWifi, "in_game.wifi", "in_game.wifi_detail").under("in_game.on_launch"),
    described(
        Id::GameBluetooth,
        "in_game.bluetooth",
        "in_game.bluetooth_detail",
    ),
    described(Id::GameSharing, "in_game.sharing", "in_game.sharing_detail"),
    described(Id::GameSsh, "in_game.ssh", "in_game.ssh_detail"),
    described(Id::MemoryZram, "in_game.zram", "in_game.zram_detail").under("in_game.memory"),
    described(
        Id::MemoryRamPlus,
        "in_game.ramplus",
        "in_game.ramplus_detail",
    ),
    described(Id::MemoryGuard, "in_game.guard", "in_game.guard_detail").kind(Kind::Toggle),
];

/// The shown value of a During a game row: sizes in MB below 1 GB, in GB
/// (with the language's decimal comma or point) from it.
pub(super) fn value(catalog: &I18n, id: Id, value: &str) -> String {
    match (id, value) {
        (Id::GameWifi | Id::GameBluetooth | Id::GameSsh, "off") => catalog.text("in_game.off"),
        (Id::GameSharing, "pause") => catalog.text("in_game.pause"),
        (Id::GameWifi | Id::GameBluetooth | Id::GameSharing | Id::GameSsh, _) => {
            catalog.text("in_game.keep")
        }
        (Id::MemoryRamPlus, "0") => catalog.text("settings.off"),
        (Id::MemoryZram | Id::MemoryRamPlus, mb) => size(catalog, mb),
        _ => value.to_owned(),
    }
}

fn size(catalog: &I18n, mb: &str) -> String {
    let Ok(mb) = mb.parse::<u32>() else {
        return mb.to_owned();
    };
    if mb < 1024 {
        return catalog.format("in_game.mb", &[("count", &mb.to_string())]);
    }
    let whole = mb / 1024;
    let tenths = (mb % 1024) * 10 / 1024;
    let count = if tenths == 0 {
        whole.to_string()
    } else {
        format!("{whole}{}{tenths}", catalog.text("in_game.decimal"))
    };
    catalog.format("in_game.gb", &[("count", &count)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Page, Settings};

    #[test]
    fn the_page_sits_in_system_with_its_rows() {
        assert_eq!(Page::InGame.rows().len(), 7);
        assert_eq!(Page::InGame.parent(), &["settings.system"]);
        assert!(Page::System.rows().iter().any(|r| r.id == Id::InGameLink));
    }

    #[test]
    fn defaults_are_valid_choices() {
        for row in ROWS {
            assert!(
                row.id.choices().contains(&row.id.default_value()),
                "{:?}",
                row.id
            );
        }
        // Nothing changes for a player who never opens the page, but the
        // sharing pauses and the memory guard watches.
        assert_eq!(Id::GameWifi.default_value(), "keep");
        assert_eq!(Id::GameBluetooth.default_value(), "keep");
        assert_eq!(Id::GameSharing.default_value(), "pause");
        assert_eq!(Id::MemoryZram.default_value(), "1024");
        assert_eq!(Id::MemoryRamPlus.default_value(), "0");
        assert_eq!(Id::MemoryGuard.default_value(), "1");
    }

    #[test]
    fn values_read_as_in_the_mockup() {
        let locales = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales"));
        let settings = Settings::default();
        let shown = |catalog: &I18n, id, v: &str| {
            let mut s = settings.clone();
            s.values.insert(id, v.into());
            s.display_value(id, catalog)
        };
        let pt = I18n::load(locales, Some("pt_BR"), None);
        assert_eq!(shown(&pt, Id::GameWifi, "keep"), "Manter");
        assert_eq!(shown(&pt, Id::GameWifi, "off"), "Desligar");
        assert_eq!(shown(&pt, Id::GameSharing, "pause"), "Pausar");
        assert_eq!(shown(&pt, Id::MemoryZram, "512"), "512 MB");
        assert_eq!(shown(&pt, Id::MemoryZram, "1024"), "1 GB");
        assert_eq!(shown(&pt, Id::MemoryZram, "1536"), "1,5 GB");
        assert_eq!(shown(&pt, Id::MemoryZram, "2048"), "2 GB");
        assert_eq!(shown(&pt, Id::MemoryRamPlus, "0"), "Desligado");
        let en = I18n::load(locales, Some("en_US"), None);
        assert_eq!(shown(&en, Id::MemoryZram, "1536"), "1.5 GB");
    }
}
