//! Updates (Atualizacoes*): state card, rows and packages panel; then the
//! run's progress, success and failure screens. Fixed layout, like every
//! settings page; the theme only gives colours, fonts and icons.
use super::*;
use crate::settings::updates::{Package, Progress, Reboot, Run, Stage, LOG};

/// The left column ends here; the packages panel takes the rest.
pub(super) const ROWS_RIGHT: f32 = 368.;
const PANEL: (f32, f32) = (384., 236.);
const CARD: f32 = 78.;
/// Rows start under the state card.
pub(super) const ROWS_TOP: f32 = VIEW_TOP + CARD + 6.;
/// Room the notes keep under the packages in the panel.
const NOTES_ROOM: f32 = 96.;
/// Progress screen: side margins and the step list (Atualizacoes-progresso).
const LEFT: f32 = 40.;
const WIDE: f32 = 560.;
const STEP: f32 = 45.;

impl View<'_, '_, '_> {
    /// "48,2 MB": one decimal, the locale's decimal mark and unit.
    fn size(&self, bytes: u64) -> String {
        let (value, key) = match bytes {
            0..1_048_576 => (bytes as f64 / 1024., "updates.kb"),
            1_048_576..1_073_741_824 => (bytes as f64 / 1_048_576., "updates.mb"),
            _ => (bytes as f64 / 1_073_741_824., "updates.gb"),
        };
        let number = format!("{value:.1}").replace('.', &self.ui.catalog.text("details.decimal"));
        self.ui.catalog.format(key, &[("value", &number)])
    }
    fn tinted(&self, ink: Rgba) -> Rgba {
        mix(self.theme.background, ink, 0.18)
    }
    pub(super) fn updates_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        match &state.updates.run {
            Some(Run::Working(progress)) => self.update_progress(state, progress),
            Some(Run::Done {
                reboot,
                from,
                to,
                count,
            }) => self.update_done(*reboot, from, to, *count),
            Some(Run::Failed {
                reason,
                line,
                changed,
            }) => self.update_failed(state, reason, line, *changed),
            None => {
                self.update_card(state)?;
                self.rows_at(state, rows, &placements(Page::Updates, rows), ROWS_RIGHT)?;
                self.update_panel(state)
            }
        }
    }
    fn update_card(&mut self, state: &Settings) -> Result<(), String> {
        let updates = &state.updates;
        let theme = self.theme;
        self.ui.rounded(
            self.rect((16., VIEW_TOP, ROWS_RIGHT - 16., CARD)),
            self.ui.px(12.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        let version = updates
            .report
            .as_ref()
            .map_or(state.info.version.as_str(), |r| r.version.as_str());
        let channel = state.display_value(Id::UpdateChannel, &self.ui.catalog);
        let title = self
            .ui
            .catalog
            .format(
                "updates.card",
                &[("version", version), ("channel", &channel)],
            )
            .to_uppercase();
        let width = ROWS_RIGHT - 44.;
        self.text(
            (29., VIEW_TOP + 9., width, 15.),
            &title,
            (11., theme.muted, Align::Left),
        )?;
        let ago = updates.checked.map(|at| {
            let age = self.ui.frame.now.saturating_duration_since(at).as_secs();
            self.ui.catalog.relative_time(0, age)
        });
        let ago = ago.unwrap_or_default();
        let (headline, detail) = if updates.checking {
            (
                self.tr("updates.checking"),
                self.tr("updates.checking_detail"),
            )
        } else if let Some(reason) = &updates.failed {
            (
                self.tr("updates.check_failed"),
                self.tr(&format!("updates.failed_{reason}")),
            )
        } else if let Some(report) = &updates.report {
            let count = report.packages.len();
            if count == 0 {
                (
                    self.tr("updates.current"),
                    self.ui.catalog.format("updates.checked", &[("ago", &ago)]),
                )
            } else {
                (
                    self.tr(&format!("updates.available|count={count}")),
                    self.ui.catalog.format(
                        "updates.to_download",
                        &[("size", &self.size(report.download)), ("ago", &ago)],
                    ),
                )
            }
        } else {
            (
                self.tr("updates.not_checked"),
                self.tr("updates.not_checked_detail"),
            )
        };
        self.text_face(
            (29., VIEW_TOP + 25., width, 26.),
            &headline,
            (18., theme.text, Align::Left),
            Face::Semibold,
        )?;
        self.note((29., VIEW_TOP + 52., width, 17.), &detail, 12.)
    }
    /// One package: the name, then "old → new · size" with the new version
    /// brighter; on a second line when both do not fit. Returns its height.
    fn package_line(&mut self, package: &Package, y: f32, draw: bool) -> Result<f32, String> {
        let (x, w) = PANEL;
        let theme = self.theme;
        let scale = self.ui.frame.scale;
        let old = if package.old.is_empty() {
            String::new()
        } else {
            format!("{} → ", package.old)
        };
        let parts = [
            (old, theme.muted),
            (package.new.clone(), theme.text),
            (format!(" · {}", self.size(package.size)), theme.muted),
        ];
        let mut widths = Vec::new();
        for (text, _) in &parts {
            widths.push(self.ui.measure(text, Face::Mono, 11.)? as f32 / scale);
        }
        let value: f32 = widths.iter().sum();
        let name = self.ui.measure(&package.name, Face::Regular, 12.)? as f32 / scale;
        let single = name + 12. + value <= w;
        let height = if single { 23. } else { 38. };
        if !draw {
            return Ok(height);
        }
        self.label((x, y + 4., w, 16.), &package.name, 12.)?;
        let mut right = x + w;
        let line = if single { y + 4. } else { y + 19. };
        for ((text, ink), width) in parts.iter().zip(&widths).rev() {
            right -= width;
            self.text_face(
                (right, line, *width + 1., 16.),
                text,
                (11., *ink, Align::Left),
                Face::Mono,
            )?;
        }
        self.ui.divider(
            self.ui.px(x),
            self.ui.px(y + height - 1.),
            self.ui.px(w) as u32,
            theme.border,
        )?;
        Ok(height)
    }
    fn update_panel(&mut self, state: &Settings) -> Result<(), String> {
        let (x, w) = PANEL;
        let theme = self.theme;
        let heading = |view: &mut Self, key: &str, y: f32| {
            let text = view.tr(key).to_uppercase();
            view.text((x, y, w, 15.), &text, (11., theme.muted, Align::Left))
        };
        heading(self, "updates.packages", VIEW_TOP)?;
        let mut y = VIEW_TOP + 21.;
        let report = state.updates.report.as_ref();
        let packages = report.map_or(&[][..], |r| r.packages.as_slice());
        if packages.is_empty() {
            self.paragraph((x, y + 2., w), &self.tr("updates.no_packages"))?;
            y += 24.;
        }
        let limit = VIEW_BOTTOM - NOTES_ROOM;
        for (n, package) in packages.iter().enumerate() {
            let height = self.package_line(package, y, false)?;
            // The last line left says how many more Y lists.
            let rest = packages.len() - n;
            if rest > 1 && y + height + 20. > limit {
                let more = self.tr(&format!("updates.more|count={rest}"));
                self.note((x, y + 3., w, 17.), &more, 12.)?;
                y += 22.;
                break;
            }
            self.package_line(package, y, true)?;
            y += height;
        }
        let news = report.map_or("", |r| r.news.as_str());
        if packages.is_empty() && news.is_empty() {
            return Ok(());
        }
        y += 8.;
        heading(self, "updates.news", y)?;
        y += 21.;
        if news.is_empty() {
            return self.paragraph((x, y, w), &self.tr("updates.no_news"));
        }
        let width = self.rect((x, y, w, 1.)).width();
        let mut lines = self.lines(news, 12., width)?;
        let room = ((VIEW_BOTTOM - y) / 17.).floor().max(1.) as usize;
        let cut = lines.len() > room;
        lines.truncate(if cut { room - 1 } else { room });
        for (n, line) in lines.iter().enumerate() {
            self.text(
                (x, y + n as f32 * 17., w, 17.),
                line,
                (12., theme.text_secondary, Align::Left),
            )?;
        }
        if cut {
            let top = y + lines.len() as f32 * 17.;
            self.note((x, top, w, 17.), &self.tr("updates.news_more"), 12.)?;
        }
        Ok(())
    }
    /// Every package, then the whole notes, in one scrolling list.
    pub(super) fn update_packages(&mut self, state: &Settings) -> Result<(), String> {
        let report = state.updates.report.clone().unwrap_or_default();
        let width = self.rect((30., 0., RIGHT - 44., 1.)).width();
        let news = if report.news.is_empty() {
            self.tr("updates.no_news")
        } else {
            report.news.clone()
        };
        let lines = self.lines(&news, 13., width)?;
        let mut top = VIEW_TOP;
        let mut positions: Vec<Placement> = report
            .packages
            .iter()
            .map(|_| {
                let place = Placement {
                    top,
                    height: 34.,
                    section: None,
                    searching: None,
                    trail: 0.,
                };
                top += 34. + ROW_GAP;
                place
            })
            .collect();
        positions.push(Placement {
            top: top + SECTION_HEIGHT,
            height: lines.len() as f32 * 17. + 12.,
            section: Some("updates.news"),
            searching: None,
            trail: 0.,
        });
        let theme = self.theme;
        self.list(state, &positions, RIGHT, |view, n, y, place, selected| {
            let ink = view.ink(selected);
            let quiet = selection(selected, theme.muted, theme.accent_text_secondary);
            let Some(package) = report.packages.get(n) else {
                for (k, line) in lines.iter().enumerate() {
                    view.text(
                        (30., y + 6. + k as f32 * 17., RIGHT - 44., 17.),
                        line,
                        (13., ink, Align::Left),
                    )?;
                }
                return Ok(());
            };
            let value = if package.old.is_empty() {
                format!("{} · {}", package.new, view.size(package.size))
            } else {
                format!(
                    "{} → {} · {}",
                    package.old,
                    package.new,
                    view.size(package.size)
                )
            };
            let face = selection(selected, Face::Regular, Face::Semibold);
            let half = (RIGHT - 46.) / 2.;
            view.text_face(
                (30., y, half, place.height),
                &package.name,
                (15., ink, Align::Left),
                face,
            )?;
            view.text_face(
                (30. + half, y, half, place.height),
                &value,
                (12., quiet, Align::Right),
                Face::Mono,
            )
        })
    }
    fn update_progress(&mut self, state: &Settings, progress: &Progress) -> Result<(), String> {
        let theme = self.theme;
        let count = progress
            .count
            .max(state.updates.count())
            .max(progress.current);
        self.text_face(
            (LEFT, 56., WIDE, 28.),
            &self.tr("updates.updating"),
            (20., theme.text, Align::Left),
            Face::Semibold,
        )?;
        let (now, percent) = match progress.stage {
            Stage::Sync => (self.tr("updates.now_sync"), 0),
            Stage::Download if progress.cancelling => {
                (self.tr("updates.cancelling"), progress.percent)
            }
            Stage::Download => (
                self.ui
                    .catalog
                    .format("updates.now_download", &[("name", &progress.package)]),
                progress.percent,
            ),
            Stage::Install => (
                self.ui
                    .catalog
                    .format("updates.now_install", &[("name", &progress.package)]),
                progress.percent,
            ),
            Stage::Hooks => (self.tr("updates.now_hooks"), 100),
        };
        let now = if progress.package.is_empty()
            && matches!(progress.stage, Stage::Download | Stage::Install)
            && !progress.cancelling
        {
            self.tr(if progress.stage == Stage::Download {
                "updates.step_download"
            } else {
                "updates.step_install"
            })
        } else {
            now
        };
        self.note((LEFT, 94., WIDE - 60., 16.), &now, 12.)?;
        self.text_face(
            (LEFT + WIDE - 60., 94., 60., 16.),
            &format!("{percent}%"),
            (12., theme.muted, Align::Right),
            Face::Mono,
        )?;
        let track = self.rect((LEFT, 120., WIDE, 8.));
        self.ui
            .rounded(track, self.ui.px(4.) as f32, theme.border, None)?;
        if percent > 0 {
            let fill = (WIDE * f32::from(percent.min(100)) / 100.).max(8.);
            let bar = self.rect((LEFT, 120., fill, 8.));
            self.ui
                .rounded(bar, self.ui.px(4.) as f32, theme.highlight_bg, None)?;
        }
        let download = if progress.stage == Stage::Download && progress.total > 0 {
            let mut text = self.ui.catalog.format(
                "updates.of",
                &[
                    ("done", &self.size(progress.bytes)),
                    ("total", &self.size(progress.total)),
                ],
            );
            if progress.speed > 0 {
                let speed = self.ui.catalog.format(
                    "updates.per_second",
                    &[("size", &self.size(progress.speed))],
                );
                text = format!("{text} · {speed}");
            }
            text
        } else {
            let total = state.updates.report.as_ref().map_or(0, |r| r.download);
            if progress.total > 0 || total > 0 {
                self.size(progress.total.max(total))
            } else {
                String::new()
            }
        };
        let install = if progress.stage == Stage::Install && !progress.package.is_empty() {
            self.ui.catalog.format(
                "updates.install_now",
                &[
                    ("done", &progress.current.to_string()),
                    ("count", &count.to_string()),
                    ("name", &progress.package),
                ],
            )
        } else {
            self.tr(&format!("updates.package_count|count={count}"))
        };
        let steps = [
            (Stage::Sync, "updates.step_sync", "pacman -Sy".to_owned()),
            (Stage::Download, "updates.step_download", download),
            (Stage::Install, "updates.step_install", install),
            (
                Stage::Hooks,
                "updates.step_hooks",
                self.tr("updates.step_hooks_detail"),
            ),
        ];
        for (n, (stage, title, detail)) in steps.iter().enumerate() {
            let y = 138. + n as f32 * STEP;
            self.step((LEFT, y), progress.stage.cmp(stage), title, detail)?;
        }
        Ok(())
    }
    /// One step: a tick when done, a turning ring while running, an empty
    /// ring before. `order` is the run's stage against this step.
    fn step(
        &mut self,
        (x, y): (f32, f32),
        order: std::cmp::Ordering,
        title: &str,
        detail: &str,
    ) -> Result<(), String> {
        use std::cmp::Ordering::{Equal, Greater};
        let theme = self.theme;
        let mark = self.rect((x, y + 12.5, 20., 20.));
        match order {
            Greater => {
                self.ui
                    .rounded(mark, mark.height() as f32 / 2., theme.success, None)?;
                let tick = self.rect((x + 4., y + 16.5, 12., 12.));
                self.ui.icon(Glyph::Check, tick, theme.highlight_text)?;
            }
            Equal => {
                let secs = self
                    .ui
                    .frame
                    .now
                    .saturating_duration_since(self.ui.born)
                    .as_secs_f64();
                let angle = if self.ui.motion == crate::motion::Motion::Off {
                    0.
                } else {
                    (secs * 360.) % 360.
                };
                self.ui
                    .icon_turned(Glyph::Loader, mark, theme.highlight_bg, angle)?;
            }
            _ => {
                let ring = mix(theme.border, theme.muted, 0.12);
                self.ui.rounded(
                    mark,
                    mark.height() as f32 / 2.,
                    theme.background,
                    Some(ring),
                )?;
            }
        }
        let ink = if order == std::cmp::Ordering::Less {
            theme.dim_text
        } else {
            theme.text
        };
        self.text(
            (x + 32., y + 6., WIDE - 32., 18.),
            &self.tr(title),
            (14., ink, Align::Left),
        )?;
        self.note((x + 32., y + 24., WIDE - 32., 16.), detail, 12.)
    }
    /// Centered result: a round mark, the title, two lines, an optional
    /// pacman line and the two answers (Atualizacoes-concluida/-erro).
    fn update_result(
        &mut self,
        (glyph, ink): (Glyph, Rgba),
        title: &str,
        body: &str,
        line: &str,
        buttons: [&str; 2],
    ) -> Result<(), String> {
        let theme = self.theme;
        let width = self.rect((0., 0., 520., 1.)).width();
        let box_lines = if line.is_empty() {
            Vec::new()
        } else {
            self.lines_face(line, 11., width - self.ui.px(20.) as u32, Face::Mono)?
        };
        let body_height = body.lines().count().max(1) as f32 * 20.;
        let box_height = if box_lines.is_empty() {
            0.
        } else {
            box_lines.len() as f32 * 14. + 16. + 12.
        };
        let total = 56. + 12. + 26. + 12. + body_height + 12. + box_height + 6. + 38.;
        let mut y = (44. + (FOOTER - 44. - total) / 2.).round();
        let mark = self.rect((292., y, 56., 56.));
        self.ui
            .rounded(mark, mark.height() as f32 / 2., self.tinted(ink), None)?;
        let icon = self.rect((306., y + 14., 28., 28.));
        self.ui.icon(glyph, icon, ink)?;
        y += 68.;
        self.text_face(
            (40., y, 560., 26.),
            title,
            (20., theme.text, Align::Center),
            Face::Semibold,
        )?;
        y += 38.;
        self.text(
            (40., y, 560., body_height),
            body,
            (13., theme.muted, Align::Center),
        )?;
        y += body_height + 12.;
        if !box_lines.is_empty() {
            let mut box_width = 0.;
            for text in &box_lines {
                let w = self.ui.measure(text, Face::Mono, 11.)? as f32 / self.ui.frame.scale;
                box_width = f32::max(box_width, w);
            }
            let box_width = box_width + 22.;
            let left = 320. - box_width / 2.;
            let height = box_height - 12.;
            self.ui.rounded(
                self.rect((left, y, box_width, height)),
                self.ui.px(8.) as f32,
                theme.background,
                Some(theme.border),
            )?;
            for (n, text) in box_lines.iter().enumerate() {
                self.text_face(
                    (left + 10., y + 8. + n as f32 * 14., box_width - 20., 14.),
                    text,
                    (11., theme.muted, Align::Center),
                    Face::Mono,
                )?;
            }
            y += box_height;
        }
        y += 6.;
        let labels = buttons.map(|key| self.tr(key));
        let mut widths = [0.; 2];
        for (n, label) in labels.iter().enumerate() {
            let face = if n == 0 {
                Face::Semibold
            } else {
                Face::Regular
            };
            widths[n] = self.ui.measure(label, face, 14.)? as f32 / self.ui.frame.scale + 44.;
        }
        let shown = if buttons[1].is_empty() { 1 } else { 2 };
        let row: f32 = widths[..shown].iter().sum::<f32>() + 10. * (shown - 1) as f32;
        let mut x = 320. - row / 2.;
        for n in 0..shown {
            let bounds = (x, y, widths[n], 38.);
            let primary = n == 0;
            self.ui.rounded(
                self.rect(bounds),
                self.ui.px(10.) as f32,
                if primary {
                    theme.highlight_bg
                } else {
                    theme.surface
                },
                (!primary).then_some(theme.border),
            )?;
            let (ink, face) = if primary {
                (theme.highlight_text, Face::Semibold)
            } else {
                (theme.text, Face::Regular)
            };
            self.text_face(bounds, &labels[n], (14., ink, Align::Center), face)?;
            x += widths[n] + 10.;
        }
        Ok(())
    }
    fn update_done(
        &mut self,
        reboot: Option<Reboot>,
        from: &str,
        to: &str,
        count: usize,
    ) -> Result<(), String> {
        let packages = self.tr(&format!("updates.updated|count={count}"));
        let first = if !from.is_empty() && !to.is_empty() && from != to {
            self.ui.catalog.format(
                "updates.version_change",
                &[("from", from), ("to", to), ("packages", &packages)],
            )
        } else {
            packages
        };
        let (second, buttons) = match reboot {
            Some(Reboot::Kernel) => (
                "updates.restart_kernel",
                ["updates.restart_now", "settings.later"],
            ),
            Some(Reboot::Launcher) => (
                "updates.restart_launcher",
                ["updates.restart_now", "settings.later"],
            ),
            None => ("updates.ready", ["updates.close", ""]),
        };
        let body = format!("{first}\n{}", self.tr(second));
        self.update_result(
            (Glyph::Check, self.theme.success),
            &self.tr("updates.done"),
            &body,
            "",
            buttons,
        )
    }
    fn update_failed(
        &mut self,
        state: &Settings,
        reason: &str,
        line: &str,
        changed: bool,
    ) -> Result<(), String> {
        let version = state
            .updates
            .report
            .as_ref()
            .map_or(state.info.version.as_str(), |r| r.version.as_str());
        let known = [
            "network", "battery", "space", "busy", "seed", "sync", "download", "install",
        ];
        let reason = if known.contains(&reason) {
            reason
        } else {
            "unknown"
        };
        let after = if changed {
            self.tr("updates.partly_installed")
        } else {
            self.ui
                .catalog
                .format("updates.nothing_installed", &[("version", version)])
        };
        let body = format!("{}\n{after}", self.tr(&format!("updates.why_{reason}")));
        self.update_result(
            (Glyph::Close, self.theme.danger),
            &self.tr(&format!("updates.failed_{reason}")),
            &body,
            line,
            ["updates.retry", "hint.back"],
        )
    }
    /// Footer note of the page: the source, the cancel rule or the log.
    pub(super) fn updates_status(&self, state: &Settings) -> Option<String> {
        match &state.updates.run {
            _ if state.page() == Page::UpdatePackages => None,
            None => Some(self.tr("updates.source")),
            Some(Run::Working(_)) => Some(self.tr("updates.cancel_rule")),
            Some(Run::Failed { .. }) => {
                Some(self.ui.catalog.format("updates.log", &[("path", LOG)]))
            }
            Some(Run::Done { .. }) => None,
        }
    }
    /// Breadcrumb of the run screens: "System / Updates / Installing".
    pub(super) fn updates_trail(&self, state: &Settings) -> Option<(Vec<String>, String)> {
        let current = match state.updates.run.as_ref()? {
            Run::Working(_) => "updates.installing",
            Run::Done { .. } => "updates.done_short",
            Run::Failed { .. } => "updates.failed_short",
        };
        Some((
            vec![self.tr("settings.system"), self.tr("updates.title")],
            self.tr(current),
        ))
    }
}
