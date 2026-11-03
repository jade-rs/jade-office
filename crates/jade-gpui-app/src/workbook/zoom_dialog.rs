use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, point, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::zoom_dialog::{ZoomChoice, ZoomDialog, ZoomKey};
type View = WorkbookView;
include!("zoom_dialog_ui.rs");
impl WorkbookView {
    fn zoom_dialog_editor(&self) -> Option<&ZoomDialog> {
        self.zoom_dialog.as_ref()
    }
    /// View ▸ Zoom…, on the zoom the sheet is at.
    pub(super) fn open_zoom_dialog(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        self.zoom_dialog = Some(ZoomDialog::new(self.zoom));
        cx.notify();
    }
    /// The sheet shown at `zoom` (Excel's 10–400%), the same cell kept at
    /// the top left, and saved with the sheet.
    pub(super) fn zoom_sheet(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let zoom = zoom.clamp(0.1, 4.0);
        let ratio = zoom / self.zoom.max(0.01);
        let offset = self.scroll.offset();
        self.scroll
            .set_offset(point(offset.x * ratio, offset.y * ratio));
        self.zoom = zoom;
        self.book.set_sheet_zoom(zoom);
        cx.notify();
    }
    /// View ▸ Zoom to Selection: the selection fills the window, scrolled to
    /// its top left.
    pub(super) fn zoom_to_selection(&mut self, cx: &mut Context<Self>) {
        let range = self.range();
        let size = (f64::from(self.viewport.0), f64::from(self.viewport.1));
        let zoom = self.book.zoom_to_selection(range, size);
        self.zoom_sheet(zoom, cx);
        let (x, y) = self.book.scroll_to_corner(range.start);
        let zoom = f64::from(self.zoom);
        self.scroll
            .set_offset(point(px(-(x * zoom) as f32), px(-(y * zoom) as f32)));
        cx.notify();
    }
    pub(super) fn zoom_dialog_action(&mut self, action: ZoomAction, cx: &mut Context<Self>) {
        match action {
            ZoomAction::Take => {
                let Some(dialog) = self.zoom_dialog.as_mut() else {
                    return;
                };
                if dialog.choice() == ZoomChoice::FitSelection {
                    self.zoom_dialog = None;
                    return self.zoom_to_selection(cx);
                }
                // Excel's 10–400% or the dialog stays open, saying so.
                let Some(zoom) = dialog.zoom(|| 1.0) else {
                    dialog.problem = true;
                    cx.notify();
                    return;
                };
                self.zoom_dialog = None;
                self.zoom_sheet(zoom, cx);
            }
            ZoomAction::Cancel => {
                self.zoom_dialog = None;
                cx.notify();
            }
            ZoomAction::Choose(choice) => {
                if let Some(dialog) = self.zoom_dialog.as_mut() {
                    dialog.choose(choice);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn zoom_dialog_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self.zoom_dialog.as_mut() else {
            return false;
        };
        match dialog.key(key, command) {
            ZoomKey::Take => self.zoom_dialog_action(ZoomAction::Take, cx),
            ZoomKey::Close => self.zoom_dialog_action(ZoomAction::Cancel, cx),
            ZoomKey::Handled => cx.notify(),
            ZoomKey::Unhandled => return false,
        }
        true
    }
}
