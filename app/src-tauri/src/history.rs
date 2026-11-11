//! Undo/redo for the open scan. Edits never delete page files (each edit writes a new one), so a
//! step is just the page list from before it.
use scan_core::session::Page;

const LIMIT: usize = 100;

#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Page>>,
    redo: Vec<Vec<Page>>,
}

impl History {
    /// Remember `before` as an undo step; a new change drops anything that could be redone.
    pub fn record(&mut self, before: Vec<Page>) {
        self.undo.push(before);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, now: Vec<Page>) -> Option<Vec<Page>> {
        let back = self.undo.pop()?;
        self.redo.push(now);
        Some(back)
    }

    pub fn redo(&mut self, now: Vec<Page>) -> Option<Vec<Page>> {
        let forward = self.redo.pop()?;
        self.undo.push(now);
        Some(forward)
    }

    pub fn clear(&mut self) {
        *self = History::default();
    }

    pub fn can(&self) -> (bool, bool) {
        (!self.undo.is_empty(), !self.redo.is_empty())
    }
}
