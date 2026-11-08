//! Copy and paste that keep the formatting, on the browser build's Word
//! document, as the native app does it too: bold, a size and a heading's
//! style come along; the paste is one undo step.

mod common;
use common::{contract, typed};
use jade_web::editing::{CharacterFormat, WordFormatting};
use jade_web::word::clipboard::WordClipboard;

#[test]
fn a_paste_keeps_the_runs_formatting() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Bold plain ");
    document.select((paragraph, 0), (paragraph, 4)).unwrap();
    document.toggle_format(CharacterFormat::Bold).unwrap();
    document.set_font_size(20.0).unwrap();
    assert!(document.format_state().bold);

    document
        .select((paragraph, 0), (paragraph, "Bold plain".len()))
        .unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.plain_text(), "Bold plain");
    assert_eq!(document.selected_text().as_deref(), Some("Bold plain"));

    // At the paragraph's end, where plain text would be plain.
    let end = document.paragraph_text(paragraph).unwrap().len();
    document.place_caret(paragraph, end);
    let after = document.paste_rich(&clip).unwrap();
    assert_eq!(after, (paragraph, end + "Bold plain".len()));
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(text.ends_with("Bold plain"), "{text}");
    let bold = document.run_style_at((paragraph, end + 1)).unwrap();
    assert_eq!(bold.bold, Some(true));
    assert_eq!(bold.font_size, Some(20.0));
    let plain = document.run_style_at((paragraph, end + 6)).unwrap();
    assert_ne!(plain.bold, Some(true));

    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap().len(), end);
}

#[test]
fn a_paragraph_copied_with_its_mark_keeps_its_style() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Title ");
    document.place_caret(paragraph, 0);
    document.set_paragraph_style(Some("Heading1")).unwrap();
    document.place_caret(paragraph, 1);
    let heading = document.format_state().style_id;
    assert!(heading.is_some());
    let below = document.paragraph_text(paragraph + 1).unwrap();
    // The whole heading, its mark included.
    document.select((paragraph, 0), (paragraph + 1, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.paragraphs.len(), 2);

    let count = document.paragraph_count();
    document.place_caret(paragraph + 1, 0);
    document.paste_rich(&clip).unwrap();
    assert_eq!(document.paragraph_count(), count + 1);
    document.place_caret(paragraph + 1, 1);
    assert_eq!(document.format_state().style_id, heading);
    // What was below goes on in its own paragraph, as it was.
    assert_eq!(document.paragraph_text(paragraph + 2).unwrap(), below);
}

/// 形式を選択して貼り付け → HTML 形式 with a copy from another application:
/// its paragraphs and formatting go in, one undo step.
#[test]
fn html_from_another_application_pastes_with_its_formatting() {
    use jade_web::word::clipboard::{PasteMode, RichClip};
    let mut document = contract();
    let paragraph = typed(&mut document, "Here: ");
    let end = "Here: ".len();
    document.place_caret(paragraph, end);
    let count = document.paragraph_count();
    let clip = RichClip::from_html(
        "<p><b>Bold</b> and <span style=\"color:#C00000\">red</span></p><p>Second</p>",
    );
    document.paste_as(&clip, PasteMode::KeepSource).unwrap();
    assert_eq!(document.paragraph_count(), count + 1);
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(text.starts_with("Here: Bold and red"), "{text}");
    let bold = document.run_style_at((paragraph, end + 1)).unwrap();
    assert_eq!(bold.bold, Some(true));
    let red = document
        .run_style_at((paragraph, end + "Bold and r".len()))
        .unwrap();
    assert_eq!(
        red.color
            .as_deref()
            .map(|color| color.trim_start_matches('#').to_ascii_uppercase())
            .as_deref(),
        Some("C00000")
    );
    assert!(document
        .paragraph_text(paragraph + 1)
        .unwrap()
        .starts_with("Second"));
    // Text only drops it.
    document.undo().unwrap();
    assert_eq!(document.paragraph_count(), count);
    document.place_caret(paragraph, end);
    document.paste_as(&clip, PasteMode::TextOnly).unwrap();
    let plain = document.run_style_at((paragraph, end + 1)).unwrap();
    assert_ne!(plain.bold, Some(true));
}

#[test]
fn html_soft_breaks_paste_without_splitting_the_document_paragraph() {
    use jade_web::word::clipboard::{PasteMode, RichClip};
    let mut document = contract();
    let paragraph = typed(&mut document, "Here: ");
    let before = document.paragraph_text(paragraph).unwrap();
    let count = document.paragraph_count();
    let clip = RichClip::from_html("<p>First<br><b>Second</b><br><br>Last</p>");
    document.place_caret(paragraph, 6);
    let end = document.paste_as(&clip, PasteMode::KeepSource).unwrap();
    assert_eq!(end, (paragraph, 6 + "First\nSecond\n\nLast".len()));
    assert_eq!(document.paragraph_count(), count);
    let after = document.paragraph_text(paragraph).unwrap();
    assert!(
        after.starts_with("Here: First\nSecond\n\nLast"),
        "{after:?}"
    );
    assert_eq!(
        document
            .run_style_at((paragraph, 6 + "First\n".len()))
            .unwrap()
            .bold,
        Some(true)
    );
    document.undo().unwrap();
    assert_eq!(document.paragraph_count(), count);
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
    document.redo().unwrap();
    assert_eq!(document.paragraph_count(), count);
    assert_eq!(document.paragraph_text(paragraph).unwrap(), after);
    document.select((paragraph, 6), end).unwrap();
    let copied = document.copy_rich().unwrap();
    let restored = RichClip::from_html(&copied.to_html());
    assert_eq!(restored.plain_text(), "First\nSecond\n\nLast");
    assert_eq!(restored.paragraphs.len(), 1);
}

#[test]
fn html_unicode_spaces_survive_paste_copy_and_history() {
    use jade_web::word::clipboard::{PasteMode, RichClip};
    let mut document = contract();
    let paragraph = typed(&mut document, "Here: ");
    let before = document.paragraph_text(paragraph).unwrap();
    let text = "10\u{202f}000\u{a0}円　　end\u{2009}";
    let clip = RichClip::from_html("<p><b>10&#x202f;000&nbsp;円</b>　　end&#x2009;</p>");
    document.place_caret(paragraph, 6);
    let end = document.paste_as(&clip, PasteMode::KeepSource).unwrap();
    assert_eq!(end, (paragraph, 6 + text.len()));
    let after = document.paragraph_text(paragraph).unwrap();
    assert!(after.starts_with(&format!("Here: {text}")), "{after:?}");
    assert_eq!(
        document.run_style_at((paragraph, 6)).unwrap().bold,
        Some(true)
    );
    document.select((paragraph, 6), end).unwrap();
    let copied = document.copy_rich().unwrap();
    assert_eq!(copied.plain_text(), text);
    assert_eq!(RichClip::from_html(&copied.to_html()).plain_text(), text);
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
    document.redo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), after);
}

#[test]
fn html_attribute_values_do_not_leak_into_pasted_text_or_formatting() {
    use jade_web::word::clipboard::{PasteMode, RichClip};
    let mut document = contract();
    let paragraph = typed(&mut document, "Here: ");
    let before = document.paragraph_text(paragraph).unwrap();
    let clip = RichClip::from_html(
        "<p title=\"a > b style='font-weight:bold'\">plain<span title='x > y' style='color:#123456'>色</span></p>",
    );
    document.place_caret(paragraph, 6);
    let end = document.paste_as(&clip, PasteMode::KeepSource).unwrap();
    assert_eq!(end, (paragraph, 6 + "plain色".len()));
    let after = document.paragraph_text(paragraph).unwrap();
    assert!(after.starts_with("Here: plain色"), "{after:?}");
    assert_ne!(
        document.run_style_at((paragraph, 6)).unwrap().bold,
        Some(true)
    );
    assert_eq!(
        document
            .run_style_at((paragraph, 11))
            .unwrap()
            .color
            .as_deref(),
        Some("#123456")
    );
    document.select((paragraph, 6), end).unwrap();
    assert_eq!(document.copy_rich().unwrap().plain_text(), "plain色");
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
    document.redo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), after);
}
