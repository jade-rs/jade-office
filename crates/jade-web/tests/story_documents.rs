//! Headers and footers edited in place (`WordDoc`): opened over a page,
//! typed into with every tool the body has — several paragraphs, fields, a
//! table — drawn over the page they belong to, and written back as one undo
//! step of the document when closed.

mod common;

use common::contract;
use jade_web::editing::{StoryChange, WordDoc, WordFormatting, WordSession};
use jade_web::word::headless::HeadlessFieldKind;
use jade_web::word::page::Story;
use jade_web::word::WordReferences;

#[test]
fn print_updates_a_toc_sharing_a_paragraph_with_body_text() {
    use jade_web::word::printing::PrintSettings;
    let bytes = std::fs::read(common::core().join("public/fixtures/toc.docx")).unwrap();
    let mut document = WordDoc::new(common::open_bytes(&bytes));
    let before = document.word_docx().unwrap();
    let selection = document.selection_range();
    let undo = document.can_undo();
    let dirty = document.is_dirty();
    let settings = PrintSettings {
        update_fields: true,
        ..Default::default()
    };
    let preview = document.print_preview(&settings, 0).unwrap();
    assert!(!preview.sheets.is_empty());
    assert_eq!(document.word_docx().unwrap(), before);
    assert_eq!(document.selection_range(), selection);
    assert_eq!(document.can_undo(), undo);
    assert_eq!(document.is_dirty(), dirty);
    document.update_fields_for_print().unwrap();
    let after = document.word_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&after).unwrap();
    assert!(package
        .part_str("word/document.xml")
        .unwrap()
        .unwrap()
        .contains("または、又は"));
    assert!(!document.print_pdf(&settings, 0).unwrap().is_empty());
    document.undo().unwrap();
    assert_eq!(document.word_docx().unwrap(), before);
    document.redo().unwrap();
    assert_eq!(document.word_docx().unwrap(), after);
}

#[test]
fn selection_printing_uses_the_open_story_instead_of_the_body_selection() {
    use jade_web::word::printing::{self, PrintRange, PrintSettings};

    let settings = PrintSettings {
        range: PrintRange::Selection,
        ..Default::default()
    };
    for role in [Story::Header, Story::Footer] {
        let mut main = contract();
        main.select((3, 0), (3, 3)).unwrap();
        let body_selection = main.selection_range();
        let body_pdf = main.print_pdf(&settings, 0).unwrap();
        let mut document = WordDoc::new(main);
        document.open_story(0, role).unwrap();
        document.insert_text("before SELECTED after").unwrap();
        document.select((0, 7), (0, 15)).unwrap();
        let selection = document.selection_range();
        let before = document.word_docx().unwrap();
        let story_bytes = document.story().unwrap().document.word_docx().unwrap();
        let selected_bytes = printing::selection_docx(&story_bytes, (0, 7), (0, 15)).unwrap();
        let mut selected = common::open_bytes(&selected_bytes);
        assert_eq!(selected.paragraph_text(0).as_deref(), Some("SELECTED"));
        let expected = selected.print_pdf(&PrintSettings::default(), 0).unwrap();

        assert_eq!(
            document.print_preview(&settings, 0).unwrap(),
            selected
                .print_preview(&PrintSettings::default(), 0)
                .unwrap()
        );
        assert_eq!(
            document
                .print_preview(
                    &PrintSettings {
                        update_fields: true,
                        ..settings.clone()
                    },
                    0
                )
                .unwrap(),
            selected
                .print_preview(&PrintSettings::default(), 0)
                .unwrap()
        );
        assert_eq!(
            document
                .prepare_print_preview(&settings, 0)
                .unwrap()
                .render_sheet(0)
                .unwrap()
                .rgba,
            selected
                .prepare_print_preview(&PrintSettings::default(), 0)
                .unwrap()
                .render_sheet(0)
                .unwrap()
                .rgba
        );
        let pdf = document.print_pdf(&settings, 0).unwrap();
        assert_eq!(pdf, expected);
        assert_ne!(pdf, body_pdf);
        assert_eq!(document.word_docx().unwrap(), before);
        assert_eq!(document.selection_range(), selection);
        assert_eq!(document.main().selection_range(), body_selection);
        assert_eq!(document.story().unwrap().role, role);
        assert!(document.is_dirty());
        assert!(document.can_undo());

        // A collapsed story selection must not fall back to the old body
        // selection or to the entire document.
        document.place_caret(0, 7);
        assert!(document.print_pdf(&settings, 0).is_err());
        assert!(document.print_preview(&settings, 0).is_err());
    }
}

#[test]
fn print_page_labels_stay_with_the_body_while_a_story_is_open() {
    use jade_web::word::page_setup::{PageBreak, WordPageSetup};

    for role in [Story::Header, Story::Footer] {
        let mut main = contract();
        main.place_caret(3, 0);
        main.insert_page_break(PageBreak::Page).unwrap();
        let labels = main.page_labels();
        assert!(labels.len() > 1);
        let mut document = WordDoc::new(main);
        document.open_story(0, role).unwrap();
        assert_eq!(document.story().unwrap().document.page_labels().len(), 1);
        assert_eq!(document.page_labels(), labels);
        assert_eq!(document.page_labels().len(), document.page_count());
    }
}

#[test]
fn print_updates_the_body_toc_while_a_story_is_open() {
    for role in [Story::Header, Story::Footer] {
        let mut main = contract();
        let heading = common::typed(&mut main, "Existing print heading");
        main.place_caret(heading, 0);
        main.set_paragraph_style(Some("Heading1")).unwrap();
        main.place_caret(0, 0);
        main.insert_table_of_contents().unwrap();
        let last = main.paragraph_count() - 1;
        let end = main.paragraph_text(last).unwrap().len();
        main.place_caret(last, end);
        main.insert_paragraph_break().unwrap();
        main.insert_text("New print heading").unwrap();
        main.set_paragraph_style(Some("Heading1")).unwrap();
        let body_text = |document: &jade_web::editing::EditableDocument| {
            (0..document.paragraph_count())
                .filter_map(|index| document.paragraph_text(index))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let before = body_text(&main);
        assert_eq!(before.matches("New print heading").count(), 1);
        let mut document = WordDoc::new(main);
        document.open_story(0, role).unwrap();
        document.insert_text("Keep this story edit").unwrap();
        let story_before = document.story().unwrap().document.word_docx().unwrap();
        let caret = document.caret();

        document.update_fields_for_print().unwrap();

        assert_eq!(
            body_text(document.main())
                .matches("New print heading")
                .count(),
            2
        );
        assert_eq!(document.story().unwrap().role, role);
        assert_eq!(document.caret(), caret);
        assert_eq!(
            document.story().unwrap().document.word_docx().unwrap(),
            story_before
        );
        assert!(document.story().unwrap().document.is_dirty());
        // Updating the body is one body undo step and leaves the story's
        // undo stack independent and usable.
        document.main_mut().undo().unwrap();
        assert_eq!(body_text(document.main()), before);
        document.undo().unwrap();
        assert!(!document
            .paragraph_text(0)
            .unwrap()
            .contains("Keep this story edit"));
    }
}

/// The parts the saved document has whose name starts with `prefix`,
/// joined.
fn parts(document: &mut WordDoc, prefix: &str) -> String {
    let bytes = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&bytes).unwrap();
    package
        .part_names()
        .iter()
        .filter(|name| name.starts_with(prefix))
        .filter_map(|name| package.part_str(name).ok().flatten())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_header_opens_over_its_page_and_takes_paragraphs() {
    let mut document = WordDoc::new(contract());
    let body_caret = (3, 0);
    document.place_caret(body_caret.0, body_caret.1);
    let plain = document.page(0).unwrap().rgba_premultiplied();

    document.open_story(0, Story::Header).unwrap();
    let story = document.story().unwrap();
    assert_eq!((story.page, story.role), (0, Story::Header));
    // The page is drawn with the body washed out behind the header.
    assert_ne!(document.page(0).unwrap().rgba_premultiplied(), plain);
    assert!(document.take_repainted_pages().contains(&0));
    // The caret is in the header, on page 1, above the body.
    let caret = document.caret_rect().unwrap();
    let (page, y) = document.page_and_offset(caret.document_y);
    assert_eq!(page, 0);
    assert!(document.in_story(0, y));
    // A click in the band lands in the story; one in the body does not.
    assert!(document.position_at(caret.x, caret.document_y).is_some());
    assert!(document
        .position_at(100.0, document.document_y(0, 500.0))
        .is_none());

    document.insert_text("first line").unwrap();
    document.insert_paragraph_break().unwrap();
    document.insert_text("second line").unwrap();
    assert!(document.is_dirty());
    // Saving while it is open writes what was typed.
    let saved = parts(&mut document, "word/header");
    assert!(saved.contains("first line") && saved.contains("second line"));

    assert!(document.close_story().unwrap());
    assert!(document.story().is_none());
    assert_eq!(document.caret(), Some(body_caret));
    let headers = parts(&mut document, "word/header");
    assert!(headers.contains("first line") && headers.contains("second line"));
    // One undo step of the document takes the whole edit back.
    document.undo().unwrap();
    assert!(!parts(&mut document, "word/header").contains("second line"));
}

#[test]
fn the_footer_takes_fields_a_table_and_formatting() {
    let mut document = WordDoc::new(contract());
    document.open_story(0, Story::Footer).unwrap();
    let today = "2026/10/22";
    for kind in [
        HeadlessFieldKind::Page,
        HeadlessFieldKind::NumPages,
        HeadlessFieldKind::Date,
        HeadlessFieldKind::FileName,
    ] {
        let cached = document.story_field_text(kind, "contract.docx", today, "見出し 1");
        document.insert_story_field(kind, cached).unwrap();
        document.insert_text(" ").unwrap();
    }
    document.insert_table(1, 2).unwrap();
    document.close_story().unwrap();
    let footers = parts(&mut document, "word/footer");
    for code in ["PAGE", "NUMPAGES", "DATE", "FILENAME"] {
        assert!(footers.contains(code), "{code} missing");
    }
    assert!(footers.contains("<w:tbl>"));
}

#[test]
fn a_different_first_page_and_the_distance_change_the_section() {
    let mut document = WordDoc::new(contract());
    document.open_story(0, Story::Header).unwrap();
    let before = document.story_layout().unwrap();
    document
        .change_story(StoryChange::DifferentFirst(!before.different_first_page))
        .unwrap();
    let story = document.story().unwrap();
    assert_eq!(
        story.variant,
        if before.different_first_page {
            "default"
        } else {
            "first"
        }
    );
    document.change_story(StoryChange::Distance(true)).unwrap();
    let after = document.story_layout().unwrap();
    assert!((after.header_distance - before.header_distance - 72.0 / 25.4).abs() < 0.01);
    assert_eq!(after.different_first_page, !before.different_first_page);
    document.close_story().unwrap();
    // Each change was its own step of the document.
    document.undo().unwrap();
    document.undo().unwrap();
    let restored = document.main().word_document().section_layout(Some(0));
    assert_eq!(restored.different_first_page, before.different_first_page);
}

#[test]
fn a_double_click_finds_the_margin_it_is_in() {
    let document = WordDoc::new(contract());
    let (_, height) = document.page_size();
    assert_eq!(document.margin_story(0, 4.0), Some(Story::Header));
    assert_eq!(document.margin_story(0, height - 4.0), Some(Story::Footer));
    assert_eq!(document.margin_story(0, height / 2.0), None);
}

#[test]
fn saving_an_open_story_saves_the_whole_document_and_keeps_editing() {
    for role in [Story::Header, Story::Footer] {
        let mut document = WordDoc::new(contract());
        document.place_caret(3, 0);
        document.insert_text("本文の未保存編集").unwrap();
        let body = document.main().paragraph_text(3).unwrap();
        document.open_story(0, role).unwrap();
        document.insert_text("未確定の編集").unwrap();
        let caret = document.caret();
        let snapshot = document.word_docx().unwrap();
        let reopened = common::open_bytes(&snapshot);
        assert_eq!(reopened.paragraph_text(3).unwrap(), body);
        assert!(document.is_dirty(), "snapshot is not a save");

        let failed = document.export_docx_with(|bytes| {
            assert_eq!(bytes, snapshot);
            Err("conversion failed".into())
        });
        assert!(failed.is_err());
        assert!(document.is_dirty());
        assert_eq!(document.caret(), caret);
        assert!(document.story().is_some());
        assert!(document.can_undo());

        assert_eq!(document.export_docx_with(Ok).unwrap(), snapshot);
        assert!(!document.is_dirty());
        assert!(document.story().is_some());
        assert_eq!(document.caret(), caret);
        assert!(document.can_undo(), "saving keeps story undo");
        document.insert_text("続き").unwrap();
        assert!(document.is_dirty());
        document.export_docx().unwrap();
        assert!(!document.is_dirty());
        document.close_story().unwrap();
        assert!(!document.is_dirty(), "closing a saved story is not an edit");
        document.undo().unwrap();
        assert!(document.is_dirty());
        let restored = jade_opc::OpcPackage::open(&document.word_docx().unwrap()).unwrap();
        for name in restored.part_names() {
            if name.starts_with("word/header") || name.starts_with("word/footer") {
                assert!(!restored
                    .part_str(name)
                    .unwrap()
                    .unwrap()
                    .contains("未確定の編集"));
            }
        }
    }
}

#[test]
fn pdf_and_print_include_an_open_story_and_the_body_without_committing_it() {
    use jade_web::word::printing::PrintSettings;

    for role in [Story::Header, Story::Footer] {
        let mut document = WordDoc::new(contract());
        let before = document.export_pdf().unwrap();
        document.open_story(0, role).unwrap();
        document.insert_text("出力にも含まれる編集").unwrap();
        let caret = document.caret();
        let snapshot = document.word_docx().unwrap();
        let mut expected = common::open_bytes(&snapshot);
        let expected_pdf = expected.export_pdf().unwrap();
        assert_ne!(expected_pdf, before);
        assert_eq!(document.export_pdf().unwrap(), expected_pdf);
        let settings = PrintSettings::default();
        assert_eq!(
            document.print_pdf(&settings, 0).unwrap(),
            expected.print_pdf(&settings, 0).unwrap()
        );
        assert!(document.story().is_some());
        assert!(document.is_dirty());
        assert_eq!(document.caret(), caret);
        assert_eq!(document.word_docx().unwrap(), snapshot);
        document.undo().unwrap();
        assert_eq!(document.export_pdf().unwrap(), before);
    }
}

#[test]
fn preview_predicts_print_time_toc_updates_without_editing_the_document() {
    use jade_web::word::printing::PrintSettings;
    let mut main = contract();
    let heading = common::typed(&mut main, "First preview heading");
    main.place_caret(heading, 0);
    main.set_paragraph_style(Some("Heading1")).unwrap();
    main.place_caret(0, 0);
    main.insert_table_of_contents().unwrap();
    for index in 0..40 {
        let heading = common::typed(&mut main, &format!("Additional preview heading {index}: a title that takes space in the generated table of contents"));
        main.place_caret(heading, 0);
        main.set_paragraph_style(Some("Heading1")).unwrap();
    }
    let source = main.word_docx().unwrap();
    for role in [None, Some(Story::Header), Some(Story::Footer)] {
        let mut document = WordDoc::new(common::open_bytes(&source));
        if let Some(role) = role {
            document.open_story(0, role).unwrap();
            document.insert_text("Unsaved story text").unwrap();
        }
        let before = document.word_docx().unwrap();
        let selection = document.selection_range();
        let dirty = document.is_dirty();
        let can_undo = document.can_undo();
        let settings = PrintSettings {
            update_fields: true,
            ..Default::default()
        };
        let stale = document
            .print_preview(&PrintSettings::default(), 0)
            .unwrap();
        let predicted = document.print_preview(&settings, 0).unwrap();
        assert_ne!(
            predicted.sheets.len(),
            stale.sheets.len(),
            "the added TOC entries change pagination"
        );
        let last_page =
            jade_web::word::printing::PrintRange::Custom(predicted.page_sizes.len().to_string());
        assert!(document
            .print_preview(
                &PrintSettings {
                    range: last_page.clone(),
                    ..Default::default()
                },
                0
            )
            .is_err());
        assert_eq!(
            document
                .print_preview(
                    &PrintSettings {
                        range: last_page,
                        ..settings.clone()
                    },
                    0
                )
                .unwrap()
                .sheets
                .len(),
            1
        );
        assert_eq!(document.word_docx().unwrap(), before);
        assert_eq!(document.selection_range(), selection);
        assert_eq!(document.is_dirty(), dirty);
        assert_eq!(document.can_undo(), can_undo);
        assert_eq!(document.story().map(|story| story.role), role);

        let predicted_image = document
            .prepare_print_preview(&settings, 0)
            .unwrap()
            .render_sheet(0)
            .unwrap()
            .rgba;
        document.update_fields_for_print().unwrap();
        assert_eq!(
            predicted_image,
            document
                .prepare_print_preview(&PrintSettings::default(), 0)
                .unwrap()
                .render_sheet(0)
                .unwrap()
                .rgba
        );
        let actual = document
            .print_preview(&PrintSettings::default(), 0)
            .unwrap();
        assert_eq!(predicted, actual);
        // Reopening the panel after an update must not add TOC entries twice.
        assert_eq!(document.print_preview(&settings, 0).unwrap(), actual);
        document.main_mut().undo().unwrap();
        assert_eq!(document.word_docx().unwrap(), before);
    }
}
