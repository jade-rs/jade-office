//! 位置合わせタブの挿入 over the browser build's Word document, as both
//! shells have it: an absolute position tab (`w:ptab`) at the caret lines
//! what follows it up at the right (or the centre) of the margins whatever
//! the tab stops say, is saved as Word writes it, and reads back.

mod common;

use jade_web::editing::WordSession;
use jade_web::word::alignment_tab::{AlignmentTabForm, TabAlign};
use jade_web::word::new::{blank_document, Blank};

const PX_PER_PT: f64 = 96.0 / 72.0;

#[test]
fn a_right_alignment_tab_puts_the_text_after_it_at_the_right_margin() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    document.insert_text("Chapter").unwrap();
    let form = AlignmentTabForm {
        alignment: TabAlign::Right,
        relative_to_indent: false,
        leader: Some("dot"),
    };
    form.submit(&mut document).unwrap();
    let after_tab = document.selection_range().unwrap().focus;
    assert_eq!(
        (after_tab.paragraph, after_tab.offset),
        (0, "Chapter\t".len())
    );
    document.insert_text("12").unwrap();

    let layout = document.word_document().section_layout(Some(0));
    let right_margin = f64::from(layout.width - layout.margin_right) * PX_PER_PT;
    let end = document.place_caret(0, "Chapter\t12".len()).unwrap();
    assert!(
        (end.x - right_margin).abs() < 2.0,
        "the page number ends at the margin: {} vs {right_margin}",
        end.x
    );

    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(
        xml.contains(r#"<w:ptab w:relativeTo="margin" w:alignment="right" w:leader="dot"/>"#),
        "{xml}"
    );
    // Read back, it is laid out the same.
    let mut reopened = common::open_bytes(&saved);
    let end = reopened.place_caret(0, "Chapter\t12".len()).unwrap();
    assert!((end.x - right_margin).abs() < 2.0);

    // One undo takes the tab away.
    document.undo().unwrap();
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("Chapter"));
}

#[test]
fn a_centre_alignment_tab_centres_what_follows_between_the_margins() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    let form = AlignmentTabForm {
        alignment: TabAlign::Center,
        ..AlignmentTabForm::default()
    };
    form.submit(&mut document).unwrap();
    document.insert_text("Title").unwrap();
    let layout = document.word_document().section_layout(Some(0));
    let centre = f64::from(
        layout.margin_left + (layout.width - layout.margin_left - layout.margin_right) / 2.0,
    ) * PX_PER_PT;
    let start = document.place_caret(0, 1).unwrap().x;
    let end = document.place_caret(0, 1 + "Title".len()).unwrap().x;
    assert!(
        ((start + end) / 2.0 - centre).abs() < 2.0,
        "{start}..{end} centred on {centre}"
    );
}
