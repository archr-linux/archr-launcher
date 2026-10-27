//! Open devices in SDL joystick index order, retaining handles for event delivery.
use sdl2::{
    controller::GameController, joystick::Joystick, GameControllerSubsystem, JoystickSubsystem,
};

use crate::{joystick_mapping::RawMappings, state::Button};
use std::{
    io::Read,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

struct Device {
    index: u32,
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
        let mut this = Self {
            joysticks: sdl.joystick()?,
            controllers: sdl.game_controller()?,
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
                let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
                if !file
                    .metadata()
                    .map_err(|error| error.to_string())?
                    .is_file()
                {
                    return Err("es_input.cfg is not a regular file".into());
                }
                let mut text = String::new();
                file.take(1024 * 1024 + 1)
                    .read_to_string(&mut text)
                    .map_err(|error| error.to_string())?;
                if text.len() > 1024 * 1024 {
                    return Err("es_input.cfg exceeds 1 MiB".into());
                }
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
    pub fn map_device_event(&self, event: &sdl2::event::Event) -> Option<Button> {
        let which = match *event {
            sdl2::event::Event::JoyButtonDown { which, .. }
            | sdl2::event::Event::JoyButtonUp { which, .. }
            | sdl2::event::Event::JoyHatMotion { which, .. }
            | sdl2::event::Event::JoyAxisMotion { which, .. }
            | sdl2::event::Event::ControllerButtonDown { which, .. }
            | sdl2::event::Event::ControllerButtonUp { which, .. }
            | sdl2::event::Event::ControllerAxisMotion { which, .. } => which,
            _ => return None,
        };
        let device = self
            .devices
            .iter()
            .find(|device| device.joystick.instance_id() == which)?;
        self.mappings.map_device(
            &device.joystick.guid().to_string(),
            &device.joystick.name(),
            device.controller.is_some(),
            event,
        )
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
                        joystick,
                        controller,
                    });
                }
                Err(error) => log::warn!("Cannot open joystick {index}: {error}"),
            }
        }
    }
    pub fn accepts_raw(&self, instance: u32) -> bool {
        if self.loading.is_some() {
            return false;
        }
        self.devices.iter().any(|device| {
            device.joystick.instance_id() == instance
                && (device.controller.is_none()
                    || self.mappings.contains_device(
                        &device.joystick.guid().to_string(),
                        &device.joystick.name(),
                    ))
        })
    }
    pub fn accepts_controller(&self, instance: u32) -> bool {
        if self.loading.is_some() {
            return false;
        }
        self.devices.iter().any(|device| {
            device.joystick.instance_id() == instance
                && device.controller.is_some()
                && !self
                    .mappings
                    .contains_device(&device.joystick.guid().to_string(), &device.joystick.name())
        })
    }
    pub fn configuration(&self) -> String {
        controller_arguments(
            self.devices
                .iter()
                .map(|device| (device.index, device.joystick.guid().to_string())),
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
