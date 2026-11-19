//! The Modules row (Modulos-inicio, Modulos-grade and their Ellegance
//! boards): a module's picture beside the list, and its card in a grid.
use super::*;
use crate::modules::Module;

/// The Modules row's own mark: a module, a square with a pin on each side.
pub(super) const MODULES_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v3"/><path d="M12 18v3"/><rect width="12" height="12" x="6" y="6" rx="2"/><path d="M3 12h3"/><path d="M18 12h3"/></svg>"#;

const fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: (hex >> 16) as u8,
        g: (hex >> 8) as u8,
        b: hex as u8,
        a: 255,
    }
}
/// A card's colours, top to bottom: the same for a module on every frame
/// and every start, by its id.
const CARDS: [(Rgba, Rgba); 6] = [
    (rgb(0x2B6BD6), rgb(0x1A3F86)),
    (rgb(0x2E8C5A), rgb(0x18502F)),
    (rgb(0x8A4FD0), rgb(0x4B2685)),
    (rgb(0xC7832A), rgb(0x7A4A14)),
    (rgb(0xC9484A), rgb(0x7A2427)),
    (rgb(0x2A9AA6), rgb(0x155A62)),
];
pub(super) fn card_colors(id: &str) -> (Rgba, Rgba) {
    // FNV-1a: stable across builds, unlike the standard hasher.
    let hash = id.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    CARDS[hash as usize % CARDS.len()]
}

/// The author without the address a manifest may add ("Name <mail>").
fn author(module: &Module) -> &str {
    match module.author.split_once('<') {
        Some((name, _)) if !name.trim().is_empty() => name.trim(),
        _ => module.author.trim(),
    }
}
/// "0.3.0 · Local": the version and where it came from, as Settings says it.
fn origin(module: &Module, catalog: &I18n) -> String {
    let key = match (module.reviewed, module.source.is_empty()) {
        (true, _) => "modules.reviewed",
        (false, true) => "modules.local",
        (false, false) => "modules.not_reviewed",
    };
    format!("{} · {}", module.version, catalog.text(key))
}

impl Ui<'_, '_> {
    /// Beside the Modules list, in the cover's place: the module's icon on
    /// the accent in a panel, its version, origin and author, then what it
    /// does, wrapped down to the footer.
    pub(super) fn module_art(
        &mut self,
        game: Option<&archr_index::Game>,
        theme: &Theme,
        layout: &Layout,
    ) -> Result<(), String> {
        // A game selected next fades in again.
        self.cover_key = None;
        self.cover_visible = false;
        let modules = self.modules.clone();
        let (Some(module), Some(art)) = (game.and_then(|g| modules.listed_module(g)), layout.art)
        else {
            return Ok(());
        };
        // The panel as wide as the cover's, a little less tall than wide.
        let full = self.area(art.rect);
        let tall = ((full.width() as f32 * 0.935).round() as u32).min(full.height());
        let rect = Rect::new(full.x(), full.y(), full.width(), tall.max(1));
        match layout.panel {
            Some(mut panel) => {
                panel.rect.h = art.rect.h * tall as f32 / full.height().max(1) as f32;
                self.layout_panel("art", &panel, theme)?;
            }
            None => {
                let radius = self.frame.size(art.radius);
                self.panel(rect, radius, theme.surface, theme.border)?;
            }
        }
        let side = (rect.width().min(rect.height()) as f32 * 0.48).round() as u32;
        let square = Rect::new(
            rect.center().x() - side as i32 / 2,
            rect.center().y() - side as i32 / 2,
            side,
            side,
        );
        self.rounded(square, side as f32 / 4., theme.highlight_bg, None)?;
        let glyph = (side as f32 * 0.46).round() as u32;
        let icon = Rect::new(
            square.center().x() - glyph as i32 / 2,
            square.center().y() - glyph as i32 / 2,
            glyph,
            glyph,
        );
        self.module_icon((module.icon.as_deref(), ""), icon, theme.highlight_text)?;
        // Version and origin on the left, the author on the right; one
        // line, or two when they do not fit side by side.
        let size = 12.;
        let line = (f32::from(self.frame.font_size(size)) * 1.45).round() as i32;
        let mut y = rect.bottom() + self.px(10.);
        let width = rect.width();
        let left = origin(module, &self.catalog);
        let lw = self.measure(&left, Face::Mono, size)?;
        let who = author(module);
        let aw = self.measure(who, Face::Regular, size)?;
        let ink = |face| (face, size, theme.muted);
        if lw + aw + self.px(12.) as u32 <= width {
            self.label(
                &left,
                ink(Face::Mono),
                Rect::new(rect.x(), y, lw.max(1), line as u32),
            )?;
            self.label_aligned(
                who,
                ink(Face::Regular),
                Rect::new(rect.right() - aw as i32, y, aw.max(1), line as u32),
                Align::Right,
            )?;
            y += line;
        } else {
            for (text, face) in [(left.as_str(), Face::Mono), (who, Face::Regular)] {
                for text in self.wrap(text, face, size, width)? {
                    self.label(&text, ink(face), Rect::new(rect.x(), y, width, line as u32))?;
                    y += line;
                }
            }
        }
        y += self.px(8.);
        // The description to the footer, smaller when it must; never cut.
        let bottom = self.frame.height as i32
            - self.frame.size(theme.footer_height).round() as i32
            - self.px(8.);
        let floor = self.frame.min_size();
        let mut size = 12_f32;
        let (lines, step) = loop {
            let lines = self.wrap(&module.description, Face::Regular, size, width)?;
            let step = (f32::from(self.frame.font_size(size)) * 1.45).round() as i32;
            if y + step * lines.len() as i32 <= bottom || size <= floor {
                break (lines, step);
            }
            size = (size - 1.).max(floor);
        };
        for text in lines {
            if y + step > bottom {
                break;
            }
            let at = Rect::new(rect.x(), y, width, step as u32);
            self.label(&text, (Face::Regular, size, theme.text_secondary), at)?;
            y += step;
        }
        Ok(())
    }

    /// A module's card in a grid: its colours, its icon in white.
    pub(super) fn module_card(
        &mut self,
        rect: Rect,
        radius: f32,
        module: &Module,
        theme: &Theme,
    ) -> Result<(), String> {
        let (from, to) = card_colors(&module.id);
        let gradient = crate::theme::Gradient {
            from,
            to,
            angle: 70.,
        };
        self.geometry
            .shaded(&mut self.canvas, rect, radius, &gradient, None)?;
        let side = (rect.height() as f32 * 0.36).round() as u32;
        let icon = Rect::new(
            rect.center().x() - side as i32 / 2,
            rect.center().y() - side as i32 / 2,
            side,
            side,
        );
        self.module_icon((module.icon.as_deref(), ""), icon, theme.highlight_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_author_leaves_the_address_out() {
        let named = |author: &str| Module {
            author: author.into(),
            ..Module::default()
        };
        assert_eq!(author(&named("ArchR <contato@archr.dev>")), "ArchR");
        assert_eq!(author(&named("Ana")), "Ana");
        assert_eq!(author(&named("<ana@x.dev>")), "<ana@x.dev>");
    }

    #[test]
    fn a_module_keeps_its_card_colours() {
        assert_eq!(card_colors("perf-limits"), card_colors("perf-limits"));
        let all: std::collections::HashSet<_> = (0..40)
            .map(|n| card_colors(&format!("module-{n}")).0.r)
            .collect();
        assert!(all.len() > 3, "the ids spread over the colours");
    }
}
