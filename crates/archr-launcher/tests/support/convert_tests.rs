use super::*;
use std::time::{Duration, SystemTime};

fn pair(e: &str, c: &str) -> (String, String) {
    (e.into(), c.into())
}
/// A device tree: roms/, DuckStation naming cards after the file, and a
/// mupen64plus.ini.
fn env(card_type: &str) -> (tempfile::TempDir, Env) {
    let root = tempfile::tempdir().unwrap();
    let env = Env {
        roms: root.path().join("roms"),
        mupen_ini: root.path().join("mupen64plus.ini"),
        duckstation: root.path().join("duckstation/settings.ini"),
    };
    for system in ["nds", "psx", "n64"] {
        fs::create_dir_all(env.roms.join(system)).unwrap();
    }
    fs::create_dir_all(env.duckstation.parent().unwrap()).unwrap();
    fs::write(
        &env.duckstation,
        format!(
            "[GameList]\nRecursivePaths = /storage/roms/psx\n\n[MemoryCards]\n\
             Directory = /storage/roms/psx\nCard1Type = {card_type}\nCard2Type = None\n"
        ),
    )
    .unwrap();
    (root, env)
}
fn card() -> Vec<u8> {
    let mut card = vec![0u8; CARD];
    card[..2].copy_from_slice(b"MC");
    card[300] = 7;
    card
}
fn age(path: &Path, seconds: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(seconds))
        .unwrap();
}

#[test]
fn pairs_are_two_formats_of_one_system() {
    assert!(Format::pair(Format::Dsv, Format::Sav));
    assert!(Format::pair(Format::Mcd, Format::PsxSrm));
    assert!(!Format::pair(Format::Dsv, Format::Dsv));
    assert!(!Format::pair(Format::Dsv, Format::Mcd));
    assert_eq!(engine(("retroarch", "pcsx_rearmed32")), "pcsx_rearmed32");
    assert_eq!(keep_dir(("melonds", "melonds-sa")), ".antes-melonds");
}
#[test]
fn dsv_footer_comes_off_and_goes_back_byte_for_byte() {
    let raw: Vec<u8> = (0..8192u32).map(|n| (n * 7) as u8).collect();
    let dsv = raw_to_dsv(&raw);
    assert_eq!(dsv.len(), 8192 + DSV_FOOTER);
    assert_eq!(dsv_to_raw(&dsv).unwrap(), raw);
    // The footer DraStic itself wrote for an 8 KiB EEPROM.
    let tail = &dsv[dsv.len() - DSV_FOOTER + DSV_SNIP.len()..];
    assert_eq!(
        &tail[..24],
        [0, 32, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(dsv_to_raw(&raw), None, "no footer: not a .dsv");
}
#[test]
fn the_blank_pak_is_mupen64plus_format() {
    use sha2::{Digest, Sha256};
    // Measured on the Controller Paks of a real Mupen64Plus-Next .srm.
    let digest: String = Sha256::digest(blank_pak())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest,
        "056d71a353d325330a0f44115c0d7ba91c64ef6a47ab666602d5380a97f3703b"
    );
}
#[test]
fn n64_takes_the_eeprom_of_its_save_type_and_refuses_anything_else() {
    let mut eep = vec![0u8; 512];
    eep[52] = b'D';
    let srm = eep_to_srm(&eep, 0x200).unwrap();
    assert_eq!(srm.len(), N64_SRM);
    assert_eq!(
        srm_to_eep(&srm, 0x200).unwrap(),
        eep,
        "4 Kbit stays 512 bytes"
    );
    // 16 Kbit comes from the save type, even when the rest looks erased.
    assert_eq!(srm_to_eep(&srm, EEPROM).unwrap().len(), EEPROM);
    assert_eq!(eep_to_srm(&eep, EEPROM), None, "a 16 Kbit game needs 2 KiB");
    let mut sram = srm.clone();
    sram[EEPROM + 4 * PAK + 10] = 1;
    assert_eq!(srm_to_eep(&sram, 0x200), None, "SRAM data was never tested");
    let mut pak = srm.clone();
    pak[EEPROM + 0x300] = 9;
    assert_eq!(srm_to_eep(&pak, 0x200), None, "a pak in use stays");
    assert_eq!(
        srm_to_eep(&eep_to_srm(&[0xff; 512], 0x200).unwrap(), 0x200),
        None
    );
}
#[test]
fn mupen_names_follow_the_core() {
    // %.32s of the good name: Zelda's is longer than 32 bytes.
    assert_eq!(
        mupen_name(
            "Legend of Zelda, The - Ocarina of Time (U) (V1.0) [!]",
            "5BD1FE107BF8106B2AB6650ABECD54D6"
        )
        .unwrap(),
        b"Legend of Zelda, The - Ocarina o-5BD1FE10"
    );
    assert_eq!(
        mupen_name("Doom 64: Complete Edition", "0123456789ABCDEF").unwrap(),
        b"Doom 64_ Complete Edition-01234567"
    );
    assert_eq!(
        mupen_name("Super Mario 64 (U) [!]", "20B854B239203BAF6C961B850A4A51A2").unwrap(),
        b"Super Mario 64 (U) [!]-20B854B2"
    );
    // Byte 32 inside a character: cut there, as the core does.
    let name = mupen_name(&format!("{}é", "x".repeat(31)), "ABCDEF0123").unwrap();
    assert_eq!(name.len(), 32 + 9);
    assert_eq!(&name[31..33], [0xc3, b'-']);
    assert!(named(Path::new("/r"), &name, "eep")
        .to_string_lossy()
        .ends_with("-ABCDEF01.eep"));
    let ini =
        "[AAA]\nGoodName=Game (U)\nRefMD5=BBB\n\n[BBB]\nGoodName=Game (E)\nSaveType=Eeprom 16KB\n\
               [CCC]\nGoodName=Zelda\nSaveType=SRAM\n";
    let game = mupen_entry(ini, "AAA").unwrap();
    assert_eq!(
        (game.good.as_str(), game.eeprom),
        ("Game (U)", Some(EEPROM))
    );
    assert_eq!(mupen_entry(ini, "CCC").unwrap().eeprom, None);
}
#[test]
fn drastic_to_melonds_keeps_the_original_and_skips_saves_already_there() {
    let (_root, env) = env("PerGameFileTitle");
    let nds = env.roms.join("nds");
    for game in ["Mario Kart DS", "Zelda", "Nothing"] {
        fs::write(nds.join(format!("{game}.nds")), b"rom").unwrap();
    }
    let dsv = raw_to_dsv(&[7u8; 512]);
    fs::write(nds.join("Mario Kart DS.dsv"), &dsv).unwrap();
    fs::write(nds.join("Zelda.dsv"), &dsv).unwrap();
    fs::write(nds.join("Zelda.sav"), b"newer").unwrap();
    age(&nds.join("Zelda.dsv"), 600);
    let offer = offer(
        &env,
        "nds",
        None,
        pair("drastic", "drastic-arch"),
        pair("melonds", "melonds-sa"),
    )
    .unwrap();
    assert_eq!((offer.count, offer.replace, offer.up_to), (1, 0, false));
    assert_eq!(
        (offer.from_name.as_str(), offer.to_name.as_str()),
        ("DraStic", "melonDS")
    );
    assert_eq!(offer.keep, "roms/nds/.antes-melonds");
    assert_eq!(run(&env, &offer), Report { done: 1, failed: 0 });
    assert_eq!(fs::read(nds.join("Mario Kart DS.sav")).unwrap(), [7u8; 512]);
    assert_eq!(fs::read(nds.join("Mario Kart DS.dsv")).unwrap(), dsv);
    assert_eq!(
        fs::read(nds.join(".antes-melonds/Mario Kart DS.dsv")).unwrap(),
        dsv
    );
    assert_eq!(fs::read(nds.join("Zelda.sav")).unwrap(), b"newer");
}
#[test]
fn an_older_save_at_the_destination_is_replaced_after_its_copy() {
    let (_root, env) = env("PerGameFileTitle");
    let nds = env.roms.join("nds");
    fs::write(nds.join("Zelda.nds"), b"rom").unwrap();
    fs::write(nds.join("Zelda.sav"), [1u8; 512]).unwrap();
    fs::write(nds.join("Zelda.dsv"), raw_to_dsv(&[9u8; 512])).unwrap();
    age(&nds.join("Zelda.sav"), 600);
    let offer = offer(
        &env,
        "nds",
        None,
        pair("drastic", "drastic-arch"),
        pair("melonds", "melonds-sa"),
    )
    .unwrap();
    assert_eq!((offer.count, offer.replace), (1, 1));
    assert_eq!(run(&env, &offer), Report { done: 1, failed: 0 });
    assert_eq!(fs::read(nds.join("Zelda.sav")).unwrap(), [9u8; 512]);
    assert_eq!(
        fs::read(nds.join(".antes-melonds/Zelda.sav")).unwrap(),
        [1u8; 512]
    );
}
#[test]
fn a_bad_save_is_counted_and_the_batch_goes_on() {
    let (_root, env) = env("PerGameFileTitle");
    let nds = env.roms.join("nds");
    for game in ["A", "B", "C"] {
        fs::write(nds.join(format!("{game}.nds")), b"rom").unwrap();
    }
    fs::write(nds.join("A.dsv"), raw_to_dsv(&[1u8; 512])).unwrap();
    fs::write(nds.join("B.dsv"), b"not a dsv").unwrap();
    // An 8 Mbit save fits.
    fs::write(nds.join("C.dsv"), raw_to_dsv(&vec![3u8; 1 << 20])).unwrap();
    let offer = offer(
        &env,
        "nds",
        None,
        pair("drastic", "drastic-arch"),
        pair("melonds", "melonds-sa"),
    )
    .unwrap();
    assert_eq!(run(&env, &offer), Report { done: 2, failed: 1 });
    assert_eq!(fs::read(nds.join("C.sav")).unwrap().len(), 1 << 20);
    assert!(!nds.join("B.sav").exists());
}
#[test]
fn backups_never_overwrite_an_older_copy() {
    let root = tempfile::tempdir().unwrap();
    let save = root.path().join("Zelda.dsv");
    let keep = root.path().join(".antes-melonds");
    back_up(&keep, &save, b"one").unwrap();
    back_up(&keep, &save, b"two").unwrap();
    assert_eq!(fs::read(keep.join("Zelda.dsv")).unwrap(), b"one");
    assert_eq!(fs::read(keep.join("Zelda.dsv.2")).unwrap(), b"two");
    assert_eq!(
        create_new(&keep.join("Zelda.dsv"), b"x")
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
}
#[test]
fn an_eep_into_an_older_srm_keeps_its_paks_sram_and_flash() {
    let (_root, mut env) = env("PerGameFileTitle");
    let n64 = env.roms.join("n64");
    // A Mupen64Plus-Next .srm with a Controller Pak in use (ghosts).
    let mut srm = eep_to_srm(&[1u8; 512], 0x200).unwrap();
    srm[EEPROM + 0x500] = 0x77;
    srm[EEPROM + 4 * PAK + 5] = 0x55;
    let eep = [2u8; 512];
    let step = Step {
        source: n64.join("g.eep"),
        target: n64.join("Game.srm"),
        replace: true,
        eeprom: 0x200,
    };
    fs::write(&step.source, eep).unwrap();
    fs::write(&step.target, &srm).unwrap();
    env.mupen_ini = n64.join("none");
    let plan = Plan {
        from: Format::MupenSa,
        to: Format::N64Srm,
        steps: Vec::new(),
        unread: 0,
    };
    let keep = n64.join(".antes-mupen64plus_next");
    assert!(apply(&plan, &step, &keep).unwrap());
    let merged = fs::read(&step.target).unwrap();
    assert_eq!(&merged[..0x200], eep);
    assert_eq!(
        &merged[0x200..],
        &srm[0x200..],
        "paks, SRAM and FlashRAM kept"
    );
    assert_eq!(fs::read(keep.join("Game.srm")).unwrap(), srm);
    assert_eq!(fs::read(keep.join("g.eep")).unwrap(), eep);
}
#[test]
fn psx_cards_follow_the_game_file_title_in_duckstations_folder() {
    let (_root, env) = env("PerGameFileTitle");
    let psx = env.roms.join("psx");
    // A cue sheet and its track are one game.
    fs::write(psx.join("Crash.cue"), b"cue").unwrap();
    fs::write(psx.join("Crash.bin"), b"bin").unwrap();
    fs::write(psx.join("Crash.srm"), card()).unwrap();
    let offer = offer(
        &env,
        "psx",
        None,
        pair("retroarch", "pcsx_rearmed32"),
        pair("duckstation", "duckstation-sa"),
    )
    .unwrap();
    assert_eq!(offer.count, 1);
    assert_eq!(offer.keep, "roms/psx/.antes-duckstation");
    assert_eq!(run(&env, &offer), Report { done: 1, failed: 0 });
    assert_eq!(fs::read(psx.join("Crash_1.mcd")).unwrap(), card());
    // And back, for a file title DuckStation cleans up.
    fs::write(psx.join("Tekken*3.chd"), b"rom").unwrap();
    fs::write(psx.join("Tekken_3_1.mcd"), card()).unwrap();
    let back = super::offer(
        &env,
        "psx",
        Some(psx.join("Tekken*3.chd")),
        pair("duckstation", "duckstation-sa"),
        pair("retroarch", "swanstation"),
    )
    .unwrap();
    assert_eq!(run(&env, &back), Report { done: 1, failed: 0 });
    assert_eq!(fs::read(psx.join("Tekken*3.srm")).unwrap(), card());
}
#[test]
fn psx_offers_nothing_while_duckstation_names_cards_by_database_title() {
    let (_root, env) = env("PerGameTitle");
    let psx = env.roms.join("psx");
    fs::write(psx.join("Crash.chd"), b"rom").unwrap();
    fs::write(psx.join("Crash.srm"), card()).unwrap();
    assert!(offer(
        &env,
        "psx",
        None,
        pair("retroarch", "pcsx_rearmed32"),
        pair("duckstation", "duckstation-sa"),
    )
    .is_none());
}
#[test]
fn a_whole_n64_system_is_counted_up_to() {
    let (_root, env) = env("PerGameFileTitle");
    let n64 = env.roms.join("n64");
    fs::write(n64.join("A.z64"), b"rom").unwrap();
    fs::write(n64.join("B.z64"), b"rom").unwrap();
    fs::write(n64.join("A.srm"), vec![0u8; N64_SRM]).unwrap();
    let offer = offer(
        &env,
        "n64",
        None,
        pair("retroarch", "mupen64plus_next"),
        pair("mupen64plus", "mupen64plus-arch"),
    )
    .unwrap();
    assert_eq!((offer.count, offer.up_to), (1, true));
}
#[test]
fn the_runner_answers_every_request_in_order() {
    let (_root, env) = env("PerGameFileTitle");
    let mut runner = Runner::new(env.clone());
    runner.send(Request::Plan {
        system: "nds".into(),
        rom: None,
        from: pair("drastic", "drastic-arch"),
        to: pair("melonds", "melonds-sa"),
    });
    runner.send(Request::Run(Box::default()));
    let mut events = Vec::new();
    while events.len() < 2 {
        events.extend(runner.poll());
        std::thread::yield_now();
    }
    assert!(matches!(events[0], Event::Offer(None)));
    assert!(matches!(events[1], Event::Done(Report { failed: 1, .. })));
}
/// Real saves, when a folder of them is given: ARCHR_REAL_SAVES holds
/// mupen64plus.ini and nds/, psx/ and n64/ as copied from a device: .dsv
/// files from DraStic, 128 KiB cards (RetroArch .srm, DuckStation .mcd),
/// and N64 games (zipped ROM, the standalone's .eep and .mpk, perhaps
/// Mupen64Plus-Next's .srm).
#[test]
fn real_saves_convert_both_ways() {
    let Some(dir) = std::env::var_os("ARCHR_REAL_SAVES").map(PathBuf::from) else {
        return;
    };
    let list = |system: &str| -> Vec<PathBuf> {
        fs::read_dir(dir.join(system))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect()
    };
    let ext = |p: &Path, e: &str| p.extension().is_some_and(|x| x == e);
    let name = |p: &Path| p.file_name().unwrap().to_str().unwrap().to_owned();
    let mut checked = 0;
    for path in list("nds").iter().filter(|p| ext(p, "dsv")) {
        let bytes = fs::read(path).unwrap();
        let raw = dsv_to_raw(&bytes).expect("a DraStic footer");
        assert_eq!(raw_to_dsv(&raw), bytes, "{}", path.display());
        checked += 1;
    }
    // A DuckStation card goes to RetroArch and back through the whole path.
    for path in list("psx").iter().filter(|p| {
        ext(p, "mcd") || (ext(p, "srm") && fs::metadata(p).unwrap().len() == CARD as u64)
    }) {
        let bytes = fs::read(path).unwrap();
        let (_root, env) = env("PerGameFileTitle");
        let psx = env.roms.join("psx");
        fs::write(psx.join("Game.chd"), b"rom").unwrap();
        let (from, to) = if ext(path, "mcd") {
            fs::write(psx.join("Game_1.mcd"), &bytes).unwrap();
            (
                pair("duckstation", "duckstation-sa"),
                pair("retroarch", "pcsx_rearmed32"),
            )
        } else {
            fs::write(psx.join("Game.srm"), &bytes).unwrap();
            (
                pair("retroarch", "pcsx_rearmed32"),
                pair("duckstation", "duckstation-sa"),
            )
        };
        let offer = offer(&env, "psx", None, from, to).expect("an offer");
        assert_eq!(run(&env, &offer), Report { done: 1, failed: 0 });
        let made = if ext(path, "mcd") {
            "Game.srm"
        } else {
            "Game_1.mcd"
        };
        assert_eq!(
            fs::read(psx.join(made)).unwrap(),
            bytes,
            "{}",
            path.display()
        );
        checked += 1;
    }
    let ini = dir.join("mupen64plus.ini");
    let files = list("n64");
    for zip in files.iter().filter(|p| ext(p, "zip")) {
        let stem = zip.file_stem().unwrap().to_str().unwrap();
        let md5 = archr_cheevos::rom_hash("n64", zip)
            .unwrap()
            .unwrap()
            .to_uppercase();
        let text = fs::read_to_string(&ini).unwrap();
        let game = mupen_entry(&text, &md5).expect("in mupen64plus.ini");
        let sa = String::from_utf8(mupen_name(&game.good, &md5).unwrap()).unwrap();
        let own: Vec<&PathBuf> = files
            .iter()
            .filter(|p| name(p).starts_with(&format!("{sa}.")) || name(p) == format!("{stem}.srm"))
            .collect();
        let real = |e: &str| own.iter().find(|p| ext(p, e)).map(|p| fs::read(p).unwrap());
        let pak_used = real("mpk").is_some_and(|b| !b.chunks(PAK).all(pak_unused));
        let fresh = |keep: &[&PathBuf]| {
            let (root, mut env) = env("PerGameFileTitle");
            env.mupen_ini = ini.clone();
            let n64 = env.roms.join("n64");
            fs::copy(zip, n64.join(name(zip))).unwrap();
            for p in keep {
                fs::copy(p, n64.join(name(p))).unwrap();
            }
            (root, env, n64)
        };
        let sa_pair = pair("mupen64plus", "mupen64plus-arch");
        let next_pair = pair("retroarch", "mupen64plus_next");
        // Standalone → Next, then that .srm back: the same .eep.
        if let Some(eep) = real("eep") {
            let keep: Vec<&PathBuf> = own.iter().copied().filter(|p| !ext(p, "srm")).collect();
            let (_r, env, n64) = fresh(&keep);
            let rom = Some(n64.join(name(zip)));
            let offer = offer(&env, "n64", rom.clone(), sa_pair.clone(), next_pair.clone());
            if pak_used {
                assert!(offer.is_none(), "{stem}: a Controller Pak in use stays");
                checked += 1;
                continue;
            }
            assert_eq!(
                run(&env, &offer.expect(stem)),
                Report { done: 1, failed: 0 }
            );
            let srm = fs::read(n64.join(format!("{stem}.srm"))).unwrap();
            if let Some(real_srm) = real("srm") {
                assert_eq!(srm, real_srm, "{stem}: Mupen64Plus-Next's own .srm");
            }
            let (_r2, env2, n64b) = fresh(&[]);
            fs::write(n64b.join(format!("{stem}.srm")), &srm).unwrap();
            let back = super::offer(
                &env2,
                "n64",
                rom.map(|_| n64b.join(name(zip))),
                next_pair,
                sa_pair,
            )
            .expect(stem);
            assert_eq!(run(&env2, &back), Report { done: 1, failed: 0 });
            assert_eq!(
                fs::read(n64b.join(format!("{sa}.eep"))).unwrap(),
                eep,
                "{stem}"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no real save in {}", dir.display());
    eprintln!("real saves checked: {checked}");
}
