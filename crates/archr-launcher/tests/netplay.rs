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
    Local {
        name: system.into(),
        cores: cores
            .iter()
            .map(|(e, c)| ((*e).into(), (*c).into()))
            .collect(),
        games: games
            .iter()
            .map(|(p, n)| (PathBuf::from(p), (*n).into()))
            .collect(),
    }
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
    assert_eq!(rooms[1].relay, Some(("198.51.100.7".into(), 55436)));
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
            &[("retroarch", "genesisplusgx")],
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
        ("203.0.113.9", 55435, nick),
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
            "ev'il $(touch pwned) `id` -connect x"
        ]
    );
}

#[test]
fn join_line_refuses_addresses_that_could_forge_runemu_arguments() {
    for ip in ["", "1.2.3.4 --host", "a--b", "$(id)", "1.2.3.4;reboot"] {
        let error = netplay_command(
            Path::new("/usr/bin/runemu.sh"),
            Path::new("/r"),
            ("s", "c", "e", ""),
            (ip, 1, "n"),
            false,
        );
        assert!(error.is_err(), "{ip}");
    }
    assert!(netplay_command(
        Path::new("/usr/bin/runemu.sh"),
        Path::new("/r"),
        ("s", "c", "e", ""),
        ("h.example", 0, "n"),
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
            ("h.example", 1, "n"),
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
fn filters_read_both_setting_names_and_default_to_showing_missing() {
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
    assert_eq!(
        load("global.NetPlayShowOnlyRelayServerGames=true\nglobal.NetPlayShowMissingGames=false\n"),
        Filters {
            relay_only: true,
            show_missing: false
        }
    );
}
