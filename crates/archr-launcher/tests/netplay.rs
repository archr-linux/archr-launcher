use archr_launcher::{
    handoff::netplay_command,
    netplay::{arrange, parse_lan, parse_lobby, resolve, Entry, Filters, Local},
};
use archr_ui::netplay::{Group, Rom};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

const LOBBY: &str = r#"[
 {"model":"x","fields":{"username":"marcos","game_name":"Super Mario World (USA)","game_crc":"B19ED489","core_name":"Snes9x","country":"br","ip":"203.0.113.9","port":55435,"host_method":2,"has_password":true}},
 {"fields":{"username":"lu","game_name":"Sonic the Hedgehog 2","game_crc":"00000000","core_name":"Genesis Plus GX","country":"es","ip":"1.2.3.4","port":55435,"host_method":3,"mitm_ip":"198.51.100.7","mitm_port":55436,"mitm_session":"abc"}},
 {"fields":{"username":"rafa","game_name":"Tekken 3","game_crc":"11111111","core_name":"Beetle PSX","country":"ca","ip":"5.6.7.8","port":1}},
 {"nofields":true}
]"#;

fn local(system: &str, cores: &[(&str, &str)], games: &[(&str, &str)]) -> Local {
    Local::from_parts(
        system,
        cores
            .iter()
            .map(|(e, c)| ((*e).into(), (*c).into()))
            .collect(),
        games.iter().map(|(p, n)| (PathBuf::from(p), (*n).into())),
    )
}

#[test]
fn lobby_json_keeps_rooms_with_fields_and_dials_the_relay_when_present() {
    let rooms = parse_lobby(LOBBY).unwrap();
    assert_eq!(rooms.len(), 3);
    assert_eq!(
        (rooms[0].crc, rooms[0].port, rooms[0].relay.clone()),
        (0xB19E_D489, 55435, None)
    );
    assert_eq!(rooms[1].crc, 0);
    assert_eq!(
        rooms[1].relay,
        Some(("198.51.100.7".into(), 55436, "abc".into()))
    );
    assert!(parse_lobby("{}").is_err() && parse_lobby("nope").is_err());
}

#[test]
fn lan_reply_is_decoded_from_the_fixed_big_endian_packet() {
    let mut packet = Vec::new();
    packet.extend(b"RANS");
    packet.extend(0xB19E_D489u32.to_be_bytes());
    packet.extend(55435i32.to_be_bytes());
    packet.extend(0u32.to_be_bytes());
    for (text, len) in [
        ("Ana", 32),
        ("RetroArch", 32),
        ("Snes9x", 32),
        ("1.0", 32),
        ("1.19", 32),
        ("Super Mario World", 256),
        ("", 256),
    ] {
        let mut field = text.as_bytes().to_vec();
        field.resize(len, 0);
        packet.extend(field);
    }
    let entry = parse_lan(&packet, "192.168.3.21").unwrap();
    assert_eq!(
        (
            entry.host.as_str(),
            entry.game.as_str(),
            entry.core.as_str()
        ),
        ("Ana", "Super Mario World", "Snes9x")
    );
    assert_eq!(
        (entry.crc, entry.port, entry.lan),
        (0xB19E_D489, 55435, true)
    );
    assert!(parse_lan(&packet[..100], "x").is_none());
    packet[0] = b'X';
    assert!(parse_lan(&packet, "x").is_none());
}

#[test]
fn rooms_are_matched_by_title_and_core_then_grouped_and_ordered() {
    let entries = parse_lobby(LOBBY).unwrap();
    let locals = [
        local(
            "snes",
            &[("retroarch", "snes9x")],
            &[("/r/snes/Super_Mario_World.sfc", "Super Mario World")],
        ),
        local(
            "megadrive",
            &[("retroarch", "genesis_plus_gx")],
            &[("/r/md/Sonic The Hedgehog 2.md", "Sonic the Hedgehog 2")],
        ),
        local("psx", &[("retroarch", "pcsx_rearmed")], &[]),
    ];
    let mut hashes = 0;
    let mut hash = |path: &Path| {
        hashes += 1;
        Some(if path.ends_with("Super_Mario_World.sfc") {
            0xB19E_D489
        } else {
            0xDEAD_BEEF
        })
    };
    let rooms: Vec<_> = entries
        .iter()
        .map(|e| resolve(e, &locals, &mut hash))
        .collect();
    assert_eq!((rooms[0].rom, rooms[0].group), (Rom::Same, Group::Online));
    let join = rooms[0].join.as_ref().unwrap();
    assert_eq!(
        (
            join.system.as_str(),
            join.core.as_str(),
            join.ip.as_str(),
            join.port
        ),
        ("snes", "snes9x", "203.0.113.9", 55435)
    );
    // Relay rooms dial the relay; an unknown lobby CRC is not a mismatch.
    assert_eq!(rooms[1].rom, Rom::Unknown);
    let join = rooms[1].join.as_ref().unwrap();
    assert_eq!(
        (join.ip.as_str(), join.port, rooms[1].relay.as_str()),
        ("198.51.100.7", 55436, "198.51.100.7")
    );
    // "Beetle PSX" is not installed anywhere.
    assert_eq!(
        (rooms[2].rom, rooms[2].group, rooms[2].join.is_none()),
        (Rom::NoCore, Group::Unavailable, true)
    );
    assert_eq!(hashes, 2);
    let arranged = arrange(rooms);
    assert_eq!(
        arranged.iter().map(|r| r.group).collect::<Vec<_>>(),
        [Group::Online, Group::Online, Group::Unavailable]
    );
    assert_eq!(arranged[0].rom, Rom::Same);
}

#[test]
fn different_rom_missing_rom_and_lan_rooms() {
    let locals = [local(
        "snes",
        &[("retroarch", "snes9x")],
        &[(
            "/r/Street Fighter II Turbo (USA).sfc",
            "Street Fighter II Turbo",
        )],
    )];
    let entry = |game: &str, lan| Entry {
        game: game.into(),
        crc: 7,
        core: "Snes9x".into(),
        ip: "192.168.3.21".into(),
        port: 55435,
        lan,
        ..Entry::default()
    };
    let mut hash = |_: &Path| Some(9);
    let different = resolve(&entry("street_fighter_ii_turbo", true), &locals, &mut hash);
    assert_eq!(
        (different.rom, different.group, different.place.as_str()),
        (Rom::Different, Group::Lan, "192.168.3.21")
    );
    let missing = resolve(&entry("Chrono Trigger", false), &locals, &mut hash);
    assert_eq!(
        (missing.rom, missing.group),
        (Rom::NoRom, Group::Unavailable)
    );
    let mut unreadable = |_: &Path| None;
    assert_eq!(
        resolve(
            &entry("Street Fighter II Turbo", false),
            &locals,
            &mut unreadable
        )
        .rom,
        Rom::Unknown
    );
    let mut hostile = entry("Street Fighter II Turbo", false);
    hostile.ip = "1.2.3.4 --host".into();
    assert!(resolve(&hostile, &locals, &mut hash).join.is_none());
}

#[test]
fn join_line_passes_untrusted_values_only_through_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let runemu = dir.path().join("runemu.sh");
    let out = dir.path().join("args");
    fs::write(
        &runemu,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n",
            out.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&runemu, fs::Permissions::from_mode(0o755)).unwrap();
    let nick = "ev'il $(touch pwned) `id` --connect x\n";
    let line = netplay_command(
        &runemu,
        Path::new("/roms/it's $x.sfc"),
        ("snes", "snes9x", "retroarch", "pad 1"),
        ("203.0.113.9", 55435, nick, ""),
        false,
    )
    .unwrap();
    let (assignments, command) = line.split_once("; '/").unwrap();
    assert!(
        !command.contains("pwned") && !command.contains("203.0.113.9"),
        "{line}"
    );
    assert!(assignments.contains("pwned"));
    assert!(Command::new("sh")
        .arg("-c")
        .arg(&line)
        .current_dir(dir.path())
        .status()
        .unwrap()
        .success());
    assert!(!dir.path().join("pwned").exists());
    let args = fs::read_to_string(out).unwrap();
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        [
            "/roms/it's $x.sfc",
            "-Psnes",
            "--core=snes9x",
            "--emulator=retroarch",
            "--controllers=pad 1",
            "--connect",
            "203.0.113.9",
            "--port",
            "55435",
            "--nick",
            "evil touch pwned id connect x"
        ]
    );
}

#[test]
fn join_line_refuses_addresses_that_could_forge_runemu_arguments() {
    for ip in [
        "",
        "1.2.3.4 --host",
        "a--b",
        "$(id)",
        "1.2.3.4;reboot",
        "1.2.3.4-Pfoo",
        "h.example",
    ] {
        let error = netplay_command(
            Path::new("/usr/bin/runemu.sh"),
            Path::new("/r"),
            ("s", "c", "e", ""),
            (ip, 1, "n", ""),
            false,
        );
        assert!(error.is_err(), "{ip}");
    }
    assert!(netplay_command(
        Path::new("/usr/bin/runemu.sh"),
        Path::new("/r"),
        ("s", "c", "e", ""),
        ("1.2.3.4", 0, "n", ""),
        false
    )
    .is_err());
}

#[test]
fn refresh_downloads_the_lobby_and_matches_the_installed_library() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    };
    write(
        "usr/share/archr/systems.cfg",
        "<systemList><system><name>snes</name><fullname>SNES</fullname><path>/roms/snes</path>\
         <extension>.sfc</extension><emulators><emulator name=\"retroarch\"><cores>\
         <core default=\"true\">snes9x</core></cores></emulator></emulators></system></systemList>",
    );
    write("roms/snes/Super Mario World (USA).sfc", "123456789");
    let curl = write(
        "usr/bin/curl",
        "#!/bin/sh\ncase \" $* \" in *' -- http://lobby.example/list '*) ;; *) exit 9;; esac\n\
         echo '[{\"fields\":{\"username\":\"marcos\",\"game_name\":\"Super Mario World\",\"game_crc\":\"CBF43926\",\"core_name\":\"Snes9x\",\"country\":\"br\",\"ip\":\"203.0.113.9\",\"port\":55435}}]'\n",
    );
    fs::set_permissions(curl, fs::Permissions::from_mode(0o755)).unwrap();
    write("storage/.config/system/configs/system.cfg", "global.netplay=1\nglobal.netplay.lobby=http://lobby.example/list\nglobal.netplay.port=55435\n");
    let paths = archr_launcher::paths::Paths::new(Some(root.to_owned()));
    let (rooms, failed) = archr_launcher::netplay::refresh(&paths);
    assert!(!failed);
    assert_eq!(rooms.len(), 1);
    assert_eq!(
        (rooms[0].rom, rooms[0].group, rooms[0].host.as_str()),
        (Rom::Same, Group::Online, "marcos")
    );
    assert!(rooms[0].join.as_ref().unwrap().rom.starts_with(root));
    fs::remove_file(root.join("usr/bin/curl")).unwrap();
    let (rooms, failed) = archr_launcher::netplay::refresh(&paths);
    assert!(failed && rooms.is_empty());
}

#[test]
fn locked_rooms_spectate_flag_and_filters() {
    let entries = parse_lobby(LOBBY).unwrap();
    assert_eq!(
        entries.iter().map(|e| e.locked).collect::<Vec<_>>(),
        [true, false, false]
    );
    let line = |spectate| {
        netplay_command(
            Path::new("/usr/bin/runemu.sh"),
            Path::new("/r"),
            ("s", "c", "e", ""),
            ("1.2.3.4", 1, "n", ""),
            spectate,
        )
        .unwrap()
    };
    assert!(line(true).ends_with(" --nick \"$ARCHR_NP_NICK\" --spectate"));
    assert!(!line(false).contains("--spectate"));
    let relay_only = Filters {
        relay_only: true,
        show_missing: false,
    };
    assert_eq!(
        entries.iter().filter(|e| relay_only.keeps_entry(e)).count(),
        1
    );
    let lan = Entry {
        lan: true,
        ..Entry::default()
    };
    assert!(relay_only.keeps_entry(&lan));
    let locals = [local("snes", &[("retroarch", "snes9x")], &[])];
    let missing = resolve(&entries[0], &locals, &mut |_: &Path| None);
    assert!(missing.locked && missing.group == Group::Unavailable);
    assert!(!relay_only.keeps_room(&missing));
    assert!(Filters {
        relay_only: false,
        show_missing: true
    }
    .keeps_room(&missing));
}

#[test]
fn filters_read_the_settings_page_names_and_default_to_showing_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("system.cfg");
    let load = |text: &str| {
        fs::write(&path, text).unwrap();
        Filters::from_config(&archr_config::SystemCfg::load(&path).unwrap())
    };
    assert_eq!(
        load(""),
        Filters {
            relay_only: false,
            show_missing: true
        }
    );
    assert_eq!(
        load("global.netplay.relay_only=1\nglobal.netplay.show_missing=0\n"),
        Filters {
            relay_only: true,
            show_missing: false
        }
    );
    // ES's own names are not read: they live in es_settings, not here.
    assert_eq!(
        load("global.NetPlayShowOnlyRelayServerGames=true\nglobal.NetPlayShowMissingGames=false\n"),
        Filters::default()
    );
}

fn entry(game: &str, core: &str) -> Entry {
    Entry {
        game: game.into(),
        crc: 7,
        core: core.into(),
        ip: "192.0.2.5".into(),
        port: 55435,
        ..Entry::default()
    }
}

#[test]
fn cores_match_by_alphanumerics_with_aliases_and_only_under_retroarch() {
    let games = [("/r/g.rom", "Game")];
    let ra = |core: &str| local("s", &[("retroarch", core)], &games);
    let mut hash = |_: &Path| Some(7);
    for (lobby, catalog) in [
        ("Genesis Plus GX", "genesis_plus_gx"),
        ("Snes9x 2002", "snes9x2002"),
        ("MAME 2003-Plus", "mame2003_plus"),
        ("PCSX-ReARMed", "pcsx_rearmed"),
        ("VBA-M", "vba_m"),
        ("Game & Watch", "gw"),
        ("EightyOne", "81"),
        ("FinalBurn Neo", "fbneo"),
        ("Beetle NeoPop", "beetle_ngp"),
        ("Beetle WonderSwan", "beetle_wswan"),
    ] {
        let room = resolve(&entry("Game", lobby), &[ra(catalog)], &mut hash);
        assert_eq!(room.rom, Rom::Same, "{lobby} / {catalog}");
    }
    let other = ra("snes9x2010");
    assert_eq!(
        resolve(&entry("Game", "Snes9x 2002"), &[other], &mut hash).rom,
        Rom::NoCore
    );
    // A standalone emulator's core never speaks netplay.
    let standalone = local("pcengine", &[("mednafen", "beetle_pce_fast")], &games);
    assert_eq!(
        resolve(&entry("Game", "Beetle PCE Fast"), &[standalone], &mut hash).rom,
        Rom::NoCore
    );
}

#[test]
fn relay_rooms_need_a_valid_session_and_forward_it() {
    let locals = [local(
        "s",
        &[("retroarch", "snes9x")],
        &[("/r/g.sfc", "Game")],
    )];
    let relay = |session: &str| Entry {
        relay: Some(("198.51.100.7".into(), 55436, session.into())),
        ..entry("Game", "Snes9x")
    };
    let mut hash = |_: &Path| Some(7);
    let ok = resolve(&relay("a1B2"), &locals, &mut hash);
    assert_eq!(ok.join.as_ref().unwrap().session, "a1B2");
    for bad in ["", "a b", "x;y", "--bad"] {
        assert!(
            resolve(&relay(bad), &locals, &mut hash).join.is_none(),
            "{bad:?}"
        );
    }
    let mut hostname = entry("Game", "Snes9x");
    hostname.ip = "evil.example".into();
    assert!(resolve(&hostname, &locals, &mut hash).join.is_none());
    let line = netplay_command(
        Path::new("/usr/bin/runemu.sh"),
        Path::new("/r"),
        ("s", "c", "e", ""),
        ("198.51.100.7", 1, "n", "a1B2"),
        true,
    )
    .unwrap();
    assert!(
        line.ends_with("--mitm-session \"$ARCHR_NP_SESSION\" --spectate"),
        "{line}"
    );
    assert!(netplay_command(
        Path::new("/usr/bin/runemu.sh"),
        Path::new("/r"),
        ("s", "c", "e", ""),
        ("::1", 1, "n", "a b"),
        false
    )
    .is_err());
    assert!(netplay_command(
        Path::new("/usr/bin/runemu.sh"),
        Path::new("/r"),
        ("s", "c", "e", ""),
        ("::1", 1, "n", ""),
        false
    )
    .is_ok());
}

#[test]
fn nickname_is_restricted_to_safe_characters_and_32_chars() {
    let dir = tempfile::tempdir().unwrap();
    let runemu = dir.path().join("runemu.sh");
    let out = dir.path().join("args");
    fs::write(
        &runemu,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n",
            out.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&runemu, fs::Permissions::from_mode(0o755)).unwrap();
    let nick = format!("Ash-Pikachu_x.y -P{}", "z".repeat(40));
    let line = netplay_command(
        &runemu,
        Path::new("/r"),
        ("s", "c", "e", ""),
        ("1.2.3.4", 1, &nick, ""),
        false,
    )
    .unwrap();
    assert!(Command::new("sh")
        .arg("-c")
        .arg(&line)
        .status()
        .unwrap()
        .success());
    let args = fs::read_to_string(out).unwrap();
    let sent = args.lines().last().unwrap();
    assert_eq!(sent.chars().count(), 32);
    assert!(sent.starts_with("AshPikachu_x.y Pzzz") && !sent.contains('-'));
}

fn fixture(
    roms: &[(&str, Vec<u8>)],
    lobby: &str,
) -> (tempfile::TempDir, archr_launcher::paths::Paths) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let write = |path: &str, data: &[u8]| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    };
    write(
        "usr/share/archr/systems.cfg",
        b"<systemList><system><name>snes</name><fullname>SNES</fullname><path>/roms/snes</path><extension>.sfc .zip</extension><emulators><emulator name=\"retroarch\"><cores><core default=\"true\">snes9x</core></cores></emulator></emulators></system></systemList>",
    );
    for (name, data) in roms {
        write(&format!("roms/snes/{name}"), data);
    }
    let curl = write(
        "usr/bin/curl",
        format!("#!/bin/sh\necho x >> \"$(dirname \"$0\")/calls\"\nsleep 1\ncat <<'EOF'\n{lobby}\nEOF\n").as_bytes(),
    );
    fs::set_permissions(curl, fs::Permissions::from_mode(0o755)).unwrap();
    let paths = archr_launcher::paths::Paths::new(Some(root.to_owned()));
    (dir, paths)
}

fn room_json(game: &str, crc: &str) -> String {
    format!(
        r#"[{{"fields":{{"username":"m","game_name":"{game}","game_crc":"{crc}","core_name":"Snes9x","country":"br","ip":"203.0.113.9","port":55435}}}}]"#
    )
}

/// One stored zip entry, as the central directory sees it.
fn zip_with_crc(crc: u32) -> Vec<u8> {
    let mut z = Vec::new();
    z.extend(b"PK\x03\x04");
    z.extend([0; 26]);
    let offset = z.len() as u32;
    z.extend(b"PK\x01\x02");
    z.extend([0; 12]);
    z.extend(crc.to_le_bytes());
    z.extend([0; 26]);
    let size = z.len() as u32 - offset;
    z.extend(b"PK\x05\x06");
    z.extend([0; 8]);
    z.extend(size.to_le_bytes());
    z.extend(offset.to_le_bytes());
    z.extend([0; 2]);
    z
}

#[test]
fn archives_are_unknown_unless_the_first_entry_crc_matches() {
    let (_dir, paths) = fixture(
        &[("Game.zip", zip_with_crc(0xCBF4_3926))],
        &room_json("Game", "CBF43926"),
    );
    let (rooms, _) = archr_launcher::netplay::refresh(&paths);
    assert_eq!(rooms[0].rom, Rom::Same);
    let (_dir, paths) = fixture(
        &[("Game.zip", zip_with_crc(0x1234_5678))],
        &room_json("Game", "CBF43926"),
    );
    let (rooms, _) = archr_launcher::netplay::refresh(&paths);
    assert_eq!(rooms[0].rom, Rom::Unknown);
    let (_dir, paths) = fixture(
        &[("Game.zip", b"not a zip".to_vec())],
        &room_json("Game", "CBF43926"),
    );
    assert_eq!(
        archr_launcher::netplay::refresh(&paths).0[0].rom,
        Rom::Unknown
    );
}

#[test]
fn hashes_are_cached_by_size_and_mtime_across_refreshes() {
    let (dir, paths) = fixture(
        &[("Game.sfc", b"123456789".to_vec())],
        &room_json("Game", "CBF43926"),
    );
    let mut cache = archr_launcher::netplay::HashCache::new();
    assert_eq!(
        archr_launcher::netplay::refresh_with(&paths, &mut cache).0[0].rom,
        Rom::Same
    );
    let key = cache.keys().next().unwrap().clone();
    // A poisoned entry with the file's real size and mtime is trusted...
    let meta = fs::metadata(&key).unwrap();
    cache.get_mut(&key).unwrap().2 = Some(1);
    assert_eq!(
        archr_launcher::netplay::refresh_with(&paths, &mut cache).0[0].rom,
        Rom::Different
    );
    // ...and dropped as soon as the file changes.
    fs::write(&key, b"123456789").unwrap();
    let new_mtime = meta.modified().unwrap() + std::time::Duration::from_secs(5);
    fs::File::options()
        .write(true)
        .open(&key)
        .unwrap()
        .set_modified(new_mtime)
        .unwrap();
    assert_eq!(
        archr_launcher::netplay::refresh_with(&paths, &mut cache).0[0].rom,
        Rom::Same
    );
    drop(dir);
}

#[test]
fn a_running_refresh_is_reused_when_the_lobby_is_reopened() {
    let (dir, paths) = fixture(&[], &room_json("Game", "CBF43926"));
    let mut runtime = archr_launcher::netplay::Runtime::new(paths);
    let mut lobby = archr_ui::netplay::Lobby::default();
    runtime.refresh();
    runtime.refresh();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !runtime.poll(&mut lobby) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("usr/bin/calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_eq!(lobby.rooms.len(), 1);
}
