//! A damaged file is an error message, never a crash: every workbook and
//! deck in the corpus, truncated or with its main parts cut short or
//! garbled, opens to an error or to something that paints — and does not
//! panic on the way. A download cut off halfway, or a file another program
//! wrote badly, is the everyday case of this.

#![cfg(feature = "tiny-skia")]

use std::panic::{catch_unwind, AssertUnwindSafe};
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

/// The ways a file arrives broken: cut short as a package, or with one of
/// its `parts` cut short or garbled inside an otherwise sound package.
fn damaged(bytes: &[u8], parts: &[&str]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for fraction in [4, 2] {
        let cut = bytes.len() * (fraction - 1) / fraction;
        out.push((format!("cut at {cut}"), bytes[..cut].to_vec()));
    }
    out.push(("last byte cut".into(), bytes[..bytes.len() - 1].to_vec()));
    let Ok(package) = jade_opc::OpcPackage::open(bytes) else {
        return out;
    };
    for part in parts {
        let Some(xml) = package.part(part).map(<[u8]>::to_vec) else {
            continue;
        };
        let middle = xml.len() / 2;
        let mut noise = xml.clone();
        noise.splice(middle..middle, b"<w:p>\"&<</x>\xff\xfe]]>".iter().copied());
        for (how, contents) in [
            ("halved", xml[..middle].to_vec()),
            ("garbled", noise),
            ("emptied", Vec::new()),
        ] {
            let Ok(mut copy) = jade_opc::OpcPackage::open(bytes) else {
                continue;
            };
            copy.replace(part, contents);
            if let Ok(bytes) = copy.write() {
                out.push((format!("{part} {how}"), bytes));
            }
        }
    }
    out
}

/// Runs `open` on every damage of every fixture, quietly; the damages that
/// panicked.
fn panics(extension: &str, parts: &[&str], open: impl Fn(&[u8])) -> Vec<String> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    for (name, bytes) in fixtures(extension) {
        for (how, bytes) in damaged(&bytes, parts) {
            if catch_unwind(AssertUnwindSafe(|| open(&bytes))).is_err() {
                failures.push(format!("{name}, {how}: panics"));
            }
        }
    }
    std::panic::set_hook(previous);
    failures
}

#[test]
fn damaged_workbooks_fail_to_open_without_crashing() {
    let failures = panics(
        "xlsx",
        &[
            "xl/workbook.xml",
            "xl/worksheets/sheet1.xml",
            "xl/styles.xml",
            "xl/sharedStrings.xml",
            "[Content_Types].xml",
        ],
        |bytes| {
            if let Ok(mut book) = Workbook::open(bytes, "UTC", backend()) {
                for index in 0..book.sheet_names().len() {
                    if book.select_sheet(index).is_ok() {
                        let _ = book
                            .render_viewport((0.0, 0.0), 320, 200, 1.0, None)
                            .and_then(|painter| TinySkiaBackend::png(&painter));
                    }
                }
            }
        },
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn damaged_decks_fail_to_open_without_crashing() {
    let failures = panics(
        "pptx",
        &[
            "ppt/presentation.xml",
            "ppt/slides/slide1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/theme/theme1.xml",
            "[Content_Types].xml",
        ],
        |bytes| {
            if let Ok(mut deck) = Deck::open(bytes, backend()) {
                for index in 0..deck.slide_count() {
                    let _ = deck.render_slide(index, 320);
                }
            }
        },
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
