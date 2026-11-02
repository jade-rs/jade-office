use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::fill_series::{
    DateUnit, SeriesDialog, SeriesField, SeriesIn, SeriesKey, SeriesType,
};
type View = WorkbookView;
include!("fill_series_ui.rs");
impl WorkbookView {
    fn series_editor(&self) -> Option<&SeriesDialog> {
        self.series_dialog.as_ref()
    }
    /// Fill ▸ Series… over the selection.
    pub(super) fn open_series_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.menu = None;
        self.series_dialog = Some(SeriesDialog::open(&self.book, self.range()));
        cx.notify();
    }
    pub(super) fn series_action(&mut self, action: SeriesAction, cx: &mut Context<Self>) {
        match action {
            SeriesAction::Take => {
                let Some(dialog) = self.series_dialog.as_mut() else {
                    return;
                };
                // A step or stop that is not a number keeps the dialog open.
                let Some(result) = dialog.apply(&mut self.book) else {
                    cx.notify();
                    return;
                };
                self.series_dialog = None;
                self.after_write(result, cx);
            }
            SeriesAction::Cancel => {
                self.series_dialog = None;
                cx.notify();
            }
            change => {
                if let Some(dialog) = self.series_dialog.as_mut() {
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
        let Some(dialog) = self.series_dialog.as_mut() else {
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
