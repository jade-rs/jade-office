//! Prints workbooks and decks to PDF as the native app prints them — Skia,
//! the files' own fonts where installed — so the pages can be set beside
//! the PDFs Excel and PowerPoint print of the same files.
//!
//! ```text
//! cargo run --release --example oracle_pdf --features skia -- <out-dir> <file>...
//! ```
//! A workbook prints each sheet as `<stem>_xlsx.s<N>.pdf`; a deck as
//! `<stem>_pptx.pdf`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use jade_office_model::deck::Deck;
use jade_office_model::print::PrintFields;
use jade_office_model::sheet::Workbook;
use jade_office_model::skia::SkiaBackend;
use jade_office_model::{aliased_faces, thai_face, THAI_FONT};

fn backend() -> SkiaBackend {
    let fonts = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts"
    ));
    let read = |path: PathBuf| Arc::new(std::fs::read(path).unwrap());
    let mut faces = aliased_faces(
        read(fonts.join("Noto_Sans_JP/static/NotoSansJP-Regular.ttf")),
        Some(read(fonts.join("Noto_Sans_JP/static/NotoSansJP-Bold.ttf"))),
    );
    faces.push(thai_face(read(fonts.join(THAI_FONT))));
    let backend = SkiaBackend::new(&faces).unwrap();
    if std::env::var("JADE_FONTS").is_ok_and(|value| value == "bundled") {
        backend
    } else {
        backend.with_installed_fonts()
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: oracle_pdf <out-dir> <file>..."));
    std::fs::create_dir_all(&out).unwrap();
    let fields = PrintFields {
        date: "2026/10/21".to_string(),
        time: "12:00".to_string(),
        file: String::new(),
    };
    for input in args {
        let path = Path::new(&input);
        let bytes = std::fs::read(path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let result = std::panic::catch_unwind(|| -> Result<usize, String> {
            match extension.as_str() {
                "xlsx" => {
                    let mut book = Workbook::open(&bytes, "Asia/Tokyo", backend())?;
                    let mut written = 0;
                    for index in 0..book.sheet_names().len() {
                        book.select_sheet(index)?;
                        let fields = PrintFields {
                            file: path.file_name().unwrap().to_string_lossy().into_owned(),
                            ..fields.clone()
                        };
                        if let Ok(pdf) = book.pdf(&fields) {
                            std::fs::write(
                                out.join(format!("{stem}_xlsx.s{}.pdf", index + 1)),
                                pdf,
                            )
                            .unwrap();
                            written += 1;
                        }
                    }
                    Ok(written)
                }
                "pptx" => {
                    let mut deck = Deck::open(&bytes, backend())?;
                    std::fs::write(out.join(format!("{stem}_pptx.pdf")), deck.pdf()?).unwrap();
                    Ok(1)
                }
                other => Err(format!("not handled: {other}")),
            }
        });
        match result {
            Ok(Ok(count)) => println!("ok\t{input}\t{count}"),
            Ok(Err(error)) => println!("error\t{input}\t{error}"),
            Err(_) => println!("panic\t{input}"),
        }
    }
}
