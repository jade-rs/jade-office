//! Every Word document in the corpus opens in the browser build and paints
//! pages with something on them: the pure-Rust host is jade-office's own, so
//! nothing upstream checks it across the corpus.

use std::path::PathBuf;

use jade_web::document::{measurer, Document};

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

#[test]
fn every_document_opens_and_its_pages_paint() {
    let fonts_dir = core().join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = measurer(&regular, Some(&bold)).unwrap();

    let mut documents: Vec<PathBuf> = std::fs::read_dir(core().join("public/fixtures"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "docx"))
        .filter(|path| {
            !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("._"))
        })
        .collect();
    documents.sort();
    assert!(documents.len() > 100, "the corpus is missing");

    let mut problems = Vec::new();
    for path in &documents {
        let name = path.file_name().unwrap().to_string_lossy();
        let bytes = std::fs::read(path).unwrap();
        let document = match Document::open(&bytes, 1.0, fonts.clone(), measurer.clone()) {
            Ok(document) => document,
            Err(error) => {
                problems.push(format!("{name}: does not open: {error}"));
                continue;
            }
        };
        if document.page_count() == 0 {
            problems.push(format!("{name}: no pages"));
            continue;
        }
        // A blank document is a page of paper; everything else has ink
        // somewhere.
        let inked = (0..document.page_count()).any(|index| {
            document.page(index).is_some_and(|page| {
                page.rgba_premultiplied()
                    .chunks_exact(4)
                    .any(|pixel| pixel[..3] != [255, 255, 255])
            })
        });
        if !inked && !name.starts_with("blank") {
            problems.push(format!("{name}: every page is blank"));
        }
    }
    assert!(
        problems.is_empty(),
        "{} of {} documents:\n{}",
        problems.len(),
        documents.len(),
        problems.join("\n")
    );
}
