mod common;

use jade_docx_editor::word_document::ParagraphStory;
use jade_web::editing::{Motion, WordSession};
use jade_web::word::{NoteForm, NoteKind};

#[test]
fn paragraph_text_end_selects_content_without_the_paragraph_break() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "日本語");
    document.place_caret(paragraph, "日本語".len());
    document.paste("\n\nNext").unwrap();
    let before = document.export_docx().unwrap();
    document.place_caret(paragraph, "日".len());
    document.move_caret(Motion::ParagraphTextEnd, true).unwrap();
    assert_eq!(
        document.selection(),
        Some(((paragraph, "日".len()), (paragraph, "日本語".len())))
    );
    assert_eq!(document.selected_text().as_deref(), Some("本語"));
    document.insert_text("選択").unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).as_deref(),
        Some("日選択")
    );
    assert_eq!(document.paragraph_text(paragraph + 1).as_deref(), Some(""));
    document.undo().unwrap();
    assert_eq!(document.export_docx().unwrap(), before);
    document.place_caret(paragraph + 1, 0);
    document.move_caret(Motion::ParagraphTextEnd, true).unwrap();
    assert_eq!(document.caret(), Some((paragraph + 1, 0)));
    assert_eq!(document.selection(), None);
}

#[test]
fn physical_page_moves_reach_wrapped_line_starts_and_extend_selection() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, &"日本語 abcdefghij ".repeat(700));
    let before = document.export_docx().unwrap();
    document.place_caret(paragraph, 9);
    let initial = document.focus_rect().unwrap();
    let first_page = document.page_and_offset(initial.document_y).0;
    document.move_caret_page(true, true).unwrap();
    let target = document.caret().unwrap();
    assert_eq!(
        target.0, paragraph,
        "the same paragraph spans physical pages"
    );
    assert!(target.1 > 9);
    assert_eq!(document.selection(), Some(((paragraph, 9), target)));
    let rect = document.focus_rect().unwrap();
    assert_eq!(
        document
            .page_and_offset(rect.document_y + rect.height / 2.0)
            .0,
        first_page + 1
    );
    document.place_caret(target.0, target.1);
    document.move_caret(Motion::Left, false).unwrap();
    let previous = document.focus_rect().unwrap();
    assert_eq!(
        document
            .page_and_offset(previous.document_y + previous.height / 2.0)
            .0,
        first_page
    );
    document.move_caret_page(true, false).unwrap();
    document.move_caret_page(false, false).unwrap();
    let top = document.caret();
    let rect = document.focus_rect().unwrap();
    assert_eq!(document.page_and_offset(rect.document_y).0, first_page);
    document.move_caret_page(false, false).unwrap();
    assert_eq!(document.caret(), top);
    document.move_caret(Motion::DocumentEnd, false).unwrap();
    let end = document.caret();
    document.move_caret_page(true, false).unwrap();
    assert_eq!(document.caret(), end);
    assert_eq!(document.export_docx().unwrap(), before);
}

#[test]
fn paragraph_moves_cross_empty_paragraphs_and_extend_selection() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "日本語");
    document.place_caret(paragraph, "日本語".len());
    document.paste("\n\nNext").unwrap();
    let before = document.export_docx().unwrap();
    document.place_caret(paragraph, "日".len());
    document.move_caret(Motion::ParagraphStart, false).unwrap();
    assert_eq!(document.caret(), Some((paragraph, 0)));
    document.move_caret(Motion::ParagraphEnd, true).unwrap();
    assert_eq!(
        document.selection(),
        Some(((paragraph, 0), (paragraph + 1, 0)))
    );
    document.move_caret(Motion::ParagraphEnd, true).unwrap();
    assert_eq!(
        document.selection(),
        Some(((paragraph, 0), (paragraph + 2, 0)))
    );
    document.move_caret(Motion::ParagraphStart, true).unwrap();
    assert_eq!(
        document.selection(),
        Some(((paragraph, 0), (paragraph + 1, 0)))
    );
    document.move_caret(Motion::DocumentStart, false).unwrap();
    let first = document.caret();
    document.move_caret(Motion::ParagraphStart, false).unwrap();
    assert_eq!(document.caret(), first);
    document.move_caret(Motion::DocumentEnd, false).unwrap();
    let last = document.caret();
    document.move_caret(Motion::ParagraphEnd, false).unwrap();
    assert_eq!(document.caret(), last);
    assert_eq!(document.export_docx().unwrap(), before);
}

#[test]
fn paragraph_moves_stay_in_the_body_or_current_note() {
    for kind in [NoteKind::Footnote, NoteKind::Endnote] {
        let mut document = common::contract();
        let paragraph = common::typed(&mut document, "Body");
        document.place_caret(paragraph, 0);
        let mut form = NoteForm::new(&document, kind);
        form.text.insert("Note");
        form.submit(&mut document).unwrap();
        let story = document.word_document().paragraph_story(paragraph).unwrap();
        let ParagraphStory::Body { index } = story else {
            panic!("body expected")
        };
        let next = document
            .word_document()
            .paragraph_story_surface(&ParagraphStory::Body { index: index + 1 })
            .unwrap();
        document.place_caret(paragraph, 0);
        document.move_caret(Motion::ParagraphEnd, false).unwrap();
        assert_eq!(document.caret(), Some((next, 0)));
        document.move_caret(Motion::ParagraphStart, false).unwrap();
        assert_eq!(document.caret(), Some((paragraph, 0)));
        let note = (0..document.paragraph_count())
            .find(|&surface| {
                matches!(
                    document.word_document().paragraph_story(surface),
                    Some(ParagraphStory::Footnote { .. } | ParagraphStory::Endnote { .. })
                )
            })
            .unwrap();
        assert_vertical_story_boundaries(&mut document, note);
        assert_vertical_story_wrapping(&mut document, note);
        let end = document.paragraph_text(note).unwrap().len();
        document.place_caret(note, 0);
        document.move_caret(Motion::ParagraphTextEnd, true).unwrap();
        assert_eq!(document.selection(), Some(((note, 0), (note, end))));
        document.place_caret(note, 0);
        document.move_caret(Motion::ParagraphStart, false).unwrap();
        assert_eq!(document.caret(), Some((note, 0)));
        document.move_caret(Motion::ParagraphEnd, false).unwrap();
        assert_eq!(document.caret(), Some((note, end)));
        document.move_caret_to_y(-10000.0, false).unwrap();
        assert_eq!(document.caret().unwrap().0, note);
        document.move_caret_to_y(10000.0, true).unwrap();
        assert_eq!(document.caret().unwrap().0, note);
        document.move_caret(Motion::ParagraphEnd, false).unwrap();
        assert_eq!(document.caret(), Some((note, end)));
    }
}

#[test]
fn paragraph_moves_skip_text_boxes_and_stay_inside_them() {
    use jade_docx_editor::document::TextBoxShape;
    use jade_web::word::shapes::WordShapes;

    let mut document = common::contract();
    let host = common::typed(&mut document, "Body");
    document.place_caret(host, 0);
    document
        .insert_shape(TextBoxShape::Rectangle, None, true)
        .unwrap();
    let Some(ParagraphStory::Body { index }) = document.word_document().paragraph_story(host)
    else {
        panic!("body expected");
    };
    let next = document
        .word_document()
        .paragraph_story_surface(&ParagraphStory::Body { index: index + 1 })
        .unwrap();
    document.place_caret(host, 0);
    document.move_caret(Motion::ParagraphEnd, false).unwrap();
    assert_eq!(document.caret(), Some((next, 0)));
    let text_box = (0..document.paragraph_count())
        .find(|&surface| {
            matches!(
                document.word_document().paragraph_story(surface),
                Some(ParagraphStory::TextBox { .. })
            )
        })
        .unwrap();
    document.place_caret(text_box, 0);
    document.insert_text("日本語").unwrap();
    assert_vertical_story_boundaries(&mut document, text_box);
    assert_vertical_story_wrapping(&mut document, text_box);
    document.move_caret(Motion::ParagraphEnd, false).unwrap();
    assert_eq!(document.caret(), Some((text_box, "日本語".len())));
    document.move_caret(Motion::ParagraphStart, false).unwrap();
    document.move_caret(Motion::ParagraphStart, false).unwrap();
    assert_eq!(document.caret(), Some((text_box, 0)));
    document.move_caret_to_y(10000.0, false).unwrap();
    assert_eq!(document.caret().unwrap().0, text_box);
    document.move_caret_to_y(-10000.0, true).unwrap();
    assert_eq!(document.caret().unwrap().0, text_box);
}

#[test]
fn screen_targets_retain_the_column_and_selection_anchor() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "abcdefghij");
    document.place_caret(paragraph, 10);
    document.paste("\nX\nabcdefghij").unwrap();
    let before = document.export_docx().unwrap();
    let mut lines = Vec::new();
    for index in paragraph..paragraph + 3 {
        document.place_caret(index, 0);
        let rect = document.caret_rect().unwrap();
        lines.push(rect.document_y + rect.height / 2.0);
    }
    document.place_caret(paragraph, 5);
    document.move_caret_to_y(lines[1], false).unwrap();
    assert_eq!(document.caret(), Some((paragraph + 1, 1)));
    document.move_caret_to_y(lines[2], false).unwrap();
    assert_eq!(document.caret(), Some((paragraph + 2, 5)));
    document.move_caret_to_y(lines[0], true).unwrap();
    assert_eq!(
        document.selection(),
        Some(((paragraph + 2, 5), (paragraph, 5)))
    );
    assert_eq!(document.export_docx().unwrap(), before);
    assert!(document.move_caret_to_y(f64::NAN, false).is_err());
}

fn assert_vertical_story_boundaries(
    document: &mut jade_web::editing::EditableDocument,
    paragraph: usize,
) {
    let before = document.export_docx().unwrap();
    let end = document.paragraph_text(paragraph).unwrap().len();
    for extend in [false, true] {
        for (motion, offset) in [(Motion::Up, 0), (Motion::Down, end)] {
            document.place_caret(paragraph, offset);
            document.move_caret(motion, extend).unwrap();
            let focus = document
                .selection()
                .map(|range| range.1)
                .or_else(|| document.caret())
                .unwrap();
            assert_eq!(
                focus,
                (paragraph, offset),
                "vertical movement escaped the story"
            );
        }
    }
    assert_eq!(document.export_docx().unwrap(), before);
}

fn assert_vertical_story_wrapping(
    document: &mut jade_web::editing::EditableDocument,
    paragraph: usize,
) {
    let before = document.export_docx().unwrap();
    document.place_caret(paragraph, 0);
    document.insert_text(&"日本語の文章 ".repeat(80)).unwrap();
    document.place_caret(paragraph, 0);
    document.move_caret(Motion::Down, false).unwrap();
    let second_line = document.caret().unwrap();
    assert_eq!(second_line.0, paragraph);
    assert!(second_line.1 > 0, "Down must advance within the story");
    document.move_caret(Motion::Up, true).unwrap();
    assert_eq!(document.selection(), Some((second_line, (paragraph, 0))));
    document.undo().unwrap();
    assert_eq!(document.export_docx().unwrap(), before);
}
