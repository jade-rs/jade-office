#![cfg(feature = "tiny-skia")]
use jade_office_model::{
    aliased_faces,
    sheet::{CellRange, CellRef, Workbook},
    tiny::TinySkiaBackend,
};
use std::sync::Arc;
const FIXTURE: &[u8] = include_bytes!("../../../jade-core/public/fixtures/pivot-cache-compat.xlsx");
fn open(bytes: &[u8]) -> Workbook<TinySkiaBackend> {
    let font = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"
    ))
    .unwrap();
    Workbook::open(
        bytes,
        "UTC",
        TinySkiaBackend::new(&aliased_faces(Arc::new(font), None)).unwrap(),
    )
    .unwrap()
}
fn at(s: &str) -> CellRef {
    CellRef::parse(s).unwrap()
}
fn assert_pivot_parts(before: &[u8], after: &[u8]) {
    let a = jade_opc::OpcPackage::open(before).unwrap();
    let b = jade_opc::OpcPackage::open(after).unwrap();
    for part in [
        "xl/pivotTables/report.xml",
        "xl/pivotCache/cache.xml",
        "xl/pivotCache/records.xml",
    ] {
        assert_eq!(a.part(part).unwrap(), b.part(part).unwrap());
    }
}
#[test]
fn shared_editor_history_and_save_keep_saved_pivot_data_and_metadata() {
    let mut b = open(FIXTURE);
    b.select_sheet(1).unwrap();
    assert_eq!(b.number(at("F6")), Some(42.));
    let before = b.save_bytes().unwrap();
    assert!(b.set_cell(at("H10"), "31").unwrap());
    let saved = b.save_bytes().unwrap();
    assert_pivot_parts(&before, &saved);
    let read = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert_eq!(read.pivots.tables.len(), 1);
    assert_eq!(read.pivots.caches.len(), 1);
    assert!(read.pivots.issues.is_empty());
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("H10")), None);
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("H10")), Some(31.));
    assert_pivot_parts(&before, &b.save_bytes().unwrap());
    let mut b = open(&saved);
    b.select_sheet(1).unwrap();
    assert_eq!(b.number(at("F6")), Some(42.));
}
#[test]
fn refused_compound_edit_keeps_pivot_payload_and_saved_report_values() {
    let mut p = jade_opc::OpcPackage::open(FIXTURE).unwrap();
    let part = "xl/worksheets/sheet2.xml";
    let xml=p.required_part_str(part).unwrap().replace("</worksheet>",r#"<sheetProtection sheet="1"/><protectedRanges><protectedRange name="Origin" sqref="H10"/></protectedRanges></worksheet>"#);
    p.replace(part, xml.into_bytes());
    let mut b = open(&p.write().unwrap());
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b
        .paste_over(CellRange::parse("H10:H11").unwrap(), "31")
        .is_err());
    assert_eq!(b.number(at("H10")), None);
    assert_eq!(b.number(at("F6")), Some(42.));
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_pivot_parts(FIXTURE, &before);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
}

#[test]
fn typed_pivot_queries_recalculate_move_save_and_undo_through_the_shared_model() {
    let mut b = open(FIXTURE);
    b.select_sheet(1).unwrap();
    assert!(b
        .set_cell(at("H10"), r#"=GETPIVOTDATA("Amount",E3,"Region","West")"#)
        .unwrap());
    assert_eq!(b.number(at("H10")), Some(27.));
    assert!(b.insert_rows(1, 2).unwrap());
    assert_eq!(b.number(at("H12")), Some(27.));
    let saved = b.save_bytes().unwrap();
    let read = jade_xlsx_reader::read_xlsx(&saved).unwrap();
    assert_eq!(
        read.pivots.tables[0].location.reference,
        CellRange::parse("E5:F8").unwrap()
    );
    assert!(b.undo().unwrap());
    assert_eq!(b.number(at("H10")), Some(27.));
    assert!(b.redo().unwrap());
    assert_eq!(b.number(at("H12")), Some(27.));
    let mut reopened = open(&saved);
    reopened.select_sheet(1).unwrap();
    assert_eq!(reopened.number(at("H12")), Some(27.));
}
#[test]
fn report_value_and_partial_structure_refusals_preserve_the_shared_model_state() {
    let mut b = open(FIXTURE);
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    let revision = b.layout_revision();
    assert!(b.set_cell(at("F4"), "700").is_err());
    assert!(b.insert_rows(4, 1).is_err());
    assert_eq!(b.number(at("F4")), Some(15.));
    assert_eq!(b.layout_revision(), revision);
    assert_eq!(b.save_bytes().unwrap(), before);
    assert!(!b.is_dirty());
    assert!(!b.undo().unwrap());
}

#[test]
fn authored_style_removal_repaints_actual_pixels_without_erasing_manual_cell_formats() {
    use jade_office_model::{
        pivots::{PivotStyle, PivotUpdate},
        sheet::Overlay,
        Backend,
    };
    fn pixel(b: &Workbook<TinySkiaBackend>, cell: &str) -> Vec<u8> {
        let painter = b
            .render_viewport((0., 0.), 900, 300, 1., &Overlay::default())
            .unwrap();
        let (pixels, width, _) = TinySkiaBackend::bgra(&painter).unwrap();
        let (x, y, _, _) = b.cell_rect(at(cell), (0., 0.)).unwrap();
        let i = ((y as usize + 2) * width as usize + x as usize + 2) * 4;
        pixels[i..i + 4].to_vec()
    }
    let mut b = open(include_bytes!(
        "../../../jade-core/public/fixtures/pivot-style-compat.xlsx"
    ));
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    assert_eq!(pixel(&b, "F3"), [255, 0, 0, 255]);
    assert_eq!(pixel(&b, "F5"), [0, 0, 255, 255]);
    b.update_pivot_named(
        "RegionalSales",
        &PivotUpdate {
            style: Some(PivotStyle {
                name: None,
                row_headers: true,
                column_headers: true,
                row_stripes: false,
                column_stripes: false,
                last_column: false,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(pixel(&b, "F3"), [255, 255, 255, 255]);
    assert_eq!(pixel(&b, "F5"), [255, 255, 255, 255]);
    assert_eq!(pixel(&b, "F4"), [0, 255, 255, 255]);
    let after = b.save_bytes().unwrap();
    let mut reopened = open(&after);
    reopened.select_sheet(1).unwrap();
    assert_eq!(pixel(&reopened, "F5"), [255, 255, 255, 255]);
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(pixel(&b, "F5"), [0, 0, 255, 255]);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), after);
    assert_eq!(pixel(&b, "F5"), [255, 255, 255, 255]);
}

#[test]
fn styled_report_growth_repaints_totals_and_keeps_manual_formats_across_history() {
    use jade_office_model::{sheet::Overlay, Backend};
    fn pixel(b: &Workbook<TinySkiaBackend>, cell: &str) -> Vec<u8> {
        let painter = b
            .render_viewport((0., 0.), 900, 300, 1., &Overlay::default())
            .unwrap();
        let (pixels, width, _) = TinySkiaBackend::bgra(&painter).unwrap();
        let (x, y, _, _) = b.cell_rect(at(cell), (0., 0.)).unwrap();
        let i = ((y as usize + 2) * width as usize + x as usize + 2) * 4;
        pixels[i..i + 4].to_vec()
    }
    let mut b = open(include_bytes!(
        "../../../jade-core/public/fixtures/pivot-style-compat.xlsx"
    ));
    b.select_sheet(1).unwrap();
    assert_eq!(pixel(&b, "F3"), [255, 0, 0, 255]);
    assert_eq!(pixel(&b, "F4"), [0, 255, 255, 255]);
    assert_eq!(pixel(&b, "F5"), [0, 0, 255, 255]);
    assert_eq!(pixel(&b, "F6"), [0, 255, 0, 255]);
    b.select_sheet(0).unwrap();
    b.set_cell(at("A3"), "North").unwrap();
    b.select_sheet(1).unwrap();
    let before = b.save_bytes().unwrap();
    assert!(b.refresh_pivot_at(at("F4")).unwrap());
    let saved = b.save_bytes().unwrap();
    assert_eq!(b.number(at("F7")), Some(42.));
    assert_eq!(pixel(&b, "F4"), [0, 255, 255, 255]);
    assert_eq!(pixel(&b, "F6"), [0, 0, 255, 255]);
    assert_eq!(pixel(&b, "F7"), [0, 255, 0, 255]);
    let original = jade_xlsx_reader::OpcPackage::open(&before).unwrap();
    let written = jade_xlsx_reader::OpcPackage::open(&saved).unwrap();
    // Refresh legitimately adds number-format XFs. Existing XFs and the
    // custom style collections must retain their identities and definitions.
    let old_styles = jade_xlsx_reader::read_xlsx(&before).unwrap().styles;
    let new_styles = jade_xlsx_reader::read_xlsx(&saved).unwrap().styles;
    assert_eq!(old_styles.table_styles, new_styles.table_styles);
    assert_eq!(
        old_styles.differential_styles,
        new_styles.differential_styles
    );
    assert_eq!(
        old_styles.cell_formats,
        new_styles.cell_formats[..old_styles.cell_formats.len()]
    );
    let styles = |p: &jade_xlsx_reader::OpcPackage| {
        let xml = p.required_part_str("xl/styles.xml").unwrap();
        xml[xml.find("<dxfs ").unwrap()..].to_string()
    };
    assert_eq!(styles(&original), styles(&written));
    let mut reopened = open(&saved);
    reopened.select_sheet(1).unwrap();
    assert_eq!(pixel(&reopened, "F7"), [0, 255, 0, 255]);
    assert!(b.undo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), before);
    assert_eq!(pixel(&b, "F6"), [0, 255, 0, 255]);
    assert!(b.redo().unwrap());
    assert_eq!(b.save_bytes().unwrap(), saved);
    assert_eq!(pixel(&b, "F6"), [0, 0, 255, 255]);
    assert_eq!(pixel(&b, "F7"), [0, 255, 0, 255]);
}
