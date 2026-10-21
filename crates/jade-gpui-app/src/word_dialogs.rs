//! Word's small dialogs, as bars over the pages that take the keyboard while
//! they are open, as the header and comment bars do: ハイパーリンクの挿入
//! (⌘K), 脚注の挿入 (⌥⌘F), 文末脚注の挿入 (⌥⌘D) and 段落 (⌥⌘M). What
//! each does is `jade-office-model`'s; the browser shell has the same
//! (`jade-gpui-web/src/view/word_dialogs.rs`).

use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, KeyDownEvent, SharedString};
use jade_office_model::text::TextEdit;
use jade_office_model::word::{
    IndentUnit, LineSpacing, LinkForm, LinkTarget, NoteForm, NoteKind, ParagraphField,
    ParagraphForm, SpecialIndent, WordLinks, WordParagraphs,
};

use crate::{t, theme, DocumentView};

/// The dialog open over the pages.
pub enum Dialog {
    /// ハイパーリンクの挿入 / ハイパーリンクの編集.
    Link(LinkForm),
    /// 脚注の挿入 / 文末脚注の挿入: the note's text.
    Note(NoteForm),
    /// 段落: indents, spacing, line spacing, pagination.
    Paragraph(Box<ParagraphForm>),
}

impl Dialog {
    /// The field typing goes into.
    pub fn field(&self) -> Option<&TextEdit> {
        match self {
            Dialog::Link(form) => Some(form.field()),
            Dialog::Note(form) => Some(&form.text),
            Dialog::Paragraph(form) => Some(form.focused()),
        }
    }

    pub fn field_mut(&mut self) -> Option<&mut TextEdit> {
        match self {
            Dialog::Link(form) => Some(form.field_mut()),
            Dialog::Note(form) => Some(&mut form.text),
            Dialog::Paragraph(form) => Some(form.focused_mut()),
        }
    }
}

/// A labelled one-line field, its caret drawn when it has the keyboard.
pub fn field_box(label: &str, field: &TextEdit, focused: bool, width: f32) -> AnyElement {
    let (before, after) = field.split();
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(theme::text_dim())
                .child(label.to_string()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .w(px(width))
                .h(px(24.0))
                .px_2()
                .rounded_md()
                .bg(theme::canvas())
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .text_sm()
                .text_color(theme::text())
                .overflow_hidden()
                .whitespace_nowrap()
                // All of it selected: what is typed replaces it.
                .when(focused && field.whole, |row| {
                    row.child(div().bg(theme::selected()).child(field.text.clone()))
                })
                .when(focused && field.whole, |row| {
                    row.child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                })
                .when(!(focused && field.whole), |row| {
                    row.child(before.to_string())
                })
                .when(focused && !field.whole, |row| {
                    row.child(div().w(px(1.0)).h(px(16.0)).bg(theme::accent()))
                })
                .when(!(focused && field.whole), |row| {
                    row.child(after.to_string())
                }),
        )
        .into_any_element()
}

/// The bar a dialog sits in, over the pages.
pub fn bar(title: &str, keys: &str, body: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .flex_none()
        .px_4()
        .py_2()
        .bg(theme::panel())
        .border_b_1()
        .border_color(theme::border())
        .child(
            div()
                .text_xs()
                .text_color(theme::text_faint())
                .child(title.to_string()),
        )
        .children(body)
        .child(
            div()
                .text_xs()
                .text_color(theme::text_faint())
                .child(keys.to_string()),
        )
        .into_any_element()
}

impl DocumentView {
    fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.find_bar = None;
        self.story_bar = None;
        self.comment_bar = None;
        self.ribbon.popup = None;
        self.dialog = Some(dialog);
        cx.notify();
    }

    /// The selection the shell holds, handed to the document so a dialog
    /// reads and writes the text the user sees selected.
    fn sync_selection(&mut self) {
        if self.document.selection().is_none() {
            self.document
                .place_caret(self.insert_at.0, self.insert_at.1);
        }
    }

    /// ⌘K: ハイパーリンクの挿入 on the selection, or the link the caret is
    /// in, to edit.
    pub(crate) fn open_link_dialog(&mut self, cx: &mut Context<Self>) {
        self.sync_selection();
        let draft = self.document.link_draft();
        self.open_dialog(Dialog::Link(LinkForm::new(draft)), cx);
    }

    /// ⌥⌘F / ⌥⌘D: the bar the note's text is typed into; its reference goes
    /// at the caret, or after the selection, when the text is put in.
    pub(crate) fn open_note_dialog(&mut self, kind: NoteKind, cx: &mut Context<Self>) {
        self.sync_selection();
        let form = NoteForm::new(&self.document, kind);
        self.open_dialog(Dialog::Note(form), cx);
    }

    /// 段落… (⌥⌘M): the dialog on the format of the caret's paragraph, or
    /// the first one selected.
    pub(crate) fn open_paragraph_dialog(&mut self, cx: &mut Context<Self>) {
        self.sync_selection();
        let form = ParagraphForm::new(self.document.paragraph_format());
        self.open_dialog(Dialog::Paragraph(Box::new(form)), cx);
    }

    /// Changes the open 段落 dialog from a click on one of its controls.
    pub(crate) fn edit_paragraph_form(
        &mut self,
        edit: impl FnOnce(&mut ParagraphForm),
        cx: &mut Context<Self>,
    ) {
        if let Some(Dialog::Paragraph(form)) = self.dialog.as_mut() {
            edit(form);
            cx.notify();
        }
    }

    /// ⌘-click: a link's address to the system, a bookmark's place to the
    /// caret. Returns whether there was a link under the point.
    pub(crate) fn follow_link_at(
        &mut self,
        x: f64,
        document_y: f64,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(link) = self.document.hyperlink_at_point(x, document_y) else {
            return false;
        };
        match self.document.link_target(&link) {
            Some(LinkTarget::Address(address)) => cx.open_url(&address),
            Some(LinkTarget::Position(position)) => {
                self.selection_anchor = None;
                self.drag_anchor = None;
                self.jump_to(position, cx);
                if let Some(caret) = self.caret {
                    let (page, _) = self.document.page_and_offset(caret.document_y);
                    self.scroll.scroll_to_item(page);
                    self.current_page = page;
                }
            }
            None => {
                self.table_status = Some(SharedString::from(t("link.missing_bookmark")));
                cx.notify();
            }
        }
        true
    }

    /// Keys an open dialog takes: Enter applies it, Escape closes it, Tab
    /// moves between its fields, and the field's own editing keys.
    pub(crate) fn handle_dialog_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(dialog) = self.dialog.as_mut() else {
            return false;
        };
        if dialog.field().is_some_and(|field| field.marked.is_some()) {
            return false;
        }
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.dialog = None,
            "enter" | "return" => return self.submit_dialog(cx),
            "tab" => match dialog {
                Dialog::Link(form) => form.switch_field(),
                Dialog::Note(form) => form.text.insert("\t"),
                Dialog::Paragraph(form) => form.step_focus(!event.keystroke.modifiers.shift),
            },
            _ => {
                let edited = dialog.field_mut().is_some_and(|field| field.edit_key(key));
                if !edited {
                    return false;
                }
            }
        }
        cx.notify();
        true
    }

    /// OK: what the dialog says, done to the document as one undo step.
    pub(crate) fn submit_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.dialog.take() else {
            return false;
        };
        let result = match &dialog {
            Dialog::Link(form) => form.submit(&mut self.document),
            Dialog::Note(form) => form.submit(&mut self.document),
            Dialog::Paragraph(form) => {
                // The caret, or the selection, stays where it is.
                let kept = self.insert_at;
                form.submit(&mut self.document).map(|()| kept)
            }
        };
        match result {
            Ok(caret) => {
                self.dirty = true;
                self.table_status = None;
                if self.document.selection().is_none() {
                    self.selection_anchor = None;
                    self.insert_at = caret;
                    self.caret = self.document.place_caret(caret.0, caret.1);
                }
                self.rebuild_all_images();
            }
            Err(error) => {
                self.table_status = Some(SharedString::from(error));
                // Left open, so what was typed can be put right.
                self.dialog = Some(dialog);
            }
        }
        cx.notify();
        true
    }

    /// The open dialog, over the pages.
    pub(crate) fn dialog_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.dialog.as_ref()?;
        Some(match dialog {
            Dialog::Link(form) => bar(
                t(if form.draft.editing {
                    "link.edit"
                } else {
                    "link.insert"
                }),
                t("link.keys"),
                vec![
                    field_box(t("link.display"), &form.display, !form.on_address, 200.0),
                    field_box(t("link.address"), &form.address, form.on_address, 280.0),
                ],
            ),
            Dialog::Note(form) => bar(
                t(match form.kind {
                    NoteKind::Footnote => "note.footnote_title",
                    NoteKind::Endnote => "note.endnote_title",
                }),
                t("note.keys"),
                vec![field_box(t("note.text"), &form.text, true, 360.0)],
            ),
            Dialog::Paragraph(form) => self.paragraph_dialog(form, cx),
        })
    }

    /// One choice of a set — 配置, 最初の行, 行間 — pressed when it is the one
    /// chosen.
    fn choice(
        &self,
        id: impl Into<ElementId>,
        label: &'static str,
        active: bool,
        cx: &mut Context<Self>,
        pick: impl Fn(&mut ParagraphForm) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .px_2()
            .h(px(22.0))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if active {
                theme::accent()
            } else {
                theme::border()
            })
            .when(active, |button| button.bg(theme::selected()))
            .hover(|button| button.bg(theme::hovered()))
            .cursor_pointer()
            .text_xs()
            .text_color(theme::text())
            .child(label)
            .on_click(cx.listener(move |view, _event, _window, cx| {
                view.edit_paragraph_form(|form| pick(form), cx)
            }))
            .into_any_element()
    }

    /// A number field of the 段落 dialog, which a click gives the keyboard.
    fn number(
        &self,
        form: &ParagraphForm,
        field: ParagraphField,
        label: &'static str,
        unit: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(ElementId::Name(format!("paragraph-{field:?}").into()))
            .flex()
            .items_center()
            .gap_1()
            .cursor_text()
            .child(field_box(
                label,
                form.field(field),
                form.focus == field,
                56.0,
            ))
            .child(div().text_xs().text_color(theme::text_dim()).child(unit))
            .on_click(cx.listener(move |view, _event, _window, cx| {
                view.edit_paragraph_form(|form| form.focus_on(field), cx)
            }))
            .into_any_element()
    }

    /// A check box of 改ページと改行.
    fn check(
        &self,
        id: &'static str,
        label: &'static str,
        on: bool,
        cx: &mut Context<Self>,
        flip: impl Fn(&mut ParagraphForm) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .size(px(14.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(if on { theme::accent() } else { theme::border() })
                    .when(on, |box_| {
                        box_.bg(theme::accent()).text_color(theme::paper())
                    })
                    .child(if on { "✓" } else { "" }),
            )
            .child(label)
            .on_click(cx.listener(move |view, _event, _window, cx| {
                view.edit_paragraph_form(|form| flip(form), cx)
            }))
            .into_any_element()
    }

    /// The 段落 dialog: インデントと行間隔, 改ページと改行, OK and キャンセル.
    fn paragraph_dialog(&self, form: &ParagraphForm, cx: &mut Context<Self>) -> AnyElement {
        let format = &form.format;
        let unit = t(match format.unit {
            IndentUnit::Characters => "paragraph.unit.chars",
            IndentUnit::Millimetres => "paragraph.unit.mm",
        });
        let caption = |text: &'static str| {
            div()
                .w(px(72.0))
                .text_xs()
                .text_color(theme::text_faint())
                .child(text)
                .into_any_element()
        };
        let row = |children: Vec<AnyElement>| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(children)
                .into_any_element()
        };
        let mut alignment = vec![caption(t("paragraph.alignment"))];
        for (value, label) in [
            ("left", "paragraph.align.left"),
            ("center", "paragraph.align.center"),
            ("right", "paragraph.align.right"),
            ("both", "paragraph.align.both"),
        ] {
            alignment.push(self.choice(
                ElementId::Name(format!("paragraph-align-{value}").into()),
                t(label),
                format.alignment == value,
                cx,
                move |form| form.format.alignment = value.to_string(),
            ));
        }
        let mut indent = vec![
            caption(t("paragraph.indent")),
            self.number(form, ParagraphField::Left, t("paragraph.left"), unit, cx),
            self.number(form, ParagraphField::Right, t("paragraph.right"), unit, cx),
            div()
                .text_xs()
                .text_color(theme::text_dim())
                .child(t("paragraph.special"))
                .into_any_element(),
        ];
        for (special, label) in [
            (SpecialIndent::None, "paragraph.special.none"),
            (SpecialIndent::FirstLine, "paragraph.special.first_line"),
            (SpecialIndent::Hanging, "paragraph.special.hanging"),
        ] {
            indent.push(self.choice(
                ElementId::Name(format!("paragraph-special-{special:?}").into()),
                t(label),
                format.special == special,
                cx,
                move |form| {
                    form.format.special = special;
                    if special != SpecialIndent::None {
                        form.focus_on(ParagraphField::SpecialBy);
                    }
                },
            ));
        }
        indent.push(self.number(form, ParagraphField::SpecialBy, t("paragraph.by"), unit, cx));
        let mut spacing = vec![
            caption(t("paragraph.spacing")),
            self.number(
                form,
                ParagraphField::Before,
                t("paragraph.before"),
                "pt",
                cx,
            ),
            self.number(form, ParagraphField::After, t("paragraph.after"), "pt", cx),
            div()
                .text_xs()
                .text_color(theme::text_dim())
                .child(t("paragraph.line"))
                .into_any_element(),
        ];
        for (line, label) in [
            (LineSpacing::Single, "paragraph.line.single"),
            (LineSpacing::OneAndHalf, "paragraph.line.one_and_half"),
            (LineSpacing::Double, "paragraph.line.double"),
            (LineSpacing::AtLeast(0.0), "paragraph.line.at_least"),
            (LineSpacing::Exactly(0.0), "paragraph.line.exactly"),
            (LineSpacing::Multiple(0.0), "paragraph.line.multiple"),
        ] {
            let active = std::mem::discriminant(&format.line) == std::mem::discriminant(&line);
            spacing.push(self.choice(
                ElementId::Name(format!("paragraph-line-{label}").into()),
                t(label),
                active,
                cx,
                move |form| form.set_line_rule(line),
            ));
        }
        if format.line.amount().is_some() {
            let unit = if matches!(format.line, LineSpacing::Multiple(_)) {
                t("paragraph.unit.lines")
            } else {
                "pt"
            };
            spacing.push(self.number(
                form,
                ParagraphField::LineAmount,
                t("paragraph.by"),
                unit,
                cx,
            ));
        }
        let pagination = vec![
            caption(t("paragraph.pagination")),
            self.check(
                "paragraph-widow",
                t("paragraph.widow_control"),
                format.widow_control,
                cx,
                |form| form.format.widow_control = !form.format.widow_control,
            ),
            self.check(
                "paragraph-keep-next",
                t("paragraph.keep_next"),
                format.keep_next,
                cx,
                |form| form.format.keep_next = !form.format.keep_next,
            ),
            self.check(
                "paragraph-keep-lines",
                t("paragraph.keep_lines"),
                format.keep_lines,
                cx,
                |form| form.format.keep_lines = !form.format.keep_lines,
            ),
            self.check(
                "paragraph-page-break",
                t("paragraph.page_break_before"),
                format.page_break_before,
                cx,
                |form| form.format.page_break_before = !form.format.page_break_before,
            ),
        ];
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .px_3()
                .h(px(24.0))
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(theme::border())
                .when(primary, |button| {
                    button.bg(theme::accent()).text_color(theme::paper())
                })
                .when(!primary, |button| button.text_color(theme::text()))
                .cursor_pointer()
                .text_xs()
                .child(label)
        };
        let buttons = vec![
            button("paragraph-ok", t("format.ok"), true)
                .on_click(cx.listener(|view, _event, _window, cx| {
                    view.submit_dialog(cx);
                }))
                .into_any_element(),
            button("paragraph-cancel", t("format.cancel"), false)
                .on_click(cx.listener(|view, _event, _window, cx| {
                    view.dialog = None;
                    cx.notify();
                }))
                .into_any_element(),
            div()
                .text_xs()
                .text_color(theme::text_faint())
                .child(t("paragraph.keys"))
                .into_any_element(),
        ];
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_2()
            .px_4()
            .py_3()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_sm()
                    .text_color(theme::text())
                    .child(t("paragraph.title")),
            )
            .child(row(alignment))
            .child(row(indent))
            .child(row(spacing))
            .child(row(pagination))
            .child(row(buttons))
            .into_any_element()
    }
}
