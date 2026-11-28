//! Small ES themes written here, converted, and what comes out read back:
//! the TOML the launcher gets and its reading without warnings.
use std::path::Path;
use toml::{Table, Value};

fn theme(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        let path = dir.path().join("es").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}
fn png(path: &Path, (w, h): (u32, u32)) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 30, 255]))
        .save(path)
        .unwrap();
}
fn convert(dir: &Path) -> archr_es2theme::Conversion {
    archr_es2theme::convert(&dir.join("es"), &dir.join("out"), &Default::default()).unwrap()
}
/// Every TOML file of the output, merged as `include` merges them.
fn read(dir: &Path) -> Table {
    let out = dir.join("out");
    let mut all: Table =
        toml::from_str(&std::fs::read_to_string(out.join("theme.toml")).unwrap()).unwrap();
    let parts: Vec<String> = all["include"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    for part in parts {
        let t: Table = toml::from_str(&std::fs::read_to_string(out.join(part)).unwrap()).unwrap();
        merge(&mut all, t);
    }
    all
}
fn merge(base: &mut Table, over: Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}
fn element<'a>(t: &'a Table, view: &str, screen: &str, id: &str) -> &'a Table {
    t["view"][view][screen]["el"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_table().unwrap())
        .find(|e| e["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("no element {id} in {view}.{screen}"))
}

#[test]
fn elements_measures_and_data_become_free_screen_elements() {
    let dir = theme(&[(
        "theme.xml",
        r#"<theme><formatVersion>7</formatVersion>
  <variables><accent>F9C440</accent><big>0.05</big></variables>
  <view name="system">
    <text name="systemName" extra="true">
      <pos>0.34 0.05</pos><size>0.6 ${big}</size><text>${system.fullName}</text>
      <fontSize>${big}</fontSize><rotation>90</rotation><rotationOrigin>0 0.5</rotationOrigin>
      <forceUppercase>1</forceUppercase><zIndex>10</zIndex>
      <storyboard><animation property="x" from="0.32" to="0.34" duration="300"/>
      <animation property="opacity" from="0" to="1" duration="300"/></storyboard>
    </text>
    <image name="arte" extra="true"><path>./arts/default.png</path><path>./arts/${system.theme}.png</path>
      <pos>0 0</pos><size>1 1</size><zIndex>-5</zIndex></image>
  </view>
  <view name="detailed">
    <textlist name="gamelist"><pos>0 0.15</pos><size>0.43 0.775</size>
      <selectorColor>${accent}</selectorColor><primaryColor>FFFFFF</primaryColor><fontSize>0.04</fontSize></textlist>
    <image name="md_image"><pos>0.7 0.4</pos><maxSize>0.5 0.5</maxSize><roundCorners>0.02</roundCorners></image>
    <text name="md_description"><pos>0.47 0.77</pos><size>0.5 0.2</size></text>
  </view>
</theme>"#,
    )]);
    png(&dir.path().join("es/arts/default.png"), (64, 48));
    png(&dir.path().join("es/arts/snes.png"), (64, 48));
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    let name = element(&t, "detailed", "systems", "systemname");
    assert_eq!(name["text"].as_str(), Some("${system.name}"));
    assert_eq!(name["rotate"].as_float(), Some(90.));
    assert_eq!(name["pivot"].as_array().unwrap()[1].as_float(), Some(0.5));
    assert_eq!(name["size"].as_float(), Some(0.05));
    assert_eq!(name["z"].as_integer(), Some(40));
    assert_eq!(name["upper"].as_bool(), Some(true));
    assert_eq!(name["enter"]["from"].as_str(), Some("left"));
    // The last path ES finds first.
    let art = element(&t, "detailed", "systems", "arte");
    let src: Vec<&str> = art["src"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(src, ["arts/${system}.png", "arts/default.png"]);
    assert_eq!(art["fit"].as_str(), Some("stretch"));
    // The game's picture centered on its pos, as ES does.
    let shot = element(&t, "detailed", "games", "md-image");
    assert_eq!(shot["anchor"].as_str(), Some("center"));
    assert_eq!(
        shot["src"].as_array().unwrap()[0].as_str(),
        Some("game.screenshot")
    );
    assert_eq!(shot["radius"].as_float(), Some(0.02));
    let list = element(&t, "detailed", "games", "gamelist");
    assert_eq!(list["kind"].as_str(), Some("list"));
    assert_eq!(list["selected"]["fill"].as_str(), Some("#F9C440FF"));
    let text = element(&t, "detailed", "games", "md-description");
    assert_eq!(text["wrap"].as_bool(), Some(true));
    // The details: the list's place for the cover, facts and buttons.
    assert_eq!(
        t["view"]["detailed"]["details"]["remove"][0].as_str(),
        Some("gamelist")
    );
    // The pictures came along, at the same paths.
    assert!(dir.path().join("out/arts/snes.png").is_file());
    assert!(std::fs::read_to_string(dir.path().join("out/CONVERSAO.md"))
        .unwrap()
        .contains("Nenhum: o tema carrega sem avisos"));
}

#[test]
fn subsets_become_options_and_the_ratio_shapes() {
    let dir = theme(&[
        (
            "theme.xml",
            r#"<theme><formatVersion>7</formatVersion>
  <subset name="colorScheme" displayName="Accent">
    <include name="blue" displayName="Blue">./blue.xml</include>
    <include name="red" displayName="Red">./red.xml</include>
  </subset>
  <subset name="Ratio" displayName="Ratio">
    <include name="4-3" displayName="4:3"/><include name="16-9" displayName="16:9"/>
  </subset>
  <subset name="Unused" displayName="Unused"><include name="a"/><include name="b"/></subset>
  <view name="system">
    <text name="label" extra="true"><pos>0.1 0.1</pos><size>0.5 0.05</size><text>OI</text>
      <color>${accent}</color><fontSize>0.04</fontSize><fontSize ifSubset="Ratio:16-9">0.06</fontSize></text>
  </view>
</theme>"#,
        ),
        (
            "blue.xml",
            "<theme><variables><accent>3060C0</accent></variables></theme>",
        ),
        (
            "red.xml",
            "<theme><variables><accent>C03030</accent></variables></theme>",
        ),
    ]);
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    let option = &t["options"]["colorscheme"];
    assert_eq!(option["default"].as_str(), Some("blue"));
    // The ES variable's own name.
    assert_eq!(
        option["choices"]["red"]["vars"]["accent"].as_str(),
        Some("#C03030FF")
    );
    assert_eq!(t["vars"]["accent"].as_str(), Some("#3060C0FF"));
    // The subset that changes nothing is left out; the ratio is no option.
    assert!(t["options"].get("unused").is_none() && t["options"].get("ratio").is_none());
    // No game list: one view with the systems screen; the 16:9 size by shape.
    let label = element(&t, "es", "systems", "label");
    let size = label["size"].as_str().unwrap().trim_start_matches('$');
    assert_eq!(t["vars"][size].as_float(), Some(0.04));
    assert_eq!(t["aspect"]["wide"]["vars"][size].as_float(), Some(0.06));
    assert!(t["view"]["es"].get("games").is_none());
}

#[test]
fn the_launcher_reads_a_bad_theme_with_warnings_the_check_catches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("theme.toml"),
        "[view.x]\nname = \"X\"\n[[view.x.systems.el]]\nkind = \"nada\"\n",
    )
    .unwrap();
    assert!(!archr_es2theme::check(dir.path()).is_empty());
}

/// Albedo (MIT), when its folder is at hand: converted, it reads without
/// a warning. `ARCHR_ES_ALBEDO` points at it.
#[test]
fn albedo_converts_without_warnings() {
    let Some(src) = std::env::var_os("ARCHR_ES_ALBEDO").map(std::path::PathBuf::from) else {
        eprintln!("ARCHR_ES_ALBEDO não definido; teste pulado");
        return;
    };
    let out = tempfile::tempdir().unwrap();
    let opts = archr_es2theme::Options {
        lang: "pt".into(),
        name: Some("Albedo".into()),
    };
    let c = archr_es2theme::convert(&src, out.path(), &opts).unwrap();
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    assert_eq!(c.views.len(), 4);
    assert!(c.options.iter().any(|o| o == "colorscheme"));
    assert!(out.path().join("original/LICENSE").is_file());
}

fn convert_pt(dir: &Path) -> archr_es2theme::Conversion {
    let opts = archr_es2theme::Options {
        lang: "pt".into(),
        name: None,
    };
    archr_es2theme::convert(&dir.join("es"), &dir.join("out"), &opts).unwrap()
}
const CAPABILITIES: &str = r#"<themeCapabilities>
  <aspectRatio>16:9</aspectRatio><aspectRatio>4:3</aspectRatio>
  <fontSize>small</fontSize><fontSize>medium</fontSize>
  <variant name="lista"><label>Lista</label></variant>
</themeCapabilities>"#;
const GAMELIST: &str = r#"<view name="gamelist">
    <textlist name="gamelist"><pos>0 0.15</pos><size>0.45 0.7</size><fontSize>0.04</fontSize></textlist>
  </view>"#;

/// ES-DE reads a file's variables before its includes, and each system's
/// own file (`${system.theme}.xml`) after the defaults: what a system
/// changes becomes a system variable, in the conversion's language.
#[test]
fn esde_system_files_become_system_vars() {
    let dir = theme(&[
        ("capabilities.xml", CAPABILITIES),
        (
            "theme.xml",
            &format!(
                r#"<theme>
  <include>${{root}}/meta/_default.xml</include>
  <include>${{root}}/meta/${{system.theme}}.xml</include>
  <variables><root>.</root></variables>
  <variant name="lista">
  <view name="system">
    <text name="descricao"><pos>0.05 0.6</pos><size>0.5 0.2</size><fontSize>0.03</fontSize>
      <text>${{systemDescription}}</text></text>
    <image name="faixa"><pos>0 0</pos><size>1 0.1</size><path>./faixa.png</path>
      <color>${{systemColor}}</color></image>
    <text name="ano"><pos>0.05 0.85</pos><size>0.3 0.05</size><fontSize>0.03</fontSize>
      <text>${{systemReleaseYear}}</text></text>
  </view>
  {GAMELIST}
  </variant>
</theme>"#
            ),
        ),
        (
            "meta/_default.xml",
            "<theme><variables><systemDescription>Jogue os jogos de ${system.fullName}</systemDescription>\
             <systemColor>333333</systemColor><systemReleaseYear></systemReleaseYear></variables></theme>",
        ),
        (
            "meta/snes.xml",
            r#"<theme><variables><systemDescription>A 16-bit console.</systemDescription>
  <systemColor>DF5142</systemColor><systemReleaseYear>1990</systemReleaseYear></variables>
  <language name="pt_BR"><variables><systemDescription>Um console de 16 bits.</systemDescription></variables></language>
  <language name="pt_PT"><variables><systemDescription>Uma consola.</systemDescription></variables></language>
</theme>"#,
        ),
        (
            "meta/genesis.xml",
            "<theme><variables><systemColor>2040C0</systemColor><systemReleaseYear>1988</systemReleaseYear></variables></theme>",
        ),
    ]);
    png(&dir.path().join("es/faixa.png"), (64, 8));
    let c = convert_pt(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    assert_eq!(t["min_launcher"].as_str(), Some("0.6.0"));
    let view = "lista-gamelist";
    let var = |e: &Table, key: &str| e[key].as_str().unwrap().trim_start_matches('$').to_owned();
    let text = element(&t, view, "systems", "descricao");
    let text_var = var(text, "text");
    assert_eq!(
        t["vars"][&text_var].as_str(),
        Some("Jogue os jogos de ${system.name}")
    );
    let snes = &t["system"]["snes"]["vars"];
    assert_eq!(snes[&text_var].as_str(), Some("Um console de 16 bits."));
    let tint = var(element(&t, view, "systems", "faixa"), "tint");
    assert_eq!(t["vars"][&tint].as_str(), Some("#333333FF"));
    assert_eq!(snes[&tint].as_str(), Some("#DF5142FF"));
    assert_eq!(
        t["system"]["genesis"]["vars"][&tint].as_str(),
        Some("#2040C0FF")
    );
    // The year the defaults leave empty: there only for the systems with one.
    let year = element(&t, view, "systems", "ano");
    let shown = var(year, "opacity");
    assert_eq!(t["vars"][&shown].as_float(), Some(0.));
    assert_eq!(snes[&shown].as_float(), Some(1.));
    assert!(
        !std::fs::read_to_string(dir.path().join("out/CONVERSAO.md"))
            .unwrap()
            .contains("include que não existe")
    );
}

/// ES-DE's names shown only on some systems: the systems', the automatic
/// collections' (favorites, recent) and the user's own (none here).
#[test]
fn esde_collection_names_show_by_when_collection() {
    let dir = theme(&[
        ("capabilities.xml", CAPABILITIES),
        (
            "theme.xml",
            &format!(
                r#"<theme><variant name="lista">
  <view name="system">
    <text name="sistema"><pos>0.1 0.1</pos><size>0.5 0.05</size><text>${{system.fullName.noCollections}}</text></text>
    <text name="auto"><pos>0.1 0.2</pos><size>0.5 0.05</size><text>${{system.fullName.autoCollections}}</text></text>
    <text name="propria"><pos>0.1 0.3</pos><size>0.5 0.05</size><text>${{system.fullName.customCollections}}</text></text>
  </view>
  {GAMELIST}
</variant></theme>"#
            ),
        ),
    ]);
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    let view = "lista-gamelist";
    let sistema = element(&t, view, "systems", "sistema");
    assert_eq!(sistema["text"].as_str(), Some("${system.name}"));
    assert_eq!(sistema["when"]["collection"].as_bool(), Some(false));
    let auto = element(&t, view, "systems", "auto");
    assert_eq!(auto["when"]["collection"].as_bool(), Some(true));
    let els = t["view"][view]["systems"]["el"].as_array().unwrap();
    assert!(!els.iter().any(|e| e["id"].as_str() == Some("propria")));
    assert_eq!(t["min_launcher"].as_str(), Some("0.6.0"));
}

/// ES-DE's font sizes are a theme option, "medium" first; a screen keeps
/// one clip (the game's), the background clip stands still and the
/// background music goes; the clock and battery are one status pill; a
/// GIF is its first frame. Nothing new for the launcher: 0.5.0.
#[test]
fn esde_font_sizes_clips_status_and_gifs() {
    let dir = theme(&[
        ("capabilities.xml", CAPABILITIES),
        (
            "theme.xml",
            r#"<theme>
  <fontSize name="small"><variables><tamanho>0.03</tamanho></variables></fontSize>
  <fontSize name="medium"><variables><tamanho>0.05</tamanho></variables></fontSize>
  <variant name="lista">
  <view name="system">
    <text name="rotulo"><pos>0.1 0.1</pos><size>0.5 0.08</size><fontSize>${tamanho}</fontSize>
      <text>OI</text><horizontalAlignment>right</horizontalAlignment></text>
    <clock name="relogio"><pos>0.98 0.02</pos><origin>1 0</origin><fontSize>0.03</fontSize></clock>
    <systemstatus name="status"><pos>0.8 0.02</pos><origin>1 0</origin><height>0.03</height></systemstatus>
    <animation name="anim"><pos>0 0</pos><size>0.2 0.2</size><path>./anim.gif</path></animation>
  </view>
  <view name="gamelist">
    <textlist name="gamelist"><pos>0 0.15</pos><size>0.45 0.7</size><fontSize>0.04</fontSize></textlist>
    <video name="fundo"><pos>0 0</pos><size>1 1</size><path>./fundo.mp4</path><audio>false</audio></video>
    <video name="musica"><pos>0 0</pos><size>0.1 0.1</size><path>./musica.mp4</path></video>
    <video name="jogo"><pos>0.5 0.2</pos><size>0.4 0.4</size></video>
  </view>
  </variant>
</theme>"#,
        ),
        ("fundo.mp4", "not a clip"),
        ("musica.mp4", "not a clip"),
        ("anim.gif", "not a picture"),
    ]);
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    assert_eq!(t["min_launcher"].as_str(), Some("0.5.0"));
    let option = &t["options"]["fontsize"];
    assert_eq!(option["default"].as_str(), Some("medium"));
    let view = "lista-gamelist";
    let rotulo = element(&t, view, "systems", "rotulo");
    assert_eq!(rotulo["align"].as_str(), Some("right"));
    let size = rotulo["size"].as_str().unwrap().trim_start_matches('$');
    assert_eq!(t["vars"][size].as_float(), Some(0.05));
    assert_eq!(
        option["choices"]["small"]["vars"][size].as_float(),
        Some(0.03)
    );
    let els = |screen: &str| -> Vec<Table> {
        t["view"][view][screen]["el"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_table().unwrap().clone())
            .collect()
    };
    let kinds = |screen: &str, kind: &str| {
        els(screen)
            .iter()
            .filter(|e| e["kind"].as_str() == Some(kind))
            .count()
    };
    assert_eq!(kinds("systems", "status"), 1);
    assert_eq!(kinds("games", "video"), 1);
    let jogo = element(&t, view, "games", "jogo");
    assert_eq!(jogo["src"].as_str(), Some("game.video"));
    let fundo = element(&t, view, "games", "fundo");
    assert_eq!(fundo["kind"].as_str(), Some("image"));
    assert_eq!(fundo["src"].as_str(), Some("fundo-still.jpg"));
    assert!(!els("games")
        .iter()
        .any(|e| e["id"].as_str() == Some("musica")));
    let anim = element(&t, view, "systems", "anim");
    assert_eq!(anim["src"].as_str(), Some("anim-still.png"));
}

/// Paths a theme names that lead out of it (`../`, absolute, a link out)
/// are refused: nothing outside the theme is read into the output, and
/// nothing is written outside the output folder.
#[test]
fn paths_out_of_the_theme_are_refused() {
    let dir = theme(&[
        ("capabilities.xml", CAPABILITIES),
        (
            "theme.xml",
            &format!(
                r#"<theme><variant name="lista">
  <view name="system">
    <image name="fora"><pos>0 0</pos><size>0.2 0.2</size><path>../segredo.png</path></image>
    <image name="absoluta"><pos>0 0</pos><size>0.2 0.2</size><path>/etc/hostname</path></image>
    <image name="elo"><pos>0 0</pos><size>0.2 0.2</size><path>./elo.png</path></image>
    <animation name="gif"><pos>0 0</pos><size>0.2 0.2</size><path>../../fora.gif</path></animation>
  </view>
  <view name="gamelist">
    <textlist name="gamelist"><pos>0 0.15</pos><size>0.45 0.7</size><fontSize>0.04</fontSize>
      <selectorImagePath>../segredo.png</selectorImagePath></textlist>
    <video name="fundo"><pos>0 0</pos><size>1 1</size><path>../fora.mp4</path></video>
    <video name="jogo"><pos>0.5 0.2</pos><size>0.4 0.4</size></video>
  </view>
</variant></theme>"#
            ),
        ),
    ]);
    png(&dir.path().join("segredo.png"), (8, 8));
    std::fs::write(dir.path().join("fora.mp4"), "x").unwrap();
    std::fs::write(dir.path().join("es/LICENSE-elo"), "x").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            dir.path().join("segredo.png"),
            dir.path().join("es/elo.png"),
        )
        .unwrap();
        std::fs::remove_file(dir.path().join("es/LICENSE-elo")).unwrap();
        std::os::unix::fs::symlink("/etc/hostname", dir.path().join("es/LICENSE-elo")).unwrap();
    }
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let out = dir.path().join("out");
    let mut files = Vec::new();
    let mut stack = vec![out.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                files.push(e.path());
            }
        }
    }
    let names: Vec<String> = files
        .iter()
        .map(|f| f.strip_prefix(&out).unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names.iter().any(|n| n.contains("segredo")
            || n.contains("elo")
            || n.contains("hostname")
            || n.contains("fora")),
        "{names:?}"
    );
    assert!(!dir.path().join("fora-still.jpg").exists());
    let t = read(dir.path());
    let list = element(&t, "lista-gamelist", "games", "gamelist");
    assert!(list.get("selected").and_then(|s| s.get("frame")).is_none());
}

/// ES-DE's collections: a grid of systems laid out from its sizes, a
/// carousel of the games' covers; elements without a zIndex take ES-DE's
/// defaults; help for its menus is left out; cropSize fills its box.
#[test]
fn esde_grids_carousels_z_and_scopes() {
    let dir = theme(&[
        ("capabilities.xml", CAPABILITIES),
        (
            "theme.xml",
            r#"<theme><variant name="lista">
  <view name="system">
    <grid name="sistemas"><pos>0.05 0.1</pos><size>0.9 0.6</size><itemSize>0.2 0.25</itemSize>
      <itemSpacing>0.02 0.03</itemSpacing><itemScale>1.1</itemScale><selectorColor>FF8800</selectorColor>
      <staticImage>./logos/${system.theme}.png</staticImage></grid>
    <helpsystem name="ajuda-menu"><scope>menu</scope><pos>0 0.95</pos></helpsystem>
    <helpsystem name="ajuda"><pos>0 0.95</pos></helpsystem>
    <image name="fundo"><pos>0 0</pos><size>1 1</size><path>./fundo.png</path></image>
    <text name="titulo"><pos>0.1 0.8</pos><size>0.5 0.05</size><text>OI</text></text>
  </view>
  <view name="gamelist">
    <carousel name="jogos"><pos>0 0.3</pos><size>1 0.4</size><type>horizontal</type>
      <maxItemCount>5</maxItemCount><itemSize>0.15 0.3</itemSize><imageType>cover</imageType>
      <reflections>true</reflections><reflectionsOpacity>0.3</reflectionsOpacity></carousel>
    <image name="tela"><pos>0.6 0.1</pos><cropSize>0.3 0.3</cropSize><imageType>screenshot</imageType>
      <cornerRadius>0.01</cornerRadius></image>
  </view>
</variant></theme>"#,
        ),
    ]);
    png(&dir.path().join("es/fundo.png"), (64, 48));
    png(&dir.path().join("es/logos/snes.png"), (64, 32));
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    let view = "lista-gamelist";
    let grid = element(&t, view, "systems", "sistemas");
    assert_eq!(grid["kind"].as_str(), Some("grid"));
    assert_eq!(grid["of"].as_str(), Some("systems"));
    // (0.9 + 0.02) / (0.2 + 0.02) = 4 columns; (0.6 + 0.03) / (0.25 + 0.03) = 2 rows.
    assert_eq!(grid["columns"].as_integer(), Some(4));
    assert_eq!(grid["rows"].as_integer(), Some(2));
    assert_eq!(
        grid["item"]["image"][0].as_str(),
        Some("system.logo"),
        "{grid:?}"
    );
    assert_eq!(
        grid["selected"]["frame"]["color"].as_str(),
        Some("#FF8800FF")
    );
    assert!(t["view"][view]["systems"]["el"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["id"].as_str() != Some("ajuda-menu")
            && e["id"].as_str() != Some("systemcarousel")));
    // ES-DE's default zIndex: image 30, text 40, grid 50 (four times here).
    let z = |id: &str| element(&t, view, "systems", id)["z"].as_integer();
    assert_eq!(
        (z("fundo"), z("titulo"), z("sistemas")),
        (Some(120), Some(160), Some(200))
    );
    let jogos = element(&t, view, "games", "jogos");
    assert_eq!(jogos["kind"].as_str(), Some("carousel"));
    assert_eq!(jogos["of"].as_str(), Some("games"));
    assert_eq!(jogos["visible"].as_integer(), Some(5));
    assert_eq!(jogos["item"]["image"][0].as_str(), Some("game.cover"));
    assert_eq!(jogos["reflection"].as_float(), Some(0.3));
    let tela = element(&t, view, "games", "tela");
    assert_eq!(tela["fit"].as_str(), Some("cover"));
    assert_eq!(tela["rect"]["w"].as_float(), Some(0.3));
    assert_eq!(tela["radius"].as_float(), Some(0.01));
}

/// Round two of ES-DE: a container's text from the top; a carousel moved
/// off the middle moves its box; labels of facts the launcher lacks go; a
/// text as wide as its words stops at the next one on its line; players
/// as the list says them; the theme's clock places the status and its art
/// goes; a system's clip has the theme's picture of it as its still; a
/// light theme's details card is light, and only the cover and buttons
/// when its games screen already shows the facts.
#[test]
fn esde_round_two_layout() {
    let dir = theme(&[
        ("capabilities.xml", CAPABILITIES),
        (
            "theme.xml",
            r#"<theme><variant name="lista">
  <view name="system">
    <carousel name="roda"><pos>0 0.3</pos><size>1 0.3</size><type>horizontal</type>
      <horizontalOffset>-0.25</horizontalOffset></carousel>
    <video name="fundo"><pos>0 0</pos><size>1 1</size><path>./clips/${system.theme}.mp4</path>
      <audio>false</audio></video>
    <clock name="relogio"><pos>0.99 0.99</pos><origin>1 1</origin><fontSize>0.05</fontSize></clock>
    <systemstatus name="status"><pos>0.01 0.95</pos></systemstatus>
    <image name="clockBG"><pos>0 0</pos><size>1 1</size><path>./lcd.png</path></image>
  </view>
  <view name="gamelist">
    <textlist name="gamelist"><pos>0.5 0.15</pos><size>0.45 0.7</size><fontSize>0.04</fontSize></textlist>
    <text name="titulo"><pos>0.05 0.1</pos><size>0.4 0.2</size><metadata>name</metadata>
      <verticalAlignment>bottom</verticalAlignment><container>true</container><color>222222</color>
      <fontSize>0.05</fontSize></text>
    <text name="descricao"><pos>0.05 0.5</pos><size>0.4 0.2</size><metadata>description</metadata><color>222222</color></text>
    <text name="jogadores"><pos>0.05 0.4</pos><origin>0 0</origin><metadata>players</metadata><fontSize>0.03</fontSize></text>
    <text name="editora"><pos>0.2 0.4</pos><size>0.2 0.04</size><metadata>publisher</metadata><fontSize>0.03</fontSize></text>
    <text name="genero"><pos>0.05 0.45</pos><size>0.2 0.04</size><metadata>genre</metadata></text>
    <text name="ano"><pos>0.25 0.45</pos><size>0.2 0.04</size><metadata>releasedate</metadata></text>
    <text name="tempoLabel"><pos>0.05 0.3</pos><size>0.2 0.04</size><text>PLAY TIME</text></text>
  </view>
</variant></theme>"#,
        ),
        ("clips/megadrive.mp4", "x"),
    ]);
    png(&dir.path().join("es/images/megadrive.png"), (64, 48));
    png(&dir.path().join("es/lcd.png"), (64, 48));
    let c = convert(dir.path());
    assert!(c.warnings.is_empty(), "{:#?}", c.warnings);
    let t = read(dir.path());
    let view = "lista-gamelist";
    let roda = element(&t, view, "systems", "roda");
    assert_eq!(roda["rect"]["x"].as_float(), Some(-0.25));
    let fundo = element(&t, view, "systems", "fundo");
    assert_eq!(fundo["fallback"][0].as_str(), Some("images/${system}.png"));
    let systems = t["view"][view]["systems"]["el"].as_array().unwrap();
    let status: Vec<&Value> = systems
        .iter()
        .filter(|e| e["kind"].as_str() == Some("status"))
        .collect();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0]["id"].as_str(), Some("relogio"));
    assert!(systems.iter().all(|e| e["id"].as_str() != Some("clockBG")));
    let titulo = element(&t, view, "games", "titulo");
    assert_ne!(titulo.get("valign").and_then(Value::as_str), Some("bottom"));
    let jogadores = element(&t, view, "games", "jogadores");
    assert_eq!(jogadores["text"].as_str(), Some("${game.players.count}"));
    let w = jogadores["rect"]["w"].as_float().unwrap();
    assert!(w < 0.15, "{w}");
    assert_eq!(jogadores["fit_text"].as_str(), Some("shrink"));
    let games = t["view"][view]["games"]["el"].as_array().unwrap();
    assert!(games
        .iter()
        .all(|e| e["id"].as_str() != Some("tempolabel") && e["id"].as_str() != Some("tempoLabel")));
    let details = t["view"][view]["details"]["el"].as_array().unwrap();
    let card = details
        .iter()
        .find(|e| e["id"].as_str() == Some("fundo-ficha"))
        .unwrap();
    assert_eq!(card["fill"].as_str(), Some("#FFFFFFE6"));
    assert!(details.iter().all(|e| e["id"].as_str() != Some("ficha")));
    assert_eq!(t["min_launcher"].as_str(), Some("0.6.0"));
}
