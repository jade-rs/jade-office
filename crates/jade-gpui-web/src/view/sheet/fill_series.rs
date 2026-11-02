use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::fill_series::{DateUnit, SeriesDialog, SeriesField, SeriesIn, SeriesKey, SeriesType};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/fill_series_ui.rs"
));
impl DocView {
    fn series_editor(&self) -> Option<&SeriesDialog> {
        self.sheet_ref()?.series_dialog.as_ref()
    }
    /// Fill ▸ Series… over the selection.
    pub(super) fn open_series_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            let dialog = SeriesDialog::open(&sheet.book, sheet.selection.range());
            sheet.series_dialog = Some(dialog);
        }
        cx.notify();
    }
    pub(super) fn series_action(&mut self, action: SeriesAction, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            SeriesAction::Take => {
                let Some(dialog) = sheet.series_dialog.as_mut() else {
                    return;
                };
                // A step or stop that is not a number keeps the dialog open.
                let Some(result) = dialog.apply(&mut sheet.book) else {
                    cx.notify();
                    return;
                };
                sheet.series_dialog = None;
                self.edited(result, cx);
            }
            SeriesAction::Cancel => {
                sheet.series_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = sheet.series_dialog.as_mut() {
                    change_series(dialog, change);
                }
                cx.notify();
            }
        }
    }
    pub(super) fn series_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.series_dialog.as_mut())
        else {
            return false;
        };
        match dialog.key(key, command) {
            SeriesKey::Take => self.series_action(SeriesAction::Take, cx),
            SeriesKey::Close => self.series_action(SeriesAction::Cancel, cx),
            SeriesKey::Handled => cx.notify(),
            SeriesKey::Unhandled => return false,
        }
        true
    }
}
