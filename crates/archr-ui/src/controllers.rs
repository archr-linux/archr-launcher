//! Open devices in SDL joystick index order, retaining handles for event delivery.
//! Mappings come only from SDL's GameController database, which the session
//! points at the distribution's gamecontrollerdb.txt.
use sdl2::event::Event;
use sdl2::{
    controller::GameController, joystick::Joystick, GameControllerSubsystem, JoystickSubsystem,
};

use crate::{
    pad::{Pad, Remap},
    startup::StartupTrace,
    startup_phase,
    state::Button,
};

struct Device {
    index: u32,
    guid: String,
    joystick: Joystick,
    controller: Option<GameController>,
}

pub struct Controllers {
    joysticks: JoystickSubsystem,
    controllers: GameControllerSubsystem,
    devices: Vec<Device>,
    remap: Remap,
}
impl Controllers {
    pub fn new(sdl: &sdl2::Sdl) -> Result<Self, String> {
        Self::new_traced(sdl, StartupTrace::default())
    }
    pub fn new_traced(sdl: &sdl2::Sdl, trace: StartupTrace) -> Result<Self, String> {
        let joysticks = sdl.joystick()?;
        startup_phase!(trace, "SDL joystick init");
        let controllers = sdl.game_controller()?;
        startup_phase!(trace, "SDL gamecontroller init");
        let mut this = Self {
            joysticks,
            controllers,
            devices: Vec::new(),
            remap: Remap::default(),
        };
        this.refresh();
        Ok(this)
    }
    /// The Controls page's choices; applied from the next event on.
    pub fn set_remap(&mut self, remap: Remap) {
        self.remap = remap;
    }
    /// Whether this event belongs to the stream used for its device: a
    /// GameController delivers both streams; use only its mapped one.
    fn routed(&self, event: &Event) -> bool {
        let Some((which, raw)) = crate::input::device_event(event) else {
            return true;
        };
        self.devices
            .iter()
            .find(|d| d.joystick.instance_id() == which)
            .is_some_and(|device| raw == device.controller.is_none())
    }
    /// None ignores a duplicate/unknown stream; Some(None) releases an input.
    pub(crate) fn read(&self, event: &Event) -> Option<Option<Button>> {
        self.routed(event).then(|| self.remap.map(event))
    }
    /// The control a fresh press comes from, for "press a button" prompts.
    pub fn pressed(&self, event: &Event) -> Option<Pad> {
        Pad::pressed(event).filter(|_| self.routed(event))
    }

    pub fn refresh(&mut self) {
        self.devices.clear();
        for index in 0..self.joysticks.num_joysticks().unwrap_or(0) {
            if self.devices.len() == 4 {
                break;
            }
            match self.joysticks.open(index) {
                Ok(joystick) => {
                    let controller = self
                        .controllers
                        .is_game_controller(index)
                        .then(|| self.controllers.open(index).ok())
                        .flatten();
                    self.devices.push(Device {
                        index,
                        guid: joystick.guid().to_string(),
                        joystick,
                        controller,
                    });
                }
                Err(error) => log::warn!("Cannot open joystick {index}: {error}"),
            }
        }
    }
    pub fn configuration(&self) -> String {
        controller_arguments(
            self.devices
                .iter()
                .map(|device| (device.index, device.guid.clone())),
        )
    }
}

/// Matches ES configureEmulators, including the spaces around each player.
pub fn controller_arguments(devices: impl IntoIterator<Item = (u32, String)>) -> String {
    devices
        .into_iter()
        .take(4)
        .enumerate()
        .map(|(player, (index, guid))| {
            let player = player + 1;
            format!(" -p{player}index {index} -p{player}guid {guid} ")
        })
        .collect()
}
