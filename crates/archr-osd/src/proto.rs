//! Datagram protocol (`volume 60 [muted]`, `brightness 40`) and the icon it selects.

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
    fn icons_are_unmodified_lucide() {
        for icon in Icon::ALL {
            assert!(icon
                .svg()
                .starts_with("<!-- @license lucide-static v0.468.0 - ISC -->"));
            assert!(icon.svg().contains("stroke=\"currentColor\""));
        }
    }
}
