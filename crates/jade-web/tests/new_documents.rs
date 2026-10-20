//! 新規文書 in the browser build, as the native app makes them too: a blank
//! document as Japanese and English Word make one opens, takes typing and a
//! heading, and saves; a template opens as a document made from it.

mod common;
use common::open_bytes;
use jade_web::editing::WordFormatting;
use jade_web::word::new::{blank_document, document_from_template, Blank};

#[test]
fn a_blank_document_opens_and_takes_typing() {
    for blank in [Blank::Japanese, Blank::English] {
        let mut document = open_bytes(&blank_document(blank));
        assert_eq!(document.paragraph_count(), 1);
        assert_eq!(document.page_count(), 1);
        // A4 (210 mm) and Letter (8.5 in), in CSS px.
        let (width, _) = document.page_size();
        match blank {
            Blank::Japanese => assert_eq!(width.round(), 794.0),
            Blank::English => assert_eq!(width.round(), 816.0),
        }
        document.place_caret(0, 0);
        document.insert_text("最初の行").unwrap();
        document.set_paragraph_style(Some("Heading1")).unwrap();
        assert_eq!(
            document.format_state().style_id.as_deref(),
            Some("Heading1")
        );
        let saved = document.export_docx().unwrap();
        let reopened = open_bytes(&saved);
        assert_eq!(reopened.paragraph_text(0).as_deref(), Some("最初の行"));
    }
}

#[test]
fn a_template_opens_as_a_document() {
    let template = std::fs::read(common::core().join("public/fixtures/blank.docx")).unwrap();
    let document = document_from_template(&template).unwrap();
    let opened = open_bytes(&document);
    assert!(opened.page_count() >= 1);
}

/// PDF として書き出す: every page, on the document's paper, the pages kept
/// as pixels before it put back.
#[test]
fn every_page_goes_into_the_pdf() {
    let mut document = common::contract();
    let pages = document.page_count();
    assert!(pages > 1);
    document.set_resident_pages(0..1).unwrap();
    let pdf = document.export_pdf().unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains(&format!("/Count {pages}")),
        "one picture a page"
    );
    assert_eq!(text.matches("/Subtype /Image").count(), pages);
    assert!(document.page(0).is_some(), "the page on show is back");
    assert!(document.page(1).is_none());
}
