//! Game hashes of small sample files, against digests computed independently.
use archr_cheevos::{rom_hash, supported};
use md5::{Digest, Md5};
use std::{fs, io::Write, path::Path};

fn md5(parts: &[&[u8]]) -> String {
    let mut md5 = Md5::new();
    for part in parts {
        md5.update(part);
    }
    md5.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn rom(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|n| (n as u8).wrapping_mul(31) ^ seed)
        .collect()
}

fn hash(system: &str, path: &Path) -> Option<String> {
    rom_hash(system, path).unwrap()
}

#[test]
fn a_known_digest_for_an_empty_game_boy_rom() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.gb");
    fs::write(&path, b"").unwrap();
    assert_eq!(
        hash("gb", &path).as_deref(),
        Some("d41d8cd98f00b204e9800998ecf8427e")
    );
}

#[test]
fn cartridge_headers_are_left_out_like_rcheevos_does() {
    let dir = tempfile::tempdir().unwrap();
    let body = rom(0x8000, 7);
    let write = |name: &str, bytes: &[u8]| {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    };
    // iNES header.
    let nes = write(
        "mario.nes",
        &[b"NES\x1a".as_slice(), &[0; 12], &body].concat(),
    );
    assert_eq!(hash("nes", &nes), Some(md5(&[&body])));
    // Copier header: 512 bytes over a multiple of 8 KiB.
    let snes = write("smw.sfc", &[vec![0xAA; 512], body.clone()].concat());
    assert_eq!(hash("snes", &snes), Some(md5(&[&body])));
    let plain = write("smw2.sfc", &body);
    assert_eq!(hash("snes", &plain), Some(md5(&[&body])));
    // Game Boy, Game Boy Advance and Mega Drive: the whole file.
    for system in ["gb", "gbc", "gba", "megadrive", "genesis", "mastersystem"] {
        assert_eq!(hash(system, &plain), Some(md5(&[&body])), "{system}");
    }
    let lynx = write(
        "game.lnx",
        &[b"LYNX\0".as_slice(), &[0; 59], &body].concat(),
    );
    assert_eq!(hash("atarilynx", &lynx), Some(md5(&[&body])));
    let a7800 = write(
        "game.a78",
        &[&[1u8][..], b"ATARI7800", &[0; 118], &body].concat(),
    );
    assert_eq!(hash("atari7800", &a7800), Some(md5(&[&body])));
    let pce = write("game.pce", &[vec![0; 512], body.clone()].concat());
    assert_eq!(hash("pcengine", &pce), Some(md5(&[&body])));
}

#[test]
fn n64_images_hash_in_big_endian_order() {
    let dir = tempfile::tempdir().unwrap();
    let z64: Vec<u8> = [&[0x80u8, 0x37, 0x12, 0x40][..], &rom(64, 3)].concat();
    let v64: Vec<u8> = z64.chunks(2).flat_map(|w| [w[1], w[0]]).collect();
    let n64: Vec<u8> = z64
        .chunks(4)
        .flat_map(|w| [w[3], w[2], w[1], w[0]])
        .collect();
    for (name, bytes) in [("a.z64", &z64), ("a.v64", &v64), ("a.n64", &n64)] {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        assert_eq!(hash("n64", &path), Some(md5(&[&z64])), "{name}");
    }
}

fn zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let (mut out, mut directory) = (Vec::new(), Vec::new());
    for (name, data, deflate) in entries {
        let packed = if *deflate {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        } else {
            data.to_vec()
        };
        let method: u16 = if *deflate { 8 } else { 0 };
        let offset = out.len() as u32;
        let sizes = [packed.len() as u32, data.len() as u32];
        out.extend(b"PK\x03\x04");
        out.extend([20, 0, 0, 0]);
        out.extend(method.to_le_bytes());
        out.extend([0; 8]); // time, date, crc
        out.extend(sizes.iter().flat_map(|s| s.to_le_bytes()));
        out.extend((name.len() as u16).to_le_bytes());
        out.extend([0, 0]);
        out.extend(name.as_bytes());
        out.extend(&packed);
        directory.extend(b"PK\x01\x02");
        directory.extend([20, 0, 20, 0, 0, 0]);
        directory.extend(method.to_le_bytes());
        directory.extend([0; 8]);
        directory.extend(sizes.iter().flat_map(|s| s.to_le_bytes()));
        directory.extend((name.len() as u16).to_le_bytes());
        directory.extend([0; 12]);
        directory.extend(offset.to_le_bytes());
        directory.extend(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend(&directory);
    out.extend(b"PK\x05\x06");
    out.extend([0; 4]);
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((directory.len() as u32).to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend([0, 0]);
    out
}

#[test]
fn zipped_roms_hash_their_first_file() {
    let dir = tempfile::tempdir().unwrap();
    let body = rom(40_000, 9);
    let nes = [b"NES\x1a".as_slice(), &[0; 12], &body].concat();
    for deflate in [false, true] {
        let path = dir.path().join(format!("mario-{deflate}.zip"));
        fs::write(
            &path,
            zip(&[("roms/", b"", false), ("mario.nes", &nes, deflate)]),
        )
        .unwrap();
        assert_eq!(hash("nes", &path), Some(md5(&[&body])));
    }
    let broken = dir.path().join("broken.zip");
    fs::write(&broken, b"PK\x03\x04 not really").unwrap();
    assert!(rom_hash("nes", &broken).is_err());
}

#[test]
fn arcade_games_hash_their_name_and_unsupported_formats_are_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sf2.zip");
    fs::write(&path, b"whatever").unwrap();
    assert_eq!(hash("arcade", &path), Some(md5(&[b"sf2"])));
    assert_eq!(hash("fbneo", &path), Some(md5(&[b"sf2"])));
    let chd = dir.path().join("game.chd");
    fs::write(&chd, b"MComprHD").unwrap();
    assert_eq!(hash("psx", &chd), None);
    assert_eq!(hash("dreamcast", &chd), None);
    assert!(supported("snes") && supported("psx") && !supported("ports"));
}

/// A tiny PlayStation disc: ISO 9660 with SYSTEM.CNF and a PS-X EXE in a
/// folder, as 2048-byte sectors (ISO) or raw MODE2/2352 sectors (cue/bin).
fn disc(exe_len: u32) -> (Vec<[u8; 2048]>, Vec<u8>) {
    let mut sectors = vec![[0u8; 2048]; 24];
    let record = |extent: u32, size: u32, name: &[u8], dir: bool| {
        let mut r = vec![0u8; 33 + name.len() + (name.len() + 1) % 2];
        r[0] = r.len() as u8;
        r[2..6].copy_from_slice(&extent.to_le_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[25] = if dir { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    };
    sectors[16][0] = 1;
    sectors[16][1..6].copy_from_slice(b"CD001");
    sectors[16][128..130].copy_from_slice(&2048u16.to_le_bytes());
    let root = record(18, 2048, b"\0", true);
    sectors[16][156..156 + root.len()].copy_from_slice(&root);
    let config = b"BOOT = cdrom:\\GAME\\SLUS_000.01;1\r\nTCB = 4\r\n";
    let entries = [
        record(18, 2048, b"\0", true),
        record(18, 2048, b"\x01", true),
        record(20, config.len() as u32, b"SYSTEM.CNF;1", false),
        record(19, 2048, b"GAME", true),
    ];
    let mut at = 0;
    for entry in entries {
        sectors[18][at..at + entry.len()].copy_from_slice(&entry);
        at += entry.len();
    }
    let exe = record(21, 3 * 2048, b"SLUS_000.01;1", false);
    sectors[19][..exe.len()].copy_from_slice(&exe);
    sectors[20][..config.len()].copy_from_slice(config);
    for (n, sector) in sectors[21..24].iter_mut().enumerate() {
        sector.fill(n as u8 + 1);
    }
    sectors[21][..8].copy_from_slice(b"PS-X EXE");
    sectors[21][28..32].copy_from_slice(&exe_len.to_le_bytes());
    // What rcheevos hashes: the name, then the header plus t_size bytes.
    let mut data = Vec::new();
    for sector in &sectors[21..24] {
        data.extend_from_slice(sector);
    }
    data.truncate(exe_len as usize + 2048);
    (sectors, data)
}

#[test]
fn playstation_discs_hash_the_boot_executable_from_cue_iso_and_m3u() {
    let dir = tempfile::tempdir().unwrap();
    let (sectors, exe) = disc(2048 + 100);
    let expected = md5(&[b"GAME\\SLUS_000.01", &exe]);
    let iso = dir.path().join("game.iso");
    fs::write(&iso, sectors.concat()).unwrap();
    assert_eq!(hash("psx", &iso), Some(expected.clone()));
    let raw: Vec<u8> = sectors
        .iter()
        .flat_map(|s| [vec![0xFFu8; 24], s.to_vec(), vec![0; 280]].concat())
        .collect();
    fs::write(dir.path().join("Game (USA).bin"), raw).unwrap();
    let cue = dir.path().join("Game (USA).cue");
    fs::write(
        &cue,
        "FILE \"Game (USA).bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n",
    )
    .unwrap();
    assert_eq!(hash("psx", &cue), Some(expected.clone()));
    let m3u = dir.path().join("Game.m3u");
    fs::write(&m3u, "# discs\nGame (USA).cue\n").unwrap();
    assert_eq!(hash("psx", &m3u), Some(expected));
}

#[test]
fn a_disc_without_system_cnf_boots_psx_exe() {
    let dir = tempfile::tempdir().unwrap();
    let (mut sectors, _) = disc(10);
    // Rename SYSTEM.CNF away and put PSX.EXE (no PS-X header) in the root.
    let root = &mut sectors[18];
    let at = root.windows(10).position(|w| w == b"SYSTEM.CNF").unwrap();
    root[at..at + 10].copy_from_slice(b"README.TXT");
    let gone = root.windows(4).position(|w| w == b"GAME").unwrap();
    root[gone - 33 + 2..gone - 33 + 6].copy_from_slice(&22u32.to_le_bytes());
    root[gone - 33 + 10..gone - 33 + 14].copy_from_slice(&700u32.to_le_bytes());
    root[gone - 33] = 40;
    root[gone - 1] = 7;
    root[gone..gone + 7].copy_from_slice(b"PSX.EXE");
    sectors[22].fill(0x42);
    let iso = dir.path().join("demo.iso");
    fs::write(&iso, sectors.concat()).unwrap();
    assert_eq!(hash("psx", &iso), Some(md5(&[b"PSX.EXE", &[0x42; 700]])));
}
