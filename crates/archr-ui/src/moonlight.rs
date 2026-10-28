//! Moonlight presentation model; no filesystem, networking or processes here.
mod keyboard;
pub use keyboard::Keyboard;

pub const SYSTEM: &str = "@moonlight";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Moonlight expects kilobits per second, not megabits.
    pub bitrate: u32,
    pub codec: Codec,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    Auto,
}
impl Codec {
    pub fn argument(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::H265 => "h265",
            Self::Auto => "auto",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "moonlight.h264",
            Self::H265 => "moonlight.h265",
            Self::Auto => "moonlight.auto",
        }
    }
}
impl Options {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            fps: 60,
            bitrate: 5000,
            codec: Codec::H264,
        }
    }
    pub fn valid(&self) -> bool {
        (1..=8192).contains(&self.width)
            && (1..=8192).contains(&self.height)
            && [30, 60].contains(&self.fps)
            && [2000, 5000, 10000, 20000].contains(&self.bitrate)
    }
    pub fn adjust(&mut self, row: usize, forward: bool, panel: (u32, u32)) {
        fn cycle<T: Copy + PartialEq>(value: T, values: &[T], forward: bool) -> T {
            let index = values.iter().position(|v| *v == value).unwrap_or(0);
            values[(index + if forward { 1 } else { values.len() - 1 }) % values.len()]
        }
        match row {
            0 => {
                (self.width, self.height) = cycle(
                    (self.width, self.height),
                    &[panel, (1280, 720), (1920, 1080)],
                    forward,
                )
            }
            1 => self.fps = cycle(self.fps, &[30, 60], forward),
            2 => self.bitrate = cycle(self.bitrate, &[2000, 5000, 10000, 20000], forward),
            3 => {
                self.codec = cycle(
                    self.codec,
                    &[Codec::H264, Codec::H265, Codec::Auto],
                    forward,
                )
            }
            _ => {}
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    pub host: String,
    pub app: String,
    pub options: Options,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Unknown,
    Online,
    NeedsPair,
    Offline,
}
impl Status {
    pub fn key(self) -> &'static str {
        match self {
            Self::Unknown => "moonlight.unknown",
            Self::Online => "moonlight.online",
            Self::NeedsPair => "moonlight.needs_pair",
            Self::Offline => "moonlight.offline",
        }
    }
}
#[derive(Clone, Debug)]
pub struct State {
    pub hosts: Vec<String>,
    pub host: usize,
    pub apps: Vec<String>,
    pub cursor: usize,
    pub status: Status,
    pub options: Options,
    pub option: Option<usize>,
    pub keyboard: Option<Keyboard>,
    pub confirm_unpair: bool,
    pub pin: Option<String>,
    pub busy: bool,
    pub error: Option<&'static str>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    App(usize),
    Pair,
    Host(usize),
    Add,
}
impl State {
    pub fn new(panel: (u32, u32)) -> Self {
        Self {
            hosts: vec![],
            host: 0,
            apps: vec![],
            cursor: 0,
            status: Status::Unknown,
            options: Options::new(panel.0, panel.1),
            option: None,
            keyboard: None,
            confirm_unpair: false,
            pin: None,
            busy: true,
            error: None,
        }
    }
    pub fn address(&self) -> Option<&str> {
        self.hosts.get(self.host).map(String::as_str)
    }
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<_> = (0..self.apps.len()).map(Row::App).collect();
        if self.address().is_some() && self.status == Status::NeedsPair {
            rows.push(Row::Pair);
        }
        rows.extend(
            (0..self.hosts.len())
                .filter(|i| *i != self.host)
                .map(Row::Host),
        );
        rows.push(Row::Add);
        rows
    }
}
