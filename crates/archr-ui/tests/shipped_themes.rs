//! Every theme shipped in themes/ loads without one warning: no unknown
//! key, no value out of range, on any screen shape and in any view. A
//! process of its own, so the log holds only what these themes say.
use archr_ui::Theme;
use std::path::Path;
use std::sync::Mutex;

static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Listen;
impl log::Log for Listen {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            SAID.lock().unwrap().push(format!("{}", record.args()));
        }
    }
    fn flush(&self) {}
}

#[test]
fn shipped_themes_load_without_a_warning() {
    log::set_logger(&Listen).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let themes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let default = Theme::load(&themes.join("default"));
    let mut dirs: Vec<_> = std::fs::read_dir(&themes)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|dir| dir.join("theme.toml").is_file())
        .collect();
    dirs.sort();
    assert!(dirs.iter().any(|d| d.ends_with("ellegance")));
    for dir in dirs {
        let theme = Theme::load_over(&dir, &default);
        for (w, h) in [(640, 480), (720, 720), (480, 320)] {
            let fitted = theme.fitted(w, h);
            for (id, _) in fitted.views() {
                let _ = fitted.view(&id);
            }
        }
        let said = std::mem::take(&mut *SAID.lock().unwrap());
        assert!(said.is_empty(), "{}: {said:#?}", dir.display());
    }
}
