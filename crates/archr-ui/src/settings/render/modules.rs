//! System › Modules (Modulos), the module store list (Loja-modulos) and the
//! permission page shared by a store module and an installed one
//! (Loja-modulo). Fixed layout like every settings page.
use super::*;
use crate::modules::{Item, Module, Permission, RowKind, Status, Tab, Tone, STATS};
use crate::settings::modules::{SECTION, SECTION_NEXT};
use crate::HintButton;

const LIST_TOP: f32 = 82.;
const CARD: f32 = 54.;
const CARD_GAP: f32 = 6.;
const EMPTY_NOTE: f32 = 52.;
/// The permission page: left column, then the list of permissions.
const LEFT: (f32, f32) = (20., 334.);
const ASKS: (f32, f32) = (370., 250.);
const BUTTON: f32 = 384.;
/// The `stats` cards over a module page's rows, and the room they take.
const CARDS: (f32, f32) = (48., 78.);
const CARDS_ROOM: f32 = 88.;
/// The detail page (Modulo-ajuste-jogo): its columns, and the picture.
const DETAIL_LEFT: (f32, f32) = (20., 348.);
const DETAIL_SIDE: (f32, f32) = (384., 236.);
const PICTURE: (f32, f32) = (96., 72.);

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
        // "atualizado sozinho ontem" for a week after the store did it.
        let week = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            .saturating_sub(7 * 86400);
        if module.auto_updated > week {
            let when = self.when(module.auto_updated);
            parts.push(self.tr(&format!("modules.auto_updated_line|when={when}")));
            return (parts.join(" · "), false);
        }
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
        let mut positions = placements(Page::Modules, rows);
        let empty = state.modules.view.modules.is_empty();
        if empty {
            // The note takes the top; the store row moves below it.
            for place in &mut positions {
                place.top += EMPTY_NOTE;
            }
        }
        self.rows_at(state, rows, &positions, RIGHT)?;
        // The rule parts the modules from the switch and the store.
        let count = state.modules.view.modules.len();
        if let (Some(before), Some(after)) = (
            count.checked_sub(1).and_then(|n| positions.get(n)),
            positions.get(count),
        ) {
            let rule = (before.bottom() + after.top) / 2.;
            self.ui.divider(
                self.ui.px(28.),
                self.ui.px(rule),
                self.ui.px(RIGHT - 40.) as u32,
                self.theme.border,
            )?;
        }
        if empty {
            let top = positions.first().map_or(VIEW_TOP, |p| p.top) - EMPTY_NOTE;
            self.note(
                (30., top, 560., EMPTY_NOTE - 8.),
                &self.tr("modules.none"),
                14.,
            )?;
        }
        Ok(())
    }
    /// A module's own page: its rows, a `progress` row as a title, the
    /// module's text on the right and a bar under them; `stats` cards on
    /// top (API 3); or the detail layout.
    pub(super) fn module_page(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let page = state.page();
        let declared = state.modules.declared(page);
        if declared.is_some_and(|p| p.detail) {
            return self.module_detail(state, rows);
        }
        let cards = state
            .modules
            .page_items(page, |k| matches!(k, RowKind::Stats { .. }));
        let mut positions = placements(page, rows);
        let top = if cards.is_empty() {
            VIEW_TOP
        } else {
            self.stat_cards(&cards[..cards.len().min(STATS)])?;
            positions.iter_mut().for_each(|p| p.top += CARDS_ROOM);
            VIEW_TOP + CARDS_ROOM
        };
        self.list_from(
            state,
            &positions,
            (top, RIGHT),
            |view, n, y, position, selected| {
                let row = &rows[n];
                if let Some(item) = state
                    .module_item(row.id)
                    .filter(|i| !i.badge.is_empty() || !i.detail.is_empty())
                {
                    return view.item_row(state, row, item, (y, position.height), selected);
                }
                let Some(progress) = state.module_progress(row.id) else {
                    return view.row(state, row, (y, position.height), RIGHT, selected);
                };
                let theme = view.theme;
                let value = state.display_value(row.id, &view.ui.catalog);
                let value_w = view.width(&value, Face::Mono, 12.)?;
                view.text(
                    (30., y + 4., RIGHT - 72. - value_w, 20.),
                    &state.label(row),
                    (15., theme.text, Align::Left),
                )?;
                view.plain(
                    &value,
                    (RIGHT - 30. - value_w, y + 6., value_w + 1., 18.),
                    (Face::Mono, 12., theme.muted),
                )?;
                view.bar(
                    (30., y + 30., RIGHT - 60., 6.),
                    progress,
                    theme.highlight_bg,
                )
            },
        )
    }
    /// A tone's ink: theme colours only.
    fn tone(&self, tone: Tone) -> Rgba {
        let theme = self.theme;
        match tone {
            Tone::Plain => theme.text,
            Tone::Good => theme.success,
            Tone::Info => mix(theme.highlight_bg, theme.text, 0.45),
            Tone::Bad => theme.danger,
            Tone::Muted => theme.muted,
        }
    }
    /// A tag in the tone's colour, right-aligned; returns its width.
    fn module_tag(
        &mut self,
        text: &str,
        tone: Tone,
        (right, y): (f32, f32),
    ) -> Result<f32, String> {
        let (ink, back) = (self.tone(tone), self.theme.background);
        let colors = (mix(back, ink, 0.14), mix(back, ink, 0.4), ink);
        let at = (self.ui.px(right), self.ui.px(y + 9.5));
        Ok(self.ui.tag(text, at, colors)? as f32 / self.ui.frame.scale)
    }
    /// A name on the left and its value on the right, over a rule.
    fn fact_line(
        &mut self,
        (x, y, w): (f32, f32, f32),
        (label, value): (&str, &str),
        (face, ink): (Face, Rgba),
    ) -> Result<(), String> {
        let theme = self.theme;
        let value_w = self.width(value, face, 12.)?.min(w * 0.55);
        self.plain(
            label,
            (x, y, w - value_w - 8., 26.),
            (Face::Regular, 12., theme.muted),
        )?;
        self.plain(
            value,
            (x + w - value_w, y, value_w + 1., 26.),
            (face, 12., ink),
        )?;
        let (left, top) = (self.ui.px(x), self.ui.px(y + 26.));
        self.ui
            .divider(left, top, self.ui.px(w) as u32, theme.border)
    }
    /// Up to three cards over the rows: name, big value, a line under it.
    fn stat_cards(&mut self, cards: &[&Item]) -> Result<(), String> {
        let theme = self.theme;
        let (top, height) = CARDS;
        let gap = 10.;
        let w = (604. - gap * (cards.len() as f32 - 1.)) / cards.len() as f32;
        for (n, card) in cards.iter().enumerate() {
            let x = 16. + n as f32 * (w + gap);
            self.panel((x, top, w, height), false)?;
            let (x, w) = (x + 12., w - 24.);
            let title = card.title.to_uppercase();
            self.text(
                (x, top + 9., w, 15.),
                &title,
                (11., theme.muted, Align::Left),
            )?;
            let value = (22., self.tone(card.tone), Align::Left);
            self.text_face((x, top + 25., w, 29.), &card.value, value, Face::Semibold)?;
            let line = (11., theme.dim_text, Align::Left);
            self.text((x, top + 55., w, 15.), &card.subtitle, line)?;
        }
        Ok(())
    }
    /// A list item with a value or a tag on top and a line under it.
    fn item_row(
        &mut self,
        state: &Settings,
        row: &Row,
        item: &Item,
        (y, height): (f32, f32),
        selected: bool,
    ) -> Result<(), String> {
        let theme = self.theme;
        let ink = self.ink(selected);
        let soft = selection(selected, theme.muted, theme.accent_text_secondary);
        let face = selection(selected, Face::Regular, Face::Semibold);
        let right = RIGHT - 14.;
        let half = ((height - 4.) / 2.).floor();
        let (top, below) = (y + 4., y + height - half - 3.);
        let mut side = self.width(&item.detail, Face::Regular, 12.)?;
        if item.badge.is_empty() {
            let w = self.width(&item.value, Face::Mono, 14.)?;
            side = side.max(w);
            self.plain(
                &item.value,
                (right - w, top, w + 1., half),
                (Face::Mono, 14., ink),
            )?;
        } else {
            side = side.max(self.module_tag(&item.badge, item.tone, (right, top + 1.))?);
        }
        let detail = (right - side - 2., below, side + 2., half);
        self.text(detail, &item.detail, (12., soft, Align::Right))?;
        let width = right - side - 46.;
        let label = (15., ink, Align::Left);
        self.text_face((30., top, width, half), &state.label(row), label, face)?;
        self.text(
            (30., below, width, half),
            &item.subtitle,
            (12., soft, Align::Left),
        )
    }
    /// Modulo-ajuste-jogo: the item's picture, name and tag, its bars and
    /// note on the left; its facts and the action buttons on the right.
    /// Nothing about another item shows while the module answers.
    fn module_detail(&mut self, state: &Settings, rows: &[Row]) -> Result<(), String> {
        let theme = self.theme;
        let page = state.page();
        let header = state.modules.detail_header(page);
        if let Some(header) = header {
            self.detail_header(state, header)?;
            let (x, w) = DETAIL_LEFT;
            let mut y = 50. + PICTURE.1 + 12.;
            let declared = state.modules.declared(page).map_or(&[][..], |p| &p.rows);
            for row in declared {
                match &row.kind {
                    RowKind::Bars { id } => {
                        let title = row.label.to_uppercase();
                        y = self.flow((x, y, w), &title, (11., theme.muted, Face::Regular), 2)?;
                        for bar in state.modules.row_items(page, id) {
                            self.detail_bar(bar, (x, y + 6., w))?;
                            y += 22.;
                        }
                        y += 10.;
                    }
                    RowKind::Text { id } => {
                        let module = state.modules.module().map_or("", |m| m.id.as_str());
                        let target = format!("row:{id}");
                        let view = &state.modules.view;
                        let text = view.value(module, &target).unwrap_or(&row.default);
                        let style = (12., theme.dim_text, Face::Regular);
                        y = self.flow((x, y, w), text, style, 4)? + 6.;
                    }
                    _ => {}
                }
            }
            let (x, w) = DETAIL_SIDE;
            let facts = state
                .modules
                .page_items(page, |k| matches!(k, RowKind::Facts { .. }));
            for (n, fact) in facts.iter().enumerate() {
                let at = (x, 50. + n as f32 * 27., w);
                let style = (Face::Mono, self.tone(fact.tone));
                self.fact_line(at, (&fact.title, &fact.value), style)?;
            }
        }
        // The buttons, bottom up: the page's action rows.
        let (x, w) = DETAIL_SIDE;
        for (n, row) in rows.iter().enumerate() {
            let y = self.footer_top() - 2. - (rows.len() - n) as f32 * 48.;
            let icon = state.module_icon(row.id);
            let label = state.label(row);
            let icon = icon.as_deref().map(|svg| (Some(svg), ""));
            self.icon_button((x, y, w, 40.), n == state.cursor, icon, &label)?;
        }
        Ok(())
    }
    /// The picture of the item's game (or the module's icon), its name,
    /// its line and its tag.
    fn detail_header(&mut self, state: &Settings, header: &Item) -> Result<(), String> {
        let theme = self.theme;
        let (x, w) = DETAIL_LEFT;
        let (pw, ph) = PICTURE;
        let frame = (x, 50., pw, ph);
        self.ui
            .rounded(self.rect(frame), self.ui.px(8.) as f32, theme.badge, None)?;
        let path = header
            .game
            .as_ref()
            .and_then(|g| state.modules.images.get(g));
        if !self.picture(path.map(|p| p.as_path()), frame, true)? {
            let icon = state.modules.module().and_then(|m| m.icon.clone());
            let at = self.rect((x + pw / 2. - 14., 50. + ph / 2. - 14., 28., 28.));
            self.ui
                .module_icon((icon.as_deref(), "gamepad-2"), at, theme.muted)?;
        }
        let (tx, tw) = (x + pw + 12., w - pw - 12.);
        let y = 50. + (ph - if header.badge.is_empty() { 52. } else { 64. }) / 2.;
        let title = (18., theme.text, Align::Left);
        self.text_face((tx, y, tw, 25.), &header.title, title, Face::Semibold)?;
        let line = (12., theme.muted, Align::Left);
        self.text((tx, y + 25., tw, 18.), &header.subtitle, line)?;
        if !header.badge.is_empty() {
            let w = self.width(&header.badge, Face::Semibold, 11.)? + 14.;
            self.module_tag(&header.badge, header.tone, (tx + w, y + 46.))?;
        }
        Ok(())
    }
    /// "408 MHz ▬▬▬▬ 91%": a bar in the item's tone.
    fn detail_bar(&mut self, bar: &Item, (x, y, w): (f32, f32, f32)) -> Result<(), String> {
        let theme = self.theme;
        let style = (Face::Mono, 12., theme.muted);
        self.plain(&bar.title, (x, y, 66., 16.), style)?;
        let fill = match bar.tone {
            Tone::Info => theme.highlight_bg,
            Tone::Plain | Tone::Muted => mix(theme.badge, theme.text, 0.14),
            tone => self.tone(tone),
        };
        self.bar(
            (x + 72., y + 4., w - 122., 8.),
            bar.progress.unwrap_or(0),
            fill,
        )?;
        let ink = if bar.tone == Tone::Bad {
            theme.danger
        } else {
            theme.text
        };
        let value = (12., ink, Align::Right);
        self.text_face((x + w - 40., y, 40., 16.), &bar.value, value, Face::Mono)
    }
    /// A module page's hints when a list item opens a page or has a second
    /// action, whose text the module gave (API 3).
    pub(super) fn module_hints(&self, state: &Settings) -> Option<Vec<(HintButton, String)>> {
        let modules = &state.modules;
        if !matches!(state.page(), Page::Module(..)) || modules.confirm.is_some() || modules.working
        {
            return None;
        }
        let (opens, alt) = state.module_item_hints()?;
        if !opens && alt.is_none() {
            return None;
        }
        let first = if opens {
            "settings.details"
        } else {
            "hint.choose"
        };
        let mut hints = vec![(HintButton::Key('A'), self.tr(first))];
        hints.extend(alt.map(|label| (HintButton::Key('X'), label)));
        hints.push((HintButton::Key('B'), self.tr("hint.back")));
        Some(hints)
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
        let visible = ((self.footer_top() - LIST_TOP) / step).floor().max(1.) as usize;
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
    /// "Revisado pelo ArchR" under a module's name: the package itself
    /// matches the reviewed list.
    fn badge_long(&mut self, x: f32, y: f32) -> Result<(), String> {
        let theme = self.theme;
        let label = self.tr("modules.reviewed_badge");
        let w = self.width(&label, Face::Semibold, 11.)? + 30.;
        self.ui.rounded(
            self.rect((x, y, w, 18.)),
            self.ui.px(8.) as f32,
            crate::appearance::mix(theme.background, theme.success, 0.12),
            Some(crate::appearance::mix(
                theme.background,
                theme.success,
                0.35,
            )),
        )?;
        self.ui.icon(
            Glyph::ShieldCheck,
            self.rect((x + 7., y + 3., 12., 12.)),
            theme.success,
        )?;
        self.text_face(
            (x + 23., y, w - 25., 18.),
            &label,
            (11., theme.success, Align::Left),
            Face::Semibold,
        )
    }
    /// Loja-modulo, for a store offer or an installed module.
    pub(super) fn module_offer(&mut self, state: &Settings) -> Result<(), String> {
        let theme = self.theme;
        let installed = state.page() == Page::ModuleInfo;
        if installed
            && state
                .modules
                .module()
                .is_some_and(|m| m.granted && m.status == Status::Ready)
        {
            return self.module_details(state);
        }
        let mut kept = Vec::new();
        let mut update = None;
        let (title, icon, meta, description, preview, asks, added, reviewed) = if installed {
            let Some(module) = state.modules.module() else {
                return Ok(());
            };
            let meta = (
                module.author.clone(),
                [
                    self.tr(&format!("modules.version|version={}", module.version)),
                    module.license.clone(),
                ],
            );
            let note = match (module.status, module.reviewed, module.source.is_empty()) {
                (Status::Invalid, ..) => module.problem.clone(),
                (Status::Incompatible, ..) => self.tr("modules.incompatible"),
                (_, true, _) => String::new(),
                (_, false, true) => self.tr("modules.local_note"),
                _ => self.tr("modules.not_reviewed_note"),
            };
            (
                module.name.clone(),
                (module.icon.clone(), String::new()),
                (meta.0, joined(&meta.1)),
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
            let version = match &offer.installed {
                Some(old) if *old != offer.version && !offer.added.is_empty() => {
                    update = Some((old.clone(), offer.added.len()));
                    kept = offer.kept.clone();
                    format!("{old} → {}", offer.version)
                }
                _ => self.tr(&format!("modules.version|version={}", offer.version)),
            };
            let meta = (
                offer.author.clone(),
                joined(&[version, size, offer.license.clone()]),
            );
            // The badge already says it is reviewed.
            let note = if offer.reviewed {
                String::new()
            } else {
                self.tr("modules.not_reviewed_note")
            };
            (
                offer.card.title().to_owned(),
                (None, offer.card.icon.clone()),
                meta,
                sentences(&offer.card.description, &note),
                offer.preview.clone(),
                offer.permissions.clone(),
                offer.added.clone(),
                offer.reviewed,
            )
        };
        let (x, w) = LEFT;
        let below = self.module_header(icon, &title, (&meta.0, &meta.1), reviewed)?;
        let below = self.flow(
            (x, below + 4., w),
            &description,
            (12., theme.text_secondary, Face::Regular),
            4,
        )?;
        if let Some((old, count)) = &update {
            // The update asks for more: what happens without it.
            let text = self.tr(&format!("modules.update_asks|count={count}|version={old}"));
            let width = self.rect((0., 0., w - 24., 1.)).width();
            let lines = self.lines_face(&text, 12., width, Face::Regular)?;
            let height = 20. + lines.len() as f32 * 17.;
            self.ui.rounded(
                self.rect((x, below + 8., w, height)),
                self.ui.px(10.) as f32,
                theme.surface,
                Some(theme.border),
            )?;
            self.flow(
                (x + 12., below + 18., w - 24.),
                &text,
                (12., theme.text_secondary, Face::Regular),
                6,
            )?;
            return self.asks(state, &asks, &added, &kept, "modules.asks_more");
        }
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
        self.asks(state, &asks, &[], &[], "modules.asks")
    }
    /// "hoje, 14:05", "ontem", "há 3 dias", or "nunca".
    pub(super) fn when(&self, unix: u64) -> String {
        if unix == 0 {
            return self.tr("modules.never");
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let day = |t: u64| {
            crate::status::local_time(t).map_or(t / 86400, |tm| {
                (t as i64 + tm.tm_gmtoff).max(0) as u64 / 86400
            })
        };
        match day(now).saturating_sub(day(unix)) {
            0 => self.tr(&format!(
                "modules.today|time={}",
                crate::status::local_clock(unix, false)
            )),
            1 => self.tr("modules.yesterday"),
            days => self.tr(&format!("modules.days_ago|count={days}")),
        }
    }
    /// The header of the permission and details pages: icon, name, meta and
    /// the reviewed badge under it; returns the bottom.
    fn module_header(
        &mut self,
        icon: (Option<std::sync::Arc<str>>, String),
        title: &str,
        (author, rest): (&str, &str),
        reviewed: bool,
    ) -> Result<f32, String> {
        let theme = self.theme;
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
        let text_x = x + 60.;
        let title_w = w - 60.;
        self.plain(
            title,
            (text_x, top, title_w, 26.),
            (Face::Semibold, 19., theme.text),
        )?;
        // Version, size and license always show: the author (or source)
        // gives way first, then the rest wraps.
        let meta = self.meta_line(author, rest, title_w)?;
        let below = self.flow(
            (text_x, top + 26., title_w),
            &meta,
            (12., theme.muted, Face::Regular),
            2,
        )?;
        if reviewed {
            self.badge_long(text_x, below + 2.)?;
            return Ok(below + 23.);
        }
        Ok(below + 11.)
    }
    /// "author · rest" in one line of `width` (see `shorten`).
    fn meta_line(&mut self, author: &str, rest: &str, width: f32) -> Result<String, String> {
        let scale = self.ui.frame.scale;
        shorten(author, rest, |text| {
            Ok(self.ui.measure(text, Face::Regular, 12.)? as f32 / scale <= width)
        })
    }
    /// Modulo-detalhes: an installed module the user allowed: its package,
    /// review, use and failures; Open, Remove, Remove with data; and the
    /// permissions it holds.
    fn module_details(&mut self, state: &Settings) -> Result<(), String> {
        let theme = self.theme;
        let Some(module) = state.modules.module() else {
            return Ok(());
        };
        let (x, w) = LEFT;
        let who = if module.source.is_empty() {
            &module.author
        } else {
            &module.source
        };
        let rest = joined(&[module.version.clone(), module.license.clone()]);
        let below = self.module_header(
            (module.icon.clone(), String::new()),
            &module.name,
            (who, &rest),
            module.reviewed,
        )?;
        let digest = module
            .digest
            .strip_prefix("sha256:")
            .unwrap_or(&module.digest);
        let package = if digest.len() == 64 {
            format!("sha256:{}…{}", &digest[..4], &digest[60..])
        } else {
            self.tr("modules.package_local")
        };
        let review = self.tr(match (module.reviewed, module.source.is_empty()) {
            (true, _) => "modules.review_match",
            (false, true) => "modules.review_local",
            (false, false) => "modules.review_none",
        });
        let facts = [
            ("modules.fact_package", package, true),
            ("modules.fact_review", review, false),
            ("modules.fact_used", self.when(module.used), false),
            ("modules.fact_failures", module.failures.to_string(), true),
        ];
        for (n, (key, value, mono)) in facts.into_iter().enumerate() {
            let face = if mono { Face::Mono } else { Face::Regular };
            let at = (x, below + 8. + n as f32 * 27., w);
            self.fact_line(at, (&self.tr(key), &value), (face, theme.text_secondary))?;
        }
        let buttons = state.details_buttons();
        let gap = 8.;
        let each = (w - gap * (buttons.len() as f32 - 1.)) / buttons.len() as f32;
        for (n, key) in buttons.iter().enumerate() {
            let rect = (x + n as f32 * (each + gap), BUTTON, each, 36.);
            let focus = n == state.modules.button.min(buttons.len() - 1);
            let (fill, edge, ink) = if focus {
                (theme.highlight_bg, None, theme.highlight_text)
            } else {
                (theme.surface, Some(theme.border), theme.text)
            };
            self.ui
                .rounded(self.rect(rect), self.ui.px(10.) as f32, fill, edge)?;
            self.text_face(
                rect,
                &self.tr(key),
                (14., ink, Align::Center),
                Face::Regular,
            )?;
        }
        let asks = module.permissions.clone();
        self.asks(state, &asks, &[], &[], "modules.you_allowed")
    }
    /// One permission card's lines: the label, then the detail, wrapped to
    /// the card; and its height.
    fn ask_lines(
        &mut self,
        permission: &Permission,
        width: f32,
        new: bool,
    ) -> Result<(Vec<String>, Vec<String>, f32), String> {
        let px = self.rect((0., 0., width, 1.)).width();
        let tag = if new {
            self.ui
                .measure(&self.tr("modules.new_tag"), Face::Semibold, 10.)?
                + self.ui.px(16.) as u32
        } else {
            0
        };
        let label = self.tr(permission.key);
        let mut label_lines =
            self.lines_face(&label, 13., px.saturating_sub(tag).max(1), Face::Regular)?;
        if label_lines.is_empty() {
            label_lines.push(String::new());
        }
        let detail = if permission.detail.is_empty() {
            Vec::new()
        } else {
            let text = self.tr(&permission.detail);
            self.lines_face(&text, 11., px, Face::Regular)?
        };
        let height = 14.
            + label_lines.len() as f32 * self.pitch(13., 17.)
            + detail.len() as f32 * self.pitch(11., 14.);
        Ok((label_lines, detail, height.max(30.)))
    }
    /// The permission column: amber for the sensitive ones, then the
    /// button. A list longer than the column scrolls with ↑↓; an update
    /// shows only what it adds, tagged, and what stays allowed under it.
    fn asks(
        &mut self,
        state: &Settings,
        asks: &[Permission],
        added: &[Permission],
        kept: &[Permission],
        heading: &str,
    ) -> Result<(), String> {
        let theme = self.theme;
        let (x, w) = ASKS;
        let update = !added.is_empty();
        let list = if update { added } else { asks };
        let button = state.offer_action();
        let bottom = if button.is_some() {
            BUTTON - 8.
        } else {
            self.footer_top() - 8.
        };
        let mut cards = Vec::new();
        for permission in list {
            cards.push(self.ask_lines(permission, w - 36., update)?);
        }
        let top = 72.;
        // The last first card that still fills the column to its end.
        let mut last = cards.len().saturating_sub(1);
        let mut room = bottom - top;
        for (n, card) in cards.iter().enumerate().rev() {
            if card.2 > room {
                break;
            }
            room -= card.2 + 6.;
            last = n;
        }
        if cards.iter().map(|c| c.2 + 6.).sum::<f32>() - 6. <= bottom - top {
            last = 0;
        }
        state.modules.asks_last.set(last);
        let first = state.modules.asks_scroll.min(last);
        let mut shown = 0;
        let mut y = top;
        for (permission, (label, detail, height)) in list.iter().zip(&cards).skip(first) {
            if y + height > bottom + 0.5 {
                break;
            }
            shown += 1;
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
                self.rect((x, y, w, *height)),
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
            let mut line_y = y + 7.;
            for (n, text) in label.iter().enumerate() {
                self.plain(
                    text,
                    (x + 28., line_y, w - 36., self.pitch(13., 17.)),
                    (Face::Regular, 13., theme.text),
                )?;
                if update && n == 0 {
                    let after = x + 28. + self.width(text, Face::Regular, 13.)? + 6.;
                    self.new_tag(after, line_y + 1.)?;
                }
                line_y += self.pitch(13., 17.);
            }
            let pitch = self.pitch(11., 14.);
            for text in detail {
                self.plain(
                    text,
                    (x + 28., line_y, w - 36., pitch),
                    (Face::Regular, 11., theme.muted),
                )?;
                line_y += pitch;
            }
            y += height + 6.;
        }
        let total = list.len();
        let overflow = shown < total;
        let range = if overflow {
            self.tr(&format!(
                "modules.range|first={}|last={}|total={total}",
                first + 1,
                first + shown
            ))
        } else {
            String::new()
        };
        let range_w = self.width(&range, Face::Regular, 11.)?;
        self.text(
            (x, 50., w - range_w - 8., 18.),
            &self.tr(heading).to_uppercase(),
            (11., theme.muted, Align::Left),
        )?;
        if overflow {
            self.text(
                (x + w - range_w - 2., 50., range_w + 2., 18.),
                &range,
                (11., theme.muted, Align::Right),
            )?;
            // The bar beside the cards: where the shown ones sit.
            let (track_y, track_h) = (top, bottom - top);
            self.ui.rounded(
                self.rect((x + w + 6., track_y, 3., track_h)),
                self.ui.px(1.5) as f32,
                theme.border,
                None,
            )?;
            let span = track_h * shown as f32 / total as f32;
            let at = track_y + (track_h - span) * first as f32 / last.max(1) as f32;
            self.ui.rounded(
                self.rect((x + w + 6., at, 3., span)),
                self.ui.px(1.5) as f32,
                theme.muted,
                None,
            )?;
        }
        if update && !kept.is_empty() && !overflow {
            let names: Vec<String> = kept
                .iter()
                .map(|p| {
                    let name = self.tr(p.key);
                    let mut chars = name.chars();
                    chars
                        .next()
                        .map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
                })
                .collect();
            let text = self.tr(&format!("modules.kept|list={}", names.join(", ")));
            self.flow((x, y + 2., w), &text, (12., theme.muted, Face::Regular), 5)?;
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
    /// Text width in layout pixels.
    pub(super) fn width(&mut self, text: &str, face: Face, size: f32) -> Result<f32, String> {
        Ok(self.ui.measure(text, face, size)? as f32 / self.ui.frame.scale)
    }
    /// The amber "NOVO" tag of a permission an update adds.
    fn new_tag(&mut self, x: f32, y: f32) -> Result<(), String> {
        let theme = self.theme;
        let label = self.tr("modules.new_tag");
        let w = self.width(&label, Face::Semibold, 10.)? + 10.;
        self.ui.rounded(
            self.rect((x, y, w, 15.)),
            self.ui.px(6.) as f32,
            theme.warning,
            None,
        )?;
        self.text_face(
            (x, y, w, 15.),
            &label,
            (10., theme.background, Align::Center),
            Face::Semibold,
        )
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
            Page::ModuleOffer
                if modules.offer.as_ref().is_some_and(|o| {
                    o.installed.as_ref().is_some_and(|v| *v != o.version) && !o.added.is_empty()
                }) =>
            {
                self.tr("modules.keeps_previous")
            }
            Page::ModuleInfo
                if modules
                    .module()
                    .is_some_and(|m| m.granted && m.status == Status::Ready) =>
            {
                self.tr(if modules.module().is_some_and(|m| m.reviewed) {
                    "modules.badge_note"
                } else {
                    "modules.isolated"
                })
            }
            Page::ModuleOffer | Page::ModuleInfo => self.tr("modules.isolated"),
            Page::Modules if state.cursor == modules.view.modules.len() => {
                self.tr("modules.never_in_game")
            }
            page @ Page::Module(..) => modules.declared(page)?.hint.clone()?,
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
        if section != SECTION && section != SECTION_NEXT {
            return None;
        }
        let id = state.visible_rows().get(n)?.id;
        Some(state.section_text(id))
    }
}

/// A description and a note as one paragraph.
fn sentences(first: &str, second: &str) -> String {
    match first.trim() {
        "" => second.to_owned(),
        text if second.is_empty() => text.to_owned(),
        text if text.ends_with(['.', '!', '?']) => format!("{text} {second}"),
        text => format!("{text}. {second}"),
    }
}

/// "author · rest" as `fits` takes it: the author shortened with "…" as far
/// as needed, dropped if even one letter does not fit; `rest` is never cut.
fn shorten(
    author: &str,
    rest: &str,
    mut fits: impl FnMut(&str) -> Result<bool, String>,
) -> Result<String, String> {
    let line = |author: &str| joined(&[author.to_owned(), rest.to_owned()]);
    if fits(&line(author))? {
        return Ok(line(author));
    }
    let chars: Vec<char> = author.chars().collect();
    for keep in (1..chars.len()).rev() {
        let short = format!("{}…", chars[..keep].iter().collect::<String>().trim_end());
        if fits(&line(&short))? {
            return Ok(line(&short));
        }
    }
    Ok(line(""))
}

/// The non-empty parts, joined by " · ".
fn joined(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod meta_tests {
    use super::shorten;

    #[test]
    fn the_author_gives_way_before_version_size_and_license() {
        let rest = "versão 1.0.0 · 23 KB · MIT";
        let width = |max: usize| move |t: &str| Ok(t.chars().count() <= max);
        let author = "ArchR <contato@archr.dev>";
        let full = format!("{author} · {rest}");
        assert_eq!(shorten(author, rest, width(80)).unwrap(), full);
        let short = shorten(author, rest, width(40)).unwrap();
        assert!(short.ends_with(&format!("… · {rest}")), "{short}");
        assert!(
            short.chars().count() <= 40 && short.starts_with("Arch"),
            "{short}"
        );
        assert_eq!(
            shorten(author, rest, width(10)).unwrap(),
            rest,
            "no room: only the rest"
        );
        assert_eq!(shorten("", rest, width(10)).unwrap(), rest);
    }
}
