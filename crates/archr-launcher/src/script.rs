//! Hidden PC-only navigation driver. Tokens are buttons, WAIT or QUIT.
use archr_launcher::state::Button;
use archr_ui::i18n::I18n;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub enum Step {
    Button(Button),
    Wait,
    Snapshot,
    Quit,
}
pub struct Script {
    steps: VecDeque<Step>,
    next: Instant,
    deadline: Instant,
}
impl Script {
    pub fn parse(source: &str, catalog: &I18n) -> Result<Self, String> {
        let mut steps = VecDeque::new();
        for token in source.split([',', ' ']).filter(|token| !token.is_empty()) {
            steps.push_back(match token.to_ascii_uppercase().as_str() {
                "A" => Step::Button(Button::A),
                "B" => Step::Button(Button::B),
                "X" => Step::Button(Button::X),
                "Y" => Step::Button(Button::Y),
                "L" => Step::Button(Button::L),
                "R" => Step::Button(Button::R),
                "UP" => Step::Button(Button::Up),
                "DOWN" => Step::Button(Button::Down),
                "START" => Step::Button(Button::Start),
                "BACK" | "SELECT" => Step::Button(Button::Select),
                "WAIT" => Step::Wait,
                "SNAPSHOT" => Step::Snapshot,
                "QUIT" => Step::Quit,
                _ => return Err(catalog.format("cli.invalid_button", &[("button", token)])),
            });
        }
        let now = Instant::now();
        Ok(Self {
            steps,
            next: now + Duration::from_millis(200),
            deadline: now + Duration::from_secs(30),
        })
    }
    /// A terminal button exits before the next script poll. Its immediately
    /// following SNAPSHOT belongs to the feedback frame presented before exit.
    pub fn take_terminal_snapshot(&mut self) -> bool {
        if matches!(self.steps.front(), Some(Step::Snapshot)) {
            self.steps.pop_front();
            true
        } else {
            false
        }
    }
    pub fn poll(&mut self, ready: bool, catalog: &I18n) -> Result<Option<Step>, String> {
        let now = Instant::now();
        if now > self.deadline {
            return Err(catalog.text("cli.script_timeout"));
        }
        if !ready || now < self.next {
            return Ok(None);
        }
        self.next = now + Duration::from_millis(150);
        Ok(self.steps.pop_front())
    }
}
