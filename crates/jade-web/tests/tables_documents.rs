//! Word's table tools in the browser build, through the shared model the
//! native app runs too (`jade-office-model`'s `WordTables`): the テーブル
//! デザイン tab's style options and gallery, selecting cells, rows, columns
//! and the table, dragging borders, 表のプロパティ, inserting and deleting
//! cells, conversions, sorting, formulas, nested tables, the pencil and the
//! eraser — each laid out as Word lays it out and saved as Word reads it.

mod common;
use common::open_bytes;
use jade_office_model::word::table::{
    AutoFit, BorderSet, BorderTarget, BordersDialog, CellShift, Field, LookOption, Pen,
    PropertiesTab, SortSpec, TableHit, TablePart, TablePropertiesDialog, TextSeparator,
    WordTables,
};
use jade_web::editing::{EditableDocument, TableCommand, WordFormatting, WordSession};
use jade_web::word::new::{blank_document, Blank};

fn blank() -> EditableDocument {
    open_bytes(&blank_document(Blank::Japanese))
}

/// A blank document with a `rows` × `columns` table and the caret in its
/// first cell.
fn with_table(rows: usize, columns: usize) -> EditableDocument {
    let mut document = blank();
    document.place_caret(0, 0);
    let caret = WordFormatting::insert_table(&mut document, rows, columns).unwrap();
    document.place_caret(caret.0, caret.1);
    document
}

/// The first paragraph of cell (`row`, `column`) of the table `order`.
fn cell_paragraph(document: &EditableDocument, order: usize, row: usize, column: usize) -> usize {
    document
        .cell_layouts()
        .iter()
        .find(|cell| {
            document.table_order_for(cell.table_id) == Some(order)
                && cell.row == row
                && cell.column == column
        })
        .and_then(|cell| cell.paragraphs.first().copied())
        .unwrap_or_else(|| panic!("cell ({row}, {column}) of table {order}"))
}

fn fill(document: &mut EditableDocument, order: usize, cells: &[&[&str]]) {
    for (row, values) in cells.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            let paragraph = cell_paragraph(document, order, row, column);
            document.place_caret(paragraph, 0);
            document.insert_text(value).unwrap();
        }
    }
}

fn cell_texts(document: &EditableDocument, order: usize) -> Vec<Vec<String>> {
    let table = document.word_document().table_by_order(order).expect("table");
    table
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| {
                    cell.blocks
                        .iter()
                        .filter_map(|block| block.as_paragraph())
                        .map(|paragraph| {
                            paragraph
                                .runs
                                .iter()
                                .map(|run| match (&run.field, run.text.is_empty()) {
                                    (Some(field), true) => field.cached_text.clone().unwrap_or_default(),
                                    _ => run.text.clone(),
                                })
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .collect()
        })
        .collect()
}

fn document_xml(bytes: &[u8]) -> String {
    jade_opc::OpcPackage::open(bytes)
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap()
}

fn caret_table_id(document: &EditableDocument) -> usize {
    let paragraph = document.selection_range().unwrap().focus.paragraph;
    document
        .cell_layout(document.cell_id_for_paragraph(paragraph).unwrap())
        .unwrap()
        .table_id
}

/// テーブルデザイン: the style options switch the style's regions, a gallery
/// style goes in by its built-in name, and both are saved.
#[test]
fn style_options_and_a_gallery_style_apply_and_save() {
    let mut document = with_table(3, 3);
    let styles = document.table_styles();
    assert!(styles.iter().any(|style| style.name == "Grid Table 4 - Accent 1"));
    assert!(styles.iter().any(|style| style.id == "TableGrid"));

    document.apply_table_style(Some("GridTable4-Accent1")).unwrap();
    assert_eq!(document.table_style_id().as_deref(), Some("GridTable4-Accent1"));
    document.set_look_option(LookOption::TotalRow, true).unwrap();
    document.set_look_option(LookOption::BandedColumns, true).unwrap();
    document.set_look_option(LookOption::HeaderRow, false).unwrap();
    let look = document.table_look().unwrap();
    assert!(look.total_row && look.banded_columns && !look.header_row && look.banded_rows);

    let saved = document.export_docx().unwrap();
    let xml = document_xml(&saved);
    assert!(xml.contains("<w:tblStyle w:val=\"GridTable4-Accent1\"/>"), "{xml}");
    assert!(xml.contains("w:lastRow=\"1\"") && xml.contains("w:noVBand=\"0\""), "{xml}");
    let mut reopened = open_bytes(&saved);
    reopened.place_caret(cell_paragraph(&reopened, 0, 0, 0), 0);
    assert_eq!(reopened.table_style_id().as_deref(), Some("GridTable4-Accent1"));
    assert_eq!(reopened.table_look(), Some(look));
}

/// 選択 ▸ セル / 列 / 行 / 表, and the same by mouse: left of a row, over a
/// column, inside a cell's left edge, the table's handle.
#[test]
fn cells_rows_columns_and_the_table_select_from_the_menu_and_the_mouse() {
    let mut document = with_table(3, 4);
    let paragraph = cell_paragraph(&document, 0, 1, 2);
    document.place_caret(paragraph, 0);
    document.select_table_part(TablePart::Column).unwrap();
    let span = document.cell_span().unwrap();
    assert_eq!((span.rows, span.columns), ((0, 2), (2, 2)));
    assert_eq!(document.selected_cell_list().unwrap().len(), 3);

    document.place_caret(paragraph, 0);
    document.select_table_part(TablePart::Row).unwrap();
    let span = document.cell_span().unwrap();
    assert_eq!((span.rows, span.columns), ((1, 1), (0, 3)));

    document.place_caret(paragraph, 0);
    document.select_table_part(TablePart::Table).unwrap();
    assert_eq!(document.selected_cell_list().unwrap().len(), 12);

    // The mouse.
    let table_id = caret_table_id(&document);
    let geometry = document.table_geometry(table_id).unwrap();
    let (top, bottom) = geometry.rows[2];
    let hit = document.table_hit(geometry.left() - 10.0, (top + bottom) / 2.0).unwrap();
    assert_eq!(hit, TableHit::RowSelector { table_id, row: 2 });
    document.click_table_hit(hit).unwrap();
    assert_eq!(document.cell_span().unwrap().rows, (2, 2));

    let x = (geometry.columns[1] + geometry.columns[2]) / 2.0;
    let hit = document.table_hit(x, geometry.top() - 6.0).unwrap();
    assert_eq!(hit, TableHit::ColumnSelector { table_id, column: 1 });
    document.click_table_hit(hit).unwrap();
    assert_eq!(document.cell_span().unwrap().columns, (1, 1));

    let (top, bottom) = geometry.rows[1];
    let hit = document
        .table_hit(geometry.columns[3] + 6.0, (top + bottom) / 2.0)
        .unwrap();
    assert!(matches!(hit, TableHit::CellSelector { .. }), "{hit:?}");
    document.click_table_hit(hit).unwrap();
    let span = document.cell_span().unwrap();
    assert_eq!((span.rows, span.columns), ((1, 1), (3, 3)));

    let hit = document
        .table_hit(geometry.left() - 14.0, geometry.top() - 14.0)
        .unwrap();
    assert_eq!(hit, TableHit::TableHandle { table_id });

    // A drag from one cell into another selects the cells between.
    let from = cell_paragraph(&document, 0, 0, 1);
    let to = cell_paragraph(&document, 0, 2, 2);
    assert!(document.select_cells_between(from, to).unwrap());
    let span = document.cell_span().unwrap();
    assert_eq!((span.rows, span.columns), ((0, 2), (1, 2)));
}

/// A column border dragged moves the line between its two columns; the
/// right edge widens the last column; a row border sets the row's height.
#[test]
fn borders_drag_to_resize_columns_and_rows() {
    let mut document = with_table(2, 3);
    let table_id = caret_table_id(&document);
    let before = document.table_geometry(table_id).unwrap();
    let (top, bottom) = before.rows[0];
    let y = (top + bottom) / 2.0;
    let hit = document.table_hit(before.columns[1] + 1.0, y).unwrap();
    assert_eq!(
        hit,
        TableHit::ColumnBorder { table_id, boundary: 1, x: before.columns[1] }
    );
    document.drag_column_border(table_id, 1, before.columns[1] + 40.0).unwrap();
    let table_id = caret_table_id(&document);
    let after = document.table_geometry(table_id).unwrap();
    let (old, new) = (before.widths_pt(), after.widths_pt());
    assert!((new[0] - old[0] - 30.0).abs() < 1.0, "{old:?} → {new:?}");
    assert!((old[1] - new[1] - 30.0).abs() < 1.0, "{old:?} → {new:?}");
    assert!((new[2] - old[2]).abs() < 1.0);

    document.drag_column_border(table_id, 3, after.right() - 20.0).unwrap();
    let table_id = caret_table_id(&document);
    let narrower = document.table_geometry(table_id).unwrap().widths_pt();
    assert!((new[2] - narrower[2] - 15.0).abs() < 1.0, "{new:?} → {narrower:?}");

    let geometry = document.table_geometry(table_id).unwrap();
    let (top, bottom) = geometry.rows[0];
    let hit = document.table_hit(geometry.left() + 20.0, bottom).unwrap();
    assert_eq!(hit, TableHit::RowBorder { table_id, row: 0, y: bottom });
    document.drag_row_border(table_id, 0, top + 80.0).unwrap();
    let table_id = caret_table_id(&document);
    let (top, bottom) = document.table_geometry(table_id).unwrap().rows[0];
    assert!((bottom - top - 80.0).abs() < 2.0, "{}", bottom - top);

    let saved = document.export_docx().unwrap();
    let reopened = open_bytes(&saved);
    let table = reopened.word_document().table_by_order(0).unwrap();
    let widths: Vec<f32> = table.columns.iter().map(|column| column.width.unwrap()).collect();
    assert!((widths[0] - narrower[0]).abs() < 1.0, "{widths:?} {narrower:?}");
    assert!((table.rows[0].height.unwrap() - 60.0).abs() < 1.5, "{:?}", table.rows[0].height);
}

/// 表のプロパティ: each tab's settings land on the table, its rows, its
/// columns and its cells, and are saved as Word saves them.
#[test]
fn table_properties_set_the_table_rows_columns_and_cells() {
    let mut document = with_table(3, 2);
    let paragraph = cell_paragraph(&document, 0, 1, 1);
    document.place_caret(paragraph, 0);
    let properties = document.table_properties().unwrap();
    assert!(properties.autofit || !properties.autofit);
    let mut dialog = TablePropertiesDialog::new(properties, PropertiesTab::Table);
    dialog.width_on = true;
    dialog.width_percent = true;
    dialog.text.insert(Field::TableWidth, "80".to_string());
    dialog.alignment = "center".to_string();
    dialog.floating = true;
    dialog.position_x_align = Some("right".to_string());
    dialog.horizontal_anchor = "margin".to_string();
    dialog.text.insert(Field::PositionVertical, "20".to_string());
    dialog.spacing_on = true;
    dialog.text.insert(Field::Spacing, "1".to_string());
    dialog.text.insert(Field::MarginLeft, "3".to_string());
    // 行
    dialog.row_height_on = true;
    dialog.text.insert(Field::RowHeight, "12".to_string());
    dialog.row_exact = true;
    dialog.allow_break = false;
    dialog.repeat_header = true;
    // 列
    dialog.column_width_on = true;
    dialog.text.insert(Field::ColumnWidth, "40".to_string());
    // セル
    dialog.vertical_align = "center".to_string();
    dialog.cell_margins_same = false;
    dialog.text.insert(Field::CellMarginTop, "2".to_string());
    document.apply_table_properties(&dialog).unwrap();

    document.place_caret(cell_paragraph(&document, 0, 1, 1), 0);
    let read = document.table_properties().unwrap();
    assert!(matches!(read.width, Some(jade_office_model::word::table::PreferredWidth::Percent(p)) if (p - 80.0).abs() < 0.1));
    assert_eq!(read.alignment, "center");
    assert!(read.floating);
    assert_eq!(read.position.horizontal_align.as_deref(), Some("right"));
    assert!((read.spacing.unwrap() - 2.83).abs() < 0.05, "{:?}", read.spacing);
    assert!((read.row_height.unwrap() - 34.0).abs() < 0.2 && read.row_exact);
    assert!(!read.allow_break && read.repeat_header);
    assert!((read.column_width.unwrap() - 113.4).abs() < 0.5, "{:?}", read.column_width);
    assert_eq!(read.vertical_align, "center");
    assert!((read.cell_margins.unwrap()[0] - 5.67).abs() < 0.1);

    // 80% of the 150 mm text column: the grid is scaled to it, as Word's
    // dialog scales the columns, so the table is drawn that wide — and then
    // the second column takes the 40 mm the 列 tab asked for.
    let grid: Vec<f32> = document
        .word_document()
        .table_by_order(0)
        .unwrap()
        .columns
        .iter()
        .map(|column| column.width.unwrap_or(0.0))
        .collect();
    assert!((grid[0] - 0.4 * 425.2).abs() < 2.0, "{grid:?}");
    assert!((grid[1] - 113.4).abs() < 1.0, "{grid:?}");
    let xml = document_xml(&document.export_docx().unwrap());
    for expected in [
        "<w:tblW w:w=\"4000\" w:type=\"pct\"/>",
        "<w:jc w:val=\"center\"/>",
        "w:tblpXSpec=\"right\"",
        "w:hRule=\"exact\"",
        "<w:cantSplit/>",
        "<w:tblHeader/>",
        "<w:vAlign w:val=\"center\"/>",
        "<w:tblCellSpacing",
    ] {
        assert!(xml.contains(expected), "{expected} in {xml}");
    }
}

/// セルの挿入 / セルの削除 with their shifts.
#[test]
fn cells_insert_and_delete_with_a_shift() {
    let mut document = with_table(2, 2);
    fill(&mut document, 0, &[&["a", "b"], &["c", "d"]]);
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    document.insert_cells(CellShift::Horizontal).unwrap();
    assert_eq!(cell_texts(&document, 0)[0], vec!["", "a", "b"]);
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    document.delete_cells(CellShift::Horizontal).unwrap();
    assert_eq!(cell_texts(&document, 0)[0], vec!["a", "b"]);
    document.place_caret(cell_paragraph(&document, 0, 0, 1), 0);
    document.insert_cells(CellShift::Vertical).unwrap();
    let texts = cell_texts(&document, 0);
    assert_eq!(texts.len(), 3);
    assert_eq!((texts[0][1].as_str(), texts[1][1].as_str(), texts[2][1].as_str()), ("", "b", "d"));
    document.place_caret(cell_paragraph(&document, 0, 0, 1), 0);
    document.delete_cells(CellShift::Vertical).unwrap();
    let texts = cell_texts(&document, 0);
    assert_eq!((texts[0][1].as_str(), texts[1][1].as_str()), ("b", "d"));
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    document.delete_cells(CellShift::EntireRow).unwrap();
    assert_eq!(cell_texts(&document, 0)[0][0], "c");
}

/// 文字列を表にする and 表の解除.
#[test]
fn text_and_tables_convert_both_ways() {
    let mut document = blank();
    document.place_caret(0, 0);
    document.insert_text("品目\t数量").unwrap();
    document.insert_paragraph_break().unwrap();
    document.insert_text("りんご\t3").unwrap();
    document.insert_paragraph_break().unwrap();
    document.insert_text("おわり").unwrap();
    document.select((0, 0), (1, 2)).unwrap();
    assert_eq!(document.text_table_shape(&TextSeparator::Tab), (2, 2));
    let caret = document.convert_text_to_table(&TextSeparator::Tab, None).unwrap();
    document.place_caret(caret.0, caret.1);
    assert_eq!(cell_texts(&document, 0), vec![vec!["品目", "数量"], vec!["りんご", "3"]]);
    assert!(document.caret_table_cell().is_some());
    assert!(document.paragraph_text(document.paragraph_count() - 1).unwrap().contains("おわり"));

    document.convert_table_to_text(&TextSeparator::Comma).unwrap();
    assert!(document.word_document().table_by_order(0).is_none());
    assert_eq!(document.paragraph_text(0).as_deref(), Some("品目,数量"));
    assert_eq!(document.paragraph_text(1).as_deref(), Some("りんご,3"));
    let reopened = open_bytes(&document.export_docx().unwrap());
    assert_eq!(reopened.paragraph_text(1).as_deref(), Some("りんご,3"));
    // And it undoes.
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(0).is_some());
}

/// 並べ替え with a title row, and 計算式 fields that update.
#[test]
fn rows_sort_and_formulas_compute() {
    let mut document = with_table(5, 2);
    fill(
        &mut document,
        0,
        &[&["品目", "金額"], &["みかん", "300"], &["りんご", "1,200"], &["ぶどう", "800"]],
    );
    document.place_caret(cell_paragraph(&document, 0, 1, 0), 0);
    assert_eq!(document.sort_columns(true), vec!["品目", "金額"]);
    document
        .sort_table(&SortSpec {
            keys: vec![jade_docx_headless_api::HeadlessTableSortKey {
                column: 1,
                kind: jade_docx_headless_api::HeadlessTableSortKind::Number,
                descending: true,
            }],
            header_row: true,
        })
        .unwrap();
    let texts = cell_texts(&document, 0);
    // The empty last row has no number: it sorts last when descending.
    assert_eq!(texts[1], vec!["りんご", "1,200"]);
    assert_eq!(texts[2], vec!["ぶどう", "800"]);
    assert_eq!(texts[3], vec!["みかん", "300"]);

    document.place_caret(cell_paragraph(&document, 0, 4, 1), 0);
    assert_eq!(document.suggested_formula(), "=SUM(ABOVE)");
    document.insert_formula("=SUM(ABOVE)", Some("#,##0")).unwrap();
    assert_eq!(cell_texts(&document, 0)[4][1], "2,300");
    // Change a number; フィールド更新 recomputes.
    let paragraph = cell_paragraph(&document, 0, 3, 1);
    document.place_caret(paragraph, 0);
    document.insert_text("1").unwrap();
    document.update_formulas().unwrap();
    assert_eq!(cell_texts(&document, 0)[4][1], "3,300");
    let xml = document_xml(&document.export_docx().unwrap());
    assert!(xml.contains("=SUM(ABOVE)"), "{xml}");
}

/// A table inserted in a cell is a nested table the table tools edit like
/// any other, and is saved inside its cell.
#[test]
fn a_table_inside_a_cell_is_edited_as_a_table() {
    let mut document = with_table(2, 2);
    document.place_caret(cell_paragraph(&document, 0, 0, 1), 0);
    let caret = WordFormatting::insert_table(&mut document, 2, 2).unwrap();
    document.place_caret(caret.0, caret.1);
    let cell = document.caret_table_cell().unwrap();
    assert_eq!(cell.table_order_index, 1, "the caret is in the nested table");
    assert_eq!((cell.rows, cell.columns), (2, 2));
    document.insert_text("内側").unwrap();
    let caret = document.table_command(TableCommand::InsertRowBelow).unwrap();
    document.place_caret(caret.0, caret.1);
    assert_eq!(document.caret_table_cell().unwrap().rows, 3);
    // The outer table is untouched.
    assert_eq!(document.word_document().table_dimensions(0), Some((2, 2)));

    let saved = document.export_docx().unwrap();
    let xml = document_xml(&saved);
    let outer_cell = xml.find("<w:tc>").unwrap();
    assert!(xml[outer_cell..].contains("<w:tbl>"), "{xml}");
    let reopened = open_bytes(&saved);
    assert_eq!(reopened.word_document().table_dimensions(1), Some((3, 2)));
    assert_eq!(cell_texts(&reopened, 1)[0][0], "内側");
}

/// 罫線を引く: a stroke across a cell splits it; 消しゴム on a line merges
/// the two cells it separates; a rectangle outside a table draws one.
#[test]
fn the_pencil_splits_and_the_eraser_merges() {
    let mut document = with_table(1, 2);
    let table_id = caret_table_id(&document);
    let geometry = document.table_geometry(table_id).unwrap();
    let (top, bottom) = geometry.rows[0];
    let middle = (top + bottom) / 2.0;
    // A vertical stroke through the first cell: two columns there.
    let x = (geometry.columns[0] + geometry.columns[1]) / 2.0;
    document.draw_stroke((x, top + 1.0), (x, bottom - 1.0), None).unwrap();
    assert_eq!(document.word_document().table_dimensions(0), Some((1, 3)));

    // The eraser on the new line joins the two halves again.
    let table_id = document.cell_layouts()[0].table_id;
    let geometry = document.table_geometry(table_id).unwrap();
    assert!(document.erase_border(geometry.columns[1], middle).unwrap());
    let table = document.word_document().table_by_order(0).unwrap();
    assert_eq!(table.rows[0].cells.len(), 2);
    assert_eq!(table.rows[0].cells[0].span, Some(2));

    // A rectangle below the table: a one-cell table that wide.
    let mut document = blank();
    document.place_caret(0, 0);
    document.insert_text("本文").unwrap();
    let caret = document
        .draw_stroke((100.0, 200.0), (340.0, 260.0), Some((0, 0)))
        .unwrap()
        .expect("a table was drawn");
    document.place_caret(caret.0, caret.1);
    let cell = document.caret_table_cell().expect("in the drawn table");
    assert_eq!((cell.rows, cell.columns), (1, 1));
    let table = document.word_document().table_by_order(0).unwrap();
    assert!((table.columns[0].width.unwrap() - 180.0).abs() < 1.0);
}

/// 罫線, 罫線と網かけ, 文字列の方向, 自動調整.
#[test]
fn borders_text_direction_and_autofit() {
    let mut document = with_table(2, 2);
    let pen = Pen {
        style: "double".to_string(),
        width: 1.5,
        color: "C00000".to_string(),
    };
    document.apply_border_set(BorderSet::Bottom, &pen).unwrap();
    let mut dialog = BordersDialog::new(BorderTarget::Table);
    dialog.pen = Pen {
        style: "dashed".to_string(),
        width: 0.75,
        color: "2F5496".to_string(),
    };
    dialog.preset(BorderSet::Outside);
    dialog.fill = Some(Some("FFF2CC".to_string()));
    document.apply_borders(&dialog).unwrap();
    document.cycle_text_direction().unwrap();
    assert_eq!(document.table_properties().unwrap().text_direction, "tbRlV");
    document.autofit(AutoFit::Window).unwrap();
    let xml = document_xml(&document.export_docx().unwrap());
    for expected in [
        "w:val=\"double\" w:sz=\"12\"",
        "w:color=\"C00000\"",
        "w:val=\"dashed\"",
        "w:fill=\"FFF2CC\"",
        "<w:textDirection w:val=\"tbRlV\"/>",
        "<w:tblW w:w=\"5000\" w:type=\"pct\"/>",
    ] {
        assert!(xml.contains(expected), "{expected} in {xml}");
    }
}
