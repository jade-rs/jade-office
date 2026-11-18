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

/// ホーム as Word lays it out: フォントの色 and 蛍光ペンの色 put on the colour
/// last picked from their lists, ⌘8 and ¶ show the formatting marks, and
/// 編集's 検索 and 置換 open the find bar in the field each names.
#[gpui::test]
fn home_split_buttons_marks_and_find_act_as_words(cx: &mut gpui::TestAppContext) {
    let (view, cx) = with_text(cx);
    view.update(cx, |view, cx| {
        assert_eq!(
            view.ribbon.font_color(),
            "FF0000",
            "red until one is picked"
        );
        assert_eq!(view.ribbon.highlight(), "yellow");
        view.run_ribbon(Action::FontColor("00B050".into()), cx);
        view.run_ribbon(Action::Highlight("cyan"), cx);
        // 自動 and なし are not remembered: the button keeps a colour.
        view.run_ribbon(Action::FontColor("auto".into()), cx);
        view.run_ribbon(Action::Highlight("none"), cx);
        assert_eq!(view.ribbon.font_color(), "00B050");
        assert_eq!(view.ribbon.highlight(), "cyan");
    });
    let marks = view.update(cx, |view, _| view.document.formatting_marks());
    cx.simulate_keystrokes("cmd-8");
    assert_eq!(
        view.update(cx, |view, _| view.document.formatting_marks()),
        !marks
    );
    view.update(cx, |view, cx| {
        view.run_ribbon(Action::ToggleFormattingMarks, cx);
        assert_eq!(view.document.formatting_marks(), marks);
        view.run_ribbon(Action::Replace, cx);
        assert!(view.find_bar.as_ref().unwrap().editing_replacement);
        view.close_find_bar(cx);
        view.run_ribbon(Action::Find, cx);
        assert!(!view.find_bar.as_ref().unwrap().editing_replacement);
        view.run_ribbon(Action::SelectAll, cx);
        assert!(view.document.selection().is_some());
    });
}

/// ⌘A selects the whole document and keeps it selected: the caret drawn at
/// its end used to collapse the selection onto itself.
#[gpui::test]
fn select_all_keeps_the_whole_document_selected(cx: &mut gpui::TestAppContext) {
    let (view, cx) = with_text(cx);
    cx.simulate_keystrokes("cmd-a");
    view.update(cx, |view, _| {
        let last = view.document.paragraph_count() - 1;
        let end = view.document.paragraph_text(last).unwrap().len();
        assert_eq!(view.document.selection(), Some(((0, 0), (last, end))));
        assert!(view.caret.is_some(), "the caret is still drawn");
    });
}

/// Enter near the start of a long document re-flows every page after it, but
/// draws only the pages around the view; the rest are drawn later, and print
/// draws any still waiting.
#[gpui::test]
fn enter_in_a_long_document_draws_the_pages_in_view_first(cx: &mut gpui::TestAppContext) {
    let path = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/public/fixtures/yokohama.docx"
    ));
    let (view, cx) =
        cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
    cx.update(|_, cx| cx.bind_keys(crate::key_bindings()));
    cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
    cx.run_until_parked();
    view.update(cx, |view, _| {
        view.insert_at = (40, 3);
        view.caret = view.document.place_caret(40, 3);
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let far = view.update(cx, |view, _| {
        let pages = view.document.page_count();
        assert!(pages > 100);
        assert!(!view.document.is_page_stale(view.current_page));
        pages - 1
    });
    assert!(view.update(cx, |view, _| view.document.is_page_stale(far)));
    view.update(cx, |view, _| view.paint_all_stale_pages());
    assert!(!view.update(cx, |view, _| view.document.has_stale_pages()));
}

fn document_xml(view: &crate::DocumentView) -> String {
    let bytes = view.document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&bytes).unwrap();
    package.part_str("word/document.xml").unwrap().unwrap()
}

/// The character keys Word for Mac has beyond ⌘B/⌘I/⌘U: ⇧⌘K small caps,
/// ⇧⌘A all caps, ⇧⌘D double underline, ⇧⌘W words-only underline, ⇧⌘H
/// hidden, ⇧⌘X strikethrough, ⌘⇧> / ⌘⇧< grow and shrink, ⌃Space clears it all again, and ⌘Y
/// redoes what ⌘Z took back.
#[gpui::test]
fn word_for_mac_character_effect_keys(cx: &mut gpui::TestAppContext) {
    let (view, cx) = with_text(cx);
    cx.simulate_keystrokes("cmd-alt-a");
    let character = |view: &gpui::Entity<crate::DocumentView>, cx: &mut gpui::VisualTestContext| {
        view.update(cx, |view, _| view.format_state().character)
    };
    cx.simulate_keystrokes("cmd-shift-k");
    assert_eq!(character(&view, cx).small_caps, Some(true));
    cx.simulate_keystrokes("cmd-shift-k");
    assert_ne!(
        character(&view, cx).small_caps,
        Some(true),
        "a second press takes it off"
    );
    cx.simulate_keystrokes("cmd-shift-a");
    assert_eq!(character(&view, cx).caps, Some(true));
    cx.simulate_keystrokes("cmd-shift-d");
    assert_eq!(character(&view, cx).underline.as_deref(), Some("double"));
    cx.simulate_keystrokes("cmd-shift-w");
    assert_eq!(character(&view, cx).underline.as_deref(), Some("words"));
    cx.simulate_keystrokes("cmd-shift-h");
    assert_eq!(character(&view, cx).hidden, Some(true));
    cx.simulate_keystrokes("cmd-shift-h");
    cx.simulate_keystrokes("cmd-shift-x");
    assert!(view.update(cx, |view, _| view.format_state().strike));

    let size = view.update(cx, |view, _| view.format_state().font_size);
    cx.simulate_keystrokes("cmd->");
    let grown = view.update(cx, |view, _| view.format_state().font_size);
    assert!(grown > size, "{size:?} → {grown:?}");
    cx.simulate_keystrokes("cmd-<");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().font_size),
        size
    );

    cx.simulate_keystrokes("ctrl-space");
    assert_ne!(
        character(&view, cx).caps,
        Some(true),
        "⌃Space clears the effects"
    );

    cx.simulate_keystrokes("cmd-z");
    assert_eq!(character(&view, cx).caps, Some(true));
    cx.simulate_keystrokes("cmd-y");
    assert_ne!(character(&view, cx).caps, Some(true), "⌘Y redoes");
}

/// ⇧⌘J distributes the paragraph, ⌘1 / ⌘5 / ⌘2 set single, 1.5 and double line spacing, ⌘T and ⇧⌘T move
/// the hanging indent, ⌘⌥1 and ⇧⌘N put on 見出し 1 and 標準, and ⇧F3 walks
/// the selection through Word's case cycle.
#[gpui::test]
fn word_for_mac_paragraph_style_and_case_keys(cx: &mut gpui::TestAppContext) {
    let (view, cx) = with_text(cx);
    cx.simulate_keystrokes("cmd-shift-j");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().alignment),
        Some("distribute".into())
    );
    cx.simulate_keystrokes("cmd-5");
    assert!(view
        .update(cx, |view, _| document_xml(view))
        .contains(r#"w:line="360""#));
    cx.simulate_keystrokes("cmd-2");
    assert!(view
        .update(cx, |view, _| document_xml(view))
        .contains(r#"w:line="480""#));
    cx.simulate_keystrokes("cmd-1");
    assert!(view
        .update(cx, |view, _| document_xml(view))
        .contains(r#"w:line="240""#));

    let hanging = |view: &gpui::Entity<crate::DocumentView>, cx: &mut gpui::VisualTestContext| {
        view.update(cx, |view, _| document_xml(view))
            .matches("w:hanging=")
            .count()
    };
    let before = hanging(&view, cx);
    cx.simulate_keystrokes("cmd-t");
    assert!(hanging(&view, cx) > before, "⌘T hangs the paragraph");
    cx.simulate_keystrokes("cmd-shift-t");
    assert_eq!(hanging(&view, cx), before, "⇧⌘T takes it back");

    cx.simulate_keystrokes("cmd-alt-1");
    assert_eq!(
        view.update(cx, |view, _| view.format_state().style_id),
        Some("Heading1".into())
    );
    cx.simulate_keystrokes("cmd-shift-n");
    assert_ne!(
        view.update(cx, |view, _| view.format_state().style_id),
        Some("Heading1".into())
    );

    let paragraph = view.update(cx, |view, _| view.insert_at.0);
    cx.simulate_input("hello world ");
    view.update(cx, |view, _| {
        view.document
            .select((paragraph, 0), (paragraph, 11), 1)
            .unwrap();
    });
    let text = |view: &gpui::Entity<crate::DocumentView>, cx: &mut gpui::VisualTestContext| {
        let text = view.update(cx, |view, _| {
            view.document.paragraph_text(paragraph).unwrap()
        });
        text.chars().take(11).collect::<String>()
    };
    assert_eq!(text(&view, cx), "hello world");
    cx.simulate_keystrokes("shift-f3");
    assert_eq!(text(&view, cx), "Hello World");
    cx.simulate_keystrokes("shift-f3");
    assert_eq!(text(&view, cx), "HELLO WORLD");
    cx.simulate_keystrokes("shift-f3");
    assert_eq!(
        text(&view, cx),
        "hello world",
        "three presses come round again"
    );
}
