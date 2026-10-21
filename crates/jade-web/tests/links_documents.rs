//! Hyperlinks in the browser build's Word document, as both shells have
//! them: ⌘K makes the selection a link to an address typed as Word takes it
//! (`www.…` gains `http://`), the link is found again under a click on its
//! text and at a caret in it, ⌘K on it edits its text, an internal link
//! (`#bookmark`) leads to the bookmark, and each is one undo step written
//! into the .docx.

mod common;

use jade_docx_editor::commands::bookmark::CreateBookmarkCommand;
use jade_web::editing::{LinkTarget, WordLinks, WordSession};

#[test]
fn a_selection_becomes_a_link_that_a_click_follows() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "jade ");
    document.select((paragraph, 0), (paragraph, 4)).unwrap();

    let draft = document.link_draft();
    assert_eq!(draft.display, "jade");
    assert!(!draft.editing);
    let caret = document
        .apply_link(&draft, &draft.display, "www.example.com")
        .unwrap();
    assert_eq!(caret, (paragraph, 4));
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("jade "));

    // At a caret in it, and under a click on its text.
    let link = document.hyperlink_at((paragraph, 2)).expect("a link");
    assert_eq!(link.address, "http://www.example.com");
    assert_eq!((link.start, link.end), (0, 4));
    let rect = document.place_caret(paragraph, 2).unwrap();
    let clicked = document
        .hyperlink_at_point(rect.x + 1.0, rect.document_y + rect.height / 2.0)
        .expect("the click is on the link");
    assert_eq!(clicked, link);
    assert_eq!(
        document.link_target(&clicked),
        Some(LinkTarget::Address("http://www.example.com".to_string()))
    );
    // Not on the text after it.
    let after = document.place_caret(paragraph, 6).unwrap();
    assert!(document
        .hyperlink_at_point(after.x + 1.0, after.document_y + after.height / 2.0)
        .is_none());

    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains("<w:hyperlink"), "{xml}");
    let rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    assert!(rels.contains("http://www.example.com"));

    document.undo().unwrap();
    assert!(document.hyperlink_at((paragraph, 2)).is_none());
}

#[test]
fn command_k_on_a_link_edits_it() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "jade ");
    document.select((paragraph, 0), (paragraph, 4)).unwrap();
    let draft = document.link_draft();
    document
        .apply_link(&draft, "jade", "https://jade.example/")
        .unwrap();

    // A bare caret in the link: the dialog opens on the whole of it.
    document.place_caret(paragraph, 2);
    let draft = document.link_draft();
    assert!(draft.editing);
    assert_eq!(draft.display, "jade");
    assert_eq!(draft.address, "https://jade.example/");
    assert_eq!(draft.range, ((paragraph, 0), (paragraph, 4)));
    let caret = document
        .apply_link(&draft, "jade office", "https://jade.example/office")
        .unwrap();
    assert_eq!(caret, (paragraph, "jade office".len()));
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("jade office "));
    let link = document.hyperlink_at((paragraph, 1)).unwrap();
    assert_eq!(link.address, "https://jade.example/office");
    assert_eq!((link.start, link.end), (0, "jade office".len()));

    // An empty address takes it off.
    document.place_caret(paragraph, 1);
    let draft = document.link_draft();
    document.apply_link(&draft, &draft.display, "").unwrap();
    assert!(document.hyperlink_at((paragraph, 1)).is_none());

    // At a bare caret outside any link the address is also the text.
    document.place_caret(paragraph, 0);
    let draft = document.link_draft();
    assert!(!draft.editing && draft.display.is_empty());
    document
        .apply_link(&draft, "", "someone@example.com")
        .unwrap();
    let text = document.paragraph_text(paragraph).unwrap();
    assert!(text.starts_with("mailto:someone@example.com"), "{text}");
    assert_eq!(
        document.hyperlink_at((paragraph, 3)).unwrap().address,
        "mailto:someone@example.com"
    );
}

#[test]
fn an_internal_link_leads_to_its_bookmark() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "see below ");
    let target = (paragraph + 3..document.paragraph_count())
        .find(|&index| {
            document.cell_id_for_paragraph(index).is_none()
                && document
                    .paragraph_text(index)
                    .is_some_and(|text| !text.trim().is_empty())
        })
        .unwrap();
    document
        .execute_command(
            Box::new(CreateBookmarkCommand::new(
                target,
                "jade_target".to_string(),
            )),
            true,
        )
        .unwrap();
    document.select((paragraph, 0), (paragraph, 9)).unwrap();
    let draft = document.link_draft();
    document
        .apply_link(&draft, "see below", "#jade_target")
        .unwrap();
    let link = document.hyperlink_at((paragraph, 1)).unwrap();
    assert!(link.is_internal());
    assert_eq!(
        document.link_target(&link),
        Some(LinkTarget::Position((target, 0)))
    );
    // A bookmark the document does not have goes nowhere.
    let mut missing = link.clone();
    missing.address = "#nowhere".to_string();
    assert_eq!(document.link_target(&missing), None);
}

#[test]
fn addresses_are_completed_as_word_does() {
    use jade_web::word::links::normalize_address;
    assert_eq!(
        normalize_address("www.example.com"),
        "http://www.example.com"
    );
    assert_eq!(normalize_address(" https://a.b/c "), "https://a.b/c");
    assert_eq!(normalize_address("me@example.com"), "mailto:me@example.com");
    assert_eq!(normalize_address("#Top"), "#Top");
    assert_eq!(normalize_address(""), "");
}
