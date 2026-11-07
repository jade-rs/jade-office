//! 罫線 › 斜め罫線 in the browser build, through the shared model the native
//! app runs too: 右下がり and 右上がり drawn across the selected cells,
//! saved as Word writes them (`w:tl2br` / `w:tr2bl`), the menu entry taking
//! them away again, and 線種とページ罫線と網かけの設定's 斜め罫線 for セル.

mod common;
use common::open_bytes;
use jade_office_model::word::table::{BorderSet, BorderTarget, BordersDialog, Pen, WordTables};
use jade_web::editing::{EditableDocument, WordFormatting};
use jade_web::word::new::{blank_document, Blank};

fn with_table() -> EditableDocument {
    let mut document = open_bytes(&blank_document(Blank::Japanese));
    document.place_caret(0, 0);
    let caret = WordFormatting::insert_table(&mut document, 2, 2).unwrap();
    document.place_caret(caret.0, caret.1);
    document
}

fn document_xml(document: &mut EditableDocument) -> String {
    jade_opc::OpcPackage::open(&document.export_docx().unwrap())
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap()
}

fn page_pixels(document: &EditableDocument) -> Vec<u8> {
    document.page(0).expect("first page").rgba_premultiplied()
}

fn red_pixels(pixels: &[u8]) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 150 && pixel[1] < 80 && pixel[2] < 80)
        .count()
}

#[test]
fn diagonal_borders_are_drawn_saved_and_toggled() {
    let mut document = with_table();
    let plain = page_pixels(&document);
    assert_eq!(red_pixels(&plain), 0);
    let pen = Pen {
        style: "single".to_string(),
        width: 1.5,
        color: "FF0000".to_string(),
    };

    // 斜め罫線 (右下がり) in the caret's cell.
    document
        .apply_border_set(BorderSet::DiagonalDown, &pen)
        .unwrap();
    assert_eq!(document.cell_diagonals(), [true, false]);
    let down = page_pixels(&document);
    let drawn = red_pixels(&down);
    assert!(
        drawn > 40,
        "the diagonal is drawn across the cell ({drawn})"
    );
    let xml = document_xml(&mut document);
    assert!(
        xml.contains(r#"<w:tl2br w:val="single" w:sz="12" w:space="0" w:color="FF0000"/>"#),
        "{xml}"
    );
    assert!(!xml.contains("w:tr2bl"));

    // Saved and opened again, it is drawn the same.
    let reopened = open_bytes(&document.export_docx().unwrap());
    assert_eq!(red_pixels(&page_pixels(&reopened)), drawn);

    // 右上がり too: both diagonals, a cross.
    document
        .apply_border_set(BorderSet::DiagonalUp, &pen)
        .unwrap();
    assert_eq!(document.cell_diagonals(), [true, true]);
    assert!(red_pixels(&page_pixels(&document)) > drawn);

    // The checked menu entry takes its diagonal away again.
    document
        .apply_border_set(BorderSet::DiagonalDown, &pen)
        .unwrap();
    assert_eq!(document.cell_diagonals(), [false, true]);
    let xml = document_xml(&mut document);
    assert!(
        !xml.contains("w:tl2br") && xml.contains("<w:tr2bl "),
        "{xml}"
    );

    // 罫線と網かけ, 設定対象 セル: the dialog's 斜め罫線.
    let mut dialog = BordersDialog::new(BorderTarget::Cell);
    dialog.pen = pen.clone();
    dialog.diagonals = [true, false];
    document.apply_borders(&dialog).unwrap();
    assert_eq!(document.cell_diagonals(), [true, false]);

    // Undo goes back step by step.
    for _ in 0..4 {
        document.undo().unwrap();
    }
    assert_eq!(document.cell_diagonals(), [false; 2]);
    assert_eq!(red_pixels(&page_pixels(&document)), 0);
}
