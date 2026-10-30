//! Storage › ROMs on the PC (Armazenamento-roms): moving the ROMs from the
//! system's ext4 partition to ARCHR_ROMS, the exFAT one any computer reads.
//! archr-roms-migrate owns the move; this page shows its preview and state,
//! and asks it to arm, cancel or retry. The host reads it off SDL's thread.
use super::{described, storage, Effect, Id, Kind, Page, Row, Settings};
use crate::state::Button;

pub const ROWS: &[Row] = &[
    // Readings, not choices: as actions their subtitle may use the width
    // the short value leaves (they are fixed, so A does nothing).
    described(Id::RomsNow, "roms.now", "roms.now_detail")
        .kind(Kind::Action)
        .under("roms.today"),
    described(Id::RomsCard, "roms.on_card", "roms.on_card_detail")
        .kind(Kind::Action)
        .under("roms.state"),
    // On an old card the "after" heading and bar come before it (drawn).
    described(Id::RomsNames, "roms.names", "").kind(Kind::Action),
    described(Id::RomsPower, "roms.power", "").kind(Kind::Action),
    described(Id::RomsMove, "roms.move", "").kind(Kind::Action),
    described(Id::RomsCancel, "roms.cancel", "roms.cancel_detail").kind(Kind::Action),
    described(Id::RomsRetry, "roms.retry", "roms.stalled").kind(Kind::Action),
    described(Id::RomsLeft, "roms.left", "").kind(Kind::Action),
];

/// `archr-roms-migrate status`, read with the Storage page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// ext4, exfat, migrating or unsupported; empty without the script.
    pub layout: String,
    /// armed, copying or stalled, while migrating.
    pub state: String,
    /// The mount's verdict: clean, checked, repaired, errors or failed.
    pub fs: String,
    /// Folders whose copy failed, kept on the system partition.
    pub left: Option<String>,
}
/// `KEY=VALUE` lines, the last one of a key winning.
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .rev()
        .filter_map(|line| line.trim().split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}
impl Status {
    pub fn parse(text: &str) -> Self {
        let get = |key| field(text, key).unwrap_or_default().to_owned();
        Self {
            layout: get("LAYOUT"),
            state: get("STATE"),
            fs: get("ROMS_FS"),
            left: field(text, "NOT_MIGRATED").map(str::to_owned),
        }
    }
    /// The card can have (or has) the ROMs partition: Storage links here.
    pub fn shown(&self) -> bool {
        matches!(self.layout.as_str(), "ext4" | "exfat" | "migrating")
    }
    pub fn ext4(&self) -> bool {
        self.layout == "ext4"
    }
    pub fn exfat(&self) -> bool {
        self.layout == "exfat"
    }
    pub fn armed(&self) -> bool {
        self.layout == "migrating" && self.state == "armed"
    }
    pub fn stalled(&self) -> bool {
        self.layout == "migrating" && self.state == "stalled"
    }
    /// The partition is read-only or not mounted: games cannot be saved.
    pub fn fault(&self) -> bool {
        matches!(self.fs.as_str(), "errors" | "failed")
    }
}

/// `archr-roms-migrate check`: what a move would do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
    pub files: u64,
    /// The ROMs as exFAT will hold them.
    pub roms_mib: u64,
    /// The system partition and the ROMs partition after the move.
    pub storage_mib: u64,
    pub part_mib: u64,
    pub renames: u64,
    pub symlinks: u64,
    pub power: bool,
    /// ok, no-space, low-power, busy, unsupported, armed or already.
    pub result: String,
}
impl Check {
    /// None when the script said nothing usable.
    pub fn parse(text: &str) -> Option<Self> {
        let number = |key| field(text, key).and_then(|v| v.parse().ok()).unwrap_or(0);
        Some(Self {
            files: number("ROMS_FILES"),
            roms_mib: number("ROMS_MIB"),
            storage_mib: number("STORAGE_AFTER_MIB"),
            part_mib: number("ROMS_PART_MIB"),
            renames: number("RENAMES"),
            symlinks: number("SYMLINKS"),
            power: field(text, "POWER") != Some("low"),
            result: field(text, "RESULT")?.to_owned(),
        })
    }
    pub fn ok(&self) -> bool {
        self.result == "ok"
    }
    /// About a minute per GB, as the page promises.
    pub fn minutes(&self) -> u64 {
        self.roms_mib.div_ceil(1024).max(1)
    }
}

/// `RENAME=<path>\t<new name>` lines of the check: what the move renames.
pub fn renames(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| line.strip_prefix("RENAME=")?.rsplit_once('\t'))
        .map(|(path, name)| (path.to_owned(), name.to_owned()))
        .collect()
}

/// What the page shows beyond the status.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// The preview, on a card that still has the old layout.
    pub check: Option<Check>,
    /// Path under roms/ and its new name: to change, or changed (the log).
    pub names: Vec<(String, String)>,
    /// Battery percent and whether it charges, on a device with a battery.
    pub battery: Option<(u8, bool)>,
}
#[derive(Clone, Debug, Default)]
pub struct Roms {
    pub scan: Option<Scan>,
    pub scanning: bool,
}

/// "8 GB", "7,8 GB": whole above 10 GB, a decimal below.
pub fn size(mib: u64) -> String {
    let gb = mib as f64 / 1024.;
    if gb >= 10. || gb.fract() < 0.05 {
        format!("@{} GB", gb.round())
    } else {
        format!("@{gb:.1} GB")
    }
}

impl Settings {
    /// Nothing known yet about an open page: the host looks.
    pub fn roms_wants_scan(&self) -> bool {
        self.page() == Page::Roms
            && self.roms.scan.is_none()
            && !self.roms.scanning
            && self.storage.working.is_none()
    }
    fn roms_check(&self) -> Option<&Check> {
        self.roms.scan.as_ref()?.check.as_ref()
    }
    pub(super) fn roms_files(&self) -> u64 {
        self.roms_check().map_or(0, |c| c.files)
    }
    fn roms_names(&self) -> usize {
        self.roms.scan.as_ref().map_or(0, |s| s.names.len())
    }
    pub(super) fn roms_visible(&self, id: Id) -> bool {
        let status = &self.storage.roms;
        let fresh = status.ext4();
        match id {
            Id::RomsNow | Id::RomsPower => fresh,
            Id::RomsNames => fresh || status.exfat() && self.roms_names() > 0,
            Id::RomsMove => fresh || status.armed(),
            Id::RomsCancel => status.armed(),
            Id::RomsRetry => status.stalled(),
            Id::RomsCard => status.exfat(),
            Id::RomsLeft => status.left.is_some(),
            _ => true,
        }
    }
    /// Title references that change with the state.
    pub(super) fn roms_label(&self, id: Id) -> Option<&'static str> {
        match id {
            Id::RomsNames if self.storage.roms.exfat() => Some("roms.renamed"),
            Id::RomsMove if self.storage.roms.armed() => Some("roms.restart_now"),
            _ => None,
        }
    }
    pub(super) fn roms_subtitle(&self, id: Id) -> Option<String> {
        let status = &self.storage.roms;
        Some(match id {
            Id::RomsLink if status.fault() => "roms.fault_short".into(),
            Id::RomsLink if status.left.is_some() => "roms.left_short".into(),
            Id::RomsLink if status.armed() => "roms.armed".into(),
            Id::RomsLink if status.stalled() => "roms.stalled_short".into(),
            Id::RomsLink if status.exfat() => "roms.link_exfat".into(),
            Id::RomsLink => "roms.link_ext4".into(),
            Id::RomsNames if self.roms_names() == 0 => (if self.roms.scan.is_some() {
                "roms.names_none"
            } else {
                ""
            })
            .into(),
            Id::RomsNames if status.exfat() => {
                format!("roms.renamed_count|count={}", self.roms_names())
            }
            Id::RomsNames => format!("roms.names_count|count={}", self.roms_names()),
            Id::RomsPower => match self.roms.scan.as_ref()?.battery {
                Some((_, true)) => "roms.charging".into(),
                Some((percent, false)) => format!("roms.battery|count={percent}"),
                None => "roms.mains".into(),
            },
            Id::RomsMove if status.armed() => "roms.armed".into(),
            Id::RomsMove => match self.roms_check()?.result.as_str() {
                "ok" => format!("roms.move_detail|count={}", self.roms_check()?.minutes()),
                "no-space" => "roms.no_space".into(),
                "low-power" => "roms.low_power".into(),
                "busy" => "roms.busy".into(),
                _ => "roms.unsupported".into(),
            },
            Id::RomsLeft => status.left.clone()?,
            _ => return None,
        })
    }
    /// The short reading on the right of a row.
    pub(super) fn roms_value(&self, id: Id) -> String {
        let check = self.roms_check();
        match id {
            Id::RomsNow => match check {
                Some(check) => format!(
                    "roms.now_value|size={}|count={}",
                    size(check.roms_mib),
                    check.files
                ),
                None if self.roms.scan.is_some() => String::new(),
                None => "roms.reading".into(),
            },
            Id::RomsNames if self.roms_names() > 0 => "roms.see_list".into(),
            Id::RomsPower => match check {
                Some(check) if check.power => "roms.power_ok".into(),
                Some(_) => "roms.power_low".into(),
                None => String::new(),
            },
            Id::RomsCard => match self.storage.roms.fs.as_str() {
                "errors" => "roms.fs_readonly",
                "failed" => "roms.fs_failed",
                "repaired" => "roms.fs_repaired",
                _ => "roms.fs_ok",
            }
            .into(),
            Id::RomsMove | Id::RomsCancel | Id::RomsRetry | Id::RomsLink => "›".into(),
            _ => String::new(),
        }
    }
    pub(super) fn roms_disabled(&self, id: Id) -> bool {
        id == Id::RomsMove
            && !self.storage.roms.armed()
            && !self.roms_check().is_some_and(Check::ok)
            || id == Id::RomsNames && self.roms_names() == 0
    }
    /// Rows that read as a warning: the partition failed, folders were left
    /// behind, the copy stalled.
    pub(crate) fn roms_warning(&self, id: Id) -> bool {
        let status = &self.storage.roms;
        match id {
            Id::RomsLink => status.fault() || status.left.is_some() || status.stalled(),
            Id::RomsLeft | Id::RomsRetry => true,
            _ => false,
        }
    }
    /// The power reading's colour: enough (true) or not.
    pub(crate) fn roms_ink(&self, id: Id) -> Option<bool> {
        (id == Id::RomsPower).then(|| self.roms_check().map(|c| c.power))?
    }
    /// A on a row of the page, or on the Storage link.
    pub(super) fn roms_activate(&mut self, id: Id) -> Option<Effect> {
        match id {
            // Every visit looks again.
            Id::RomsLink => {
                self.roms.scan = None;
                self.open(Page::Roms);
            }
            Id::RomsNames => self.open(Page::RomsList),
            Id::RomsMove if self.storage.roms.armed() => {
                self.reboot_prompt = Some("roms.now_reboot_question");
            }
            Id::RomsMove => self.storage.confirm = Some(storage::Confirm::RomsMove),
            Id::RomsCancel => return Some(self.roms_run(storage::Action::RomsCancel)),
            Id::RomsRetry => return Some(self.roms_run(storage::Action::RomsRetry)),
            _ => return None,
        }
        Some(Effect::None)
    }
    /// Runs an action of the page; the page looks again once it is done.
    pub(super) fn roms_run(&mut self, action: storage::Action) -> Effect {
        self.roms.scan = None;
        self.storage.working = Some(action.progress());
        Effect::Storage(action)
    }
    /// The list page scrolls through the names.
    pub(super) fn roms_list_input(&mut self, button: Button) -> Option<Effect> {
        let last = self.roms_names().saturating_sub(1);
        match button {
            Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::Down => self.cursor = (self.cursor + 1).min(last),
            Button::B => return None,
            _ => {}
        }
        Some(Effect::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::storage::{Action, Confirm};

    const EXT4: &str = "LAYOUT=ext4\n";
    const CHECK: &str = "LAYOUT=ext4\nROMS_FILES=1507\nROMS_MIB=7987\nSTORAGE_USED_MIB=9100\n\
        STORAGE_AFTER_MIB=10124\nROMS_PART_MIB=21504\nRENAMES=3\nSYMLINKS=1\nPOWER=ok\nRESULT=ok\n";

    fn page(status: &str, scan: Option<Scan>) -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Storage);
        state.storage.roms = Status::parse(status);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::RomsLink)
            .expect("the link shows");
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Roms);
        assert!(state.roms_wants_scan());
        state.roms.scan = scan;
        state
    }
    fn scan(check: &str) -> Scan {
        Scan {
            check: Check::parse(check),
            names: vec![
                ("snes/Zelda: Link.sfc".into(), "Zelda_ Link.sfc".into()),
                ("gba/mario.gba".into(), "mario (2).gba".into()),
            ],
            battery: Some((84, true)),
        }
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }

    #[test]
    fn check_and_status_parse_the_scripts_lines() {
        let check = Check::parse(CHECK).unwrap();
        assert_eq!(
            check,
            Check {
                files: 1507,
                roms_mib: 7987,
                storage_mib: 10124,
                part_mib: 21504,
                renames: 3,
                symlinks: 1,
                power: true,
                result: "ok".into(),
            }
        );
        assert_eq!(check.minutes(), 8);
        assert_eq!(
            renames("RENAMES=2\nRENAME=snes/A:B.sfc\tA_B.sfc\nRENAME=gba/Pokémon.gba\tPokémon (2).gba\n"),
            [
                ("snes/A:B.sfc".to_owned(), "A_B.sfc".to_owned()),
                ("gba/Pokémon.gba".into(), "Pokémon (2).gba".into()),
            ]
        );
        let low = Check::parse("LAYOUT=ext4\nPOWER=low\nRESULT=low-power\n").unwrap();
        assert!(!low.power && !low.ok() && low.files == 0);
        assert_eq!(Check::parse("usage: archr-roms-migrate"), None);
        let status = Status::parse(
            "LAYOUT=migrating\nSTATE=stalled\nPROGRESS=3/9\nENTRY=snes\n\
             ROMS_FS=errors\nNOT_MIGRATED=/storage/roms-not-migrated\n",
        );
        assert!(status.stalled() && status.fault() && !status.armed());
        assert_eq!(status.left.as_deref(), Some("/storage/roms-not-migrated"));
        assert!(!Status::parse("").shown());
        assert!(!Status::parse("LAYOUT=unsupported").shown());
        assert_eq!(size(8192), "@8 GB");
        assert_eq!(size(7987), "@7.8 GB");
        assert_eq!(size(21504), "@21 GB");
    }
    #[test]
    fn an_old_card_previews_the_move_and_asks_before_arming_it() {
        let mut state = page(EXT4, None);
        assert_eq!(state.roms_value(Id::RomsNow), "roms.reading");
        assert!(state.disabled(Id::RomsMove));
        state.roms.scan = Some(scan(CHECK));
        assert_eq!(
            ids(&state),
            [Id::RomsNow, Id::RomsNames, Id::RomsPower, Id::RomsMove]
        );
        assert_eq!(
            state.roms_value(Id::RomsNow),
            "roms.now_value|size=@7.8 GB|count=1507"
        );
        assert_eq!(
            state.subtitle(&state.visible_rows()[3]).unwrap(),
            "roms.move_detail|count=8"
        );
        // The list of names, then back.
        state.cursor = 1;
        state.handle(Button::A);
        assert_eq!(state.page(), Page::RomsList);
        for _ in 0..4 {
            state.handle(Button::Down);
        }
        assert_eq!(state.cursor, 1);
        state.handle(Button::B);
        assert_eq!(state.page(), Page::Roms);
        // Move: a question first, then the host arms it.
        state.cursor = 3;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.storage.confirm, Some(Confirm::RomsMove));
        assert_eq!(
            state.handle(Button::A),
            Effect::Storage(Action::RomsStart { files: 1507 })
        );
        assert!(state.storage.working.is_some() && state.roms.scan.is_none());
        // Armed: the host asks to restart now.
        state.storage_done(Ok("roms.start_reboot_question"));
        assert_eq!(state.reboot_prompt, Some("roms.start_reboot_question"));
        assert_eq!(state.handle(Button::A), Effect::Reboot);
    }
    #[test]
    fn a_move_that_cannot_run_says_why_and_does_nothing() {
        let check = CHECK.replace("RESULT=ok", "RESULT=no-space");
        let mut state = page(EXT4, Some(scan(&check)));
        state.cursor = 3;
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(state.storage.confirm.is_none());
        assert_eq!(
            state.subtitle(&state.visible_rows()[3]).unwrap(),
            "roms.no_space"
        );
    }
    #[test]
    fn armed_restarts_or_cancels_and_stalled_retries() {
        let mut state = page("LAYOUT=migrating\nSTATE=armed\n", Some(Scan::default()));
        assert_eq!(ids(&state), [Id::RomsMove, Id::RomsCancel]);
        state.handle(Button::A);
        assert_eq!(state.reboot_prompt, Some("roms.now_reboot_question"));
        state.handle(Button::B);
        state.cursor = 1;
        assert_eq!(state.handle(Button::A), Effect::Storage(Action::RomsCancel));
        state.storage_done(Ok("roms.cancelled"));
        let mut state = page(
            "LAYOUT=migrating\nSTATE=stalled\nNOT_MIGRATED=/storage/roms-not-migrated\n",
            Some(Scan::default()),
        );
        assert_eq!(ids(&state), [Id::RomsRetry, Id::RomsLeft]);
        assert_eq!(state.handle(Button::A), Effect::Storage(Action::RomsRetry));
    }
    #[test]
    fn a_moved_card_shows_only_its_state() {
        let mut state = page("LAYOUT=exfat\nROMS_FS=errors\n", Some(Scan::default()));
        assert_eq!(ids(&state), [Id::RomsCard]);
        assert_eq!(state.roms_value(Id::RomsCard), "roms.fs_readonly");
        state.roms.scan = Some(scan(EXT4));
        assert_eq!(ids(&state), [Id::RomsCard, Id::RomsNames]);
        assert_eq!(
            state.label(&state.visible_rows()[1]),
            "roms.renamed".to_owned()
        );
        state.handle(Button::B);
        assert_eq!(
            state.subtitle(
                &state
                    .visible_rows()
                    .iter()
                    .find(|r| r.id == Id::RomsLink)
                    .copied()
                    .unwrap()
            ),
            Some("roms.fault_short".into())
        );
    }
}
