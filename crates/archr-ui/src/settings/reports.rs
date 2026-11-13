//! System › Error reports (Relatorios-ajustes) and the last report's text
//! (Relatorios-ver), plus the notice that reports are on (Relatorios-aviso).
//! archr-error-report writes and sends the reports; the host reads its
//! folders when the page opens.
use super::{described, Effect, Id, Kind, Page, Row, Settings};
use crate::state::Button;

/// archr-error-report's daily limit.
pub const DAILY: usize = 10;
/// The viewer wraps the text at this many characters and shows this many
/// lines (monospace; the renderer fits its size to both).
pub const COLUMNS: usize = 78;
pub const PAGE_LINES: usize = 19;

pub const ROWS: &[Row] = &[
    described(Id::ErrorReports, "reports.send", "reports.send_detail")
        .kind(Kind::Toggle)
        .under("reports.section"),
    described(Id::ReportsQueue, "reports.queue", "reports.queue_detail").kind(Kind::Action),
    described(Id::ReportsToday, "reports.today", "reports.today_detail").kind(Kind::Action),
    described(
        Id::ConsoleId,
        "reports.console_id",
        "reports.console_id_detail",
    )
    .kind(Kind::Action),
    described(Id::ReportsLatest, "", "")
        .kind(Kind::Action)
        .under("reports.last_section"),
    described(Id::ReportsView, "reports.view", "reports.view_detail").kind(Kind::Action),
];

/// The newest report, queued or sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Last {
    /// `2026-11-10_12-31-05.txt`.
    pub name: String,
    pub sent: bool,
    /// When it went out, `HH:MM`, for the viewer's footer.
    pub sent_at: String,
    /// The text as it was sent, wrapped at `COLUMNS`.
    pub lines: Vec<String>,
}
impl Last {
    pub fn new(name: String, sent: bool, sent_at: String, text: &str) -> Self {
        let lines = text
            .lines()
            .flat_map(|line| {
                let chars: Vec<char> = line.chars().filter(|c| !c.is_control()).collect();
                let pieces: Vec<String> = chars
                    .chunks(COLUMNS)
                    .map(|piece| piece.iter().collect())
                    .collect();
                if pieces.is_empty() {
                    vec![String::new()]
                } else {
                    pieces
                }
            })
            .collect();
        Self {
            name,
            sent,
            sent_at,
            lines,
        }
    }
    /// A `name: value` of the header (the lines before the first section).
    fn field(&self, key: &str) -> &str {
        self.lines
            .iter()
            .take_while(|line| !line.starts_with("---"))
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
            .unwrap_or_default()
    }
    /// What failed, as a catalog reference.
    pub fn title(&self) -> String {
        let what = self.field("what").replace('|', "");
        match self.field("event") {
            "game" => format!("reports.failed_game|what={what}"),
            "unit" => format!(
                "reports.failed_unit|what={}",
                what.trim_end_matches(".service")
            ),
            "module" => format!("reports.failed_module|what={what}"),
            "kernel" => "reports.failed_kernel".into(),
            "launcher" => "reports.failed_launcher".into(),
            _ => "reports.failed".into(),
        }
    }
    /// "10/11 12:31 · Nintendo 64 · Super Mario 64 (USA)".
    pub fn detail(&self) -> String {
        let stem = &self.name[..self.name.len().min(19)];
        let (day, time) = stem.split_once('_').unwrap_or_default();
        let mut date = day.splitn(3, '-').skip(1);
        let (month, day) = (date.next().unwrap_or(""), date.next().unwrap_or(""));
        let time = time.get(..5).unwrap_or("").replace('-', ":");
        // "n64 (Nintendo 64)": the name, else the folder.
        let system = self.field("system");
        let system = system
            .split_once(" (")
            .map_or(system, |(_, name)| name.trim_end_matches(')'));
        let game = self.field("game");
        let game = game.rsplit_once('.').map_or(game, |(stem, _)| stem);
        let place: Vec<&str> = [system, game]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        let when = format!("day={day}|month={month}|time={time}");
        if place.is_empty() {
            return format!("reports.last_when|{when}");
        }
        format!(
            "reports.last_detail|{when}|place={}",
            place.join(" · ").replace('|', "")
        )
    }
}
/// `archr-console-id`'s answer, when it is one: 12 lowercase hex digits.
pub fn console_id(output: &str) -> Option<String> {
    let id = output.trim();
    (id.len() == 12 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| id.to_owned())
}
/// What the host read from archr-error-report's folders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub queued: usize,
    pub sent_today: usize,
    pub last: Option<Last>,
    /// This console's id (archr-console-id); None when the system has none.
    pub console_id: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Reports {
    pub facts: Option<Facts>,
    pub scanning: bool,
    /// The viewer's first line.
    pub scroll: usize,
}

/// The notice that reports are on, over the Start screen once: "OK" or
/// "Turn off reports" (Relatorios-aviso).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notice {
    /// "OK" has the focus (else "Turn off reports").
    pub on_ok: bool,
}
impl Default for Notice {
    fn default() -> Self {
        Self { on_ok: true }
    }
}
impl Notice {
    /// ‹ › move between the buttons; A answers the focused one, B keeps
    /// them on. Some(true) keeps reports on.
    pub fn handle(&mut self, button: Button) -> Option<bool> {
        match button {
            Button::L | Button::R => self.on_ok = !self.on_ok,
            Button::A => return Some(self.on_ok),
            Button::B => return Some(true),
            _ => {}
        }
        None
    }
}

impl Settings {
    /// Nothing known yet about an open page: the host looks.
    pub fn reports_wants_scan(&self) -> bool {
        self.page() == Page::ErrorReports && self.reports.facts.is_none() && !self.reports.scanning
    }
    pub fn reports_last(&self) -> Option<&Last> {
        self.reports.facts.as_ref()?.last.as_ref()
    }
    fn console(&self) -> Option<&str> {
        self.reports.facts.as_ref()?.console_id.as_deref()
    }
    pub(super) fn reports_visible(&self, id: Id) -> bool {
        match id {
            Id::ConsoleId => self.console().is_some(),
            Id::ReportsLatest | Id::ReportsView => self.reports_last().is_some(),
            _ => true,
        }
    }
    pub(super) fn reports_value(&self, id: Id) -> String {
        let facts = self.reports.facts.as_ref();
        match (id, facts) {
            (Id::ReportsLink | Id::ReportsView, _) => "›".into(),
            (Id::ReportsQueue, Some(f)) => format!("@{}", f.queued),
            (Id::ConsoleId, _) => self.console().unwrap_or_default().into(),
            (Id::ReportsToday, Some(f)) => {
                format!("reports.today_count|count={}|limit={DAILY}", f.sent_today)
            }
            (Id::ReportsLatest, _) => match self.reports_last() {
                Some(last) if last.sent => "reports.sent".into(),
                Some(_) => "reports.queued".into(),
                None => String::new(),
            },
            _ => String::new(),
        }
    }
    pub(super) fn reports_activate(&mut self, id: Id) -> Option<Effect> {
        match id {
            // Every visit looks again.
            Id::ReportsLink => {
                self.reports.facts = None;
                self.open(Page::ErrorReports);
                Some(Effect::None)
            }
            Id::ReportsView if self.reports_last().is_some() => {
                self.reports.scroll = 0;
                self.open(Page::ReportText);
                Some(Effect::None)
            }
            _ => None,
        }
    }
    /// The viewer scrolls by a line (↑ ↓) or a page (L R).
    pub(super) fn report_text_input(&mut self, button: Button) -> Option<Effect> {
        let lines = self.reports_last().map_or(0, |last| last.lines.len());
        let last = lines.saturating_sub(PAGE_LINES);
        let scroll = &mut self.reports.scroll;
        match button {
            Button::Up => *scroll = scroll.saturating_sub(1),
            Button::Down => *scroll = (*scroll + 1).min(last),
            Button::L => *scroll = scroll.saturating_sub(PAGE_LINES),
            Button::R => *scroll = (*scroll + PAGE_LINES).min(last),
            Button::B => return None,
            _ => {}
        }
        Some(Effect::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "ArchR error report\nwhen: 2026-11-10 12:31:05 -0300\nevent: game\n\
        what: mupen64plus-arch\nsystem: n64 (Nintendo 64)\ngame: Super Mario 64 (USA).z64\n\
        exit: 139 (SIGSEGV) after 42 s\n\n--- run output ---\ngame: not a header line\n";

    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::System);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::ReportsLink)
            .expect("System links to Error reports");
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::ErrorReports);
        assert!(state.reports_wants_scan());
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }
    fn last(text: &str) -> Last {
        Last::new("2026-11-10_12-31-05.txt".into(), true, "12:32".into(), text)
    }

    #[test]
    fn the_header_names_what_failed_where_and_when() {
        let last = last(TEXT);
        assert_eq!(last.title(), "reports.failed_game|what=mupen64plus-arch");
        assert_eq!(
            last.detail(),
            "reports.last_detail|day=10|month=11|time=12:31|place=Nintendo 64 · Super Mario 64 (USA)"
        );
        let kernel = Last::new(
            "2026-01-02_03-04-05.txt".into(),
            false,
            String::new(),
            "event: kernel\nwhat: kernel oops\n",
        );
        assert_eq!(kernel.title(), "reports.failed_kernel");
        assert_eq!(
            kernel.detail(),
            "reports.last_when|day=02|month=01|time=03:04"
        );
        assert_eq!(Last::default().title(), "reports.failed");
        let unit = Last::new(
            "2026-01-02_03-04-05.txt".into(),
            true,
            String::new(),
            "event: unit\nwhat: iwd.service\n",
        );
        assert_eq!(unit.title(), "reports.failed_unit|what=iwd");
    }
    #[test]
    fn long_lines_wrap_and_blank_lines_stay() {
        let text = format!("a\n\n{}\n", "x".repeat(COLUMNS + 5));
        let last = last(&text);
        assert_eq!(last.lines.len(), 4);
        assert_eq!(last.lines[1], "");
        assert_eq!(last.lines[2].chars().count(), COLUMNS);
        assert_eq!(last.lines[3], "xxxxx");
    }
    #[test]
    fn the_page_shows_the_queue_today_and_the_last_report() {
        let mut state = page();
        assert_eq!(
            ids(&state),
            [Id::ErrorReports, Id::ReportsQueue, Id::ReportsToday],
            "no report yet"
        );
        assert_eq!(state.reports_value(Id::ReportsQueue), "", "not read yet");
        state.reports.facts = Some(Facts {
            queued: 1,
            sent_today: 2,
            last: Some(last(TEXT)),
            console_id: None,
        });
        assert_eq!(
            ids(&state),
            [
                Id::ErrorReports,
                Id::ReportsQueue,
                Id::ReportsToday,
                Id::ReportsLatest,
                Id::ReportsView
            ]
        );
        assert_eq!(state.reports_value(Id::ReportsQueue), "@1");
        assert_eq!(
            state.reports_value(Id::ReportsToday),
            "reports.today_count|count=2|limit=10"
        );
        assert_eq!(state.reports_value(Id::ReportsLatest), "reports.sent");
        assert!(state.fixed(Id::ReportsQueue) && state.fixed(Id::ReportsLatest));
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::ErrorReports, "0".into()),
            "on by default; the host saves the switch"
        );
    }
    #[test]
    fn the_console_id_shows_only_when_the_system_gave_one() {
        assert_eq!(
            console_id("3f9a0c2b7d14\n").as_deref(),
            Some("3f9a0c2b7d14")
        );
        for bad in [
            "",
            "3F9A0C2B7D14",
            "3f9a0c2b7d1",
            "3f9a0c2b7d14a",
            "3f9a0c2b7d1g",
        ] {
            assert_eq!(console_id(bad), None, "{bad:?}");
        }
        let mut state = page();
        state.reports.facts = Some(Facts::default());
        assert!(!ids(&state).contains(&Id::ConsoleId), "no id: no row");
        state.reports.facts = Some(Facts {
            console_id: Some("3f9a0c2b7d14".into()),
            ..Facts::default()
        });
        assert_eq!(
            ids(&state),
            [
                Id::ErrorReports,
                Id::ReportsQueue,
                Id::ReportsToday,
                Id::ConsoleId
            ]
        );
        assert_eq!(state.reports_value(Id::ConsoleId), "3f9a0c2b7d14");
        assert!(state.fixed(Id::ConsoleId), "a reading, not an action");
    }
    #[test]
    fn the_viewer_scrolls_by_lines_and_pages_and_b_goes_back() {
        let mut state = page();
        let text: String = (0..30).map(|n| format!("line {n}\n")).collect();
        state.reports.facts = Some(Facts {
            last: Some(last(&text)),
            ..Facts::default()
        });
        state.cursor = ids(&state)
            .iter()
            .position(|id| *id == Id::ReportsView)
            .unwrap();
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::ReportText);
        state.handle(Button::Up);
        assert_eq!(state.reports.scroll, 0);
        state.handle(Button::Down);
        assert_eq!(state.reports.scroll, 1);
        state.handle(Button::R);
        assert_eq!(state.reports.scroll, 30 - PAGE_LINES, "never past the end");
        state.handle(Button::L);
        assert_eq!(state.reports.scroll, 0);
        state.handle(Button::B);
        assert_eq!(state.page(), Page::ErrorReports);
    }
    #[test]
    fn the_notice_keeps_reports_on_unless_turn_off_is_chosen() {
        let mut notice = Notice::default();
        assert_eq!(notice.handle(Button::Down), None);
        assert_eq!(notice.handle(Button::A), Some(true));
        assert_eq!(notice.handle(Button::R), None);
        assert_eq!(notice.handle(Button::A), Some(false));
        assert_eq!(notice.handle(Button::B), Some(true), "B keeps them on");
    }
}
