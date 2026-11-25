//! System › Port compatibility (Compat-ports): ports made for other Debian systems that
//! need Debian itself run inside a Debian runtime (archr-compat-debian).
//! The page holds the global switch, how text-mode tools show, the runtime
//! (download, its size and the layer that grows with what ports install)
//! and the ports runemu.sh sends to Debian. The host reads and acts.
use super::{described, launcher::size, row, Effect, Id, Kind, Page, Row, Settings};
use crate::{state::Button, HintButton};

pub const ROWS: &[Row] = &[
    described(Id::Compat, "compat.enabled", "compat.enabled_detail")
        .kind(Kind::Toggle)
        .under("compat.section_mode"),
    described(
        Id::CompatTerminal,
        "compat.terminal",
        "compat.terminal_detail",
    )
    .kind(Kind::Toggle),
    described(Id::CompatRuntime, "compat.runtime", "").under("compat.section_runtime"),
    described(
        Id::CompatDownload,
        "compat.download",
        "compat.download_detail",
    )
    .kind(Kind::Action),
    described(Id::CompatLayer, "compat.layer", "compat.layer_detail"),
    described(Id::CompatReset, "compat.reset", "compat.reset_detail").kind(Kind::Action),
    row(Id::CompatNone, "compat.ports_none", Kind::Value).under("compat.section_ports"),
];

/// What the host does for the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Read the manifest, the runtime's status and the marks.
    Look,
    /// `archr-compat-debian ensure`: download the image once.
    Download,
    /// `archr-compat-debian reset`: drop what ports installed inside.
    Reset,
}

/// Why runemu.sh sends a port to Debian, or no longer does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// `ports["X.sh"].compat=debian`, from the game menu.
    Chosen,
    /// A run failed for want of Debian (`wanted/X.sh`).
    Learned,
    /// It failed in Debian too: back to Arch R for good (`no/X.sh`).
    Refused,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    /// The port's script, as system.cfg and the marks name it.
    pub script: String,
    pub mark: Mark,
}

/// The page's readings, from the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The manifest's image: its date and its size.
    pub version: String,
    pub image: u64,
    pub installed: bool,
    /// The writable layer of the installed image.
    pub layer: u64,
    pub ports: Vec<Port>,
}
impl Facts {
    /// The manifest's `KEY=VALUE` lines and `archr-compat-debian status`.
    pub fn parse(manifest: &str, status: &str, ports: Vec<Port>) -> Self {
        let get = |text: &str, key: &str| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix(key))
                .unwrap_or_default()
                .trim()
                .to_owned()
        };
        let installed = get(status, "installed:");
        Self {
            version: get(manifest, "VERSION="),
            image: get(manifest, "SIZE=").parse().unwrap_or(0),
            installed: !installed.is_empty() && installed != "no",
            layer: du_bytes(
                get(status, "layer:")
                    .split_whitespace()
                    .next()
                    .unwrap_or(""),
            ),
            ports,
        }
    }
}
/// `du -h`'s "643M", "1.2G", "12K" or "0", in bytes.
fn du_bytes(text: &str) -> u64 {
    let (number, unit) = text.split_at(
        text.find(|c: char| c.is_ascii_alphabetic())
            .unwrap_or(text.len()),
    );
    let shift = match unit {
        "K" => 10,
        "M" => 20,
        "G" => 30,
        "T" => 40,
        _ => 0,
    };
    number
        .replace(',', ".")
        .parse::<f64>()
        .map_or(0, |n| (n * (1u64 << shift) as f64) as u64)
}

#[derive(Clone, Debug, Default)]
pub struct CompatPage {
    pub facts: Option<Facts>,
    pub looking: bool,
    /// The question before freeing the layer is open.
    pub confirm: bool,
    /// The action under way: its row reads "…".
    pub busy: Option<Id>,
}

pub const CONFIRM_HINTS: &[(HintButton, &str)] = &[
    (HintButton::Key('A'), "compat.reset_yes"),
    (HintButton::Key('B'), "hint.cancel"),
];

impl Settings {
    /// Nothing known yet about the open page: the host looks.
    pub fn compat_wants_look(&self) -> bool {
        self.page() == Page::Compat && self.compat.facts.is_none() && !self.compat.looking
    }
    /// The host's readings after a look or an action, and what to say.
    pub fn compat_done(&mut self, said: Result<Option<&str>, &str>, facts: Facts) {
        self.compat.facts = Some(facts);
        self.compat.looking = false;
        self.compat.busy = None;
        match said {
            Ok(Some(note)) => self.note(note.into()),
            Ok(None) => {}
            Err(error) => self.fail(None, error.into()),
        }
    }
    /// The page's rows: its own, then one per port runemu.sh marked.
    pub(super) fn compat_rows(&self) -> Vec<Row> {
        let facts = self.compat.facts.as_ref();
        let installed = facts.is_some_and(|f| f.installed);
        let ports = facts.map_or(0, |f| f.ports.len().min(usize::from(u8::MAX)));
        let mut rows: Vec<Row> = ROWS
            .iter()
            .filter(|row| match row.id {
                Id::CompatDownload => facts.is_some() && !installed,
                Id::CompatLayer | Id::CompatReset => installed,
                Id::CompatNone => ports == 0,
                _ => true,
            })
            .copied()
            .collect();
        rows.extend((0..ports).map(|n| Row {
            id: Id::CompatPort(n as u8),
            label: "",
            subtitle: None,
            kind: Kind::Value,
            section: (n == 0).then_some("compat.section_ports"),
        }));
        rows
    }
    fn compat_port(&self, id: Id) -> Option<&Port> {
        let Id::CompatPort(n) = id else { return None };
        self.compat.facts.as_ref()?.ports.get(usize::from(n))
    }
    /// A port row's title: its script without ".sh".
    pub(super) fn compat_label(&self, id: Id) -> Option<String> {
        let script = &self.compat_port(id)?.script;
        Some(script.strip_suffix(".sh").unwrap_or(script).to_owned())
    }
    /// The reading on the right of a row, as a catalog reference.
    pub(super) fn compat_value(&self, id: Id) -> String {
        if self.compat.busy == Some(id) {
            return "launcher.working".into();
        }
        if id == Id::CompatLink {
            return "›".into();
        }
        let Some(facts) = &self.compat.facts else {
            return String::new();
        };
        match id {
            Id::CompatRuntime if facts.installed => "compat.installed".into(),
            Id::CompatRuntime => "compat.missing".into(),
            Id::CompatDownload => size(facts.image),
            Id::CompatLayer => size(facts.layer),
            Id::CompatReset if facts.layer > 0 => "›".into(),
            id => match self.compat_port(id).map(|p| p.mark) {
                Some(Mark::Refused) => "compat.native".into(),
                Some(_) => "compat.debian".into(),
                None => String::new(),
            },
        }
    }
    /// A reading followed by "›": the row acts on it.
    pub(super) fn compat_arrow(&self, id: Id) -> bool {
        id == Id::CompatDownload && self.compat.busy.is_none()
    }
    pub(super) fn compat_subtitle(&self, row: &Row) -> Option<String> {
        if self.compat.busy == Some(Id::CompatDownload) && row.id == Id::CompatDownload {
            return Some("compat.downloading".into());
        }
        if let Some(port) = self.compat_port(row.id) {
            return Some(
                match port.mark {
                    Mark::Chosen => "compat.mark_chosen",
                    Mark::Learned => "compat.mark_learned",
                    Mark::Refused => "compat.mark_refused",
                }
                .into(),
            );
        }
        let facts = self.compat.facts.as_ref();
        match (row.id, facts) {
            (Id::CompatRuntime, Some(f)) if f.installed => Some(format!(
                "compat.runtime_installed|version={}|size={}",
                f.version,
                size(f.image)
            )),
            (Id::CompatRuntime, Some(f)) => {
                Some(format!("compat.runtime_missing|size={}", size(f.image)))
            }
            (Id::CompatRuntime, None) => None,
            _ => row.subtitle.map(Into::into),
        }
    }
    /// Readings: no input.
    pub(super) fn compat_fixed(&self, id: Id) -> bool {
        matches!(
            id,
            Id::CompatRuntime | Id::CompatLayer | Id::CompatNone | Id::CompatPort(_)
        )
    }
    pub(super) fn compat_disabled(&self, id: Id) -> bool {
        match id {
            Id::CompatDownload | Id::CompatReset if self.compat.busy.is_some() => true,
            Id::CompatReset => self.compat.facts.as_ref().is_none_or(|f| f.layer == 0),
            _ => false,
        }
    }
    pub(super) fn compat_activate(&mut self, id: Id) -> Option<Effect> {
        match id {
            // Every visit looks again.
            Id::CompatLink => {
                self.compat.facts = None;
                self.open(Page::Compat);
                Some(Effect::None)
            }
            _ if self.compat_disabled(id) => Some(Effect::None),
            Id::CompatDownload => {
                self.compat.busy = Some(id);
                Some(Effect::Compat(Action::Download))
            }
            // What ports installed goes, and comes back only as they run.
            Id::CompatReset => {
                self.compat.confirm = true;
                Some(Effect::None)
            }
            _ => None,
        }
    }
    /// The question before freeing the layer: A goes ahead, any other
    /// button cancels.
    pub(super) fn compat_input(&mut self, button: Button) -> Option<Effect> {
        if !std::mem::take(&mut self.compat.confirm) {
            return None;
        }
        Some(if button == Button::A {
            self.compat.busy = Some(Id::CompatReset);
            Effect::Compat(Action::Reset)
        } else {
            Effect::None
        })
    }
    /// The open question, for the dialog: title and detail.
    pub fn compat_question(&self) -> Option<(String, &'static str)> {
        let layer = self
            .compat
            .facts
            .as_ref()
            .filter(|_| self.compat.confirm)?
            .layer;
        Some((
            format!("compat.reset_question|size={}", size(layer)),
            "compat.reset_what",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(installed: bool, ports: Vec<Port>) -> Facts {
        let status = if installed {
            "manifest: x (1 bytes)\ninstalled: /storage/x\nlayer: 643M\t/storage/up\n"
        } else {
            "manifest: x (1 bytes)\ninstalled: no\n"
        };
        Facts::parse("VERSION=20261125\nSIZE=65671168\n", status, ports)
    }
    fn open(facts: Facts) -> Settings {
        let mut state = Settings::default();
        state.compat_supported = true;
        state.open(Page::System);
        state.compat_activate(Id::CompatLink);
        state.compat_done(Ok(None), facts);
        state
    }
    fn ids(state: &Settings) -> Vec<Id> {
        state.visible_rows().iter().map(|r| r.id).collect()
    }

    #[test]
    fn status_and_manifest_are_read() {
        let f = facts(true, Vec::new());
        assert_eq!(
            (f.version.as_str(), f.image, f.installed),
            ("20261125", 65671168, true)
        );
        assert_eq!(f.layer, 643 << 20);
        assert!(!facts(false, Vec::new()).installed);
        assert_eq!(du_bytes("1.5G"), 3 << 29);
        assert_eq!(du_bytes("0"), 0);
    }

    #[test]
    fn the_link_hides_without_the_command() {
        let mut state = Settings::default();
        state.open(Page::System);
        assert!(!ids(&state).contains(&Id::CompatLink));
        state.compat_supported = true;
        assert!(ids(&state).contains(&Id::CompatLink));
    }

    #[test]
    fn rows_follow_the_runtime_and_the_marks() {
        let missing = open(facts(false, Vec::new()));
        assert_eq!(
            ids(&missing),
            [
                Id::Compat,
                Id::CompatTerminal,
                Id::CompatRuntime,
                Id::CompatDownload,
                Id::CompatNone
            ]
        );
        let port = |script: &str, mark| Port {
            script: script.into(),
            mark,
        };
        let ready = open(facts(
            true,
            vec![
                port("Terminal-Launcher.sh", Mark::Learned),
                port("Halo.sh", Mark::Refused),
            ],
        ));
        assert_eq!(
            ids(&ready),
            [
                Id::Compat,
                Id::CompatTerminal,
                Id::CompatRuntime,
                Id::CompatLayer,
                Id::CompatReset,
                Id::CompatPort(0),
                Id::CompatPort(1)
            ]
        );
        assert_eq!(
            ready.compat_label(Id::CompatPort(0)).as_deref(),
            Some("Terminal-Launcher")
        );
        assert_eq!(ready.compat_value(Id::CompatPort(1)), "compat.native");
        assert!(ready.fixed(Id::CompatPort(0)) && ready.fixed(Id::CompatLayer));
    }

    #[test]
    fn download_runs_and_reset_asks_first() {
        let mut missing = open(facts(false, Vec::new()));
        assert_eq!(
            missing.compat_activate(Id::CompatDownload),
            Some(Effect::Compat(Action::Download))
        );
        assert!(missing.compat_disabled(Id::CompatDownload), "one at a time");
        let mut ready = open(facts(true, Vec::new()));
        assert_eq!(ready.compat_activate(Id::CompatReset), Some(Effect::None));
        assert!(ready.compat_question().is_some());
        assert_eq!(ready.compat_input(Button::B), Some(Effect::None));
        assert_eq!(ready.compat_activate(Id::CompatReset), Some(Effect::None));
        assert_eq!(
            ready.compat_input(Button::A),
            Some(Effect::Compat(Action::Reset))
        );
        assert_eq!(ready.compat.busy, Some(Id::CompatReset));
        let mut empty = open(Facts {
            layer: 0,
            ..facts(true, Vec::new())
        });
        assert!(empty.compat_disabled(Id::CompatReset));
        assert_eq!(empty.compat_activate(Id::CompatReset), Some(Effect::None));
        assert!(!empty.compat.confirm);
    }
}
