//! System › Import from RetroArch (Config-importar-ra): what the host found in
//! roms/retroarch-import/, the list of what applies, and one import. The host
//! scans when the page opens and does the work off SDL's thread.
use super::{described, Effect, Id, Kind, Page, Row, Settings};
use crate::{state::Button, HintButton};

/// Where the user copies a RetroArch setup, as the ROMs share shows it.
pub const FOLDER: &str = "roms/retroarch-import/";
pub const ROWS: &[Row] = &[
    described(Id::RaConfig, "raimport.config", "raimport.config_detail")
        .kind(Kind::Action)
        .under("raimport.found"),
    described(
        Id::RaOverrides,
        "raimport.overrides",
        "raimport.overrides_detail",
    )
    .kind(Kind::Action),
    described(Id::RaShaders, "raimport.shaders", "raimport.shaders_detail").kind(Kind::Action),
    described(Id::RaPreview, "raimport.preview", "raimport.preview_detail")
        .kind(Kind::Action)
        .under("raimport.import"),
    described(Id::RaBackup, "raimport.backup", "raimport.backup_detail").kind(Kind::Toggle),
    described(Id::RaRun, "raimport.run", "raimport.run_detail").kind(Kind::Action),
];

/// The host's scan: counts for the rows, and each item for the list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// retroarch.cfg keys that apply, and those left out.
    pub settings: usize,
    pub skipped: usize,
    /// Cores with override or option files, and how many files.
    pub cores: Vec<String>,
    pub overrides: usize,
    pub presets: usize,
    pub remaps: usize,
    /// Key or file, and its value or a catalog reference.
    pub apply: Vec<(String, String)>,
    /// Key or file, and why (a catalog key).
    pub ignored: Vec<(String, String)>,
}
#[derive(Clone, Debug, Default)]
pub struct RaImport {
    pub found: Option<Found>,
    pub scanning: bool,
    /// An import runs: input waits for the host's reply.
    pub working: bool,
}
impl RaImport {
    /// Nothing known yet about an open page: the host looks.
    pub fn wants_scan(&self) -> bool {
        self.found.is_none() && !self.scanning && !self.working
    }
    /// Entries of the list page: what applies, then what does not.
    pub fn lines(&self) -> usize {
        self.found
            .as_ref()
            .map_or(0, |f| f.apply.len() + f.ignored.len())
    }
}

/// Catalog references translated and joined: "14 settings · 9 left out".
pub fn joined(catalog: &crate::i18n::I18n, refs: &[String]) -> String {
    refs.iter()
        .map(|r| super::information::value(catalog, r))
        .collect::<Vec<_>>()
        .join(" · ")
}

impl Settings {
    /// Catalog references for the three "found" rows, joined with " · ".
    pub(super) fn raimport_refs(&self, id: Id) -> Vec<String> {
        let Some(found) = &self.raimport.found else {
            return vec!["raimport.searching".into()];
        };
        let count = |key: &str, n: usize| format!("raimport.{key}|count={n}");
        match id {
            Id::RaConfig if found.settings + found.skipped > 0 => vec![
                count("settings_count", found.settings),
                count("ignored_count", found.skipped),
            ],
            Id::RaOverrides if found.overrides > 0 => vec![count("files", found.overrides)],
            Id::RaShaders if found.presets + found.remaps > 0 => {
                vec![count("files", found.presets + found.remaps)]
            }
            _ => vec!["raimport.missing".into()],
        }
    }
    /// Subtitle of the overrides row (the cores) and the shaders row (counts).
    pub(crate) fn raimport_detail(&self, id: Id) -> Option<Vec<String>> {
        let found = self.raimport.found.as_ref()?;
        match id {
            Id::RaOverrides if !found.cores.is_empty() => Some(vec![found.cores.join(", ")]),
            Id::RaShaders if found.presets + found.remaps > 0 => Some(vec![
                format!("raimport.presets_count|count={}", found.presets),
                format!("raimport.remaps_count|count={}", found.remaps),
            ]),
            _ => None,
        }
    }
    pub(super) fn raimport_input(&mut self, button: Button) -> Option<Effect> {
        if self.raimport.working {
            return Some(Effect::None);
        }
        if self.page() == Page::RaImportList {
            let last = self.raimport.lines().saturating_sub(1);
            match button {
                Button::Up => self.cursor = self.cursor.saturating_sub(1),
                Button::Down => self.cursor = (self.cursor + 1).min(last),
                Button::B => return None,
                _ => {}
            }
            return Some(Effect::None);
        }
        let id = self.visible_rows().get(self.cursor)?.id;
        let listed = self.raimport.lines() > 0;
        match (id, button) {
            // Only for this import: never written anywhere.
            (Id::RaBackup, Button::A | Button::L | Button::R) => {
                let next = if self.value(id) == "1" { "0" } else { "1" };
                self.values.insert(id, next.into());
            }
            (Id::RaConfig | Id::RaOverrides | Id::RaShaders | Id::RaPreview, Button::A)
                if listed =>
            {
                self.open(Page::RaImportList);
            }
            (Id::RaRun, Button::A) => {
                if self
                    .raimport
                    .found
                    .as_ref()
                    .is_some_and(|f| !f.apply.is_empty())
                {
                    self.raimport.working = true;
                    return Some(Effect::RaImport {
                        backup: self.value(Id::RaBackup) == "1",
                    });
                }
                self.fail(None, "raimport.nothing".into());
            }
            (_, Button::A | Button::L | Button::R) => {}
            _ => return None,
        }
        Some(Effect::None)
    }
    /// Settles an import: the message, and a fresh scan of what is left.
    pub fn raimport_done(&mut self, result: Result<String, String>) {
        self.raimport.working = false;
        self.raimport.found = None;
        match result {
            Ok(message) | Err(message) => self.fail(None, message),
        }
    }
    pub(super) fn raimport_hints(&self) -> &'static [(HintButton, &'static str)] {
        use HintButton::{Key, Pair};
        if self.raimport.working {
            return &[];
        }
        let id = self.visible_rows().get(self.cursor).map(|r| r.id);
        match (self.page(), id) {
            (Page::RaImportList, _) => &[(Key('B'), "hint.back")],
            (_, Some(Id::RaBackup)) => &[(Pair('‹', '›'), "hint.adjust"), (Key('B'), "hint.back")],
            _ => &[(Key('A'), "hint.choose"), (Key('B'), "hint.back")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(found: Option<Found>) -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::System);
        state.cursor = Page::System
            .rows()
            .iter()
            .position(|r| r.id == Id::RaImportLink)
            .unwrap();
        state.handle(Button::A);
        state.raimport.found = found;
        state
    }
    fn at(state: &mut Settings, id: Id) {
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == id)
            .unwrap();
    }
    fn found() -> Found {
        Found {
            settings: 2,
            skipped: 1,
            cores: vec!["mGBA".into()],
            overrides: 1,
            apply: vec![
                ("video_scale_integer".into(), "true".into()),
                (
                    "config/mGBA/mGBA.cfg".into(),
                    "raimport.file_settings|count=1".into(),
                ),
            ],
            ignored: vec![("video_driver".into(), "raimport.why_blocked".into())],
            ..Default::default()
        }
    }
    #[test]
    fn opens_from_system_and_asks_the_host_to_look() {
        let state = page(None);
        assert_eq!(state.page(), Page::RaImport);
        assert!(state.raimport.wants_scan());
        assert_eq!(state.raimport_refs(Id::RaConfig), ["raimport.searching"]);
        assert_eq!(state.value(Id::RaBackup), "1");
    }
    #[test]
    fn rows_show_counts_and_the_list_shows_every_item() {
        let mut state = page(Some(found()));
        assert_eq!(
            state.raimport_refs(Id::RaConfig),
            [
                "raimport.settings_count|count=2",
                "raimport.ignored_count|count=1"
            ]
        );
        assert_eq!(state.raimport_refs(Id::RaShaders), ["raimport.missing"]);
        assert_eq!(
            state.raimport_detail(Id::RaOverrides),
            Some(vec!["mGBA".into()])
        );
        at(&mut state, Id::RaPreview);
        state.handle(Button::A);
        assert_eq!(state.page(), Page::RaImportList);
        for _ in 0..5 {
            state.handle(Button::Down);
        }
        assert_eq!(state.cursor, 2);
        state.handle(Button::B);
        assert_eq!(state.page(), Page::RaImport);
    }
    #[test]
    fn backup_is_local_and_import_carries_it() {
        let mut state = page(Some(found()));
        at(&mut state, Id::RaBackup);
        assert_eq!(state.handle(Button::R), Effect::None);
        assert_eq!(state.value(Id::RaBackup), "0");
        at(&mut state, Id::RaRun);
        assert_eq!(state.handle(Button::A), Effect::RaImport { backup: false });
        assert!(state.raimport.working);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.hints().is_empty());
        state.raimport_done(Ok("raimport.done".into()));
        assert!(!state.raimport.working && state.raimport.wants_scan());
    }
    #[test]
    fn nothing_to_import_only_says_so() {
        let mut state = page(Some(Found::default()));
        at(&mut state, Id::RaRun);
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(!state.raimport.working);
        assert!(state.error.is_some());
        at(&mut state, Id::RaPreview);
        state.handle(Button::A);
        assert_eq!(state.page(), Page::RaImport);
    }
}
