//! Open devices in SDL joystick index order, retaining handles for event delivery.
use archr_io::{read_utf8, Symlinks::Follow};
use sdl2::event::Event;
use sdl2::{
    controller::GameController, joystick::Joystick, GameControllerSubsystem, JoystickSubsystem,
};

use crate::{joystick_mapping::RawMappings, startup::StartupTrace, startup_phase, state::Button};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

struct Device {
    index: u32,
    guid: String,
    name: String,
    joystick: Joystick,
    controller: Option<GameController>,
}

pub struct Controllers {
    joysticks: JoystickSubsystem,
    controllers: GameControllerSubsystem,
    devices: Vec<Device>,
    mappings: RawMappings,
    loading: Option<Receiver<RawMappings>>,
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
            mappings: RawMappings::default(),
            loading: None,
        };
        this.refresh();
        Ok(this)
    }
    pub fn load_mappings(&mut self, path: PathBuf) {
        let (done, ready) = mpsc::channel();
        self.loading = Some(ready);
        std::thread::spawn(move || {
            let read = || -> Result<RawMappings, String> {
                let text =
                    read_utf8(&path, 1024 * 1024, Follow).map_err(|error| error.to_string())?;
                RawMappings::parse(&text)
            };
            let mappings = read().unwrap_or_else(|error| {
                log::warn!("Joystick mappings: {error}");
                RawMappings::default()
            });
            let _ = done.send(mappings);
        });
    }
    pub fn poll_mappings(&mut self) {
        if let Some(mappings) = self
            .loading
            .as_ref()
            .and_then(|ready| ready.try_recv().ok())
        {
            self.mappings = mappings;
            self.loading = None;
        }
    }
    /// None ignores a duplicate/unknown stream; Some(None) releases an input.
    pub(crate) fn read(&self, event: &Event) -> Option<Option<Button>> {
        let Some((which, raw)) = crate::input::device_event(event) else {
            return Some(crate::input::map_event(event));
        };
        if self.loading.is_some() {
            return None;
        }
        let device = self
            .devices
            .iter()
            .find(|d| d.joystick.instance_id() == which)?;
        let raw_stream = device.controller.is_none()
            || self.mappings.contains_device(&device.guid, &device.name);
        (raw == raw_stream).then(|| {
            self.mappings.map_device(
                &device.guid,
                &device.name,
                device.controller.is_some(),
                event,
            )
        })
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
                        name: joystick.name(),
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
