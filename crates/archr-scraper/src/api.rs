//! Typed, strict views of the ScreenScraper XML replies.
use serde::Deserialize;

#[derive(Deserialize)]
struct Data {
    ssuser: Option<User>,
    jeu: Option<Game>,
    jeux: Option<Games>,
}

#[derive(Deserialize)]
pub struct User {
    #[serde(default)]
    pub id: String,
    #[serde(default, deserialize_with = "number")]
    pub maxthreads: Option<i64>,
    #[serde(default, deserialize_with = "number")]
    pub maxrequestspermin: Option<i64>,
    #[serde(default, deserialize_with = "number")]
    pub requeststoday: Option<i64>,
    #[serde(default, deserialize_with = "number")]
    pub maxrequestsperday: Option<i64>,
}

#[derive(Deserialize)]
pub struct Game {
    #[serde(rename = "@id")]
    id: Option<String>,
    medias: Option<Medias>,
    noms: Option<Noms>,
    synopsis: Option<Synopses>,
    dates: Option<Dates>,
    genres: Option<Genres>,
    developpeur: Option<Text>,
    editeur: Option<Text>,
    joueurs: Option<Text>,
    note: Option<Text>,
}

#[derive(Deserialize)]
struct Noms {
    #[serde(default)]
    nom: Vec<Text>,
}
#[derive(Deserialize)]
struct Synopses {
    #[serde(default)]
    synopsis: Vec<Text>,
}
#[derive(Deserialize)]
struct Dates {
    #[serde(default)]
    date: Vec<Text>,
}
#[derive(Deserialize)]
struct Genres {
    #[serde(default)]
    genre: Vec<Text>,
}

#[derive(Deserialize, Default)]
struct Text {
    #[serde(rename = "@region", default)]
    region: String,
    #[serde(rename = "@langue", default)]
    langue: String,
    #[serde(rename = "@principale", default)]
    principale: String,
    #[serde(rename = "$text", default)]
    text: String,
}

/// Text metadata in ES's gamelist format, chosen for the launcher language.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub name: String,
    pub desc: String,
    /// `YYYYMMDDT000000`
    pub releasedate: String,
    pub players: String,
    /// 0-1, ES style (`0.85`).
    pub rating: String,
    pub genre: String,
    pub developer: String,
    pub publisher: String,
}

#[derive(Deserialize)]
struct Games {
    #[serde(default)]
    jeu: Vec<Game>,
}

#[derive(Deserialize)]
struct Medias {
    #[serde(default)]
    media: Vec<Media>,
}

#[derive(Deserialize)]
pub struct Media {
    #[serde(rename = "@type")]
    pub kind: String,
    #[serde(rename = "@region", default)]
    pub region: String,
    #[serde(rename = "$text", default)]
    pub url: String,
}

/// Empty or non-numeric counters read as absent rather than failing the reply.
fn number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.and_then(|text| text.trim().parse().ok()))
}

fn data(body: &[u8]) -> Option<Data> {
    quick_xml::de::from_reader(body).ok()
}

/// The account block; `None` for anything else (maintenance pages, truncated
/// XML, replies without a user).
pub fn account(body: &[u8]) -> Option<User> {
    data(body)?.ssuser
}

/// `jeuInfos` reply: the identified game.
pub fn game(body: &[u8]) -> Option<Game> {
    data(body)?.jeu.filter(|game| game.id.is_some())
}

/// `jeuRecherche` reply: first real hit (empty `<jeu/>` means no match).
pub fn search(body: &[u8]) -> Option<Game> {
    data(body)?
        .jeux?
        .jeu
        .into_iter()
        .find(|game| game.id.is_some())
}

impl Game {
    pub fn medias(&self) -> &[Media] {
        self.medias.as_ref().map_or(&[], |m| &m.media)
    }
}

impl Game {
    /// `lang` is a two-letter language (`pt`), `regions` the art preference.
    pub fn meta(&self, lang: &str, regions: &[&str]) -> Meta {
        let clean = |text: &str| text.replace("&nbsp;", " ").trim().to_owned();
        let by = |items: Vec<&Text>, keys: &[&str], key: fn(&Text) -> &str| {
            keys.iter()
                .find_map(|k| items.iter().find(|t| key(t) == *k))
                .or(items.first())
                .map(|t| clean(&t.text))
                .unwrap_or_default()
        };
        let names = self.noms.iter().flat_map(|n| &n.nom).collect();
        let texts = self.synopsis.iter().flat_map(|n| &n.synopsis).collect();
        let dates = self.dates.iter().flat_map(|n| &n.date).collect();
        let genres: Vec<&Text> = self.genres.iter().flat_map(|n| &n.genre).collect();
        let genre = [(lang, "1"), ("en", "1"), (lang, ""), ("en", "")]
            .iter()
            .find_map(|(l, main)| {
                genres
                    .iter()
                    .find(|g| g.langue == *l && (main.is_empty() || g.principale == *main))
            })
            .map(|g| clean(&g.text))
            .unwrap_or_default();
        let note = self
            .note
            .as_ref()
            .and_then(|n| n.text.trim().parse::<u32>().ok());
        let field = |t: &Option<Text>| t.as_ref().map(|t| clean(&t.text)).unwrap_or_default();
        Meta {
            name: by(names, regions, |t| &t.region),
            desc: by(texts, &[lang, "en", "wor"], |t| &t.langue),
            releasedate: es_date(&by(dates, regions, |t| &t.region)),
            players: field(&self.joueurs),
            rating: note
                .filter(|n| (1..=20).contains(n))
                .map(|n| format!("{}", n as f32 / 20.))
                .unwrap_or_default(),
            genre,
            developer: field(&self.developpeur),
            publisher: field(&self.editeur),
        }
    }
}

/// `1995-03-11`, `1995-03` or `1995` as ES's `19950311T000000`.
fn es_date(date: &str) -> String {
    let digits = |p: &str, n: usize| p.len() == n && p.bytes().all(|b| b.is_ascii_digit());
    let mut parts = date.trim().split('-');
    let year = parts.next().filter(|y| digits(y, 4));
    let month = parts.next().filter(|m| digits(m, 2)).unwrap_or("01");
    let day = parts.next().filter(|d| digits(d, 2)).unwrap_or("01");
    year.map_or_else(String::new, |year| format!("{year}{month}{day}T000000"))
}
