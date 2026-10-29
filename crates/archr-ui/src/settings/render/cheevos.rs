//! Achievements page (Config-ra): options on the left, the account card on
//! the right. Layout is fixed like every settings page.
use super::*;

/// Left edge and width of the account card; the rows stop 16 px before it.
const CARD: (f32, f32) = (390., 230.);
pub(super) const ROWS_RIGHT: f32 = CARD.0 - 16.;

impl View<'_, '_, '_> {
    pub(super) fn cheevos_panel(&mut self, state: &Settings) -> Result<(), String> {
        let cheevos = &state.cheevos;
        let (x, w) = CARD;
        let theme = self.theme;
        self.ui.rounded(
            self.rect((x, VIEW_TOP, w, 374.)),
            self.ui.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        let (left, inner) = (x + 14., w - 28.);
        let top = VIEW_TOP + 14.;
        // Avatar: the user's initial, or the trophy while signed out.
        let avatar = self.rect((left, top, 44., 44.));
        self.ui
            .rounded(avatar, self.ui.px(22.) as f32, theme.badge, None)?;
        let initial = cheevos
            .user
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string());
        match &initial {
            Some(initial) => self.text_face(
                (left, top, 44., 44.),
                initial,
                (18., theme.highlight_bg, Align::Center),
                Face::Semibold,
            )?,
            None => self.ui.icon(
                Glyph::Trophy,
                self.rect((left + 12., top + 12., 20., 20.)),
                theme.highlight_bg,
            )?,
        }
        let name_x = left + 56.;
        let (name, detail) = if cheevos.verifying {
            (self.tr("settings.verifying"), String::new())
        } else if cheevos.user.is_empty() {
            (self.tr("settings.not_signed_in"), String::new())
        } else {
            let since = cheevos
                .profile
                .as_ref()
                .filter(|p| !p.since.is_empty())
                .map(|p| self.tr(&format!("settings.cheevos_since|year={}", p.since)))
                .unwrap_or_default();
            (cheevos.user.clone(), since)
        };
        let name_h = if detail.is_empty() { 44. } else { 26. };
        self.text_face(
            (name_x, top, left + inner - name_x, name_h),
            &name,
            (16., theme.text, Align::Left),
            Face::Semibold,
        )?;
        if !detail.is_empty() {
            self.note(
                (name_x, top + 24., left + inner - name_x, 18.),
                &detail,
                12.,
            )?;
        }
        let mut y = top + 44. + 10.;
        if cheevos.user.is_empty() {
            return self.paragraph((left, y, inner), &self.tr("settings.cheevos_card_note"));
        }
        // Points, hardcore and softcore, in two tiles.
        let profile = cheevos.profile.as_ref();
        let tile = (inner - 8.) / 2.;
        for (n, (value, label)) in [
            (profile.map(|p| p.points), "settings.cheevos_points"),
            (profile.map(|p| p.softcore), "settings.cheevos_softcore"),
        ]
        .into_iter()
        .enumerate()
        {
            let tx = left + n as f32 * (tile + 8.);
            self.ui.rounded(
                self.rect((tx, y, tile, 52.)),
                self.ui.px(8.) as f32,
                theme.background,
                None,
            )?;
            let value = value.map_or_else(|| "—".to_owned(), |v| v.to_string());
            self.text_face(
                (tx + 8., y + 7., tile - 16., 22.),
                &value,
                (16., theme.text, Align::Left),
                Face::Mono,
            )?;
            self.note((tx + 8., y + 29., tile - 16., 16.), &self.tr(label), 11.)?;
        }
        y += 52. + 10.;
        if let Some(game) = profile.map(|p| &p.last_game).filter(|g| !g.is_empty()) {
            let line = self.tr(&format!(
                "settings.cheevos_last_game|game={}",
                game.replace('|', "")
            ));
            self.paragraph((left, y, inner), &line)?;
        }
        Ok(())
    }
    /// Muted 12 px text from the top of the space, as tall as its lines.
    pub(super) fn paragraph(
        &mut self,
        (x, y, w): (f32, f32, f32),
        text: &str,
    ) -> Result<(), String> {
        let width = self.rect((x, y, w, 1.)).width();
        let lines = self.lines(text, 12., width)?.len() as f32;
        self.note((x, y, w, lines * 15.), text, 12.)
    }
}
