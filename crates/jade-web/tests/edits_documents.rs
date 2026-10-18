//! Word documents are edited in the browser build the way the native app
//! edits them: a click places the caret, typing inserts at it, Backspace and
//! Delete remove characters (not bytes), Enter splits the paragraph, undo
//! and redo step through it, the page repaints to show it, and a save
//! carries every edit into the .docx.

use std::path::PathBuf;

use jade_web::editing::EditableDocument;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn open(name: &str) -> EditableDocument {
    let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    let bytes = std::fs::read(core().join(name)).unwrap();
    EditableDocument::open(&bytes, 1.0, fonts, measurer).unwrap()
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
