//! Printing a Word document in the browser: the 印刷 settings the native
//! app has, made into a PDF of the sheets.

use std::path::PathBuf;

use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::page::Paper;
use jade_web::word::printing::{PrintRange, PrintSettings};

fn contract() -> EditableDocument {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    let bytes = std::fs::read(core.join("contract.docx")).unwrap();
    EditableDocument::open(&bytes, 1.0, fonts, measurer).unwrap()
}

/// The `/MediaBox` of each page of a PDF the browser build wrote.
fn sheets(pdf: &[u8]) -> Vec<(f64, f64)> {
    let text = String::from_utf8_lossy(pdf);
    text.match_indices("/MediaBox [0 0 ")
        .map(|(at, prefix)| {
            let rest = &text[at + prefix.len()..];
            let numbers: Vec<f64> = rest
                .split(']')
                .next()
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            (numbers[0], numbers[1])
        })
        .collect()
}

#[test]
fn the_sheets_follow_the_settings() {
    let mut document = contract();
    let pages = document.page_count();
    assert!(pages >= 2);
    let all = document.print_pdf(&PrintSettings::default(), 0).unwrap();
    assert_eq!(sheets(&all).len(), pages);
    let two_up = document
        .print_pdf(
            &PrintSettings {
                pages_per_sheet: 2,
                ..PrintSettings::default()
            },
            0,
        )
        .unwrap();
    let two_up = sheets(&two_up);
    assert_eq!(two_up.len(), pages.div_ceil(2));
    assert!(two_up[0].0 > two_up[0].1, "two pages side by side");
    let range = document
        .print_pdf(
            &PrintSettings {
                range: PrintRange::Custom("2".into()),
                scale_to: Some(Paper::A3),
                ..PrintSettings::default()
            },
            0,
        )
        .unwrap();
    let range = sheets(&range);
    assert_eq!(range.len(), 1);
    assert!((range[0].0 - 841.9).abs() < 1.0);
    assert!(document
        .print_pdf(
            &PrintSettings {
                range: PrintRange::Custom("p9s9".into()),
                ..PrintSettings::default()
            },
            0
        )
        .is_err());
}

#[test]
fn selection_printing_requires_selected_content() {
    let mut document = contract();
    let settings = PrintSettings {
        range: PrintRange::Selection,
        ..Default::default()
    };
    assert!(document.print_pdf(&settings, 0).is_err());
    assert!(document.print_preview(&settings, 0).is_err());
    document.place_caret(3, 0);
    assert!(document.print_pdf(&settings, 0).is_err());
    assert!(document.print_preview(&settings, 0).is_err());
    document.select((3, 0), (3, 3)).unwrap();
    assert!(document.print_pdf(&settings, 0).is_ok());
}

#[test]
fn pdf_export_uses_paper_layout_without_changing_the_editor() {
    use jade_docx_editor::canvas_renderer::ViewGeometry;

    let mut document = contract();
    document.set_formatting_marks(false).unwrap();
    let expected = document.export_pdf().unwrap();
    let expected_pages = sheets(&expected);
    document.place_caret(0, 0);
    document.insert_text("保存前の変更").unwrap();
    let edited_pdf = document.export_pdf().unwrap();
    assert_ne!(edited_pdf, expected, "PDF includes unsaved edits");
    let bytes = document.word_docx().unwrap();
    let caret = document.caret();
    let geometry = ViewGeometry {
        page_width: 360.0,
        page_height: 480.0,
        margin_top: 12.0,
        margin_right: 12.0,
        margin_bottom: 12.0,
        margin_left: 12.0,
        hide_page_chrome: true,
    };
    document.set_view_geometry(Some(geometry)).unwrap();
    document.set_collapsed_blocks(vec![1]).unwrap();
    document.set_formatting_marks(true).unwrap();
    document.set_dark_paper(true).unwrap();
    document.set_resident_pages(0..1).unwrap();
    let page_count = document.page_count();
    let pixels = document.page(0).unwrap().rgba_premultiplied();
    document.take_repainted_pages();

    let actual = document.export_pdf().unwrap();
    assert_eq!(sheets(&actual), expected_pages);
    assert_eq!(actual, edited_pdf, "view settings never enter the PDF");
    assert_eq!(document.view_geometry(), Some(geometry));
    assert_eq!(document.collapsed_blocks(), &[1]);
    assert!(document.formatting_marks());
    assert!(document.is_dirty());
    assert_eq!(document.caret(), caret);
    assert_eq!(document.word_docx().unwrap(), bytes);
    assert_eq!(document.page_count(), page_count);
    assert_eq!(document.page(0).unwrap().rgba_premultiplied(), pixels);
    assert!(document.take_repainted_pages().is_empty());
    document.undo().unwrap();
    assert_eq!(document.export_pdf().unwrap(), expected);
}

#[test]
fn pdf_export_does_not_print_editing_marks() {
    let mut document = contract();
    assert!(document.formatting_marks());
    let with_marks_on_screen = document.export_pdf().unwrap();
    document.set_formatting_marks(false).unwrap();
    assert_eq!(document.export_pdf().unwrap(), with_marks_on_screen);
    // Both output paths use the same clean paper layout at print resolution.
    assert_eq!(
        document.print_pdf(&PrintSettings::default(), 0).unwrap(),
        with_marks_on_screen
    );
}

#[test]
fn preview_uses_output_pagination_and_preserves_editor_state() {
    use jade_docx_editor::canvas_renderer::ViewGeometry;
    let mut document = contract();
    document.select((3, 0), (3, 3)).unwrap();
    document
        .set_view_geometry(Some(ViewGeometry {
            page_width: 360.0,
            page_height: 480.0,
            margin_top: 12.0,
            margin_right: 12.0,
            margin_bottom: 12.0,
            margin_left: 12.0,
            hide_page_chrome: true,
        }))
        .unwrap();
    let bytes = document.word_docx().unwrap();
    let selection = document.selection_range();
    let dirty = document.is_dirty();
    for settings in [
        PrintSettings::default(),
        PrintSettings {
            range: PrintRange::Selection,
            ..Default::default()
        },
        PrintSettings {
            pages_per_sheet: 2,
            scale_to: Some(Paper::A3),
            ..Default::default()
        },
        PrintSettings {
            range: PrintRange::Custom("2".into()),
            ..Default::default()
        },
    ] {
        let preview = document.print_preview(&settings, 0).unwrap();
        let pdf = document.print_pdf(&settings, 0).unwrap();
        let output = sheets(&pdf);
        assert_eq!(preview.sheets.len(), output.len());
        for (sheet, (width, height)) in preview.sheets.iter().zip(output) {
            assert!((sheet.width - width).abs() < 0.01);
            assert!((sheet.height - height).abs() < 0.01);
        }
        if settings.range == PrintRange::Selection {
            assert_eq!(preview.page_sizes.len(), 1);
        }
    }
    assert_eq!(document.word_docx().unwrap(), bytes);
    assert_eq!(document.selection_range(), selection);
    assert_eq!(document.is_dirty(), dirty);
}

#[test]
fn preview_retains_each_sections_paper_dimensions() {
    use jade_web::word::page::{SectionBreak, WordPage};
    use jade_web::word::page_setup::{SetupTab, WordPageSetup};
    let mut document = contract();
    document.place_caret(3, 0);
    document
        .insert_section_break(SectionBreak::NextPage)
        .unwrap();
    document.place_caret(4, 0);
    let mut dialog = document.page_setup_dialog(SetupTab::Margins);
    dialog.set_landscape(true);
    document.apply_page_setup(&dialog).unwrap();
    let preview = document
        .print_preview(&PrintSettings::default(), 0)
        .unwrap();
    assert!(preview.page_sizes.iter().any(|(w, h)| w < h));
    assert!(preview.page_sizes.iter().any(|(w, h)| w > h));
    let output = sheets(&document.print_pdf(&PrintSettings::default(), 0).unwrap());
    assert_eq!(preview.page_sizes.len(), output.len());
    for ((w, h), (ow, oh)) in preview.page_sizes.iter().zip(output) {
        assert!((w - ow).abs() < 0.01 && (h - oh).abs() < 0.01);
    }
}

#[test]
fn preview_sheet_images_match_pdf_printing() {
    let mut document = contract();
    document.select((3, 0), (3, 3)).unwrap();
    document.set_dark_paper(true).unwrap();
    let before = document.word_docx().unwrap();
    let selection = document.selection_range();
    let dirty = document.is_dirty();
    for settings in [
        PrintSettings::default(),
        PrintSettings {
            range: PrintRange::Selection,
            ..Default::default()
        },
        PrintSettings {
            pages_per_sheet: 2,
            scale_to: Some(Paper::A3),
            ..Default::default()
        },
        PrintSettings {
            range: PrintRange::Custom("2,1,2".into()),
            ..Default::default()
        },
    ] {
        let mut preview = document.prepare_print_preview(&settings, 0).unwrap();
        let count = preview.layout().sheets.len();
        assert!(preview.render_sheet(count).is_err());
        let mut pages = Vec::new();
        for index in 0..count {
            let page = preview.render_sheet(index).unwrap();
            assert_eq!(
                page.rgba.len(),
                (page.pixel_width * page.pixel_height * 4) as usize
            );
            assert!(
                page.rgba
                    .chunks_exact(4)
                    .any(|pixel| pixel[0] < 200 && pixel[1] < 200 && pixel[2] < 200),
                "page contains printed content"
            );
            assert_eq!(
                &page.rgba[..4],
                &[255, 255, 255, 255],
                "white paper despite the dark editor"
            );
            pages.push(page);
        }
        assert_eq!(
            preview.render_sheet(0).unwrap().rgba,
            pages[0].rgba,
            "navigation repaints the same snapshot"
        );
        assert_eq!(
            jade_web::pdf::pdf_of_pages(&pages).unwrap(),
            document.print_pdf(&settings, 0).unwrap()
        );
    }
    assert_eq!(document.word_docx().unwrap(), before);
    assert_eq!(document.selection_range(), selection);
    assert_eq!(document.is_dirty(), dirty);
}
