//! Word documents are edited in the browser build the way the native app
//! edits them: a click places the caret, typing inserts at it, Backspace and
//! Delete remove characters (not bytes), Enter splits the paragraph, undo
//! and redo step through it, the page repaints to show it, and a save
//! carries every edit into the .docx.

use std::path::PathBuf;

use jade_web::editing::{EditableDocument, Motion};

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn open(name: &str) -> EditableDocument {
    open_bytes(&std::fs::read(core().join(name)).unwrap())
}

fn open_bytes(bytes: &[u8]) -> EditableDocument {
    let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    EditableDocument::open(bytes, 1.0, fonts, measurer).unwrap()
}

/// The first paragraph with text on the first page, and its text.
fn first_text_paragraph(document: &EditableDocument) -> (usize, String) {
    (0..document.paragraph_count())
        .find_map(|index| {
            document
                .paragraph_text(index)
                .filter(|text| !text.trim().is_empty())
                .map(|text| (index, text))
        })
        .expect("a paragraph with text")
}

fn page_rgba(document: &EditableDocument, index: usize) -> Vec<u8> {
    document.page(index).unwrap().rgba_premultiplied()
}

#[test]
fn typing_deleting_and_undo_edit_the_text_and_repaint_the_page() {
    let mut document = open("contract.docx");
    let pages = document.page_count();
    assert!(pages >= 1);
    let (paragraph, before) = first_text_paragraph(&document);
    let page_before = page_rgba(&document, 0);

    assert!(document.place_caret(paragraph, 0).is_some());
    document.insert_text("★テスト").unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).unwrap(),
        format!("★テスト{before}")
    );
    assert_eq!(document.caret(), Some((paragraph, "★テスト".len())));
    assert!(document.is_dirty());
    assert_ne!(
        page_rgba(&document, 0),
        page_before,
        "the page did not repaint"
    );

    // Backspace removes a character — three bytes of ト — not a byte.
    document.delete(true).unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).unwrap(),
        format!("★テス{before}")
    );

    document.undo().unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).unwrap(),
        format!("★テスト{before}")
    );
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
    assert_eq!(
        page_rgba(&document, 0),
        page_before,
        "undo did not restore the page"
    );
    document.redo().unwrap();
    assert!(document.paragraph_text(paragraph).unwrap().starts_with('★'));
    assert_eq!(document.page_count(), pages);
}

#[test]
fn enter_splits_the_paragraph_and_the_caret_follows() {
    let mut document = open("contract.docx");
    let (paragraph, before) = first_text_paragraph(&document);
    let count = document.paragraph_count();
    let split = before.char_indices().nth(2).map(|(at, _)| at).unwrap();
    document.place_caret(paragraph, split).unwrap();
    document.insert_paragraph_break().unwrap();
    assert_eq!(document.paragraph_count(), count + 1);
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before[..split]);
    assert_eq!(
        document.paragraph_text(paragraph + 1).unwrap(),
        before[split..]
    );
    assert_eq!(document.caret(), Some((paragraph + 1, 0)));
}

#[test]
fn a_click_lands_where_the_caret_is_drawn() {
    let mut document = open("contract.docx");
    let mut checked = 0;
    for paragraph in 0..document.paragraph_count() {
        let text = document.paragraph_text(paragraph).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        // Mid-paragraph, so wrapped lines are covered as well as first ones.
        let middle = text.chars().count() / 2;
        let offset = text.char_indices().nth(middle).map_or(0, |(at, _)| at);
        let caret = document.place_caret(paragraph, offset).unwrap();
        let (page, y_on_page) = document.page_and_offset(caret.document_y);
        if page > 0 {
            break;
        }
        assert!(y_on_page > 0.0 && y_on_page < document.page_size().1);
        // Just right of the caret, anywhere from its top to its bottom.
        for fraction in [0.1, 0.5, 0.9] {
            let hit =
                document.position_at(caret.x + 1.0, caret.document_y + caret.height * fraction);
            assert_eq!(
                hit,
                Some((paragraph, offset)),
                "a click {fraction} down the caret in paragraph {paragraph}"
            );
        }
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} paragraphs on the first page");
}

#[test]
fn a_save_carries_the_edits() {
    let mut document = open("contract.docx");
    let (paragraph, _) = first_text_paragraph(&document);
    document.place_caret(paragraph, 0).unwrap();
    document.insert_text("保存").unwrap();
    let saved = document.export_docx().unwrap();
    assert!(!document.is_dirty());
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains("保存"));
    let reopened = {
        let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
        let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
        let fonts = jade_web::backend(regular.clone(), None).unwrap().fonts();
        let measurer = jade_web::document::measurer(&regular, None).unwrap();
        EditableDocument::open(&saved, 1.0, fonts, measurer).unwrap()
    };
    assert!(reopened
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("保存"));
}

#[test]
fn a_composition_replaces_itself_until_committed() {
    let mut document = open("contract.docx");
    let (paragraph, before) = first_text_paragraph(&document);
    document.place_caret(paragraph, 0).unwrap();
    // An IME shows か, then かな, then commits 仮名.
    document.set_composition("か").unwrap();
    document.set_composition("かな").unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).unwrap(),
        format!("かな{before}")
    );
    document.commit_composition("仮名").unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).unwrap(),
        format!("仮名{before}")
    );
    assert_eq!(document.caret(), Some((paragraph, "仮名".len())));
    // One undo takes the committed word back.
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
}

/// The paragraph after `paragraph` that has text.
fn next_text_paragraph(document: &EditableDocument, paragraph: usize) -> (usize, String) {
    ((paragraph + 1)..document.paragraph_count())
        .find_map(|index| {
            document
                .paragraph_text(index)
                .filter(|text| !text.trim().is_empty())
                .map(|text| (index, text))
        })
        .expect("another paragraph with text")
}

fn char_offset(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(at, _)| at)
}

#[test]
fn a_selection_is_highlighted_copied_and_replaced_by_typing() {
    let mut document = open("contract.docx");
    let (paragraph, before) = first_text_paragraph(&document);
    let page_before = page_rgba(&document, 0);
    let three = char_offset(&before, 3);
    document.select((paragraph, 0), (paragraph, three)).unwrap();
    assert_eq!(
        document.selection(),
        Some(((paragraph, 0), (paragraph, three)))
    );
    assert_eq!(document.selected_text().unwrap(), before[..three]);
    // Highlighted over the page, which is not repainted.
    assert_eq!(
        page_rgba(&document, 0),
        page_before,
        "the page was repainted"
    );
    let rects = document.selection_rects();
    assert_eq!(rects.len(), 1, "{rects:?}");
    assert_eq!(rects[0].page, 0);
    assert!(rects[0].width > 10.0, "{rects:?}");

    // Typing replaces the selection, in one undoable step.
    document.insert_text("X").unwrap();
    assert_eq!(
        document.paragraph_text(paragraph).unwrap(),
        format!("X{}", &before[three..])
    );
    assert_eq!(document.selection(), None);
    assert_eq!(document.caret(), Some((paragraph, 1)));
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
}

#[test]
fn placing_the_caret_clears_the_highlight() {
    let mut document = open("contract.docx");
    let (paragraph, before) = first_text_paragraph(&document);
    document.place_caret(paragraph, 0).unwrap();
    let page_before = page_rgba(&document, 0);
    document
        .select((paragraph, 0), (paragraph, char_offset(&before, 4)))
        .unwrap();
    document.place_caret(paragraph, 0).unwrap();
    assert_eq!(document.selection(), None);
    assert_eq!(
        page_rgba(&document, 0),
        page_before,
        "highlight left behind"
    );
}

#[test]
fn a_selection_across_paragraphs_deletes_as_one_edit() {
    let mut document = open("contract.docx");
    let (first, first_text) = first_text_paragraph(&document);
    let (second, second_text) = next_text_paragraph(&document, first);
    let count = document.paragraph_count();
    let (from, to) = (char_offset(&first_text, 2), char_offset(&second_text, 2));
    // Dragged upwards: the focus comes before the anchor.
    document.select((second, to), (first, from)).unwrap();
    let copied = document.selected_text().unwrap();
    assert!(copied.starts_with(&first_text[from..]));
    assert!(copied.ends_with(&second_text[..to]));
    assert_eq!(copied.matches('\n').count(), second - first);

    document.delete(true).unwrap();
    assert_eq!(document.paragraph_count(), count - (second - first));
    assert_eq!(
        document.paragraph_text(first).unwrap(),
        format!("{}{}", &first_text[..from], &second_text[to..])
    );
    assert_eq!(document.caret(), Some((first, from)));
    document.undo().unwrap();
    assert_eq!(document.paragraph_count(), count);
    assert_eq!(document.paragraph_text(first).unwrap(), first_text);
    assert_eq!(document.paragraph_text(second).unwrap(), second_text);
}

#[test]
fn arrows_home_and_end_move_the_caret_and_shift_extends_the_selection() {
    let mut document = open("contract.docx");
    let (first, first_text) = first_text_paragraph(&document);
    let (second, _) = next_text_paragraph(&document, first);
    // The last line of the first paragraph, so Down leaves it.
    document.place_caret(first, first_text.len()).unwrap();
    let last_line = document.caret_rect().unwrap().document_y;
    document.move_caret(Motion::LineStart, false).unwrap();
    let line_start = document.caret().unwrap();
    assert_eq!(line_start.0, first);
    assert!(line_start.1 < first_text.len());
    // Drawn at the start of that line, not the end of the one above.
    assert_eq!(document.caret_rect().unwrap().document_y, last_line);
    document.move_caret(Motion::LineEnd, false).unwrap();
    assert_eq!(document.caret(), Some((first, first_text.len())));

    // Home again, then down a line and back up to the same place.
    document.move_caret(Motion::LineStart, false).unwrap();
    let top = document.caret_rect().unwrap().document_y;
    document.move_caret(Motion::Down, false).unwrap();
    assert_eq!(document.caret().unwrap().0, second);
    assert!(document.caret_rect().unwrap().document_y > top);
    document.move_caret(Motion::Up, false).unwrap();
    assert_eq!(document.caret(), Some((first, line_start.1)));

    document.move_caret(Motion::Right, true).unwrap();
    document.move_caret(Motion::Right, true).unwrap();
    let two = char_offset(&first_text[line_start.1..], 2);
    assert_eq!(
        document.selection(),
        Some(((first, line_start.1), (first, line_start.1 + two)))
    );
    // An arrow without Shift collapses the selection.
    document.move_caret(Motion::Left, false).unwrap();
    assert_eq!(document.selection(), None);
}

#[test]
fn select_all_takes_every_paragraph() {
    let mut document = open("contract.docx");
    document.select_all().unwrap();
    let copied = document.selected_text().unwrap();
    assert_eq!(copied.matches('\n').count(), document.paragraph_count() - 1);
    let (_, first) = first_text_paragraph(&document);
    assert!(copied.contains(&first));
}

#[test]
fn pasting_lines_makes_paragraphs_and_undoes_in_one_step() {
    let mut document = open("contract.docx");
    let (paragraph, before) = first_text_paragraph(&document);
    let count = document.paragraph_count();
    let two = char_offset(&before, 2);
    document.select((paragraph, 0), (paragraph, two)).unwrap();
    document.paste("一行目\n二行目").unwrap();
    assert_eq!(document.paragraph_count(), count + 1);
    assert_eq!(document.paragraph_text(paragraph).unwrap(), "一行目");
    assert_eq!(
        document.paragraph_text(paragraph + 1).unwrap(),
        format!("二行目{}", &before[two..])
    );
    assert_eq!(document.caret(), Some((paragraph + 1, "二行目".len())));
    document.undo().unwrap();
    assert_eq!(document.paragraph_count(), count);
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
}

#[test]
fn a_selection_repaints_nothing_and_is_highlighted_on_its_page() {
    let mut document = open("contract.docx");
    assert!(document.page_count() >= 2);
    let last = document.paragraph_count() - 1;
    let end = document.paragraph_text(last).unwrap_or_default().len();
    let start = document.place_caret(last, 0).unwrap();
    let (page, top) = document.page_and_offset(start.document_y);
    assert!(page > 0);
    let _ = document.take_repainted_pages();
    document.select((last, end), (last, 0)).unwrap();
    assert_eq!(document.take_repainted_pages(), Vec::<usize>::new());
    let rects = document.selection_rects();
    assert!(!rects.is_empty());
    for rect in &rects {
        // In the page's own px, on the line the paragraph starts on or below.
        assert_eq!(rect.page, page, "{rect:?}");
        assert!(rect.y + rect.height > top - 1.0, "{rect:?} above {top}");
        assert!(rect.y < document.page_size().1, "{rect:?}");
    }
    // The focus is where a caret at it would be drawn.
    assert_eq!(document.focus_rect(), Some(start));
    document.place_caret(last, 0);
    assert!(document.selection_rects().is_empty());
}

#[test]
fn a_double_click_selects_a_word_and_a_triple_click_the_paragraph() {
    let mut document = open("contract.docx");
    let (paragraph, text) = first_text_paragraph(&document);
    let inside = char_offset(&text, 1);
    document.select_word((paragraph, inside)).unwrap();
    let ((_, from), (_, to)) = document.selection().expect("a word selected");
    assert!(
        from <= inside && inside < to,
        "{from}..{to} around {inside}"
    );
    assert!(text[from..to].chars().all(char::is_alphanumeric));
    document.select_paragraph(paragraph).unwrap();
    assert_eq!(document.selected_text().unwrap(), text);
}

/// A keystroke repaints only its paragraph's band of the page and keeps every
/// other pixel; the page it leaves must be the page a whole repaint draws.
/// Checked against the same text opened afresh, at 1x and at a Retina 2x. The
/// band leaves the anti-aliased tails that the lines beside it drop into it
/// as the clear left them, a level or two off — as the browser editor's band
/// does; anything more is a band in the wrong place.
#[test]
fn typing_repaints_the_same_pixels_as_opening_the_result() {
    for scale in [1.0, 2.0] {
        let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
        let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
        let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
        let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
            .unwrap()
            .fonts();
        let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
        let open = |bytes: &[u8]| {
            EditableDocument::open(
                bytes,
                scale,
                std::rc::Rc::clone(&fonts),
                std::rc::Rc::clone(&measurer),
            )
            .unwrap()
        };
        let mut document = open(&std::fs::read(core().join("contract.docx")).unwrap());
        let (paragraph, _) = first_text_paragraph(&document);
        document.place_caret(paragraph, 0);
        for ch in "abcあいう12345".chars() {
            document.insert_text(&ch.to_string()).unwrap();
        }
        document.delete(true).unwrap();
        document.delete(true).unwrap();
        let reopened = open(&document.export_docx().unwrap());
        assert_eq!(document.page_count(), reopened.page_count());
        for index in 0..document.page_count() {
            let (typed, fresh) = (page_rgba(&document, index), page_rgba(&reopened, index));
            let worst = typed
                .iter()
                .zip(&fresh)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            assert!(
                worst <= 2,
                "page {} at {scale}x: a channel is {worst} levels off",
                index + 1
            );
        }
    }
}

/// Pages before the edited paragraph's are not drawn again, which is what
/// keeps a keystroke as cheap on the last page as on the first; whatever is
/// typed where — a numbered paragraph the renderer repaints whole, a plain
/// one it repaints as a band, Enter and Backspace that move what follows —
/// every page must still be the page opening the result draws.
#[test]
fn typing_further_down_repaints_the_same_pixels_as_opening_the_result() {
    let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    let open = |bytes: &[u8]| {
        EditableDocument::open(
            bytes,
            2.0,
            std::rc::Rc::clone(&fonts),
            std::rc::Rc::clone(&measurer),
        )
        .unwrap()
    };
    let mut document = open(&std::fs::read(core().join("contract.docx")).unwrap());
    assert!(document.page_count() >= 3);
    let last = document.paragraph_count() - 1;
    let before: Vec<Vec<u8>> = (0..document.page_count())
        .map(|index| page_rgba(&document, index))
        .collect();
    // On the last page: nothing before it is drawn, nothing before it changes.
    let (paragraph, text) = (last - 4..=last)
        .find_map(|index| {
            document
                .paragraph_text(index)
                .filter(|text| text.chars().count() > 4)
                .map(|text| (index, text))
        })
        .expect("text near the end");
    document.place_caret(paragraph, char_offset(&text, 2));
    let page = document
        .page_and_offset(document.caret_rect().unwrap().document_y)
        .0;
    assert!(page > 0);
    document.take_repainted_pages();
    document.insert_text("x").unwrap();
    assert!(document.take_repainted_pages().iter().all(|&at| at >= page));
    for (index, pixels) in before.iter().enumerate().take(page) {
        assert!(
            &page_rgba(&document, index) == pixels,
            "page {} changed",
            index + 1
        );
    }
    // Then a run of edits across the document.
    for paragraph in (1..document.paragraph_count()).step_by(7) {
        let Some(text) = document
            .paragraph_text(paragraph)
            .filter(|text| text.chars().count() > 3)
        else {
            continue;
        };
        document.place_caret(paragraph, char_offset(&text, 2));
        for ch in "aあ1".chars() {
            document.insert_text(&ch.to_string()).unwrap();
        }
        document.delete(true).unwrap();
        if paragraph % 3 == 0 {
            document.insert_paragraph_break().unwrap();
            document.delete(true).unwrap();
        }
    }
    let reopened = open(&document.export_docx().unwrap());
    assert_eq!(document.page_count(), reopened.page_count());
    for index in 0..document.page_count() {
        let (typed, fresh) = (page_rgba(&document, index), page_rgba(&reopened, index));
        let worst = typed
            .iter()
            .zip(&fresh)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(
            worst <= 2,
            "page {}: a channel is {worst} levels off",
            index + 1
        );
    }
}

/// A keystroke that keeps its paragraph's height reports the one page it
/// repainted, which is all a shell keeping page images has to re-read; one
/// that pushes the text below down reports every page it moved.
#[test]
fn a_keystroke_reports_the_pages_it_repainted() {
    let mut document = open("contract.docx");
    let (paragraph, _) = first_text_paragraph(&document);
    document.place_caret(paragraph, 0);
    document.take_repainted_pages();
    let before: Vec<Vec<u8>> = (0..document.page_count())
        .map(|index| page_rgba(&document, index))
        .collect();

    document.insert_text("a").unwrap();
    assert_eq!(document.take_repainted_pages(), vec![0]);
    for (index, page) in before.iter().enumerate().skip(1) {
        assert!(
            &page_rgba(&document, index) == page,
            "page {} changed",
            index + 1
        );
    }

    document.insert_paragraph_break().unwrap();
    assert_eq!(
        document.take_repainted_pages(),
        (0..document.page_count()).collect::<Vec<_>>()
    );
}

/// The first offset in `paragraph` where a soft wrap falls: the caret there
/// is drawn on one line and the next character's on the line below.
fn first_wrap(document: &mut EditableDocument, paragraph: usize) -> (usize, f64, f64) {
    let text = document.paragraph_text(paragraph).unwrap();
    let offsets: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
    for pair in offsets.windows(2) {
        let here = document.place_caret(paragraph, pair[0]).unwrap();
        let next = document.place_caret(paragraph, pair[1]).unwrap();
        if next.document_y > here.document_y + here.height / 2.0 {
            // The wrap is at pair[0] when the caret before it is still above.
            let wrap = document.place_caret(paragraph, pair[0]).unwrap();
            return (pair[0], wrap.document_y, next.document_y);
        }
    }
    panic!("paragraph {paragraph} does not wrap");
}

#[test]
fn a_click_at_a_wrapped_lines_start_puts_the_caret_there() {
    let mut document = open("contract.docx");
    let (paragraph, _) = first_text_paragraph(&document);
    let (wrap, above, below) = first_wrap(&mut document, paragraph);
    let height = document.caret_rect().unwrap().height;

    // Left of the line below: its first position, drawn on that line.
    let caret = document.place_caret_at(0.0, below + height / 2.0).unwrap();
    assert_eq!(document.caret(), Some((paragraph, wrap)));
    assert_eq!(caret.document_y, below);
    assert_eq!(document.caret_rect().unwrap().document_y, below);
    // Past the end of the line above: the same offset, drawn at that end.
    let caret = document
        .place_caret_at(10_000.0, above + height / 2.0)
        .unwrap();
    assert_eq!(document.caret(), Some((paragraph, wrap)));
    assert_eq!(caret.document_y, above);
}

/// Dark paper turns every page over — dark ground, light ink — for the
/// screen only: switching back gives the same pixels, and the saved file is
/// the one a light-paper session saves.
#[test]
fn dark_paper_turns_the_pages_over_and_back() {
    let mut document = open("contract.docx");
    let light: Vec<Vec<u8>> = (0..document.page_count())
        .map(|index| page_rgba(&document, index))
        .collect();
    let saved = document.export_docx().unwrap();

    assert!(document.set_dark_paper(true).unwrap());
    assert!(!document.set_dark_paper(true).unwrap(), "already dark");
    assert_eq!(document.take_repainted_from(), Some(0));
    for index in 0..document.page_count() {
        let pixels = page_rgba(&document, index);
        assert!(
            pixels[..3].iter().all(|&channel| channel < 60),
            "page {} corner {:?}",
            index + 1,
            &pixels[..4]
        );
        let lit = pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 150)
            .count();
        assert!(lit > 2000, "page {} has {lit} light pixels", index + 1);
    }
    assert_eq!(document.export_docx().unwrap(), saved);

    assert!(document.set_dark_paper(false).unwrap());
    for (index, before) in light.iter().enumerate() {
        assert!(
            &page_rgba(&document, index) == before,
            "page {} differs after switching back",
            index + 1
        );
    }
}

/// Enter at the end of a paragraph leaves that paragraph exactly as it was, so
/// only the paragraphs after it move; the repaint must still show the new one.
#[test]
fn enter_at_the_end_of_a_paragraph_repaints_the_pages() {
    let mut document = open("contract.docx");
    let paragraph = (0..document.paragraph_count())
        .find(|&index| {
            document
                .paragraph_text(index)
                .is_some_and(|text| text.starts_with("開示を受けたときに既に保有"))
        })
        .expect("the first list item");
    let end = document.paragraph_text(paragraph).unwrap().len();
    document.place_caret(paragraph, end).unwrap();
    document.insert_paragraph_break().unwrap();
    document.insert_text("abc").unwrap();

    let fresh = open_bytes(&document.export_docx().unwrap());
    assert_eq!(page_rgba(&document, 0), page_rgba(&fresh, 0));
}

#[test]
fn home_and_end_in_a_table_cell_stay_in_the_cell() {
    use jade_web::editing::WordFormatting;
    let mut document = open("contract.docx");
    document.place_caret(0, 0);
    let first = document.insert_table(2, 3).unwrap();
    document.place_caret(first.0, first.1);
    document.insert_text("left cell text").unwrap();
    document.move_caret(Motion::Right, false).unwrap();
    let second = document.caret().unwrap();
    assert_ne!(second.0, first.0, "Right goes on to the next cell");
    document.insert_text("middle").unwrap();

    document.place_caret(first.0, 4);
    document.move_caret(Motion::LineEnd, false).unwrap();
    assert_eq!(document.caret(), Some((first.0, "left cell text".len())));
    document.place_caret(second.0, 3);
    document.move_caret(Motion::LineStart, false).unwrap();
    assert_eq!(document.caret(), Some((second.0, 0)));
    document.place_caret(second.0, 3);
    document.move_caret(Motion::LineEnd, true).unwrap();
    assert_eq!(
        document.selection(),
        Some(((second.0, 3), (second.0, "middle".len())))
    );
}
