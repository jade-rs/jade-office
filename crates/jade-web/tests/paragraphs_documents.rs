//! The 段落 dialog over the browser build's Word document, as both shells
//! have it: it opens on the caret paragraph's format — indents in 字 in a
//! Japanese document, millimetres in an English one — and OK writes what was
//! changed, on every paragraph selected, as one undo step that the .docx
//! carries.

mod common;

use jade_web::editing::WordFormatting;
use jade_web::word::new::{blank_document, Blank};
use jade_web::word::{
    IndentUnit, LineSpacing, ParagraphField, ParagraphForm, SpecialIndent, WordParagraphs,
};

fn type_into(form: &mut ParagraphForm, field: ParagraphField, text: &str) {
    form.focus_on(field);
    form.focused_mut().insert(text);
}

#[test]
fn the_dialog_writes_what_was_changed_in_characters() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "段落");
    document.place_caret(paragraph, 0);
    let was = document.paragraph_format();
    assert_eq!(was.unit, IndentUnit::Characters);

    let mut form = ParagraphForm::new(was.clone());
    form.format.alignment = "center".to_string();
    type_into(&mut form, ParagraphField::Left, "2");
    form.format.special = SpecialIndent::FirstLine;
    type_into(&mut form, ParagraphField::SpecialBy, "1");
    type_into(&mut form, ParagraphField::Before, "6");
    type_into(&mut form, ParagraphField::After, "3");
    form.set_line_rule(LineSpacing::Exactly(0.0));
    type_into(&mut form, ParagraphField::LineAmount, "18");
    form.format.keep_next = true;
    form.format.page_break_before = true;
    form.format.widow_control = !was.widow_control;
    form.submit(&mut document).unwrap();

    let now = document.paragraph_format();
    assert_eq!(now.alignment, "center");
    assert_eq!((now.left, now.right), (2.0, was.right));
    assert_eq!(
        (now.special, now.special_by),
        (SpecialIndent::FirstLine, 1.0)
    );
    assert_eq!((now.before, now.after), (6.0, 3.0));
    assert_eq!(now.line, LineSpacing::Exactly(18.0));
    assert!(now.keep_next && now.page_break_before && !now.keep_lines);
    assert_eq!(now.widow_control, !was.widow_control);

    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    for written in [
        r#"w:leftChars="200""#,
        r#"w:firstLineChars="100""#,
        r#"w:before="120""#,
        r#"w:after="60""#,
        r#"w:line="360""#,
        r#"w:lineRule="exact""#,
        "<w:keepNext/>",
        "<w:pageBreakBefore/>",
        r#"<w:jc w:val="center"/>"#,
    ] {
        assert!(xml.contains(written), "{written} is not written");
    }

    // One undo takes it all back.
    document.undo().unwrap();
    assert_eq!(document.paragraph_format(), was);
}

#[test]
fn an_english_document_measures_in_millimetres_and_hanging_indents() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    document.insert_text("Paragraph one").unwrap();
    let was = document.paragraph_format();
    assert_eq!(was.unit, IndentUnit::Millimetres);

    let mut form = ParagraphForm::new(was.clone());
    type_into(&mut form, ParagraphField::Left, "25.4");
    form.format.special = SpecialIndent::Hanging;
    type_into(&mut form, ParagraphField::SpecialBy, "12.7");
    form.set_line_rule(LineSpacing::Multiple(0.0));
    type_into(&mut form, ParagraphField::LineAmount, "1.2");
    form.format.keep_lines = true;
    form.submit(&mut document).unwrap();

    let now = document.paragraph_format();
    assert_eq!(now.left, 25.4);
    assert_eq!(
        (now.special, now.special_by),
        (SpecialIndent::Hanging, 12.7)
    );
    assert_eq!(now.line, LineSpacing::Multiple(1.2));
    assert!(now.keep_lines);
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains(r#"w:left="1440""#), "one inch");
    assert!(xml.contains(r#"w:hanging="720""#));
    assert!(xml.contains(r#"w:line="288""#));
    assert!(xml.contains("<w:keepLines/>"));

    // Back to none: the hanging indent goes, the left edge stays.
    let mut form = ParagraphForm::new(now.clone());
    form.format.special = SpecialIndent::None;
    form.submit(&mut document).unwrap();
    let cleared = document.paragraph_format();
    assert_eq!((cleared.special, cleared.left), (SpecialIndent::None, 25.4));
}

/// The ribbon's line spacing list writes the multiple it says.
#[test]
fn the_ribbon_line_spacing_reads_back() {
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "行間");
    document.place_caret(paragraph, 0);
    document.set_line_spacing(1.5).unwrap();
    assert_eq!(document.paragraph_format().line, LineSpacing::OneAndHalf);
    document.set_line_spacing(1.15).unwrap();
    assert_eq!(
        document.paragraph_format().line,
        LineSpacing::Multiple(1.15)
    );
}
