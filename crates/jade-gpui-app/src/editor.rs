//! What the workspace needs from a view in one of its tabs.
//!
//! The document, workbook and deck views share almost nothing inside, and
//! they stay separate types. What they share is the frame round them: a
//! status line saying where you are, and a zoom the status bar can step and
//! show. [`Editor`] is that surface; [`EditorHandle`] is the same thing on an
//! `Entity`, with the type erased, so the workspace can keep one list of tabs
//! whatever is in them.

use gpui::{AnyView, App, Context, Entity, FocusHandle, Focusable, Render, SharedString};

/// The zoom range every editor accepts, as factors (1.0 is 100%).
pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 4.0;

/// The stops ⌘= and ⌘- step through, Office's own set.
const ZOOM_STOPS: &[f32] = &[
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];

/// The next stop above (`up`) or below `current`. A zoom between stops — a
/// fitted one — moves to the nearest stop in that direction rather than
/// skipping one.
pub fn zoom_step(current: f32, up: bool) -> f32 {
    const EPSILON: f32 = 0.005;
    if up {
        ZOOM_STOPS
            .iter()
            .copied()
            .find(|stop| *stop > current + EPSILON)
            .unwrap_or(ZOOM_MAX)
    } else {
        ZOOM_STOPS
            .iter()
            .rev()
            .copied()
            .find(|stop| *stop < current - EPSILON)
            .unwrap_or(ZOOM_MIN)
    }
}

pub fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    }
}

pub trait Editor: Render + Focusable {
    /// Where the reader is, for the left of the status bar: the page, the
    /// sheet, the slide.
    fn status_items(&self) -> Vec<SharedString>;

    /// The zoom on screen now, as a factor.
    fn zoom(&self) -> f32;

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>);

    /// Whether there are edits not yet saved, which the tab shows as a dot.
    fn is_dirty(&self) -> bool {
        false
    }

    /// Zooms so the content fits the space it has. A view with nothing to fit
    /// goes back to 100%.
    fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        self.set_zoom(1.0, cx);
    }
}

/// An [`Editor`] entity with its type erased.
pub trait EditorHandle {
    fn view(&self) -> AnyView;
    fn focus_handle(&self, cx: &App) -> FocusHandle;
    fn status_items(&self, cx: &App) -> Vec<SharedString>;
    fn zoom(&self, cx: &App) -> f32;
    fn is_dirty(&self, cx: &App) -> bool;
    fn set_zoom(&self, zoom: f32, cx: &mut App);
    fn zoom_to_fit(&self, cx: &mut App);
}

impl<T: Editor> EditorHandle for Entity<T> {
    fn view(&self) -> AnyView {
        self.clone().into()
    }

    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }

    fn status_items(&self, cx: &App) -> Vec<SharedString> {
        self.read(cx).status_items()
    }

    fn zoom(&self, cx: &App) -> f32 {
        self.read(cx).zoom()
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.read(cx).is_dirty()
    }

    fn set_zoom(&self, zoom: f32, cx: &mut App) {
        self.update(cx, |editor, cx| editor.set_zoom(clamp_zoom(zoom), cx));
    }

    fn zoom_to_fit(&self, cx: &mut App) {
        self.update(cx, |editor, cx| editor.zoom_to_fit(cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_through_the_stops_and_stops_at_the_ends() {
        assert_eq!(zoom_step(1.0, true), 1.1);
        assert_eq!(zoom_step(1.0, false), 0.9);
        assert_eq!(zoom_step(4.0, true), ZOOM_MAX);
        assert_eq!(zoom_step(0.25, false), ZOOM_MIN);
    }

    #[test]
    fn a_fitted_zoom_moves_to_the_nearest_stop() {
        assert_eq!(zoom_step(0.83, true), 0.9);
        assert_eq!(zoom_step(0.83, false), 0.8);
    }

    #[test]
    fn zoom_is_clamped_and_never_nan() {
        assert_eq!(clamp_zoom(10.0), ZOOM_MAX);
        assert_eq!(clamp_zoom(0.0), ZOOM_MIN);
        assert_eq!(clamp_zoom(f32::NAN), 1.0);
    }
}
