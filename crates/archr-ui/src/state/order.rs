//! User ordering of the systems list: the merge rules and the move mode.
use super::{App, Button, DataSource, Outcome};

/// An uncommitted reorder: the working rows and where the item started.
pub(super) struct Moving {
    rows: Vec<String>,
    from: usize,
}

impl App {
    /// Rows in their default order: collections first, then systems with games.
    pub fn default_rows(data: &dyn DataSource) -> Vec<String> {
        ["@favorites", "@recent"]
            .into_iter()
            .filter(|name| data.collection_len(name) > 0)
            .map(str::to_owned)
            .chain(
                data.systems()
                    .iter()
                    .filter(|system| system.game_count > 0)
                    .map(|system| system.name.clone()),
            )
            .collect()
    }

    /// Saved order applied to the default rows. Unknown saved ids are ignored,
    /// and ids missing from the saved list keep their default order, last.
    pub fn system_rows(data: &dyn DataSource) -> Vec<String> {
        let mut rows = Self::default_rows(data);
        let saved = data.system_order();
        rows.sort_by_key(|id| saved.iter().position(|s| s == id).unwrap_or(usize::MAX));
        rows
    }

    /// Shared row ordering for navigation and rendering, including a move in progress.
    pub fn rows(&self, data: &dyn DataSource) -> Vec<String> {
        match &self.moving {
            Some(moving) => moving.rows.clone(),
            None => Self::system_rows(data),
        }
    }

    /// The row being moved and its 1-based position, for the footer.
    pub fn moving(&self) -> Option<(&str, usize, usize)> {
        let moving = self.moving.as_ref()?;
        let name = moving.rows.get(self.cursor)?;
        Some((name, self.cursor + 1, moving.rows.len()))
    }

    pub(super) fn begin_move(&mut self, data: &dyn DataSource) {
        let rows = Self::system_rows(data);
        if self.cursor < rows.len() {
            self.moving = Some(Moving {
                rows,
                from: self.cursor,
            });
        }
    }

    pub(super) fn handle_move(&mut self, b: Button) -> Outcome {
        let Some(moving) = &mut self.moving else {
            return Outcome::None;
        };
        let last = moving.rows.len().saturating_sub(1);
        // A carousel moves the lifted system with ← and → as well.
        let across = self.systems_nav.axis == crate::layout::Axis::Horizontal;
        let to = match b {
            Button::Up => self.cursor.saturating_sub(1),
            Button::Down => (self.cursor + 1).min(last),
            Button::L if across => self.cursor.saturating_sub(1),
            Button::R if across => (self.cursor + 1).min(last),
            Button::A => {
                let rows = self.moving.take().map(|moving| moving.rows);
                return Outcome::SetOrder(rows.unwrap_or_default());
            }
            Button::B => {
                self.cursor = moving.from;
                self.moving = None;
                return Outcome::None;
            }
            _ => return Outcome::None,
        };
        moving.rows.swap(self.cursor, to);
        self.cursor = to;
        Outcome::None
    }

    /// Keeps the cursor on the same row while it returns to its default place.
    pub(super) fn restore_order(&mut self, data: &dyn DataSource) -> Outcome {
        let current = Self::system_rows(data).get(self.cursor).cloned();
        let default = Self::default_rows(data);
        self.cursor = current
            .and_then(|name| default.iter().position(|row| *row == name))
            .unwrap_or(0);
        Outcome::SetOrder(Vec::new())
    }
}
