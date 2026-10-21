//! ページ設定 by section on the browser build's Word document — the commands
//! the native app runs too: the dialog's change on the caret's section, on
//! the rest of the document from the caret, on all of it; the 段組み, 行番号
//! and ページ番号の書式 dialogs; 縦書き; and the breaks inside a section.
//! Each is one undo step.

mod common;
use common::{contract, typed};
use jade_web::editing::WordSession;
use jade_web::word::page::{SectionBreak, WordPage};
use jade_web::word::page_setup::{
    mm_to_points, ApplyTo, ColumnPreset, ColumnsDialog, Field, LineNumbersPreset, PageBreak,
    PageNumberDialog, SetupTab, WordPageSetup,
};

fn part(bytes: &[u8], name: &str) -> String {
    let package = jade_opc::OpcPackage::open(bytes).unwrap();
    package.part_str(name).unwrap().unwrap()
}

#[test]
fn this_section_leaves_the_others_as_they_were() {
    let mut document = contract();
    let paragraph = typed(&mut document, "before|after ");
    document.place_caret(paragraph, "before|".len());
    document
        .insert_section_break(SectionBreak::NextPage)
        .unwrap();
    // The caret is in the second section now.
    document.place_caret(paragraph + 1, 0);
    let first = document.word_document().section_layout(Some(0));

    let mut dialog = document.page_setup_dialog(SetupTab::Margins);
    assert_eq!(dialog.apply_to, ApplyTo::ThisSection);
    dialog.set_landscape(true);
    dialog.set_value(Field::MarginLeft, 40.0);
    document.apply_page_setup(&dialog).unwrap();

    let second = document.section_layout();
    assert!(second.landscape);
    assert!((second.margin_left - mm_to_points(40.0)).abs() < 0.05);
    assert_eq!(
        document.word_document().section_layout(Some(0)),
        first,
        "the first section keeps its page"
    );
    document.undo().unwrap();
    assert!(!document.section_layout().landscape);
}

#[test]
fn this_point_forward_starts_a_section_at_the_caret() {
    let mut document = contract();
    let paragraph = typed(&mut document, "from here ");
    let sections = document.word_document().section_count();
    document.place_caret(paragraph, 0);
    let mut dialog = document.page_setup_dialog(SetupTab::Margins);
    dialog.apply_to = ApplyTo::ThisPointForward;
    dialog.set_value(Field::MarginTop, 50.0);
    document.apply_page_setup(&dialog).unwrap();
    let after = document.word_document().section_count();
    assert_eq!(after, sections + 1);
    let last = document.word_document().section_layout(Some(after - 1));
    assert!((last.margin_top - mm_to_points(50.0)).abs() < 0.05);
    let first = document.word_document().section_layout(Some(0));
    assert!((first.margin_top - mm_to_points(50.0)).abs() > 1.0);
    document.undo().unwrap();
    assert_eq!(document.word_document().section_count(), sections);
}

#[test]
fn columns_line_numbers_and_page_numbers_are_saved() {
    let mut document = contract();
    let paragraph = typed(&mut document, "x");
    document.place_caret(paragraph, 0);

    let setup = document.page_setup_dialog(SetupTab::Grid);
    let mut columns = ColumnsDialog::open(&setup);
    columns.apply_preset(ColumnPreset::Right);
    columns.columns.separator = true;
    document.apply_columns(&columns).unwrap();
    document
        .set_line_numbers(LineNumbersPreset::Continuous)
        .unwrap();
    let mut numbers = PageNumberDialog::open(document.section_layout().page_numbering);
    numbers.numbering.format = "upperRoman".to_string();
    numbers.set_start(4);
    numbers.set_chapter(Some(1));
    document.apply_page_numbering(&numbers).unwrap();

    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"w:equalWidth="0""#), "{xml}");
    assert!(xml.contains(r#"w:sep="1""#));
    assert!(xml.contains(r#"w:restart="continuous""#));
    assert!(xml.contains(r#"w:fmt="upperRoman" w:start="4" w:chapStyle="1" w:chapSep="hyphen""#));
    for _ in 0..3 {
        document.undo().unwrap();
    }
    let layout = document.section_layout();
    assert!(!layout.line_numbering.enabled);
    assert!(layout.columns.equal_width);
}

#[test]
fn vertical_text_turns_the_page() {
    let mut document = contract();
    let (width, height) = document.page_size();
    document.set_vertical_text(true).unwrap();
    assert!(document.vertical_text());
    let (turned_width, turned_height) = document.page_size();
    assert!(turned_width > turned_height && width < height);
    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"<w:textDirection w:val="tbRl"/>"#));
}

#[test]
fn breaks_go_in_at_the_caret() {
    let mut document = contract();
    let paragraph = typed(&mut document, "ab");
    let paragraphs = document.paragraph_count();
    document.place_caret(paragraph, 1);
    document.insert_page_break(PageBreak::Page).unwrap();
    assert_eq!(document.paragraph_count(), paragraphs);
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("a\u{000C}b"));
    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"<w:br w:type="page"/>"#));
    document.undo().unwrap();
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("ab"));
}
