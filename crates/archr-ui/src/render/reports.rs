//! Notices over the main screens: error reports are on, said once over the
//! Start screen (Relatorios-aviso), and a port that needs the panfrost
//! driver (Ports-panfrost).
use super::*;
use crate::settings::reports::Notice;

impl Ui<'_, '_> {
    pub(crate) fn reports_notice(&mut self, notice: &Notice, theme: &Theme) -> Result<(), String> {
        // What it does, what goes, where to change it.
        let paragraphs = [
            "reports.notice_body",
            "reports.notice_detail",
            "reports.notice_where",
        ]
        .map(|key| self.catalog.text(key));
        self.notice_dialog(
            &self.catalog.text("reports.notice_title"),
            &paragraphs,
            (
                ["reports.notice_ok", "reports.notice_off"],
                notice.on_ok,
                false,
            ),
            &[
                (HintButton::Pair('‹', '›'), "hint.choose"),
                ('A'.into(), "hint.confirm"),
            ],
            theme,
        )
    }
    pub(crate) fn panfrost_notice(&mut self, title: &str, theme: &Theme) -> Result<(), String> {
        // Why it cannot run, then what the switch does and how to undo it.
        let paragraphs =
            ["ports.panfrost_body", "ports.panfrost_note"].map(|key| self.catalog.text(key));
        self.notice_dialog(
            &self
                .catalog
                .format("ports.panfrost_title", &[("name", title)]),
            &paragraphs,
            // The switch restarts the console.
            (["ports.panfrost_switch", "hint.cancel"], true, true),
            &[
                ('A'.into(), "ports.panfrost_switch"),
                ('B'.into(), "hint.cancel"),
            ],
            theme,
        )
    }
    /// A titled panel: the first paragraph in the text color, the rest
    /// muted, two buttons (`first` filled) and the footer's `hints`.
    fn notice_dialog(
        &mut self,
        title: &str,
        paragraphs: &[String],
        (buttons, first, danger): ([&str; 2], bool, bool),
        hints: &[(HintButton, &str)],
        theme: &Theme,
    ) -> Result<(), String> {
        let Frame { width, height, .. } = self.frame;
        let footer = self.frame.size(theme.footer_height).round().max(1.) as u32;
        self.scrim(theme, 1.2)?;
        let w = self.question_width(theme, 448.);
        let pad = self.px(22.);
        let inner = (w - 2 * pad).max(1) as u32;
        let gap = self.px(12.);
        let icon = self.px(22.);
        let title_room = (inner as i32 - icon - self.px(10.)).max(1) as u32;
        let titles = self.wrap(title, Face::Semibold, 17., title_room)?;
        let title_step = self.px(22.);
        let mut texts = Vec::new();
        for (n, text) in paragraphs.iter().enumerate() {
            let (size, ink) = if n == 0 {
                (13., theme.text)
            } else {
                (12., theme.muted)
            };
            let lines = self.wrap(text, Face::Regular, size, inner)?;
            texts.push((lines, size, ink, self.px(size * 1.55)));
        }
        let button_h = self.px(40.);
        let body: i32 = texts
            .iter()
            .map(|(lines, _, _, step)| step * lines.len() as i32 + gap)
            .sum();
        let h = self.px(20.)
            + title_step * titles.len() as i32
            + gap
            + body
            + self.px(4.)
            + button_h
            + self.px(18.);
        let room = height as i32 - footer as i32;
        let panel = Rect::new(
            (width as i32 - w) / 2,
            ((room - h) / 2).max(0),
            w as u32,
            h as u32,
        );
        self.question_panel(theme, panel, self.px(14.) as f32, theme.surface)?;
        let accent = theme.highlight_bg;
        let x = panel.x() + pad;
        let mut y = panel.y() + self.px(20.);
        self.icon(
            Glyph::Alert,
            Rect::new(x, y, icon as u32, icon as u32),
            accent,
        )?;
        for line in &titles {
            self.label(
                line,
                (Face::Semibold, 17., theme.text),
                Rect::new(x + icon + self.px(10.), y, title_room, title_step as u32),
            )?;
            y += title_step;
        }
        y += gap;
        for (lines, size, ink, step) in &texts {
            for line in lines {
                self.label(
                    line,
                    (Face::Regular, *size, *ink),
                    Rect::new(x, y, inner, *step as u32),
                )?;
                y += step;
            }
            y += gap;
        }
        y += self.px(4.);
        // The focused button is filled.
        let half = ((inner as i32 - self.px(10.)) / 2).max(1) as u32;
        for (n, key) in buttons.into_iter().enumerate() {
            let rect = Rect::new(
                x + n as i32 * (half as i32 + self.px(10.)),
                y,
                half,
                button_h as u32,
            );
            let focused = (n == 0) == first;
            let plain = if focused {
                (accent, None, theme.highlight_text, Face::Semibold)
            } else {
                (theme.badge, Some(theme.border), theme.text, Face::Regular)
            };
            let radius = self.px(10.) as f32;
            let (ink, face) =
                self.answer_button(theme, rect, radius, (focused, danger && n == 0), plain)?;
            let label = self.catalog.text(key);
            let floor = self.frame.min_size();
            let room = rect.width().saturating_sub(self.px(20.) as u32).max(1);
            let size = self
                .shrink(&label, face, 14., floor, room)?
                .unwrap_or(floor);
            self.label_aligned(&label, (face, size, ink), rect, Align::Center)?;
        }
        // The footer answers the notice.
        let fy = height as i32 - footer as i32;
        self.canvas.set_draw_color(color(theme.background));
        self.canvas.fill_rect(Rect::new(0, fy, width, footer))?;
        self.divider(0, fy, width, theme.border)?;
        self.hints(hints, theme, width, fy, footer)?;
        Ok(())
    }
}
