//! Engine behaviour against a mocked HTTP layer: no test touches the network.
use archr_index::{scan_system, SystemDef};
use archr_scraper::{
    Credentials, Engine, Event, Http, Job, Md5, Media, NetError, Options, Report, Response, Stop,
    Timing,
};
use std::{
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::{Duration, Instant},
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";
const DEV: &str = "devid=tester&devpassword=hunter2";

type Handler = dyn Fn(&str) -> Option<(u16, Vec<u8>)> + Send + Sync;
struct Mock {
    handler: Box<Handler>,
    seen: Mutex<Vec<String>>,
}
impl Http for Mock {
    fn get(&self, url: &str, _limit: u64, _cancel: &AtomicBool) -> Result<Response, NetError> {
        self.seen.lock().unwrap().push(url.to_owned());
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
    b"<Data><ssuser><maxthreads>1</maxthreads><requeststoday>5</requeststoday><maxrequestsperday>20000</maxrequestsperday></ssuser></Data>".to_vec()
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
        Some((200, PNG.to_vec()))
    } else {
        Some((404, b"not found".to_vec()))
    }
}

#[test]
fn md5_matches_rfc_vectors() {
    let hex = |text: &[u8]| {
        let mut md5 = Md5::new();
        md5.update(text);
        md5.finish()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    let long = vec![b'a'; 1000];
    assert_eq!(hex(&long), "cabe45dcc9ae5b66ba86600cca6b8ba8");
}

#[test]
fn scrapes_missing_art_into_the_media_layout_the_library_reads() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(
        &dir.path().join("roms"),
        &["Alpha (USA).sfc", "Beta.sfc", "Done.sfc"],
    );
    fs::create_dir_all(sys.path.join("media/images")).unwrap();
    fs::write(sys.path.join("media/images/Done.png"), PNG).unwrap();
    let http = mock(happy);
    let job = engine(http.clone(), &dir.path().join("cache")).start(
        vec![sys.clone()],
        Options {
            language: "pt-BR".into(),
            ..Options::new("pt-BR")
        },
    );
    let (events, report) = finish(&job);
    assert_eq!(report.stop, Stop::Completed);
    assert_eq!((report.progress.total, report.progress.skipped), (2, 1));
    assert_eq!((report.progress.done, report.progress.scraped), (2, 2));
    assert_eq!(events.last().unwrap().done, 2);
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
fn video_is_opt_in_and_html_error_pages_are_not_saved_as_art() {
    let dir = tempfile::tempdir().unwrap();
    let sys = system(dir.path(), &["A.sfc"]);
    let http = mock(|url| {
        if url.contains("jeuInfos.php") {
            let xml = "<Data><jeu id=\"1\"><medias>\
<media type=\"box-2D\" region=\"us\">https://www.screenscraper.fr/m/box</media>\
<media type=\"video\" region=\"\">https://www.screenscraper.fr/m/video</media></medias></jeu></Data>";
            Some((200, xml.as_bytes().to_vec()))
        } else if url.contains("/m/box") {
            Some((200, b"CRCOK: error text".to_vec()))
        } else if url.contains("/m/video") {
            Some((200, b"\0\0\0\x20ftypisom".to_vec()))
        } else {
            happy(url)
        }
    });
    let mut opts = Options::new("en");
    opts.media = vec![Media::Box2d, Media::Video];
    let (_, report) = finish(&engine(http, dir.path()).start(vec![sys.clone()], opts));
    assert_eq!(report.progress.scraped, 1);
    assert!(!sys.path.join("media/images/A.png").exists());
    assert!(sys.path.join("media/videos/A.mp4").is_file());
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
}

#[test]
fn verify_reports_the_account_limits() {
    let dir = tempfile::tempdir().unwrap();
    let info = engine(mock(happy), dir.path()).verify().unwrap();
    assert_eq!((info.threads, info.max_requests_per_day), (1, 20000));
    let denied = engine(mock(|_| Some((403, vec![]))), dir.path()).verify();
    assert_eq!(denied, Err(Stop::Auth));
}
