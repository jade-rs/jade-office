//! ルビ: Word's Phonetic Guide dialog — the selection's bases (文字列全体 or
//! 文字単位) each with its guide typed in, 配置, フォント, サイズ and
//! オフセット, a preview, and ルビの解除.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::text::TextEdit;
use jade_office_model::word::asian::{RubyDialog, WordAsian, RUBY_ALIGNMENTS};

use super::{button, caption, check, installed_font_lists, points, stepper, Dialog};
use crate::{i18n, theme, DocumentView};

/// The dialog, the paragraph's text and the base whose guide is typed.
pub struct RubyState {
    pub dialog: Box<RubyDialog>,
    pub text: String,
    pub editing: usize,
}

fn state(view: &mut DocumentView) -> Option<&mut RubyState> {
    match view.characters.dialog.as_mut()? {
        Dialog::Ruby(state) => Some(state),
        _ => None,
    }
}

/// Puts what was typed into the base being edited.
pub(super) fn commit_field(view: &mut DocumentView) {
    let typed = view.characters.field.text.clone();
    if let Some(state) = state(view) {
        let editing = state.editing;
        if let Some(group) = state.dialog.groups.get_mut(editing) {
            group.ruby = typed;
        }
    }
}

fn focus(view: &mut DocumentView, index: usize) {
    commit_field(view);
    let Some(state) = state(view) else { return };
    state.editing = index;
    let text = state
        .dialog
        .groups
        .get(index)
        .map(|group| group.ruby.clone())
        .unwrap_or_default();
    view.characters.field = TextEdit::new(text);
}

/// Moves typing to another base's guide, as a click on its field does.
#[cfg(test)]
pub(super) fn focus_for_test(view: &mut DocumentView, index: usize) {
    focus(view, index);
}

fn edit(view: &mut DocumentView, change: impl FnOnce(&mut RubyState)) {
    commit_field(view);
    if let Some(state) = state(view) {
        change(state);
    }
    focus(view, 0);
}

pub fn open(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.document.selection().is_none() {
        view.document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    match WordAsian::ruby_dialog(&view.document) {
        Ok(dialog) => {
            let text = view
                .document
                .paragraph_text(dialog.paragraph)
                .unwrap_or_default();
            let first = dialog
                .groups
                .first()
                .map(|group| group.ruby.clone())
                .unwrap_or_default();
            view.characters.open(Dialog::Ruby(RubyState {
                dialog: Box::new(dialog),
                text,
                editing: 0,
            }));
            view.characters.field = TextEdit::new(first);
        }
        Err(error) => view.table_status = Some(error.into()),
    }
    cx.notify();
}

pub(super) fn confirm(view: &mut DocumentView, state: RubyState, cx: &mut Context<DocumentView>) {
    let done = WordAsian::apply_ruby(&mut view.document, &state.dialog);
    view.after_character_edit(done, cx);
}

fn align_label(align: &str) -> &'static str {
    i18n::text(match align {
        "center" => "chars.ruby.align.center",
        "distributeLetter" => "chars.ruby.align.distribute_letter",
        "left" => "chars.ruby.align.left",
        "right" => "chars.ruby.align.right",
        "rightVertical" => "chars.ruby.align.right_vertical",
        _ => "chars.ruby.align.distribute_space",
    })
}

pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let field = view.characters.field.text.clone();
    let Some(current) = state(view) else {
        return div().into_any_element();
    };
    let dialog = current.dialog.clone();
    let editing = current.editing;
    let rows = dialog.groups.iter().enumerate().map(|(index, group)| {
        let active = index == editing;
        let shown = if active {
            format!("{field}▏")
        } else {
            group.ruby.clone()
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(150.0))
                    .px_1()
                    .border_1()
                    .border_color(theme::border())
                    .child(group.base.clone()),
            )
            .child(
                div()
                    .id(("ruby-text", index))
                    .w(px(220.0))
                    .min_h(px(20.0))
                    .px_1()
                    .border_1()
                    .border_color(if active {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(theme::canvas())
                    .cursor_text()
                    .child(shown)
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        focus(view, index);
                        cx.notify();
                    })),
            )
    });
    let (japanese, _) = installed_font_lists(cx);
    let fonts: Vec<Option<String>> = std::iter::once(None)
        .chain(japanese.into_iter().take(6).map(Some))
        .collect();
    let preview = div()
        .flex()
        .items_end()
        .gap_1()
        .children(dialog.groups.iter().map(|group| {
            let guide = if dialog
                .groups
                .get(editing)
                .is_some_and(|edited| edited.start == group.start)
            {
                field.clone()
            } else {
                group.ruby.clone()
            };
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(gpui::rgb(0x000000))
                        .child(guide),
                )
                .child(
                    div()
                        .text_size(px(18.0))
                        .text_color(gpui::rgb(0x000000))
                        .child(group.base.clone()),
                )
        }));
    let size = dialog.size;
    let raise = dialog.raise;
    div()
        .w(px(480.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .gap_2()
                .child(caption(i18n::text("chars.ruby.base")).w(px(150.0)))
                .child(caption(i18n::text("chars.ruby.ruby"))),
        )
        .children(rows)
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button("ruby-whole", i18n::text("chars.ruby.whole"), false).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| {
                            edit(view, |state| state.dialog.join_whole(&state.text));
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    button("ruby-each", i18n::text("chars.ruby.per_character"), false).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| {
                            edit(view, |state| state.dialog.split_per_character(&state.text));
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    button("ruby-clear", i18n::text("chars.ruby.clear"), false).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| {
                            edit(view, |state| state.dialog.clear());
                            cx.notify();
                        }),
                    ),
                ),
        )
        .child(caption(i18n::text("chars.ruby.align")))
        .child(div().flex().flex_wrap().gap_x_3().gap_y_1().children(
            RUBY_ALIGNMENTS.iter().enumerate().map(|(index, align)| {
                check(
                    ("ruby-align", index),
                    align_label(align),
                    dialog.align == *align,
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    commit_field(view);
                    if let Some(state) = state(view) {
                        state.dialog.align = align.to_string();
                    }
                    cx.notify();
                }))
            }),
        ))
        .child(caption(i18n::text("chars.ruby.font")))
        .child(div().flex().flex_wrap().gap_x_3().gap_y_1().children(
            fonts.into_iter().enumerate().map(|(index, font)| {
                let label = font
                    .clone()
                    .unwrap_or_else(|| i18n::text("chars.ruby.font_same").to_string());
                check(("ruby-font", index), label, dialog.font == font).on_click(cx.listener(
                    move |view, _: &ClickEvent, _, cx| {
                        commit_field(view);
                        if let Some(state) = state(view) {
                            state.dialog.font = font.clone();
                        }
                        cx.notify();
                    },
                ))
            }),
        ))
        .child(
            div()
                .flex()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(caption(i18n::text("chars.ruby.size")))
                        .child(stepper(
                            "ruby-size",
                            size.map(points)
                                .unwrap_or_else(|| i18n::text("chars.ruby.auto").to_string()),
                            true,
                            cx,
                            move |view, more| {
                                commit_field(view);
                                if let Some(state) = state(view) {
                                    let current = state.dialog.size.unwrap_or(5.0);
                                    state.dialog.size =
                                        Some((current + if more { 0.5 } else { -0.5 }).max(1.0));
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(caption(i18n::text("chars.ruby.offset")))
                        .child(stepper(
                            "ruby-offset",
                            raise
                                .map(points)
                                .unwrap_or_else(|| i18n::text("chars.ruby.auto").to_string()),
                            true,
                            cx,
                            move |view, more| {
                                commit_field(view);
                                if let Some(state) = state(view) {
                                    let current = state.dialog.raise.unwrap_or(9.0);
                                    state.dialog.raise =
                                        Some((current + if more { 0.5 } else { -0.5 }).max(0.0));
                                }
                            },
                        )),
                ),
        )
        .child(caption(i18n::text("chars.font.preview")))
        .child(
            div()
                .h(px(56.0))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(theme::border())
                .bg(gpui::white())
                .child(preview),
        )
        .into_any_element()
}
