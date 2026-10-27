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
    let first = thumbnail_path(&cache, &source).unwrap();
    assert_ne!(first, thumbnail_path(&cache, &other).unwrap());
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
    let second = thumbnail_path(&cache, &source).unwrap();
    assert_ne!(first, second);
    load_thumbnail(&cache, &source, 40, 40).unwrap();
    assert!(second.is_file());
    let resized = load_thumbnail(&cache, &source, 20, 20).unwrap();
    assert_eq!((resized.width, resized.height), (20, 16));
}
