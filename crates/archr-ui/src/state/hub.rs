//! START hub navigation, separate from the systems and game list cursors.
use super::{App, Button, DataSource, Outcome, Screen, SessionAction};

impl App {
    /// Recent and Tools are offered only when they would list something.
    pub(crate) fn hub_tile_enabled(&self, data: &dyn DataSource, tile: usize) -> bool {
        match tile {
            0 => data.collection_len("@recent") > 0,
            2 => data
                .systems()
                .iter()
                .any(|s| s.name == "tools" && s.game_count > 0),
            _ => true,
        }
    }
    pub(super) fn handle_hub(&mut self, button: Button, data: &dyn DataSource) -> Outcome {
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
            Button::L | Button::R => {
                let forward = button == Button::R;
                if row == 0 {
                    // Focus skips tiles that are not offered.
                    let mut reachable = (0..3).filter(|&t| {
                        (t > tile) == forward && t != tile && self.hub_tile_enabled(data, t)
                    });
                    tile = if forward {
                        reachable.next()
                    } else {
                        reachable.next_back()
                    }
                    .unwrap_or(tile);
                } else if forward {
                    shortcut = (shortcut + 1).min(5);
                } else {
                    shortcut = shortcut.saturating_sub(1);
                }
            }
            Button::A if row == 0 => {
                self.screen = Screen::Systems;
                match tile {
                    0 => self.enter(Screen::Games {
                        system: "@recent".to_owned(),
                    }),
                    2 => self.enter(Screen::Games {
                        system: "tools".to_owned(),
                    }),
                    _ => {}
                }
                return Outcome::None;
            }
            Button::A => {
                return match shortcut {
                    0 => Outcome::Settings,
                    1 => Outcome::Updates,
                    2 => Outcome::ToggleWifi,
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
