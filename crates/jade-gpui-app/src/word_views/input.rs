//! What typing sets off: オートコレクト and 入力オートフォーマット after a
//! character or Enter (`jade_office_model::word::autocorrect`), each one
//! undo step after the typing, as in Word.

use gpui::{Context, SharedString};
use jade_office_model::word::autocorrect::{
    corrections, heading_from, list_start, on_enter, position_after, WordAutoCorrect,
};

use super::settings;
use crate::DocumentView;

/// After `typed` went in before the caret: the corrections it sets off.
pub fn after_typing(view: &mut DocumentView, typed: &str, cx: &mut Context<DocumentView>) {
    if typed.is_empty() || view.marked.is_some() || view.document.selection().is_some() {
        return;
    }
    let options = settings::autocorrect();
    let (paragraph, caret) = view.insert_at;
    let Some(text) = view.document.paragraph_text(paragraph) else {
        return;
    };
    if let Some(list) = list_start(&text, caret, &options) {
        if caret == text.len() || text[caret..].trim().is_empty() {
            match view.document.start_list(paragraph, &list) {
                Ok(()) => {
                    view.insert_at = (paragraph, 0);
                    view.caret = view.document.place_caret(paragraph, 0);
                    view.rebuild_after_edit();
                }
                Err(error) => view.table_status = Some(SharedString::from(error)),
            }
            return cx.notify();
        }
    }
    let fixes = corrections(&text, caret, &options);
    if fixes.is_empty() {
        return;
    }
    match view.document.apply_fixes(paragraph, &fixes, caret) {
        Ok(_) => {
            let moved = position_after((paragraph, caret), paragraph, &fixes);
            view.insert_at = moved;
            view.caret = view.document.place_caret(moved.0, moved.1);
            view.rebuild_after_edit();
        }
        Err(error) => view.table_status = Some(SharedString::from(error)),
    }
    cx.notify();
}

/// After Enter ended paragraph `ended`: corrections at its end, a border,
/// a 結語 or 以上, a heading.
pub fn after_enter(view: &mut DocumentView, ended: usize, cx: &mut Context<DocumentView>) {
    let options = settings::autocorrect();
    let Some(text) = view.document.paragraph_text(ended) else {
        return;
    };
    let mut changed = false;
    // The paragraph's last word, as if a space had ended it.
    let with_end = format!("{text}\n");
    let fixes: Vec<_> = corrections(&with_end, with_end.len(), &options)
        .into_iter()
        .filter(|fix| fix.end <= text.len())
        .collect();
    if !fixes.is_empty() && view.document.apply_fixes(ended, &fixes, text.len()).is_ok() {
        changed = true;
    }
    let text = view.document.paragraph_text(ended).unwrap_or_default();
    let following: String = (ended + 1..(ended + 40).min(view.document.paragraph_count()))
        .filter_map(|index| view.document.paragraph_text(index))
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(format) = on_enter(&text, &following, &options) {
        match view.document.format_on_enter(ended, &format) {
            Ok(()) => changed = true,
            Err(error) => view.table_status = Some(SharedString::from(error)),
        }
    } else if text.trim().is_empty() && ended > 0 {
        let previous = view.document.paragraph_text(ended - 1).unwrap_or_default();
        if heading_from(&previous, &options) && view.document.make_heading(ended - 1).is_ok() {
            changed = true;
        }
    }
    if changed {
        let caret = view.insert_at;
        view.caret = view.document.place_caret(caret.0, caret.1);
        view.rebuild_after_edit();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::super::settings;
    use jade_office_model::word::autocorrect::AutoCorrectOptions;

    /// Enter, as the key handler gets it (the test harness would also type
    /// a newline through the input handler, which the platform does not).
    pub(crate) fn press(
        view: &gpui::Entity<crate::DocumentView>,
        cx: &mut gpui::VisualTestContext,
        key: &str,
    ) {
        view.update(cx, |view, cx| {
            let event = gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse(key).unwrap(),
                is_held: false,
            };
            view.handle_key(&event, cx);
        });
    }

    fn open_at_start(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
        usize,
    ) {
        settings::set_autocorrect(AutoCorrectOptions::default());
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        // A paragraph of its own to type into.
        let paragraph = view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            let end = view.current_paragraph_text().len();
            view.insert_at = (paragraph, end);
            view.caret = view.document.place_caret(paragraph, end);
            paragraph
        });
        press(&view, cx, "enter");
        (view, cx, paragraph + 1)
    }

    /// teh corrects as the space goes in; ⌘Z takes the correction back
    /// and leaves what was typed.
    #[gpui::test]
    fn a_word_corrects_as_it_ends(cx: &mut gpui::TestAppContext) {
        let (view, cx, paragraph) = open_at_start(cx);
        cx.simulate_input("I saw teh ");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.document.paragraph_text(paragraph).unwrap(),
                "I saw the "
            );
            assert_eq!(view.insert_at, (paragraph, "I saw the ".len()));
        });
        cx.dispatch_action(crate::Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.document.paragraph_text(paragraph).unwrap(),
                "I saw teh "
            );
        });
    }

    /// "1. " starts a numbered list; "---" and Enter becomes a border.
    #[gpui::test]
    fn markers_and_dashes_format_as_typed(cx: &mut gpui::TestAppContext) {
        let (view, cx, paragraph) = open_at_start(cx);
        cx.simulate_input("1. ");
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), "");
            let numbered = view
                .document
                .word_document()
                .paragraph(paragraph)
                .and_then(|p| p.properties.as_ref())
                .and_then(|props| props.numbering.as_ref())
                .is_some();
            assert!(numbered, "the paragraph is in a list");
        });
        cx.simulate_input("item");
        press(&view, cx, "enter");
        press(&view, cx, "enter");
        let border_paragraph = view.read_with(cx, |view, _| view.insert_at.0);
        cx.simulate_input("---");
        press(&view, cx, "enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_text(border_paragraph).unwrap(), "");
            let borders = view
                .document
                .word_document()
                .paragraph(border_paragraph)
                .and_then(|p| p.properties.as_ref())
                .and_then(|props| props.borders.as_ref())
                .is_some();
            assert!(borders, "the dashes became a border");
        });
    }

    /// 拝啓 and Enter: 敬具 goes in, right-aligned, below the caret's line.
    #[gpui::test]
    fn an_opening_brings_its_closing(cx: &mut gpui::TestAppContext) {
        let (view, cx, paragraph) = open_at_start(cx);
        cx.simulate_input("拝啓");
        press(&view, cx, "enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.insert_at, (paragraph + 1, 0));
            assert_eq!(view.document.paragraph_text(paragraph + 2).unwrap(), "敬具");
        });
    }

    /// Switched off in the settings, nothing changes.
    #[gpui::test]
    fn the_settings_switch_it_off(cx: &mut gpui::TestAppContext) {
        let (view, cx, paragraph) = open_at_start(cx);
        settings::set_autocorrect(AutoCorrectOptions {
            replace_text: false,
            capitalize_sentences: false,
            ..AutoCorrectOptions::default()
        });
        cx.simulate_input("teh ");
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), "teh ");
        });
        settings::set_autocorrect(AutoCorrectOptions::default());
    }
}
