//! System › Modules (Modulos), the module store list (Loja-modulos) and the
//! permission page shared by a store module and an installed one
//! (Loja-modulo). Fixed layout like every settings page.
use super::*;
use crate::modules::{Module, Permission, Status, Tab};
use crate::settings::modules::SECTION;

const LIST_TOP: f32 = 82.;
const CARD: f32 = 54.;
const CARD_GAP: f32 = 6.;
/// The permission page: left column, then the list of permissions.
const LEFT: (f32, f32) = (20., 334.);
const ASKS: (f32, f32) = (370., 250.);
const BUTTON: f32 = 384.;

impl View<'_, '_, '_> {
    /// "1.0.2 · Revisado · bloco no Start", or why it is off.
    pub(super) fn module_line(&self, module: &Module) -> (String, bool) {
        let tr = |key: &str| self.tr(key);
        let warn = |key: &str| (tr(key), true);
        match module.status {
            Status::Failed => return warn("modules.status_failed"),
            Status::Incompatible => return warn("modules.status_incompatible"),
            Status::Invalid => return warn("modules.status_invalid"),
            Status::Ready if !module.granted => return warn("modules.status_consent"),
            Status::Ready => {}
        }
        let mut parts = vec![match &module.update {
            Some(next) => self.tr(&format!(
                "modules.update_line|from={}|to={next}",
                module.version
            )),
            None => module.version.clone(),
        }];
        parts.push(tr(match (module.reviewed, module.source.is_empty()) {
            (true, _) => "modules.reviewed",
            (false, true) => "modules.local",
            (false, false) => "modules.not_reviewed",
        }));
        // The first place it shows up: "bloco no Start".
        let uses = module.permissions.iter().find_map(|p| match p.key {
            "modules.perm_tile" => Some("modules.uses_tile"),
            "modules.perm_shortcut" => Some("modules.uses_shortcut"),
            "modules.perm_quick" => Some("modules.uses_quick"),
            "modules.perm_page" => Some("modules.uses_page"),
            _ => None,
        });
        parts.extend(uses.map(tr));
        (parts.join(" · "), false)
    }
    /// The modules list: switches, a rule, the store.
    pub(super) fn modules_list(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let positions = placements(Page::Modules, rows);
        self.rows_at(state, rows, &positions, RIGHT)?;
        if let [.., before, last] = positions.as_slice() {
            let rule = (before.bottom() + last.top) / 2.;
            self.ui.divider(
                self.ui.px(28.),
                self.ui.px(rule),
                self.ui.px(RIGHT - 40.) as u32,
                self.theme.border,
            )?;
        }
        if state.modules.view.modules.is_empty() {
            self.note((30., 50., 560., 44.), &self.tr("modules.none"), 14.)?;
        }
        Ok(())
    }
    /// Loja-modulos: tabs, then one card per repository.
    pub(super) fn module_store(&mut self, state: &Settings) -> Result<(), String> {
        let modules = &state.modules;
        let theme = self.theme;
        let keys = Tab::ALL.map(Tab::key);
        let note = format!("store.source|topic={}", "archr-launcher-module");
        self.tabs(&keys, modules.tab as usize, &note)?;
        let cards = modules.cards();
        if cards.is_empty() {
            let key = match modules.problem {
                _ if modules.loading => "modules.searching",
                Some(problem) => problem,
                None if modules.tab == Tab::Installed => "modules.none_installed",
                None => "modules.empty",
            };
            return self.note((24., LIST_TOP, 592., 44.), &self.tr(key), 14.);
        }
        let step = CARD + CARD_GAP;
        let visible = ((FOOTER - LIST_TOP) / step).floor().max(1.) as usize;
        let first = state.cursor.saturating_sub(visible - 1);
        let installed: Vec<(&str, &str)> = modules
            .view
            .modules
            .iter()
            .map(|m| (m.source.as_str(), m.version.as_str()))
            .collect();
        for (n, card) in cards.iter().enumerate().skip(first).take(visible) {
            let y = LIST_TOP + (n - first) as f32 * step;
            let selected = n == state.cursor;
            let (fill, ink) = selection(
                selected,
                (theme.surface, theme.text),
                (theme.highlight_bg, theme.highlight_text),
            );
            self.ui.rounded(
                self.rect((20., y, 600., CARD)),
                self.ui.px(12.) as f32,
                fill,
                None,
            )?;
            let back = if selected {
                Rgba {
                    a: 34,
                    ..theme.highlight_text
                }
            } else {
                theme.badge
            };
            self.ui.rounded(
                self.rect((32., y + 10., 34., 34.)),
                self.ui.px(9.) as f32,
                back,
                None,
            )?;
            let icon = self.rect((40., y + 18., 18., 18.));
            let glyph_ink = selection(selected, theme.text_secondary, theme.highlight_text);
            self.ui.module_icon((None, &card.icon), icon, glyph_ink)?;
            let source = card.source();
            let right = match installed.iter().find(|(s, _)| *s == source) {
                Some((_, version)) if !card.version.is_empty() && *version != card.version => {
                    Some(("modules.update", theme.muted))
                }
                Some(_) => Some(("store.installed_badge", theme.success)),
                None => None,
            };
            let right_x = match right {
                Some((key, color)) => {
                    let label = self.tr(key);
                    let w =
                        self.ui.measure(&label, Face::Regular, 12.)? as f32 / self.ui.frame.scale;
                    let ink = if selected {
                        theme.highlight_text
                    } else {
                        color
                    };
                    self.text((608. - w, y, w, CARD), &label, (12., ink, Align::Right))?;
                    608. - w - 12.
                }
                None if card.stars > 0 => {
                    let ink = selection(selected, theme.muted, theme.highlight_text);
                    self.stars(card.stars, 608., (y, CARD), (12., ink))? - 12.
                }
                None => 608.,
            };
            let face = selection(selected, Face::Regular, Face::Semibold);
            let title_w = (self.ui.measure(card.title(), face, 15.)? as f32 / self.ui.frame.scale)
                .min(right_x - 78. - 90.);
            self.plain(card.title(), (78., y + 8., title_w, 20.), (face, 15., ink))?;
            if card.reviewed {
                self.badge(78. + title_w + 8., y + 10.)?;
            }
            let detail = if card.description.is_empty() {
                card.owner.clone()
            } else {
                format!("{} · {}", card.description, card.owner)
            };
            let sub = selection(selected, theme.muted, theme.accent_text_secondary);
            self.plain(
                &detail,
                (78., y + 29., right_x - 78., 17.),
                (Face::Regular, 12., sub),
            )?;
        }
        Ok(())
    }
    /// The green "Revisado" badge with its shield.
    fn badge(&mut self, x: f32, y: f32) -> Result<(), String> {
        let theme = self.theme;
        let label = self.tr("modules.reviewed");
        let w = self.ui.measure(&label, Face::Regular, 10.)? as f32 / self.ui.frame.scale + 26.;
        self.ui.rounded(
            self.rect((x, y, w, 16.)),
            self.ui.px(8.) as f32,
            crate::appearance::mix(theme.background, theme.success, 0.18),
            None,
        )?;
        self.ui.icon(
            Glyph::ShieldCheck,
            self.rect((x + 6., y + 2.5, 11., 11.)),
            theme.success,
        )?;
        self.text(
            (x + 20., y, w - 22., 16.),
            &label,
            (10., theme.success, Align::Left),
        )
    }
    /// Loja-modulo, for a store offer or an installed module.
    pub(super) fn module_offer(&mut self, state: &Settings) -> Result<(), String> {
        let theme = self.theme;
        let installed = state.page() == Page::ModuleInfo;
        let (title, icon, meta, description, preview, asks, added, reviewed) = if installed {
            let Some(module) = state.modules.module() else {
                return Ok(());
            };
            let meta = [
                module.author.clone(),
                self.tr(&format!("modules.version|version={}", module.version)),
                module.license.clone(),
            ];
            let note = match (module.status, module.reviewed, module.source.is_empty()) {
                (Status::Invalid, ..) => module.problem.clone(),
                (Status::Incompatible, ..) => self.tr("modules.incompatible"),
                (_, true, _) => self.tr("modules.reviewed_note"),
                (_, false, true) => self.tr("modules.local_note"),
                _ => self.tr("modules.not_reviewed_note"),
            };
            (
                module.name.clone(),
                (module.icon.clone(), String::new()),
                meta.into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
                sentences(&module.description, &note),
                None,
                module.permissions.clone(),
                Vec::new(),
                module.reviewed,
            )
        } else {
            let Some(offer) = &state.modules.offer else {
                let key = state.modules.problem.unwrap_or("store.loading");
                return self.note((24., 60., 592., 44.), &self.tr(key), 14.);
            };
            let size = if offer.size >= 1 << 20 {
                self.tr(&format!(
                    "store.megabytes|count={:.1}",
                    offer.size as f64 / f64::from(1 << 20)
                ))
            } else {
                self.tr(&format!(
                    "store.kilobytes|count={}",
                    offer.size.div_ceil(1024)
                ))
            };
            let meta = [
                offer.author.clone(),
                self.tr(&format!("modules.version|version={}", offer.version)),
                size,
                offer.license.clone(),
            ];
            let note = self.tr(if offer.reviewed {
                "modules.reviewed_note"
            } else {
                "modules.not_reviewed_note"
            });
            (
                offer.card.title().to_owned(),
                (None, offer.card.icon.clone()),
                meta.into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
                sentences(&offer.card.description, &note),
                offer.preview.clone(),
                offer.permissions.clone(),
                offer.added.clone(),
                offer.reviewed,
            )
        };
        let (x, w) = LEFT;
        let top = 52.;
        self.ui.rounded(
            self.rect((x, top, 48., 48.)),
            self.ui.px(12.) as f32,
            theme.badge,
            None,
        )?;
        self.ui.module_icon(
            (icon.0.as_deref(), &icon.1),
            self.rect((x + 12., top + 12., 24., 24.)),
            theme.text_secondary,
        )?;
        let title_w = w - 60.;
        let mut text_x = x + 60.;
        let name_w = (self.ui.measure(&title, Face::Semibold, 19.)? as f32 / self.ui.frame.scale)
            .min(title_w);
        self.plain(
            &title,
            (text_x, top + 3., name_w, 26.),
            (Face::Semibold, 19., theme.text),
        )?;
        if reviewed && name_w + 90. < title_w {
            self.badge(text_x + name_w + 8., top + 8.)?;
        }
        text_x = x + 60.;
        self.plain(
            &meta,
            (text_x, top + 29., title_w, 17.),
            (Face::Regular, 12., theme.muted),
        )?;
        let below = self.flow(
            (x, top + 58., w),
            &description,
            (12., theme.text_secondary, Face::Regular),
            4,
        )?;
        let picture = (x, below + 8., w, (BUTTON + 36. - below - 8.).min(118.));
        if picture.3 > 40. {
            self.ui.rounded(
                self.rect(picture),
                self.ui.px(10.) as f32,
                theme.surface,
                Some(theme.border),
            )?;
            let inner = (
                picture.0 + 1.,
                picture.1 + 1.,
                picture.2 - 2.,
                picture.3 - 2.,
            );
            if !self.picture(preview.as_deref(), inner, false)? {
                self.text(
                    picture,
                    &self.tr("modules.no_preview"),
                    (11., theme.dim_text, Align::Center),
                )?;
            }
        }
        self.asks(state, &asks, &added)
    }
    /// The permission column: amber for the sensitive ones, then the button.
    fn asks(
        &mut self,
        state: &Settings,
        asks: &[Permission],
        added: &[Permission],
    ) -> Result<(), String> {
        let theme = self.theme;
        let (x, w) = ASKS;
        let (heading, list) = if added.is_empty() {
            ("modules.asks", asks)
        } else {
            ("modules.asks_more", added)
        };
        self.text(
            (x, 50., w, 18.),
            &self.tr(heading).to_uppercase(),
            (11., theme.muted, Align::Left),
        )?;
        let mut y = 72.;
        let button = state.offer_action();
        let bottom = if button.is_some() {
            BUTTON - 8.
        } else {
            FOOTER - 8.
        };
        for (n, permission) in list.iter().enumerate() {
            let detail = if permission.detail.is_empty() {
                None
            } else {
                Some(self.tr(&permission.detail))
            };
            let height = if detail.is_some() { 40. } else { 30. };
            if y + height > bottom {
                let rest = self.tr(&format!("modules.more|count={}", list.len() - n));
                self.text((x, y, w, 18.), &rest, (11., theme.muted, Align::Left))?;
                break;
            }
            let (fill, edge, dot) = if permission.sensitive {
                (
                    crate::appearance::mix(theme.background, theme.warning, 0.12),
                    crate::appearance::mix(theme.background, theme.warning, 0.35),
                    theme.warning,
                )
            } else {
                (theme.surface, theme.border, theme.success)
            };
            self.ui.rounded(
                self.rect((x, y, w, height)),
                self.ui.px(9.) as f32,
                fill,
                Some(edge),
            )?;
            self.ui.rounded(
                self.rect((x + 10., y + height / 2. - 4., 8., 8.)),
                self.ui.px(4.) as f32,
                dot,
                None,
            )?;
            let label = self.tr(permission.key);
            match detail {
                Some(detail) => {
                    self.plain(
                        &label,
                        (x + 28., y + 4., w - 36., 18.),
                        (Face::Regular, 13., theme.text),
                    )?;
                    self.plain(
                        &detail,
                        (x + 28., y + 21., w - 36., 15.),
                        (Face::Regular, 11., theme.muted),
                    )?;
                }
                None => self.text(
                    (x + 28., y, w - 36., height),
                    &label,
                    (13., theme.text, Align::Left),
                )?,
            }
            y += height + 6.;
        }
        if let Some(action) = button {
            let installed = action == "modules.installed";
            let (fill, ink) = if installed {
                (theme.surface, theme.success)
            } else {
                (theme.highlight_bg, theme.highlight_text)
            };
            let rect = (x, BUTTON, w, 36.);
            self.ui
                .rounded(self.rect(rect), self.ui.px(10.) as f32, fill, None)?;
            self.text_face(
                rect,
                &self.tr(action),
                (14., ink, Align::Center),
                Face::Semibold,
            )?;
        }
        Ok(())
    }
    /// The footer note of the module pages.
    pub(super) fn modules_status(&self, state: &Settings) -> Option<String> {
        let modules = &state.modules;
        Some(match state.page() {
            Page::ModuleStore => match modules.problem {
                Some(problem) if !modules.cards().is_empty() => self.tr(problem),
                _ if modules.loading && !modules.cards().is_empty() => self.tr("modules.searching"),
                _ => self.tr("modules.not_in_games"),
            },
            Page::ModuleOffer | Page::ModuleInfo => self.tr("modules.isolated"),
            _ => return None,
        })
    }
    /// The heading of a module page row.
    pub(super) fn module_section(
        &self,
        state: &Settings,
        section: &str,
        n: usize,
    ) -> Option<String> {
        (section == SECTION).then(|| state.section_text(n))
    }
}

/// A description and a note as one paragraph.
fn sentences(first: &str, second: &str) -> String {
    match first.trim() {
        "" => second.to_owned(),
        text if text.ends_with(['.', '!', '?']) => format!("{text} {second}"),
        text => format!("{text}. {second}"),
    }
}
