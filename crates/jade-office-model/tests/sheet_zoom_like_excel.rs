//! A sheet opens at the zoom it was saved at, as Excel opens it.

#![cfg(feature = "tiny-skia")]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::Workbook;
use jade_office_model::tiny::TinySkiaBackend;

/// The grid-operations fixture with its first sheet's view zoomed to
/// `percent`.
fn zoomed(percent: u32) -> Vec<u8> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
            if name == "xl/worksheets/sheet1.xml" {
                let xml = String::from_utf8(contents).unwrap();
                let xml = xml.replacen(
                    "<sheetView ",
                    &format!("<sheetView zoomScale=\"{percent}\" "),
                    1,
                );
                contents = xml.into_bytes();
            }
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&contents).unwrap();
        }
        writer.finish().unwrap();
    }
    out
}

fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    Workbook::open(bytes, "UTC", backend).unwrap()
}

#[test]
fn a_sheet_saved_at_85_percent_opens_there() {
    assert_eq!(open(&zoomed(85)).sheet_zoom(), 0.85);
    assert_eq!(open(&zoomed(250)).sheet_zoom(), 2.5);
    // The fixture as it is: Excel's 100%.
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let plain = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    assert_eq!(open(&plain).sheet_zoom(), 1.0);
}
