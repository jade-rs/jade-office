//! The Word view's dialogs — タブとリーダー, 線種とページ罫線と網かけの設定,
//! ドロップキャップ, 番号の設定, リストのインデントの調整, 新しいアウトライン
//! の定義, 新しいリスト スタイルの定義, スタイルの作成 / 変更, クイック パーツ
//! の保存 — over the pages, and the small kit they are built from: text
//! fields that take typing (Japanese input included) and keys, choice chips,
//! check boxes and the OK / キャンセル row.
//!
//! A dialog has the keyboard while it is open: Tab moves between its
//! fields, Enter is OK and Escape キャンセル. What a dialog does is in its
//! area's module; this only frames it.

use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, MouseButton, SharedString};
use jade_office_model::text::TextEdit;

use crate::word_design::QuickPartDialog;
use crate::word_lists::{DefineListDialog, ListIndentDialog, ListStartDialog};
use crate::word_paragraph::{BordersDialog, DropCapDialog, TabsDialog};
use crate::word_styles::StyleDialog;
use crate::{i18n, theme, DocumentView};

/// The text fields of a dialog, and which one has the keyboard.
#[derive(Default)]
pub struct Inputs {
    pub fields: Vec<TextEdit>,
    pub focus: Option<usize>,
    /// Where typing goes while no field has the keyboard: nowhere.
    sink: Option<TextEdit>,
}

impl Inputs {
    pub fn new(values: Vec<String>) -> Self {
        let focus = (!values.is_empty()).then_some(0);
        Inputs {
            fields: values.into_iter().map(TextEdit::selected).collect(),
            focus,
            sink: None,
        }
    }

    pub fn text(&self, index: usize) -> &str {
        self.fields
            .get(index)
            .map(|field| field.text.as_str())
            .unwrap_or("")
    }

    pub fn set(&mut self, index: usize, value: String) {
        if let Some(field) = self.fields.get_mut(index) {
            *field = TextEdit::selected(value);
        }
    }

    pub fn focused_mut(&mut self) -> &mut TextEdit {
        match self.focus.and_then(|index| self.fields.get_mut(index)) {
            Some(field) => field,
            None => self.sink.insert(TextEdit::new(String::new())),
        }
    }

    pub fn focused(&self) -> Option<&TextEdit> {
        self.focus.and_then(|index| self.fields.get(index))
    }

    fn next(&mut self, backwards: bool) {
        let count = self.fields.len();
        if count == 0 {
            return;
        }
        let current = self.focus.unwrap_or(0);
        let next = if backwards {
            (current + count - 1) % count
        } else {
            (current + 1) % count
        };
        self.focus = Some(next);
        if let Some(field) = self.fields.get_mut(next) {
            field.whole = true;
        }
    }
}

/// The dialog open over the pages.
pub enum WordDialog {
    Tabs(TabsDialog),
    Borders(BordersDialog),
    DropCap(DropCapDialog),
    ListStart(ListStartDialog),
    ListIndent(ListIndentDialog),
    DefineList(DefineListDialog),
    Style(StyleDialog),
    QuickPart(QuickPartDialog),
}

impl WordDialog {
    pub fn inputs(&self) -> &Inputs {
        match self {
            WordDialog::Tabs(dialog) => &dialog.inputs,
            WordDialog::Borders(dialog) => &dialog.inputs,
            WordDialog::DropCap(dialog) => &dialog.inputs,
            WordDialog::ListStart(dialog) => &dialog.inputs,
            WordDialog::ListIndent(dialog) => &dialog.inputs,
            WordDialog::DefineList(dialog) => &dialog.inputs,
            WordDialog::Style(dialog) => &dialog.inputs,
            WordDialog::QuickPart(dialog) => &dialog.inputs,
        }
    }

    pub fn inputs_mut(&mut self) -> &mut Inputs {
        match self {
            WordDialog::Tabs(dialog) => &mut dialog.inputs,
            WordDialog::Borders(dialog) => &mut dialog.inputs,
            WordDialog::DropCap(dialog) => &mut dialog.inputs,
            WordDialog::ListStart(dialog) => &mut dialog.inputs,
            WordDialog::ListIndent(dialog) => &mut dialog.inputs,
            WordDialog::DefineList(dialog) => &mut dialog.inputs,
            WordDialog::Style(dialog) => &mut dialog.inputs,
            WordDialog::QuickPart(dialog) => &mut dialog.inputs,
        }
    }
}

impl DocumentView {
    /// The field typing goes into while a dialog is open.
    pub(crate) fn dialog_field(&self) -> Option<&TextEdit> {
        self.tools.dialog.as_ref()?.inputs().focused()
    }

    pub(crate) fn dialog_field_mut(&mut self) -> Option<&mut TextEdit> {
        Some(self.tools.dialog.as_mut()?.inputs_mut().focused_mut())
    }

    /// Changes the open dialog and redraws it.
    pub(crate) fn edit_dialog(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut WordDialog),
    ) {
        if let Some(dialog) = self.tools.dialog.as_mut() {
            edit(dialog);
            cx.notify();
        }
    }

    /// Keys while a dialog is open; it takes them all.
    pub(crate) fn handle_dialog_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(dialog) = self.tools.dialog.as_mut() else {
            return false;
        };
        let inputs = dialog.inputs_mut();
        if inputs.focused().is_some_and(|field| field.marked.is_some()) {
            return false;
        }
        let shift = event.keystroke.modifiers.shift;
        match event.keystroke.key.as_str() {
            "escape" => self.tools.dialog = None,
            "enter" | "return" => {
                return {
                    self.dialog_ok(cx);
                    true
                }
            }
            "tab" => inputs.next(shift),
            "backspace" => inputs.focused_mut().backspace(),
            "delete" => inputs.focused_mut().delete(),
            "left" => inputs.focused_mut().step(false),
            "right" => inputs.focused_mut().step(true),
            "home" | "up" => inputs.focused_mut().to_end(false),
            "end" | "down" => inputs.focused_mut().to_end(true),
            // Everything else — a character — comes through the input
            // handler into the focused field (which the dialog routes it
            // to), so the key goes on to it.
            _ => return false,
        }
        cx.notify();
        true
    }

    /// OK: the dialog's change, as one undo step.
    pub(crate) fn dialog_ok(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.tools.dialog.take() else {
            return;
        };
        self.tools_caret();
        let result = match dialog {
            WordDialog::Tabs(dialog) => self.apply_tabs_dialog(dialog),
            WordDialog::Borders(dialog) => self.apply_borders_dialog(dialog),
            WordDialog::DropCap(dialog) => self.apply_drop_cap_dialog(dialog),
            WordDialog::ListStart(dialog) => self.apply_list_start_dialog(dialog),
            WordDialog::ListIndent(dialog) => self.apply_list_indent_dialog(dialog),
            WordDialog::DefineList(dialog) => self.apply_define_list_dialog(dialog),
            WordDialog::Style(dialog) => self.apply_style_dialog(dialog),
            WordDialog::QuickPart(dialog) => self.apply_quick_part_dialog(dialog),
        };
        match result {
            // A dialog that is not done (a value it cannot take) stays open.
            Err(DialogError::Stay(dialog, message)) => {
                self.tools.dialog = Some(dialog);
                self.tools.status = Some(SharedString::from(message));
                cx.notify();
            }
            Err(DialogError::Failed(message)) => self.tool_done(Err(message), cx),
            Ok(()) => self.tool_done(Ok(()), cx),
        }
    }

    /// The open dialog, over everything.
    pub(crate) fn dialog_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.tools.dialog.as_ref()?;
        let (title, width, body) = match dialog {
            WordDialog::Tabs(dialog) => (
                i18n::text("word.tabs.title"),
                460.0,
                self.tabs_dialog_body(dialog, cx),
            ),
            WordDialog::Borders(dialog) => (
                i18n::text("word.borders.title"),
                560.0,
                self.borders_dialog_body(dialog, cx),
            ),
            WordDialog::DropCap(dialog) => (
                i18n::text("word.drop_cap.title"),
                380.0,
                self.drop_cap_dialog_body(dialog, cx),
            ),
            WordDialog::ListStart(dialog) => (
                i18n::text("word.list_start.title"),
                380.0,
                self.list_start_dialog_body(dialog, cx),
            ),
            WordDialog::ListIndent(dialog) => (
                i18n::text("word.list_indent.title"),
                420.0,
                self.list_indent_dialog_body(dialog, cx),
            ),
            WordDialog::DefineList(dialog) => (
                if dialog.style_name.is_some() {
                    i18n::text("word.list_style.title")
                } else {
                    i18n::text("word.define_list.title")
                },
                640.0,
                self.define_list_dialog_body(dialog, cx),
            ),
            WordDialog::Style(dialog) => (
                if dialog.editing.is_some() {
                    i18n::text("word.style.modify_title")
                } else {
                    i18n::text("word.style.new_title")
                },
                600.0,
                self.style_dialog_body(dialog, cx),
            ),
            WordDialog::QuickPart(dialog) => (
                i18n::text("word.quick_part.title"),
                420.0,
                self.quick_part_dialog_body(dialog, cx),
            ),
        };
        let status = self.tools.status.clone();
        Some(
            div()
                .id("word-dialog-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000055))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("word-dialog")
                        .w(px(width))
                        .max_h(px(680.0))
                        .overflow_y_scroll()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_md()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(title))
                        .child(body)
                        .children(
                            status
                                .map(|status| div().text_color(theme::text_faint()).child(status)),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    button(
                                        "word-dialog-cancel",
                                        i18n::text("format.cancel"),
                                        false,
                                    )
                                    .on_click(on(
                                        cx,
                                        |view, cx| {
                                            view.tools.dialog = None;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    button("word-dialog-ok", i18n::text("format.ok"), true)
                                        .on_click(on(cx, |view, cx| view.dialog_ok(cx))),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// A text field of the open dialog, focused on a click.
    pub(crate) fn dialog_field_element(
        &self,
        index: usize,
        width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let inputs = self.tools.dialog.as_ref().map(|dialog| dialog.inputs());
        let field = inputs.and_then(|inputs| inputs.fields.get(index));
        let focused = inputs.is_some_and(|inputs| inputs.focus == Some(index));
        let (before, after) = field.map(|field| field.split()).unwrap_or(("", ""));
        let whole = field.is_some_and(|field| field.whole);
        div()
            .id(("word-dialog-field", index))
            .flex()
            .items_center()
            .w(px(width))
            .h(px(24.0))
            .px_2()
            .rounded_sm()
            .border_1()
            .border_color(if focused {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(theme::canvas())
            .text_color(theme::text())
            .cursor_text()
            .overflow_hidden()
            .whitespace_nowrap()
            .on_click(on(cx, move |view, cx| {
                view.edit_dialog(cx, |dialog| {
                    let inputs = dialog.inputs_mut();
                    inputs.focus = Some(index);
                    if let Some(field) = inputs.fields.get_mut(index) {
                        field.whole = false;
                        field.caret = field.text.len();
                    }
                })
            }))
            .child(
                div()
                    .when(whole && focused, |text| text.bg(theme::selected()))
                    .child(before.to_string()),
            )
            .when(focused && !whole, |field| {
                field.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
            })
            .child(after.to_string())
    }
}

/// A click handler on the view, as `cx.listener` makes one but without
/// holding `cx` borrowed: an element built with it can sit beside others
/// built with `&mut cx`.
pub(crate) fn on<E: ?Sized>(
    cx: &Context<DocumentView>,
    f: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
) -> impl Fn(&E, &mut gpui::Window, &mut gpui::App) + 'static {
    let view = cx.weak_entity();
    move |_event, _window, cx| {
        view.update(cx, |view, cx| f(view, cx)).ok();
    }
}

/// [`on`] for a handler that reads the event.
pub(crate) fn on_event<E: ?Sized>(
    cx: &Context<DocumentView>,
    f: impl Fn(&mut DocumentView, &E, &mut gpui::Window, &mut Context<DocumentView>) + 'static,
) -> impl Fn(&E, &mut gpui::Window, &mut gpui::App) + 'static {
    let view = cx.weak_entity();
    move |event, window, cx| {
        view.update(cx, |view, cx| f(view, event, window, cx)).ok();
    }
}

/// Why OK did not close the dialog.
pub enum DialogError {
    /// A value it cannot take: the dialog stays open with the message.
    Stay(WordDialog, String),
    /// The edit failed: the dialog closes with the message.
    Failed(String),
}

impl From<String> for DialogError {
    fn from(message: String) -> Self {
        DialogError::Failed(message)
    }
}

/// A choice among choices: highlighted when chosen.
pub fn chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |chip| chip.bg(theme::selected()))
        .cursor_pointer()
        .hover(|chip| chip.bg(theme::hovered()))
        .whitespace_nowrap()
        .child(label.into())
}

/// A check box and its label.
pub fn check(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1p5()
        .cursor_pointer()
        .child(
            div()
                .size(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .when(on, |square| {
                    square.child(
                        gpui::svg()
                            .path("icons/check.svg")
                            .size_3()
                            .text_color(theme::accent()),
                    )
                }),
        )
        .child(label.into())
}

/// A dialog button; `primary` is OK's look.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if primary {
            theme::accent()
        } else {
            theme::border()
        })
        .when(primary, |button| button.text_color(theme::accent()))
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .whitespace_nowrap()
        .child(label.into())
}

/// A row: its label, then what is set there.
pub fn labeled(label: impl Into<SharedString>) -> gpui::Div {
    div().flex().flex_wrap().items_center().gap_1().child(
        div()
            .w(px(120.0))
            .flex_none()
            .text_color(theme::text_dim())
            .child(label.into()),
    )
}

/// A section heading inside a dialog.
pub fn section(label: impl Into<SharedString>) -> gpui::Div {
    div()
        .pt_1()
        .text_color(theme::text_faint())
        .child(label.into())
}
