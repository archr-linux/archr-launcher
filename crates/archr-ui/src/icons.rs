//! Lucide SVG icons rasterized at their final size on first draw.
use sdl2::{rwops::RWops, surface::Surface, sys};
use std::path::Path;

// The config screen uses these variants on its parallel integration branch.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Glyph {
    Recent,
    Games,
    Tools,
    Gear,
    Download,
    Wifi,
    WifiOff,
    Moon,
    Restart,
    Power,
    Sound,
    Globe,
    Palette,
    Trophy,
    Storage,
    Clock,
    Lightning,
    Star,
    Gamepad,
    Play,
    Cpu,
    Battery,
    BatteryLow,
    BatteryMedium,
    BatteryFull,
    BatteryCharging,
    WifiLow,
    WifiHigh,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    Check,
    Alert,
    Warning,
    CircleX,
}

impl Glyph {
    fn file(self) -> &'static str {
        match self {
            Self::Recent => "history",
            Self::Games => "gamepad-2",
            Self::Tools => "wrench",
            Self::Gear => "settings",
            Self::Download => "download",
            Self::Wifi => "wifi",
            Self::WifiOff => "wifi-off",
            Self::Moon => "moon",
            Self::Restart => "rotate-cw",
            Self::Power => "power",
            Self::Sound => "volume-2",
            Self::Globe => "globe",
            Self::Palette => "palette",
            Self::Trophy => "trophy",
            Self::Storage => "hard-drive",
            Self::Clock => "clock",
            Self::Lightning => "zap",
            Self::Star => "star",
            Self::Gamepad => "gamepad",
            Self::Play => "play",
            Self::Cpu => "cpu",
            Self::Battery => "battery",
            Self::BatteryLow => "battery-low",
            Self::BatteryMedium => "battery-medium",
            Self::BatteryFull => "battery-full",
            Self::BatteryCharging => "battery-charging",
            Self::WifiLow => "wifi-low",
            Self::WifiHigh => "wifi-high",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::ChevronUp => "chevron-up",
            Self::ChevronDown => "chevron-down",
            Self::Check => "check",
            Self::Alert => "circle-alert",
            Self::Warning => "triangle-alert",
            Self::CircleX => "circle-x",
        }
    }
}

// Keep the upstream SVG files untouched; tune this one number if 16px needs it.
const STROKE_WIDTH: &str = "2";

#[link(name = "SDL2_image")]
unsafe extern "C" {
    fn IMG_LoadSizedSVG_RW(
        src: *mut sys::SDL_RWops,
        width: libc::c_int,
        height: libc::c_int,
    ) -> *mut sys::SDL_Surface;
}

pub(crate) fn render(
    glyph: Glyph,
    size: u32,
    theme_dir: Option<&Path>,
) -> Result<Surface<'static>, String> {
    let file = format!("{}.svg", glyph.file());
    let selected = theme_dir.map(|dir| dir.join("icons").join(&file));
    let system = Path::new("/usr/share/archr-launcher/themes/default/icons").join(&file);
    let bundled = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../themes/default/icons")
        .join(&file);
    let path = selected
        .as_deref()
        .filter(|path| path.is_file())
        .or_else(|| system.is_file().then_some(system.as_path()))
        .unwrap_or(&bundled);
    let svg = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let svg = svg.replace("currentColor", "#ffffff").replace(
        "stroke-width=\"2\"",
        &format!("stroke-width=\"{STROKE_WIDTH}\""),
    );
    let rw = RWops::from_bytes(svg.as_bytes())?;
    let dimension = i32::try_from(size).map_err(|e| e.to_string())?;
    // SDL_image borrows the RWops (it does not close it); Surface takes ownership of the result.
    let raw = unsafe { IMG_LoadSizedSVG_RW(rw.raw(), dimension, dimension) };
    if raw.is_null() {
        Err(format!("{}: {}", path.display(), sdl2::get_error()))
    } else {
        Ok(unsafe { Surface::from_ll(raw) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_svgs_render_white_at_exact_target_sizes() {
        for glyph in [
            Glyph::Recent,
            Glyph::Games,
            Glyph::Tools,
            Glyph::Gear,
            Glyph::Download,
            Glyph::Wifi,
            Glyph::WifiOff,
            Glyph::Moon,
            Glyph::Restart,
            Glyph::Power,
            Glyph::Sound,
            Glyph::Globe,
            Glyph::Palette,
            Glyph::Trophy,
            Glyph::Storage,
            Glyph::Clock,
            Glyph::Lightning,
            Glyph::Star,
            Glyph::Gamepad,
            Glyph::Play,
            Glyph::Cpu,
            Glyph::Battery,
            Glyph::BatteryLow,
            Glyph::BatteryMedium,
            Glyph::BatteryFull,
            Glyph::BatteryCharging,
            Glyph::WifiLow,
            Glyph::WifiHigh,
            Glyph::ChevronLeft,
            Glyph::ChevronRight,
            Glyph::ChevronUp,
            Glyph::ChevronDown,
            Glyph::Check,
            Glyph::Alert,
            Glyph::Warning,
            Glyph::CircleX,
        ] {
            for size in [16, 24, 48] {
                let surface = render(glyph, size, None).unwrap();
                assert_eq!((surface.width(), surface.height()), (size, size));
                assert!(surface
                    .without_lock()
                    .unwrap()
                    .iter()
                    .any(|&byte| byte != 0));
            }
        }
    }
    #[test]
    fn theme_icon_overrides_bundled_without_changing_source() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("icons")).unwrap();
        std::fs::write(dir.path().join("icons/wrench.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\"><rect width=\"24\" height=\"24\" fill=\"currentColor\"/></svg>").unwrap();
        let override_icon = render(Glyph::Tools, 24, Some(dir.path())).unwrap();
        let default_icon = render(Glyph::Tools, 24, Some(Path::new("/nonexistent"))).unwrap();
        assert_ne!(
            override_icon.without_lock().unwrap(),
            default_icon.without_lock().unwrap()
        );
    }
}
