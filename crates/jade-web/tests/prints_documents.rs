//! Printing a Word document in the browser: the 印刷 settings the native
//! app has, made into a PDF of the sheets.

use std::path::PathBuf;

use jade_web::editing::EditableDocument;
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
