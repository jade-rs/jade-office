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

/// 見開きページのインデント設定: the paragraph's left indent is its inside
/// one, so on an even page it is taken from the right, as Word lays it out.
#[test]
fn mirror_indents_put_the_left_indent_inside() {
    let mut document = common::open_bytes(&blank_document(Blank::English));
    document.place_caret(0, 0);
    document.insert_text("Page one").unwrap();
    document.insert_paragraph_break().unwrap();
    document
        .insert_text("Page two, indented from the inside of the spread")
        .unwrap();
    let was = document.paragraph_format();
    let mut form = ParagraphForm::new(was.clone());
    type_into(&mut form, ParagraphField::Left, "50");
    form.format.page_break_before = true;
    form.submit(&mut document).unwrap();
    let plain = document.page(1).expect("page two").rgba_premultiplied();

    let was = document.paragraph_format();
    assert!(!was.mirror_indents);
    let mut form = ParagraphForm::new(was.clone());
    form.format.mirror_indents = true;
    form.submit(&mut document).unwrap();
    assert!(document.paragraph_format().mirror_indents);
    assert!(
        document.page(1).unwrap().rgba_premultiplied() != plain,
        "on an even page the inside indent is on the right"
    );

    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains("<w:mirrorIndents/>"), "{xml}");
    document.undo().unwrap();
    assert!(!document.paragraph_format().mirror_indents);
}

/// アウトライン レベル and 既定で折りたたみ: a body paragraph given レベル 2
/// joins the navigation pane and アウトライン at that level, saved as
/// `w:outlineLvl` and `w15:collapsed`; the reopened document collapses it.
/// 本文 takes both away again, and a 見出し style's level is greyed out.
#[test]
fn the_outline_level_is_written_and_followed() {
    use jade_web::word::outline::level_of;
    use jade_web::word::references::navigation::WordNavigation;
    use jade_web::word::{WordSession, BODY_TEXT};

    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "概要");
    document.place_caret(paragraph, 0);
    let was = document.paragraph_format();
    assert_eq!((was.outline_level, was.outline_locked), (BODY_TEXT, false));

    let mut form = ParagraphForm::new(was.clone());
    form.toggle_collapsed();
    assert!(!form.format.collapsed, "本文 does not collapse");
    form.set_outline_level(1);
    form.toggle_collapsed();
    form.submit(&mut document).unwrap();

    let now = document.paragraph_format();
    assert_eq!((now.outline_level, now.collapsed), (1, true));
    assert_eq!(level_of(document.word_document(), paragraph), Some(2));
    assert!(document
        .outline()
        .iter()
        .any(|heading| heading.paragraph == paragraph && heading.level == 2));

    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains(r#"<w:outlineLvl w:val="1"/>"#), "the level");
    assert!(xml.contains("<w15:collapsed/>"), "collapsed by default");
    assert!(xml.contains("xmlns:w15="), "its namespace is declared");
    let reopened = common::open_bytes(&saved);
    assert!(reopened.collapsed_by_default().contains(&paragraph));

    // 本文 again: neither is left behind.
    let mut form = ParagraphForm::new(now);
    form.set_outline_level(BODY_TEXT);
    assert!(!form.format.collapsed);
    form.submit(&mut document).unwrap();
    assert_eq!(level_of(document.word_document(), paragraph), Some(10));
    let xml = jade_opc::OpcPackage::open(&document.export_docx().unwrap())
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap();
    assert!(!xml.contains("<w:outlineLvl") && !xml.contains("<w15:collapsed"));

    // A 見出し 1 paragraph's level is its style's: the list does nothing.
    use jade_web::word::styles::WordStyles;
    WordStyles::apply_style(&mut document, "Heading1").unwrap();
    let heading = document.paragraph_format();
    assert_eq!((heading.outline_level, heading.outline_locked), (0, true));
    let mut form = ParagraphForm::new(heading.clone());
    form.set_outline_level(4);
    assert_eq!(form.format.outline_level, 0);
}

/// 書式の詳細: the caret's フォント, 段落 and セクション as the pane lists
/// them, following what was just set.
#[test]
fn reveal_formatting_lists_the_font_paragraph_and_section() {
    use jade_web::word::reveal_formatting::{
        RevealGroup, RevealRow, RevealValue, WordRevealFormatting,
    };
    let mut document = common::open_bytes(&blank_document(Blank::Japanese));
    document.place_caret(0, 0);
    document.insert_text("書式").unwrap();
    document.select((0, 0), (0, "書式".len())).unwrap();
    WordFormatting::toggle_format(&mut document, jade_web::editing::CharacterFormat::Bold).unwrap();
    WordFormatting::set_alignment(&mut document, "center").unwrap();
    document.select((0, 0), (0, "書".len())).unwrap();

    let font = document.reveal(RevealGroup::Font);
    assert!(
        font.contains(&RevealRow {
            label: "reveal.effect",
            value: RevealValue::Key("reveal.bold"),
        }),
        "{font:?}"
    );
    assert!(font.iter().any(|row| row.label == "reveal.size"));
    let paragraph = document.reveal(RevealGroup::Paragraph);
    assert!(paragraph.contains(&RevealRow {
        label: "reveal.alignment",
        value: RevealValue::Key("reveal.align.center"),
    }));
    assert!(paragraph.iter().any(|row| row.label == "reveal.style"));
    let section = document.reveal(RevealGroup::Section);
    assert!(
        section.contains(&RevealRow {
            label: "reveal.paper",
            value: RevealValue::Text("210 × 297 mm".to_string()),
        }),
        "{section:?}"
    );
    assert!(section.contains(&RevealRow {
        label: "reveal.orientation",
        value: RevealValue::Key("reveal.portrait"),
    }));
}

#[test]
fn text_with_similar_formatting_is_selected_and_formatted_together() {
    use jade_web::editing::CharacterFormat;
    use jade_web::word::similar::{format, still_selected, RangeFormat, WordSimilar};
    let mut document = common::open_bytes(&blank_document(Blank::Japanese));
    document.place_caret(0, 0);
    document.insert_text("AAbbAAcc").unwrap();
    for start in [0, 4] {
        document.select((0, start), (0, start + 2)).unwrap();
        WordFormatting::toggle_format(&mut document, CharacterFormat::Bold).unwrap();
    }
    let italic = |document: &jade_web::editing::EditableDocument, at: usize| {
        WordFormatting::run_style_at(document, (0, at))
            .and_then(|style| style.italic)
            .unwrap_or(false)
    };

    // A caret in the first bold run finds both.
    document.place_caret(0, 1);
    let ranges = document.select_similar_formatting();
    assert_eq!(ranges, vec![((0, 0), (0, 2)), ((0, 4), (0, 6))]);
    assert_eq!(document.selection(), Some(((0, 0), (0, 2))));
    assert!(still_selected(&document, &ranges));

    // Italic goes on both, not between them, as one undo step.
    format(
        &mut document,
        &ranges,
        RangeFormat::Toggle(CharacterFormat::Italic),
    )
    .unwrap();
    assert!(italic(&document, 1) && italic(&document, 5));
    assert!(!italic(&document, 3));
    assert!(still_selected(&document, &ranges));
    document.undo().unwrap();
    assert!(!italic(&document, 1) && !italic(&document, 5));

    // Selecting elsewhere ends it: the command takes the selection alone.
    document.select((0, 2), (0, 4)).unwrap();
    assert!(!still_selected(&document, &ranges));
    format(
        &mut document,
        &ranges,
        RangeFormat::Toggle(CharacterFormat::Italic),
    )
    .unwrap();
    assert!(italic(&document, 3) && !italic(&document, 1));
}
