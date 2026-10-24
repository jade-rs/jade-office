use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::pivots::{
    PivotArea, PivotCreation, PivotCreationField, PivotDialog, PivotDialogField, PivotPanel,
    PivotReportLayout, PivotStyle,
};
type View = WorkbookView;
include!("pivots_ui.rs");
impl WorkbookView {
    fn pivot_editor(&self) -> Option<&PivotDialog> {
        self.pivot_dialog.as_ref().and_then(PivotPanel::editor)
    }
    fn pivot_creator(&self) -> Option<&PivotCreation> {
        self.pivot_dialog.as_ref().and_then(PivotPanel::creator)
    }
    pub(super) fn open_create_pivot(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.pivot_dialog = Some(PivotPanel::create(PivotCreation::open(
            &self.book,
            self.range(),
            self.active(),
        )));
        self.menu = None;
        cx.notify();
    }
    fn pivot_body_height(&self) -> f32 {
        (self.viewport.1 - 110.).clamp(80., 580.)
    }
    fn pivot_body_width(&self) -> f32 {
        (self.viewport.0 - 24.).clamp(200., 680.)
    }
    pub(super) fn pivot_editor_marked(&self) -> bool {
        self.pivot_dialog
            .as_ref()
            .is_some_and(|d| d.field().marked.is_some())
    }
    pub(super) fn open_pivot_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        match PivotDialog::open(&self.book, self.active()) {
            Ok(d) => self.pivot_dialog = Some(PivotPanel::edit(d)),
            Err(error) => {
                self.after_write(Err(error), cx);
                return;
            }
        }
        self.menu = None;
        cx.notify();
    }
    pub(super) fn pivot_action(&mut self, action: PivotAction, cx: &mut Context<Self>) {
        if matches!(action, PivotAction::Cancel) {
            self.pivot_dialog = None;
            cx.notify();
            return;
        }
        if self.pivot_creator().is_some() {
            if matches!(action, PivotAction::Take) {
                let result = self
                    .pivot_dialog
                    .as_ref()
                    .unwrap()
                    .creator()
                    .unwrap()
                    .apply(&mut self.book)
                    .map_err(pivot_problem);
                match &result {
                    Ok(summary) => {
                        self.select_cell(summary.reference.start, false, cx);
                        self.pivot_dialog = PivotDialog::open(&self.book, summary.reference.start)
                            .ok()
                            .map(PivotPanel::edit);
                    }
                    Err(error) => {
                        if let Some(d) =
                            self.pivot_dialog.as_mut().and_then(PivotPanel::creator_mut)
                        {
                            d.problem = Some(error.clone());
                        }
                    }
                }
                self.sheets_changed(result.map(|_| true), cx);
            } else if let Some(d) = self.pivot_dialog.as_mut().and_then(PivotPanel::creator_mut) {
                change_pivot_creation(d, action);
                cx.notify();
            }
            return;
        }
        if matches!(action, PivotAction::RefreshItems) {
            let Some(d) = self.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) else {
                return;
            };
            let result = d.refresh_items(&mut self.book).map_err(pivot_problem);
            if let Some(d) = self.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) {
                d.problem = result.as_ref().err().cloned();
            }
            self.after_write(result, cx);
            return;
        }
        if matches!(action, PivotAction::Take) {
            let Some(d) = self.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) else {
                return;
            };
            let result = d.apply(&mut self.book).map_err(pivot_problem);
            if result.is_ok() {
                self.pivot_dialog = None;
            } else if let Some(d) = self.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) {
                d.problem = result.as_ref().err().cloned();
            }
            self.after_write(result, cx);
            return;
        }
        if let Some(d) = self.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) {
            change_pivot_dialog(d, action);
        }
        cx.notify();
    }
    pub(super) fn pivot_key(
        &mut self,
        key: &str,
        shift: bool,
        command: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(d) = self.pivot_dialog.as_mut() {
            match key {
                "enter" => self.pivot_action(PivotAction::Take, cx),
                "escape" => self.pivot_action(PivotAction::Cancel, cx),
                _ => {
                    d.key(key, shift, command);
                    cx.notify();
                }
            }
            true
        } else {
            false
        }
    }
}
