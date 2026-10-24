use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::pivots::{
    PivotArea, PivotCreation, PivotCreationField, PivotDialog, PivotDialogField, PivotPanel,
    PivotReportLayout, PivotStyle,
};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/pivots_ui.rs"
));
impl DocView {
    fn pivot_editor(&self) -> Option<&PivotDialog> {
        self.sheet_ref()?
            .pivot_dialog
            .as_ref()
            .and_then(PivotPanel::editor)
    }
    fn pivot_creator(&self) -> Option<&PivotCreation> {
        self.sheet_ref()?
            .pivot_dialog
            .as_ref()
            .and_then(PivotPanel::creator)
    }
    pub(super) fn open_create_pivot(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.pivot_dialog = Some(PivotPanel::create(PivotCreation::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            )));
            s.menu = None;
        }
        cx.notify();
    }
    fn pivot_body_height(&self) -> f32 {
        self.sheet_ref()
            .map(|s| (s.viewport.1 - 110.).clamp(80., 580.))
            .unwrap_or(300.)
    }
    fn pivot_body_width(&self) -> f32 {
        self.sheet_ref()
            .map(|s| (s.viewport.0 - 24.).clamp(200., 680.))
            .unwrap_or(480.)
    }
    pub(super) fn pivot_editor_marked(&self) -> bool {
        self.sheet_ref()
            .and_then(|s| s.pivot_dialog.as_ref())
            .is_some_and(|d| d.field().marked.is_some())
    }
    pub(super) fn open_pivot_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let Some(s) = self.sheet_mut() else { return };
        match PivotDialog::open(&s.book, s.selection.active) {
            Ok(d) => s.pivot_dialog = Some(PivotPanel::edit(d)),
            Err(error) => {
                self.edited(Err(error), cx);
                return;
            }
        }
        if let Some(s) = self.sheet_mut() {
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn pivot_action(&mut self, action: PivotAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else { return };
        if matches!(action, PivotAction::Cancel) {
            s.pivot_dialog = None;
            cx.notify();
            return;
        }
        if s.pivot_dialog
            .as_ref()
            .and_then(PivotPanel::creator)
            .is_some()
        {
            if matches!(action, PivotAction::Take) {
                let result = s
                    .pivot_dialog
                    .as_ref()
                    .unwrap()
                    .creator()
                    .unwrap()
                    .apply(&mut s.book)
                    .map_err(pivot_problem);
                match &result {
                    Ok(summary) => {
                        s.pivot_dialog = PivotDialog::open(&s.book, summary.reference.start)
                            .ok()
                            .map(PivotPanel::edit);
                        self.select_cell(summary.reference.start, false, cx);
                    }
                    Err(error) => {
                        if let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::creator_mut) {
                            d.problem = Some(error.clone());
                        }
                    }
                }
                self.sheets_changed(result.map(|_| true), cx);
            } else if let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::creator_mut) {
                change_pivot_creation(d, action);
                cx.notify();
            }
            return;
        }
        if matches!(action, PivotAction::RefreshItems) {
            let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) else {
                return;
            };
            let result = d.refresh_items(&mut s.book).map_err(pivot_problem);
            if let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) {
                d.problem = result.as_ref().err().cloned();
            }
            self.edited(result, cx);
            return;
        }
        if matches!(action, PivotAction::Take) {
            let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) else {
                return;
            };
            let result = d.apply(&mut s.book).map_err(pivot_problem);
            if result.is_ok() {
                s.pivot_dialog = None;
            } else if let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) {
                d.problem = result.as_ref().err().cloned();
            }
            self.edited(result, cx);
            return;
        }
        if let Some(d) = s.pivot_dialog.as_mut().and_then(PivotPanel::editor_mut) {
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
        if let Some(d) = self.sheet_mut().and_then(|s| s.pivot_dialog.as_mut()) {
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
