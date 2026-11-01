use super::super::DocView;
use crate::i18n;
use gpui::Context;
use jade_web::grid::Selection;
impl DocView {
    /// Data ▸ Flash Fill (⌘E) in the active cell's column: the rest of the
    /// list filled by the pattern its examples follow, selected; or, with
    /// none to follow, Excel's word for it.
    pub(super) fn sheet_flash_fill(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let Some(s) = self.sheet_mut() else {
            return;
        };
        s.menu = None;
        let result = match s.book.flash_fill(s.selection.active) {
            Ok(Some(filled)) => {
                s.selection = Selection::cell(filled.start);
                s.selection.focus = filled.end;
                Ok(true)
            }
            Ok(None) => {
                self.status = i18n::text("sheet.flash_fill_none").into();
                Ok(false)
            }
            Err(error) => Err(error),
        };
        self.edited(result, cx);
    }
}
