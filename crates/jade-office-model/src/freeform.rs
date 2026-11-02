//! Insert ▸ Shapes ▸ Freeform: a shape drawn a click at a time. Each click
//! adds a corner; a click back on the first closes the shape, filled; a
//! double-click or Enter ends it open, as a line.

/// How near the first corner a click closes the shape, in sheet px.
pub const CLOSE_DISTANCE: f64 = 6.0;

/// The corners clicked so far, in sheet px.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreeformDraft {
    pub points: Vec<(f64, f64)>,
}

/// What a click did to the draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeformStep {
    /// A corner more; drawing goes on.
    Continue,
    /// Back on the first corner: the shape is closed.
    Close,
}

impl FreeformDraft {
    /// A click at `point` (sheet px). The second press of a double-click,
    /// on the corner just added, adds nothing.
    pub fn add(&mut self, point: (f64, f64)) -> FreeformStep {
        let near =
            |a: (f64, f64), b: (f64, f64), distance: f64| (a.0 - b.0).hypot(a.1 - b.1) <= distance;
        if self.points.len() >= 3 && near(self.points[0], point, CLOSE_DISTANCE) {
            return FreeformStep::Close;
        }
        if self
            .points
            .last()
            .is_some_and(|last| near(*last, point, 1.0))
        {
            return FreeformStep::Continue;
        }
        self.points.push(point);
        FreeformStep::Continue
    }

    /// Whether the corners make a shape: two at least.
    pub fn can_finish(&self) -> bool {
        self.points.len() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_add_corners_and_the_first_closes() {
        let mut draft = FreeformDraft::default();
        assert_eq!(draft.add((10.0, 10.0)), FreeformStep::Continue);
        assert!(!draft.can_finish());
        assert_eq!(draft.add((50.0, 10.0)), FreeformStep::Continue);
        // A double-click's second press adds no corner.
        assert_eq!(draft.add((50.5, 10.0)), FreeformStep::Continue);
        assert_eq!(draft.points.len(), 2);
        assert!(draft.can_finish());
        // Near the first with only two corners is a corner, not a close.
        assert_eq!(draft.add((30.0, 40.0)), FreeformStep::Continue);
        assert_eq!(draft.add((12.0, 13.0)), FreeformStep::Close);
        assert_eq!(draft.points.len(), 3);
    }
}
