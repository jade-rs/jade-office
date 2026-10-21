//! 体裁: Word's Japanese typography — the 段落 dialog's 体裁 tab for the
//! selected paragraphs, and the document's 文字体裁 (禁則処理のレベル with its
//! 行頭 / 行末禁則文字, 文字間隔の調整).

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::text::TextEdit;
use jade_web::word::typography::{TypographyDialog, KINSOKU_LEVELS, SPACING_CONTROLS};

use super::{caption, check, Dialog};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

/// Which custom set typing goes into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KinsokuSet {
    LineStart,
    LineEnd,
}

/// The dialog and the set being typed.
pub struct TypographyState {
    pub dialog: Box<TypographyDialog>,
    pub editing: Option<KinsokuSet>,
}

fn state(view: &mut DocumentView) -> Option<&mut TypographyState> {
    match view.characters.dialog.as_mut()? {
        Dialog::Typography(state) => Some(state),
        _ => None,
    }
}

/// Puts what was typed into the set being edited.
pub(super) fn commit_field(view: &mut DocumentView) {
    let text = view.characters.field.text.clone();
    if let Some(state) = state(view) {
        match state.editing {
            Some(KinsokuSet::LineStart) => state.dialog.document.no_line_break_before = Some(text),
            Some(KinsokuSet::LineEnd) => state.dialog.document.no_line_break_after = Some(text),
            None => {}
        }
    }
}

fn edit(view: &mut DocumentView, change: impl FnOnce(&mut TypographyDialog)) {
    commit_field(view);
    if let Some(state) = state(view) {
        change(&mut state.dialog);
    }
}

fn focus(view: &mut DocumentView, set: KinsokuSet) {
    commit_field(view);
    let Some(state) = state(view) else { return };
    state.editing = Some(set);
    let text = match set {
        KinsokuSet::LineStart => state.dialog.line_start_set(),
        KinsokuSet::LineEnd => state.dialog.line_end_set(),
    };
    view.characters.field = TextEdit::new(text);
}

/// A 体裁 check box: its label, whether it is checked, and how it sets.
type TypographyOption = (&'static str, bool, fn(&mut TypographyDialog, bool));

fn level_label(level: &str) -> &'static str {
    i18n::text(match level {
        "strict" => "chars.kinsoku.strict",
        "custom" => "chars.kinsoku.custom",
        _ => "chars.kinsoku.standard",
    })
}

fn control_label(control: &str) -> &'static str {
    i18n::text(match control {
        "compressPunctuation" => "chars.compress.punctuation",
        "compressPunctuationAndJapaneseKana" => "chars.compress.kana",
        _ => "chars.compress.none",
    })
}

pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let field = view.characters.field.text.clone();
    let Some(current) = state(view) else {
        return div().into_any_element();
    };
    let dialog = current.dialog.clone();
    let editing = current.editing;
    let paragraph = &dialog.paragraph;
    let options: [TypographyOption; 6] = [
        (
            "chars.typography.kinsoku",
            paragraph.kinsoku == Some(true),
            |dialog, on| dialog.paragraph.kinsoku = Some(on),
        ),
        (
            "chars.typography.word_wrap",
            paragraph.word_wrap == Some(false),
            |dialog, on| dialog.paragraph.word_wrap = Some(!on),
        ),
        (
            "chars.typography.overflow_punct",
            paragraph.overflow_punctuation == Some(true),
            |dialog, on| dialog.paragraph.overflow_punctuation = Some(on),
        ),
        (
            "chars.typography.top_line_punct",
            paragraph.top_line_punctuation == Some(true),
            |dialog, on| dialog.paragraph.top_line_punctuation = Some(on),
        ),
        (
            "chars.typography.auto_space_de",
            paragraph.auto_space_de == Some(true),
            |dialog, on| dialog.paragraph.auto_space_de = Some(on),
        ),
        (
            "chars.typography.auto_space_dn",
            paragraph.auto_space_dn == Some(true),
            |dialog, on| dialog.paragraph.auto_space_dn = Some(on),
        ),
    ];
    let paragraph_options =
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, on, set))| {
                        check(("typography-option", index), i18n::text(label), on).on_click(
                            cx.listener(move |view, _: &ClickEvent, _, cx| {
                                edit(view, |dialog| set(dialog, !on));
                                cx.notify();
                            }),
                        )
                    }),
            );
    let level = dialog
        .document
        .kinsoku_level
        .clone()
        .unwrap_or_else(|| "standard".into());
    let levels = div()
        .flex()
        .gap_3()
        .children(KINSOKU_LEVELS.iter().enumerate().map(|(index, value)| {
            check(
                ("kinsoku-level", index),
                level_label(value),
                level == *value,
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                edit(view, |dialog| dialog.set_level(value));
                if let Some(state) = state(view) {
                    state.editing = None;
                }
                view.characters.field = TextEdit::new(String::new());
                cx.notify();
            }))
        }));
    let custom = level == "custom";
    let set_box = |set: KinsokuSet, text: String, cx: &mut Context<DocumentView>| {
        let active = editing == Some(set);
        let shown = if active { format!("{field}▏") } else { text };
        div()
            .id(match set {
                KinsokuSet::LineStart => "kinsoku-start",
                KinsokuSet::LineEnd => "kinsoku-end",
            })
            .w(px(460.0))
            .min_h(px(22.0))
            .px_1()
            .border_1()
            .border_color(if active {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(theme::canvas())
            .when(!custom, |field| field.text_color(theme::faint()))
            .when(custom, |field| {
                field
                    .cursor_text()
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        focus(view, set);
                        cx.notify();
                    }))
            })
            .child(shown)
    };
    let control = dialog
        .document
        .character_spacing_control
        .clone()
        .unwrap_or_else(|| "doNotCompress".into());
    let controls =
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(SPACING_CONTROLS.iter().enumerate().map(|(index, value)| {
                check(
                    ("spacing-control", index),
                    control_label(value),
                    control == *value,
                )
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    edit(view, |dialog| {
                        dialog.document.character_spacing_control = Some(value.to_string())
                    });
                    cx.notify();
                }))
            }));
    div()
        .w(px(500.0))
        .flex()
        .flex_col()
        .gap_3()
        .child(caption(i18n::text("chars.typography.paragraph")))
        .child(paragraph_options)
        .child(caption(i18n::text("chars.typography.document")))
        .child(caption(i18n::text("chars.kinsoku.level")))
        .child(levels)
        .child(caption(i18n::text("chars.kinsoku.line_start")))
        .child(set_box(KinsokuSet::LineStart, dialog.line_start_set(), cx))
        .child(caption(i18n::text("chars.kinsoku.line_end")))
        .child(set_box(KinsokuSet::LineEnd, dialog.line_end_set(), cx))
        .child(caption(i18n::text("chars.compress.title")))
        .child(controls)
        .into_any_element()
}
