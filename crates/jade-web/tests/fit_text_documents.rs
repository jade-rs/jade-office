//! ホーム › 均等割り付け on selected text in the browser build — the commands
//! the native app runs too: 文字の均等割り付け spreads two characters over
//! four 字 (drawn wider, saved as `w:fitText`, read back), 解除 puts them
//! back, and a bare caret leaves the button to the paragraph.

mod common;
use common::{contract, open_bytes};
use jade_office_model::word::fields::{SelectionRange, TextPosition};
use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::fit_text::WordFitText;

fn paragraph_text(
    document: &jade_docx_editor::word_document::WordDocument,
    index: usize,
) -> Option<String> {
    let paragraph = document.paragraph(index)?;
    Some(paragraph.runs.iter().map(|run| run.text.as_str()).collect())
}

fn page_pixels(document: &EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

fn alike(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
            < 500
}

fn select(document: &mut EditableDocument, paragraph: usize, from: usize, to: usize) {
    document.set_selection_range(Some(SelectionRange::new(
        TextPosition::new(paragraph, from),
        TextPosition::new(paragraph, to),
    )));
}

#[test]
fn selected_text_is_spread_saved_and_put_back() {
    let mut document = contract();
    let paragraph = (0..document.word_document().paragraph_count())
        .find(|&index| {
            document.word_document().is_body_paragraph_surface(index)
                && paragraph_text(document.word_document(), index)
                    .is_some_and(|text| text.trim().chars().count() >= 6)
        })
        .expect("a paragraph with text");
    let text = paragraph_text(document.word_document(), paragraph).unwrap();
    let two: usize = text.chars().take(2).map(char::len_utf8).sum();
    let plain = page_pixels(&document);

    // A bare caret: no dialog, the button is the paragraph's.
    select(&mut document, paragraph, 0, 0);
    assert!(document.fit_text_dialog().is_none());

    select(&mut document, paragraph, 0, two);
    let mut dialog = document.fit_text_dialog().expect("the dialog");
    assert!(!dialog.fitted);
    assert!(dialog.current > 0.0 && dialog.current <= 2.0);
    dialog.new_width = "4".into();
    document.apply_fit_text(&dialog).unwrap();
    let spread = page_pixels(&document);
    assert!(!alike(&spread, &plain), "the characters are drawn spread");

    // Read back: four 字 at the run's size.
    select(&mut document, paragraph, 0, two);
    let back = document.fit_text_dialog().unwrap();
    assert!(back.fitted);
    assert!((back.current - 4.0).abs() < 0.01, "{}", back.current);

    // Saved and opened again: drawn the same, the same width.
    let bytes = document.export_docx().unwrap();
    let mut reopened = open_bytes(&bytes);
    assert!(alike(&page_pixels(&reopened), &spread));
    select(&mut reopened, paragraph, 0, two);
    assert!(reopened.fit_text_dialog().unwrap().fitted);

    // 解除 puts the text back.
    document.remove_fit_text(&back).unwrap();
    assert!(
        alike(&page_pixels(&document), &plain),
        "解除 restores the line"
    );
    select(&mut document, paragraph, 0, two);
    assert!(!document.fit_text_dialog().unwrap().fitted);
    document.undo().unwrap();
    assert!(alike(&page_pixels(&document), &spread));
}
