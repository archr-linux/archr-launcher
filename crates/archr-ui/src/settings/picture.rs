//! Sound and display › Picture (Tela-imagem): the panel's gamma and colour
//! temperature, held by archr-gamma. Each step is saved and shown at once;
//! A keeps what is on screen, B puts back what the page found, and Restore
//! returns to the panel as it is (1.00, 6500 K). Saturation, in the mockup,
//! is not offered: the RK3326 VOP has a gamma table per channel, no matrix.
use super::{described, row, Effect, Id, Kind, Page, Row, Settings};
use crate::state::Button;

pub const IMAGE: &[Row] = &[
    described(Id::Gamma, "image.gamma", "image.gamma_detail").under("image.section"),
    described(
        Id::ColorTemp,
        "image.temperature",
        "image.temperature_detail",
    ),
    row(Id::ImageRestore, "image.restore", Kind::Action),
];
/// The panel as it is: archr-gamma holds no curve.
pub const DEFAULT: (&str, &str) = ("1.00", "6500");

impl Settings {
    fn picture(&self) -> (String, String) {
        (
            self.value(Id::Gamma).to_owned(),
            self.value(Id::ColorTemp).to_owned(),
        )
    }
    pub(super) fn open_image(&mut self) {
        self.image_entry = Some(self.picture());
        self.open(Page::Image);
    }
    fn set_picture(&mut self, (gamma, temperature): (String, String)) -> Effect {
        self.values.insert(Id::Gamma, gamma.clone());
        self.values.insert(Id::ColorTemp, temperature.clone());
        Effect::Picture(gamma, temperature)
    }
    pub(super) fn image_input(&mut self, button: Button) -> Option<Effect> {
        let restore = self.visible_rows().get(self.cursor).map(|r| r.id) == Some(Id::ImageRestore);
        match button {
            Button::A if restore => Some(self.set_picture((DEFAULT.0.into(), DEFAULT.1.into()))),
            Button::A => {
                self.image_entry = None;
                Some(self.back())
            }
            Button::B => {
                let entry = self.image_entry.take().filter(|e| *e != self.picture());
                let back = self.back();
                Some(entry.map_or(back, |e| self.set_picture(e)))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> Settings {
        let mut state = Settings {
            loading: false,
            ..Default::default()
        };
        state.open(Page::Sound);
        state.values.insert(Id::Gamma, "1.10".into());
        state.open_image();
        state
    }
    /// What the host's reply does once the value is saved.
    fn saved(state: &mut Settings, id: Id, value: &str) {
        state.pending = None;
        state.values.insert(id, value.into());
    }
    #[test]
    fn a_keeps_what_is_on_screen() {
        let mut state = page();
        assert_eq!(
            state.handle(Button::R),
            Effect::Set(Id::Gamma, "1.20".into())
        );
        saved(&mut state, Id::Gamma, "1.20");
        assert_eq!(state.handle(Button::A), Effect::None);
        assert_eq!(state.page(), Page::Sound);
        assert_eq!(state.value(Id::Gamma), "1.20");
    }
    #[test]
    fn b_puts_back_what_the_page_found() {
        let mut state = page();
        state.handle(Button::R);
        saved(&mut state, Id::Gamma, "1.20");
        state.cursor = 1;
        state.handle(Button::R);
        saved(&mut state, Id::ColorTemp, "6000");
        assert_eq!(
            state.handle(Button::B),
            Effect::Picture("1.10".into(), "6500".into())
        );
        assert_eq!(state.page(), Page::Sound);
        assert_eq!(state.value(Id::Gamma), "1.10");
        // Nothing changed: B only leaves.
        state.open_image();
        assert_eq!(state.handle(Button::B), Effect::None);
    }
    #[test]
    fn restore_returns_to_the_panel_as_it_is() {
        let mut state = page();
        state.cursor = 2;
        assert_eq!(
            state.handle(Button::A),
            Effect::Picture("1.00".into(), "6500".into())
        );
        assert_eq!(state.page(), Page::Image);
        assert_eq!(
            state.handle(Button::B),
            Effect::Picture("1.10".into(), "6500".into()),
            "B undoes the restore too"
        );
    }
}
