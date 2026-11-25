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
fn a_page_break_at_the_caret_splits_the_paragraph_ahead_of_the_next_page() {
    let mut document = contract();
    let paragraph = typed(&mut document, "ab");
    let paragraphs = document.paragraph_count();
    document.place_caret(paragraph, 1);
    document.insert_page_break(PageBreak::Page).unwrap();
    assert_eq!(document.paragraph_count(), paragraphs + 2);
    assert_eq!(document.paragraph_text(paragraph).as_deref(), Some("a"));
    assert_eq!(
        document.paragraph_text(paragraph + 1).as_deref(),
        Some("\u{000C}")
    );
    assert!(document
        .paragraph_text(paragraph + 2)
        .unwrap()
        .starts_with('b'));
    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"<w:br w:type="page"/>"#));
    document.undo().unwrap();
    assert_eq!(document.paragraph_count(), paragraphs);
    assert!(document
        .paragraph_text(paragraph)
        .unwrap()
        .starts_with("ab"));
}

#[test]
fn a_column_break_moves_the_text_to_the_next_column() {
    let mut document = contract();
    let paragraph = typed(&mut document, "left|right ");
    document.place_caret(paragraph, 0);
    document.set_column_preset(ColumnPreset::Two).unwrap();
    let before = document.place_caret(paragraph, "left|".len()).unwrap();
    document.insert_page_break(PageBreak::Column).unwrap();
    let text = document.paragraph_text(paragraph).unwrap();
    let after_break = text.find('\u{000E}').unwrap() + '\u{000E}'.len_utf8();
    let after = document.place_caret(paragraph, after_break).unwrap();
    let (width, _) = document.page_size();
    assert!(
        after.x > width / 2.0 && after.x > before.x,
        "the text after the break is in the right column: {before:?} → {after:?}"
    );
    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"<w:br w:type="column"/>"#), "{xml}");
}

#[test]
fn a_book_fold_shows_half_sheet_pages() {
    let mut document = contract();
    let (width, height) = document.page_size();
    let mut dialog = document.page_setup_dialog(SetupTab::Margins);
    dialog.apply_to = ApplyTo::WholeDocument;
    dialog.set_multiple_pages(jade_web::word::page_setup::MultiplePages::BookFold);
    document.apply_page_setup(&dialog).unwrap();
    let (folded_width, folded_height) = document.page_size();
    assert!(
        (folded_width - height.max(width) / 2.0).abs() < 1.0
            && (folded_height - width.min(height)).abs() < 1.0,
        "{width}x{height} → {folded_width}x{folded_height}"
    );
    let settings = part(&document.export_docx().unwrap(), "word/settings.xml");
    assert!(settings.contains("w:bookFoldPrinting"));

    let mut dialog = document.page_setup_dialog(SetupTab::Margins);
    dialog.apply_to = ApplyTo::WholeDocument;
    dialog.set_multiple_pages(jade_web::word::page_setup::MultiplePages::TwoOnOne);
    dialog.set_landscape(false);
    document.apply_page_setup(&dialog).unwrap();
    let (two_up_width, two_up_height) = document.page_size();
    assert!(
        two_up_width > two_up_height,
        "an upright sheet carries two wide pages"
    );
}

#[test]
fn a_paragraph_leaves_the_grid() {
    let mut document = contract();
    let paragraph = typed(&mut document, "x");
    document.place_caret(paragraph, 0);
    assert!(document.snap_to_grid());
    document.set_snap_to_grid(false).unwrap();
    assert!(!document.snap_to_grid());
    let xml = part(&document.export_docx().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"<w:snapToGrid w:val="0"/>"#));
    document.undo().unwrap();
    assert!(document.snap_to_grid());
}

#[test]
fn page_borders_color_and_a_cover_page() {
    use jade_web::word::page_setup::{BorderSetting, CoverPage, PageBorderDialog};
    let mut document = contract();
    let paragraphs = document.paragraph_count();

    let setup = document.page_setup_dialog(SetupTab::Other);
    let mut borders = PageBorderDialog::open(&setup);
    borders.set_setting(BorderSetting::Box);
    borders.style = "double".to_string();
    borders.width = 1.5;
    borders.color = "C00000".to_string();
    borders.offset_from = "page".to_string();
    document.apply_page_borders(&borders).unwrap();
    let frame = document.section_layout().page_borders;
    assert_eq!(frame.top.as_ref().unwrap().style.as_deref(), Some("double"));
    assert!(frame.left.is_some() && frame.right.is_some() && frame.bottom.is_some());

    document.set_page_color(Some("FFF9E6")).unwrap();
    assert_eq!(document.page_color().as_deref(), Some("FFF9E6"));

    document.insert_cover_page(CoverPage::Banner, true).unwrap();
    assert!(document.paragraph_count() > paragraphs);
    assert!(document
        .paragraph_text(0)
        .unwrap()
        .contains("[文書のタイトル]"));
    assert!(
        document.section_layout().different_first_page,
        "the cover page keeps the header and footer off it"
    );

    let bytes = document.export_docx().unwrap();
    let xml = part(&bytes, "word/document.xml");
    assert!(xml.contains("w:pgBorders"), "{xml}");
    assert!(xml.contains(r#"w:offsetFrom="page""#));
    assert!(xml.contains(r#"w:val="double""#));
    assert!(xml.contains("FFF9E6"));
    for _ in 0..3 {
        document.undo().unwrap();
    }
    assert!(document.section_layout().page_borders.is_empty());
    assert_eq!(document.paragraph_count(), paragraphs);
}

#[test]
fn the_ribbon_shows_and_sets_the_caret_section() {
    use jade_web::word::page::PageCommand;
    let mut document = contract();
    let paragraph = typed(&mut document, "one|two ");
    document.place_caret(paragraph, "one|".len());
    document
        .insert_section_break(SectionBreak::NextPage)
        .unwrap();
    document.place_caret(0, 0);
    let first = document.page_setup();
    document
        .page_command(PageCommand::Landscape(!first.landscape), true)
        .unwrap();
    assert_eq!(document.page_setup().landscape, !first.landscape);
    // The second section keeps its page, and the ribbon shows it there.
    document.place_caret(paragraph + 1, 0);
    assert_eq!(document.page_setup().landscape, first.landscape);
}

#[test]
fn paragraph_grid_can_override_an_off_style_and_survives_history_and_reopen() {
    use common::open_bytes;
    use jade_office_model::word::new::{blank_document, Blank};
    use jade_office_model::word::package::Package;

    let mut package = Package::open(&blank_document(Blank::Japanese)).unwrap();
    let main = package.main().to_string();
    let styles = package.related(&main, "styles").unwrap();
    package.put_bytes(&styles, br#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="GridOff"><w:name w:val="Grid off"/><w:pPr><w:snapToGrid w:val="0"/></w:pPr></w:style></w:styles>"#.to_vec());
    package.put_bytes(&main, br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="GridOff"/><w:keepNext/></w:pPr><w:r><w:t>First</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="GridOff"/></w:pPr><w:r><w:t>Second</w:t></w:r></w:p></w:body></w:document>"#.to_vec());
    let mut document = open_bytes(&package.write().unwrap());
    document.place_caret(0, 0);
    assert!(!document.snap_to_grid());
    let before = document.export_docx().unwrap();
    document.set_snap_to_grid(true).unwrap();
    assert!(document.snap_to_grid());
    let enabled = document.export_docx().unwrap();
    let xml = part(&enabled, "word/document.xml");
    assert!(xml.contains(r#"<w:snapToGrid w:val="1"/>"#), "{xml}");
    assert!(xml.contains("<w:keepNext/>"));
    let mut reopened = open_bytes(&enabled);
    reopened.place_caret(0, 0);
    assert!(reopened.snap_to_grid());
    reopened.place_caret(1, 0);
    assert!(!reopened.snap_to_grid());
    document.undo().unwrap();
    assert!(!document.snap_to_grid());
    assert_eq!(document.export_docx().unwrap(), before);
    document.redo().unwrap();
    assert!(document.snap_to_grid());
    assert_eq!(document.export_docx().unwrap(), enabled);
    document.set_snap_to_grid(false).unwrap();
    let mut reopened = open_bytes(&document.export_docx().unwrap());
    reopened.place_caret(0, 0);
    assert!(!reopened.snap_to_grid());
}

/// A section turned landscape among portrait ones: each page keeps its own
/// box, and the document's coordinates count each page at its own height,
/// so a click on a portrait page after a landscape one lands where it is.
#[test]
fn a_landscape_section_has_pages_of_its_own_size() {
    use jade_web::word::page::PageCommand;
    let bytes =
        std::fs::read(common::core().join("public/fixtures/header-footer-sections.docx")).unwrap();
    let mut document = common::open_bytes(&bytes);
    let portrait = document.page_size_of(0);
    assert!(portrait.1 > portrait.0);
    document.place_caret(0, 0);
    document
        .page_command(PageCommand::Landscape(true), true)
        .unwrap();
    assert_eq!(document.page_count(), 2);
    let first = document.page_size_of(0);
    let second = document.page_size_of(1);
    assert!(first.0 > first.1, "section 1 is landscape: {first:?}");
    assert!(second.1 > second.0, "section 2 stays portrait: {second:?}");
    // Page 2 starts below page 1's short height, not a portrait one.
    let top = document.document_y(1, 0.0);
    assert!(top > first.1 && top < portrait.1, "{top}");
    assert_eq!(document.page_and_offset(top + 10.0), (1, 10.0));
    // A click on page 2's text puts the caret there.
    let text = (0..document.paragraph_count())
        .find(|&index| document.paragraph_text(index).as_deref() == Some("Section two paragraph 1"))
        .unwrap();
    let caret = document.place_caret(text, 0).unwrap();
    assert_eq!(document.page_and_offset(caret.document_y).0, 1);
    document.place_caret(0, 0);
    assert_eq!(
        document.place_caret_at(caret.x + 1.0, caret.document_y + caret.height / 2.0),
        document.caret_rect()
    );
    assert_eq!(document.caret().map(|(paragraph, _)| paragraph), Some(text));
}

/// A paragraph's mark carries the section break it ends: a copy of it
/// pastes a section set up as the copied one (landscape here), and the
/// section it lands in goes on starting as it did, as Word pastes it. One
/// undo takes it back.
#[test]
fn a_copied_section_break_pastes_with_its_page_setup() {
    use jade_web::editing::WordSession;
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::{
        HeadlessCommand, HeadlessSectionLayoutPatch, HeadlessSectionScope,
    };
    use jade_web::word::page::{SectionBreak, WordPage};
    let mut document = common::contract();
    let paragraph = common::typed(&mut document, "landscape");
    document.place_caret(paragraph, "landscape".len());
    document
        .insert_section_break(SectionBreak::Continuous)
        .unwrap();
    document
        .apply_commands(
            vec![HeadlessCommand::SetSectionLayout {
                scope: HeadlessSectionScope::Index { index: 0 },
                patch: Box::new(HeadlessSectionLayoutPatch {
                    landscape: Some(true),
                    ..Default::default()
                }),
            }],
            false,
        )
        .unwrap();
    let sections = |document: &jade_web::editing::EditableDocument| {
        let word = document.word_document();
        (0..word.section_count())
            .map(|index| {
                let layout = word.section_layout(Some(index));
                (layout.landscape, layout.start)
            })
            .collect::<Vec<_>>()
    };
    let before = sections(&document);
    assert_eq!(before.len(), 2);
    assert!(before[0].0 && !before[1].0);

    document.select((paragraph, 0), (paragraph + 1, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    let last = document.paragraph_count() - 1;
    let end = document.word_document().rendered_paragraph_len(last);
    document.place_caret(last, end);
    document.insert_paragraph_break().unwrap();
    let landing = document.paragraph_count() - 1;
    document.place_caret(landing, 0);
    document.paste_rich(&clip).unwrap();
    let after = sections(&document);
    // The copied section again, and the rest of the one it landed in
    // starting as it did.
    assert_eq!(
        after,
        [before[0].clone(), before[0].clone(), before[1].clone()]
    );
    assert_eq!(after[2].1, "continuous");
    document.undo().unwrap();
    assert_eq!(sections(&document).len(), 2);
}
