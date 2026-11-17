//! The active theme as the launcher hands it over
//! (/run/archr-launcher/osd-theme.toml): the level's shape and colours, the
//! cards' palette and the fonts. Missing or broken, the OSD draws as it
//! always has.
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const PATH: &str = "/run/archr-launcher/osd-theme.toml";
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Pill,
    Capsule,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Edge {
    #[default]
    Right,
    Left,
    Top,
    Bottom,
}
impl Edge {
    /// At the top and bottom the level lies down and fills from the left.
    pub fn lying(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// `[osd]`: the volume and brightness level.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    pub shape: Shape,
    pub edge: Edge,
    pub fill: Option<[u8; 4]>,
    pub track: Option<[u8; 4]>,
    /// Corner radius, a size of 480.
    pub radius: Option<f32>,
}

/// The cards' colours; the default is the default theme's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub veil: [u8; 4],
    pub card: [u8; 4],
    pub danger: [u8; 3],
    pub danger_edge: [u8; 3],
    pub warning: [u8; 3],
    pub accent: [u8; 3],
    pub success: [u8; 3],
    pub edge: [u8; 3],
    pub text: [u8; 3],
    pub body: [u8; 3],
    pub muted: [u8; 3],
    pub track: [u8; 3],
}
impl Palette {
    pub const DEFAULT: Self = Self {
        veil: [5, 8, 11, 115],   // rgba(5,8,11,.45)
        card: [17, 22, 28, 240], // rgba(17,22,28,.94)
        danger: [0xF8, 0x51, 0x49],
        danger_edge: [0x5C, 0x23, 0x28],
        warning: [0xE3, 0xA0, 0x08],
        accent: [0x1E, 0x88, 0xFF],
        success: [0x3F, 0xB9, 0x50],
        edge: [0x1C, 0x23, 0x2C],
        text: [0xE6, 0xED, 0xF3],
        body: [0xC5, 0xCF, 0xDA],
        muted: [0x9B, 0xA7, 0xB4],
        track: [0x1C, 0x25, 0x30],
    };
}
impl Default for Palette {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    pub level: Level,
    pub palette: Palette,
    pub font: Option<PathBuf>,
    pub font_semibold: Option<PathBuf>,
}

impl Look {
    /// Every key on its own: a wrong one keeps its default.
    pub fn parse(source: &str) -> Self {
        let mut look = Self::default();
        let table = match toml::from_str::<toml::Table>(source) {
            Ok(table) => table,
            Err(e) => {
                eprintln!("archr-osd: theme: {e}");
                return look;
            }
        };
        let path = |key: &str| {
            table
                .get(key)
                .and_then(toml::Value::as_str)
                .filter(|p| p.starts_with('/'))
                .map(PathBuf::from)
        };
        look.font = path("font");
        look.font_semibold = path("font_semibold");
        if let Some(colors) = table.get("colors").and_then(toml::Value::as_table) {
            let get = |key: &str| colors.get(key).and_then(toml::Value::as_str).and_then(hex);
            let p = &mut look.palette;
            let rgb = |c: [u8; 4]| [c[0], c[1], c[2]];
            if let Some(c) = get("surface") {
                p.card = [c[0], c[1], c[2], (u16::from(c[3]) * 240 / 255) as u8];
            }
            if let Some(c) = get("scrim") {
                // The launcher's notices at 0.6, the OSD's veil at 0.45.
                p.veil = [c[0], c[1], c[2], (f32::from(c[3]) * 0.75).round() as u8];
            }
            for (key, slot) in [
                ("danger", &mut p.danger),
                ("warning", &mut p.warning),
                ("accent", &mut p.accent),
                ("success", &mut p.success),
                ("border", &mut p.edge),
                ("text", &mut p.text),
                ("text_secondary", &mut p.body),
                ("muted", &mut p.muted),
                ("badge", &mut p.track),
            ] {
                if let Some(c) = get(key) {
                    *slot = rgb(c);
                }
            }
            if get("danger").is_some() || get("surface").is_some() {
                p.danger_edge = mix(rgb(p.card), p.danger, 0.3);
            }
        }
        if let Some(osd) = table.get("osd").and_then(toml::Value::as_table) {
            let word = |key: &str| osd.get(key).and_then(toml::Value::as_str);
            let l = &mut look.level;
            l.shape = match word("style") {
                Some("capsule") => Shape::Capsule,
                _ => Shape::Pill,
            };
            l.edge = match word("position") {
                Some("left") => Edge::Left,
                Some("top") => Edge::Top,
                Some("bottom") => Edge::Bottom,
                _ => Edge::Right,
            };
            l.fill = word("fill").and_then(hex);
            l.track = word("track").and_then(hex);
            l.radius = osd
                .get("radius")
                .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64)))
                .map(|r| r as f32)
                .filter(|r| (0.0..=0.1).contains(r));
        }
        look
    }

    pub fn load(path: &Path) -> Self {
        let mut source = String::new();
        match std::fs::File::open(path)
            .and_then(|file| file.take(MAX_BYTES).read_to_string(&mut source))
        {
            Ok(_) => Self::parse(&source),
            Err(e) => {
                eprintln!("archr-osd: {}: {e}", path.display());
                Self::default()
            }
        }
    }
}

fn hex(value: &str) -> Option<[u8; 4]> {
    let digits = value.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let at = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([
        at(0)?,
        at(2)?,
        at(4)?,
        if digits.len() == 8 { at(6)? } else { 255 },
    ])
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// The theme file, read again whenever it changes (or appears, or goes).
pub struct Watch {
    path: PathBuf,
    stamp: Option<Option<(SystemTime, u64)>>,
}
impl Watch {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            stamp: None,
        }
    }
    /// The new look when the file changed since the last call.
    pub fn poll(&mut self) -> Option<Look> {
        let now = std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        if self.stamp == Some(now) {
            return None;
        }
        self.stamp = Some(now);
        Some(now.map_or_else(Look::default, |_| Look::load(&self.path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_or_garbage_is_the_default() {
        assert_eq!(Look::parse(""), Look::default());
        assert_eq!(Look::parse("[osd\nstyle ="), Look::default());
        assert_eq!(
            Look::load(Path::new("/nonexistent/osd-theme.toml")),
            Look::default()
        );
        // What the launcher writes for the default theme.
        let default = Look::parse(
            "font = \"/usr/share/archr-launcher/themes/default/fonts/Inter-Regular.ttf\"\n\
             [osd]\nstyle = \"pill\"\nposition = \"right\"\n",
        );
        assert_eq!(default.level, Level::default());
        assert_eq!(default.palette, Palette::DEFAULT);
        assert!(default.font.is_some() && default.font_semibold.is_none());
    }

    #[test]
    fn the_file_sets_the_level_and_the_palette() {
        let look = Look::parse(
            "font = \"relative.ttf\"\n\
             [colors]\nwarning = \"#FFAA00\"\nscrim = \"#00000080\"\ndanger = \"#FF0000\"\n\
             accent = \"nope\"\n\
             [osd]\nstyle = \"capsule\"\nposition = \"top\"\nfill = \"#FFFFFF\"\n\
             track = \"#10101080\"\nradius = 0.05\n",
        );
        assert_eq!(look.font, None, "only absolute paths");
        let p = look.palette;
        assert_eq!(p.warning, [0xFF, 0xAA, 0]);
        assert_eq!(p.veil, [0, 0, 0, 96]);
        assert_eq!(p.danger, [0xFF, 0, 0]);
        assert_eq!(p.danger_edge, mix([17, 22, 28], [0xFF, 0, 0], 0.3));
        assert_eq!(p.accent, Palette::DEFAULT.accent);
        let l = look.level;
        assert_eq!((l.shape, l.edge), (Shape::Capsule, Edge::Top));
        assert!(l.edge.lying());
        assert_eq!(l.fill, Some([255, 255, 255, 255]));
        assert_eq!(l.track, Some([16, 16, 16, 128]));
        assert_eq!(l.radius, Some(0.05));
        let odd = Look::parse("[osd]\nstyle = \"blob\"\nposition = 3\nradius = 2\n");
        assert_eq!(odd.level, Level::default());
    }

    #[test]
    fn the_watch_reads_again_only_after_a_change() {
        let dir = std::env::temp_dir().join(format!("archr-osd-look-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("osd-theme.toml");
        let _ = std::fs::remove_file(&path);
        let mut watch = Watch::new(&path);
        assert_eq!(watch.poll(), Some(Look::default()));
        assert_eq!(watch.poll(), None);
        std::fs::write(&path, "[osd]\nstyle = \"capsule\"\n").unwrap();
        assert_eq!(watch.poll().unwrap().level.shape, Shape::Capsule);
        assert_eq!(watch.poll(), None);
        std::fs::write(&path, "[osd]\nstyle = \"pill\"\nposition = \"left\"\n").unwrap();
        assert_eq!(watch.poll().unwrap().level.edge, Edge::Left);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(watch.poll(), Some(Look::default()));
        let _ = std::fs::remove_dir(&dir);
    }
}
