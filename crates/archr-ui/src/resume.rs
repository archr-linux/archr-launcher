//! The switcher (Alternador) and the resume at power on (Retomar-boot):
//! the games the host found and their buttons. The host fills them from
//! the recent games and archr-game-save-exit's records.
use crate::state::{App, Button, Outcome, Screen};
use std::path::PathBuf;

/// A recent game as the switcher and the resume screen show it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recent {
    /// The real system and the game's index in it.
    pub system: String,
    pub game: usize,
    pub name: String,
    /// The screen when it was left, else its screenshot or cover.
    pub picture: Option<PathBuf>,
    /// When it was left by "save and exit", with or without a state.
    pub left: Option<u64>,
    /// runemu.sh's argument to start from the saved state; None: the game
    /// starts as usual (Sem retomar).
    pub entry: Option<String>,
}

/// The resume screen at power on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boot {
    pub game: Recent,
    /// Found in the library: A plays, the countdown runs.
    pub ready: bool,
    /// Left by a power off (battery, a long suspend), not by the switcher.
    pub power_off: bool,
    /// The countdown, 0 to 1, set by the host.
    pub progress: f32,
}

impl App {
    fn resume(game: &Recent) -> Outcome {
        Outcome::Continue {
            system: game.system.clone(),
            game: game.game,
            entry: game.entry.clone(),
        }
    }
    pub(crate) fn handle_switcher(&mut self, b: Button) -> Outcome {
        let last = self.switcher.len().saturating_sub(1);
        self.cursor = self.cursor.min(last);
        match b {
            Button::L | Button::Up => self.cursor = self.cursor.saturating_sub(1),
            Button::R | Button::Down => self.cursor = (self.cursor + 1).min(last),
            Button::A => {
                if let Some(game) = self.switcher.get(self.cursor) {
                    return Self::resume(game);
                }
            }
            Button::X if !self.switcher.is_empty() => {
                let game = self.switcher.remove(self.cursor);
                self.cursor = self.cursor.min(self.switcher.len().saturating_sub(1));
                return Outcome::ForgetRecent {
                    system: game.system,
                    game: game.game,
                };
            }
            // Back to the game just left, even when taken off the list.
            Button::B => match self.switcher_left.clone() {
                Some(game) => return Self::resume(&game),
                None => self.leave_switcher(),
            },
            Button::Start | Button::Select => self.leave_switcher(),
            _ => {}
        }
        Outcome::None
    }
    fn leave_switcher(&mut self) {
        self.screen = Screen::Systems;
        self.cursor = 0;
        self.switcher.clear();
        self.switcher_left = None;
    }
    pub(crate) fn handle_resume(&mut self, b: Button) -> Outcome {
        match (b, &self.boot) {
            (Button::A, Some(boot)) if boot.ready => Self::resume(&boot.game),
            (Button::B, _) => {
                self.boot = None;
                self.screen = Screen::Systems;
                self.cursor = 0;
                Outcome::SkipResume
            }
            _ => Outcome::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(n: usize, entry: Option<&str>) -> Recent {
        Recent {
            system: "snes".into(),
            game: n,
            name: format!("Game {n}"),
            entry: entry.map(Into::into),
            ..Recent::default()
        }
    }
    fn switcher() -> App {
        let mut app = App::new(5);
        app.screen = Screen::Switcher;
        app.switcher = vec![
            game(0, Some("--entryslot=auto")),
            game(1, None),
            game(2, None),
        ];
        app.switcher_left = app.switcher.first().cloned();
        app
    }

    #[test]
    fn switcher_moves_continues_and_forgets() {
        let mut app = switcher();
        assert_eq!(app.handle_switcher(Button::L), Outcome::None);
        assert_eq!(app.cursor, 0);
        app.handle_switcher(Button::R);
        app.handle_switcher(Button::R);
        app.handle_switcher(Button::R);
        assert_eq!(app.cursor, 2);
        assert_eq!(
            app.handle_switcher(Button::A),
            Outcome::Continue {
                system: "snes".into(),
                game: 2,
                entry: None
            }
        );
        assert_eq!(
            app.handle_switcher(Button::X),
            Outcome::ForgetRecent {
                system: "snes".into(),
                game: 2
            }
        );
        assert_eq!((app.switcher.len(), app.cursor), (2, 1));
    }

    #[test]
    fn b_goes_back_to_the_game_left_even_off_the_list() {
        let mut app = switcher();
        app.handle_switcher(Button::X);
        assert_eq!(
            app.handle_switcher(Button::B),
            Outcome::Continue {
                system: "snes".into(),
                game: 0,
                entry: Some("--entryslot=auto".into())
            }
        );
        let mut app = switcher();
        app.switcher_left = None;
        assert_eq!(app.handle_switcher(Button::B), Outcome::None);
        assert_eq!(app.screen, Screen::Systems);
    }

    #[test]
    fn resume_plays_once_found_and_b_goes_to_the_menu() {
        let mut app = App::new(5);
        app.screen = Screen::Resume;
        app.boot = Some(Boot {
            game: game(4, Some("--entrystate=/s.st0")),
            ..Boot::default()
        });
        assert_eq!(app.handle_resume(Button::A), Outcome::None);
        app.boot.as_mut().unwrap().ready = true;
        assert!(matches!(
            app.handle_resume(Button::A),
            Outcome::Continue { game: 4, .. }
        ));
        assert_eq!(app.handle_resume(Button::B), Outcome::SkipResume);
        assert_eq!(
            (app.screen.clone(), app.boot.is_none()),
            (Screen::Systems, true)
        );
    }
}
