//! USB Wi-Fi and Bluetooth adapters as archr-usb-advisor reports them: the
//! build table (/usr/share/archr/usb-advisor.tsv), what is plugged now
//! (/run/archr/usb-advice) and what was seen before
//! (/storage/.config/archr/usb-seen). Parsing only; the files are read off
//! the draw thread.
use crate::i18n::I18n;

pub const TABLE: &str = "/usr/share/archr/usb-advisor.tsv";
pub const ADVICE: &str = "/run/archr/usb-advice";
pub const SEEN: &str = "/storage/.config/archr/usb-seen";
/// The last announcement the launcher showed, so a restart does not repeat it.
pub const ACK: &str = "/run/archr/usb-advice.ack";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Works,
    NoDriver,
    Firmware,
    NotBound,
}
impl Verdict {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "works" => Self::Works,
            "no_driver" => Self::NoDriver,
            "firmware" => Self::Firmware,
            "not_bound" => Self::NotBound,
            _ => return None,
        })
    }
    /// The list's badge.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Works => "usb.works",
            Self::NoDriver => "usb.no_driver",
            Self::Firmware => "usb.firmware_missing",
            Self::NotBound => "usb.not_bound",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    /// `vid:pid`, lowercase.
    pub id: String,
    pub name: String,
    /// `wifi`, `bt` or `wifi+bt`.
    pub kind: String,
    /// `2.4`, `2.4+5`, `bt` or `-`.
    pub bands: String,
    /// Drivers in the image that claim this ID ("" when none).
    pub modules: String,
    /// The driver that took it ("" when none).
    pub bound: String,
    /// Network or Bluetooth interface it created ("" when none).
    pub iface: String,
    /// Firmware the driver lacks or failed to load ("" when none).
    pub missing: String,
    pub verdict: Verdict,
}
impl Adapter {
    fn parse(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('\t').collect();
        let [tag, id, name, kind, bands, modules, bound, iface, missing, verdict] = fields[..]
        else {
            return None;
        };
        let plain = |s: &str| if s == "-" { String::new() } else { s.into() };
        (tag == "dev" && valid_id(id)).then_some(())?;
        Some(Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            bands: bands.into(),
            modules: plain(modules),
            bound: plain(bound),
            iface: plain(iface),
            missing: plain(missing),
            verdict: Verdict::parse(verdict)?,
        })
    }
    pub fn bluetooth(&self) -> bool {
        self.kind == "bt"
    }
    /// "Wi‑Fi 2,4 GHz · 2604:0013": the band only when it is the lone one.
    pub fn line(&self, catalog: &I18n) -> String {
        let kind = catalog.text(match self.kind.as_str() {
            "bt" => "usb.kind_bt",
            "wifi+bt" => "usb.kind_both",
            _ => "usb.kind_wifi",
        });
        let kind = match self.bands.as_str() {
            "2.4" if self.kind == "wifi" => format!("{kind} {}", catalog.text("usb.band_24")),
            _ => kind,
        };
        format!("{kind} · {}", self.id)
    }
    /// The driver as the details panel says it.
    pub fn driver(&self, catalog: &I18n) -> String {
        if !self.bound.is_empty() {
            return self.bound.clone();
        }
        match self.modules.split(',').next().filter(|m| !m.is_empty()) {
            Some(module) => catalog.format("usb.driver_unbound", &[("name", module)]),
            // The chip's own name, as its driver would be called.
            None => catalog.format(
                "usb.driver_absent",
                &[(
                    "name",
                    &self
                        .name
                        .rsplit(' ')
                        .next()
                        .unwrap_or(&self.id)
                        .to_lowercase(),
                )],
            ),
        }
    }
    pub fn firmware(&self, catalog: &I18n) -> String {
        match self.verdict {
            Verdict::NoDriver => catalog.text("usb.not_applicable"),
            _ if self.missing.is_empty() => catalog.text("usb.firmware_ok"),
            _ => catalog.format(
                "usb.firmware_lacks",
                &[("file", self.missing.split(',').next().unwrap_or_default())],
            ),
        }
    }
    pub fn interface(&self, catalog: &I18n) -> String {
        if self.iface.is_empty() {
            catalog.text("usb.none")
        } else {
            self.iface.replace(',', ", ")
        }
    }
    /// The advice: what happened and, unless it works, what to buy instead.
    pub fn advice(&self, chips: &[Chip], catalog: &I18n) -> String {
        let chips = suggest(chips, self.bluetooth(), catalog);
        let driver = [self.bound.as_str(), &self.modules]
            .into_iter()
            .find_map(|m| m.split(',').next().filter(|m| !m.is_empty()))
            .unwrap_or_default();
        let file = self.missing.split(',').next().unwrap_or_default();
        let key = match self.verdict {
            Verdict::Works if self.bluetooth() => "usb.advice_works_bt",
            Verdict::Works => "usb.advice_works",
            Verdict::NoDriver => "usb.advice_no_driver",
            Verdict::Firmware => "usb.advice_firmware",
            Verdict::NotBound => "usb.advice_not_bound",
        };
        let text = catalog.format(
            key,
            &[
                ("driver", driver),
                ("file", file),
                ("iface", &self.interface(catalog)),
            ],
        );
        // What to buy instead, when the image's table names any chip.
        if self.verdict == Verdict::Works || chips.is_empty() {
            return text;
        }
        let suggestion = catalog.format("usb.suggest", &[("chips", &chips)]);
        format!("{text} {suggestion}")
    }
    /// The announcement's title.
    pub fn title(&self) -> &'static str {
        match (self.verdict, self.bluetooth()) {
            (Verdict::Works, false) => "usb.title_works",
            (Verdict::Works, true) => "usb.title_works_bt",
            (Verdict::NoDriver, false) => "usb.title_no_driver",
            (Verdict::NoDriver, true) => "usb.title_no_driver_bt",
            (Verdict::Firmware, false) => "usb.title_firmware",
            (Verdict::Firmware, true) => "usb.title_firmware_bt",
            (Verdict::NotBound, false) => "usb.title_not_bound",
            (Verdict::NotBound, true) => "usb.title_not_bound_bt",
        }
    }
}
fn valid_id(id: &str) -> bool {
    id.len() == 9
        && id.bytes().enumerate().all(|(n, b)| {
            if n == 4 {
                b == b':'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
/// "RTL8821CU, RTL8188FTV ou MT7601U": the first three that work, of the kind.
fn suggest(chips: &[Chip], bluetooth: bool, catalog: &I18n) -> String {
    let names: Vec<&str> = chips
        .iter()
        .filter(|c| (c.kind == "bt") == bluetooth)
        .map(|c| c.name.split(" / ").next().unwrap_or(&c.name))
        .take(3)
        .collect();
    match names.split_last() {
        Some((last, [])) => (*last).into(),
        Some((last, rest)) => format!("{} {} {last}", rest.join(", "), catalog.text("usb.or")),
        None => String::new(),
    }
}

/// A chip that works in this image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub name: String,
    pub kind: String,
    pub bands: String,
}
impl Chip {
    pub fn bands(&self, catalog: &I18n) -> String {
        catalog.text(match self.bands.as_str() {
            "2.4" => "usb.band_24",
            "2.4+5" => "usb.band_both",
            _ => "usb.kind_bt",
        })
    }
}

/// The build table's date and chips; its per-ID lines stay on disk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    /// `yyyy-mm-dd`.
    pub date: String,
    pub chips: Vec<Chip>,
}
impl Table {
    pub fn parse(text: &str) -> Self {
        let mut table = Self::default();
        for line in text.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields[..] {
                ["date", date] => table.date = date.into(),
                ["chip", name, kind, bands] => table.chips.push(Chip {
                    name: name.into(),
                    kind: kind.into(),
                    bands: bands.into(),
                }),
                _ => {}
            }
        }
        table
    }
    /// "30/10/2026" (en: 2026-10-30, ru: 30.10.2026).
    pub fn day(&self, language: &str) -> String {
        let mut parts = self.date.split('-');
        let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
            return self.date.clone();
        };
        match language {
            "en" | "en_US" => self.date.clone(),
            "ru" | "ru_RU" => format!("{d}.{m}.{y}"),
            _ => format!("{d}/{m}/{y}"),
        }
    }
}

/// /run/archr/usb-advice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Advice {
    pub seq: u64,
    /// The adapter just plugged that the launcher should announce.
    pub added: Option<String>,
    pub now: Vec<Adapter>,
}
impl Advice {
    pub fn parse(text: &str) -> Self {
        let mut advice = Self::default();
        for line in text.lines() {
            match line.split_once('\t') {
                Some(("seq", n)) => advice.seq = n.trim().parse().unwrap_or(0),
                Some(("added", id)) if valid_id(id) => advice.added = Some(id.into()),
                _ => advice.now.extend(Adapter::parse(line)),
            }
        }
        advice
    }
    /// The adapter to announce, unless `acked` already covers it.
    pub fn announce(&self, acked: u64) -> Option<&Adapter> {
        let id = self.added.as_ref().filter(|_| self.seq > acked)?;
        self.now.iter().find(|a| &a.id == id)
    }
}
/// The remembered adapters, newest first.
pub fn parse_seen(text: &str) -> Vec<Adapter> {
    text.lines().filter_map(Adapter::parse).collect()
}

/// Everything the Adapters page shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Adapters {
    pub now: Vec<Adapter>,
    /// Seen before and not plugged now.
    pub seen: Vec<Adapter>,
    pub table: Table,
}
impl Adapters {
    pub fn new(advice: &str, seen: &str, table: &str) -> Self {
        let now = Advice::parse(advice).now;
        let seen = parse_seen(seen)
            .into_iter()
            .filter(|a| !now.iter().any(|n| n.id == a.id))
            .collect();
        Self {
            now,
            seen,
            table: Table::parse(table),
        }
    }
    pub fn get(&self, n: usize) -> Option<&Adapter> {
        self.now
            .get(n)
            .or_else(|| self.seen.get(n - self.now.len()))
    }
    pub fn len(&self) -> usize {
        self.now.len() + self.seen.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The announcement over the screen (Dongle-aviso).
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub adapter: Adapter,
    pub chips: Vec<Chip>,
    pub since: std::time::Instant,
}
impl Card {
    /// A working adapter's card leaves on its own; a problem waits for a press.
    pub fn expired(&self, now: std::time::Instant) -> bool {
        self.adapter.verdict == Verdict::Works
            && now.saturating_duration_since(self.since) >= std::time::Duration::from_secs(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ADVICE: &str = "seq\t7\nadded\t0bda:b733\n\
        dev\t0bda:b733\tRealtek RTL8733BU\twifi+bt\t2.4+5\t-\t-\t-\t-\tno_driver\n\
        dev\t0bda:0179\tRealtek RTL8188ETV\twifi\t2.4\trtl8xxxu,8188eu\t8188eu\twlan0\t-\tworks\n\
        dev\tbroken line\n";
    const TABLE: &str = "date\t2026-10-30\ndev\t0bda:0179\t8188eu\tok\tx\twifi\t2.4\n\
        chip\tRTL8821CU\twifi\t2.4+5\nchip\tRTL8188FTV / 8188EUS\twifi\t2.4\n\
        chip\tMT7601U\twifi\t2.4\nchip\tRTL8761B\tbt\tbt\nchip\tRTL8812BU\twifi\t2.4+5\n";
    fn pt() -> I18n {
        I18n::load(std::path::Path::new("/none"), Some("pt_BR"), None)
    }
    #[test]
    fn advice_lists_the_adapters_and_announces_once() {
        let advice = Advice::parse(ADVICE);
        assert_eq!(advice.seq, 7);
        assert_eq!(advice.now.len(), 2);
        assert_eq!(advice.now[1].bound, "8188eu");
        assert!(advice.now[0].modules.is_empty());
        assert_eq!(advice.announce(6).unwrap().name, "Realtek RTL8733BU");
        assert_eq!(advice.announce(7), None, "acknowledged");
        assert_eq!(Advice::parse("seq\t3\n").announce(0), None);
    }
    #[test]
    fn the_page_shows_seen_adapters_once() {
        let seen = "dev\t0bda:0179\tRealtek RTL8188ETV\twifi\t2.4\t8188eu\t8188eu\twlan0\t-\tworks\n\
            dev\t1286:2043\tMarvell 88W8797\twifi\t2.4+5\tmwifiex_usb\tmwifiex_usb\t-\tmrvl/usb8797_uapsta.bin\tfirmware\n";
        let page = Adapters::new(ADVICE, seen, TABLE);
        assert_eq!(page.len(), 3);
        assert_eq!(page.seen.len(), 1, "plugged now is not repeated");
        assert_eq!(page.get(2).unwrap().id, "1286:2043");
        assert_eq!(page.table.chips.len(), 5);
        assert_eq!(page.table.day("pt_BR"), "30/10/2026");
        assert_eq!(page.table.day("ru_RU"), "30.10.2026");
    }
    #[test]
    fn texts_follow_the_mockups() {
        let catalog = pt();
        let page = Adapters::new(ADVICE, "", TABLE);
        let missing = &page.now[0];
        assert_eq!(missing.line(&catalog), "Wi‑Fi e Bluetooth · 0bda:b733");
        assert_eq!(missing.driver(&catalog), "rtl8733bu, não incluído");
        assert_eq!(missing.firmware(&catalog), "não se aplica");
        assert_eq!(missing.interface(&catalog), "nenhuma");
        assert_eq!(
            missing.advice(&page.table.chips, &catalog),
            "Esta imagem não traz driver para este chip. Adaptadores com RTL8821CU, \
             RTL8188FTV ou MT7601U funcionam sem configurar nada."
        );
        // Without the image's table: the verdict alone, never a hole.
        assert_eq!(
            missing.advice(&[], &catalog),
            "Esta imagem não traz driver para este chip."
        );
        let works = &page.now[1];
        assert_eq!(works.line(&catalog), "Wi‑Fi 2,4 GHz · 0bda:0179");
        assert_eq!(works.interface(&catalog), "wlan0");
        let marvell = Adapter::parse(
            "dev\t1286:2043\tMarvell 88W8797\twifi\t2.4+5\tmwifiex_usb\tmwifiex_usb\t-\tmrvl/a.bin,mrvl/b.bin\tfirmware",
        )
        .unwrap();
        assert_eq!(marvell.line(&catalog), "Wi‑Fi · 1286:2043");
        assert_eq!(marvell.firmware(&catalog), "falta mrvl/a.bin");
        assert_eq!(page.table.chips[3].bands(&catalog), "Bluetooth");
    }
}
