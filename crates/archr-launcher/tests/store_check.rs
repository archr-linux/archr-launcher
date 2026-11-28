//! The theme store's own install rules run over theme folders before they
//! are published: `ARCHR_STORE_CHECK` lists folders (separated by `:`),
//! each packed as GitHub packs a repository (one top folder, gzip) and
//! unpacked by the store's extractor. Without it, a small theme is checked.
use std::path::Path;

/// `dir` as GitHub's tarball of a repository: one top folder, gzip;
/// files and folders starting with `.` stay out, as in a repository.
fn tarball(dir: &Path) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    let mut pending = vec![dir.to_path_buf()];
    while let Some(at) = pending.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&at).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            let path = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap();
                tar.append_path_with_name(&path, Path::new("owner-repo-0123abc").join(rel))
                    .unwrap();
            }
        }
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// Files and bytes under `dir`.
fn count(dir: &Path) -> (usize, u64) {
    let mut found = (0, 0);
    let mut pending = vec![dir.to_path_buf()];
    while let Some(at) = pending.pop() {
        for e in std::fs::read_dir(&at).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.0 += 1;
                found.1 += e.metadata().unwrap().len();
            }
        }
    }
    found
}

fn check(dir: &Path) {
    let packed = tarball(dir);
    let (entries, _) = count(dir);
    assert!(
        (packed.len() as u64) <= archr_launcher::themestore::TARBALL_LIMIT,
        "{}: download of {} bytes",
        dir.display(),
        packed.len()
    );
    let into = tempfile::tempdir().unwrap();
    archr_launcher::themestore::extract(&packed, into.path())
        .unwrap_or_else(|e| panic!("{}: the store refuses it: {e}", dir.display()));
    let (kept, bytes) = count(into.path());
    assert!(
        into.path().join("theme.toml").is_file(),
        "{}",
        dir.display()
    );
    println!(
        "{}: download {:.1} MB, installed {:.1} MB, {kept} of {entries} files kept",
        dir.file_name().unwrap().to_string_lossy(),
        packed.len() as f64 / 1_048_576.,
        bytes as f64 / 1_048_576.
    );
}

#[test]
fn theme_folders_pass_the_store_rules() {
    match std::env::var_os("ARCHR_STORE_CHECK") {
        Some(dirs) => std::env::split_paths(&dirs).for_each(|d| check(&d)),
        None => {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("theme.toml"), "name = \"T\"\n").unwrap();
            std::fs::create_dir_all(dir.path().join("videos")).unwrap();
            std::fs::write(dir.path().join("videos/snes.mp4"), [0u8; 64]).unwrap();
            std::fs::write(dir.path().join("run.sh"), "x").unwrap();
            check(dir.path());
        }
    }
}
