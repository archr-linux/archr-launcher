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
            // The lower layers of the /tmp/shaders overlay.
            factory: ["common-shaders", "glsl-shaders", "slang-shaders"]
                .iter()
                .map(|dir| self.paths.at(&format!("/usr/share/{dir}")))
                .collect(),
        }
    }
    pub(super) fn ra_scan(&self) -> Found {
        scan(&self.ra_roots()).found
    }
    /// A fresh scan that must match what the user reviewed (`seen`), then
    /// the backup when asked (its failure stops everything), then the import.
    pub(super) fn ra_import(&self, backup: bool, seen: &Found) -> Result<String, String> {
        let roots = self.ra_roots();
        let plan = scan(&roots);
        if plan.found != *seen {
            return Err("raimport.changed".into());
        }
        if plan.found.apply.is_empty() {
            return Err("raimport.nothing".into());
        }
        if backup {
            self.command_with_timeout("backuptool", &["backup"], BACKUP)
                .map_err(|error| {
                    log::warn!("Import: backup failed: {error}");
                    "raimport.backup_failed".to_owned()
                })?;
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
        let seen = backend.ra_scan();
        assert_eq!(seen.settings, 1);
        assert_eq!(
            backend.ra_import(true, &seen),
            Ok("raimport.done|count=1".into())
        );
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
        let seen = backend.ra_scan();
        assert_eq!(
            backend.ra_import(true, &seen),
            Err("raimport.backup_failed".into())
        );
        assert_eq!(saved(&paths), "video_smooth = \"false\"\n");
        assert_eq!(
            backend.ra_import(false, &seen),
            Ok("raimport.done|count=1".into())
        );
    }
    #[test]
    fn files_changed_since_the_review_import_nothing() {
        let (root, paths) = fixture("");
        let backend = Backend::new(paths.clone());
        let seen = backend.ra_scan();
        fs::write(
            paths.at("/storage/roms/retroarch-import/retroarch.cfg"),
            "video_smooth = \"true\"\nvideo_scale_integer = \"true\"\n",
        )
        .unwrap();
        assert_eq!(
            backend.ra_import(true, &seen),
            Err("raimport.changed".into())
        );
        assert!(!root.path().join("calls").exists(), "no backup either");
        assert_eq!(saved(&paths), "video_smooth = \"false\"\n");
    }
}
