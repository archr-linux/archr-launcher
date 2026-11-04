//! Datagram protocol (`volume 60 [muted]`, `brightness 40`) and the icon it
//! selects; and the battery card (`battery-low 10`, `battery 3 LEFT TOTAL
//! game|menu`, a second of archr-battery-guard's countdown).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    VolumeX,
    Volume1,
    Volume2,
    Sun,
}

impl Icon {
    pub const ALL: [Self; 4] = [Self::VolumeX, Self::Volume1, Self::Volume2, Self::Sun];

    /// Unmodified Lucide source, shared with the launcher theme.
    pub fn svg(self) -> &'static str {
        match self {
            Self::VolumeX => include_str!("../../../themes/default/icons/volume-x.svg"),
            Self::Volume1 => include_str!("../../../themes/default/icons/volume-1.svg"),
            Self::Volume2 => include_str!("../../../themes/default/icons/volume-2.svg"),
            Self::Sun => include_str!("../../../themes/default/icons/sun.svg"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub icon: Icon,
    /// Percent, 0..=100.
    pub level: u8,
}

/// The battery card: the level, the countdown (seconds left of the total)
/// for the critical one, and whether a game is being saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    pub level: u8,
    pub left: Option<(u8, u8)>,
    pub game: bool,
}

/// The cards over the screen: the battery's, and "this emulator saves no
/// state" when the switcher's shortcut cannot save the game (`nostate`);
/// the diagnostic report's, as archr-report --osd starts and ends
/// (`report start|done|failed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alert {
    Battery(Battery),
    NoState,
    Report(Report),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    Start,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Level(Message),
    Alert(Alert),
}

/// A level pill or a battery card; anything else is `None`.
pub fn parse_notice(text: &str) -> Option<Notice> {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    let number = |w: &str| w.parse::<u8>().ok().filter(|n| *n <= 100);
    match words[..] {
        ["nostate"] => Some(Notice::Alert(Alert::NoState)),
        ["report", step] => Some(Notice::Alert(Alert::Report(match step {
            "start" => Report::Start,
            "done" => Report::Done,
            "failed" => Report::Failed,
            _ => return None,
        }))),
        ["battery-low", level] => Some(Notice::Alert(Alert::Battery(Battery {
            level: number(level)?,
            left: None,
            game: false,
        }))),
        ["battery", level, left, total, game @ ("game" | "menu")] => {
            let (left, total) = (number(left)?, number(total)?);
            (left >= 1 && left <= total).then_some(())?;
            Some(Notice::Alert(Alert::Battery(Battery {
                level: number(level)?,
                left: Some((left, total)),
                game: game == "game",
            })))
        }
        _ => parse(text).map(Notice::Level),
    }
}

/// Parses one message; anything else (unknown kind, missing or non-numeric
/// level, extra words) is `None`. Levels above 100 are clamped.
pub fn parse(text: &str) -> Option<Message> {
    let mut words = text.split_ascii_whitespace();
    let kind = words.next()?;
    let level = words.next()?.parse::<u32>().ok()?.min(100) as u8;
    let flag = words.next();
    if words.next().is_some() {
        return None;
    }
    let icon = match (kind, flag) {
        ("volume", None) => volume_icon(level, false),
        ("volume", Some("muted")) => volume_icon(level, true),
        ("brightness", None) => Icon::Sun,
        _ => return None,
    };
    Some(Message { icon, level })
}

fn volume_icon(level: u8, muted: bool) -> Icon {
    match level {
        _ if muted => Icon::VolumeX,
        0 => Icon::VolumeX,
        1..=49 => Icon::Volume1,
        _ => Icon::Volume2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(icon: Icon, level: u8) -> Option<Message> {
        Some(Message { icon, level })
    }

    #[test]
    fn parses_volume_and_brightness() {
        assert_eq!(parse("volume 60"), msg(Icon::Volume2, 60));
        assert_eq!(parse("  volume\t49\n"), msg(Icon::Volume1, 49));
        assert_eq!(parse("volume 60 muted"), msg(Icon::VolumeX, 60));
        assert_eq!(parse("brightness 40"), msg(Icon::Sun, 40));
        assert_eq!(parse("brightness 0"), msg(Icon::Sun, 0));
        assert_eq!(parse("volume 250"), msg(Icon::Volume2, 100));
    }

    #[test]
    fn icon_follows_level_and_mute() {
        assert_eq!(volume_icon(0, false), Icon::VolumeX);
        assert_eq!(volume_icon(1, false), Icon::Volume1);
        assert_eq!(volume_icon(49, false), Icon::Volume1);
        assert_eq!(volume_icon(50, false), Icon::Volume2);
        assert_eq!(volume_icon(100, false), Icon::Volume2);
        assert_eq!(volume_icon(100, true), Icon::VolumeX);
    }

    #[test]
    fn rejects_malformed_input() {
        for text in [
            "",
            "volume",
            "volume -5",
            "volume 6.5",
            "volume sixty",
            "volume 60 loud",
            "volume 60 muted now",
            "brightness 40 muted",
            "Volume 60",
            "wifi 3",
            "\u{0}\u{ff}",
        ] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn parses_battery_cards() {
        let battery =
            |level, left, game| Some(Notice::Alert(Alert::Battery(Battery { level, left, game })));
        assert_eq!(parse_notice("nostate"), Some(Notice::Alert(Alert::NoState)));
        assert_eq!(
            parse_notice("report done"),
            Some(Notice::Alert(Alert::Report(Report::Done)))
        );
        assert_eq!(parse_notice("battery-low 10"), battery(10, None, false));
        assert_eq!(
            parse_notice("battery 3 8 8 game"),
            battery(3, Some((8, 8)), true)
        );
        assert_eq!(
            parse_notice("battery 3 1 8 menu"),
            battery(3, Some((1, 8)), false)
        );
        assert_eq!(
            parse_notice("volume 60"),
            Some(Notice::Level(Message {
                icon: Icon::Volume2,
                level: 60
            }))
        );
        for text in [
            "battery 3 9 8 game",
            "battery 3 0 8 game",
            "battery 3 8 8",
            "battery-low",
            "battery-low 300",
            "battery 3 8 8 other",
            "report",
            "report later",
            "report done now",
        ] {
            assert_eq!(parse_notice(text), None, "{text}");
        }
    }

    #[test]
    fn icons_are_unmodified_lucide() {
        for icon in Icon::ALL {
            assert!(icon
                .svg()
                .starts_with("<!-- @license lucide-static v0.468.0 - ISC -->"));
            assert!(icon.svg().contains("stroke=\"currentColor\""));
        }
    }
}
