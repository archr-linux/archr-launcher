//! Hidden PC-only navigation driver. Tokens are buttons, WAIT or QUIT.
use archr_launcher::state::Button;
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
    pub fn parse(source: &str) -> Result<Self, String> {
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
                _ => return Err(format!("Botão de script inválido: {token}")),
            });
        }
        let now = Instant::now();
        Ok(Self {
            steps,
            next: now + Duration::from_millis(200),
            deadline: now + Duration::from_secs(30),
        })
    }
    pub fn poll(&mut self, ready: bool) -> Result<Option<Step>, String> {
        let now = Instant::now();
        if now > self.deadline {
            return Err("Script excedeu 30 s sem lançar jogo ou QUIT".to_owned());
        }
        if !ready || now < self.next {
            return Ok(None);
        }
        self.next = now + Duration::from_millis(150);
        Ok(self.steps.pop_front())
    }
}
