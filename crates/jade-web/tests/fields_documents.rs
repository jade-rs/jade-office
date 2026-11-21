//! Opening DATE/TIME fields uses the host clock without editing saved results.
mod common;

use jade_web::word::fields::FieldDateTime;
use jade_web::word::new::{blank_document, Blank};

const DATE_FIELD: &str = r#"<w:p><w:fldSimple w:instr='DATE \@ "yyyy-MM-dd"'><w:r><w:t>2000-01-01</w:t></w:r></w:fldSimple></w:p>"#;
const TIME_FIELD: &str = r#"<w:p><w:fldSimple w:instr='TIME \@ "HH:mm:ss"'><w:r><w:t>00:00:00</w:t></w:r></w:fldSimple></w:p>"#;

fn clock_document(story: &str, fields: &str, blank: Blank) -> Vec<u8> {
    let mut package = jade_opc::OpcPackage::open(&blank_document(blank)).unwrap();
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    if story == "body" {
        assert!(xml.contains("<w:body><w:p/>"));
        package.replace(
            "word/document.xml",
            xml.replace("<w:body><w:p/>", &format!("<w:body>{fields}"))
                .into_bytes(),
        );
    } else {
        let tag = if story == "header" { "hdr" } else { "ftr" };
        package.replace(
            &format!("word/{story}1.xml"),
            format!(r#"<w:{tag} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{fields}</w:{tag}>"#).into_bytes(),
        );
        assert!(xml.contains("<w:sectPr>"));
        package.replace(
            "word/document.xml",
            xml.replace(
                "<w:sectPr>",
                &format!(r#"<w:sectPr><w:{story}Reference w:type="default" r:id="clockStory"/>"#),
            )
            .into_bytes(),
        );
        let rels = package
            .part_str("word/_rels/document.xml.rels")
            .unwrap()
            .unwrap();
        package.replace("word/_rels/document.xml.rels", rels.replace("</Relationships>", &format!(r#"<Relationship Id="clockStory" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{story}" Target="{story}1.xml"/></Relationships>"#)).into_bytes());
        let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
        package.replace("[Content_Types].xml", types.replace("</Types>", &format!(r#"<Override PartName="/word/{story}1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.{story}+xml"/></Types>"#)).into_bytes());
    }
    package.write().unwrap()
}

fn assert_opening_clock(story: &str, fields: &str, blank: Blank) {
    let bytes = clock_document(story, fields, blank);
    let saved_clock = common::open_bytes(&bytes);
    let saved_before = saved_clock.word_docx().unwrap();
    let before = saved_clock.page(0).unwrap().rgba_premultiplied();
    let document = common::open_bytes_with_datetime(
        &bytes,
        Some(FieldDateTime {
            year: 2026,
            month: 11,
            day: 7,
            hour: 13,
            minute: 24,
            second: 35,
            weekday: 6,
        }),
    );
    assert_ne!(
        document.page(0).unwrap().rgba_premultiplied(),
        before,
        "{story} clock must affect the initial paint"
    );
    assert!(!document.is_dirty());
    assert!(!document.can_undo());
    let saved_after = document.word_docx().unwrap();
    let before = jade_opc::OpcPackage::open(&saved_before).unwrap();
    let after = jade_opc::OpcPackage::open(&saved_after).unwrap();
    for part in ["word/document.xml".to_owned(), format!("word/{story}1.xml")] {
        assert_eq!(
            before.part_str(&part).unwrap(),
            after.part_str(&part).unwrap()
        );
    }
}

#[test]
fn opening_body_clock_preserves_saved_results() {
    for blank in [Blank::English, Blank::Japanese] {
        for fields in [DATE_FIELD, TIME_FIELD] {
            assert_opening_clock("body", fields, blank);
        }
    }
}

#[test]
fn opening_header_only_clock_preserves_saved_results() {
    for fields in [DATE_FIELD, TIME_FIELD] {
        assert_opening_clock("header", fields, Blank::English);
    }
}

#[test]
fn opening_footer_only_clock_preserves_saved_results() {
    for fields in [DATE_FIELD, TIME_FIELD] {
        assert_opening_clock("footer", fields, Blank::English);
    }
}

/// A field the layout shows at another length than its saved result (a
/// PAGE field saved as "999" showing "1") is walked at what it shows: the
/// arrows step over its value and on through whole characters after it.
#[test]
fn arrows_walk_a_field_at_the_value_it_shows() {
    use jade_web::editing::{Motion, WordSession};
    use jade_web::word::headless::{HeadlessCommand, HeadlessFieldKind};
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "ab");
    document
        .apply_commands(
            vec![HeadlessCommand::InsertField {
                start_paragraph: paragraph,
                start_offset: 1,
                end_paragraph: paragraph,
                end_offset: 1,
                kind: HeadlessFieldKind::Page,
                cached_text: "999".into(),
            }],
            false,
        )
        .unwrap();
    let text = document.caret_text(paragraph).unwrap();
    assert!(text.starts_with("a1b"), "{text:?}");
    assert_eq!(
        text.len(),
        document.word_document().rendered_paragraph_len(paragraph)
    );
    document.place_caret(paragraph, 0);
    let mut stops = Vec::new();
    for _ in 0..5 {
        document.move_caret(Motion::Right, false).unwrap();
        stops.push(document.caret().unwrap().1);
    }
    let expected: Vec<_> = text
        .char_indices()
        .skip(1)
        .take(5)
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(stops, expected);
}

/// A copied field pastes as a field of the same kind, as Word does: the
/// PAGE field saved as "999" goes on showing the page, not "999", and one
/// undo takes the paste back. テキストのみ保持 takes the result as text.
#[test]
fn a_copied_field_pastes_as_a_field() {
    use jade_docx_editor::document::RunFieldKind;
    use jade_web::editing::{EditableDocument, WordSession};
    use jade_web::word::clipboard::{PasteMode, WordClipboard};
    use jade_web::word::headless::{HeadlessCommand, HeadlessFieldKind};
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "ab");
    document
        .apply_commands(
            vec![HeadlessCommand::InsertField {
                start_paragraph: paragraph,
                start_offset: 1,
                end_paragraph: paragraph,
                end_offset: 1,
                kind: HeadlessFieldKind::Page,
                cached_text: "999".into(),
            }],
            false,
        )
        .unwrap();
    let shown = document.caret_text(paragraph).unwrap();
    let page_fields = |document: &EditableDocument| {
        document
            .word_document()
            .paragraph(paragraph)
            .unwrap()
            .runs
            .iter()
            .filter(|run| {
                run.field
                    .as_ref()
                    .is_some_and(|field| matches!(field.kind, RunFieldKind::PageNumber))
            })
            .count()
    };
    assert_eq!(page_fields(&document), 1);

    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.select((paragraph, 0), (paragraph, end)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.fields.len(), 1);
    document.place_caret(paragraph, end);
    let caret = document.paste_rich(&clip).unwrap();

    let text = document.caret_text(paragraph).unwrap();
    assert_eq!(text, format!("{shown}{shown}"));
    assert_eq!(page_fields(&document), 2);
    assert_eq!(caret, (paragraph, text.len()));
    document.undo().unwrap();
    assert_eq!(document.caret_text(paragraph).unwrap(), shown);

    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    document.paste_as(&clip, PasteMode::TextOnly).unwrap();
    assert_eq!(page_fields(&document), 1);
    assert!(document.caret_text(paragraph).unwrap().contains("a999b"));
}

/// A header's page number keeps no text of its own once reopened — layout
/// draws its digits — and a copy of "Page 1 of 1" there still pastes its
/// PAGE and NUMPAGES as fields, as Word does.
#[test]
fn a_copied_header_page_number_pastes_as_a_field() {
    use jade_web::editing::{WordDoc, WordSession};
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::HeadlessFieldKind;
    use jade_web::word::page::Story;
    let saved_codes = |document: &mut WordDoc| {
        let bytes = document.export_docx().unwrap();
        let package = jade_opc::OpcPackage::open(&bytes).unwrap();
        let header = package
            .part_names()
            .iter()
            .filter(|name| name.starts_with("word/header"))
            .filter_map(|name| package.part_str(name).ok().flatten())
            .collect::<String>();
        let numpages = header.matches("NUMPAGES").count();
        (header.matches("PAGE").count() - numpages, numpages)
    };
    let mut document = WordDoc::new(common::contract());
    document.open_story(0, Story::Header).unwrap();
    document.insert_text("Page ").unwrap();
    document
        .insert_story_field(HeadlessFieldKind::Page, "1".into())
        .unwrap();
    document.insert_text(" of ").unwrap();
    document
        .insert_story_field(HeadlessFieldKind::NumPages, "1".into())
        .unwrap();
    assert!(document.close_story().unwrap());
    assert_eq!(saved_codes(&mut document), (1, 1));

    document.open_story(0, Story::Header).unwrap();
    let end = |document: &WordDoc| {
        document
            .story()
            .unwrap()
            .document
            .word_document()
            .rendered_paragraph_len(0)
    };
    let shown = end(&document);
    document.select((0, 0), (0, shown)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.plain_text(), "Page 1 of 1");
    assert_eq!(clip.fields.len(), 2);
    document.place_caret(0, shown);
    let caret = document.paste_rich(&clip).unwrap();
    assert_eq!(caret, (0, end(&document)));
    assert!(document.close_story().unwrap());
    assert_eq!(saved_codes(&mut document), (2, 2));
}

/// An index entry (XE) shows nothing, and a copy of the word it marks
/// brings it along: the pasted word is marked too.
#[test]
fn a_copied_index_entry_comes_with_its_text() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::references::index::{MarkEntryDialog, WordIndex};
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "契約 ");
    document
        .select((paragraph, 0), (paragraph, "契約".len()))
        .unwrap();
    document
        .mark_index_entry(&MarkEntryDialog::for_text("契約"))
        .unwrap();
    assert_eq!(document.index_entries().len(), 1);

    let end = document.caret_text(paragraph).unwrap().find(' ').unwrap();
    document.select((paragraph, 0), (paragraph, end)).unwrap();
    let clip = document.copy_rich().unwrap();
    document.place_caret(paragraph, end);
    document.paste_rich(&clip).unwrap();
    assert!(document
        .caret_text(paragraph)
        .unwrap()
        .starts_with("契約契約 "));
    assert_eq!(document.index_entries().len(), 2);
    document.undo().unwrap();
    assert_eq!(document.index_entries().len(), 1);
}
