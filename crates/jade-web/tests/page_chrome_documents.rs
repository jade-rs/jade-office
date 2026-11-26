//! A footer's `PAGE of NUMPAGES` and a header's STYLEREF as the browser
//! build draws them: a footer on page 1 is drawn before the last page is
//! reached, and a header before the body below it, so the pass that drew
//! one wrong is followed by one that draws it with the totals and headings
//! the layout ended with — at most a couple, for a field that moves what it
//! shows, and none for a keystroke on a page already drawn right.

mod common;

use jade_web::editing::{EditableDocument, WordFormatting};
use jade_web::word::new::{blank_document, Blank};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

const FILLER: &str = "The quick brown fox jumps over the lazy dog while the band \
     redraw keeps every other pixel of the page exactly as it was painted.";

const STYLES: &str = r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>"#;

const LETTER: &str = r#"<w:pgSz w:w="12240" w:h="15840"/>"#;
const A4: &str = r#"<w:pgSz w:w="11906" w:h="16838"/>"#;

/// A document of `body` on pages of `size`, with a header holding `header`
/// and a footer holding `footer` (paragraph content) when given.
fn docx(body: &str, size: &str, header: Option<&str>, footer: Option<&str>) -> Vec<u8> {
    let mut package = jade_opc::OpcPackage::open(&blank_document(Blank::English)).unwrap();
    let mut references = String::new();
    let mut rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    let mut types = package.part_str("[Content_Types].xml").unwrap().unwrap();
    for (story, tag, content) in [("header", "hdr", header), ("footer", "ftr", footer)] {
        let Some(content) = content else { continue };
        package.replace(
            &format!("word/{story}1.xml"),
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:{tag} xmlns:w="{W}"><w:p>{content}</w:p></w:{tag}>"#)
                .into_bytes(),
        );
        references.push_str(&format!(
            r#"<w:{story}Reference w:type="default" r:id="chrome{story}"/>"#
        ));
        rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="chrome{story}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{story}" Target="{story}1.xml"/></Relationships>"#));
        types = types.replace("</Types>", &format!(r#"<Override PartName="/word/{story}1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.{story}+xml"/></Types>"#));
    }
    package.replace("word/_rels/document.xml.rels", rels.into_bytes());
    package.replace("[Content_Types].xml", types.into_bytes());
    package.replace(
        "word/document.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr>{references}{size}<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
        )
        .into_bytes(),
    );
    package.replace(
        "word/styles.xml",
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}">{STYLES}</w:styles>"#)
            .into_bytes(),
    );
    package.write().unwrap()
}

fn open_resident(bytes: &[u8], resident: std::ops::Range<usize>) -> EditableDocument {
    let fonts_dir = common::core().join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    EditableDocument::open_resident(bytes, 1.0, fonts, measurer, resident).unwrap()
}

fn pixels(document: &EditableDocument, page: usize) -> Vec<u8> {
    document.page(page).unwrap().rgba_premultiplied()
}

fn assert_same_pages(label: &str, document: &EditableDocument, reference: &EditableDocument) {
    assert_eq!(document.page_count(), reference.page_count(), "{label}");
    for page in 0..reference.page_count() {
        assert!(
            pixels(document, page) == pixels(reference, page),
            "{label}: page {page} differs"
        );
    }
}

/// The pixels of page `page` below its body (the 1in bottom margin).
fn footer_band(document: &EditableDocument, page: usize) -> Vec<u8> {
    let pixels = pixels(document, page);
    let width = document.page(page).unwrap().width() as usize;
    let rows = 96 * width * 4;
    pixels[pixels.len() - rows..].to_vec()
}

/// `Page X of N` as fields, their saved results both 1.
const PAGE_OF_NUMPAGES: &str = r#"<w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> of </w:t></w:r><w:fldSimple w:instr=" NUMPAGES "><w:r><w:t>1</w:t></w:r></w:fldSimple>"#;

/// What [`PAGE_OF_NUMPAGES`] reads on the first of three pages, as text.
const ONE_OF_THREE: &str = r#"<w:r><w:t>1</w:t></w:r><w:r><w:t xml:space="preserve"> of </w:t></w:r><w:r><w:t>3</w:t></w:r>"#;

/// Three pages, a page break ending each of the first two.
fn three_pages() -> String {
    let mut body = String::new();
    for page in 0..3 {
        body.push_str(&format!("<w:p><w:r><w:t>Page {page}</w:t></w:r></w:p>"));
        if page < 2 {
            body.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
        }
    }
    body
}

#[test]
fn page_one_of_a_fresh_open_reads_the_settled_total() {
    for (name, size) in [("Letter", LETTER), ("A4", A4)] {
        let fields = docx(&three_pages(), size, None, Some(PAGE_OF_NUMPAGES));
        let text = docx(&three_pages(), size, None, Some(ONE_OF_THREE));
        let reference = common::open_bytes(&text);
        // Every page resident, and only the first: the total is drawn from
        // the end of the layout either way.
        for resident in [0..usize::MAX, 0..1] {
            let mut document = open_resident(&fields, resident.clone());
            assert_eq!(document.page_count(), 3, "{name}");
            assert!(
                footer_band(&document, 0) == footer_band(&reference, 0),
                "{name}, resident {resident:?}: page 1 does not read 1 of 3"
            );
            // The pages not resident at the open are drawn right when they
            // come in.
            document.set_resident_pages(0..3).unwrap();
            let settled = common::open_bytes(&fields);
            assert_same_pages(&format!("{name}, {resident:?}"), &document, &settled);
        }
    }
}

#[test]
fn pages_added_by_typing_are_counted_on_the_first_page() {
    for size in [LETTER, A4] {
        let mut document = common::open_bytes(&docx(
            "<w:p><w:r><w:t>Start</w:t></w:r></w:p>",
            size,
            None,
            Some(PAGE_OF_NUMPAGES),
        ));
        assert_eq!(document.page_count(), 1);
        // Paragraph breaks and words, each a bounded pass, until there are
        // three pages.
        document.place_caret(0, "Start".len());
        let mut typed = 0;
        while document.page_count() < 3 {
            document.insert_paragraph_break().unwrap();
            document.insert_text("Line").unwrap();
            typed += 1;
            assert!(typed < 400, "three pages never came");
        }
        // The footers catch up when the shell is idle.
        document.settle_pending().unwrap();
        assert!(!document.has_pending_settle());
        document.take_repainted_pages();
        let reference = docx(
            &(0..=typed)
                .map(|line| {
                    let text = if line == 0 { "Start" } else { "Line" };
                    format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
                })
                .collect::<String>(),
            size,
            None,
            Some(ONE_OF_THREE),
        );
        let reference = common::open_bytes(&reference);
        assert!(
            footer_band(&document, 0) == footer_band(&reference, 0),
            "page 1 does not read 1 of 3"
        );
        let fresh = common::open_bytes(&document.export_docx().unwrap());
        assert_same_pages("typed to three pages", &document, &fresh);
    }
}

const NOW_HEADER: &str = r#"<w:r><w:t xml:space="preserve">Now: </w:t></w:r><w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; "><w:r><w:t>stale</w:t></w:r></w:fldSimple>"#;

/// Eight chapters, each a heading and 18 paragraphs of text; `normal` lists
/// the chapters whose heading is a Normal paragraph instead.
fn chapters_docx(normal: &[usize]) -> Vec<u8> {
    let mut body = String::new();
    for chapter in 0..8 {
        let style = if normal.contains(&chapter) {
            "Normal"
        } else {
            "Heading1"
        };
        body.push_str(&format!(
            r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r><w:t>Chapter {chapter}</w:t></w:r></w:p>"#
        ));
        for index in 0..18 {
            body.push_str(&format!(
                "<w:p><w:r><w:t>{chapter}.{index} {FILLER}</w:t></w:r></w:p>"
            ));
        }
    }
    docx(&body, LETTER, Some(NOW_HEADER), Some(PAGE_OF_NUMPAGES))
}

/// The paragraph index of chapter `chapter`'s heading.
fn heading(chapter: usize) -> usize {
    chapter * 19
}

#[test]
fn a_fresh_open_draws_the_headings_of_each_page() {
    let bytes = chapters_docx(&[]);
    let document = common::open_bytes(&bytes);
    let mut reference = common::open_bytes(&bytes);
    // A second layout has every heading's page from the one before it.
    reference.set_formatting_marks(false).unwrap();
    reference.set_formatting_marks(true).unwrap();
    assert_same_pages("fresh open", &document, &reference);
}

#[test]
fn a_restyled_heading_is_drawn_in_the_headers_that_show_it() {
    let mut document = common::open_bytes(&chapters_docx(&[]));
    let reference = common::open_bytes(&chapters_docx(&[1]));
    document.place_caret(heading(1), 0);
    document.take_repainted_pages();
    document.set_paragraph_style(Some("Normal")).unwrap();
    assert_same_pages("restyled", &document, &reference);
    document.undo().unwrap();
    assert_same_pages(
        "undone",
        &document,
        &common::open_bytes(&chapters_docx(&[])),
    );
}

#[test]
fn a_retyped_or_moved_heading_is_drawn_in_the_headers_that_show_it() {
    // Retyped: the heading's own page is drawn in a bounded pass whose
    // header came before it, and the pages after it show it in headers the
    // pass did not draw again.
    let mut document = common::open_bytes(&chapters_docx(&[]));
    let page = document.paragraph_start_page(heading(1)).unwrap();
    assert!(page > 0, "chapter 1 starts after the first page");
    document.place_caret(heading(1), "Chapter".len());
    document.take_repainted_pages();
    let passes = document.settling_passes();
    document.insert_text("X").unwrap();
    // The keystroke draws its own page's band, and leaves the headers
    // showing the heading for the shell's idle settle: no whole-document
    // pass on the key.
    assert_eq!(document.settling_passes(), passes, "the key settled");
    assert!(document.has_pending_settle());
    assert!(document.settle_pending().unwrap());
    assert!(!document.has_pending_settle());
    let repainted = document.take_repainted_pages();
    assert!(repainted.contains(&page), "{repainted:?}");
    assert!(repainted.contains(&(page + 1)), "{repainted:?}");
    let fresh = common::open_bytes(&document.export_docx().unwrap());
    assert_same_pages("retyped", &document, &fresh);

    // Moved: paragraph breaks above it push it down the page, then onto
    // the next.
    let mut document = common::open_bytes(&chapters_docx(&[]));
    let page = document.paragraph_start_page(heading(1)).unwrap();
    document.place_caret(heading(1), 0);
    while document.paragraph_start_page(document.caret().unwrap().0) == Some(page) {
        document.insert_paragraph_break().unwrap();
    }
    document.settle_pending().unwrap();
    let fresh = common::open_bytes(&document.export_docx().unwrap());
    assert_same_pages("moved", &document, &fresh);
}

/// A header showing only `STYLEREF "heading 1" \l` above a heading A,
/// filler and a long heading B kept together: showing B, the header wraps
/// and pushes B to the next page, where the header shows A, shrinks, and
/// lets B back. No value is right; the passes that draw it stop all the
/// same.
#[test]
fn a_styleref_that_moves_its_own_heading_stops_repainting() {
    let header = r#"<w:fldSimple w:instr=" STYLEREF &quot;heading 1&quot; \l "><w:r><w:t>stale</w:t></w:r></w:fldSimple>"#;
    let mut settled_any = false;
    for filler in 10..34 {
        let mut body = String::from(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>A</w:t></w:r></w:p>"#,
        );
        for index in 0..filler {
            body.push_str(&format!("<w:p><w:r><w:t>Line {index}</w:t></w:r></w:p>"));
        }
        body.push_str(&format!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:keepLines/></w:pPr><w:r><w:t>B {FILLER} {FILLER} {FILLER} {FILLER}</w:t></w:r></w:p><w:p><w:r><w:t>After</w:t></w:r></w:p>"#
        ));
        let mut document = common::open_bytes(&docx(&body, LETTER, Some(header), None));
        let mut passes = document.settling_passes();
        assert!(passes <= 2, "filler {filler}: {passes} passes at the open");
        settled_any |= passes > 0;
        for edit in 0..3 {
            // An edit starts the count again, and ends the same way.
            document.place_caret(1, 0);
            document.insert_text("z").unwrap();
            assert_eq!(document.settling_passes(), passes, "the key settled");
            let mut idles = 0;
            while document.settle_pending().unwrap() {
                idles += 1;
                assert!(idles <= 2, "filler {filler}, edit {edit}: settles forever");
            }
            let after = document.settling_passes();
            assert!(
                after - passes <= 2,
                "filler {filler}, edit {edit}: {} passes",
                after - passes
            );
            passes = after;
            document.set_formatting_marks(edit % 2 == 0).unwrap();
            let after = document.settling_passes();
            assert!(
                after - passes <= 2,
                "filler {filler}, toggle {edit}: {} passes",
                after - passes
            );
            passes = after;
        }
    }
    assert!(settled_any, "no STYLEREF was ever drawn wrong");
}

#[test]
fn typing_on_a_page_drawn_right_runs_no_extra_pass() {
    let mut document = common::open_bytes(&chapters_docx(&[]));
    let before = document.settling_passes();
    // A body paragraph on the second page, under the STYLEREF header and
    // above the NUMPAGES footer.
    let paragraph = heading(1) + 2;
    let length = document.caret_text(paragraph).unwrap().len();
    document.place_caret(paragraph, length);
    for character in "typed on a settled page".chars() {
        document.insert_text(&character.to_string()).unwrap();
    }
    document.delete(true).unwrap();
    assert!(!document.has_pending_settle());
    assert!(!document.settle_pending().unwrap());
    assert_eq!(document.settling_passes(), before);
    let fresh = common::open_bytes(&document.export_docx().unwrap());
    assert_same_pages("typed", &document, &fresh);

    // And on a document with no fields at all.
    let mut contract = common::contract();
    let before = contract.settling_passes();
    common::typed(&mut contract, "typed");
    for character in "and more".chars() {
        contract.insert_text(&character.to_string()).unwrap();
    }
    assert!(!contract.has_pending_settle());
    assert_eq!(contract.settling_passes(), before);
}

/// Page 1 of `document` against a fresh render of it saved, once its
/// footers have settled.
fn assert_first_page_settled(label: &str, document: &mut EditableDocument) {
    document.settle_pending().unwrap();
    assert!(!document.has_pending_settle(), "{label}");
    let fresh = common::open_bytes(&document.export_docx().unwrap());
    assert_eq!(document.page_count(), fresh.page_count(), "{label}");
    assert!(
        pixels(document, 0) == pixels(&fresh, 0),
        "{label}: page 1 differs from a fresh render"
    );
}

/// The total comes down again: a document typed out to four pages with
/// Enter and taken back to one, by Backspace and by undo, reads its page
/// count on page 1 at every step.
#[test]
fn the_total_follows_the_page_count_down() {
    for (name, resident) in [("all resident", 0..usize::MAX), ("page 1 resident", 0..1)] {
        for by_undo in [false, true] {
            let label = format!("{name}, {}", if by_undo { "undo" } else { "backspace" });
            let bytes = docx(
                "<w:p><w:r><w:t>Start</w:t></w:r></w:p>",
                LETTER,
                None,
                Some(PAGE_OF_NUMPAGES),
            );
            let mut document = open_resident(&bytes, resident.clone());
            document.place_caret(0, "Start".len());
            let mut count = document.page_count();
            let mut typed = 0;
            while document.page_count() < 4 {
                document.insert_paragraph_break().unwrap();
                document.insert_text("Line").unwrap();
                typed += 1;
                assert!(typed < 600, "{label}: four pages never came");
                if document.page_count() != count {
                    count = document.page_count();
                    assert_first_page_settled(&format!("{label}, up to {count}"), &mut document);
                }
            }
            let mut steps = 0;
            while document.page_count() > 1 {
                if by_undo {
                    assert!(document.undo().unwrap(), "{label}: nothing to undo");
                } else {
                    document.delete(true).unwrap();
                }
                steps += 1;
                assert!(steps < 6000, "{label}: one page never came back");
                if document.page_count() != count {
                    count = document.page_count();
                    assert_first_page_settled(&format!("{label}, down to {count}"), &mut document);
                }
            }
            assert_first_page_settled(&format!("{label}, back to one"), &mut document);
        }
    }
}

mod settle_timing {
    use std::time::Duration;

    use jade_web::editing::settle_wait;

    const QUIET: Duration = Duration::from_millis(300);
    const MAX_WAIT: Duration = Duration::from_millis(1500);

    fn wait(first_pending: f64, last_edit: f64, now: f64) -> f64 {
        let left = settle_wait(first_pending, last_edit, now, QUIET, MAX_WAIT);
        (left.as_secs_f64() * 1000.0).round()
    }

    #[test]
    fn a_pause_after_the_last_edit_settles() {
        // One edit at 0: due 300 ms on.
        assert_eq!(wait(0.0, 0.0, 0.0), 300.0);
        assert_eq!(wait(0.0, 0.0, 200.0), 100.0);
        assert_eq!(wait(0.0, 0.0, 300.0), 0.0);
        assert_eq!(wait(0.0, 0.0, 5000.0), 0.0);
    }

    #[test]
    fn each_edit_starts_the_wait_again() {
        // Keys at 0, 250 and 500: due 300 ms after the last.
        assert_eq!(wait(0.0, 250.0, 300.0), 250.0);
        assert_eq!(wait(0.0, 500.0, 600.0), 200.0);
        assert_eq!(wait(0.0, 500.0, 800.0), 0.0);
    }

    #[test]
    fn edits_that_never_pause_wait_no_longer_than_the_cap() {
        // A key every 100 ms from 0: due 1.5 s after the first pending
        // frame all the same.
        assert_eq!(wait(0.0, 1300.0, 1300.0), 200.0);
        assert_eq!(wait(0.0, 1400.0, 1450.0), 50.0);
        assert_eq!(wait(0.0, 1500.0, 1500.0), 0.0);
        assert_eq!(wait(0.0, 9000.0, 9000.0), 0.0);
    }
}
