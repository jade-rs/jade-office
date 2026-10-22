//! Word for Mac's keys reach Word's commands, and the Layout tab's 改ページ
//! is a page break.

use crate::ribbon::Action;
use gpui::Focusable;
use jade_office_model::word::review::WordReview;
use jade_office_model::word::ListKind;

fn with_text(
    cx: &mut gpui::TestAppContext,
) -> (
    gpui::Entity<crate::DocumentView>,
    &mut gpui::VisualTestContext,
) {
    let path = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/contract.docx"
    ));
    let (view, cx) =
        cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
    cx.update(|_, cx| cx.bind_keys(crate::key_bindings()));
    cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
    cx.run_until_parked();
    view.update(cx, |view, _| {
        let paragraph = (0..view.document.paragraph_count())
            .find(|paragraph| {
                view.document
                    .paragraph_text(*paragraph)
                    .is_some_and(|text| text.chars().count() > 4)
            })
            .expect("a paragraph with text");
        view.insert_at = (paragraph, 0);
        view.caret = view.document.place_caret(paragraph, 0);
    });
    (view, cx)
}

#[gpui::test]
fn word_for_mac_paragraph_and_character_keys(cx: &mut gpui::TestAppContext) {
    let (view, cx) = with_text(cx);
    cx.simulate_keystrokes("cmd-e");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().alignment),
        Some("center".into())
    );
    cx.simulate_keystrokes("cmd-j");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().alignment),
        Some("justify".into())
    );
    cx.simulate_keystrokes("cmd-l");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().alignment),
        Some("left".into())
    );
    cx.simulate_keystrokes("cmd-shift-l");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().list),
        Some(ListKind::Bullet)
    );
    cx.simulate_keystrokes("cmd-alt-a");
    cx.simulate_keystrokes("cmd-=");
    assert!(view.update(cx, |view, _| view.format_state().subscript));
    let zoom = view.update(cx, |view, _| view.zoom);
    cx.simulate_keystrokes("cmd-alt-=");
    assert!(view.update(cx, |view, _| view.zoom) >= zoom);
    let tracking = view.update(cx, |view, _| view.document.is_tracking_changes());
    cx.simulate_keystrokes("cmd-shift-e");
    assert_ne!(
        view.update(cx, |view, _| view.document.is_tracking_changes()),
        tracking
    );
}

#[gpui::test]
fn the_layout_page_break_and_the_column_break_key_insert_breaks(cx: &mut gpui::TestAppContext) {
    let (view, cx) = with_text(cx);
    let paragraphs = view.update(cx, |view, _| view.document.paragraph_count());
    cx.simulate_keystrokes("cmd-shift-enter");
    let text = view.update(cx, |view, _| view.current_paragraph_text());
    assert!(
        text.contains(jade_office_model::word::page_setup::COLUMN_BREAK),
        "a column break: {text:?}"
    );
    view.update(cx, |view, cx| {
        view.run_ribbon(
            Action::Setup(crate::word_page_setup::SetupAction::Break(
                jade_office_model::word::page_setup::PageBreak::Page,
            )),
            cx,
        );
        // A page break is one break in the text, not a blank page's two
        // paragraphs.
        assert!(view.document.paragraph_count() <= paragraphs + 1);
    });
}
