use archr_ui::covers::{load_thumbnail, thumbnail_path};
use std::{
    fs,
    time::{Duration, SystemTime},
};

#[test]
fn thumbnail_cached_by_path_and_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("cover.png");
    let other = dir.path().join("other.png");
    let cache = dir.path().join("thumbs");
    image::RgbaImage::from_pixel(100, 80, image::Rgba([200, 20, 30, 255]))
        .save(&source)
        .unwrap();
    fs::copy(&source, &other).unwrap();
    let first = thumbnail_path(&cache, &source, 40, 40).unwrap();
    assert_ne!(first, thumbnail_path(&cache, &other, 40, 40).unwrap());
    let thumb = load_thumbnail(&cache, &source, 40, 40).unwrap();
    assert_eq!((thumb.width, thumb.height), (40, 32));
    assert!(first.is_file());
    let cached_mtime = fs::metadata(&first).unwrap().modified().unwrap();
    assert_eq!(
        load_thumbnail(&cache, &source, 40, 40).unwrap().rgba,
        thumb.rgba
    );
    assert_eq!(
        fs::metadata(&first).unwrap().modified().unwrap(),
        cached_mtime
    );
    let changed = SystemTime::now() + Duration::from_secs(2);
    fs::File::open(&source)
        .unwrap()
        .set_modified(changed)
        .unwrap();
    let second = thumbnail_path(&cache, &source, 40, 40).unwrap();
    assert_ne!(first, second);
    load_thumbnail(&cache, &source, 40, 40).unwrap();
    assert!(second.is_file());
    assert_ne!(second, thumbnail_path(&cache, &source, 20, 20).unwrap());
    let second_mtime = fs::metadata(&second).unwrap().modified().unwrap();
    let resized = load_thumbnail(&cache, &source, 20, 20).unwrap();
    assert_eq!((resized.width, resized.height), (20, 16));
    assert_eq!(
        fs::metadata(&second).unwrap().modified().unwrap(),
        second_mtime,
        "a second size must not replace an existing thumbnail"
    );
    assert!(
        fs::read_dir(&cache).unwrap().all(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "png")
        }),
        "thumbnail dimensions belong in the key, not a sidecar"
    );
}
