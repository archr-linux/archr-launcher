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
