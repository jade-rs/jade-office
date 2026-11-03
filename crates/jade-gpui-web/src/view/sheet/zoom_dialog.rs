use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, point, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::zoom_dialog::{ZoomChoice, ZoomDialog, ZoomKey};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/zoom_dialog_ui.rs"
));
impl DocView {
    fn zoom_dialog_editor(&self) -> Option<&ZoomDialog> {
        self.sheet_ref()?.zoom_dialog.as_ref()
    }
    /// View ▸ Zoom…, on the zoom the sheet is at.
    pub(super) fn open_zoom_dialog(&mut self, cx: &mut Context<Self>) {
        let zoom = self.zoom();
        if let Some(sheet) = self.sheet_mut() {
            sheet.menu = None;
            sheet.zoom_dialog = Some(ZoomDialog::new(zoom));
        }
        cx.notify();
    }
    /// View ▸ Zoom to Selection: the selection fills the window, scrolled to
    /// its top left.
    pub(super) fn zoom_to_selection(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_ref() else {
            return;
        };
        let range = sheet.selection.range();
        let size = (f64::from(sheet.viewport.0), f64::from(sheet.viewport.1));
        let zoom = sheet.book.zoom_to_selection(range, size);
        let (x, y) = sheet.book.scroll_to_corner(range.start);
        self.set_zoom(zoom, cx);
        let zoom = f64::from(self.zoom());
        if let Some(sheet) = self.sheet_mut() {
            sheet
                .scroll
                .set_offset(point(px(-(x * zoom) as f32), px(-(y * zoom) as f32)));
        }
        cx.notify();
    }
    pub(super) fn zoom_dialog_action(&mut self, action: ZoomAction, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            ZoomAction::Take => {
                let Some(dialog) = sheet.zoom_dialog.as_mut() else {
                    return;
                };
                if dialog.choice() == ZoomChoice::FitSelection {
                    sheet.zoom_dialog = None;
                    return self.zoom_to_selection(cx);
                }
                // Excel's 10–400% or the dialog stays open, saying so.
                let Some(zoom) = dialog.zoom(|| 1.0) else {
                    dialog.problem = true;
                    cx.notify();
                    return;
                };
                sheet.zoom_dialog = None;
                self.set_zoom(zoom, cx);
            }
            ZoomAction::Cancel => {
                sheet.zoom_dialog = None;
                cx.notify();
            }
            ZoomAction::Choose(choice) => {
                if let Some(dialog) = sheet.zoom_dialog.as_mut() {
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
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.zoom_dialog.as_mut())
        else {
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
