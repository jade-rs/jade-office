//! 校閲 › アクセシビリティ チェック and 代替テキスト on the browser build's
//! Word document — the commands the native app runs too: a picture without
//! alternative text, a table without a header row, a heading level
//! skipped, a link whose text is its address and faint text are each found,
//! and the quick fixes and the 代替テキスト edit clear them.

mod common;
use common::open_bytes;
use jade_web::editing::EditableDocument;
use jade_web::word::accessibility::{AltText, IssueKind, Severity, WordAccessibility};
use jade_web::word::new::{blank_document, Blank};
use jade_web::word::picture::WordPicture;
use jade_web::word::{LinkDraft, WordFormatting, WordLinks};

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap()
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn end_of(document: &EditableDocument, paragraph: usize) -> usize {
    document.paragraph_text(paragraph).unwrap().len()
}

/// A paragraph of `text` after the last one, the caret at its end.
fn line(document: &mut EditableDocument, text: &str) -> usize {
    let last = document.paragraph_count() - 1;
    let end = end_of(document, last);
    document.place_caret(last, end);
    if end > 0 || last > 0 {
        document.insert_paragraph_break().unwrap();
    }
    let paragraph = document.paragraph_count() - 1;
    document.insert_text(text).unwrap();
    paragraph
}

fn kinds(document: &EditableDocument) -> Vec<IssueKind> {
    document
        .accessibility_issues()
        .into_iter()
        .map(|issue| issue.kind)
        .collect()
}

#[test]
fn the_checker_finds_what_word_does_and_the_fixes_clear_it() {
    let mut document = open_bytes(&blank_document(Blank::English));
    assert!(document.accessibility_issues().is_empty());

    // A heading 1, then a heading 3.
    let first = line(&mut document, "Overview");
    document.place_caret(first, 0);
    document.set_paragraph_style(Some("Heading1")).unwrap();
    let third = line(&mut document, "Details");
    document.place_caret(third, 0);
    document.set_paragraph_style(Some("Heading3")).unwrap();

    // A link whose text is its address.
    let link = line(&mut document, "https://example.com");
    let draft = LinkDraft {
        display: "https://example.com".into(),
        address: String::new(),
        range: ((link, 0), (link, end_of(&document, link))),
        editing: false,
    };
    document
        .apply_link(&draft, "https://example.com", "https://example.com")
        .unwrap();

    // Light grey text.
    let faint = line(&mut document, "Faint words");
    document
        .select((faint, 0), (faint, end_of(&document, faint)))
        .unwrap();
    document.set_font_color("BBBBBB").unwrap();

    // A picture.
    let picture = line(&mut document, "Picture: ");
    document.place_caret(picture, end_of(&document, picture));
    document.insert_picture(png()).unwrap();

    // A table of two rows.
    let table_line = line(&mut document, "");
    document.place_caret(table_line, 0);
    document.insert_table(2, 2).unwrap();

    let found = kinds(&document);
    let has = |test: &dyn Fn(&IssueKind) -> bool| found.iter().any(test);
    assert!(
        has(&|kind| matches!(kind, IssueKind::MissingAltText(_))),
        "{found:?}"
    );
    assert!(
        has(&|kind| matches!(kind, IssueKind::NoHeaderRow { .. })),
        "{found:?}"
    );
    assert!(
        has(&|kind| matches!(kind, IssueKind::SkippedHeading { from: 1, to: 3 })),
        "{found:?}"
    );
    assert!(
        has(&|kind| *kind == IssueKind::UnclearLinkText),
        "{found:?}"
    );
    assert!(
        has(&|kind| matches!(kind, IssueKind::LowContrast { .. })),
        "{found:?}"
    );
    // Errors come first.
    let issues = document.accessibility_issues();
    assert_eq!(issues[0].severity(), Severity::Error);

    // 代替テキスト on the picture; the table's quick fix.
    let target = issues
        .iter()
        .find_map(|issue| match issue.kind {
            IssueKind::MissingAltText(target) => Some(target),
            _ => None,
        })
        .unwrap();
    document
        .set_alt_text(
            target,
            AltText {
                description: Some("A red and a blue pixel".into()),
                decorative: false,
            },
        )
        .unwrap();
    let table = issues
        .iter()
        .find(|issue| matches!(issue.kind, IssueKind::NoHeaderRow { .. }))
        .unwrap()
        .clone();
    document.quick_fix(&table).unwrap();
    let found = kinds(&document);
    assert!(!found.iter().any(|kind| matches!(
        kind,
        IssueKind::MissingAltText(_) | IssueKind::NoHeaderRow { .. }
    )));
    assert_eq!(
        document.alt_text(target).unwrap().description.as_deref(),
        Some("A red and a blue pixel")
    );
    // Saved, the description is the drawing's; undone, it is gone again.
    let saved = document.export_docx().unwrap();
    let xml = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap();
    assert!(xml.contains(r#"descr="A red and a blue pixel""#), "{xml}");
    document.undo().unwrap();
    document.undo().unwrap();
    assert!(kinds(&document)
        .iter()
        .any(|kind| matches!(kind, IssueKind::MissingAltText(_))));

    // 装飾用にする, the quick fix for a picture.
    let issue = document
        .accessibility_issues()
        .into_iter()
        .find(|issue| matches!(issue.kind, IssueKind::MissingAltText(_)))
        .unwrap();
    document.quick_fix(&issue).unwrap();
    assert!(document.alt_text(target).unwrap().decorative);
}
