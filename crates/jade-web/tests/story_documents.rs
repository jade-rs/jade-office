//! Headers and footers edited in place (`WordDoc`): opened over a page,
//! typed into with every tool the body has — several paragraphs, fields, a
//! table — drawn over the page they belong to, and written back as one undo
//! step of the document when closed.

mod common;

use common::contract;
use jade_web::editing::{StoryChange, WordDoc, WordFormatting, WordSession};
use jade_web::word::headless::HeadlessFieldKind;
use jade_web::word::page::Story;

/// The parts the saved document has whose name starts with `prefix`,
/// joined.
fn parts(document: &mut WordDoc, prefix: &str) -> String {
    let bytes = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&bytes).unwrap();
    package
        .part_names()
        .iter()
        .filter(|name| name.starts_with(prefix))
        .filter_map(|name| package.part_str(name).ok().flatten())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_header_opens_over_its_page_and_takes_paragraphs() {
    let mut document = WordDoc::new(contract());
    let body_caret = (3, 0);
    document.place_caret(body_caret.0, body_caret.1);
    let plain = document.page(0).unwrap().rgba_premultiplied();

    document.open_story(0, Story::Header).unwrap();
    let story = document.story().unwrap();
    assert_eq!((story.page, story.role), (0, Story::Header));
    // The page is drawn with the body washed out behind the header.
    assert_ne!(document.page(0).unwrap().rgba_premultiplied(), plain);
    assert!(document.take_repainted_pages().contains(&0));
    // The caret is in the header, on page 1, above the body.
    let caret = document.caret_rect().unwrap();
    let (page, y) = document.page_and_offset(caret.document_y);
    assert_eq!(page, 0);
    assert!(document.in_story(0, y));
    // A click in the band lands in the story; one in the body does not.
    assert!(document.position_at(caret.x, caret.document_y).is_some());
    assert!(document
        .position_at(100.0, document.document_y(0, 500.0))
        .is_none());

    document.insert_text("first line").unwrap();
    document.insert_paragraph_break().unwrap();
    document.insert_text("second line").unwrap();
    assert!(document.is_dirty());
    // Saving while it is open writes what was typed.
    let saved = parts(&mut document, "word/header");
    assert!(saved.contains("first line") && saved.contains("second line"));

    assert!(document.close_story().unwrap());
    assert!(document.story().is_none());
    assert_eq!(document.caret(), Some(body_caret));
    let headers = parts(&mut document, "word/header");
    assert!(headers.contains("first line") && headers.contains("second line"));
    // One undo step of the document takes the whole edit back.
    document.undo().unwrap();
    assert!(!parts(&mut document, "word/header").contains("second line"));
}

#[test]
fn the_footer_takes_fields_a_table_and_formatting() {
    let mut document = WordDoc::new(contract());
    document.open_story(0, Story::Footer).unwrap();
    let today = "2026/10/22";
    for kind in [
        HeadlessFieldKind::Page,
        HeadlessFieldKind::NumPages,
        HeadlessFieldKind::Date,
        HeadlessFieldKind::FileName,
    ] {
        let cached = document.story_field_text(kind, "contract.docx", today, "見出し 1");
        document.insert_story_field(kind, cached).unwrap();
        document.insert_text(" ").unwrap();
    }
    document.insert_table(1, 2).unwrap();
    document.close_story().unwrap();
    let footers = parts(&mut document, "word/footer");
    for code in ["PAGE", "NUMPAGES", "DATE", "FILENAME"] {
        assert!(footers.contains(code), "{code} missing");
    }
    assert!(footers.contains("<w:tbl>"));
}

#[test]
fn a_different_first_page_and_the_distance_change_the_section() {
    let mut document = WordDoc::new(contract());
    document.open_story(0, Story::Header).unwrap();
    let before = document.story_layout().unwrap();
    document
        .change_story(StoryChange::DifferentFirst(!before.different_first_page))
        .unwrap();
    let story = document.story().unwrap();
    assert_eq!(
        story.variant,
        if before.different_first_page {
            "default"
        } else {
            "first"
        }
    );
    document.change_story(StoryChange::Distance(true)).unwrap();
    let after = document.story_layout().unwrap();
    assert!((after.header_distance - before.header_distance - 72.0 / 25.4).abs() < 0.01);
    assert_eq!(after.different_first_page, !before.different_first_page);
    document.close_story().unwrap();
    // Each change was its own step of the document.
    document.undo().unwrap();
    document.undo().unwrap();
    let restored = document.main().word_document().section_layout(Some(0));
    assert_eq!(restored.different_first_page, before.different_first_page);
}

#[test]
fn a_double_click_finds_the_margin_it_is_in() {
    let document = WordDoc::new(contract());
    let (_, height) = document.page_size();
    assert_eq!(document.margin_story(0, 4.0), Some(Story::Header));
    assert_eq!(document.margin_story(0, height - 4.0), Some(Story::Footer));
    assert_eq!(document.margin_story(0, height / 2.0), None);
}
