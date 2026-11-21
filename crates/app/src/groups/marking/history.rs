//! Undo and redo: every change is kept as the state before it.

/// How many changes undo reaches back.
pub const LIMIT: usize = 300;

/// The states to go back and forward to.
#[derive(Clone, Debug)]
pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<T> History<T> {
    /// A change was made from `before`: it can be undone, and what was
    /// undone before can no longer be redone.
    pub fn record(&mut self, before: T) {
        self.undo.push(before);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The state to go back to from `current`, which redo then returns to.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let before = self.undo.pop()?;
        self.redo.push(current);
        Some(before)
    }

    /// The state undone last, going forward from `current`.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let after = self.redo.pop()?;
        self.undo.push(current);
        Some(after)
    }

    /// Takes back the last change for good, as if it had never been made
    /// (the first tap of a double tap).
    pub fn forget_last(&mut self) -> Option<T> {
        self.undo.pop()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// How many changes can be undone.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
}
