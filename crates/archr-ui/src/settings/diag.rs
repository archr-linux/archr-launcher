//! System › Diagnostics (Sistema-diagnostico): the debug mode, the crashes
//! pstore kept and the report. archr-debug owns system.loglevel and the
//! journal, archr-report writes the zip and, asked first, sends it; the host
//! runs them off SDL's thread.
use super::{described, Effect, Id, Kind, Page, Row, Settings};
use crate::{state::Button, HintButton};
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
    described(Id::ReportSend, "diag.send", "diag.send_detail").kind(Kind::Action),
];
/// The answers of the question before a report is sent.
pub const SEND_HINTS: &[(HintButton, &str)] = &[
    (HintButton::Key('A'), "diag.send_yes"),
    (HintButton::Key('B'), "hint.cancel"),
];

/// What `archr-report send` said, by its exit code, as a catalog key.
pub fn sent_message(code: i32) -> &'static str {
    match code {
        0 => "diag.sent",
        2 => "diag.send_off",
        3 => "diag.send_offline",
        4 => "diag.send_missing",
        5 => "diag.send_refused",
        _ => "diag.send_failed",
    }
}

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
    /// The question before sending the last report is open.
    pub confirm_send: bool,
    /// The last report is going out.
    pub sending: bool,
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
        !matches!(id, Id::ReportLast | Id::ReportSend) || self.diag_last().is_some()
    }
    /// Sending needs error reports on, and one thing at a time.
    pub(super) fn diag_send_disabled(&self) -> bool {
        self.value(Id::ErrorReports) != "1" || self.diag.sending || self.diag.running
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
            Id::ReportSend if self.diag_send_disabled() => String::new(),
            Id::ReportNow | Id::ReportSend | Id::DiagLink => "›".into(),
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
            Id::ReportSend if self.diag.sending => Some("diag.sending".into()),
            Id::ReportSend if self.value(Id::ErrorReports) != "1" => {
                Some("diag.send_needs_reports".into())
            }
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
            // Nothing leaves the console before the question is answered.
            Id::ReportSend if !self.diag_send_disabled() => {
                self.diag.confirm_send = true;
                Some(Effect::None)
            }
            _ => None,
        }
    }
    /// The question before sending: A sends, any other button cancels.
    pub(super) fn diag_input(&mut self, button: Button) -> Option<Effect> {
        if !self.diag.confirm_send {
            return None;
        }
        self.diag.confirm_send = false;
        let name = self.diag_last().map(|r| r.name.clone());
        Some(match name {
            Some(name) if button == Button::A && !self.diag_send_disabled() => {
                self.diag.sending = true;
                Effect::SendReport(name)
            }
            _ => Effect::None,
        })
    }
    /// The send's outcome, by `archr-report send`'s exit code.
    pub fn report_sent(&mut self, result: Result<i32, String>) {
        self.diag.sending = false;
        match result {
            Ok(0) => self.note(sent_message(0).into()),
            Ok(code) => self.fail(None, sent_message(code).into()),
            Err(error) => {
                log::warn!("archr-report send: {error}");
                self.fail(None, "diag.send_failed".into());
            }
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
            [
                Id::DebugMode,
                Id::Crashes,
                Id::ReportNow,
                Id::ReportLast,
                Id::ReportSend
            ]
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
    /// The page with a report on disk, the cursor on "Send the last report".
    fn with_report() -> Settings {
        let mut state = page();
        state.diag.facts = Some(Facts::parse(
            "",
            Some(Report {
                name: "archr-relatorio-20261113-0915.zip".into(),
                bytes: 900_000,
                at: SystemTime::now(),
            }),
        ));
        state.cursor = ids(&state)
            .iter()
            .position(|id| *id == Id::ReportSend)
            .expect("a report on disk offers sending it");
        state
    }
    #[test]
    fn each_exit_code_of_the_send_has_its_message() {
        for (code, key) in [
            (0, "diag.sent"),
            (2, "diag.send_off"),
            (3, "diag.send_offline"),
            (4, "diag.send_missing"),
            (5, "diag.send_refused"),
            (1, "diag.send_failed"),
            (137, "diag.send_failed"),
        ] {
            assert_eq!(sent_message(code), key, "exit {code}");
        }
        let mut state = with_report();
        state.diag.sending = true;
        state.report_sent(Ok(0));
        assert!(!state.diag.sending);
        assert_eq!(state.error, Some((None, "diag.sent".into())));
        assert!(state.brief, "success is a brief note");
        state.report_sent(Ok(3));
        assert_eq!(state.error, Some((None, "diag.send_offline".into())));
        assert!(!state.brief);
        state.report_sent(Err("timeout".into()));
        assert_eq!(state.error, Some((None, "diag.send_failed".into())));
    }
    #[test]
    fn sending_waits_for_the_question_and_b_cancels() {
        let mut state = with_report();
        assert_eq!(state.handle(Button::A), Effect::None, "only asks");
        assert!(state.diag.confirm_send && !state.diag.sending);
        assert_eq!(state.hints(), SEND_HINTS);
        assert_eq!(state.handle(Button::B), Effect::None);
        assert!(!state.diag.confirm_send && !state.diag.sending);
        assert_eq!(
            state.page(),
            Page::Diagnostics,
            "B closes only the question"
        );
        state.handle(Button::A);
        assert_eq!(
            state.handle(Button::A),
            Effect::SendReport("archr-relatorio-20261113-0915.zip".into())
        );
        assert!(state.diag.sending && !state.diag.confirm_send);
        assert_eq!(
            state.diag_subtitle(Id::ReportSend).as_deref(),
            Some("diag.sending")
        );
        assert_eq!(state.handle(Button::A), Effect::None, "one at a time");
        assert!(!state.diag.confirm_send);
    }
    #[test]
    fn sending_is_shown_but_disabled_while_error_reports_are_off() {
        let mut state = with_report();
        assert!(!state.disabled(Id::ReportSend), "on by default");
        assert_eq!(state.diag_value(Id::ReportSend, SystemTime::now()), "›");
        state.values.insert(Id::ErrorReports, "0".into());
        assert!(ids(&state).contains(&Id::ReportSend), "shown, not hidden");
        assert!(state.disabled(Id::ReportSend));
        assert_eq!(state.diag_value(Id::ReportSend, SystemTime::now()), "");
        assert_eq!(
            state.diag_subtitle(Id::ReportSend).as_deref(),
            Some("diag.send_needs_reports")
        );
        assert_eq!(state.handle(Button::A), Effect::None);
        assert!(!state.diag.confirm_send, "no question either");
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
