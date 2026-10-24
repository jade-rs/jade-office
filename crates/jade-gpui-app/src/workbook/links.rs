use super::WorkbookView;
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, MouseButton};
use jade_office_model::links::{LinkDialog, LinkField};
type View = WorkbookView;
include!("links_ui.rs");
impl WorkbookView {
    fn link_editor(&self) -> Option<&LinkDialog> {
        self.link_dialog.as_ref()
    }
    pub(super) fn link_editor_marked(&self) -> bool {
        self.link_editor()
            .is_some_and(|d| d.field().marked.is_some())
    }
    pub(super) fn open_link_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        self.link_dialog = Some(LinkDialog::open(&self.book, self.range(), self.active()));
        self.menu = None;
        cx.notify();
    }
    pub(super) fn link_action(&mut self, action: LinkAction, cx: &mut Context<Self>) {
        if matches!(action, LinkAction::Take) {
            let Some(dialog) = self.link_dialog.as_ref() else {
                return;
            };
            let link = match dialog.rule() {
                Ok(link) => link,
                Err(problem) => {
                    self.link_dialog.as_mut().unwrap().problem = Some(problem);
                    cx.notify();
                    return;
                }
            };
            let label = dialog.label().map(str::to_string);
            let result = self.book.insert_hyperlink(&link, label.as_deref());
            if result.is_ok() {
                self.link_dialog = None;
            }
            self.after_write(result, cx);
            return;
        }
        match action {
            LinkAction::Cancel => self.link_dialog = None,
            LinkAction::Focus(field) => {
                if let Some(d) = self.link_dialog.as_mut() {
                    d.focus(field);
                }
            }
            LinkAction::Place(i) => {
                if let Some(d) = self.link_dialog.as_mut() {
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
        if let Some(dialog) = self.link_dialog.as_mut() {
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
