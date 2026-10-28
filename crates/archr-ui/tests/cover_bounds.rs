use archr_ui::covers::{load_thumbnail, thumbnail_path};

#[test]
fn oversized_cached_thumbnail_is_rebuilt_with_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("cover.png");
    let cache = dir.path().join("thumbs");
    image::RgbaImage::new(100, 80).save(&source).unwrap();
    let file = thumbnail_path(&cache, &source, 40, 40).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    image::RgbaImage::new(400, 400).save(&file).unwrap();
    let thumbnail = load_thumbnail(&cache, &source, 40, 40).unwrap();
    assert_eq!((thumbnail.width, thumbnail.height), (40, 32));
}

#[test]
fn obsolete_fifo_size_marker_is_never_opened() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("cover.png");
    let cache = dir.path().join("thumbs");
    image::RgbaImage::new(100, 80).save(&source).unwrap();
    let marker = thumbnail_path(&cache, &source, 40, 40)
        .unwrap()
        .with_extension("size");
    std::fs::create_dir_all(&cache).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(&marker)
        .status()
        .unwrap()
        .success());
    let (done, ready) = std::sync::mpsc::channel();
    std::thread::spawn(move || done.send(load_thumbnail(&cache, &source, 40, 40)).unwrap());
    let thumbnail = ready
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!((thumbnail.width, thumbnail.height), (40, 32));
    use std::os::unix::fs::FileTypeExt;
    assert!(std::fs::metadata(&marker).unwrap().file_type().is_fifo());
}
