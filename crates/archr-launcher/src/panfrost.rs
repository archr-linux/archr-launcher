//! Ports-panfrost: PortMaster ports that need desktop OpenGL run only on
//! panfrost (Mesa); libmali has OpenGL ES alone. Read at a port's launch,
//! never during the library scan.
use std::{fs, io::Read, path::Path};

/// Arch R's PortMaster packages that need panfrost, one per line.
pub const LIST: &str = "/usr/share/archr/portmaster/panfrost.txt";

/// Whether the port `script` needs panfrost: its package is on `list`, or
/// its PortMaster `port.json` requires OpenGL. A script without the
/// PortMaster header never does.
pub fn needed(script: &Path, list: &Path) -> bool {
    let mut head = Vec::new();
    if fs::File::open(script)
        .and_then(|file| file.take(4096).read_to_end(&mut head))
        .is_err()
    {
        return false;
    }
    let head = String::from_utf8_lossy(&head);
    let Some(package) = package(&head) else {
        return false;
    };
    if listed(&fs::read_to_string(list).unwrap_or_default(), package) {
        return true;
    }
    // PortMaster installs `<folder>/port.json` next to the script; the first
    // one naming the package answers.
    let Some(entries) = script.parent().and_then(|dir| fs::read_dir(dir).ok()) else {
        return false;
    };
    entries
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("port.json")).ok())
        .find_map(|json| opengl(&json, package))
        .unwrap_or(false)
}

/// `bugdom.zip` of `# PORTMASTER: bugdom.zip, Bugdom.sh`, in the first lines.
fn package(head: &str) -> Option<&str> {
    head.lines()
        .take(10)
        .find_map(|line| {
            line.strip_prefix('#')?
                .trim_start()
                .strip_prefix("PORTMASTER:")
        })
        .and_then(|rest| rest.split(',').next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

/// The list's names, `#` starting a comment.
fn listed(list: &str, package: &str) -> bool {
    list.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .any(|name| name.eq_ignore_ascii_case(package))
}

/// For the port.json of `package`, whether it requires OpenGL; None for
/// another port's or an unreadable one.
fn opengl(json: &str, package: &str) -> Option<bool> {
    let port: serde_json::Value = serde_json::from_str(json).ok()?;
    port["name"]
        .as_str()
        .filter(|name| name.eq_ignore_ascii_case(package))?;
    let reqs = port["attr"]["reqs"].as_array();
    Some(reqs.is_some_and(|reqs| {
        reqs.iter().any(|req| {
            req.as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case("opengl"))
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "#!/bin/bash\n# PORTMASTER: bugdom.zip, Bugdom.sh\n\nexec true\n";

    #[test]
    fn the_header_names_the_package() {
        assert_eq!(package(SCRIPT), Some("bugdom.zip"));
        assert_eq!(package("#!/bin/bash\n#PORTMASTER: A.zip\n"), Some("A.zip"));
        assert_eq!(package("#!/bin/bash\nexec true\n"), None);
        assert_eq!(package("# PORTMASTER: , A.sh\n"), None);
        let late = format!("{}# PORTMASTER: late.zip, Late.sh\n", "#\n".repeat(10));
        assert_eq!(package(&late), None, "only the first lines");
    }
    #[test]
    fn the_list_has_one_name_per_line_and_comments() {
        let list = "# needs desktop GL\nBZFlag.zip\n  Bugdom.zip  # crashes in libmali\n";
        assert!(listed(list, "bugdom.zip"));
        assert!(listed(list, "BZFlag.zip"));
        assert!(!listed(list, "needs"));
        assert!(!listed("", "bugdom.zip"));
    }
    #[test]
    fn port_json_requires_opengl_for_its_own_package() {
        let json = r#"{"name": "bugdom.zip", "items": ["Bugdom.sh", "bugdom"],
            "attr": {"reqs": ["OpenGL", "power"]}}"#;
        assert_eq!(opengl(json, "bugdom.zip"), Some(true));
        assert_eq!(opengl(json, "Other.zip"), None);
        let plain = r#"{"name": "A.zip", "attr": {"reqs": []}}"#;
        assert_eq!(opengl(plain, "A.zip"), Some(false));
        assert_eq!(opengl(r#"{"name": "A.zip"}"#, "A.zip"), Some(false));
        assert_eq!(opengl("not json", "A.zip"), None);
    }
    #[test]
    fn launch_reads_the_script_the_list_and_the_ports_folders() {
        let dir = tempfile::tempdir().unwrap();
        let ports = dir.path().join("ports");
        fs::create_dir_all(ports.join("bugdom")).unwrap();
        fs::create_dir_all(ports.join("other")).unwrap();
        let script = ports.join("Bugdom.sh");
        let list = dir.path().join("panfrost.txt");
        assert!(!needed(&script, &list), "no script");
        fs::write(&script, SCRIPT).unwrap();
        assert!(!needed(&script, &list), "no list, no port.json");
        fs::write(&list, "Bugdom.zip\n").unwrap();
        assert!(needed(&script, &list), "listed");
        fs::remove_file(&list).unwrap();
        fs::write(
            ports.join("other/port.json"),
            r#"{"name": "Other.zip", "attr": {"reqs": ["opengl"]}}"#,
        )
        .unwrap();
        assert!(!needed(&script, &list), "another port's requirement");
        fs::write(
            ports.join("bugdom/port.json"),
            r#"{"name": "bugdom.zip", "attr": {"reqs": ["opengl"]}}"#,
        )
        .unwrap();
        assert!(needed(&script, &list), "its own port.json");
        let plain = ports.join("Plain.sh");
        fs::write(&plain, "#!/bin/bash\nexec true\n").unwrap();
        fs::write(&list, "Bugdom.zip\n").unwrap();
        assert!(!needed(&plain, &list), "no header, never");
    }
}
