//! BIOS (Bios, Bios-livre): the systems on the left with their verdict, the
//! open system's files and the free BIOS on the right, and the question
//! before installing one. Fixed layout, like every settings page; the
//! theme only gives colours, fonts and icons.
use super::*;
use crate::settings::bios::{FileState, System, Verdict};

const ROW: f32 = 42.;
const LIST_RIGHT: f32 = 342.;
const PANEL_X: f32 = 358.;
const PANEL_W: f32 = 262.;
const BUTTON_TOP: f32 = 384.;

impl View<'_, '_, '_> {
    pub(super) fn bios_page(&mut self, state: &Settings) -> Result<(), String> {
        let Some(systems) = &state.bios.systems else {
            return self.text(
                (40., VIEW_TOP + 20., 560., 60.),
                &self.tr("bios.reading"),
                (15., self.theme.muted, Align::Center),
            );
        };
        if systems.is_empty() {
            return self.text(
                (40., VIEW_TOP + 20., 560., 80.),
                &self.tr("bios.empty"),
                (15., self.theme.muted, Align::Center),
            );
        }
        let positions: Vec<Placement> = (0..systems.len())
            .map(|n| Placement {
                top: 48. + n as f32 * (ROW + ROW_GAP),
                height: ROW,
                section: None,
                searching: None,
                trail: 0.,
            })
            .collect();
        self.list(state, &positions, LIST_RIGHT, |view, n, y, _, selected| {
            view.bios_row(&systems[n], y, selected)
        })?;
        match state.bios_system() {
            Some(system) => self.bios_panel(system),
            None => Ok(()),
        }
    }
    fn bios_row(&mut self, system: &System, y: f32, selected: bool) -> Result<(), String> {
        let ink = self.ink(selected);
        let face = selection(selected, Face::Regular, Face::Semibold);
        let (key, tone) = verdict(system.verdict);
        let label = self.tr(key);
        let badge =
            self.ui.measure(&label, Face::Semibold, 11.)? as f32 / self.ui.frame.scale + 16.;
        let x = LIST_RIGHT - 12. - badge;
        let width = x - 10. - 28.;
        let quiet = selection(selected, self.theme.muted, self.theme.accent_text_secondary);
        // Long emulator names take the row's whole width; the pill moves
        // up beside the system's name.
        let cores =
            self.ui.measure(&system.cores, Face::Regular, 12.)? as f32 / self.ui.frame.scale;
        let (pill_y, cores_width) = if cores > width {
            (y + 4., LIST_RIGHT - 12. - 28.)
        } else {
            (y + (ROW - 19.) / 2., width)
        };
        self.text_face(
            (28., y + 1., width, 23.),
            &system.name,
            (15., ink, Align::Left),
            face,
        )?;
        self.text(
            (28., y + 21., cores_width, 18.),
            &system.cores,
            (12., quiet, Align::Left),
        )?;
        self.pill((x, pill_y, badge, 19.), &label, tone, selected)
    }
    /// A status pill: tinted fill and ring of its tone, white on the
    /// highlighted row.
    fn pill(
        &mut self,
        bounds: Bounds,
        label: &str,
        tone: Tone,
        selected: bool,
    ) -> Result<(), String> {
        let theme = self.theme;
        let base = match tone {
            Tone::Good => theme.success,
            Tone::Info => theme.highlight_bg,
            Tone::Warn => theme.warning,
            Tone::Bad => theme.danger,
            Tone::Quiet => theme.muted,
        };
        let (fill, edge, ink) = if selected {
            let white = |a| Rgba {
                a,
                ..theme.highlight_text
            };
            (white(46), white(89), theme.highlight_text)
        } else if tone == Tone::Quiet {
            (theme.surface_elevated, theme.border, theme.muted)
        } else {
            let ink = match tone {
                Tone::Info | Tone::Warn => mix(base, theme.text, 0.35),
                _ => base,
            };
            (
                mix(theme.background, base, 0.13),
                mix(theme.background, base, 0.33),
                ink,
            )
        };
        let rect = self.rect(bounds);
        self.ui
            .rounded(rect, self.ui.px(8.) as f32, fill, Some(edge))?;
        self.text_face(bounds, label, (11., ink, Align::Center), Face::Semibold)
    }
    /// The open system: what the cores do without a file, each file and its
    /// MD5, where to copy, and the free BIOS to install.
    fn bios_panel(&mut self, system: &System) -> Result<(), String> {
        let theme = self.theme;
        let note = self.tr(&format!("bios.note_{}", system.id));
        let note = if note.starts_with("bios.") {
            self.tr(if system.verdict == Verdict::Builtin {
                "bios.note_builtin"
            } else {
                "bios.note_required"
            })
        } else {
            note
        };
        let lines = self.lines(&note, 12., self.ui.px(PANEL_W) as u32)?;
        let step = 17.;
        let mut y = 48.;
        for line in &lines {
            self.text(
                (PANEL_X, y, PANEL_W, step),
                line,
                (12., theme.text_secondary, Align::Left),
            )?;
            y += step;
        }
        y += 6.;
        let free = system
            .free
            .as_deref()
            .map(|free| (free, system.free_installed));
        let bottom = if system.installable() {
            BUTTON_TOP - 30.
        } else {
            VIEW_BOTTOM - 24.
        };
        let entries = system.files.len() + usize::from(free.is_some());
        let room = ((bottom - y) / 46.).floor().max(0.) as usize;
        let shown = entries.min(room);
        let mut drawn = 0;
        if let Some((free, installed)) = free.filter(|_| shown > 0) {
            // The free BIOS first when the console's files would push it out.
            let files = system.files.len().min(shown - 1);
            for file in &system.files[..files] {
                y = self.bios_file(y, file.state, &file.file, &self.md5(&file.md5))?;
            }
            let (name, detail) = free_names(free, installed);
            let state = if installed {
                FileState::Free
            } else {
                FileState::Missing
            };
            y = self.bios_free(y, state, &self.tr(name), &self.tr(detail))?;
            drawn = files + 1;
        } else {
            for file in &system.files[..shown] {
                y = self.bios_file(y, file.state, &file.file, &self.md5(&file.md5))?;
                drawn += 1;
            }
        }
        if drawn < entries {
            let more = format!("bios.more|count={}", entries - drawn);
            self.text(
                (PANEL_X, y + 2., PANEL_W, 16.),
                &self.tr(&more),
                (12., theme.muted, Align::Left),
            )?;
            y += 18.;
        }
        // "Copy to roms/bios/".
        let y = y + 6.;
        self.ui.icon(
            Glyph::FolderOpen,
            self.rect((PANEL_X, y + 1., 14., 14.)),
            theme.muted,
        )?;
        let copy = self.tr("bios.copy_to");
        let width = self.ui.measure(&copy, Face::Regular, 12.)? as f32 / self.ui.frame.scale;
        self.text(
            (PANEL_X + 20., y, width + 1., 16.),
            &copy,
            (12., theme.muted, Align::Left),
        )?;
        self.text_face(
            (PANEL_X + 24. + width, y, PANEL_W - 24. - width, 16.),
            "roms/bios/",
            (12., theme.text, Align::Left),
            Face::Mono,
        )?;
        if system.installable() {
            self.icon_button(
                (PANEL_X, BUTTON_TOP, PANEL_W, 40.),
                true,
                Some((None, "download")),
                &self.tr("bios.install"),
            )?;
        }
        Ok(())
    }
    fn md5(&self, md5: &str) -> String {
        if md5.is_empty() {
            self.tr("bios.md5_any")
        } else {
            md5.to_owned()
        }
    }
    /// One file: its state icon and name, then its MD5 in monospace.
    fn bios_file(
        &mut self,
        y: f32,
        state: FileState,
        name: &str,
        detail: &str,
    ) -> Result<f32, String> {
        let theme = self.theme;
        let (glyph, ink) = match state {
            FileState::Ok => (Glyph::CircleCheck, theme.success),
            FileState::Missing => (Glyph::CircleX, theme.danger),
            FileState::Bad => (Glyph::CircleAlert, theme.warning),
            FileState::Free => (Glyph::CircleCheck, theme.highlight_bg),
        };
        let name = if state == FileState::Free {
            self.tr(&format!("bios.free_file|name={name}"))
        } else {
            name.to_owned()
        };
        self.bios_entry(y, (glyph, ink), &name, detail)
    }
    /// The free BIOS: dashed until installed.
    fn bios_free(
        &mut self,
        y: f32,
        state: FileState,
        name: &str,
        detail: &str,
    ) -> Result<f32, String> {
        let glyph = if state == FileState::Free {
            Glyph::CircleCheck
        } else {
            Glyph::CircleDashed
        };
        self.bios_entry(y, (glyph, self.theme.highlight_bg), name, detail)
    }
    fn bios_entry(
        &mut self,
        y: f32,
        (glyph, ink): (Glyph, Rgba),
        name: &str,
        detail: &str,
    ) -> Result<f32, String> {
        let theme = self.theme;
        self.ui
            .icon(glyph, self.rect((PANEL_X, y + 8., 14., 14.)), ink)?;
        self.text(
            (PANEL_X + 20., y + 5., PANEL_W - 20., 19.),
            name,
            (13., theme.text, Align::Left),
        )?;
        self.ui.label_aligned(
            detail,
            (Face::Mono, 10.5, theme.muted),
            self.rect((PANEL_X, y + 25., PANEL_W, 15.)),
            Align::Left,
        )?;
        self.ui.divider(
            self.ui.px(PANEL_X),
            self.ui.px(y + 45.),
            self.ui.px(PANEL_W) as u32,
            theme.border,
        )?;
        Ok(y + 46.)
    }
    /// A button: the module's own SVG or a shipped Lucide glyph by name,
    /// then the label, centred together.
    pub(super) fn icon_button(
        &mut self,
        bounds: Bounds,
        primary: bool,
        icon: Option<(Option<&str>, &str)>,
        label: &str,
    ) -> Result<(), String> {
        let theme = self.theme;
        let (fill, edge, ink, face) = if primary {
            (
                theme.highlight_bg,
                None,
                theme.highlight_text,
                Face::Semibold,
            )
        } else {
            (theme.badge, Some(theme.border), theme.text, Face::Regular)
        };
        self.ui
            .rounded(self.rect(bounds), self.ui.px(10.) as f32, fill, edge)?;
        let (x, y, w, h) = bounds;
        let text = self.ui.measure(label, face, 15.)? as f32 / self.ui.frame.scale;
        let room = if icon.is_some() { 23. } else { 0. };
        let left = x + ((w - text - room) / 2.).max(8.);
        if let Some(icon) = icon {
            let at = self.rect((left, y + (h - 15.) / 2., 15., 15.));
            self.ui.module_icon(icon, at, ink)?;
        }
        self.text_face(
            (left + room, y, w - (left + room - x) - 8., h),
            label,
            (15., ink, Align::Left),
            face,
        )
    }
    /// Bios-livre: the question, what is installed and from where, and the
    /// two answers; the footer stays above the scrim.
    pub(super) fn bios_question(&mut self, state: &Settings) -> Result<(), String> {
        let Some(system) = state.bios_system() else {
            return Ok(());
        };
        let theme = self.theme;
        let free = system.free.as_deref().unwrap_or_default();
        let (x, w) = (130., 380.);
        let inner = w - 40.;
        let title = self.tr(&format!("bios.install_question|name={}", system.name));
        let title_lines =
            self.lines_face(&title, 17., self.ui.px(inner - 48.) as u32, Face::Semibold)?;
        let head = (title_lines.len() as f32 * 21.).max(36.);
        let detail = self.tr(&format!("bios.install_detail_{free}"));
        let body = self.lines(&detail, 12., self.ui.px(inner) as u32)?;
        let facts = free_facts(free);
        let table = 8. + facts.len() as f32 * 25.;
        let height = 20. + head + 12. + table + 12. + body.len() as f32 * 17.4 + 14. + 40. + 18.;
        let top = 66.;
        self.ui.rounded(
            self.rect((x, top, w, height)),
            self.ui.px(16.) as f32,
            theme.surface,
            Some(theme.border),
        )?;
        let (left, mut y) = (x + 20., top + 20.);
        let circle = self.rect((left, y + (head - 36.) / 2., 36., 36.));
        let tint = Rgba {
            a: 36,
            ..theme.highlight_bg
        };
        self.ui
            .rounded(circle, self.ui.px(18.) as f32, tint, None)?;
        self.ui.icon(
            Glyph::Download,
            self.rect((left + 9., y + (head - 18.) / 2., 18., 18.)),
            theme.highlight_bg,
        )?;
        for (n, line) in title_lines.iter().enumerate() {
            let line_top = y + (head - title_lines.len() as f32 * 21.) / 2. + n as f32 * 21.;
            self.text_face(
                (left + 48., line_top, inner - 48., 21.),
                line,
                (17., theme.text, Align::Left),
                Face::Semibold,
            )?;
        }
        y += head + 12.;
        self.ui.rounded(
            self.rect((left, y, inner, table)),
            self.ui.px(10.) as f32,
            theme.background,
            Some(theme.border),
        )?;
        for (n, (key, value, mono)) in facts.iter().enumerate() {
            let row = y + 4. + n as f32 * 25.;
            self.text(
                (left + 12., row, 90., 25.),
                &self.tr(key),
                (12., theme.muted, Align::Left),
            )?;
            let value = self.tr(value);
            let face = if *mono { Face::Mono } else { Face::Regular };
            self.text_face(
                (left + 100., row, inner - 112., 25.),
                &value,
                (12., theme.text, Align::Right),
                face,
            )?;
            if n + 1 < facts.len() {
                self.ui.divider(
                    self.ui.px(left + 12.),
                    self.ui.px(row + 25.),
                    self.ui.px(inner - 24.) as u32,
                    theme.border,
                )?;
            }
        }
        y += table + 12.;
        for line in &body {
            self.text(
                (left, y, inner, 17.4),
                line,
                (12., theme.muted, Align::Left),
            )?;
            y += 17.4;
        }
        y += 14.;
        let half = (inner - 10.) / 2.;
        let install = state.bios.choice == 0;
        self.icon_button(
            (left, y, half, 40.),
            install,
            Some((None, "download")),
            &self.tr("bios.install_short"),
        )?;
        self.icon_button(
            (left + half + 10., y, half, 40.),
            !install,
            None,
            &self.tr("hint.cancel"),
        )?;
        // The footer of the mockup stays above the dim.
        self.ui
            .canvas
            .set_draw_color(crate::text::color(theme.background));
        self.ui
            .canvas
            .fill_rect(self.rect((0., FOOTER, 640., 480. - FOOTER)))?;
        self.ui
            .divider(0, self.ui.px(FOOTER), self.ui.frame.width, theme.border)?;
        self.ui
            .hints(
                state.hints(),
                theme,
                self.ui.frame.width,
                self.ui.px(FOOTER),
                self.ui.px(480. - FOOTER) as u32,
            )
            .map(|_| ())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Good,
    Info,
    Warn,
    Bad,
    Quiet,
}
fn verdict(verdict: Verdict) -> (&'static str, Tone) {
    match verdict {
        Verdict::Ok => ("bios.ok", Tone::Good),
        Verdict::Builtin => ("bios.builtin", Tone::Info),
        Verdict::Free => ("bios.free", Tone::Info),
        Verdict::Optional => ("bios.optional", Tone::Quiet),
        Verdict::Missing => ("bios.missing", Tone::Bad),
        Verdict::Bad => ("bios.bad", Tone::Warn),
    }
}
/// The free BIOS row: its name and what it is for.
fn free_names(free: &str, installed: bool) -> (&'static str, &'static str) {
    match (free, installed) {
        ("msx", false) => ("bios.cbios", "bios.cbios_missing"),
        ("msx", true) => ("bios.cbios", "bios.cbios_installed"),
        (_, false) => ("bios.openbios", "bios.openbios_missing"),
        (_, true) => ("bios.openbios", "bios.openbios_installed"),
    }
}
/// File, project, licence and origin of a free BIOS (catalog keys or text).
fn free_facts(free: &str) -> [(&'static str, &'static str, bool); 4] {
    let (file, project, license) = match free {
        "msx" => ("MSX.ROM · MSX2.ROM", "C-BIOS", "BSD-2-Clause"),
        _ => ("openbios.bin", "PCSX‑Redux", "MIT"),
    };
    [
        ("bios.fact_file", file, true),
        ("bios.fact_project", project, false),
        ("bios.fact_license", license, false),
        ("bios.fact_source", "bios.fact_package", false),
    ]
}
