//! Find and Replace with Word's options — 大文字と小文字を区別する and
//! 完全に一致する単語だけ — on the browser build's Word document, which the
//! native app runs as well: matches found, walked with wrapping, replaced one
//! at a time or all at once, a replace-all one undo step.

use std::path::PathBuf;

use jade_web::editing::{EditableDocument, FindOptions, WordFind, WordSession};

fn contract() -> EditableDocument {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    let bytes = std::fs::read(core.join("contract.docx")).unwrap();
    EditableDocument::open(&bytes, 1.0, fonts, measurer).unwrap()
}

/// A body paragraph with `text` typed at its start.
fn typed(document: &mut EditableDocument, text: &str) -> usize {
    let paragraph = (0..document.paragraph_count())
        .find(|&index| {
            document.cell_id_for_paragraph(index).is_none()
                && document
                    .paragraph_text(index)
                    .is_some_and(|text| text.chars().count() > 3)
        })
        .unwrap();
    document.place_caret(paragraph, 0);
    document.insert_text(text).unwrap();
    paragraph
}

fn in_paragraph(
    document: &EditableDocument,
    query: &str,
    options: FindOptions,
    paragraph: usize,
) -> usize {
    document
        .find_all(query, options)
        .iter()
        .filter(|found| found.paragraph == paragraph)
        .count()
}

const ANY: FindOptions = FindOptions {
    match_case: false,
    whole_word: false,
};
const CASE: FindOptions = FindOptions {
    match_case: true,
    whole_word: false,
};
const WORDS: FindOptions = FindOptions {
    match_case: false,
    whole_word: true,
};

#[test]
fn the_options_narrow_what_is_found() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Jade jade JADE jaded. ");
    assert_eq!(in_paragraph(&document, "jade", ANY, paragraph), 4);
    assert_eq!(in_paragraph(&document, "jade", CASE, paragraph), 2);
    assert_eq!(in_paragraph(&document, "jade", WORDS, paragraph), 3);
    let both = FindOptions {
        match_case: true,
        whole_word: true,
    };
    assert_eq!(in_paragraph(&document, "jade", both, paragraph), 1);

    // 次を検索 goes on from where it is; 前を検索 goes back.
    let first = document.find_next("jade", CASE, (paragraph, 0)).unwrap();
    assert_eq!((first.paragraph, first.start), (paragraph, 5));
    let second = document.find_next("jade", CASE, first.end()).unwrap();
    assert_eq!((second.paragraph, second.start), (paragraph, 15));
    assert_eq!(
        document.find_previous("jade", CASE, second.start()),
        Some(first)
    );
}

#[test]
fn replace_all_is_one_undo_step() {
    let mut document = contract();
    let paragraph = typed(&mut document, "Jade jade JADE jaded. ");
    let before = document.paragraph_text(paragraph).unwrap();
    let everywhere = document.find_all("jade", WORDS).len();
    assert_eq!(
        document.replace_all("jade", "Word", WORDS).unwrap(),
        everywhere
    );
    let after = document.paragraph_text(paragraph).unwrap();
    assert!(after.starts_with("Word Word Word jaded. "), "{after}");
    assert!(document.find_all("jade", WORDS).is_empty());

    document.undo().unwrap();
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);

    // One at a time: the match replaced, and where to go on from.
    let exact = FindOptions {
        match_case: true,
        whole_word: true,
    };
    let found = document.find_next("JADE", exact, (paragraph, 0)).unwrap();
    let next = document.replace_found(found, "W").unwrap();
    assert_eq!(next, (paragraph, found.start + 1));
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("Jade jade W jaded."));
}

#[test]
fn a_failed_batch_takes_back_what_it_did() {
    use jade_docx_headless_api::HeadlessCommand;
    let mut document = contract();
    let paragraph = typed(&mut document, "kept ");
    let before = document.paragraph_text(paragraph).unwrap();
    let result = document.apply_commands(
        vec![
            HeadlessCommand::InsertText {
                paragraph,
                offset: 0,
                text: "gone ".to_string(),
            },
            HeadlessCommand::InsertText {
                paragraph: usize::MAX,
                offset: 0,
                text: "nowhere".to_string(),
            },
        ],
        false,
    );
    assert!(result.is_err());
    assert_eq!(document.paragraph_text(paragraph).unwrap(), before);
}
