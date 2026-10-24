use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_web::links::{LinkDialog, LinkField};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/links_ui.rs"
));
impl DocView {
    fn link_editor(&self) -> Option<&LinkDialog> {
        self.sheet_ref()?.link_dialog.as_ref()
    }
    pub(super) fn link_editor_marked(&self) -> bool {
        self.link_editor()
            .is_some_and(|d| d.field().marked.is_some())
    }
    pub(super) fn open_sheet_link_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        if let Some(s) = self.sheet_mut() {
            s.link_dialog = Some(LinkDialog::open(
                &s.book,
                s.selection.range(),
                s.selection.active,
            ));
            s.menu = None;
        }
        cx.notify();
    }
    pub(super) fn link_action(&mut self, action: LinkAction, cx: &mut Context<Self>) {
        let Some(s) = self.sheet_mut() else {
            return;
        };
        if matches!(action, LinkAction::Take) {
            let Some(dialog) = s.link_dialog.as_ref() else {
                return;
            };
            let link = match dialog.rule() {
                Ok(link) => link,
                Err(problem) => {
                    s.link_dialog.as_mut().unwrap().problem = Some(problem);
                    cx.notify();
                    return;
                }
            };
            let label = dialog.label().map(str::to_string);
            let result = s.book.insert_hyperlink(&link, label.as_deref());
            if result.is_ok() {
                s.link_dialog = None;
            }
            self.edited(result, cx);
            return;
        }
        match action {
            LinkAction::Cancel => s.link_dialog = None,
            LinkAction::Focus(field) => {
                if let Some(d) = s.link_dialog.as_mut() {
                    d.focus(field);
                }
            }
            LinkAction::Place(i) => {
                if let Some(d) = s.link_dialog.as_mut() {
                    d.choose_place(i);
                }
            }
            _ => {}
        }
        cx.notify();
    }
    pub(super) fn link_key(
        &mut self,
        key: &str,
        shift: bool,
        command: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(dialog) = self.sheet_mut().and_then(|s| s.link_dialog.as_mut()) {
            match key {
                "enter" => self.link_action(LinkAction::Take, cx),
                "escape" => self.link_action(LinkAction::Cancel, cx),
                _ => {
                    dialog.key(key, shift, command);
                    cx.notify();
                }
            }
            true
        } else {
            false
        }
    }
}
