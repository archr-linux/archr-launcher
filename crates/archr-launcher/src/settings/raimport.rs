//! Import from RetroArch: the system paths and the backup around crate::raimport.
use super::Backend;
use crate::raimport::{apply, scan, Roots};
use archr_ui::settings::raimport::Found;
use std::time::Duration;

/// backuptool zips all settings; give it the time the Storage page gives it.
const BACKUP: Duration = Duration::from_secs(15 * 60);

impl Backend {
    fn ra_roots(&self) -> Roots {
        Roots {
            source: self.paths.at("/storage/roms/retroarch-import"),
            retroarch: self.paths.at("/storage/.config/retroarch"),
            remaps: self.paths.at("/storage/remappings"),
            shaders: self.paths.at("/tmp/shaders"),
            cores: self.paths.at("/tmp/cores"),
        }
    }
    pub(super) fn ra_scan(&self) -> Found {
        scan(&self.ra_roots()).found
    }
    /// The backup first when asked (its failure stops everything), then a
    /// fresh scan applied as a whole.
    pub(super) fn ra_import(&self, backup: bool) -> Result<String, String> {
        if backup {
            self.command_with_timeout("backuptool", &["backup"], BACKUP)
                .map_err(|error| {
                    log::warn!("Import: backup failed: {error}");
                    "raimport.backup_failed".to_owned()
                })?;
        }
        let roots = self.ra_roots();
        let plan = scan(&roots);
        if plan.found.apply.is_empty() {
            return Err("raimport.nothing".into());
        }
        let count = apply(&plan, &roots)?;
        Ok(format!("raimport.done|count={count}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(backuptool: &str) -> (tempfile::TempDir, Paths) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        let write = |path: &str, text: &str| {
            let path = paths.at(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "/storage/.config/retroarch/retroarch.cfg",
            "video_smooth = \"false\"\n",
        );
        write(
            "/storage/roms/retroarch-import/retroarch.cfg",
            "video_smooth = \"true\"\n",
        );
        write(
            "/usr/bin/backuptool",
            &format!(
                "#!/bin/sh\necho \"$*\" >> {}\n{backuptool}\n",
                root.path().join("calls").display()
            ),
        );
        fs::set_permissions(
            paths.at("/usr/bin/backuptool"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        (root, paths)
    }
    fn saved(paths: &Paths) -> String {
        fs::read_to_string(paths.at("/storage/.config/retroarch/retroarch.cfg")).unwrap()
    }
    #[test]
    fn backs_up_first_and_imports_into_retroarch_cfg() {
        let (root, paths) = fixture("");
        let backend = Backend::new(paths.clone());
        assert_eq!(backend.ra_scan().settings, 1);
        assert_eq!(backend.ra_import(true), Ok("raimport.done|count=1".into()));
        assert_eq!(
            fs::read_to_string(root.path().join("calls")).unwrap(),
            "backup\n"
        );
        assert_eq!(saved(&paths), "video_smooth = \"true\"\n");
    }
    #[test]
    fn a_failed_backup_imports_nothing() {
        let (_root, paths) = fixture("exit 1");
        let backend = Backend::new(paths.clone());
        assert_eq!(
            backend.ra_import(true),
            Err("raimport.backup_failed".into())
        );
        assert_eq!(saved(&paths), "video_smooth = \"false\"\n");
        assert_eq!(backend.ra_import(false), Ok("raimport.done|count=1".into()));
    }
}
