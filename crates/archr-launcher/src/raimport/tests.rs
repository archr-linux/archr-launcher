use super::*;
use std::os::unix::fs::symlink;

struct Fixture {
    _dir: tempfile::TempDir,
    roots: Roots,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let at = |p: &str| dir.path().join(p);
    let roots = Roots {
        source: at("import"),
        retroarch: at("retroarch"),
        remaps: at("remappings"),
        shaders: at("shaders"),
        cores: at("cores"),
    };
    for path in [
        &roots.source,
        &roots.retroarch,
        &roots.shaders,
        &roots.cores,
    ] {
        fs::create_dir_all(path).unwrap();
    }
    // mGBA is installed; Snes9x only has its info file.
    put(&roots.cores, concat!("mgba", "_libretro.so"), "");
    put(
        &roots.cores,
        concat!("mgba", "_libretro.info"),
        "corename = \"mGBA\"\n",
    );
    put(
        &roots.cores,
        "snes9x_libretro.info",
        "corename = \"Snes9x\"\n",
    );
    put(&roots.shaders, "crt/zfast.slang", "// shader");
    put(
        &roots.retroarch,
        "retroarch.cfg",
        "video_driver = \"gl\"\nvideo_scale_integer = \"false\"\nmenu_driver = \"ozone\"\n",
    );
    Fixture { _dir: dir, roots }
}
fn put(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn why<'a>(plan: &'a Plan, item: &str) -> Option<&'a str> {
    plan.found
        .ignored
        .iter()
        .find(|(name, _)| name == item)
        .map(|(_, why)| why.as_str())
}

#[test]
fn retroarch_cfg_takes_only_allowed_keys() {
    let f = fixture();
    put(
        &f.roots.source,
        "retroarch.cfg",
        "# comment\n\
         video_scale_integer = \"true\"\n\
         video_scale_integer_overscale = \"true\"\n\
         aspect_ratio_index = \"23\"\n\
         custom_viewport_y = \"12\"\n\
         input_menu_toggle_btn = \"2\"\n\
         input_menu_toggle_gamepad_combo = \"2\"\n\
         input_overlay_opacity = \"0.7\"\n\
         audio_latency = \"64\"\n\
         video_rotation = \"1\"\n\
         video_frame_delay = \"4\"\n\
         video_font_size = \"30\"\n\
         config_save_on_exit = \"true\"\n\
         video_driver = \"vulkan\"\n\
         libretro_directory = \"~/cores\"\n\
         video_threaded = \"true\"\n\
         video_fullscreen_x = \"1920\"\n\
         cheevos_password = \"secret\"\n\
         input_player1_joypad_index = \"3\"\n\
         input_player1_a_btn = \"1\"\n\
         menu_wallpaper = \"/home/me/bg.png\"\n\
         audio_out_rate = \"48000\"\n\
         #include \"other.cfg\"\n",
    );
    let plan = scan(&f.roots);
    let keys: Vec<&str> = plan.settings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        [
            "aspect_ratio_index",
            "audio_latency",
            "input_menu_toggle_gamepad_combo",
            "input_overlay_opacity",
            "video_scale_integer",
            "video_scale_integer_overscale",
        ]
    );
    for key in [
        "video_driver",
        "libretro_directory",
        "video_threaded",
        "video_fullscreen_x",
        "cheevos_password",
        "input_player1_joypad_index",
        "input_player1_a_btn",
        "menu_wallpaper",
    ] {
        assert_eq!(why(&plan, key), Some(BLOCKED), "{key}");
    }
    assert_eq!(why(&plan, "audio_out_rate"), Some(UNKNOWN));
    // Button numbers belong to the other device's pad.
    assert_eq!(why(&plan, "input_menu_toggle_btn"), Some(UNKNOWN));
    for key in [
        "custom_viewport_y",
        "video_rotation",
        "video_frame_delay",
        "video_font_size",
        "config_save_on_exit",
    ] {
        assert_eq!(why(&plan, key), Some(UNKNOWN), "{key}");
    }
    assert_eq!(why(&plan, "retroarch.cfg: #include"), Some(INCLUDE));
    assert_eq!((plan.found.settings, plan.found.skipped), (6, 16));
}

#[test]
fn files_must_be_regular_bounded_utf8() {
    let f = fixture();
    let outside = f.roots.retroarch.join("retroarch.cfg");
    symlink(&outside, f.roots.source.join("retroarch.cfg")).unwrap();
    assert_eq!(why(&scan(&f.roots), "retroarch.cfg"), Some(BAD_FILE));
    fs::remove_file(f.roots.source.join("retroarch.cfg")).unwrap();
    fs::write(f.roots.source.join("retroarch.cfg"), [0xff, 0xfe, b'\n']).unwrap();
    assert_eq!(why(&scan(&f.roots), "retroarch.cfg"), Some(BAD_FILE));
    let big = "video_smooth = \"true\"\n".repeat(60_000);
    fs::write(f.roots.source.join("retroarch.cfg"), big).unwrap();
    let plan = scan(&f.roots);
    assert_eq!(why(&plan, "retroarch.cfg"), Some(BAD_FILE));
    assert!(plan.settings.is_empty());
}

#[test]
fn overrides_need_an_installed_core_and_keep_only_safe_keys() {
    let f = fixture();
    put(
        &f.roots.source,
        "config/mGBA/mGBA.cfg",
        "video_scale_integer = \"true\"\nvideo_driver = \"gl\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.opt",
        "mgba_skip_bios = \"ON\"\nmgba_frameskip = \"0\"\n",
    );
    put(&f.roots.source, "config/mGBA/notes.txt", "hi");
    put(
        &f.roots.source,
        "config/Snes9x/Snes9x.cfg",
        "video_smooth = \"true\"\n",
    );
    symlink(
        f.roots.retroarch.join("retroarch.cfg"),
        f.roots.source.join("config/mGBA/Game.cfg"),
    )
    .unwrap();
    put(
        &f.roots.source,
        "remaps/mGBA/mGBA.rmp",
        "input_remap_port_p1 = \"0\"\ninput_player1_btn_a = \"8\"\ninput_player1_joypad_index = \"2\"\n",
    );
    let plan = scan(&f.roots);
    assert_eq!(plan.found.cores, ["mGBA"]);
    assert_eq!((plan.found.overrides, plan.found.remaps), (2, 1));
    assert_eq!(why(&plan, "config/Snes9x"), Some(NO_CORE));
    assert_eq!(why(&plan, "config/mGBA/notes.txt"), Some(UNKNOWN));
    assert_eq!(why(&plan, "config/mGBA/Game.cfg"), Some(BAD_FILE));
    let file = |end: &str| {
        plan.files
            .iter()
            .find(|(path, _)| path.ends_with(end))
            .map(|(_, text)| text.as_str())
            .unwrap()
    };
    assert_eq!(
        file("config/mGBA/mGBA.cfg"),
        "video_scale_integer = \"true\"\n"
    );
    assert!(file("config/mGBA/mGBA.opt").contains("mgba_skip_bios = \"ON\""));
    assert_eq!(
        file("remappings/mGBA/mGBA.rmp"),
        "input_player1_btn_a = \"8\"\ninput_remap_port_p1 = \"0\"\n"
    );
}

#[test]
fn shader_presets_point_at_this_device() {
    let f = fixture();
    put(
        &f.roots.source,
        "shaders/presets/mGBA/Game.slangp",
        "shaders = \"1\"\n\
         shader0 = \"C:\\RetroArch\\shaders\\shaders_slang\\crt\\zfast.slang\"\n\
         filter_linear0 = \"false\"\n",
    );
    put(
        &f.roots.source,
        "shaders/Mine.slangp",
        "#reference \"presets/mGBA/Game.slangp\"\n",
    );
    put(
        &f.roots.source,
        "shaders/missing.glslp",
        "shaders = \"1\"\nshader0 = \"/home/me/shaders/shaders_glsl/nope/x.glsl\"\n",
    );
    put(
        &f.roots.source,
        "shaders/escape.glslp",
        "shaders = \"1\"\nshader0 = \"../../../etc/passwd\"\n",
    );
    let plan = scan(&f.roots);
    let zfast = f.roots.shaders.join("crt/zfast.slang");
    let game = plan
        .files
        .iter()
        .find(|(path, _)| path.ends_with("presets/mGBA/Game.slangp"))
        .unwrap();
    assert_eq!(game.0, f.roots.shaders.join("presets/mGBA/Game.slangp"));
    assert!(game
        .1
        .contains(&format!("shader0 = \"{}\"", zfast.display())));
    assert!(game.1.contains("filter_linear0 = \"false\""));
    let mine = plan
        .files
        .iter()
        .find(|(p, _)| p.ends_with("Mine.slangp"))
        .unwrap();
    assert_eq!(
        mine.1,
        format!(
            "#reference \"{}\"\n",
            f.roots.shaders.join("presets/mGBA/Game.slangp").display()
        )
    );
    assert_eq!(plan.found.presets, 2);
    assert_eq!(why(&plan, "shaders/missing.glslp"), Some(NO_SHADER));
    assert_eq!(why(&plan, "shaders/escape.glslp"), Some(NO_SHADER));
}

#[test]
fn import_merges_retroarch_cfg_and_writes_files() {
    let f = fixture();
    put(
        &f.roots.source,
        "retroarch.cfg",
        "video_scale_integer = \"true\"\ninput_menu_toggle_gamepad_combo = \"2\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.opt",
        "mgba_skip_bios = \"ON\"\n",
    );
    let plan = scan(&f.roots);
    assert_eq!(apply(&plan, &f.roots), Ok(3));
    assert_eq!(
        fs::read_to_string(f.roots.retroarch.join("retroarch.cfg")).unwrap(),
        "video_driver = \"gl\"\nvideo_scale_integer = \"true\"\nmenu_driver = \"ozone\"\n\
         input_menu_toggle_gamepad_combo = \"2\"\n"
    );
    assert!(f.roots.retroarch.join("config/mGBA/mGBA.opt").is_file());
}

#[test]
fn a_failed_write_puts_everything_back() {
    let f = fixture();
    put(
        &f.roots.source,
        "retroarch.cfg",
        "video_smooth = \"true\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.opt",
        "mgba_skip_bios = \"ON\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.cfg",
        "video_smooth = \"true\"\n",
    );
    put(
        &f.roots.retroarch,
        "config/mGBA/mGBA.cfg",
        "video_smooth = \"false\"\n",
    );
    // A directory where the core options go: that write fails.
    fs::create_dir_all(f.roots.retroarch.join("config/mGBA/mGBA.opt")).unwrap();
    let before = fs::read_to_string(f.roots.retroarch.join("retroarch.cfg")).unwrap();
    let plan = scan(&f.roots);
    assert_eq!(apply(&plan, &f.roots), Err("raimport.failed".into()));
    assert_eq!(
        fs::read_to_string(f.roots.retroarch.join("config/mGBA/mGBA.cfg")).unwrap(),
        "video_smooth = \"false\"\n"
    );
    assert_eq!(
        fs::read_to_string(f.roots.retroarch.join("retroarch.cfg")).unwrap(),
        before
    );
}

#[test]
fn an_empty_or_missing_folder_finds_nothing() {
    let f = fixture();
    fs::remove_dir(&f.roots.source).unwrap();
    let plan = scan(&f.roots);
    assert!(plan.found.apply.is_empty() && plan.found.ignored.is_empty());
    assert!(plan.files.is_empty() && plan.settings.is_empty());
}

#[test]
fn an_unreadable_destination_stops_before_any_write() {
    let f = fixture();
    put(
        &f.roots.source,
        "retroarch.cfg",
        "video_smooth = \"true\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.opt",
        "mgba_skip_bios = \"ON\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.cfg",
        "video_smooth = \"true\"\n",
    );
    // Not UTF-8 is fine to keep; a symbolic link is not a file to replace.
    fs::create_dir_all(f.roots.retroarch.join("config/mGBA")).unwrap();
    fs::write(f.roots.retroarch.join("config/mGBA/mGBA.cfg"), [0xff, 0x00]).unwrap();
    symlink(
        f.roots.retroarch.join("retroarch.cfg"),
        f.roots.retroarch.join("config/mGBA/mGBA.opt"),
    )
    .unwrap();
    let before = fs::read(f.roots.retroarch.join("retroarch.cfg")).unwrap();
    let plan = scan(&f.roots);
    assert_eq!(apply(&plan, &f.roots), Err("raimport.failed".into()));
    assert_eq!(
        fs::read(f.roots.retroarch.join("retroarch.cfg")).unwrap(),
        before
    );
    assert_eq!(
        fs::read(f.roots.retroarch.join("config/mGBA/mGBA.cfg")).unwrap(),
        [0xff, 0x00]
    );
}

#[test]
fn undo_restores_bytes_and_never_removes_a_file_that_existed() {
    let f = fixture();
    put(
        &f.roots.source,
        "config/mGBA/mGBA.cfg",
        "video_smooth = \"true\"\n",
    );
    put(
        &f.roots.source,
        "config/mGBA/mGBA.opt",
        "mgba_skip_bios = \"ON\"\n",
    );
    put(
        &f.roots.source,
        "remaps/mGBA/mGBA.rmp",
        "input_remap_port_p1 = \"0\"\n",
    );
    fs::create_dir_all(f.roots.retroarch.join("config/mGBA")).unwrap();
    // Existing, not UTF-8: it must come back byte for byte.
    fs::write(f.roots.retroarch.join("config/mGBA/mGBA.cfg"), [0xff, b'x']).unwrap();
    let plan = scan(&f.roots);
    // The last write fails: a file stands where the remap folder goes.
    fs::write(&f.roots.remaps, "").unwrap();
    assert_eq!(apply(&plan, &f.roots), Err("raimport.failed".into()));
    assert_eq!(
        fs::read(f.roots.retroarch.join("config/mGBA/mGBA.cfg")).unwrap(),
        [0xff, b'x']
    );
    assert!(!f.roots.retroarch.join("config/mGBA/mGBA.opt").exists());
}

#[test]
fn long_lists_end_in_one_plus_line() {
    let f = fixture();
    let text: String = (0..300).map(|n| format!("unknown_{n} = \"1\"\n")).collect();
    put(&f.roots.source, "retroarch.cfg", &text);
    let plan = scan(&f.roots);
    assert_eq!(plan.found.skipped, 300);
    assert_eq!(plan.found.ignored.len(), 200);
    assert_eq!(
        plan.found.ignored[199],
        ("+101".into(), "raimport.more|count=101".into())
    );
}
