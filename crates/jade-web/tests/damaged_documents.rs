//! A damaged Word document is an error message, never a crash: every
//! fixture, truncated or with a main part cut short or garbled, opens to an
//! error or to pages — and does not panic on the way.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use jade_web::document::{measurer, Document};

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

const PARTS: [&str; 4] = [
    "word/document.xml",
    "word/styles.xml",
    "word/numbering.xml",
    "[Content_Types].xml",
];

/// The ways a file arrives broken: cut short as a package, or with one of
/// its main parts cut short, garbled or emptied inside a sound package.
fn damaged(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for fraction in [4, 2] {
        let cut = bytes.len() * (fraction - 1) / fraction;
        out.push((format!("cut at {cut}"), bytes[..cut].to_vec()));
    }
    out.push(("last byte cut".into(), bytes[..bytes.len() - 1].to_vec()));
    let Ok(package) = jade_opc::OpcPackage::open(bytes) else {
        return out;
    };
    for part in PARTS {
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

#[test]
fn damaged_documents_fail_to_open_without_crashing() {
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

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    for path in &documents {
        let name = path.file_name().unwrap().to_string_lossy();
        let bytes = std::fs::read(path).unwrap();
        for (how, bytes) in damaged(&bytes) {
            let opened = catch_unwind(AssertUnwindSafe(|| {
                if let Ok(document) = Document::open(&bytes, 1.0, fonts.clone(), measurer.clone()) {
                    for index in 0..document.page_count() {
                        let _ = document.page(index);
                    }
                }
            }));
            if opened.is_err() {
                failures.push(format!("{name}, {how}: panics"));
            }
        }
    }
    std::panic::set_hook(previous);
    assert!(
        failures.is_empty(),
        "{} damaged documents panic:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
