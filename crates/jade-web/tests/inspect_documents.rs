//! ファイル › 情報 › ドキュメント検査 on the browser build's Word document —
//! what both shells run: comments and changes, who wrote it, custom XML,
//! hidden text and headers and footers are found, and すべて削除 takes
//! each away, from the document or from what the save writes.

mod common;
use common::{contract, open_bytes, typed};
use jade_web::word::font::{Effect, FontTab, WordCharacters};
use jade_web::word::inspect::{clean_package, InspectCategory, PackageCleaning, WordInspect};
use jade_web::word::review::WordReview;
use jade_web::word::reviewer::Reviewer;

#[test]
fn the_inspector_finds_and_removes_what_a_document_carries() {
    let mut document = contract();
    let ann = Reviewer::named("Ann Lee");
    let paragraph = typed(&mut document, "Secret ");
    document.select((paragraph, 0), (paragraph, 6)).unwrap();
    let mut dialog = document.font_dialog();
    dialog.tab = FontTab::Font;
    dialog.set_effect(Effect::Hidden, true);
    document.apply_font_dialog(&dialog).unwrap();
    document.place_caret(paragraph, 8);
    document.add_comment("look", &ann).unwrap();
    document.set_tracking_changes(true, &ann).unwrap();
    document.place_caret(paragraph, 7);
    document.insert_text("new ").unwrap();
    document.set_tracking_changes(false, &ann).unwrap();

    let report = document.inspect();
    assert_eq!(report.comments, 1);
    assert!(report.revisions >= 1);
    assert_eq!(report.hidden_runs, 1);
    assert!(report.custom_xml_parts >= 1, "the fixture has custom XML");
    assert!(report.personal.iter().any(|(name, _)| name == "author"));
    assert!(!report.is_clean());

    document
        .remove_inspected(InspectCategory::CommentsAndRevisions)
        .unwrap();
    document
        .remove_inspected(InspectCategory::HiddenText)
        .unwrap();
    let report = document.inspect();
    assert_eq!(
        (report.comments, report.revisions, report.hidden_runs),
        (0, 0, 0)
    );
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with(" new "));

    // Undo takes すべて削除 back.
    document.undo().unwrap();
    assert_eq!(document.inspect().hidden_runs, 1);
    document.redo().unwrap();

    // Who wrote it and custom XML go from what the save writes.
    let saved = clean_package(
        &document.export_docx().unwrap(),
        PackageCleaning {
            personal: true,
            custom_xml: true,
        },
    )
    .unwrap();
    let reopened = open_bytes(&saved);
    let report = reopened.inspect();
    assert!(report.personal.is_empty(), "{:?}", report.personal);
    assert_eq!(report.custom_xml_parts, 0);
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    assert!(!rels.contains("customXml"));
}

#[test]
fn a_document_word_marked_loses_its_authors_on_every_save() {
    let mut document = contract();
    let ann = Reviewer::named("Ann Lee");
    document.set_tracking_changes(true, &ann).unwrap();
    typed(&mut document, "tracked ");
    let bytes = document.export_docx().unwrap();
    let mut package = jade_opc::OpcPackage::open(&bytes).unwrap();
    let settings = package.part_str("word/settings.xml").unwrap().unwrap();
    let settings = settings.replacen("<w:zoom", "<w:removePersonalInformation/><w:zoom", 1);
    package.replace("word/settings.xml", settings.into_bytes());
    let marked = package.write().unwrap();

    let saved = clean_package(&marked, PackageCleaning::default()).unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let body = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(!body.contains("Ann Lee"));
    assert!(body.contains("w:author=\"Author\""));
}
