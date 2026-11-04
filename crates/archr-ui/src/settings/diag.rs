//! System › Diagnostics (Sistema-diagnostico): the debug mode, the crashes
//! pstore kept and the report. archr-debug owns system.loglevel and the
//! journal, archr-report writes the zip; the host runs both off SDL's thread.
use super::{described, Effect, Id, Kind, Page, Row, Settings};
use std::time::SystemTime;

pub const ROWS: &[Row] = &[
    described(Id::DebugMode, "diag.debug", "diag.debug_detail")
        .kind(Kind::Toggle)
        .under("diag.debug_section"),
    // A reading: as an action its subtitle may use the width the value leaves.
    described(Id::Crashes, "diag.crashes", "diag.crashes_detail").kind(Kind::Action),
    described(Id::ReportNow, "diag.report_now", "diag.report_time")
        .kind(Kind::Action)
        .under("diag.report_section"),
    described(Id::ReportLast, "diag.report_last", "").kind(Kind::Action),
];

/// The newest zip in roms/relatorios.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub name: String,
    pub bytes: u64,
    pub at: SystemTime,
}
/// `archr-debug status` and the last report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The pstore backend; None when the kernel has none (no ramoops node).
    pub backend: Option<String>,
    pub crashes: usize,
    pub last: Option<Report>,
}
impl Facts {
    /// `KEY=VALUE` lines of `archr-debug status`.
    pub fn parse(status: &str, last: Option<Report>) -> Self {
        let get = |key: &str| {
            status
                .lines()
                .find_map(|line| line.trim().strip_prefix(key)?.strip_prefix('='))
                .unwrap_or_default()
        };
        Self {
            backend: Some(get("PSTORE"))
                .filter(|b| !b.is_empty() && *b != "none")
                .map(str::to_owned),
            crashes: get("CRASHES").parse().unwrap_or(0),
            last,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Diag {
    pub facts: Option<Facts>,
    pub scanning: bool,
    /// A report is being written.
    pub running: bool,
}

/// "1.4 MB", "640 KB": the size of a report, with the number localized.
pub fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("@{:.1} MB", bytes as f64 / (1024. * 1024.))
    } else {
        format!("@{} KB", bytes.div_ceil(1024))
    }
}

impl Settings {
    /// Nothing known yet about an open page: the host looks.
    pub fn diag_wants_scan(&self) -> bool {
        self.page() == Page::Diagnostics && self.diag.facts.is_none() && !self.diag.scanning
    }
    pub(super) fn diag_visible(&self, id: Id) -> bool {
        id != Id::ReportLast || self.diag_last().is_some()
    }
    fn diag_last(&self) -> Option<&Report> {
        self.diag.facts.as_ref()?.last.as_ref()
    }
    /// The reading on the right of a row, as a catalog reference.
    pub(super) fn diag_value(&self, id: Id, now: SystemTime) -> String {
        match id {
            Id::Crashes => match &self.diag.facts {
                None => String::new(),
                Some(f) if f.backend.is_none() => "diag.crashes_unavailable".into(),
                Some(f) if f.crashes == 0 => "diag.crashes_none".into(),
                Some(f) => format!("diag.crashes_count|count={}", f.crashes),
            },
            Id::ReportNow if self.diag.running => String::new(),
            Id::ReportNow | Id::DiagLink => "›".into(),
            Id::ReportLast => match self.diag_last() {
                Some(report) => {
                    let minutes = now.duration_since(report.at).unwrap_or_default().as_secs() / 60;
                    match minutes {
                        0 => "time.now".into(),
                        m if m < 60 => format!("time.minutes|count={m}"),
                        m if m < 48 * 60 => format!("time.hours|count={}", m / 60),
                        m => format!("time.days|count={}", m / (24 * 60)),
                    }
                }
                None => String::new(),
            },
            _ => String::new(),
        }
    }
    pub(super) fn diag_subtitle(&self, id: Id) -> Option<String> {
        match id {
            Id::ReportNow if self.diag.running => Some("diag.report_running".into()),
            Id::ReportLast => self
                .diag_last()
                .map(|r| format!("{} · {}", r.name, size(r.bytes))),
            _ => None,
        }
    }
    /// Crashes kept read as a warning; none, as good news.
    pub(crate) fn diag_ink(&self, id: Id) -> Option<bool> {
        let facts = self.diag.facts.as_ref().filter(|_| id == Id::Crashes)?;
        facts.backend.as_ref()?;
        Some(facts.crashes == 0)
    }
    pub(super) fn diag_activate(&mut self, id: Id) -> Option<Effect> {
        match id {
            // Every visit looks again.
            Id::DiagLink => {
                self.diag.facts = None;
                self.open(Page::Diagnostics);
                Some(Effect::None)
            }
            Id::ReportNow if self.diag.running => Some(Effect::None),
            Id::ReportNow => {
                self.diag.running = true;
                Some(Effect::Report)
            }
            _ => None,
        }
    }
    /// The report's outcome: the page shows where it went.
    pub fn report_done(&mut self, result: Result<Report, String>) {
        self.diag.running = false;
        match result {
            Ok(report) => {
                let message = format!("diag.report_saved|name={}", report.name);
                if let Some(facts) = &mut self.diag.facts {
                    facts.last = Some(report);
                }
                self.fail(None, message);
            }
            Err(error) => {
                log::warn!("archr-report: {error}");
                self.fail(None, "diag.report_failed".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Button;
    use std::time::Duration;

    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::System);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::DiagLink)
            .expect("System links to Diagnostics");
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Diagnostics);
        assert!(state.diag_wants_scan());
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }

    #[test]
    fn status_parses_the_backend_and_the_crashes() {
        let facts = Facts::parse(
            "DEBUG=1\nJOURNAL=persistent\nPSTORE=ramoops\nCRASHES=2\n",
            None,
        );
        assert_eq!(facts.backend.as_deref(), Some("ramoops"));
        assert_eq!(facts.crashes, 2);
        assert_eq!(Facts::parse("PSTORE=none\nCRASHES=0", None).backend, None);
        assert_eq!(Facts::parse("", None), Facts::default());
        assert_eq!(size(1_468_006), "@1.4 MB");
        assert_eq!(size(2_000), "@2 KB");
    }
    #[test]
    fn the_page_reads_pstore_and_offers_the_last_report() {
        let mut state = page();
        assert_eq!(
            ids(&state),
            [Id::DebugMode, Id::Crashes, Id::ReportNow],
            "no report yet"
        );
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        assert_eq!(state.diag_value(Id::Crashes, now), "");
        state.diag.facts = Some(Facts::parse("PSTORE=none\n", None));
        assert_eq!(
            state.diag_value(Id::Crashes, now),
            "diag.crashes_unavailable"
        );
        assert_eq!(state.diag_ink(Id::Crashes), None);
        state.diag.facts = Some(Facts::parse(
            "PSTORE=ramoops\nCRASHES=0\n",
            Some(Report {
                name: "archr-relatorio-20261030-2236.zip".into(),
                bytes: 1_468_006,
                at: now - Duration::from_secs(150),
            }),
        ));
        assert_eq!(state.diag_value(Id::Crashes, now), "diag.crashes_none");
        assert_eq!(state.diag_ink(Id::Crashes), Some(true));
        assert_eq!(
            ids(&state),
            [Id::DebugMode, Id::Crashes, Id::ReportNow, Id::ReportLast]
        );
        assert_eq!(
            state.diag_value(Id::ReportLast, now),
            "time.minutes|count=2"
        );
        assert_eq!(
            state.diag_subtitle(Id::ReportLast).unwrap(),
            "archr-relatorio-20261030-2236.zip · @1.4 MB"
        );
        assert!(state.fixed(Id::Crashes) && state.fixed(Id::ReportLast));
    }
    #[test]
    fn a_report_runs_once_and_says_where_it_went() {
        let mut state = page();
        state.diag.facts = Some(Facts::default());
        state.cursor = 2;
        assert_eq!(state.handle(Button::A), Effect::Report);
        assert_eq!(state.handle(Button::A), Effect::None, "one at a time");
        assert_eq!(
            state.diag_subtitle(Id::ReportNow).as_deref(),
            Some("diag.report_running")
        );
        let report = Report {
            name: "archr-relatorio-20261031-0102.zip".into(),
            bytes: 90_000,
            at: SystemTime::now(),
        };
        state.report_done(Ok(report.clone()));
        assert!(!state.diag.running);
        assert_eq!(state.diag_last(), Some(&report));
        assert_eq!(
            state.error,
            Some((
                None,
                "diag.report_saved|name=archr-relatorio-20261031-0102.zip".into()
            ))
        );
        state.report_done(Err("timeout".into()));
        assert_eq!(state.error, Some((None, "diag.report_failed".into())));
    }
    #[test]
    fn the_debug_switch_is_a_setting_the_host_applies() {
        let mut state = page();
        state.diag.facts = Some(Facts::default());
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::DebugMode, "1".into())
        );
    }
}
