//! A system's list by folder, and its search. Both are views over the real
//! list, built from each game's folder below the system's own: favorites,
//! keys and modules keep pointing at the real games, and nothing is read
//! from the card.

/// One folder of the list: its subfolders, then its own games.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Level {
    /// Subfolder names in alphabetical order, with the games each holds at
    /// any depth; a folder without games is never listed.
    pub folders: Vec<(String, usize)>,
    /// The folder's own games, as indices into the system's list, in its order.
    pub games: Vec<usize>,
}

impl Level {
    /// `dirs[i]` is game i's folder: "" at the system's root, "/" between names.
    pub fn of(dirs: &[&str], at: &str) -> Self {
        let mut folders: std::collections::BTreeMap<&str, usize> = Default::default();
        let mut games = Vec::new();
        for (game, dir) in dirs.iter().enumerate() {
            match below(dir, at) {
                Some("") => games.push(game),
                Some(rest) => {
                    *folders
                        .entry(rest.split('/').next().unwrap_or(rest))
                        .or_default() += 1
                }
                None => {}
            }
        }
        let mut folders: Vec<_> = folders
            .into_iter()
            .map(|(n, c)| (n.to_owned(), c))
            .collect();
        folders.sort_by_cached_key(|(name, _)| fold(name));
        Self { folders, games }
    }
    /// The whole list as one level: no folders, its `count` games in order.
    pub fn flat(count: usize) -> Self {
        Self {
            folders: Vec::new(),
            games: (0..count).collect(),
        }
    }
    /// Favorites above the other games, each group keeping its order.
    pub fn favorites_first(&mut self, favorite: impl Fn(usize) -> bool) {
        self.games.sort_by_key(|&game| !favorite(game));
    }
    pub fn len(&self) -> usize {
        self.folders.len() + self.games.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn folder(&self, row: usize) -> Option<&str> {
        self.folders.get(row).map(|(name, _)| name.as_str())
    }
    pub fn game(&self, row: usize) -> Option<usize> {
        self.games
            .get(row.checked_sub(self.folders.len())?)
            .copied()
    }
    pub fn folder_row(&self, name: &str) -> Option<usize> {
        self.folders.iter().position(|(n, _)| n == name)
    }
    pub fn game_row(&self, game: usize) -> Option<usize> {
        Some(self.folders.len() + self.games.iter().position(|g| *g == game)?)
    }
    /// The games below this folder, at any depth.
    pub fn total(&self) -> usize {
        self.folders.iter().map(|(_, n)| n).sum::<usize>() + self.games.len()
    }
    /// The row's place, from 1, among those games as listed: a folder's
    /// games first, so a folder counts from its first game.
    pub fn position(&self, row: usize) -> usize {
        let before: usize = self.folders.iter().take(row).map(|(_, n)| n).sum();
        before + row.saturating_sub(self.folders.len()) + 1
    }
}

/// The part of `dir` below `at`: "" for `at` itself, None outside it.
fn below<'a>(dir: &'a str, at: &str) -> Option<&'a str> {
    match dir.strip_prefix(at)? {
        rest if at.is_empty() || rest.is_empty() => Some(rest),
        rest => rest.strip_prefix('/'),
    }
}

/// The subfolder `name` of `at`.
pub fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}/{name}")
    }
}

/// The games below `at`, at any depth, in list order.
pub fn within<'a>(dirs: &'a [&str], at: &'a str) -> impl Iterator<Item = usize> + 'a {
    (0..dirs.len()).filter(move |&n| below(dirs[n], at).is_some())
}

/// Lower case without accents: "Pokémon" and "POKEMON" are the same.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è'..='ë' | 'ē' | 'ę' | 'ě' => 'e',
            'ì'..='ï' | 'ī' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            c => c,
        })
        .collect()
}

/// The games whose name holds `query`, in list order, wherever they lie.
pub fn search<'a>(names: impl Iterator<Item = &'a str>, query: &str) -> Vec<usize> {
    let query = fold(query.trim());
    names
        .enumerate()
        .filter(|(_, name)| fold(name).contains(&query))
        .map(|(n, _)| n)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRS: [&str; 7] = [
        "",
        "Hacks",
        "Hacks/Pokemon",
        "",
        "hacks2",
        "Hacks",
        "Zeta/A/B",
    ];

    #[test]
    fn a_level_lists_its_folders_in_order_then_its_games() {
        let root = Level::of(&DIRS, "");
        let names: Vec<_> = root.folders.iter().map(|(n, c)| (n.as_str(), *c)).collect();
        assert_eq!(names, [("Hacks", 3), ("hacks2", 1), ("Zeta", 1)]);
        assert_eq!(root.games, [0, 3]);
        assert_eq!((root.len(), root.total()), (5, 7));
        assert_eq!(
            (root.folder(1), root.game(1), root.game(3)),
            (Some("hacks2"), None, Some(0))
        );
        let hacks = Level::of(&DIRS, "Hacks");
        assert_eq!(hacks.folders, [("Pokemon".to_owned(), 1)]);
        assert_eq!(hacks.games, [1, 5]);
        // "Hacks" is not a prefix of "hacks2"'s path, nor of a deeper name.
        assert_eq!(Level::of(&["Hacks2"], "Hacks"), Level::default());
        let zeta = Level::of(&DIRS, "Zeta");
        assert_eq!((zeta.folders.len(), zeta.games.len()), (1, 0));
    }

    #[test]
    fn folders_without_games_never_show_and_rows_map_back() {
        let level = Level::of(&["", "A/B/C"], "A");
        assert_eq!(level.folders, [("B".to_owned(), 1)]);
        assert!(Level::of(&["", ""], "Empty").is_empty());
        let root = Level::of(&DIRS, "");
        assert_eq!(root.folder_row("Zeta"), Some(2));
        assert_eq!(root.game_row(3), Some(4));
        assert_eq!(root.game_row(1), None);
        // Positions count the real games: Hacks holds 3, hacks2 1, Zeta 1.
        let places: Vec<_> = (0..root.len()).map(|r| root.position(r)).collect();
        assert_eq!(places, [1, 4, 5, 6, 7]);
        assert_eq!(within(&DIRS, "Hacks").collect::<Vec<_>>(), [1, 2, 5]);
        assert_eq!(within(&DIRS, "").count(), DIRS.len());
        assert_eq!(join("", "A"), "A");
        assert_eq!(join("A", "B"), "A/B");
    }

    #[test]
    fn search_ignores_case_and_accents() {
        let names = ["Pokémon Ruby", "Mega Man", "POKEMON Fire Red", "Pokedex"];
        assert_eq!(search(names.into_iter(), "pokem"), [0, 2]);
        assert_eq!(search(names.into_iter(), "  MAN "), [1]);
        assert_eq!(search(names.into_iter(), ""), [0, 1, 2, 3]);
        assert_eq!(fold("Ação Çé"), "acao ce");
    }
}
