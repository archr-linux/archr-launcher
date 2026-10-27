use archr_ui::covers::{load_thumbnail, thumbnail_path};

#[test]
fn oversized_cached_thumbnail_is_rebuilt_with_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("cover.png");
    let cache = dir.path().join("thumbs");
    image::RgbaImage::new(100, 80).save(&source).unwrap();
    let file = thumbnail_path(&cache, &source).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    image::RgbaImage::new(400, 400).save(&file).unwrap();
    std::fs::write(file.with_extension("size"), "40x40").unwrap();
    let thumbnail = load_thumbnail(&cache, &source, 40, 40).unwrap();
    assert_eq!((thumbnail.width, thumbnail.height), (40, 32));
}
