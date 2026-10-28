//! System information model and two-column navigation, independent of settings rows.
use super::{Effect, Page, Settings};
use crate::{icons::Glyph, state::Button};

pub(crate) const CPU: usize = 0;
pub(crate) const STORAGE: usize = 3;
pub(crate) const ITEMS: [(&str, Glyph); 6] = [
    ("settings.processor", Glyph::Cpu),
    ("settings.graphics", Glyph::Gpu),
    ("settings.memory", Glyph::Memory),
    ("settings.storage", Glyph::Storage),
    ("settings.network", Glyph::Wifi),
    ("settings.battery", Glyph::Battery),
];
#[derive(Clone, Default)]
pub struct InfoMeter {
    pub label: String,
    pub value: String,
    pub percent: Option<u8>,
}
#[derive(Clone, Default)]
pub struct InfoDetail {
    pub heading: String,
    pub subtitle: String,
    pub rows: Vec<(&'static str, String)>,
    pub meters: Vec<InfoMeter>,
}
#[derive(Clone, Default)]
pub struct InfoCard {
    /// Summary: prominent main line, then secondary line.
    pub lines: Vec<String>,
    pub percent: Option<u8>,
    pub detail: InfoDetail,
}
#[derive(Clone, Default)]
pub struct Information {
    pub version: String,
    pub device: String,
    pub kernel: String,
    pub cards: Vec<InfoCard>,
}
impl Settings {
    pub(super) fn information_input(&mut self, button: Button) -> Option<Effect> {
        match self.page() {
            Page::Info => {
                match button {
                    Button::Up if self.cursor >= 2 => self.cursor -= 2,
                    Button::Down if self.cursor < 4 => self.cursor += 2,
                    Button::L => self.cursor &= !1,
                    Button::R => self.cursor |= 1,
                    Button::A => self.open(Page::InfoDetail(self.cursor)),
                    Button::B => return None,
                    _ => {}
                }
                Some(Effect::None)
            }
            Page::InfoDetail(item) => {
                let next = match button {
                    Button::L => (item + 5) % 6,
                    Button::R => (item + 1) % 6,
                    Button::B => return None,
                    _ => return Some(Effect::None),
                };
                self.stack.last_mut().expect("detail page").0 = Page::InfoDetail(next);
                // Returning from a paged detail focuses the item now on screen.
                let parent = self.stack.len() - 2;
                self.stack[parent].1 = next;
                Some(Effect::None)
            }
            _ => None,
        }
    }
}

/// Tagged translations carry their own parameter names. Only explicit numeric
/// tokens (`@1234.5`) receive grouping/decimal localization, never IPs or versions.
pub(crate) fn value(catalog: &crate::i18n::I18n, text: &str) -> String {
    let mut fields = text.split('|');
    let key = fields.next().unwrap_or_default();
    if key.starts_with("settings.") {
        let fields: Vec<_> = fields.filter_map(|f| f.split_once('=')).collect();
        if fields.is_empty() {
            return catalog.text(key);
        }
        let count = fields
            .iter()
            .find(|(name, _)| *name == "count")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0.);
        let params: Vec<_> = fields
            .iter()
            .map(|(name, text)| (*name, value(catalog, text)))
            .collect();
        return catalog.plural(
            key,
            count,
            &params
                .iter()
                .map(|(k, v)| (*k, v.as_str()))
                .collect::<Vec<_>>(),
        );
    }
    text.split_inclusive(char::is_whitespace)
        .map(|token| {
            let Some(number) = token.strip_prefix('@') else {
                return token.to_owned();
            };
            let digits = number.trim_end();
            let (integer, fraction) = digits
                .split_once('.')
                .map_or((digits, None), |(a, b)| (a, Some(b)));
            let (sign, integer) = integer
                .strip_prefix('-')
                .map_or(("", integer), |n| ("-", n));
            if integer.is_empty()
                || !integer.bytes().all(|b| b.is_ascii_digit())
                || fraction.is_some_and(|f| !f.bytes().all(|b| b.is_ascii_digit()))
            {
                return token.to_owned();
            }
            let grouped: String = integer
                .chars()
                .enumerate()
                .flat_map(|(i, ch)| {
                    let separator = i > 0 && (integer.len() - i) % 3 == 0;
                    separator
                        .then_some(' ')
                        .into_iter()
                        .chain(std::iter::once(ch))
                })
                .collect();
            let decimal = if catalog.language() == "en" { '.' } else { ',' };
            format!(
                "{sign}{grouped}{}{}",
                fraction
                    .map(|f| format!("{decimal}{f}"))
                    .unwrap_or_default(),
                &number[digits.len()..]
            )
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn localized_values_group_numbers_pluralize_cores_and_preserve_ips() {
        let catalog =
            crate::i18n::I18n::load(std::path::Path::new("/nonexistent"), Some("ru"), None);
        assert_eq!(value(&catalog, "@1008 MHz"), "1 008 MHz");
        assert_eq!(
            value(&catalog, "@0.42 · @0.31 · @0.18"),
            "0,42 · 0,31 · 0,18"
        );
        assert_eq!(
            value(&catalog, "192.168.1.1 · Mesa 26.2.5"),
            "192.168.1.1 · Mesa 26.2.5"
        );
        for (count, noun) in [(1, "ядро"), (4, "ядра"), (5, "ядер")] {
            assert_eq!(
                value(
                    &catalog,
                    &format!("settings.cpu_summary|model=RK|count={count}")
                ),
                format!("RK · {count} {noun}")
            );
        }
        for (tag, want) in [
            ("settings.duration_hours|hours=2|minutes=59", "2 ч 59 мин"),
            (
                "settings.uptime_summary_hours|hours=2|minutes=59",
                "работает 2 ч 59 мин",
            ),
            (
                "settings.uptime_summary_minutes|count=59",
                "работает 59 мин",
            ),
        ] {
            assert_eq!(value(&catalog, tag), want);
        }
    }
}
