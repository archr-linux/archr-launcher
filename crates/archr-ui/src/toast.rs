//! Toasts (Avisos): small cards in a top corner, under the status pill, that
//! say something just happened. They never take input: the screen under them
//! keeps working. The queue rules live here; the card is drawn in
//! `render/toast.rs`, the spool is read by the launcher.
use crate::settings::Id;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Any process may drop `<name>.toast` here (docs/avisos.md).
pub const SPOOL: &str = "/run/archr/notify";
/// Cards on screen at once; the next ones wait their turn.
pub const SHOWN: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Done,
    Info,
    Error,
}
/// The Notifications page's "Show" switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Updates,
    Network,
    Storage,
    Errors,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub kind: Kind,
    pub category: Category,
    pub title: String,
    /// Wrapped to two lines on the card; may be empty.
    pub detail: String,
}
impl Toast {
    pub fn new(kind: Kind, category: Category, title: impl Into<String>) -> Self {
        Self {
            kind,
            category,
            title: title.into(),
            detail: String::new(),
        }
    }
    pub fn detail(self, detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            ..self
        }
    }
    /// A spool file: kind, category and title lines, then an optional
    /// detail line. Anything else is refused.
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines().map(str::trim);
        let kind = match lines.next()? {
            "done" => Kind::Done,
            "info" => Kind::Info,
            "error" => Kind::Error,
            _ => return None,
        };
        let category = match lines.next()? {
            "updates" => Category::Updates,
            "network" => Category::Network,
            "storage" => Category::Storage,
            "errors" => Category::Errors,
            _ => return None,
        };
        let title = lines.next().filter(|t| !t.is_empty())?;
        let detail = lines.next().unwrap_or_default();
        lines
            .all(str::is_empty)
            .then(|| Self::new(kind, category, title).detail(detail))
    }
    /// The same news again: it refreshes the card instead of stacking.
    fn same(&self, other: &Self) -> bool {
        (self.kind, &self.title) == (other.kind, &other.title)
    }
}

/// Settings › Appearance › Notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub on: bool,
    pub left: bool,
    pub seconds: u64,
    /// Updates, network, storage and errors, in `Category` order.
    pub show: [bool; 4],
}
impl Prefs {
    pub fn read<'a>(value: impl Fn(Id) -> &'a str) -> Self {
        let on = |id| value(id) == "1";
        Self {
            on: on(Id::Toasts),
            left: value(Id::ToastsPosition) == "left",
            seconds: value(Id::ToastsDuration).parse().unwrap_or(4),
            show: [
                on(Id::ToastsUpdates),
                on(Id::ToastsNetwork),
                on(Id::ToastsStorage),
                on(Id::ToastsErrors),
            ],
        }
    }
    fn allows(&self, category: Category) -> bool {
        self.on && self.show[category as usize]
    }
    /// Errors stay twice as long, to be read.
    fn duration(&self, kind: Kind) -> Duration {
        Duration::from_secs(self.seconds * if kind == Kind::Error { 2 } else { 1 })
    }
}
impl Default for Prefs {
    fn default() -> Self {
        Self::read(Id::default_value)
    }
}

/// A card on screen: since when, and how long it stays from its last refresh.
#[derive(Clone, Debug)]
pub struct Shown {
    pub toast: Toast,
    pub since: Instant,
    until: Instant,
    total: Duration,
}
impl Shown {
    /// The time bar: 1 when it arrives, 0 when it leaves.
    pub fn left(&self, now: Instant) -> f32 {
        let left = self.until.saturating_duration_since(now).as_secs_f32();
        (left / self.total.as_secs_f32().max(0.001)).clamp(0., 1.)
    }
    /// Time to its nearest edge (arrival or departure), for the slide.
    pub fn edge(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.since)
            .min(self.until.saturating_duration_since(now))
    }
}

#[derive(Default)]
pub struct Toasts {
    pub prefs: Prefs,
    /// Newest first.
    shown: Vec<Shown>,
    waiting: VecDeque<Toast>,
}
impl Toasts {
    pub fn push(&mut self, toast: Toast, now: Instant) {
        if !self.prefs.allows(toast.category) {
            return;
        }
        if let Some(card) = self.shown.iter_mut().find(|s| s.toast.same(&toast)) {
            card.total = self.prefs.duration(toast.kind);
            card.until = now + card.total;
            card.toast = toast;
        } else if let Some(queued) = self.waiting.iter_mut().find(|t| t.same(&toast)) {
            *queued = toast;
        } else {
            self.waiting.push_back(toast);
            self.tick(now);
        }
    }
    /// Expired cards leave and waiting ones take their place; true when
    /// the cards changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let before = self.shown.len();
        self.shown.retain(|s| now < s.until);
        let mut changed = self.shown.len() != before;
        if !self.prefs.on {
            changed |= !self.shown.is_empty();
            self.shown.clear();
            self.waiting.clear();
        }
        while self.shown.len() < SHOWN {
            let Some(toast) = self.waiting.pop_front() else {
                break;
            };
            let total = self.prefs.duration(toast.kind);
            self.shown.insert(
                0,
                Shown {
                    toast,
                    since: now,
                    until: now + total,
                    total,
                },
            );
            changed = true;
        }
        changed
    }
    pub fn shown(&self) -> &[Shown] {
        &self.shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(title: &str) -> Toast {
        Toast::new(Kind::Info, Category::Updates, title)
    }
    fn titles(toasts: &Toasts) -> Vec<&str> {
        toasts
            .shown()
            .iter()
            .map(|s| s.toast.title.as_str())
            .collect()
    }

    #[test]
    fn three_on_screen_newest_on_top_and_the_fourth_waits() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        for (n, title) in ["a", "b", "c", "d"].into_iter().enumerate() {
            toasts.push(info(title), now + Duration::from_millis(n as u64));
        }
        assert_eq!(titles(&toasts), ["c", "b", "a"]);
        // "a" leaves first; "d" takes the top, its own time from then.
        let later = now + Duration::from_secs(4);
        assert!(toasts.tick(later));
        assert_eq!(titles(&toasts), ["d", "c", "b"]);
        assert_eq!(toasts.shown()[0].left(later), 1.);
    }
    #[test]
    fn the_same_toast_refreshes_its_card_instead_of_stacking() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(info("Wi-Fi connected").detail("Casa"), now);
        let later = now + Duration::from_secs(3);
        toasts.push(info("Wi-Fi connected").detail("Casa_5G"), later);
        assert_eq!(titles(&toasts), ["Wi-Fi connected"]);
        assert_eq!(toasts.shown()[0].toast.detail, "Casa_5G");
        assert_eq!(toasts.shown()[0].since, now, "no second arrival");
        assert!(!toasts.tick(now + Duration::from_secs(5)), "refreshed");
        // Another kind with the same title is other news.
        toasts.push(
            Toast::new(Kind::Error, Category::Updates, "Wi-Fi connected"),
            later,
        );
        assert_eq!(toasts.shown().len(), 2);
    }
    #[test]
    fn errors_stay_twice_as_long_and_cards_expire() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(info("done"), now);
        toasts.push(Toast::new(Kind::Error, Category::Errors, "failed"), now);
        toasts.tick(now + Duration::from_millis(3999));
        assert_eq!(toasts.shown().len(), 2);
        toasts.tick(now + Duration::from_secs(4));
        assert_eq!(titles(&toasts), ["failed"]);
        assert!(toasts.shown()[0].left(now + Duration::from_secs(6)) > 0.2);
        assert!(toasts.tick(now + Duration::from_secs(8)));
        assert!(toasts.shown().is_empty());
    }
    #[test]
    fn settings_choose_the_categories_the_time_and_the_corner() {
        let values = [
            (Id::Toasts, "1"),
            (Id::ToastsPosition, "left"),
            (Id::ToastsDuration, "6"),
            (Id::ToastsUpdates, "0"),
            (Id::ToastsNetwork, "1"),
            (Id::ToastsStorage, "1"),
            (Id::ToastsErrors, "1"),
        ];
        let prefs = Prefs::read(|id| values.iter().find(|v| v.0 == id).map_or("", |v| v.1));
        assert!(prefs.left);
        assert_eq!(prefs.duration(Kind::Error), Duration::from_secs(12));
        let defaults = Prefs::default();
        assert!(defaults.on && !defaults.left);
        assert_eq!(defaults.seconds, 4);
        assert_eq!(defaults.show, [true, true, false, true], "card and USB off");
        let now = Instant::now();
        let mut toasts = Toasts {
            prefs,
            ..Toasts::default()
        };
        toasts.push(info("update"), now);
        toasts.push(Toast::new(Kind::Done, Category::Storage, "card"), now);
        assert_eq!(titles(&toasts), ["card"]);
        toasts.prefs.on = false;
        toasts.push(Toast::new(Kind::Done, Category::Network, "wifi"), now);
        assert!(toasts.tick(now), "switched off: the cards go too");
        assert!(toasts.shown().is_empty());
    }
    #[test]
    fn spool_files_parse_or_are_refused() {
        assert_eq!(
            Toast::parse("done\nupdates\nBackup finished\n12 saves copied\n"),
            Some(
                Toast::new(Kind::Done, Category::Updates, "Backup finished")
                    .detail("12 saves copied")
            )
        );
        assert_eq!(
            Toast::parse("error\nstorage\nCard removed"),
            Some(Toast::new(Kind::Error, Category::Storage, "Card removed"))
        );
        for bad in [
            "",
            "warning\nupdates\nTitle",
            "info\nsound\nTitle",
            "info\nupdates\n",
            "info\nupdates\n  \ndetail",
            "info\nupdates\nTitle\ndetail\nmore",
        ] {
            assert_eq!(Toast::parse(bad), None, "{bad:?}");
        }
    }
}
