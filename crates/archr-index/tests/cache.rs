use std::fs::{self, File, FileTimes};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use archr_index::cache::{
    build_overview, load_games, load_overview, save_overview, Overview, SystemEntry, CACHE_VERSION,
};
use archr_index::SystemDef;

fn system(path: &Path) -> SystemDef {
    SystemDef {
        name: "snes".into(),
        fullname: "Super Nintendo".into(),
        path: path.to_owned(),
        extensions: vec![".sfc".into()],
        emulators: Vec::new(),
    }
}

fn overview() -> Overview {
    Overview {
        systems: vec![SystemEntry {
            name: "snes".into(),
            fullname: "Super Nintendo".into(),
            game_count: 42,
            dir_mtime: 1_700_000_000_123_456_789,
        }],
    }
}

fn set_mtime(path: &Path, nanos: u64) {
    let time = UNIX_EPOCH + Duration::from_nanos(nanos);
    File::open(path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(time))
        .unwrap();
}

#[test]
fn roundtrip_overview() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("new/cache");
    assert!(load_overview(&cache).is_none());
    save_overview(&cache, &overview()).unwrap();
    let bytes = fs::read(cache.join("overview.bin")).unwrap();
    assert_eq!(&bytes[..4], b"ALAX");
    assert_eq!(&bytes[4..8], &CACHE_VERSION.to_le_bytes());
    let loaded = load_overview(&cache).unwrap();
    assert_eq!(loaded.systems.len(), 1);
    assert_eq!(loaded.systems[0].name, "snes");
    assert_eq!(loaded.systems[0].fullname, "Super Nintendo");
    assert_eq!(loaded.systems[0].game_count, 42);
    assert_eq!(loaded.systems[0].dir_mtime, 1_700_000_000_123_456_789);

    save_overview(&cache, &Overview { systems: vec![] }).unwrap();
    assert!(load_overview(&cache).unwrap().systems.is_empty());
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
}

#[test]
fn wrong_version_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    save_overview(dir.path(), &overview()).unwrap();
    let path = dir.path().join("overview.bin");
    let mut bytes = fs::read(&path).unwrap();
    bytes[4..8].copy_from_slice(&(CACHE_VERSION + 1).to_le_bytes());
    fs::write(path, bytes).unwrap();
    assert!(load_overview(dir.path()).is_none());
}

#[test]
fn truncated_file_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    save_overview(dir.path(), &overview()).unwrap();
    let path = dir.path().join("overview.bin");
    let bytes = fs::read(&path).unwrap();
    for length in 0..bytes.len() {
        fs::write(&path, &bytes[..length]).unwrap();
        assert!(load_overview(dir.path()).is_none(), "length {length}");
    }
}

#[test]
fn corrupt_or_trailing_data_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    save_overview(dir.path(), &overview()).unwrap();
    let path = dir.path().join("overview.bin");
    let bytes = fs::read(&path).unwrap();
    let mut bad_magic = bytes.clone();
    bad_magic[0] ^= 0xff;
    let mut trailing = bytes.clone();
    trailing.push(0);
    let mut bad_length = bytes;
    bad_length[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    for corrupted in [bad_magic, trailing, bad_length] {
        fs::write(&path, corrupted).unwrap();
        assert!(load_overview(dir.path()).is_none());
    }
}

#[test]
fn stale_mtime_triggers_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("First.sfc"), []).unwrap();
    set_mtime(&roms, 1_700_000_000_000_000_001);
    let sys = system(&roms);
    let cache = dir.path().join("cache");
    assert_eq!(load_games(&cache, &sys).len(), 1);
    fs::write(roms.join("Second.sfc"), []).unwrap();
    set_mtime(&roms, 1_700_000_000_000_000_002);
    let games = load_games(&cache, &sys);
    assert_eq!(games.len(), 2);
    assert_eq!(games[1].name, "Second");
}

#[test]
fn empty_systems_have_zero_count() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();
    let entries = build_overview(&[system(&empty), system(&dir.path().join("missing"))]);
    assert_eq!(entries.systems.len(), 2);
    assert!(entries.systems.iter().all(|entry| entry.game_count == 0));
    assert_eq!(entries.systems[1].dir_mtime, 0);
    assert!(build_overview(&[]).systems.is_empty());
}

#[test]
fn overview_scans_every_system() {
    let dir = tempfile::tempdir().unwrap();
    let mut systems = Vec::new();
    for (name, count) in [("snes", 2), ("nes", 1)] {
        let path = dir.path().join(name);
        fs::create_dir(&path).unwrap();
        for index in 0..count {
            fs::write(path.join(format!("Game{index}.sfc")), []).unwrap();
        }
        let mut sys = system(&path);
        sys.name = name.into();
        systems.push(sys);
    }
    let overview = build_overview(&systems);
    assert_eq!(overview.systems[0].name, "snes");
    assert_eq!(overview.systems[0].fullname, "Super Nintendo");
    assert_eq!(overview.systems[0].game_count, 2);
    assert_eq!(overview.systems[1].name, "nes");
    assert_eq!(overview.systems[1].game_count, 1);
}

#[test]
fn unchanged_fingerprint_uses_cache_and_preserves_all_game_fields() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    for name in ["Game.sfc", "cover.png", "clip.mp4"] {
        fs::write(roms.join(name), []).unwrap();
    }
    fs::write(roms.join("gamelist.xml"),
        "<gameList><game><path>Game.sfc</path><name>Custom title</name><image>cover.png</image><video>clip.mp4</video><favorite>true</favorite><lastplayed>20260102T030405</lastplayed></game></gameList>").unwrap();
    let sys = system(&roms);
    let cache = dir.path().join("cache");
    let fingerprint = build_overview(std::slice::from_ref(&sys)).systems[0].dir_mtime;
    let games = load_games(&cache, &sys);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].path, roms.join("Game.sfc"));
    assert_eq!(games[0].name, "Custom title");
    assert_eq!(games[0].image, Some(roms.join("cover.png")));
    assert_eq!(games[0].video, Some(roms.join("clip.mp4")));
    assert!(games[0].favorite);
    assert!(games[0].last_played.is_some());
    let bytes = fs::read(cache.join("systems/snes.bin")).unwrap();
    assert_eq!(&bytes[..4], b"ALAX");
    assert_eq!(&bytes[4..8], &CACHE_VERSION.to_le_bytes());
    assert_eq!(&bytes[8..16], &fingerprint.to_le_bytes());

    // Removing a ROM but restoring the root mtime distinguishes a hit from a scan.
    let modified = fs::metadata(&roms).unwrap().modified().unwrap();
    fs::remove_file(roms.join("Game.sfc")).unwrap();
    File::open(&roms)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(load_games(&cache, &sys), games);
}

#[test]
fn in_place_gamelist_edit_triggers_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("Game.sfc"), []).unwrap();
    let gamelist = roms.join("gamelist.xml");
    fs::write(&gamelist, "<gameList/>").unwrap();
    set_mtime(&roms, 1_700_000_000_000_000_000);
    set_mtime(&gamelist, 1_700_000_000_000_000_001);
    let sys = system(&roms);
    let cache = dir.path().join("cache");
    assert_eq!(load_games(&cache, &sys)[0].name, "Game");
    fs::write(&gamelist,
        "<gameList><game><path>Game.sfc</path><name>New title</name><favorite>true</favorite></game></gameList>").unwrap();
    set_mtime(&gamelist, 1_700_000_000_000_000_002);
    assert_eq!(load_games(&cache, &sys)[0].name, "New title");
    assert!(load_games(&cache, &sys)[0].favorite);
    assert_eq!(
        build_overview(&[sys]).systems[0].dir_mtime,
        1_700_000_000_000_000_002
    );
}

#[test]
fn new_media_triggers_rescan_in_each_supported_directory() {
    for (directory, filename, video) in [
        ("media/images", "Game.png", false),
        ("media/covers", "Game.jpg", false),
        ("media/boxart", "Game.jpeg", false),
        ("media/thumbnails", "Game.png", false),
        ("media/videos", "Game.mp4", true),
        ("images", "Game-image.png", false),
        ("videos", "Game-video.mp4", true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let roms = dir.path().join("roms");
        let media = roms.join(directory);
        fs::create_dir_all(&media).unwrap();
        fs::write(roms.join("Game.sfc"), []).unwrap();
        set_mtime(&roms, 1_700_000_000_000_000_000);
        set_mtime(&media, 1_700_000_000_000_000_001);
        let sys = system(&roms);
        let cache = dir.path().join("cache");
        let games = load_games(&cache, &sys);
        assert!(games[0].image.is_none());
        assert!(games[0].video.is_none());
        let path = media.join(filename);
        fs::write(&path, []).unwrap();
        set_mtime(&media, 1_700_000_000_000_000_002);
        let games = load_games(&cache, &sys);
        let found = if video {
            &games[0].video
        } else {
            &games[0].image
        };
        assert_eq!(found.as_ref(), Some(&path), "{directory}");
    }
}

#[test]
fn corrupt_game_cache_is_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("Game.sfc"), []).unwrap();
    let sys = system(&roms);
    let cache = dir.path().join("cache");
    let games = load_games(&cache, &sys);
    let path = cache.join("systems/snes.bin");
    let bytes = fs::read(&path).unwrap();
    let mut wrong_version = bytes.clone();
    wrong_version[4..8].copy_from_slice(&(CACHE_VERSION + 1).to_le_bytes());
    let mut trailing = bytes.clone();
    trailing.push(1);
    for corrupted in [
        vec![],
        b"garbage".to_vec(),
        bytes[..9].to_vec(),
        wrong_version,
        trailing,
    ] {
        fs::write(&path, corrupted).unwrap();
        assert_eq!(load_games(&cache, &sys), games);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn failed_cache_write_still_returns_scanned_games() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("Game.sfc"), []).unwrap();
    let cache = dir.path().join("not-a-directory");
    fs::write(&cache, []).unwrap();
    let games = load_games(&cache, &system(&roms));
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].name, "Game");
    assert!(save_overview(&cache, &overview()).is_err());
}

#[test]
fn unsafe_system_names_stay_inside_cache_without_collisions() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("Game.sfc"), []).unwrap();
    let cache = dir.path().join("cache");
    for name in [
        "../escape",
        "/absolute",
        "a/b",
        "a_b",
        "a%2Fb",
        "",
        ".",
        "..",
        "système",
    ] {
        let mut sys = system(&roms);
        sys.name = name.into();
        assert_eq!(load_games(&cache, &sys).len(), 1);
    }
    let entries: Vec<_> = fs::read_dir(cache.join("systems")).unwrap().collect();
    assert_eq!(entries.len(), 9);
    assert!(entries
        .iter()
        .all(|entry| entry.as_ref().unwrap().file_type().unwrap().is_file()));
    assert!(!cache.join("escape.bin").exists());
}

fn cache_header() -> Vec<u8> {
    let mut bytes = b"ALAX".to_vec();
    bytes.extend_from_slice(&CACHE_VERSION.to_le_bytes());
    bytes
}

// A sparse, valid UTF-8 string keeps oversized fixtures cheap to create. The
// zero-filled hole is part of a complete payload, not invalid trailing padding.
fn write_sparse_string(path: &Path, prefix: &[u8], length: u64, suffix: &[u8]) {
    let mut file = File::create(path).unwrap();
    file.write_all(prefix).unwrap();
    file.write_all(&length.to_le_bytes()).unwrap();
    file.seek(SeekFrom::Current(i64::try_from(length).unwrap()))
        .unwrap();
    file.write_all(suffix).unwrap();
}

#[test]
fn oversized_overview_is_ignored_without_removing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("overview.bin");
    let mut prefix = cache_header();
    // One system: name, empty fullname, zero count, zero fingerprint.
    prefix.extend_from_slice(&1_u64.to_le_bytes());
    write_sparse_string(&path, &prefix, 1024 * 1024, &[0; 20]);
    let original_len = fs::metadata(&path).unwrap().len();
    assert!(load_overview(dir.path()).is_none());
    assert_eq!(fs::metadata(&path).unwrap().len(), original_len);
}

#[test]
fn oversized_system_cache_is_ignored_and_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("Game.sfc"), []).unwrap();
    let sys = system(&roms);
    let cache = dir.path().join("cache");
    load_games(&cache, &sys);
    let path = cache.join("systems/snes.bin");
    let mut prefix = fs::read(&path).unwrap()[..16].to_vec(); // Header + fingerprint.
    prefix.extend_from_slice(&1_u64.to_le_bytes()); // One game.
    prefix.extend_from_slice(&10_u64.to_le_bytes());
    prefix.extend_from_slice(b"Cached.sfc");
    // Huge name, no image/video/last_played, not favorite.
    write_sparse_string(&path, &prefix, 32 * 1024 * 1024, &[0; 4]);
    let games = load_games(&cache, &sys);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].path, roms.join("Game.sfc"));
    assert!(fs::metadata(&path).unwrap().len() < 1024);
}

#[test]
fn huge_overview_vector_and_string_lengths_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("overview.bin");
    // These tiny payloads claim enormous allocations but have no element data.
    for lengths in [vec![u64::MAX], vec![1, u64::MAX], vec![1, 0, u64::MAX]] {
        let mut bytes = cache_header();
        for length in lengths {
            bytes.extend_from_slice(&length.to_le_bytes());
        }
        fs::write(&path, bytes).unwrap();
        assert!(load_overview(dir.path()).is_none());
    }
}

#[test]
fn huge_system_vector_and_string_lengths_are_ignored_and_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("roms");
    fs::create_dir(&roms).unwrap();
    fs::write(roms.join("Game.sfc"), []).unwrap();
    let sys = system(&roms);
    let cache = dir.path().join("cache");
    let expected = load_games(&cache, &sys);
    let path = cache.join("systems/snes.bin");
    let original = fs::read(&path).unwrap();
    // Game vector, game path string, then game name string.
    for lengths in [vec![u64::MAX], vec![1, u64::MAX], vec![1, 0, u64::MAX]] {
        let mut bytes = original[..16].to_vec();
        for length in lengths {
            bytes.extend_from_slice(&length.to_le_bytes());
        }
        fs::write(&path, bytes).unwrap();
        assert_eq!(load_games(&cache, &sys), expected);
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}

#[test]
fn oversized_overview_write_preserves_existing_cache() {
    let dir = tempfile::tempdir().unwrap();
    save_overview(dir.path(), &overview()).unwrap();
    let path = dir.path().join("overview.bin");
    let original = fs::read(&path).unwrap();
    let mut oversized = overview();
    oversized.systems[0].name = "x".repeat(1024 * 1024);
    assert!(save_overview(dir.path(), &oversized).is_err());
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
