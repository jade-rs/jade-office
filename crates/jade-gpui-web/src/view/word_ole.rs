//! 挿入 › オブジェクト in the browser, as the native app's `word_ole`: a
//! file picked from the device is embedded as an OLE package, shown as its
//! icon (アイコンで表示) or a Word document's first page; a double click
//! on it hands the file it holds back to the browser, which saves or opens
//! it — the page cannot start another app.

use std::cell::Cell;

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_web::word::picture::PlacedObject;

use super::word_tools_ui::Pick;
use super::{DocView as DocumentView, Document};
use crate::ribbon::{group, listener, row, Action};
use crate::tooltip::Tooltip;
use crate::{i18n, palette as theme};

/// What the オブジェクト group's controls do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OleAction {
    /// アイコンで表示, on or off.
    ToggleIcon,
    /// ファイルから…: the page's picker, the file embedded.
    FromFile,
}

thread_local! {
    /// アイコンで表示, as the group shows it: on, as in Word.
    static AS_ICON: Cell<bool> = const { Cell::new(true) };
}

/// Whether the next object goes in as its icon.
pub fn as_icon() -> bool {
    AS_ICON.with(Cell::get)
}

/// 挿入's オブジェクト group: ファイルから… and アイコンで表示.
pub fn insert_group(cx: &mut Context<DocumentView>) -> AnyElement {
    let icon = as_icon();
    let from_file = div()
        .id("insert-object")
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(i18n::text("object.tooltip")))
        .on_click(listener(Action::Ole(OleAction::FromFile), cx))
        .child(i18n::text("object.from_file"))
        .into_any_element();
    let as_icon = super::word_kit::check("insert-object-icon", i18n::text("object.icon"), icon)
        .on_click(listener(Action::Ole(OleAction::ToggleIcon), cx))
        .into_any_element();
    group(
        i18n::text("object.group"),
        vec![row(vec![from_file]), row(vec![as_icon])],
    )
}

impl DocumentView {
    pub(crate) fn ole_action(&mut self, action: OleAction, cx: &mut Context<Self>) {
        match action {
            OleAction::ToggleIcon => {
                AS_ICON.with(|on| on.set(!on.get()));
                cx.notify();
            }
            OleAction::FromFile => {
                self.tools.picking = Some(Pick::Object);
                self.host.pick();
            }
        }
    }

    /// The file the picker brought, embedded at the caret.
    pub(crate) fn insert_object_file(&mut self, name: &str, bytes: &[u8], cx: &mut Context<Self>) {
        let (name, bytes, icon) = (name.to_string(), bytes.to_vec(), as_icon());
        self.word_edit(cx, move |document| {
            document.insert_object_file(&name, bytes, icon)
        });
    }

    /// A double click on `object`: when it embeds a file, the browser is
    /// handed the file. Whether it was.
    pub(crate) fn open_object(&mut self, object: &PlacedObject) -> bool {
        let Document::Pages { document, .. } = &self.document else {
            return false;
        };
        let Some(file) =
            document.object_file(object.target.paragraph_index, object.target.run_index)
        else {
            return false;
        };
        self.host.save(&file.name, &file.bytes);
        true
    }
}
