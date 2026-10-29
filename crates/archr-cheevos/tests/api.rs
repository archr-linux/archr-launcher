//! The client against canned replies: no test reaches the service.
use archr_cheevos::{Client, Error};
use archr_scraper::{Http, NetError, Response};
use std::sync::{atomic::AtomicBool, Arc, Mutex};

type Reply = Box<dyn Fn(&str, &str) -> Result<(u16, Vec<u8>), NetError> + Send + Sync>;

#[derive(Clone)]
struct Fake {
    seen: Arc<Mutex<Vec<(String, String)>>>,
    reply: Arc<Reply>,
}
impl Http for Fake {
    fn get(&self, url: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
        self.post(url, "", 0, &AtomicBool::new(false))
    }
    fn post(&self, url: &str, form: &str, _: u64, _: &AtomicBool) -> Result<Response, NetError> {
        self.seen.lock().unwrap().push((url.into(), form.into()));
        (self.reply)(url, form).map(|(status, body)| Response { status, body })
    }
}

fn serve(
    reply: impl Fn(&str, &str) -> Result<(u16, Vec<u8>), NetError> + Send + Sync + 'static,
) -> (Client, Fake) {
    let fake = Fake {
        seen: Arc::default(),
        reply: Arc::new(Box::new(reply)),
    };
    (Client::new(Box::new(fake.clone())), fake)
}

fn json(status: u16, body: &str) -> Result<(u16, Vec<u8>), NetError> {
    Ok((status, body.as_bytes().to_vec()))
}

static NEVER: AtomicBool = AtomicBool::new(false);

#[test]
fn login_posts_the_password_once_and_keeps_only_the_token() {
    let (client, fake) = serve(|_, _| {
        json(
            200,
            r#"{"Success":true,"User":"Douglas","Token":"tok123","Score":1240,"SoftcoreScore":35,"Messages":0}"#,
        )
    });
    let account = client.login(" douglas ", "p&ss word", &NEVER).unwrap();
    assert_eq!(
        (account.user.as_str(), account.token.as_str()),
        ("Douglas", "tok123")
    );
    assert_eq!((account.points, account.softcore), (1240, 35));
    let seen = fake.seen.lock().unwrap();
    assert_eq!(seen[0].0, "https://retroachievements.org/dorequest.php");
    assert_eq!(seen[0].1, "r=login2&u=douglas&p=p%26ss%20word");
    // The token never shows in a debug print.
    assert!(!format!("{account:?}").contains("tok123"));
}

#[test]
fn rejected_logins_and_tokens_are_auth_errors() {
    let (client, _) = serve(|_, _| {
        json(
            401,
            r#"{"Success":false,"Status":401,"Code":"invalid_credentials","Error":"Invalid user/password combination."}"#,
        )
    });
    assert_eq!(client.login("u", "p", &NEVER), Err(Error::Auth));
    let (client, _) = serve(|_, _| {
        json(
            200,
            r#"{"Success":false,"Code":"expired_token","Error":"The access token has expired."}"#,
        )
    });
    assert_eq!(client.refresh("u", "t", &NEVER), Err(Error::Auth));
    let (client, _) = serve(|_, _| Err(NetError::Timeout));
    assert_eq!(client.login("u", "p", &NEVER), Err(Error::Network));
    let (client, _) = serve(|_, _| json(200, "<html>maintenance</html>"));
    assert_eq!(client.login("u", "p", &NEVER), Err(Error::Service));
    // A reply for another user or without a token is not a login.
    let (client, _) = serve(|_, _| json(200, r#"{"Success":true,"User":"Other","Token":"x"}"#));
    assert_eq!(client.login("u", "p", &NEVER), Err(Error::Service));
    assert_eq!(Error::Auth.key(), "cheevos.bad_login");
}

const PATCH: &str = r#"{"Success":true,"PatchData":{"ID":228,"Title":"Super Mario World","Achievements":[
 {"ID":1,"Title":"Começo de jornada","Description":"Termine a primeira fase","Points":5,"BadgeName":"00123","Flags":3},
 {"ID":2,"Title":"Caça\u0007 às moedas","Description":"Junte 100 moedas","Points":10,"BadgeName":"../../x","Flags":3},
 {"ID":3,"Title":"Teste","Description":"Não oficial","Points":0,"BadgeName":"9","Flags":5},
 {"ID":4,"Title":"Mundo das estrelas","Description":"Abra a Estrada Estelar","Points":"25","BadgeName":"00456","Flags":3}]}}"#;

#[test]
fn a_game_set_comes_with_the_users_unlocks_and_without_unofficial_ones() {
    let (client, fake) = serve(|_, form| {
        if form.starts_with("r=patch") {
            json(200, PATCH)
        } else {
            json(
                200,
                r#"{"Success":true,"UserUnlocks":[2,"4"],"GameID":228,"HardcoreMode":true}"#,
            )
        }
    });
    let game = client
        .game(
            ("douglas", "tok"),
            "0123456789abcdef0123456789abcdef",
            true,
            &NEVER,
        )
        .unwrap();
    assert_eq!((game.id, game.title.as_str()), (228, "Super Mario World"));
    assert!(game.hardcore);
    let ids: Vec<_> = game
        .achievements
        .iter()
        .map(|a| (a.id, a.unlocked))
        .collect();
    assert_eq!(ids, [(1, false), (2, true), (4, true)]);
    assert_eq!(game.achievements[1].title, "Caça  às moedas");
    assert_eq!(game.achievements[2].points, 25);
    // A badge name that is not digits never becomes a URL.
    assert_eq!(game.achievements[1].badge, "");
    let seen = fake.seen.lock().unwrap();
    assert_eq!(
        seen[0].1,
        "r=patch&u=douglas&t=tok&m=0123456789abcdef0123456789abcdef"
    );
    assert_eq!(seen[1].1, "r=unlocks&u=douglas&t=tok&g=228&h=1");
    assert!(seen.iter().all(|(url, _)| !url.contains("tok")));
}

#[test]
fn unknown_games_are_not_found() {
    let (client, _) = serve(|_, _| json(200, r#"{"Success":true,"PatchData":{"ID":0}}"#));
    assert_eq!(
        client.game(("u", "t"), "00", false, &NEVER),
        Err(Error::NotFound)
    );
    let (client, _) = serve(|_, _| {
        json(
            404,
            r#"{"Success":false,"Code":"not_found","Error":"Unknown game"}"#,
        )
    });
    assert_eq!(
        client.game(("u", "t"), "00", false, &NEVER),
        Err(Error::NotFound)
    );
}

fn png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(4, 4)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

#[test]
fn badges_must_be_images_from_the_media_server() {
    let (client, fake) = serve(|url, _| {
        if url.ends_with("00123_lock.png") {
            Ok((200, png()))
        } else {
            json(200, "<html>not an image</html>")
        }
    });
    assert_eq!(client.badge("00123", true, &NEVER), Ok(png()));
    assert_eq!(
        fake.seen.lock().unwrap()[0].0,
        "https://media.retroachievements.org/Badge/00123_lock.png"
    );
    assert_eq!(client.badge("00123", false, &NEVER), Err(Error::Service));
    assert_eq!(client.badge("../1", false, &NEVER), Err(Error::Service));
    assert_eq!(fake.seen.lock().unwrap().len(), 2);
}

#[test]
fn the_profile_summary_needs_the_developer_login() {
    let (client, fake) = serve(|_, _| {
        json(
            200,
            r#"{"MemberSince":"2021-03-04 10:00:00","RecentlyPlayed":[{"Title":"Super Mario World"}]}"#,
        )
    });
    assert_eq!(client.summary_as("bad", "douglas", &NEVER), None);
    let summary = client.summary_as("z=dev&y=key", "douglas", &NEVER).unwrap();
    assert_eq!(summary.since, "2021");
    assert_eq!(summary.last_game, "Super Mario World");
    assert_eq!(
        fake.seen.lock().unwrap()[0].0,
        "https://retroachievements.org/API/API_GetUserSummary.php?z=dev&y=key&u=douglas&g=1&a=0"
    );
}
