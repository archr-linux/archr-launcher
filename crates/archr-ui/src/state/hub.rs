//! START hub navigation, separate from the systems and game list cursors.
use super::{App, Button, DataSource, Outcome, Screen, SessionAction};
use crate::modules::{Request, Target};

/// Recent, Games and Tools; module tiles follow.
const TILES: usize = 3;
/// Settings, Update and Wi-Fi come before the module shortcuts.
const BEFORE: usize = 3;

impl App {
    /// What choosing a module's tile or shortcut asks for.
    fn module_target(&self, module: usize, target: &Target) -> Outcome {
        let module = self.modules.modules[module].id.clone();
        Outcome::Module(match target {
            Target::Page(page) => Request::Open {
                module,
                page: *page,
            },
            Target::Action(action) => Request::Invoke {
                module,
                action: action.clone(),
                call: None,
            },
        })
    }
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
    /// What A does on built-in shortcut `index` (module shortcuts not
    /// counted); None for one not offered yet.
    fn shortcut(index: usize) -> Outcome {
        match index {
            0 => Outcome::Settings,
            1 => Outcome::Updates,
            2 => Outcome::ToggleWifi,
            3 => Outcome::Action(SessionAction::Suspend),
            4 => Outcome::Action(SessionAction::Reboot),
            5 => Outcome::Action(SessionAction::Shutdown),
            _ => Outcome::None,
        }
    }
    /// Shortcuts that do something are drawn enabled, the rest dimmed.
    pub(crate) fn hub_shortcut_enabled(index: usize) -> bool {
        !matches!(Self::shortcut(index), Outcome::None)
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
        // Rows: 0 tiles, 2 module widgets (between, when there are any),
        // 1 shortcuts.
        let widgets = self.modules.widgets();
        match button {
            Button::Start | Button::B => self.screen = Screen::Systems,
            Button::Up => {
                row = if row == 1 && !widgets.is_empty() {
                    2
                } else {
                    0
                }
            }
            Button::Down => {
                row = if row == 0 && !widgets.is_empty() {
                    2
                } else {
                    1
                }
            }
            Button::L | Button::R if row == 2 => {
                let last = widgets.len().saturating_sub(1);
                self.hub_widget = if button == Button::R {
                    (self.hub_widget + 1).min(last)
                } else {
                    self.hub_widget.saturating_sub(1)
                };
            }
            Button::A if row == 2 => {
                let Some((module, entry)) =
                    widgets.get(self.hub_widget.min(widgets.len().saturating_sub(1)))
                else {
                    return Outcome::None;
                };
                return self.module_target(*module, &entry.target);
            }
            Button::L | Button::R => {
                let forward = button == Button::R;
                if row == 0 {
                    // Focus skips tiles that are not offered.
                    let mut reachable = (0..TILES + self.modules.tiles().len()).filter(|&t| {
                        (t > tile) == forward && t != tile && self.hub_tile_enabled(data, t)
                    });
                    tile = if forward {
                        reachable.next()
                    } else {
                        reachable.next_back()
                    }
                    .unwrap_or(tile);
                } else if forward {
                    shortcut = (shortcut + 1).min(5 + self.modules.shortcuts().len());
                } else {
                    shortcut = shortcut.saturating_sub(1);
                }
            }
            Button::A if row == 0 && tile >= TILES => {
                let tiles = self.modules.tiles();
                let Some((module, entry)) = tiles.get(tile - TILES) else {
                    return Outcome::None;
                };
                return self.module_target(*module, &entry.target);
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
                let extra = self.modules.shortcuts();
                if let Some((module, entry)) =
                    shortcut.checked_sub(BEFORE).and_then(|n| extra.get(n))
                {
                    return self.module_target(*module, &entry.target);
                }
                let shortcut = if shortcut >= BEFORE {
                    shortcut - extra.len()
                } else {
                    shortcut
                };
                return Self::shortcut(shortcut);
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
