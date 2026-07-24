//! Snapshot undo/redo stack for scene authoring (editor).

use crate::scene::Scene;

const DEFAULT_MAX_DEPTH: usize = 50;

/// Snapshot stack of [`Scene`] values. Push the pre-mutation scene, then mutate.
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    undo: Vec<Scene>,
    redo: Vec<Scene>,
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_depth: max_depth.max(1),
        }
    }

    pub fn with_default_depth() -> Self {
        Self::new(DEFAULT_MAX_DEPTH)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Record `before` (state prior to a mutation). Clears the redo stack.
    pub fn push(&mut self, before: &Scene) {
        self.push_owned(before.clone());
    }

    pub fn push_owned(&mut self, before: Scene) {
        self.undo.push(before);
        if self.undo.len() > self.max_depth {
            let overflow = self.undo.len() - self.max_depth;
            self.undo.drain(0..overflow);
        }
        self.redo.clear();
    }

    /// Restore previous snapshot into `current`. Returns false if empty.
    pub fn undo(&mut self, current: &mut Scene) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(current, prev));
        true
    }

    /// Re-apply a redone snapshot into `current`. Returns false if empty.
    pub fn redo(&mut self, current: &mut Scene) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(current, next));
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutate::{add_entity, MutateOpts};
    use crate::scene::Scene;

    fn scene_with(name: &str) -> Scene {
        let mut s = Scene::new("test");
        add_entity(&mut s, name, &MutateOpts::default()).unwrap();
        s
    }

    #[test]
    fn push_undo_redo() {
        let mut stack = UndoStack::new(50);
        let mut scene = scene_with("a");
        stack.push(&scene);
        add_entity(&mut scene, "b", &MutateOpts::default()).unwrap();
        assert_eq!(scene.entities.len(), 2);

        assert!(stack.undo(&mut scene));
        assert_eq!(scene.entities.len(), 1);
        assert_eq!(scene.entities[0].name, "a");
        assert!(stack.can_redo());

        assert!(stack.redo(&mut scene));
        assert_eq!(scene.entities.len(), 2);
        assert!(!stack.can_redo());
    }

    #[test]
    fn new_mutation_clears_redo() {
        let mut stack = UndoStack::new(50);
        let mut scene = scene_with("a");
        stack.push(&scene);
        add_entity(&mut scene, "b", &MutateOpts::default()).unwrap();
        stack.undo(&mut scene);
        assert!(stack.can_redo());

        stack.push(&scene);
        add_entity(&mut scene, "c", &MutateOpts::default()).unwrap();
        assert!(!stack.can_redo());
        assert_eq!(stack.redo_len(), 0);
    }

    #[test]
    fn caps_depth() {
        let mut stack = UndoStack::new(3);
        let mut scene = Scene::new("t");
        for i in 0..5 {
            stack.push(&scene);
            add_entity(&mut scene, &format!("e{i}"), &MutateOpts::default()).unwrap();
        }
        assert_eq!(stack.undo_len(), 3);
        // Oldest snapshots dropped; can undo 3 times only.
        assert!(stack.undo(&mut scene));
        assert!(stack.undo(&mut scene));
        assert!(stack.undo(&mut scene));
        assert!(!stack.undo(&mut scene));
    }
}
