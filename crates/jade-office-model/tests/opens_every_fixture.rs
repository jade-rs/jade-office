//! Every workbook and deck in the corpus opens in the app's models and
//! paints: every sheet's first screen, every slide, each with ink on it.
//! A file the app cannot open, or opens blank, is the first thing a person
//! notices; this is the corpus-wide check for it, on the browser build's
//! pure-Rust painter (the native one paints from the same models).

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::deck::Deck;
use jade_office_model::sheet::Workbook;
use jade_office_model::tiny::TinySkiaBackend;
use jade_office_model::{aliased_faces, Backend};

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn backend() -> TinySkiaBackend {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        Some(Arc::new(
            std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap(),
        )),
    ))
    .unwrap()
}

fn fixtures(extension: &str) -> Vec<(String, Vec<u8>)> {
    let mut fixtures: Vec<_> = std::fs::read_dir(core().join("public/fixtures"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .filter(|path| {
            !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("._"))
        })
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read(&path).unwrap())
        })
        .collect();
    fixtures.sort();
    fixtures
}

/// Whether a PNG has more than one colour in it: something was drawn.
fn has_ink(png: &[u8]) -> bool {
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    let pixels = &pixels[..info.buffer_size()];
    let stride = info.color_type.samples();
    let first = &pixels[..stride];
    pixels.chunks(stride).any(|pixel| pixel != first)
}

#[test]
fn every_workbook_opens_and_every_sheet_paints() {
    let workbooks = fixtures("xlsx");
    assert!(workbooks.len() >= 20);
    let mut problems = Vec::new();
    for (name, bytes) in &workbooks {
        let mut book = match Workbook::open(bytes, "UTC", backend()) {
            Ok(book) => book,
            Err(error) => {
                problems.push(format!("{name}: does not open: {error}"));
                continue;
            }
        };
        for (index, sheet) in book.sheet_names().into_iter().enumerate() {
            if let Err(error) = book.select_sheet(index) {
                problems.push(format!("{name} / {sheet}: cannot show: {error}"));
                continue;
            }
            match book.render_viewport(
                (0.0, 0.0),
                640,
                400,
                1.0,
                &jade_office_model::sheet::Overlay::default(),
            ) {
                Some(painter) => {
                    // The grid and its headers draw even on an empty sheet.
                    let png = TinySkiaBackend::png(&painter).unwrap();
                    if !has_ink(&png) {
                        problems.push(format!("{name} / {sheet}: paints blank"));
                    }
                }
                None => problems.push(format!("{name} / {sheet}: does not paint")),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_deck_opens_and_every_slide_paints() {
    let decks = fixtures("pptx");
    assert!(!decks.is_empty());
    let mut problems = Vec::new();
    for (name, bytes) in &decks {
        let mut deck = match Deck::open(bytes, backend()) {
            Ok(deck) => deck,
            Err(error) => {
                problems.push(format!("{name}: does not open: {error}"));
                continue;
            }
        };
        for index in 0..deck.slide_count() {
            match deck.render_slide(index, 640) {
                Some(painter) => {
                    let png = TinySkiaBackend::png(&painter).unwrap();
                    if !has_ink(&png) {
                        problems.push(format!("{name}: slide {} paints blank", index + 1));
                    }
                }
                None => problems.push(format!("{name}: slide {} does not paint", index + 1)),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
