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
