//! Word's small dialogs over a document's pages, as bars that take the
//! keyboard while they are open, as the header and comment bars do:
//! ハイパーリンクの挿入 (⌘K), 脚注の挿入 (⌥⌘F), 文末脚注の挿入 (⌥⌘D) and 段落
//! (⌥⌘M). What each does is `jade-office-model`'s; the
//! native app has the same (`jade-gpui-app/src/word_dialogs.rs`).

use gpui::ElementId;
use jade_web::word::{
    IndentUnit, LineSpacing, LinkForm, LinkTarget, NoteForm, NoteKind, ParagraphField,
    ParagraphForm, SpecialIndent, WordLinks, WordParagraphs,
};

use super::sheet::caret_line;
use super::*;

/// The dialog open over the pages.
pub(super) enum Dialog {
    /// ハイパーリンクの挿入 / ハイパーリンクの編集.
    Link(LinkForm),
    /// 脚注の挿入 / 文末脚注の挿入: the note's text.
    Note(NoteForm),
    /// 段落: indents, spacing, line spacing, pagination.
    Paragraph(Box<ParagraphForm>),
    /// ユーザー情報: the reviewer's name and initials.
    Reviewer(jade_web::word::reviewer::ReviewerForm),
}

impl Dialog {
    pub fn field(&self) -> Option<&TextEdit> {
        match self {
            Dialog::Link(form) => Some(form.field()),
            Dialog::Note(form) => Some(&form.text),
            Dialog::Paragraph(form) => Some(form.focused()),
            Dialog::Reviewer(form) => Some(form.focused()),
        }
    }

    pub fn field_mut(&mut self) -> Option<&mut TextEdit> {
        match self {
            Dialog::Link(form) => Some(form.field_mut()),
            Dialog::Note(form) => Some(&mut form.text),
            Dialog::Paragraph(form) => Some(form.focused_mut()),
            Dialog::Reviewer(form) => Some(form.focused_mut()),
        }
    }
}

/// A labelled one-line field, its caret drawn when it has the keyboard.
fn field_box(label: &'static str, field: &TextEdit, focused: bool, width: f32) -> gpui::AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(div().text_color(palette::dim()).child(label))
        .child(
            div()
                .w(px(width))
                .h(px(24.0))
                .px_2()
                .flex()
                .items_center()
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(if focused {
                    palette::accent()
                } else {
                    palette::border()
                })
                .bg(palette::canvas())
                .child(if focused {
                    caret_line(field, palette::accent().into(), false)
                } else {
                    div().whitespace_nowrap().child(field.text.clone())
                }),
        )
        .into_any_element()
}

/// The bar a dialog sits in, over the pages.
fn bar(title: &'static str, keys: &'static str, body: Vec<gpui::AnyElement>) -> gpui::AnyElement {
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap_3()
        .px_4()
        .py_1p5()
        .bg(palette::panel())
        .border_b_1()
        .border_color(palette::border())
        .text_xs()
        .text_color(palette::text())
        .child(div().text_color(palette::faint()).child(title))
        .children(body)
        .child(div().text_color(palette::dim()).child(keys))
        .into_any_element()
}

impl DocView {
    pub(super) fn dialog_open(&self) -> bool {
        matches!(
            &self.document,
            Document::Pages {
                dialog: Some(_),
                ..
            }
        )
    }

    fn open_dialog(&mut self, opened: Dialog, cx: &mut Context<Self>) {
        let Document::Pages {
            dialog,
            find,
            story,
            comment,
            ribbon,
            ..
        } = &mut self.document
        else {
            return;
        };
        *find = None;
        *story = None;
        *comment = None;
        ribbon.popup = None;
        *dialog = Some(Box::new(opened));
        cx.notify();
    }

    /// ⌘K: ハイパーリンクの挿入 on the selection, or the link the caret is
    /// in, to edit.
    pub(super) fn open_link_dialog(&mut self, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        let form = LinkForm::new(document.link_draft());
        self.open_dialog(Dialog::Link(form), cx);
    }

    /// ⌥⌘F / ⌥⌘D: the bar the note's text is typed into; its reference goes
    /// at the caret, or after the selection, when the text is put in.
    pub(super) fn open_note_dialog(&mut self, kind: NoteKind, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        let form = NoteForm::new(document.as_ref(), kind);
        self.open_dialog(Dialog::Note(form), cx);
    }

    /// 段落… (⌥⌘M): the dialog on the format of the caret's paragraph, or
    /// the first one selected.
    pub(super) fn open_paragraph_dialog(&mut self, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        let form = ParagraphForm::new(document.paragraph_format());
        self.open_dialog(Dialog::Paragraph(Box::new(form)), cx);
    }

    /// 校閲 › ユーザー情報: who comments and changes are put down to.
    pub(super) fn open_reviewer_dialog(&mut self, cx: &mut Context<Self>) {
        let form = jade_web::word::reviewer::ReviewerForm::new(&super::word_review::reviewer());
        self.open_dialog(Dialog::Reviewer(form), cx);
    }

    /// Changes the open 段落 dialog from a click on one of its controls.
    fn edit_paragraph_form(
        &mut self,
        edit: impl FnOnce(&mut ParagraphForm),
        cx: &mut Context<Self>,
    ) {
        if let Document::Pages {
            dialog: Some(dialog),
            ..
        } = &mut self.document
        {
            if let Dialog::Paragraph(form) = dialog.as_mut() {
                edit(form);
                cx.notify();
            }
        }
    }

    /// ⌘-click: a link's address opened in a new tab, a bookmark's place
    /// given the caret. Returns whether there was a link under the point.
    pub(super) fn follow_link_at(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let Document::Pages { document, .. } = &mut self.document else {
            return false;
        };
        let Some(link) = document.hyperlink_at_point(x, y) else {
            return false;
        };
        match document.link_target(&link) {
            Some(LinkTarget::Address(address)) => cx.open_url(&address),
            Some(LinkTarget::Position((paragraph, offset))) => {
                document.place_caret(paragraph, offset);
                self.reveal_caret();
                cx.notify();
            }
            None => {
                self.status = i18n::text("link.missing_bookmark").into();
                cx.notify();
            }
        }
        true
    }

    /// Keys an open dialog takes: Enter applies it, Escape closes it, Tab
    /// moves between its fields, and the field's own editing keys.
    pub(super) fn dialog_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        let Document::Pages { dialog, .. } = &mut self.document else {
            return false;
        };
        let Some(open) = dialog.as_deref_mut() else {
            return false;
        };
        if open.field().is_some_and(|field| field.marked.is_some()) {
            return false;
        }
        match key {
            "escape" => *dialog = None,
            "enter" => return self.submit_dialog(cx),
            "tab" => match open {
                Dialog::Link(form) => form.switch_field(),
                Dialog::Note(form) => form.text.insert("\t"),
                Dialog::Paragraph(form) => form.step_focus(!event.keystroke.modifiers.shift),
                Dialog::Reviewer(form) => form.next_field(),
            },
            _ => {
                if !open.field_mut().is_some_and(|field| field.edit_key(key)) {
                    return false;
                }
            }
        }
        cx.notify();
        true
    }

    /// OK: what the dialog says, done to the document as one undo step.
    fn submit_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let Document::Pages {
            document, dialog, ..
        } = &mut self.document
        else {
            return false;
        };
        let Some(open) = dialog.take() else {
            return false;
        };
        let result = match open.as_ref() {
            Dialog::Link(form) => form.submit(document.as_mut()).map(Some),
            Dialog::Note(form) => form.submit(document.as_mut()).map(Some),
            // The caret, or the selection, stays where it is.
            Dialog::Paragraph(form) => form.submit(document.as_mut()).map(|()| None),
            Dialog::Reviewer(form) => {
                use jade_web::word::review::WordReview;
                if let Some(chosen) = form.reviewer() {
                    super::word_review::set_reviewer(chosen.clone());
                    // Changes typed from now on go down to the new name.
                    if document.is_tracking_changes() {
                        document.set_tracking_changes(true, &chosen).map(|()| None)
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
        };
        match result {
            Ok(caret) => {
                if let Some((paragraph, offset)) = caret {
                    document.place_caret(paragraph, offset);
                }
                self.refresh_repainted();
                self.reveal_caret();
                self.word_result(Ok(()), cx);
            }
            Err(error) => {
                // Left open, so what was typed can be put right.
                *dialog = Some(open);
                self.word_result(Err(error), cx);
            }
        }
        true
    }

    pub(super) fn dialog_element(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let Document::Pages {
            dialog: Some(dialog),
            ..
        } = &self.document
        else {
            return None;
        };
        Some(match dialog.as_ref() {
            Dialog::Link(form) => bar(
                i18n::text(if form.draft.editing {
                    "link.edit"
                } else {
                    "link.insert"
                }),
                i18n::text("link.keys"),
                vec![
                    field_box(
                        i18n::text("link.display"),
                        &form.display,
                        !form.on_address,
                        200.0,
                    ),
                    field_box(
                        i18n::text("link.address"),
                        &form.address,
                        form.on_address,
                        280.0,
                    ),
                ],
            ),
            Dialog::Note(form) => bar(
                i18n::text(match form.kind {
                    NoteKind::Footnote => "note.footnote_title",
                    NoteKind::Endnote => "note.endnote_title",
                }),
                i18n::text("note.keys"),
                vec![field_box(i18n::text("note.text"), &form.text, true, 360.0)],
            ),
            Dialog::Paragraph(form) => self.paragraph_dialog(form, cx),
            Dialog::Reviewer(form) => {
                use jade_web::word::reviewer::ReviewerField;
                bar(
                    i18n::text("review.user.title"),
                    i18n::text("link.keys"),
                    vec![
                        field_box(
                            i18n::text("review.user.name"),
                            &form.name,
                            form.focus == ReviewerField::Name,
                            200.0,
                        ),
                        field_box(
                            i18n::text("review.user.initials"),
                            &form.initials,
                            form.focus == ReviewerField::Initials,
                            80.0,
                        ),
                    ],
                )
            }
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
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .px_2()
            .h(px(22.0))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if active {
                palette::accent()
            } else {
                palette::border()
            })
            .when(active, |button| button.bg(palette::selected()))
            .hover(|button| button.bg(palette::hovered()))
            .cursor_pointer()
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
    ) -> gpui::AnyElement {
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
            .child(div().text_color(palette::dim()).child(unit))
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
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .child(
                div()
                    .size(px(14.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(if on {
                        palette::accent()
                    } else {
                        palette::border()
                    })
                    .when(on, |check| {
                        check.bg(palette::accent()).text_color(palette::paper())
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
    fn paragraph_dialog(&self, form: &ParagraphForm, cx: &mut Context<Self>) -> gpui::AnyElement {
        let format = &form.format;
        let unit = i18n::text(match format.unit {
            IndentUnit::Characters => "paragraph.unit.chars",
            IndentUnit::Millimetres => "paragraph.unit.mm",
        });
        let caption = |text: &'static str| {
            div()
                .w(px(72.0))
                .text_color(palette::faint())
                .child(text)
                .into_any_element()
        };
        let label = |text: &'static str| {
            div()
                .text_color(palette::dim())
                .child(text)
                .into_any_element()
        };
        let row = |children: Vec<gpui::AnyElement>| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(children)
                .into_any_element()
        };
        let mut alignment = vec![caption(i18n::text("paragraph.alignment"))];
        for (value, key) in [
            ("left", "paragraph.align.left"),
            ("center", "paragraph.align.center"),
            ("right", "paragraph.align.right"),
            ("both", "paragraph.align.both"),
        ] {
            alignment.push(self.choice(
                ElementId::Name(format!("paragraph-align-{value}").into()),
                i18n::text(key),
                format.alignment == value,
                cx,
                move |form| form.format.alignment = value.to_string(),
            ));
        }
        let mut indent = vec![
            caption(i18n::text("paragraph.indent")),
            self.number(
                form,
                ParagraphField::Left,
                i18n::text("paragraph.left"),
                unit,
                cx,
            ),
            self.number(
                form,
                ParagraphField::Right,
                i18n::text("paragraph.right"),
                unit,
                cx,
            ),
            label(i18n::text("paragraph.special")),
        ];
        for (special, key) in [
            (SpecialIndent::None, "paragraph.special.none"),
            (SpecialIndent::FirstLine, "paragraph.special.first_line"),
            (SpecialIndent::Hanging, "paragraph.special.hanging"),
        ] {
            indent.push(self.choice(
                ElementId::Name(format!("paragraph-special-{special:?}").into()),
                i18n::text(key),
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
        indent.push(self.number(
            form,
            ParagraphField::SpecialBy,
            i18n::text("paragraph.by"),
            unit,
            cx,
        ));
        let mut spacing = vec![
            caption(i18n::text("paragraph.spacing")),
            self.number(
                form,
                ParagraphField::Before,
                i18n::text("paragraph.before"),
                "pt",
                cx,
            ),
            self.number(
                form,
                ParagraphField::After,
                i18n::text("paragraph.after"),
                "pt",
                cx,
            ),
            label(i18n::text("paragraph.line")),
        ];
        for (line, key) in [
            (LineSpacing::Single, "paragraph.line.single"),
            (LineSpacing::OneAndHalf, "paragraph.line.one_and_half"),
            (LineSpacing::Double, "paragraph.line.double"),
            (LineSpacing::AtLeast(0.0), "paragraph.line.at_least"),
            (LineSpacing::Exactly(0.0), "paragraph.line.exactly"),
            (LineSpacing::Multiple(0.0), "paragraph.line.multiple"),
        ] {
            let active = std::mem::discriminant(&format.line) == std::mem::discriminant(&line);
            spacing.push(self.choice(
                ElementId::Name(format!("paragraph-line-{key}").into()),
                i18n::text(key),
                active,
                cx,
                move |form| form.set_line_rule(line),
            ));
        }
        if format.line.amount().is_some() {
            let unit = if matches!(format.line, LineSpacing::Multiple(_)) {
                i18n::text("paragraph.unit.lines")
            } else {
                "pt"
            };
            spacing.push(self.number(
                form,
                ParagraphField::LineAmount,
                i18n::text("paragraph.by"),
                unit,
                cx,
            ));
        }
        let pagination = vec![
            caption(i18n::text("paragraph.pagination")),
            self.check(
                "paragraph-widow",
                i18n::text("paragraph.widow_control"),
                format.widow_control,
                cx,
                |form| form.format.widow_control = !form.format.widow_control,
            ),
            self.check(
                "paragraph-keep-next",
                i18n::text("paragraph.keep_next"),
                format.keep_next,
                cx,
                |form| form.format.keep_next = !form.format.keep_next,
            ),
            self.check(
                "paragraph-keep-lines",
                i18n::text("paragraph.keep_lines"),
                format.keep_lines,
                cx,
                |form| form.format.keep_lines = !form.format.keep_lines,
            ),
            self.check(
                "paragraph-page-break",
                i18n::text("paragraph.page_break_before"),
                format.page_break_before,
                cx,
                |form| form.format.page_break_before = !form.format.page_break_before,
            ),
        ];
        let button = |id: &'static str, text: &'static str, primary: bool| {
            div()
                .id(id)
                .px_3()
                .h(px(24.0))
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(palette::border())
                .when(primary, |button| {
                    button.bg(palette::accent()).text_color(palette::paper())
                })
                .cursor_pointer()
                .child(text)
        };
        let buttons = vec![
            button("paragraph-ok", i18n::text("format.ok"), true)
                .on_click(cx.listener(|view, _event, _window, cx| {
                    view.submit_dialog(cx);
                }))
                .into_any_element(),
            button("paragraph-cancel", i18n::text("format.cancel"), false)
                .on_click(cx.listener(|view, _event, _window, cx| {
                    if let Document::Pages { dialog, .. } = &mut view.document {
                        *dialog = None;
                    }
                    cx.notify();
                }))
                .into_any_element(),
            label(i18n::text("paragraph.keys")),
        ];
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_2()
            .px_4()
            .py_2()
            .bg(palette::panel())
            .border_b_1()
            .border_color(palette::border())
            .text_xs()
            .text_color(palette::text())
            .child(div().text_sm().child(i18n::text("paragraph.title")))
            .child(row(alignment))
            .child(row(indent))
            .child(row(spacing))
            .child(row(pagination))
            .child(row(buttons))
            .into_any_element()
    }
}
