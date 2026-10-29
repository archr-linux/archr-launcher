//! Theme discovery and loading, shared by startup and the Appearance page.
use crate::paths::Paths;
use archr_config::SystemCfg;
use archr_ui::{appearance::Appearance, settings::Id, theme_dir, Theme};
use std::{fs, path::PathBuf};

const USER_THEMES: &str = "/storage/.config/archr-launcher/themes";
const SYSTEM_THEMES: &str = "/usr/share/archr-launcher/themes";

/// The named theme and its directory, or the builtin when it cannot be found.
pub fn load_theme(paths: &Paths, name: &str) -> (Theme, Option<PathBuf>) {
    let dir = theme_dir(&paths.at(USER_THEMES), &paths.at(SYSTEM_THEMES), name);
    let mut theme = dir.as_deref().map_or_else(Theme::builtin, Theme::load);
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
        animations: get(Id::Animations) == "1",
    }
}
