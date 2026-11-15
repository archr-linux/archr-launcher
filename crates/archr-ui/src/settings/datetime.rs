//! Region and language › Date and time: the clock set by hand, one field per
//! row (day, month, year, hour, minute) moved with L and R (left and right), then
//! Apply. For a console without network whose RTC lost the date: a first boot
//! in the past cannot load the ArchR package key, created later.
use super::{row, Effect, Id, Kind, Page, Row, Settings};
use crate::state::Button;

pub const ROWS: &[Row] = &[
    row(Id::DateDay, "datetime.day", Kind::Value),
    row(Id::DateMonth, "datetime.month", Kind::Value),
    row(Id::DateYear, "datetime.year", Kind::Value),
    row(Id::DateHour, "datetime.hour", Kind::Value),
    row(Id::DateMinute, "datetime.minute", Kind::Value),
    row(Id::DateApply, "datetime.apply", Kind::Action),
];

/// The years offered: the ArchR key dates from 2026, the clock from 2024.
const YEARS: (i32, i32) = (2024, 2099);

/// The clock being set: year, month 1-12, day, hour, minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draft {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub minute: i32,
}

fn days_in(year: i32, month: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `value` moved by `step`, wrapping inside `min..=max`.
fn wrap(value: i32, step: i32, min: i32, max: i32) -> i32 {
    (value - min + step).rem_euclid(max - min + 1) + min
}

impl Draft {
    /// The local time now, or the earliest offered day when it reads earlier
    /// (a clock in the past is the reason this page exists).
    pub fn now() -> Self {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        crate::status::local_time(unix)
            .map(|t| Self {
                year: (t.tm_year + 1900).clamp(YEARS.0, YEARS.1),
                month: t.tm_mon + 1,
                day: t.tm_mday,
                hour: t.tm_hour,
                minute: t.tm_min,
            })
            .unwrap_or(Self {
                year: YEARS.0,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
            })
    }
    /// One field moved by `step`; the day follows a shorter month.
    pub fn step(&mut self, id: Id, step: i32) {
        match id {
            Id::DateDay => self.day = wrap(self.day, step, 1, days_in(self.year, self.month)),
            Id::DateMonth => self.month = wrap(self.month, step, 1, 12),
            Id::DateYear => self.year = wrap(self.year, step, YEARS.0, YEARS.1),
            Id::DateHour => self.hour = wrap(self.hour, step, 0, 23),
            Id::DateMinute => self.minute = wrap(self.minute, step, 0, 59),
            _ => return,
        }
        self.day = self.day.min(days_in(self.year, self.month));
    }
    /// What `archr-config time` takes.
    pub fn text(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute
        )
    }
}

impl Settings {
    pub(super) fn open_datetime(&mut self) {
        self.clock_draft = Some(Draft::now());
        self.open(Page::DateTime);
    }
    /// L and R move the field under the cursor, A on Apply sets the
    /// clock; up, down and B stay the page's (B leaves without setting).
    pub(super) fn datetime_input(&mut self, button: Button) -> Option<Effect> {
        let id = self.visible_rows().get(self.cursor)?.id;
        let step = match button {
            Button::L => -1,
            Button::R => 1,
            Button::A if id == Id::DateApply => {
                let draft = self.clock_draft.take()?;
                self.back();
                return Some(Effect::Set(Id::DateTime, draft.text()));
            }
            Button::B => {
                self.clock_draft = None;
                return None;
            }
            _ => return None,
        };
        if let Some(draft) = self.clock_draft.as_mut() {
            draft.step(id, step);
        }
        Some(Effect::None)
    }
    /// A field of the page being set: the month by its name.
    pub(super) fn datetime_field(&self, id: Id, catalog: &crate::i18n::I18n) -> String {
        let Some(d) = self.clock_draft else {
            return String::new();
        };
        match id {
            Id::DateDay => d.day.to_string(),
            Id::DateMonth => catalog.text(&format!("datetime.month_{}", d.month)),
            Id::DateYear => d.year.to_string(),
            Id::DateHour => format!("{:02}", d.hour),
            Id::DateMinute => format!("{:02}", d.minute),
            _ => String::new(),
        }
    }
    /// The Region row: "15 Nov 2026, 18:12" as the clock format says.
    pub(super) fn datetime_now(&self, catalog: &crate::i18n::I18n) -> String {
        let d = Draft::now();
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let clock = crate::status::local_clock(unix, self.value(Id::ClockFormat) == "12");
        let month = catalog.text(&format!("datetime.short_{}", d.month));
        format!("{} {} {}, {clock}", d.day, month, d.year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(year: i32, month: i32, day: i32) -> Draft {
        Draft { year, month, day, hour: 23, minute: 59 }
    }

    #[test]
    fn fields_wrap_and_the_day_follows_the_month() {
        let mut d = draft(2026, 1, 31);
        d.step(Id::DateMonth, 1);
        assert_eq!((d.month, d.day), (2, 28), "31 Jan to Feb");
        d.step(Id::DateYear, 2);
        assert_eq!(d.year, 2028);
        d.step(Id::DateDay, 1);
        assert_eq!(d.day, 29, "leap year");
        d.step(Id::DateDay, 1);
        assert_eq!(d.day, 1, "wraps");
        d.step(Id::DateMinute, 1);
        d.step(Id::DateHour, 1);
        assert_eq!((d.hour, d.minute), (0, 0));
        d.step(Id::DateMonth, -2);
        assert_eq!(d.month, 12);
        let mut d = draft(2099, 12, 1);
        d.step(Id::DateYear, 1);
        assert_eq!(d.year, 2024);
        assert_eq!(draft(2026, 3, 5).text(), "2026-03-05 23:59");
        assert!(crate::keyboard::datetime_valid(&Draft::now().text()));
    }

    #[test]
    fn the_page_sets_the_clock_only_from_apply() {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Region);
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::DateTime)
            .expect("Region lists the clock");
        state.handle(Button::A);
        assert_eq!(state.page(), Page::DateTime);
        let start = state.clock_draft.expect("a draft from now");
        state.cursor = 2; // year
        state.handle(Button::R);
        let after = state.clock_draft.unwrap();
        assert_ne!(after.year, start.year);
        state.cursor = ROWS.len() - 1;
        let effect = state.handle(Button::A);
        assert_eq!(effect, Effect::Set(Id::DateTime, after.text()));
        assert_eq!(state.page(), Page::Region);
        // B leaves without setting anything.
        state.cursor = state
            .visible_rows()
            .iter()
            .position(|r| r.id == Id::DateTime)
            .unwrap();
        state.handle(Button::A);
        assert_ne!(state.handle(Button::B), Effect::Set(Id::DateTime, after.text()));
        assert_eq!(state.page(), Page::Region);
        assert!(state.clock_draft.is_none());
    }
}
