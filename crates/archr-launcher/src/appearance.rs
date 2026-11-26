//! Theme discovery and loading, shared by startup and the Appearance page.
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_ui::{appearance::Appearance, settings::Id, theme_dir, Theme, DEFAULT_THEME_DIR};
use std::{fs, path::PathBuf};

pub(crate) const USER_THEMES: &str = "/storage/.config/archr-launcher/themes";
const SYSTEM_THEMES: &str = "/usr/share/archr-launcher/themes";

/// The named theme and its directory, or the default when it cannot be
/// found. Every theme starts from the installed default, fonts included.
pub fn load_theme(paths: &Paths, name: &str) -> (Theme, Option<PathBuf>) {
    let dir = theme_dir(&paths.at(USER_THEMES), &paths.at(SYSTEM_THEMES), name);
    let default = paths.at(DEFAULT_THEME_DIR);
    let base = if default.join("theme.toml").is_file() {
        Theme::load(&default)
    } else {
        Theme::builtin()
    };
    let mut theme = match dir.as_deref() {
        Some(dir) if dir != default => Theme::load_over(dir, &base),
        _ => base,
    };
    theme.font = theme.font.as_deref().map(|font| paths.remap(font));
    // The choices made for this theme's options come back with it.
    if !theme.options().is_empty() {
        if let Ok(cfg) = SystemCfg::load(&paths.config) {
            let chosen: Vec<(String, String)> = theme
                .options()
                .iter()
                .filter_map(|o| {
                    let key = archr_ui::appearance::option_key(name, &o.id)?;
                    Some((o.id.clone(), cfg.get_setting(&key, None, None)?))
                })
                .collect();
            theme = theme.with_options(&chosen);
        }
    }
    (theme, dir)
}

/// Installed themes, user and system, always including the bundled one.
pub fn theme_names(paths: &Paths) -> Vec<String> {
    let mut names = vec!["default".to_owned()];
    for root in [USER_THEMES, SYSTEM_THEMES] {
        let Ok(entries) = fs::read_dir(paths.at(root)) else {
            continue;
        };
        names.extend(
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().join("theme.toml").is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| !name.starts_with('.')),
        );
    }
    names.sort();
    names.dedup();
    names
}

/// The saved choices, each falling back to its default.
pub fn saved(cfg: Option<&SystemCfg>) -> Appearance {
    let get = |id: Id| {
        cfg.and_then(|cfg| cfg.get_setting(id.key(), None, None))
            .unwrap_or_else(|| id.default_value().into())
    };
    Appearance {
        theme: get(Id::Theme),
        accent: get(Id::Accent),
        background: get(Id::Background),
        font: get(Id::Font),
        text_size: get(Id::TextSize),
        animations: archr_ui::motion::Motion::parse(&get(Id::Animations)),
        view: get(Id::View),
        font_weight: get(Id::FontWeight),
        battery_percent: get(Id::BatteryPercent),
        game_art: get(Id::GameArt),
        art_width: get(Id::ArtWidth),
        art_radius: get(Id::ArtRadius),
        switcher_curtain: get(Id::SwitcherCurtain),
        text_color: get(Id::TextColor),
        selected_text_color: get(Id::SelectedTextColor),
        secondary_color: get(Id::SecondaryColor),
        text_hints: get(Id::TextHints),
        system_background: get(Id::SystemBackground),
        // load_theme brings the theme's options back with it.
        options: Vec::new(),
    }
}

/// What archr-osd draws with, in /run/archr-launcher: the active theme
/// resolved (docs/temas.md, OSD de volume e brilho).
pub const OSD_THEME: &str = "osd-theme.toml";

/// The part of `theme` archr-osd uses: the fonts, `[osd]` and, in
/// `[colors]`, the colours that differ from the default theme's (the OSD's
/// own palette is the default theme's, so the default writes none).
pub fn osd_theme(theme: &Theme) -> String {
    use archr_ui::screens::{OsdEdge, OsdShape};
    use toml::{Table, Value};
    let hex = |c: archr_ui::Rgba| {
        let rgb = format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b);
        Value::String(if c.a == 255 {
            rgb
        } else {
            format!("{rgb}{:02X}", c.a)
        })
    };
    let mut root = Table::new();
    for (key, font) in [
        ("font", &theme.font),
        ("font_semibold", &theme.font_semibold),
    ] {
        if let Some(path) = font {
            root.insert(key.into(), Value::String(path.to_string_lossy().into()));
        }
    }
    let base = Theme::builtin();
    let colors: Table = [
        ("surface", theme.surface, base.surface),
        ("border", theme.border, base.border),
        ("badge", theme.badge, base.badge),
        ("text", theme.text, base.text),
        ("text_secondary", theme.text_secondary, base.text_secondary),
        ("muted", theme.muted, base.muted),
        ("accent", theme.highlight_bg, base.highlight_bg),
        ("success", theme.success, base.success),
        ("warning", theme.warning, base.warning),
        ("danger", theme.danger, base.danger),
        ("scrim", theme.scrim, base.scrim),
    ]
    .into_iter()
    .filter(|(_, own, default)| own != default)
    .map(|(key, own, _)| (key.to_owned(), hex(own)))
    .collect();
    if !colors.is_empty() {
        root.insert("colors".into(), Value::Table(colors));
    }
    let o = theme.screens.osd;
    let mut osd = Table::new();
    let style = match o.style {
        OsdShape::Pill => "pill",
        OsdShape::Capsule => "capsule",
    };
    let position = match o.position {
        OsdEdge::Right => "right",
        OsdEdge::Left => "left",
        OsdEdge::Top => "top",
        OsdEdge::Bottom => "bottom",
    };
    osd.insert("style".into(), Value::String(style.into()));
    osd.insert("position".into(), Value::String(position.into()));
    for (key, color) in [("fill", o.fill), ("track", o.track)] {
        if let Some(color) = color {
            osd.insert(key.into(), hex(color));
        }
    }
    if let Some(radius) = o.radius {
        let radius = (f64::from(radius) * 1e6).round() / 1e6;
        osd.insert("radius".into(), Value::Float(radius));
    }
    root.insert("osd".into(), Value::Table(osd));
    toml::to_string(&root).unwrap_or_default()
}

/// Hands the active theme to archr-osd, whole or not at all: it reads the
/// file again when it changes. A failure only costs the OSD the theme.
pub fn publish_osd_theme(paths: &Paths, theme: &Theme) {
    use std::io::Write;
    let path = paths.run.join(OSD_THEME);
    let text = osd_theme(theme);
    if let Err(e) = archr_io::atomic_publish(&path, false, |file| {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o644))?;
        file.write_all(text.as_bytes())
    }) {
        log::warn!("Cannot write {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ellegance hands the OSD its capsule on the right, its own colours
    /// and Inter. ARCHR_OSD_THEME_OUT=<file> keeps what it writes (for the
    /// OSD's `over_screens` capture).
    #[test]
    fn ellegance_gives_the_osd_its_capsule() {
        let themes = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        let theme = Theme::load(&themes.join("ellegance"));
        let text = osd_theme(&theme);
        let table: toml::Table = toml::from_str(&text).unwrap();
        assert_eq!(table["osd"]["style"].as_str(), Some("capsule"));
        assert_eq!(table["osd"]["position"].as_str(), Some("right"));
        assert_eq!(table["colors"]["danger"].as_str(), Some("#FF453A"));
        assert!(table["font"]
            .as_str()
            .unwrap()
            .ends_with("Inter-Regular.ttf"));
        if let Some(out) = std::env::var_os("ARCHR_OSD_THEME_OUT") {
            fs::write(out, text).unwrap();
        }
    }

    #[test]
    fn the_osd_gets_the_fonts_its_table_and_only_changed_colours() {
        let root = tempfile::TempDir::new().unwrap();
        let paths = Paths::new(Some(root.path().to_path_buf()));
        let path = paths.run.join(OSD_THEME);
        let mut theme = Theme::builtin();
        theme.font =
            Some("/usr/share/archr-launcher/themes/default/fonts/Inter-Regular.ttf".into());
        publish_osd_theme(&paths, &theme);
        let table: toml::Table = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            table.get("colors").is_none(),
            "the default writes no colour"
        );
        assert_eq!(
            table["font"].as_str(),
            Some("/usr/share/archr-launcher/themes/default/fonts/Inter-Regular.ttf")
        );
        assert_eq!(table["osd"]["style"].as_str(), Some("pill"));
        assert_eq!(table["osd"]["position"].as_str(), Some("right"));
        assert!(table["osd"].get("fill").is_none());

        let dir = root.path().join("glass");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("theme.toml"),
            "warning = \"#FFAA00\"\nscrim = \"#00000080\"\n\
             [osd]\nstyle = \"capsule\"\nposition = \"bottom\"\nfill = \"#FFFFFF\"\nradius = 0.05\n",
        )
        .unwrap();
        publish_osd_theme(&paths, &Theme::load(&dir));
        let table: toml::Table = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let colors = table["colors"].as_table().unwrap();
        assert_eq!(colors.len(), 2, "{colors:?}");
        assert_eq!(colors["warning"].as_str(), Some("#FFAA00"));
        assert_eq!(colors["scrim"].as_str(), Some("#00000080"));
        let osd = table["osd"].as_table().unwrap();
        assert_eq!(osd["style"].as_str(), Some("capsule"));
        assert_eq!(osd["position"].as_str(), Some("bottom"));
        assert_eq!(osd["fill"].as_str(), Some("#FFFFFF"));
        assert_eq!(osd["radius"].as_float(), Some(0.05));
        // Replaced whole: no temporary file is left beside it.
        assert_eq!(fs::read_dir(&paths.run).unwrap().count(), 1);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn themes_without_fonts_take_the_default_themes_family_by_family() {
        let root = tempfile::TempDir::new().unwrap();
        let paths = Paths::new(Some(root.path().to_path_buf()));
        let default = paths.at(DEFAULT_THEME_DIR);
        fs::create_dir_all(default.join("fonts")).unwrap();
        for face in ["Sans.ttf", "SansBold.ttf", "Mono.ttf"] {
            fs::write(default.join("fonts").join(face), "").unwrap();
        }
        fs::write(
            default.join("theme.toml"),
            "font = \"fonts/Sans.ttf\"\nfont_semibold = \"fonts/SansBold.ttf\"\n\
             font_mono = \"fonts/Mono.ttf\"\n",
        )
        .unwrap();
        let theme = |name: &str, toml: &str| {
            let dir = paths.at(SYSTEM_THEMES).join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("theme.toml"), toml).unwrap();
            fs::write(dir.join("Own.ttf"), "").unwrap();
            load_theme(&paths, name).0
        };
        let font = |name: &str| Some(default.join("fonts").join(name).canonicalize().unwrap());
        let plain = theme("plain", "accent = \"#FFC93C\"\n");
        assert_eq!(plain.font, font("Sans.ttf"));
        assert_eq!(plain.font_semibold, font("SansBold.ttf"));
        assert_eq!(plain.font_mono, font("Mono.ttf"));
        assert_eq!(plain.font_mono_medium, None);
        // A theme naming its own regular face keeps it for semibold too.
        let own = theme("own", "font = \"Own.ttf\"\n");
        assert!(own.font.unwrap().ends_with("own/Own.ttf"));
        assert_eq!(own.font_semibold, None);
        assert_eq!(own.font_mono, font("Mono.ttf"));
        assert_eq!(load_theme(&paths, "default").0.font, font("Sans.ttf"));
    }
}
