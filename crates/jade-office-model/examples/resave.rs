//! Opens each workbook and deck given, saves it as the app saves it — once
//! untouched and once after an everyday edit — and writes both beside
//! `<out-dir>`, so Office can be asked whether it opens them cleanly.
//!
//! ```text
//! cargo run --example resave --features tiny-skia -- <out-dir> <file>...
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::deck::Deck;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;

fn backend() -> TinySkiaBackend {
    let directory = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static"
    ));
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        Some(Arc::new(
            std::fs::read(directory.join("NotoSansJP-Bold.ttf")).unwrap(),
        )),
    ))
    .unwrap()
}

fn workbook(bytes: &[u8], edit: bool) -> Result<Vec<u8>, String> {
    let mut book = Workbook::open(bytes, "Asia/Tokyo", backend())?;
    if std::env::var("RESAVE_SCATTER").is_ok() && edit {
        // Cells between, before and beside the ones there: where a writer
        // that appends would put rows and cells out of order.
        let end = book.used_end();
        for (column, row) in [
            (1, 1),
            (30, 1),
            (1, end.row / 2 + 1),
            (end.column + 3, end.row / 2 + 1),
            (1, end.row + 5),
            (2, 2),
        ] {
            let _ = book.set_cell(CellRef::new(column, row), "x");
        }
        return book.save_bytes();
    }
    if edit {
        let end = book.used_end();
        book.set_cell(CellRef::new(1, end.row + 2), "jade 編集")?;
        book.set_cell(CellRef::new(2, end.row + 2), "=1+2")?;
    }
    book.save_bytes()
}

fn deck(bytes: &[u8], edit: bool) -> Result<Vec<u8>, String> {
    let mut deck = Deck::open(bytes, backend())?;
    if edit {
        let first = deck.shapes(0).first().map(|shape| shape.id);
        if let Some(id) = first {
            deck.move_shape(0, id, 12.0, 12.0)?;
        }
        deck.set_notes(0, "jade notes")?;
    }
    Ok(deck.save_bytes())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: resave <out-dir> <file>..."));
    std::fs::create_dir_all(&out).unwrap();
    for input in args {
        let path = Path::new(&input);
        let bytes = std::fs::read(path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        for edit in [false, true] {
            let result = std::panic::catch_unwind(|| match extension.as_str() {
                "xlsx" => workbook(&bytes, edit),
                "pptx" => deck(&bytes, edit),
                other => Err(format!("not handled: {other}")),
            });
            let tag = if edit { "edited" } else { "saved" };
            match result {
                Ok(Ok(saved)) => {
                    std::fs::write(out.join(format!("{stem}.{tag}.{extension}")), saved).unwrap();
                    println!("ok\t{input}\t{tag}");
                }
                Ok(Err(error)) => println!("error\t{input}\t{tag}\t{error}"),
                Err(_) => println!("panic\t{input}\t{tag}"),
            }
        }
    }
}
