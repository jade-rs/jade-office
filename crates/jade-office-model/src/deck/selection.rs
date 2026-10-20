//! Several shapes at once, and copies of them: what PowerPoint does to a
//! selection — moved, deleted, duplicated, each as one step of Undo — and
//! to shapes copied and pasted, onto another slide where they were or onto
//! their own slide offset.

use super::{Deck, DragMode, ShapeDrag, ShapeRect};
use crate::Backend;

impl<B: Backend> Deck<B> {
    /// Pastes shapes `ids` of slide `from` onto slide `to`, as one step of
    /// Undo: onto another slide where they were, their pictures, charts and
    /// diagrams with them; onto their own slide offset, as Duplicate puts
    /// them. Those that cannot be copied are left out. Returns the copies'
    /// ids on `to`.
    pub fn paste_shapes(
        &mut self,
        from: usize,
        ids: &[u32],
        to: usize,
    ) -> Result<Vec<u32>, String> {
        let ids: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|&id| {
                self.shape(from, id)
                    .is_some_and(|shape| shape.can_duplicate)
            })
            .collect();
        if ids.is_empty() || to >= self.slide_count() {
            return Ok(Vec::new());
        }
        let copies = self
            .session
            .copy_shapes(from, &ids, to)
            .map_err(|error| error.to_string())?;
        self.reload()?;
        Ok(copies)
    }

    /// Duplicate (⌘D) of a selection: copies offset on its slide, selected
    /// in its place.
    pub fn duplicate_shapes(&mut self, slide: usize, ids: &[u32]) -> Result<Vec<u32>, String> {
        self.paste_shapes(slide, ids, slide)
    }

    /// Moves the shapes of a selection that may move by `(dx, dy)` slide px,
    /// as one step of Undo.
    pub fn move_shapes(
        &mut self,
        slide: usize,
        ids: &[u32],
        dx: f64,
        dy: f64,
    ) -> Result<bool, String> {
        let changes: Vec<_> = ids
            .iter()
            .filter_map(|&id| {
                let shape = self.shape(slide, id).filter(|shape| shape.can_move)?;
                let transform = shape.transform?;
                let moved = ShapeRect::of(&transform)
                    .dragged(DragMode::Move, dx, dy)
                    .apply_to(transform);
                (moved != transform).then_some((id, moved))
            })
            .collect();
        if changes.is_empty() {
            return Ok(false);
        }
        let result = self.session.set_shape_transforms(slide, &changes);
        self.after(result)
    }

    /// Deletes the shapes of a selection that may be deleted, as one step of
    /// Undo.
    pub fn delete_shapes(&mut self, slide: usize, ids: &[u32]) -> Result<bool, String> {
        let ids: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|&id| self.shape(slide, id).is_some_and(|shape| shape.can_delete))
            .collect();
        if ids.is_empty() {
            return Ok(false);
        }
        let result = self.session.delete_shapes(slide, &ids);
        self.after(result)
    }

    /// Writes a finished drag on a shape of `selection`: a move takes every
    /// shape of it along; a corner resizes the shape alone.
    pub fn drag_shapes(
        &mut self,
        slide: usize,
        drag: &ShapeDrag,
        selection: &[u32],
    ) -> Result<bool, String> {
        if drag.mode != DragMode::Move || selection.len() <= 1 || !selection.contains(&drag.id) {
            return self.drag_shape(slide, drag);
        }
        if !drag.moved() {
            return Ok(false);
        }
        self.move_shapes(slide, selection, drag.delta.0, drag.delta.1)
    }

    /// Every shape on the slide that may be selected: what ⌘A selects.
    pub fn selectable_shapes(&mut self, slide: usize) -> Vec<u32> {
        self.selectable(slide)
    }
}
