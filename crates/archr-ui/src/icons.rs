//! Unmodified Lucide SVGs rasterized at their target size, with resilient theme lookup.
use sdl2::{rwops::RWops, surface::Surface, sys};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub const DEFAULT_ICONS_DIR: &str = "/usr/share/archr-launcher/themes/default/icons";

macro_rules! glyphs {
    ($($variant:ident => $file:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        pub(crate) enum Glyph { $($variant),+ }
        impl Glyph {
            fn file(self) -> &'static str {
                match self { $(Self::$variant => $file),+ }
            }
            fn embedded(self) -> &'static str {
                match self {
                    $(Self::$variant => include_str!(concat!("../../../themes/default/icons/", $file, ".svg"))),+
                }
            }
            #[cfg(test)]
            const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}
glyphs! {
    Recent => "history", Games => "gamepad-2", Tools => "wrench",
    Gear => "settings", Download => "download", Wifi => "wifi",
    WifiOff => "wifi-off", Moon => "moon", Restart => "rotate-cw",
    Power => "power", Clock => "clock", Star => "star",
    Battery => "battery", BatteryLow => "battery-low",
    BatteryMedium => "battery-medium", BatteryFull => "battery-full",
    BatteryCharging => "battery-charging",
    Controls => "gamepad", Play => "play", Sound => "volume-2",
    Globe => "globe", Palette => "palette", Trophy => "trophy", Storage => "hard-drive",
    ChevronLeft => "chevron-left", ChevronRight => "chevron-right",
}

#[link(name = "SDL2_image")]
unsafe extern "C" {
    fn IMG_LoadSizedSVG_RW(
        src: *mut sys::SDL_RWops,
        width: libc::c_int,
        height: libc::c_int,
    ) -> *mut sys::SDL_Surface;
}

/// One loader per UI cache; bad files and failed sizes are not retried every frame.
pub(crate) struct IconLoader {
    theme_dir: Option<PathBuf>,
    system_icons_dir: PathBuf,
    failed_files: HashSet<PathBuf>,
    failed_sizes: HashSet<(Glyph, u32)>,
    warned: HashSet<Glyph>,
}

impl IconLoader {
    pub(crate) fn new(theme_dir: Option<&Path>, system_icons_dir: &Path) -> Self {
        Self {
            theme_dir: theme_dir.map(Path::to_path_buf),
            system_icons_dir: system_icons_dir.to_path_buf(),
            failed_files: HashSet::new(),
            failed_sizes: HashSet::new(),
            warned: HashSet::new(),
        }
    }

    fn warn_once(&mut self, glyph: Glyph, message: &str) {
        if self.warned.insert(glyph) {
            log::warn!("Lucide icon {glyph:?}: {message}");
        }
    }

    pub(crate) fn render(&mut self, glyph: Glyph, size: u32) -> Option<Surface<'static>> {
        if self.failed_sizes.contains(&(glyph, size)) {
            return None;
        }
        let file = format!("{}.svg", glyph.file());
        let paths = [
            self.theme_dir
                .as_ref()
                .map(|dir| dir.join("icons").join(&file)),
            Some(self.system_icons_dir.join(&file)),
        ];
        for path in paths.into_iter().flatten() {
            if self.failed_files.contains(&path) || !path.is_file() {
                continue;
            }
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|svg| rasterize(&svg, size))
            {
                Ok(surface) => return Some(surface),
                Err(error) => {
                    self.failed_files.insert(path.clone());
                    self.warn_once(
                        glyph,
                        &format!("{}: {error}; trying fallback", path.display()),
                    );
                }
            }
        }
        if let Ok(surface) = rasterize(glyph.embedded(), size) {
            if !self.system_icons_dir.join(&file).is_file() {
                self.warn_once(glyph, "installed SVG unavailable; using embedded copy");
            }
            return Some(surface);
        }
        self.failed_sizes.insert((glyph, size));
        self.warn_once(glyph, "all SVG sources failed; omitting icon");
        None
    }
}

// Only Lucide's exact two-unit-stroke dot forms; this is not a general SVG parser.
fn normalize_dots(svg: &str) -> String {
    let mut result = svg.to_owned();
    let numeric = |s: &str| s.parse::<f32>().is_ok();
    for (tag, _) in svg.split('<').filter_map(|s| s.split_once("/>")) {
        let fields: Vec<_> = tag.trim_end().split('"').collect();
        let point = match fields.as_slice() {
            ["path d=", d, ""] => d
                .strip_prefix('M')
                .and_then(|d| d.strip_suffix("h.01").or_else(|| d.strip_suffix("v.01")))
                .and_then(|d| d.split_once(' ')),
            ["line x1=", x, " x2=", end, " y1=", y, " y2=", ey, ""] => {
                (*end == format!("{x}.01") && y == ey).then_some((*x, *y))
            }
            _ => None,
        };
        if let Some((x, y)) = point.filter(|(x, y)| numeric(x) && numeric(y)) {
            let circle =
                format!(r#"<circle cx="{x}" cy="{y}" r="1" fill="currentColor" stroke="none" />"#);
            result = result.replace(&format!("<{tag}/>"), &circle);
        }
    }
    result
}

fn rasterize(svg: &str, size: u32) -> Result<Surface<'static>, String> {
    let svg = normalize_dots(svg).replace("currentColor", "#ffffff");
    let rw = RWops::from_bytes(svg.as_bytes())?;
    let dimension = i32::try_from(size).map_err(|e| e.to_string())?;
    // SDL_image borrows the RWops; Surface takes ownership of the result.
    let raw = unsafe { IMG_LoadSizedSVG_RW(rw.raw(), dimension, dimension) };
    if raw.is_null() {
        Err(sdl2::get_error())
    } else {
        Ok(unsafe { Surface::from_ll(raw) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dot_transform_handles_exact_forms_and_leaves_complex_paths_alone() {
        for path in ["M12 20h.01", "M15 9h.01", "M12 20v.01"] {
            let svg = format!(r#"<path d="{path}" />"#);
            assert!(normalize_dots(&svg).contains("<circle"));
        }
        for path in ["M12 20h.01m-8-16h8", "M2 2h8M12 20h.01", "M12 20h.012"] {
            let svg = format!(r#"<path d="{path}" />"#);
            assert_eq!(normalize_dots(&svg), svg);
        }
    }

    #[test]
    fn bundled_dot_audit_changes_only_the_five_affected_icons() {
        for &glyph in Glyph::ALL {
            let svg = glyph.embedded();
            let normalized = normalize_dots(svg);
            let added = match glyph {
                Glyph::Wifi | Glyph::WifiOff => 1,
                Glyph::Controls | Glyph::Games | Glyph::Storage => 2,
                _ => 0,
            };
            assert_eq!(
                normalized.matches("<circle").count(),
                svg.matches("<circle").count() + added,
                "{glyph:?}"
            );
            if added == 0 {
                assert_eq!(normalized, svg, "unrelated geometry in {glyph:?}");
            }
        }
    }

    #[test]
    fn wifi_dot_survives_small_raster_sizes() {
        for size in [16, 24] {
            let surface = rasterize(Glyph::Wifi.embedded(), size)
                .unwrap()
                .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
                .unwrap();
            let (x, y) = (size / 2, size * 20 / 24);
            surface.with_lock(|pixels| {
                assert!(
                    (y - 1..=y).any(|y| (x - 1..=x)
                        .any(|x| pixels[(y * surface.pitch() + x * 4 + 3) as usize] > 32)),
                    "Wi-Fi dot disappeared at {size}px"
                );
            });
        }
    }

    #[test]
    fn embedded_svgs_render_at_target_sizes() {
        for &glyph in Glyph::ALL {
            for size in [16, 24, 48] {
                let surface = rasterize(glyph.embedded(), size).unwrap();
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
    fn theme_overrides_system_and_bad_file_falls_back_without_retry() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("theme");
        let system = dir.path().join("system");
        std::fs::create_dir_all(theme.join("icons")).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        let rect = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\"><rect width=\"24\" height=\"24\" fill=\"currentColor\"/></svg>";
        std::fs::write(system.join("wrench.svg"), rect).unwrap();
        let mut default = IconLoader::new(None, &system);
        let system_pixels = default.render(Glyph::Tools, 24).unwrap();
        std::fs::write(
            theme.join("icons/wrench.svg"),
            rect.replace("rect", "circle"),
        )
        .unwrap();
        let mut loader = IconLoader::new(Some(&theme), &system);
        let theme_pixels = loader.render(Glyph::Tools, 24).unwrap();
        assert_ne!(
            theme_pixels.without_lock().unwrap(),
            system_pixels.without_lock().unwrap()
        );
        std::fs::write(theme.join("icons/wrench.svg"), "not an SVG").unwrap();
        let mut loader = IconLoader::new(Some(&theme), &system);
        let fallback = loader.render(Glyph::Tools, 24).unwrap();
        assert_eq!(
            fallback.without_lock().unwrap(),
            system_pixels.without_lock().unwrap()
        );
        assert_eq!(loader.failed_files.len(), 1);
        std::fs::write(
            theme.join("icons/wrench.svg"),
            rect.replace("rect", "circle"),
        )
        .unwrap();
        let cached_fallback = loader.render(Glyph::Tools, 16).unwrap();
        let system_small = default.render(Glyph::Tools, 16).unwrap();
        assert_eq!(
            cached_fallback.without_lock().unwrap(),
            system_small.without_lock().unwrap()
        );
        assert_eq!(loader.warned.len(), 1);
        std::fs::remove_file(system.join("wrench.svg")).unwrap();
        assert!(
            loader.render(Glyph::Tools, 48).is_some(),
            "embedded copy survives missing install"
        );
    }
}
