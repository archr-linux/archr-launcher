use archr_io::{
    atomic_publish, read_bytes, read_utf8,
    Symlinks::{Follow, Reject},
};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::{symlink, PermissionsExt},
    path::Path,
    process::Command,
};

#[test]
fn reads_enforce_byte_boundaries_and_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data");
    for (source, limit, accepted) in [
        ("", 0, true),
        ("é", 2, true),
        ("é", 1, false),
        ("abc", 2, false),
    ] {
        fs::write(&path, source).unwrap();
        assert_eq!(read_utf8(&path, limit, Follow).is_ok(), accepted);
        if accepted {
            assert_eq!(read_utf8(&path, limit, Follow).unwrap(), source);
        }
    }
    fs::write(&path, [0xff]).unwrap();
    assert_eq!(read_bytes(&path, 1, Follow).unwrap(), [0xff]);
    assert_eq!(
        read_utf8(&path, 1, Follow).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(read_bytes(&path, u64::MAX, Follow).unwrap(), [0xff]);
    assert_eq!(
        read_bytes(&dir.path().join("missing"), 1, Follow)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn actual_read_not_just_metadata_is_limited() {
    // procfs reports zero length but supplies bytes when read.
    let path = Path::new("/proc/self/cmdline");
    assert_eq!(fs::metadata(path).unwrap().len(), 0);
    assert_eq!(
        read_bytes(path, 1, Follow).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn nonregular_descriptors_never_block() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("fifo");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    // Bound failures even if a regression opens the FIFO in blocking mode.
    let (done, ready) = std::sync::mpsc::channel();
    let root = dir.path().to_owned();
    let worker = std::thread::spawn(move || {
        for path in [&fifo, &root, Path::new("/dev/null")] {
            assert_eq!(
                read_bytes(path, 8, Follow).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        done.send(()).unwrap();
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    worker.join().unwrap();
}

#[test]
fn symlink_reads_follow_policy_and_publish_replaces_link_not_target() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&path, "media").unwrap();
    symlink(&path, &link).unwrap();
    assert_eq!(read_utf8(&link, 5, Follow).unwrap(), "media");
    assert!(read_utf8(&link, 5, Reject).is_err());
    atomic_publish(&link, true, |file| file.write_all(b"new")).unwrap();
    assert!(!fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(fs::read(path).unwrap(), b"media");
    assert_eq!(fs::read(link).unwrap(), b"new");
}

#[test]
fn publish_replaces_without_truncation_for_durable_or_ephemeral_data() {
    let dir = tempfile::tempdir().unwrap();
    for durable in [false, true] {
        let path = dir.path().join("nested/data");
        atomic_publish(&path, durable, |file| file.write_all(b"old")).unwrap();
        let mut old = fs::File::open(&path).unwrap();
        atomic_publish(&path, durable, |file| {
            file.set_permissions(fs::Permissions::from_mode(0o640))?;
            file.write_all(b"new")
        })
        .unwrap();
        let mut source = String::new();
        old.read_to_string(&mut source).unwrap();
        assert_eq!(source, "old");
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
}

#[test]
fn failed_write_or_rename_preserves_destination_and_cleans_temporary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data");
    fs::write(&path, "old").unwrap();
    let error = atomic_publish(&path, true, |file| {
        file.write_all(b"partial")?;
        Err(io::Error::other("write failed"))
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "write failed");
    assert_eq!(fs::read(&path).unwrap(), b"old");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(atomic_publish(&path, true, |file| file.write_all(b"new")).is_err());
    assert!(path.is_dir());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn bounded_read_uses_the_open_descriptor_after_path_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data");
    fs::write(&path, "original").unwrap();
    let file = archr_io::open_regular(&path, Follow).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(Command::new("mkfifo")
        .arg(&path)
        .status()
        .unwrap()
        .success());
    assert_eq!(archr_io::read_bounded(&file, 8).unwrap(), b"original");
}

#[test]
fn parent_directory_handles_bare_and_nested_paths() {
    for (path, parent) in [
        ("store.toml", "."),
        ("config/store.toml", "config"),
        ("/store.toml", "/"),
    ] {
        assert_eq!(archr_io::parent_dir(Path::new(path)), Path::new(parent));
    }
}
