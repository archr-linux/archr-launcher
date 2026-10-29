//! Engine behaviour against a mocked HTTP layer: no test touches the network.
use archr_index::{scan_system, SystemDef};
use archr_scraper::{
    trusted, Credentials, Curl, Engine, Event, Http, Job, Media, NetError, Options, Report,
    Response, Stop, Timing,
};
use std::{
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::{Duration, Instant},
};

fn encode(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(width, height)
        .write_to(&mut out, format)
        .unwrap();
    out.into_inner()
}

fn png(width: u32, height: u32) -> Vec<u8> {
    encode(width, height, image::ImageFormat::Png)
}

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    encode(width, height, image::ImageFormat::Jpeg)
}

/// Tests that swap PATH for a stand-in `curl` must not overlap.
static PATH_LOCK: Mutex<()> = Mutex::new(());

const DEV: &str = "devid=tester&devpassword=hunter2";

type Handler = dyn Fn(&str) -> Option<(u16, Vec<u8>)> + Send + Sync;
struct Mock {
    handler: Box<Handler>,
    seen: Mutex<Vec<String>>,
    limits: Mutex<Vec<(String, u64)>>,
}
impl Http for Mock {
    fn get(&self, url: &str, limit: u64, _cancel: &AtomicBool) -> Result<Response, NetError> {
        self.seen.lock().unwrap().push(url.to_owned());
        self.limits.lock().unwrap().push((url.to_owned(), limit));
        match (self.handler)(url) {
            Some((status, body)) => Ok(Response { status, body }),
            None => Err(NetError::Failed),
        }
    }
}

fn mock(handler: impl Fn(&str) -> Option<(u16, Vec<u8>)> + Send + Sync + 'static) -> Arc<Mock> {
    Arc::new(Mock {
        handler: Box::new(handler),
        seen: Mutex::new(Vec::new()),
        limits: Mutex::new(Vec::new()),
    })
}

fn engine(http: Arc<Mock>, cache: &Path) -> Engine {
    let creds = Credentials {
        user: "ana".into(),
        password: "s3cret pw".into(),
    };
    let mut engine = Engine::new(http, DEV, cache.to_owned(), Some(creds));
    engine.timing = Timing {
        spacing: Duration::ZERO,
        retry: Duration::ZERO,
    };
    engine
}

fn user_xml() -> Vec<u8> {
    b"<Data><ssuser><id>ana</id><maxthreads>1</maxthreads><requeststoday>5</requeststoday><maxrequestsperday>20000</maxrequestsperday></ssuser></Data>".to_vec()
}

fn game_xml(id: &str) -> Vec<u8> {
    format!(
        "<Data><jeu id=\"{id}\"><medias>\
<media type=\"box-2D\" region=\"eu\" format=\"png\">https://www.screenscraper.fr/m/{id}/box-eu</media>\
<media type=\"box-2D\" region=\"br\" format=\"png\">https://www.screenscraper.fr/m/{id}/box-br?a=1&amp;b=2</media>\
<media type=\"ss\" region=\"\" format=\"png\">https://www.screenscraper.fr/m/{id}/ss</media>\
<media type=\"wheel\" region=\"wor\" format=\"png\">https://evil.example/m/{id}/wheel</media>\
</medias></jeu></Data>"
    )
    .into_bytes()
}

fn system(root: &Path, names: &[&str]) -> SystemDef {
    let path = root.join("snes");
    fs::create_dir_all(&path).unwrap();
    for name in names {
        fs::write(path.join(name), name.as_bytes()).unwrap();
    }
    SystemDef {
        name: "snes".into(),
        path,
        extensions: vec![".sfc".into()],
        ..SystemDef::default()
    }
}

fn finish(job: &Job) -> (Vec<archr_scraper::Progress>, Report) {
    let end = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    while Instant::now() < end {
        match job.poll() {
            Some(Event::Progress(p)) => seen.push(p),
            Some(Event::Done(report)) => return (seen, report),
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("job did not finish");
}

fn happy(url: &str) -> Option<(u16, Vec<u8>)> {
    if url.contains("ssuserInfos.php") {
        Some((200, user_xml()))
    } else if url.contains("jeuInfos.php") {
        Some((200, game_xml("7")))
    } else if url.contains("/m/") {
        Some((200, png(64, 48)))
    } else {
        Some((404, b"not found".to_vec()))
    }
}

#[test]
fn scrapes_missing_art_into_the_media_layout_the_library_reads() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(
        &dir.path().join("roms"),
        &["Alpha (USA).sfc", "Beta.sfc", "Done.sfc"],
    );
    fs::create_dir_all(sys.path.join("media/images")).unwrap();
    fs::write(sys.path.join("media/images/Done.png"), png(1, 1)).unwrap();
    let http = mock(happy);
    let job = engine(http.clone(), &dir.path().join("cache")).start(
        vec![sys.clone()],
        Options {
            media: vec![Media::Box2d, Media::Screenshot, Media::Wheel],
            ..Options::new("pt-BR")
        },
    );
    let (events, report) = finish(&job);
    assert_eq!(report.stop, Stop::Completed);
    // Done.sfc has its cover but still lacks the requested screenshot/wheel.
    assert_eq!((report.progress.total, report.progress.skipped), (3, 0));
    assert_eq!((report.progress.done, report.progress.scraped), (3, 3));
    assert_eq!(events.last().unwrap().done, 3);
    for stem in ["Alpha (USA)", "Beta"] {
        assert!(sys.path.join(format!("media/images/{stem}.png")).is_file());
        assert!(sys
            .path
            .join(format!("media/screenshots/{stem}.png"))
            .is_file());
        // The wheel points outside screenscraper.fr: never followed.
        assert!(!sys.path.join(format!("media/wheel/{stem}.png")).exists());
    }
    // The existing UI lookup now finds the new covers without changes.
    let games = scan_system(&sys);
    assert!(games.iter().all(|g| g.image.is_some()));

    let seen = http.seen.lock().unwrap().join("\n");
    assert!(!seen.contains("evil.example"));
    assert!(seen.contains(DEV), "developer login is sent verbatim");
    assert!(seen.contains("ssid=ana&sspassword=s3cret%20pw"));
    assert!(seen.contains("crc=") && seen.contains("md5=") && seen.contains("romtaille="));
    // pt-BR prefers the Brazilian box art over the EU one.
    assert!(seen.contains("/box-br?a=1&b=2"));
    assert!(!seen.contains("/box-eu"));
}

#[test]
fn falls_back_to_name_search_and_reports_misses() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["Hit (Europe).sfc", "Miss.sfc"]);
    let http = mock(|url| {
        if url.contains("jeuInfos.php") {
            Some((404, b"Erreur".to_vec()))
        } else if url.contains("jeuRecherche.php") && url.ends_with("recherche=Hit") {
            Some((
                200,
                format!("<Data><jeux>{}</jeux></Data>", jeu_body()).into_bytes(),
            ))
        } else if url.contains("jeuRecherche.php") {
            Some((200, b"<Data><jeux><jeu/></jeux></Data>".to_vec()))
        } else {
            happy(url)
        }
    });
    let job =
        engine(http.clone(), &dir.path().join("c")).start(vec![sys.clone()], Options::new("en"));
    let (_, report) = finish(&job);
    assert_eq!((report.progress.scraped, report.progress.not_found), (1, 1));
    assert!(sys.path.join("media/images/Hit (Europe).png").is_file());
    assert!(!sys.path.join("media/images/Miss.png").exists());
    let seen = http.seen.lock().unwrap().join("\n");
    assert!(seen.contains("recherche=Hit") && seen.contains("systemeid=4"));
}

fn jeu_body() -> String {
    "<jeu id=\"9\"><medias><media type=\"box-2D\" region=\"us\">https://www.screenscraper.fr/m/9/box</media></medias></jeu>".into()
}

#[test]
fn quota_error_stops_the_run_and_resume_skips_finished_games() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let sys = system(dir.path(), &["A.sfc", "B.sfc", "C.sfc"]);
    let calls = Arc::new(Mutex::new(0));
    let counter = calls.clone();
    let http = mock(move |url| {
        if url.contains("jeuInfos.php") {
            let mut n = counter.lock().unwrap();
            *n += 1;
            if *n == 2 {
                return Some((430, b"quota".to_vec()));
            }
        }
        happy(url)
    });
    let job = engine(http, &cache).start(vec![sys.clone()], Options::new("en"));
    let (_, first) = finish(&job);
    assert_eq!(first.stop, Stop::Quota);
    assert_eq!(first.progress.scraped, 1);

    let http = mock(happy);
    let opts = Options {
        only_missing: false,
        ..Options::new("en")
    };
    let job = engine(http.clone(), &cache).start(vec![sys.clone()], opts);
    let (_, second) = finish(&job);
    assert_eq!(second.stop, Stop::Completed);
    // One game finished earlier is skipped; the other two are processed.
    assert_eq!((second.progress.skipped, second.progress.total), (1, 2));
}

#[test]
fn cancel_stops_promptly_and_reports_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..30).map(|i| format!("G{i:02}.sfc")).collect();
    let sys = system(
        dir.path(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let http = mock(|url| {
        std::thread::sleep(Duration::from_millis(20));
        happy(url)
    });
    let job = engine(http, &dir.path().join("c")).start(vec![sys], Options::new("en"));
    std::thread::sleep(Duration::from_millis(200));
    job.cancel();
    let (_, report) = finish(&job);
    assert_eq!(report.stop, Stop::Cancelled);
    assert!(report.progress.done < 30);
}

#[test]
fn bad_login_and_unreachable_network_end_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["A.sfc"]);
    let http = mock(|_| Some((403, b"login".to_vec())));
    let (_, report) =
        finish(&engine(http, dir.path()).start(vec![sys.clone()], Options::new("en")));
    assert_eq!(report.stop, Stop::Auth);

    let http = mock(|_| None);
    let (_, report) = finish(&engine(http, dir.path()).start(vec![sys], Options::new("en")));
    assert_eq!(report.stop, Stop::Network);
}

#[test]
fn error_pages_served_as_art_are_failures_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["A.sfc"]);
    let http = mock(|url| {
        if url.contains("/box-") {
            Some((200, b"CRCOK: error text".to_vec()))
        } else {
            happy(url)
        }
    });
    let (_, report) =
        finish(&engine(http, dir.path()).start(vec![sys.clone()], Options::new("en")));
    assert_eq!((report.progress.scraped, report.progress.failed), (0, 1));
    assert!(!sys.path.join("media/images").exists());
}

#[test]
fn credentials_file_is_private_and_debug_hides_the_password() {
    let dir = tempfile::tempdir().unwrap();
    let creds = Credentials {
        user: "ana".into(),
        password: "pw1".into(),
    };
    creds.save(dir.path()).unwrap();
    let mode = std::os::unix::fs::PermissionsExt::mode(
        &fs::metadata(dir.path().join("scraper.toml"))
            .unwrap()
            .permissions(),
    );
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(Credentials::load(dir.path()), Some(creds.clone()));
    assert!(!format!("{creds:?}").contains("pw1"));
    Credentials::clear(dir.path()).unwrap();
    assert_eq!(Credentials::load(dir.path()), None);
    // A link planted in its place is ignored, not followed.
    let elsewhere = tempfile::tempdir().unwrap();
    creds.save(elsewhere.path()).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.path().join("scraper.toml"),
        dir.path().join("scraper.toml"),
    )
    .unwrap();
    assert_eq!(Credentials::load(dir.path()), None);
}

#[test]
fn verify_reports_the_account_limits() {
    let dir = tempfile::tempdir().unwrap();
    let info = engine(mock(happy), dir.path()).verify().unwrap();
    assert_eq!((info.threads, info.max_requests_per_day), (1, 20000));
    let denied = engine(mock(|_| Some((403, vec![]))), dir.path()).verify();
    assert_eq!(denied, Err(Stop::Auth));
}

// ---- regression tests for the review findings ----

fn cue_system(root: &Path) -> SystemDef {
    let sys = system(root, &["Disc (USA).cue"]);
    SystemDef {
        extensions: vec![".cue".into()],
        ..sys
    }
}

#[test]
fn unsafe_media_hosts_are_never_trusted() {
    for bad in [
        "https://example.invalid#.screenscraper.fr",
        "https://example.invalid#.screenscraper.fr/x",
        "https://user@www.screenscraper.fr/x",
        "https://www.screenscraper.fr@evil.example/x",
        "https://www.screenscraper.fr:444/x",
        "https://www.screenscraper.fr.evil.example/x",
        "https://WWW.screenscraper.fr/x",
        "https://www.screenscraper.fr\\@evil/x",
        "http://www.screenscraper.fr/x",
        "https://www.screenscraper.fr/x\nhelp\n#",
        "https://www.screenscraper.fr/x\"",
    ] {
        assert!(!trusted(bad), "{bad:?}");
    }
    assert!(trusted("https://www.screenscraper.fr/medias/1?a=1&b=2"));
    assert!(trusted("https://screenscraper.fr/x"));
}

/// A stand-in `curl` on PATH records how it was invoked.
fn fake_curl(dir: &Path, reply: &str) {
    use std::os::unix::fs::PermissionsExt;
    let script = format!(
        "#!/bin/sh\necho \"$@\" > {d}/args\nenv > {d}/env\ncat > {d}/stdin\n\
out=$(sed -n 's/^output = \"\\(.*\\)\"$/\\1/p' {d}/stdin)\nprintf hello > \"$out\"\nprintf '{reply}'\n",
        d = dir.display()
    );
    let path = dir.join("curl");
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn curl_is_hardened_and_config_injection_is_refused() {
    let _path = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let never = AtomicBool::new(false);
    let url = "https://www.screenscraper.fr/api2/x?devid=a&devpassword=b";
    let curl = |agent: &str| Curl {
        tmp: dir.path().join("tmp"),
        user_agent: agent.into(),
        hosts: archr_scraper::SCREENSCRAPER,
    };
    // Control characters (a newline would inject a curl directive) never reach curl.
    for bad in [
        "https://www.screenscraper.fr/x\nhelp\n#",
        "https://www.screenscraper.fr/\rx",
    ] {
        assert!(curl("ua").get(bad, 1024, &never).is_err());
    }
    assert!(curl("ua\nhelp").get(url, 1024, &never).is_err());

    std::env::set_var("SCREENSCRAPER_DEV_LOGIN", "devid=leak&devpassword=leak");
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", dir.path().display()));
    fake_curl(dir.path(), "200 ");
    let response = curl("ua \"q\"").get(url, 1024, &never).unwrap();
    assert_eq!(
        (response.status, response.body.as_slice()),
        (200, &b"hello"[..])
    );
    let args = fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(args.starts_with("-q "), "{args}");
    assert!(args.contains("--globoff") && args.contains("--max-redirs 0"));
    assert!(!args.contains("screenscraper") && !args.contains("devpassword"));
    let stdin = fs::read_to_string(dir.path().join("stdin")).unwrap();
    assert!(stdin.contains("url = \"https://www.screenscraper.fr/api2/x?devid=a&devpassword=b\""));
    assert!(stdin.contains("user-agent = \"ua \\\"q\\\"\""));
    let env = fs::read_to_string(dir.path().join("env")).unwrap();
    assert!(!env.contains("SCREENSCRAPER_DEV_LOGIN") && !env.contains("leak"));

    // A redirect off the allowlist is refused; one back onto it is followed.
    fake_curl(dir.path(), "302 https://evil.example/x");
    assert!(curl("ua").get(url, 1024, &never).is_err());
    fake_curl(dir.path(), "301 https://example.invalid#.screenscraper.fr");
    assert!(curl("ua").get(url, 1024, &never).is_err());
}

#[test]
fn failed_asset_is_retried_without_refetching_what_exists() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let sys = system(dir.path(), &["A.sfc"]);
    let xml = "<Data><jeu id=\"1\"><medias>\
<media type=\"box-2D\" region=\"us\">https://www.screenscraper.fr/m/box</media>\
<media type=\"ss\">https://www.screenscraper.fr/m/ss</media></medias></jeu></Data>";
    let broken = mock(move |url| {
        if url.contains("jeuInfos.php") {
            Some((200, xml.as_bytes().to_vec()))
        } else if url.ends_with("/m/ss") {
            Some((500, b"later".to_vec()))
        } else {
            happy(url)
        }
    });
    let mut opts = Options::new("en");
    opts.media = vec![Media::Box2d, Media::Screenshot];
    let (_, first) = finish(&engine(broken, &cache).start(vec![sys.clone()], opts.clone()));
    assert_eq!((first.progress.scraped, first.progress.failed), (0, 1));
    assert!(sys.path.join("media/images/A.png").is_file());
    assert!(!sys.path.join("media/screenshots/A.png").exists());

    let fixed = mock(move |url| {
        if url.contains("jeuInfos.php") {
            Some((200, xml.as_bytes().to_vec()))
        } else {
            happy(url)
        }
    });
    let (_, second) = finish(&engine(fixed, &cache).start(vec![sys.clone()], opts));
    assert_eq!((second.progress.total, second.progress.scraped), (1, 1));
    assert!(sys.path.join("media/images/A.png").is_file());
    assert!(sys.path.join("media/screenshots/A.png").is_file());
}

fn png_without_pixels() -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    out.extend(1u32.to_be_bytes());
    out.extend(1u32.to_be_bytes());
    out.extend(b"\x08\x02\0\0\0\0\0\0\0\0\0\0\0IEND\xAEB`\x82");
    out
}

/// A tiny grayscale JPEG whose frame header claims a progressive 8192x8192
/// image: what once made the decoder allocate ~200 MiB for a 385 KiB file.
fn huge_progressive_jpeg() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::GrayImage::new(8, 8)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .unwrap();
    let mut bytes = out.into_inner();
    let sof = bytes.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    bytes[sof + 1] = 0xC2;
    bytes[sof + 5..sof + 9].copy_from_slice(&[0x20, 0x00, 0x20, 0x00]);
    bytes
}

/// A 1x1 PNG carrying an iCCP chunk that inflates to 128 MiB: about 130 KB on
/// the wire, and a decoder that expands metadata while reading the header
/// would allocate it all.
fn iccp_bomb() -> Vec<u8> {
    use std::io::Write;
    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let zeros = vec![0u8; 1 << 20];
    for _ in 0..128 {
        zlib.write_all(&zeros).unwrap();
    }
    let mut data = b"icc\0\0".to_vec();
    data.extend(zlib.finish().unwrap());
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    let mut body = b"iCCP".to_vec();
    body.extend(&data);
    chunk.extend(&body);
    chunk.extend(crc32fast::hash(&body).to_be_bytes());
    let mut bytes = png(1, 1);
    bytes.splice(33..33, chunk); // right after the 25-byte IHDR chunk
    bytes
}

/// Peak resident memory of this process, in KiB.
fn peak_kib() -> u64 {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let line = status.lines().find(|l| l.starts_with("VmHWM:")).unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn metadata_bomb_png_does_not_inflate_into_memory() {
    let bomb = iccp_bomb();
    assert!(bomb.len() < 400 << 10);
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["A.sfc"]);
    let http = mock(move |url| {
        if url.contains("/box-") {
            Some((200, bomb.clone()))
        } else {
            happy(url)
        }
    });
    let before = peak_kib();
    let (_, report) = finish(&engine(http, dir.path()).start(vec![sys], Options::new("en")));
    assert_eq!(report.stop, Stop::Completed);
    // Unbounded, the 128 MiB chunk is inflated while reading the header.
    assert!(
        peak_kib() - before < 64 << 10,
        "peak grew by {} KiB",
        peak_kib() - before
    );
}

#[test]
fn only_complete_decodable_images_are_saved() {
    let whole_png = png(64, 48);
    let whole_jpeg = jpeg(64, 48);
    for (name, body, saved) in [
        ("header only", b"\x89PNG".to_vec(), false),
        (
            "truncated png",
            whole_png[..whole_png.len() / 2].to_vec(),
            false,
        ),
        ("png without pixels", png_without_pixels(), false),
        ("png too wide", png(4097, 1), false),
        ("jpeg too wide", jpeg(4097, 1), false),
        ("png over 4 megapixels", png(2049, 2049), false),
        ("progressive 8192x8192 jpeg", huge_progressive_jpeg(), false),
        ("png at the limit", png(2048, 2048), true),
        (
            "truncated jpeg",
            whole_jpeg[..whole_jpeg.len() / 2].to_vec(),
            false,
        ),
        (
            "jpeg zero-length frame",
            vec![0xff, 0xd8, 0xff, 0xc0, 0, 0, 0xff, 0xd9],
            false,
        ),
        (
            "jpeg without scan",
            vec![0xff, 0xd8, 0xff, 0xc0, 0, 7, 8, 0, 1, 0, 1, 0xff, 0xd9],
            false,
        ),
        ("jpeg", whole_jpeg.clone(), true),
        ("png", whole_png.clone(), true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sys = system(dir.path(), &["A.sfc"]);
        let http = mock(move |url| {
            if url.contains("/box-") {
                Some((200, body.clone()))
            } else {
                happy(url)
            }
        });
        let (_, report) =
            finish(&engine(http, dir.path()).start(vec![sys.clone()], Options::new("en")));
        assert_eq!(report.stop, Stop::Completed, "{name}");
        let kept =
            fs::read_dir(sys.path.join("media/images")).is_ok_and(|mut d| d.next().is_some());
        assert_eq!(
            (kept, report.progress.failed == 0),
            (saved, saved),
            "{name}"
        );
    }
}

/// Stops the first run at its second game with a quota error.
fn interrupted(root: &Path, cache: &Path, names: &[&str]) -> SystemDef {
    let sys = system(root, names);
    let calls = Arc::new(Mutex::new(0));
    let http = mock(move |url| {
        if url.contains("jeuInfos.php") {
            let mut n = calls.lock().unwrap();
            *n += 1;
            if *n == 2 {
                return Some((430, vec![]));
            }
        }
        happy(url)
    });
    let (_, report) = finish(&engine(http, cache).start(vec![sys.clone()], Options::new("en")));
    assert_eq!((report.stop, report.progress.scraped), (Stop::Quota, 1));
    sys
}

#[test]
fn completed_runs_clear_the_journal_so_deleted_art_is_fetched_again() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let sys = system(dir.path(), &["A.sfc"]);
    let (_, first) =
        finish(&engine(mock(happy), &cache).start(vec![sys.clone()], Options::new("en")));
    assert_eq!(first.progress.scraped, 1);
    assert!(!cache.join("scraper-journal.txt").exists());
    fs::remove_file(sys.path.join("media/images/A.png")).unwrap();
    let (_, second) =
        finish(&engine(mock(happy), &cache).start(vec![sys.clone()], Options::new("en")));
    assert_eq!((second.progress.total, second.progress.scraped), (1, 1));
    assert!(sys.path.join("media/images/A.png").is_file());
}

#[test]
fn interrupted_journal_is_per_library_and_never_hides_missing_assets() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let card1 = interrupted(&dir.path().join("card1"), &cache, &["A.sfc", "B.sfc"]);
    // A different library with the same relative ROM path is not "done".
    let card2 = system(&dir.path().join("card2"), &["A.sfc"]);
    let (_, other) = finish(&engine(mock(happy), &cache).start(vec![card2], Options::new("en")));
    assert_eq!(other.progress.scraped, 1);
    // Same interrupted library, asset deleted meanwhile: fetched again.
    // (Opts differ from only_missing so the cover alone is not the reason.)
    fs::remove_file(card1.path.join("media/images/A.png")).unwrap();
    let (_, resumed) = finish(&engine(mock(happy), &cache).start(vec![card1], Options::new("en")));
    assert_eq!(resumed.progress.scraped, 2);
}

#[test]
fn resume_skips_games_whose_assets_still_exist() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let card = interrupted(dir.path(), &cache, &["A.sfc", "B.sfc"]);
    let opts = Options {
        only_missing: false,
        ..Options::new("en")
    };
    let (_, resumed) = finish(&engine(mock(happy), &cache).start(vec![card], opts));
    assert_eq!((resumed.progress.skipped, resumed.progress.scraped), (1, 1));
}

#[test]
fn asset_set_change_is_not_hidden_by_an_old_journal_entry() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let card = interrupted(dir.path(), &cache, &["A.sfc", "B.sfc"]);
    let mut opts = Options::new("en");
    opts.media = vec![Media::Box2d, Media::Screenshot];
    let (_, run) = finish(&engine(mock(happy), &cache).start(vec![card], opts));
    // A already has its cover but lacks the screenshot; B is still pending.
    assert_eq!(run.progress.total, 2);
}

#[test]
fn long_redirect_metadata_cannot_stall_curl() {
    let _path = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("curl");
    fs::write(
        &script,
        "#!/bin/sh\ncat > /dev/null\nprintf '302 https://www.screenscraper.fr/'\nhead -c 70000 /dev/zero | tr '\\0' x\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", dir.path().display()));
    let curl = Curl {
        tmp: dir.path().join("tmp"),
        user_agent: "t".into(),
        hosts: archr_scraper::SCREENSCRAPER,
    };
    let never = AtomicBool::new(false);
    let started = Instant::now();
    assert!(curl
        .get("https://www.screenscraper.fr/x", 1024, &never)
        .is_err());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn form_posts_travel_on_stdin_and_stay_on_the_given_hosts() {
    let _path = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let never = AtomicBool::new(false);
    let curl = Curl {
        tmp: dir.path().join("tmp"),
        user_agent: "ua".into(),
        hosts: &["example.org"],
    };
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", dir.path().display()));
    fake_curl(dir.path(), "200 ");
    let form = "r=login2&u=me&p=s%26cret";
    let response = curl
        .post("https://api.example.org/do", form, 1024, &never)
        .unwrap();
    assert_eq!(response.status, 200);
    let stdin = fs::read_to_string(dir.path().join("stdin")).unwrap();
    assert!(
        stdin.contains("data = \"r=login2&u=me&p=s%26cret\""),
        "{stdin}"
    );
    let args = fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(
        !args.contains("cret") && !args.contains("example"),
        "{args}"
    );
    for bad in ["https://example.org.evil/do", "https://screenscraper.fr/x"] {
        assert!(curl.post(bad, form, 1024, &never).is_err());
        assert!(curl.get(bad, 1024, &never).is_err());
    }
    assert!(!archr_scraper::trusted_on(
        "https://notexample.org/",
        &["example.org"]
    ));
    assert_eq!(archr_scraper::encode("a b&ç"), "a%20b%26%C3%A7");
}

#[test]
fn games_that_cannot_be_hashed_go_straight_to_name_search() {
    let dir = tempfile::tempdir().unwrap();
    let sys = cue_system(dir.path());
    let http = mock(|url| {
        assert!(
            !url.contains("jeuInfos.php"),
            "no digest, so no hash lookup"
        );
        if url.contains("jeuRecherche.php") {
            Some((
                200,
                format!("<Data><jeux>{}</jeux></Data>", jeu_body()).into_bytes(),
            ))
        } else {
            happy(url)
        }
    });
    let (_, report) =
        finish(&engine(http.clone(), dir.path()).start(vec![sys], Options::new("en")));
    assert_eq!(report.progress.scraped, 1);
    assert!(http
        .seen
        .lock()
        .unwrap()
        .iter()
        .any(|u| u.ends_with("recherche=Disc")));
}

#[test]
fn account_minute_limit_sets_the_request_pace() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["A.sfc"]);
    let stamps = Arc::new(Mutex::new(Vec::new()));
    let log = stamps.clone();
    let http = mock(move |url| {
        log.lock().unwrap().push(Instant::now());
        if url.contains("ssuserInfos.php") {
            Some((200, b"<Data><ssuser><id>ana</id><maxrequestspermin>600</maxrequestspermin></ssuser></Data>".to_vec()))
        } else {
            happy(url)
        }
    });
    finish(&engine(http, dir.path()).start(vec![sys], Options::new("en")));
    let times = stamps.lock().unwrap();
    assert!(times.len() >= 3);
    assert!(times
        .windows(2)
        .all(|w| w[1] - w[0] >= Duration::from_millis(90)));
}

#[test]
fn blacklist_and_restricted_access_end_the_run_distinctly() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["A.sfc", "B.sfc"]);
    for (status, stop) in [
        (426, Stop::Outdated),
        (401, Stop::Restricted),
        (403, Stop::Auth),
    ] {
        let http = mock(move |url| {
            if url.contains("jeuInfos.php") {
                Some((status, vec![]))
            } else {
                happy(url)
            }
        });
        let (_, report) =
            finish(&engine(http, dir.path()).start(vec![sys.clone()], Options::new("en")));
        assert_eq!(report.stop, stop);
        assert_eq!(report.progress.done, 0);
    }
}

#[test]
fn verify_rejects_malformed_success_replies_and_other_accounts() {
    let dir = tempfile::tempdir().unwrap();
    for body in [
        &b"maintenance"[..],
        b"<Data><ssuser>",
        b"<Data></Data>",
        b"<html/>",
    ] {
        let http = mock(move |_| Some((200, body.to_vec())));
        assert_eq!(
            engine(http, dir.path()).verify(),
            Err(Stop::Closed),
            "{body:?}"
        );
    }
    let other = mock(|_| {
        Some((
            200,
            b"<Data><ssuser><id>someone-else</id></ssuser></Data>".to_vec(),
        ))
    });
    assert_eq!(engine(other, dir.path()).verify(), Err(Stop::Auth));
}

// ---- metadata, every media kind and single-game runs ----

fn rich_xml() -> Vec<u8> {
    "<Data><jeu id=\"5\">\
<noms><nom region=\"jp\">Kurono</nom><nom region=\"us\">Chrono Trigger</nom></noms>\
<synopsis><synopsis langue=\"en\">Time travel.</synopsis><synopsis langue=\"pt\">Viagem &amp; tempo.</synopsis></synopsis>\
<editeur id=\"1\">Square&amp;nbsp;Soft</editeur><developpeur id=\"2\">Square</developpeur>\
<joueurs>1</joueurs><note>19</note>\
<dates><date region=\"jp\">1995-03-11</date><date region=\"us\">1995-08</date></dates>\
<genres><genre id=\"1\" principale=\"1\" langue=\"en\">Role playing</genre>\
<genre id=\"1\" principale=\"1\" langue=\"pt\">RPG</genre></genres>\
<medias>\
<media type=\"box-2D\" region=\"us\" format=\"png\">https://www.screenscraper.fr/m/box2d</media>\
<media type=\"box-3D\" region=\"us\" format=\"png\">https://www.screenscraper.fr/m/box3d</media>\
<media type=\"mixrbv2\" region=\"wor\" format=\"png\">https://www.screenscraper.fr/m/mix</media>\
<media type=\"ss\" region=\"wor\" format=\"png\">https://www.screenscraper.fr/m/ss</media>\
<media type=\"sstitle\" region=\"wor\" format=\"png\">https://www.screenscraper.fr/m/title</media>\
<media type=\"wheel-hd\" region=\"wor\" format=\"png\">https://www.screenscraper.fr/m/wheel</media>\
<media type=\"video-normalized\" format=\"mp4\">https://www.screenscraper.fr/m/video</media>\
<media type=\"manuel\" region=\"us\" format=\"pdf\">https://www.screenscraper.fr/m/manual</media>\
</medias></jeu></Data>"
        .as_bytes()
        .to_vec()
}

fn rich(url: &str) -> Option<(u16, Vec<u8>)> {
    if url.contains("jeuInfos.php") {
        Some((200, rich_xml()))
    } else if url.ends_with("/m/video") {
        Some((200, b"\0\0\0\x18ftypmp42 clip".to_vec()))
    } else if url.ends_with("/m/manual") {
        Some((200, b"%PDF-1.4 manual".to_vec()))
    } else {
        happy(url)
    }
}

fn all_media(lang: &str) -> Options {
    Options {
        media: Media::ALL.to_vec(),
        ..Options::new(lang)
    }
}

#[test]
fn media_setting_lists_parse_in_order_and_ignore_unknown_tokens() {
    assert_eq!(
        Media::parse_list("box, video,bogus,box,manual"),
        [Media::Box2d, Media::Video, Media::Manual]
    );
    assert!(Media::parse_list("").is_empty());
    let tokens: Vec<_> = Media::ALL.iter().map(|m| m.token()).collect();
    assert_eq!(Media::parse_list(&tokens.join(",")), Media::ALL);
}

#[test]
fn every_media_kind_and_the_metadata_land_in_the_gamelist() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["Chrono.sfc"]);
    let http = mock(rich);
    let (_, report) = finish(
        &engine(http.clone(), &dir.path().join("c")).start(vec![sys.clone()], all_media("pt-BR")),
    );
    assert_eq!((report.stop, report.progress.scraped), (Stop::Completed, 1));
    for file in [
        "images/Chrono.png",
        "box3d/Chrono.png",
        "mix/Chrono.png",
        "screenshots/Chrono.png",
        "titlescreens/Chrono.png",
        "wheel/Chrono.png",
        "videos/Chrono.mp4",
        "manuals/Chrono.pdf",
    ] {
        assert!(sys.path.join("media").join(file).is_file(), "{file}");
    }
    let xml = fs::read_to_string(sys.path.join("gamelist.xml")).unwrap();
    for tag in [
        "<name>Chrono Trigger</name>",
        "<desc>Viagem &amp; tempo.</desc>",
        "<releasedate>19950801T000000</releasedate>",
        "<players>1</players>",
        "<rating>0.95</rating>",
        "<genre>RPG</genre>",
        "<developer>Square</developer>",
        "<publisher>Square Soft</publisher>",
        "<thumbnail>./media/images/Chrono.png</thumbnail>",
        "<image>./media/screenshots/Chrono.png</image>",
        "<marquee>./media/wheel/Chrono.png</marquee>",
        "<video>./media/videos/Chrono.mp4</video>",
        "<manual>./media/manuals/Chrono.pdf</manual>",
        "<titleshot>./media/titlescreens/Chrono.png</titleshot>",
        "<boxart>./media/box3d/Chrono.png</boxart>",
        "<mix>./media/mix/Chrono.png</mix>",
    ] {
        assert!(xml.contains(tag), "{tag} missing:\n{xml}");
    }
    // The library reads it all back.
    let game = scan_system(&sys).remove(0);
    assert_eq!(game.name, "Chrono Trigger");
    assert_eq!(game.details.genre, "RPG");
    assert_eq!(game.details.rating, Some(95));
    assert!(game.details.logo.is_some() && game.details.screenshot.is_some());
    assert!(game.video.is_some());
    // Each asset kind is fetched under its own size cap.
    let limits = http.limits.lock().unwrap();
    let cap = |end: &str| limits.iter().find(|(u, _)| u.ends_with(end)).unwrap().1;
    assert_eq!(cap("/m/box2d"), 8 << 20);
    assert_eq!(cap("/m/video"), 16 << 20);
    assert_eq!(cap("/m/manual"), 24 << 20);

    // English picks the English text.
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["Chrono.sfc"]);
    finish(&engine(mock(rich), &dir.path().join("c")).start(vec![sys.clone()], Options::new("en")));
    let xml = fs::read_to_string(sys.path.join("gamelist.xml")).unwrap();
    assert!(
        xml.contains("<desc>Time travel.</desc>") && xml.contains("<genre>Role playing</genre>")
    );
}

#[test]
fn error_text_served_as_video_or_manual_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["Chrono.sfc"]);
    let http = mock(|url| {
        if url.ends_with("/m/video") || url.ends_with("/m/manual") {
            Some((200, b"Erreur : media introuvable".to_vec()))
        } else {
            rich(url)
        }
    });
    let mut opts = Options::new("en");
    opts.media = vec![Media::Video, Media::Manual];
    let (_, report) = finish(&engine(http, &dir.path().join("c")).start(vec![sys.clone()], opts));
    assert_eq!(report.progress.failed, 1);
    assert!(!sys.path.join("media/videos").exists());
    assert!(!sys.path.join("media/manuals").exists());
}

#[test]
fn bulk_runs_keep_user_edits_and_a_forced_single_game_run_replaces_them() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let sys = system(dir.path(), &["Chrono.sfc", "Other.sfc"]);
    fs::write(
        sys.path.join("gamelist.xml"),
        "<gameList><game><path>./Chrono.sfc</path><name>Mine</name><favorite>true</favorite></game></gameList>",
    )
    .unwrap();
    let mut opts = Options::new("en");
    opts.only_missing = false;
    finish(&engine(mock(rich), &cache).start(vec![sys.clone()], opts));
    let xml = fs::read_to_string(sys.path.join("gamelist.xml")).unwrap();
    assert!(xml.contains("<name>Mine</name>") && xml.contains("<desc>Time travel.</desc>"));

    // An interrupted bulk run leaves a journal that the single run must not touch.
    fs::write(cache.join("scraper-journal.txt"), "pending bulk run\n").unwrap();
    let http = mock(rich);
    let only = sys.path.join("Chrono.sfc");
    let opts = Options {
        game: Some(only.clone()),
        force: true,
        ..Options::new("en")
    };
    let (_, report) = finish(&engine(http.clone(), &cache).start(vec![sys.clone()], opts));
    assert_eq!((report.stop, report.progress.total), (Stop::Completed, 1));
    assert_eq!(report.progress.scraped, 1);
    let xml = fs::read_to_string(sys.path.join("gamelist.xml")).unwrap();
    assert!(xml.contains("<name>Chrono Trigger</name>") && !xml.contains("Mine"));
    assert!(xml.contains("<favorite>true</favorite>"));
    // The cover existed already; forced runs fetch it again.
    assert!(http
        .seen
        .lock()
        .unwrap()
        .iter()
        .any(|u| u.ends_with("/m/box2d")));
    assert_eq!(
        fs::read_to_string(cache.join("scraper-journal.txt")).unwrap(),
        "pending bulk run\n"
    );
}

#[test]
fn a_game_on_the_second_card_gets_its_art_and_gamelist_there() {
    let dir = tempfile::tempdir().unwrap();
    let mut sys = system(&dir.path().join("sd1"), &["Alpha.sfc"]);
    let sd2 = system(&dir.path().join("sd2"), &["Beta.sfc"]).path;
    sys.external = Some(sd2.clone());
    let (_, report) = finish(
        &engine(mock(happy), &dir.path().join("c")).start(vec![sys.clone()], Options::new("en")),
    );
    assert_eq!((report.stop, report.progress.scraped), (Stop::Completed, 2));
    for (root, stem) in [(&sys.path, "Alpha"), (&sd2, "Beta")] {
        assert!(root.join(format!("media/images/{stem}.png")).is_file());
        let xml = fs::read_to_string(root.join("gamelist.xml")).unwrap();
        assert!(xml.contains(&format!("<path>./{stem}.sfc</path>")), "{xml}");
        assert_eq!(xml.matches("<game>").count(), 1, "{xml}");
    }
}
