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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
