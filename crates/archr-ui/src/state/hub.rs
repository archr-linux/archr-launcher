//! START hub navigation, separate from the systems and game list cursors.
use super::{App, Button, DataSource, Outcome, Screen, SessionAction};

impl App {
    pub(super) fn handle_hub(
        &mut self,
        button: Button,
        data: &dyn DataSource,
        wifi: bool,
    ) -> Outcome {
        let Screen::StartHub {
            mut row,
            mut tile,
            mut shortcut,
        } = self.screen
        else {
            unreachable!()
        };
        match button {
            Button::Start | Button::B => self.screen = Screen::Systems,
            Button::Up => row = 0,
            Button::Down => row = 1,
            Button::L => {
                if row == 0 {
                    tile = tile.saturating_sub(1);
                } else {
                    shortcut = shortcut.saturating_sub(1);
                }
            }
            Button::R => {
                if row == 0 {
                    tile = (tile + 1).min(2);
                } else {
                    shortcut = (shortcut + 1).min(5);
                }
            }
            Button::A if row == 0 => {
                let system = match tile {
                    0 => "@recent",
                    1 => {
                        self.screen = Screen::Systems;
                        return Outcome::None;
                    }
                    2 if data.systems().iter().any(|s| s.name == "tools") => "tools",
                    _ => return Outcome::None,
                };
                self.screen = Screen::Systems;
                self.enter(Screen::Games {
                    system: system.to_owned(),
                });
                return Outcome::None;
            }
            Button::A => {
                return match shortcut {
                    2 => Outcome::ToggleWifi(!wifi),
                    3 => Outcome::Action(SessionAction::Suspend),
                    4 => Outcome::Action(SessionAction::Reboot),
                    5 => Outcome::Action(SessionAction::Shutdown),
                    _ => Outcome::None,
                }
            }
            _ => {}
        }
        if matches!(self.screen, Screen::StartHub { .. }) {
            self.screen = Screen::StartHub {
                row,
                tile,
                shortcut,
            };
        }
        Outcome::None
    }
}
