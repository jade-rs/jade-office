//! Macro-enabled workbooks and decks (`.xlsm`, `.pptm`) and templates
//! (`.xltx`, `.potx`) are the same packages with another main content type
//! and, for macros, a VBA project beside it. The app opens them, edits them,
//! and saves them without losing either: a save that dropped the macros
//! would quietly break the file for whoever runs them next.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::deck::Deck;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::tiny::TinySkiaBackend;
use jade_opc::OpcPackage;

const VBA: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 1, 2, 3, 4];
const VBA_RELATIONSHIP: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaProject";

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn backend() -> TinySkiaBackend {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap()
}

/// `fixture` with its main content type swapped and, when `vba` is given,
/// a VBA project related from the main part at `main_part`.
fn variant(fixture: &str, from: &str, to: &str, vba: Option<&str>, main_part: &str) -> Vec<u8> {
    let bytes = std::fs::read(core().join("public/fixtures").join(fixture)).unwrap();
    let mut package = OpcPackage::open(&bytes).unwrap();
    let mut types = package.part_str("[Content_Types].xml").unwrap().unwrap();
    assert!(types.contains(from), "{fixture} has no {from}");
    types = types.replace(from, to);
    if let Some(vba) = vba {
        types = types.replace(
            "<Default ",
            "<Default Extension=\"bin\" ContentType=\"application/vnd.ms-office.vbaProject\"/><Default ",
        );
        package.replace(vba, VBA.to_vec());
        let directory = main_part.rsplit_once('/').unwrap().0;
        let file = main_part.rsplit_once('/').unwrap().1;
        let rels_name = format!("{directory}/_rels/{file}.rels");
        let rels = package.part_str(&rels_name).unwrap().unwrap();
        let target = vba.rsplit_once('/').unwrap().1;
        package.replace(
            &rels_name,
            rels.replace(
                "</Relationships>",
                &format!(
                    "<Relationship Id=\"rIdJadeVba\" Type=\"{VBA_RELATIONSHIP}\" Target=\"{target}\"/></Relationships>"
                ),
            )
            .into_bytes(),
        );
    }
    package.replace("[Content_Types].xml", types.into_bytes());
    package.write().unwrap()
}

/// What a saved copy must still carry.
fn assert_kept(saved: &[u8], content_type: &str, vba: Option<&str>) {
    let package = OpcPackage::open(saved).unwrap();
    let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
    assert!(
        types.contains(content_type),
        "the main content type is lost: {types}"
    );
    if let Some(vba) = vba {
        assert_eq!(package.part(vba), Some(VBA), "the VBA project is lost");
        let related = package
            .part_names()
            .iter()
            .filter(|name| name.ends_with(".rels"))
            .filter_map(|name| package.part_str(name).ok().flatten())
            .any(|rels| rels.contains(VBA_RELATIONSHIP));
        assert!(related, "the VBA project is no longer related");
    }
}

const SHEET_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const DECK_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";

#[test]
fn macro_workbooks_and_templates_open_edit_and_save_whole() {
    for (to, vba) in [
        (
            "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
            Some("xl/vbaProject.bin"),
        ),
        (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
            None,
        ),
        (
            "application/vnd.ms-excel.template.macroEnabled.main+xml",
            Some("xl/vbaProject.bin"),
        ),
    ] {
        let bytes = variant(
            "grid-operations-workbook.xlsx",
            SHEET_MAIN,
            to,
            vba,
            "xl/workbook.xml",
        );
        let mut book = Workbook::open(&bytes, "UTC", backend())
            .unwrap_or_else(|error| panic!("{to}: {error}"));
        assert!(book.set_cell(CellRef::new(8, 1), "42").unwrap());
        let saved = book.save_bytes().unwrap();
        assert_kept(&saved, to, vba);
        let reopened = Workbook::open(&saved, "UTC", backend()).unwrap();
        assert_eq!(reopened.edit_text(CellRef::new(8, 1)), "42");
    }
}

#[test]
fn macro_decks_and_templates_open_edit_and_save_whole() {
    for (to, vba) in [
        (
            "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
            Some("ppt/vbaProject.bin"),
        ),
        (
            "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
            None,
        ),
    ] {
        let bytes = variant(
            "sample-deck.pptx",
            DECK_MAIN,
            to,
            vba,
            "ppt/presentation.xml",
        );
        let mut deck =
            Deck::open(&bytes, backend()).unwrap_or_else(|error| panic!("{to}: {error}"));
        let id = deck
            .shape_at(0, 480.0, 270.0)
            .or_else(|| deck.next_shape(0, None, false))
            .unwrap();
        if deck.shape_text(0, id).is_some() {
            assert!(deck.set_shape_text(0, id, "edited").unwrap());
        }
        let saved = deck.save_bytes();
        assert_kept(&saved, to, vba);
        assert!(Deck::open(&saved, backend()).is_ok());
    }
}

const DOCUMENT_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// `.docm` / `.dotm` / `.dotx`: Word opens, edits and saves them with the
/// VBA project untouched and never run.
#[test]
fn macro_documents_and_templates_open_edit_and_save_whole() {
    use jade_docx_editor::word_document::WordDocument;
    use jade_docx_editor::Options;
    use jade_docx_headless_api::{HeadlessCommand, HeadlessEditRequest};
    for (to, vba) in [
        (
            "application/vnd.ms-word.document.macroEnabled.main+xml",
            Some("word/vbaProject.bin"),
        ),
        (
            "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
            Some("word/vbaProject.bin"),
        ),
        (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
            None,
        ),
    ] {
        let bytes = variant("hello.docx", DOCUMENT_MAIN, to, vba, "word/document.xml");
        let mut document = block_on(WordDocument::load(&bytes, &Options::default()))
            .unwrap_or_else(|error| panic!("{to}: {error:?}"));
        jade_docx_editor::headless_api::apply_headless_commands_to_document(
            &mut document,
            &HeadlessEditRequest {
                commands: vec![HeadlessCommand::InsertText {
                    paragraph: 0,
                    offset: 0,
                    text: "マクロ".to_string(),
                }],
            },
        )
        .unwrap();
        let saved = document.export_docx().unwrap();
        assert_kept(&saved, to, vba);
        let reopened = block_on(WordDocument::load(&saved, &Options::default())).unwrap();
        let body = reopened.export_docx().unwrap();
        let package = OpcPackage::open(&body).unwrap();
        let xml = package.part_str("word/document.xml").unwrap().unwrap();
        assert!(xml.contains("マクロ"), "{to}: the edit is lost");
    }
}
