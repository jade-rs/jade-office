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
    PropertiesTab, SortSpec, TableHit, TablePart, TablePropertiesDialog, TextSeparator, WordTables,
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
    let table = document
        .word_document()
        .table_by_order(order)
        .expect("table");
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
                                    (Some(field), true) => {
                                        field.cached_text.clone().unwrap_or_default()
                                    }
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
    assert!(styles
        .iter()
        .any(|style| style.name == "Grid Table 4 - Accent 1"));
    assert!(styles.iter().any(|style| style.id == "TableGrid"));

    document
        .apply_table_style(Some("GridTable4-Accent1"))
        .unwrap();
    assert_eq!(
        document.table_style_id().as_deref(),
        Some("GridTable4-Accent1")
    );
    document
        .set_look_option(LookOption::TotalRow, true)
        .unwrap();
    document
        .set_look_option(LookOption::BandedColumns, true)
        .unwrap();
    document
        .set_look_option(LookOption::HeaderRow, false)
        .unwrap();
    let look = document.table_look().unwrap();
    assert!(look.total_row && look.banded_columns && !look.header_row && look.banded_rows);

    let saved = document.export_docx().unwrap();
    let xml = document_xml(&saved);
    assert!(
        xml.contains("<w:tblStyle w:val=\"GridTable4-Accent1\"/>"),
        "{xml}"
    );
    assert!(
        xml.contains("w:lastRow=\"1\"") && xml.contains("w:noVBand=\"0\""),
        "{xml}"
    );
    let mut reopened = open_bytes(&saved);
    reopened.place_caret(cell_paragraph(&reopened, 0, 0, 0), 0);
    assert_eq!(
        reopened.table_style_id().as_deref(),
        Some("GridTable4-Accent1")
    );
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
    let hit = document
        .table_hit(geometry.left() - 10.0, (top + bottom) / 2.0)
        .unwrap();
    assert_eq!(hit, TableHit::RowSelector { table_id, row: 2 });
    document.click_table_hit(hit).unwrap();
    assert_eq!(document.cell_span().unwrap().rows, (2, 2));

    let x = (geometry.columns[1] + geometry.columns[2]) / 2.0;
    let hit = document.table_hit(x, geometry.top() - 6.0).unwrap();
    assert_eq!(
        hit,
        TableHit::ColumnSelector {
            table_id,
            column: 1
        }
    );
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
        TableHit::ColumnBorder {
            table_id,
            boundary: 1,
            x: before.columns[1]
        }
    );
    document
        .drag_column_border(table_id, 1, before.columns[1] + 40.0)
        .unwrap();
    let table_id = caret_table_id(&document);
    let after = document.table_geometry(table_id).unwrap();
    let (old, new) = (before.widths_pt(), after.widths_pt());
    assert!((new[0] - old[0] - 30.0).abs() < 1.0, "{old:?} → {new:?}");
    assert!((old[1] - new[1] - 30.0).abs() < 1.0, "{old:?} → {new:?}");
    assert!((new[2] - old[2]).abs() < 1.0);

    document
        .drag_column_border(table_id, 3, after.right() - 20.0)
        .unwrap();
    let table_id = caret_table_id(&document);
    let narrower = document.table_geometry(table_id).unwrap().widths_pt();
    assert!(
        (new[2] - narrower[2] - 15.0).abs() < 1.0,
        "{new:?} → {narrower:?}"
    );

    let geometry = document.table_geometry(table_id).unwrap();
    let (top, bottom) = geometry.rows[0];
    let hit = document.table_hit(geometry.left() + 20.0, bottom).unwrap();
    assert_eq!(
        hit,
        TableHit::RowBorder {
            table_id,
            row: 0,
            y: bottom
        }
    );
    document.drag_row_border(table_id, 0, top + 80.0).unwrap();
    let table_id = caret_table_id(&document);
    let (top, bottom) = document.table_geometry(table_id).unwrap().rows[0];
    assert!((bottom - top - 80.0).abs() < 2.0, "{}", bottom - top);

    let saved = document.export_docx().unwrap();
    let reopened = open_bytes(&saved);
    let table = reopened.word_document().table_by_order(0).unwrap();
    let widths: Vec<f32> = table
        .columns
        .iter()
        .map(|column| column.width.unwrap())
        .collect();
    assert!(
        (widths[0] - narrower[0]).abs() < 1.0,
        "{widths:?} {narrower:?}"
    );
    assert!(
        (table.rows[0].height.unwrap() - 60.0).abs() < 1.5,
        "{:?}",
        table.rows[0].height
    );
}

/// 表のプロパティ: each tab's settings land on the table, its rows, its
/// columns and its cells, and are saved as Word saves them.
#[test]
fn table_properties_set_the_table_rows_columns_and_cells() {
    let mut document = with_table(3, 2);
    let paragraph = cell_paragraph(&document, 0, 1, 1);
    document.place_caret(paragraph, 0);
    let properties = document.table_properties().unwrap();
    assert!(
        !properties.autofit,
        "an inserted table keeps its column widths"
    );
    let mut dialog = TablePropertiesDialog::new(properties, PropertiesTab::Table);
    dialog.width_on = true;
    dialog.width_percent = true;
    dialog.text.insert(Field::TableWidth, "80".to_string());
    dialog.alignment = "center".to_string();
    dialog.floating = true;
    dialog.position_x_align = Some("right".to_string());
    dialog.horizontal_anchor = "margin".to_string();
    dialog
        .text
        .insert(Field::PositionVertical, "20".to_string());
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
    assert!(
        matches!(read.width, Some(jade_office_model::word::table::PreferredWidth::Percent(p)) if (p - 80.0).abs() < 0.1)
    );
    assert_eq!(read.alignment, "center");
    assert!(read.floating);
    assert_eq!(read.position.horizontal_align.as_deref(), Some("right"));
    assert!(
        (read.spacing.unwrap() - 2.83).abs() < 0.05,
        "{:?}",
        read.spacing
    );
    assert!((read.row_height.unwrap() - 34.0).abs() < 0.2 && read.row_exact);
    assert!(!read.allow_break && read.repeat_header);
    assert!(
        (read.column_width.unwrap() - 113.4).abs() < 0.5,
        "{:?}",
        read.column_width
    );
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
    assert_eq!(
        (
            texts[0][1].as_str(),
            texts[1][1].as_str(),
            texts[2][1].as_str()
        ),
        ("", "b", "d")
    );
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
    let caret = document
        .convert_text_to_table(&TextSeparator::Tab, None)
        .unwrap();
    document.place_caret(caret.0, caret.1);
    assert_eq!(
        cell_texts(&document, 0),
        vec![vec!["品目", "数量"], vec!["りんご", "3"]]
    );
    assert!(document.caret_table_cell().is_some());
    assert!(document
        .paragraph_text(document.paragraph_count() - 1)
        .unwrap()
        .contains("おわり"));

    document
        .convert_table_to_text(&TextSeparator::Comma)
        .unwrap();
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
        &[
            &["品目", "金額"],
            &["みかん", "300"],
            &["りんご", "1,200"],
            &["ぶどう", "800"],
        ],
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
    document
        .insert_formula("=SUM(ABOVE)", Some("#,##0"))
        .unwrap();
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
    assert_eq!(
        cell.table_order_index, 1,
        "the caret is in the nested table"
    );
    assert_eq!((cell.rows, cell.columns), (2, 2));
    // Word's 挿入 ▸ 表 gives a table its lines: 表 (格子).
    assert_eq!(document.table_style_id().as_deref(), Some("TableGrid"));
    document.insert_text("内側").unwrap();
    let caret = document
        .table_command(TableCommand::InsertRowBelow)
        .unwrap();
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
    document
        .draw_stroke((x, top + 1.0), (x, bottom - 1.0), None)
        .unwrap();
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

/// ホーム › 並べ替え outside a table: the selected paragraphs by 段落, with a
/// 見出し行, through the same dialog the table sorts with; one undo.
#[test]
fn paragraphs_sort_from_the_sort_dialog() {
    use jade_office_model::word::table_ui::{TableAction, TableDialog, TableTools};
    let mut document = blank();
    document.place_caret(0, 0);
    for (index, text) in ["果物", "みかん", "Banana", "apple"].iter().enumerate() {
        if index > 0 {
            document.insert_paragraph_break().unwrap();
        }
        document.insert_text(text).unwrap();
    }
    document.insert_paragraph_break().unwrap();
    document.insert_text("おわり").unwrap();
    document.select((0, 0), (3, 5)).unwrap();
    let mut tools = TableTools::default();
    tools.action(&mut document, TableAction::Sort);
    let Some(TableDialog::Sort { headings, keys, .. }) = tools.dialog.as_mut() else {
        panic!("the sort dialog opens outside a table");
    };
    assert!(headings.is_empty(), "the 段落 key, not columns");
    keys[0] = Some((
        0,
        jade_docx_headless_api::HeadlessTableSortKind::Text,
        false,
    ));
    if let Some(TableDialog::Sort { header, .. }) = tools.dialog.as_mut() {
        *header = true;
    }
    tools.apply_dialog(&mut document);
    let texts: Vec<String> = (0..5)
        .map(|index| document.paragraph_text(index).unwrap_or_default())
        .collect();
    assert_eq!(texts, vec!["果物", "apple", "Banana", "みかん", "おわり"]);
    let reopened = open_bytes(&document.export_docx().unwrap());
    assert_eq!(reopened.paragraph_text(1).as_deref(), Some("apple"));
    document.undo().unwrap();
    assert_eq!(document.paragraph_text(1).as_deref(), Some("みかん"));

    // 降順 by number.
    let mut document = blank();
    document.place_caret(0, 0);
    for (index, text) in ["2 件", "10 件", "7 件"].iter().enumerate() {
        if index > 0 {
            document.insert_paragraph_break().unwrap();
        }
        document.insert_text(text).unwrap();
    }
    document.select((0, 0), (2, 3)).unwrap();
    document
        .sort_table(&SortSpec {
            keys: vec![jade_docx_headless_api::HeadlessTableSortKey {
                column: 0,
                kind: jade_docx_headless_api::HeadlessTableSortKind::Number,
                descending: true,
            }],
            header_row: false,
        })
        .unwrap();
    assert_eq!(document.paragraph_text(0).as_deref(), Some("10 件"));
    assert_eq!(document.paragraph_text(2).as_deref(), Some("2 件"));
}

/// セル › オプション › 文字列をセル幅に合わせる: the cell's text is spread
/// to the cell's width on the page, saved as `w:tcFitText` and read back.
#[test]
fn fit_text_spreads_a_cell_and_is_saved() {
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["氏名", ""]]);
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    let before = document.page(0).unwrap().encode_png().unwrap();
    let properties = document.table_properties().unwrap();
    assert!(!properties.fit_text);
    let mut dialog = TablePropertiesDialog::new(properties, PropertiesTab::Cell);
    dialog.fit_text = true;
    document.apply_table_properties(&dialog).unwrap();

    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    assert!(document.table_properties().unwrap().fit_text);
    let after = document.page(0).unwrap().encode_png().unwrap();
    assert_ne!(before, after, "the cell's text is spread on the page");
    let bytes = document.export_docx().unwrap();
    let xml = document_xml(&bytes);
    assert_eq!(xml.matches("<w:tcFitText/>").count(), 1, "{xml}");

    let mut reopened = open_bytes(&bytes);
    reopened.place_caret(cell_paragraph(&reopened, 0, 0, 0), 0);
    assert!(reopened.table_properties().unwrap().fit_text);
    reopened.place_caret(cell_paragraph(&reopened, 0, 0, 1), 0);
    assert!(!reopened.table_properties().unwrap().fit_text);
}

/// 表 › 表の方向 › 右から左: the first column moves to the right edge of the
/// page, saved as `w:bidiVisual` and read back.
#[test]
fn table_direction_mirrors_the_columns_and_is_saved() {
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["一", "二"]]);
    let first_left = |document: &EditableDocument| {
        let left = |column: usize| {
            document
                .cell_layouts()
                .iter()
                .find(|cell| cell.row == 0 && cell.column == column)
                .map(|cell| cell.left)
                .unwrap()
        };
        left(0) < left(1)
    };
    assert!(first_left(&document));
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    let properties = document.table_properties().unwrap();
    assert!(!properties.right_to_left);
    let mut dialog = TablePropertiesDialog::new(properties, PropertiesTab::Table);
    dialog.right_to_left = true;
    document.apply_table_properties(&dialog).unwrap();

    assert!(
        !first_left(&document),
        "the first column is drawn at the right"
    );
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    assert!(document.table_properties().unwrap().right_to_left);
    let bytes = document.export_docx().unwrap();
    assert!(document_xml(&bytes).contains("<w:bidiVisual/>"));
    let mut reopened = open_bytes(&bytes);
    reopened.place_caret(cell_paragraph(&reopened, 0, 0, 0), 0);
    assert!(reopened.table_properties().unwrap().right_to_left);
    assert!(!first_left(&reopened));
}

/// 代替テキスト: typed into the tab's boxes, saved as `w:tblCaption` and
/// `w:tblDescription`, read back into the dialog.
#[test]
fn alt_text_is_typed_saved_and_read_back() {
    let mut document = with_table(2, 2);
    let properties = document.table_properties().unwrap();
    assert_eq!(properties.alt_title, "");
    let mut dialog = TablePropertiesDialog::new(properties, PropertiesTab::AltText);
    dialog.type_text("社員名簿");
    dialog.next_alt_field();
    dialog.type_text("氏名と部署の一覧");
    document.apply_table_properties(&dialog).unwrap();

    let bytes = document.export_docx().unwrap();
    let xml = document_xml(&bytes);
    assert!(xml.contains(r#"<w:tblCaption w:val="社員名簿"/>"#), "{xml}");
    assert!(
        xml.contains(r#"<w:tblDescription w:val="氏名と部署の一覧"/>"#),
        "{xml}"
    );
    let mut reopened = open_bytes(&bytes);
    reopened.place_caret(cell_paragraph(&reopened, 0, 0, 0), 0);
    let read = reopened.table_properties().unwrap();
    assert_eq!(read.alt_title, "社員名簿");
    assert_eq!(read.alt_description, "氏名と部署の一覧");
    // Emptied, it is taken off.
    let mut dialog = TablePropertiesDialog::new(read, PropertiesTab::AltText);
    dialog.alt_title.text.clear();
    reopened.apply_table_properties(&dialog).unwrap();
    let xml = document_xml(&reopened.export_docx().unwrap());
    assert!(!xml.contains("tblCaption"), "{xml}");
    assert!(xml.contains("tblDescription"), "{xml}");
}

/// Enter at the very start of a table at the top of the document: Word puts
/// an empty paragraph in front of the table and leaves the caret in the
/// cell; with text before the table, Enter splits the cell's paragraph.
#[test]
fn enter_at_the_start_of_a_leading_table_adds_a_paragraph_above_it() {
    let mut document = with_table(2, 2);
    fill(&mut document, 0, &[&["a", "b"], &["c", "d"]]);
    let first = cell_paragraph(&document, 0, 0, 0);
    assert_eq!(first, 0, "the table starts the document");
    document.place_caret(first, 0);
    document.insert_paragraph_break().unwrap();
    assert_eq!(document.cell_id_for_paragraph(0), None);
    assert_eq!(document.paragraph_text(0).unwrap(), "");
    let caret = document.caret().unwrap();
    assert_eq!(caret, (cell_paragraph(&document, 0, 0, 0), 0));
    assert_eq!(cell_texts(&document, 0), [["a", "b"], ["c", "d"]]);

    // One undo takes the paragraph away again.
    document.undo().unwrap();
    assert_eq!(cell_paragraph(&document, 0, 0, 0), 0);
    assert_eq!(cell_texts(&document, 0), [["a", "b"], ["c", "d"]]);

    // Now the paragraph before the table is there: Enter splits the cell.
    document.redo().unwrap();
    let first = cell_paragraph(&document, 0, 0, 0);
    document.place_caret(first, 0);
    document.insert_paragraph_break().unwrap();
    assert_eq!(cell_texts(&document, 0), [["\na", "b"], ["c", "d"]]);
}

/// An empty paragraph beside a table goes as in Word: Delete in one just
/// before the table, Backspace in one just after it, the caret going into
/// the table's nearest cell; undo brings it back and a save has it gone.
#[test]
fn delete_and_backspace_remove_an_empty_paragraph_beside_a_table() {
    let mut document = blank();
    document.place_caret(0, 0);
    document.insert_text("before").unwrap();
    document.insert_paragraph_break().unwrap();
    let caret = WordFormatting::insert_table(&mut document, 1, 2).unwrap();
    document.place_caret(caret.0, caret.1);
    fill(&mut document, 0, &[&["a", "b"]]);
    let last = document.paragraph_count() - 1;
    document.place_caret(last, 0);
    document.insert_text("after").unwrap();
    let texts = |document: &EditableDocument| -> Vec<String> {
        (0..document.paragraph_count())
            .map(|paragraph| document.paragraph_text(paragraph).unwrap())
            .collect()
    };
    assert_eq!(texts(&document), ["before", "a", "b", "after"]);

    // Delete in an empty paragraph before the table.
    document.place_caret(0, 6);
    document.insert_paragraph_break().unwrap();
    assert_eq!(texts(&document), ["before", "", "a", "b", "after"]);
    document.delete(false).unwrap();
    assert_eq!(texts(&document), ["before", "a", "b", "after"]);
    assert_eq!(
        document.caret(),
        Some((cell_paragraph(&document, 0, 0, 0), 0))
    );
    document.undo().unwrap();
    assert_eq!(texts(&document), ["before", "", "a", "b", "after"]);
    document.redo().unwrap();

    // Backspace in an empty paragraph after the table.
    document.place_caret(3, 0);
    document.insert_paragraph_break().unwrap();
    assert_eq!(texts(&document), ["before", "a", "b", "", "after"]);
    document.place_caret(3, 0);
    document.delete(true).unwrap();
    assert_eq!(texts(&document), ["before", "a", "b", "after"]);
    assert_eq!(
        document.caret(),
        Some((cell_paragraph(&document, 0, 0, 1), 1))
    );

    // Text in the paragraph, or a table on both sides: nothing goes.
    document.place_caret(3, 0);
    document.delete(true).unwrap();
    document.place_caret(0, 6);
    document.delete(false).unwrap();
    assert_eq!(texts(&document), ["before", "a", "b", "after"]);

    let xml = document_xml(&document.export_docx().unwrap());
    assert_eq!(
        xml.matches("<w:p>").count() + xml.matches("<w:p ").count(),
        4
    );
}

/// Selected cells take Backspace and Delete as Word takes them: Backspace
/// deletes whole rows, columns or the table; Delete, or Backspace over cells
/// that are not whole rows or columns, empties just the selected cells. 行の
/// 削除 and 列の削除 take every selected row or column.
#[test]
fn backspace_and_delete_on_selected_cells_act_as_word_does() {
    fn filled() -> EditableDocument {
        let mut document = with_table(3, 3);
        fill(
            &mut document,
            0,
            &[&["a", "b", "c"], &["d", "e", "f"], &["g", "h", "i"]],
        );
        document
    }
    fn select(document: &mut EditableDocument, part: TablePart) {
        let paragraph = cell_paragraph(document, 0, 1, 1);
        document.place_caret(paragraph, 0);
        document.select_table_part(part).unwrap();
    }

    // Backspace: the row, the column, the table.
    let mut document = filled();
    select(&mut document, TablePart::Row);
    document.delete(true).unwrap();
    assert_eq!(cell_texts(&document, 0), [["a", "b", "c"], ["g", "h", "i"]]);
    document.undo().unwrap();
    assert_eq!(cell_texts(&document, 0).len(), 3);

    let mut document = filled();
    select(&mut document, TablePart::Column);
    document.delete(true).unwrap();
    assert_eq!(
        cell_texts(&document, 0),
        [["a", "c"], ["d", "f"], ["g", "i"]]
    );

    let mut document = filled();
    select(&mut document, TablePart::Table);
    document.delete(true).unwrap();
    assert!(document.word_document().table_by_order(0).is_none());

    // Delete empties exactly the selected cells.
    let mut document = filled();
    select(&mut document, TablePart::Column);
    document.delete(false).unwrap();
    assert_eq!(
        cell_texts(&document, 0),
        [["a", "", "c"], ["d", "", "f"], ["g", "", "i"]]
    );
    document.undo().unwrap();
    assert_eq!(cell_texts(&document, 0)[1], ["d", "e", "f"]);

    let mut document = filled();
    select(&mut document, TablePart::Table);
    document.delete(false).unwrap();
    assert_eq!(
        cell_texts(&document, 0),
        [["", "", ""], ["", "", ""], ["", "", ""]]
    );

    // Backspace over cells that are not whole rows or columns empties them.
    let mut document = filled();
    let from = cell_paragraph(&document, 0, 0, 0);
    let to = cell_paragraph(&document, 0, 1, 1);
    assert!(document.select_cells_between(from, to).unwrap());
    document.delete(true).unwrap();
    assert_eq!(
        cell_texts(&document, 0),
        [["", "", "c"], ["", "", "f"], ["g", "h", "i"]]
    );

    // 行の削除 with two rows selected takes both.
    let mut document = filled();
    let from = cell_paragraph(&document, 0, 0, 1);
    let to = cell_paragraph(&document, 0, 1, 1);
    assert!(document.select_cells_between(from, to).unwrap());
    WordFormatting::table_command(&mut document, TableCommand::DeleteRow).unwrap();
    assert_eq!(cell_texts(&document, 0), [["g", "h", "i"]]);
    // 列の削除 with every column selected takes the table.
    let mut document = filled();
    let from = cell_paragraph(&document, 0, 2, 0);
    let to = cell_paragraph(&document, 0, 2, 2);
    assert!(document.select_cells_between(from, to).unwrap());
    WordFormatting::table_command(&mut document, TableCommand::DeleteColumn).unwrap();
    assert!(document.word_document().table_by_order(0).is_none());
}

#[test]
fn a_copied_table_pastes_as_a_table() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = with_table(2, 2);
    document
        .apply_table_style(Some("GridTable4-Accent1"))
        .unwrap();
    fill(&mut document, 0, &[&["a", "b"], &["c", "d"]]);
    let after = document.paragraph_count() - 1;
    // The whole table, from its first cell to the paragraph after it.
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.tables.len(), 1);
    assert_eq!(clip.tables[0].columns, 2);

    document.place_caret(after, 0);
    document.paste_rich(&clip).unwrap();
    assert_eq!(cell_texts(&document, 1), [["a", "b"], ["c", "d"]]);
    // It takes the copied table's style.
    let style = |order: usize| {
        document
            .word_document()
            .table_by_order(order)
            .and_then(|table| table.style_name.clone())
    };
    assert_eq!(style(0).as_deref(), Some("GridTable4-Accent1"));
    assert_eq!(style(1), style(0));
    let xml = document_xml(&document.export_docx().unwrap());
    assert_eq!(xml.matches("<w:tbl>").count(), 2, "{xml}");
    // One undo takes the whole table back.
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
    assert_eq!(document.paragraph_count(), after + 1);
}

#[test]
fn a_table_pasted_inside_a_line_gets_lines_of_its_own() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["x", "y"]]);
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    document.place_caret(after, 0);
    document.insert_text("left right").unwrap();
    document.place_caret(after, "left".len());
    document.paste_rich(&clip).unwrap();
    assert_eq!(cell_texts(&document, 1), [["x", "y"]]);
    let texts: Vec<String> = (0..document.paragraph_count())
        .filter(|&index| document.cell_id_for_paragraph(index).is_none())
        .filter_map(|index| document.paragraph_text(index))
        .collect();
    assert_eq!(texts, ["left", " right"]);
}

#[test]
fn text_copied_from_one_cell_pastes_as_text() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["xyz", "w"]]);
    let cell = cell_paragraph(&document, 0, 0, 0);
    document.select((cell, 0), (cell, 2)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert!(clip.tables.is_empty());
    let after = document.paragraph_count() - 1;
    document.place_caret(after, 0);
    document.paste_rich(&clip).unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
    assert_eq!(document.paragraph_text(after).as_deref(), Some("xy"));
}

#[test]
fn a_copied_cell_of_several_paragraphs_keeps_them() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["x", "w"]]);
    let cell = cell_paragraph(&document, 0, 0, 0);
    document.place_caret(cell, 1);
    document.insert_paragraph_break().unwrap();
    document.insert_text("y").unwrap();
    assert_eq!(cell_texts(&document, 0), [["x\ny", "w"]]);
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.tables.len(), 1);
    assert_eq!(clip.tables[0].cell_paragraphs, [2, 1]);

    document.place_caret(after, 0);
    let caret = document.paste_rich(&clip).unwrap();
    assert_eq!(cell_texts(&document, 1), [["x\ny", "w"]]);
    // The caret is in the paragraph after the pasted table.
    assert_eq!(caret, (document.paragraph_count() - 1, 0));
    assert!(document.cell_id_for_paragraph(caret.0).is_none());
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
    assert_eq!(document.paragraph_count(), after + 1);
}

#[test]
fn a_pasted_table_keeps_its_column_widths_and_shading() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::HeadlessCommand;
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["x", "y"]]);
    document
        .apply_commands(
            vec![
                HeadlessCommand::SetTableColumnWidths {
                    table_order_index: 0,
                    widths: vec![100.0, 200.0],
                },
                HeadlessCommand::SetTableCellShading {
                    table_order_index: 0,
                    row: 0,
                    column: 1,
                    color: Some("#FF0000".into()),
                },
            ],
            false,
        )
        .unwrap();
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    document.place_caret(after, 0);
    document.paste_rich(&clip).unwrap();

    let table = document.word_document().table_by_order(1).unwrap();
    let widths: Vec<_> = table.columns.iter().map(|column| column.width).collect();
    assert_eq!(widths, [Some(100.0), Some(200.0)]);
    let fill = |column: usize| {
        table.rows[0].cells[column]
            .style
            .get("background-color")
            .cloned()
    };
    assert_eq!(fill(1).as_deref(), Some("#FF0000"));
    assert_ne!(fill(0).as_deref(), Some("#FF0000"));
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
}

#[test]
fn a_pasted_table_keeps_its_merged_cells() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::{HeadlessCommand, HeadlessTableCellSelection};
    let mut document = with_table(2, 3);
    fill(&mut document, 0, &[&["a", "b", "c"], &["d", "e", ""]]);
    let cell = |row, column| HeadlessTableCellSelection {
        row,
        column,
        column_span: 1,
    };
    document
        .apply_command(
            HeadlessCommand::MergeTableCells {
                table_order_index: 0,
                selected_cells: vec![cell(0, 0), cell(0, 1)],
            },
            false,
        )
        .unwrap();
    document
        .apply_command(
            HeadlessCommand::MergeTableCells {
                table_order_index: 0,
                selected_cells: vec![cell(0, 2), cell(1, 2)],
            },
            false,
        )
        .unwrap();
    let shape = |document: &EditableDocument, order: usize| {
        let table = document.word_document().table_by_order(order).unwrap();
        table
            .rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .map(|cell| (cell.span.unwrap_or(1), cell.vertical_merge.clone()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let source = shape(&document, 0);
    let texts = cell_texts(&document, 0);
    assert_eq!(texts, [vec!["a\nb", "c"], vec!["d", "e", ""]]);

    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.tables.len(), 1);
    document.place_caret(after, 0);
    document.paste_rich(&clip).unwrap();
    assert_eq!(shape(&document, 1), source);
    assert_eq!(cell_texts(&document, 1), texts);
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
}

/// Copied rows pasted with the caret in a table go in as new rows above
/// the caret's, as Word inserts them, not as text in one cell; one undo
/// takes them back.
#[test]
fn copied_rows_pasted_in_a_table_go_in_as_rows() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = with_table(2, 2);
    fill(&mut document, 0, &[&["a", "b"], &["c", "d"]]);
    document
        .apply_commands(
            vec![
                jade_web::word::headless::HeadlessCommand::SetTableCellShading {
                    table_order_index: 0,
                    row: 0,
                    column: 0,
                    color: Some("#FFFF00".into()),
                },
            ],
            false,
        )
        .unwrap();
    let first = cell_paragraph(&document, 0, 0, 0);
    let second_row = cell_paragraph(&document, 0, 1, 0);
    document.select((first, 0), (second_row, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.tables.len(), 1);

    let caret_cell = cell_paragraph(&document, 0, 1, 1);
    document.place_caret(caret_cell, 0);
    let caret = document.paste_rich(&clip).unwrap();
    assert_eq!(
        cell_texts(&document, 0),
        [["a", "b"], ["a", "b"], ["c", "d"]]
    );
    assert!(document.word_document().table_by_order(1).is_none());
    // The pasted row keeps its shading.
    let xml = document_xml(&document.export_docx().unwrap());
    assert_eq!(xml.matches("FFFF00").count(), 2, "{xml}");
    // The caret stays in its cell, now a row down.
    assert_eq!(caret, (cell_paragraph(&document, 0, 2, 1), 0));
    document.undo().unwrap();
    assert_eq!(cell_texts(&document, 0), [["a", "b"], ["c", "d"]]);
}

/// What a copied cell holds lands in that cell of the pasted table: a
/// link in the second cell is on the second cell's text again.
#[test]
fn a_link_in_a_copied_cell_lands_in_its_cell() {
    use jade_web::editing::WordLinks;
    use jade_web::word::clipboard::WordClipboard;
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["plain", "linked"]]);
    let linked = cell_paragraph(&document, 0, 0, 1);
    document.select((linked, 0), (linked, 6)).unwrap();
    let draft = document.link_draft();
    document
        .apply_link(&draft, &draft.display, "www.example.com")
        .unwrap();
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    document.place_caret(after, 0);
    document.insert_text("x").unwrap();
    // In the middle of a line, so the table takes lines of its own.
    document.place_caret(after, 1);
    document.paste_rich(&clip).unwrap();
    let pasted = cell_paragraph(&document, 1, 0, 1);
    let link = document.hyperlink_at((pasted, 2)).expect("the pasted link");
    assert_eq!((link.start, link.end), (0, 6));
    assert!(document
        .hyperlink_at((cell_paragraph(&document, 1, 0, 0), 2))
        .is_none());
}

#[test]
fn a_pasted_table_keeps_its_heading_row_and_row_heights() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::HeadlessCommand;
    let mut document = with_table(3, 2);
    fill(&mut document, 0, &[&["h", "h"], &["a", "b"], &["c", "d"]]);
    document
        .apply_command(
            HeadlessCommand::SetTableRowProperties {
                table_order_index: 0,
                row: 0,
                repeat_header: Some(true),
                cant_split: Some(true),
            },
            true,
        )
        .unwrap();
    document
        .apply_command(
            HeadlessCommand::SetTableRowHeightProperties {
                table_order_index: 0,
                row: 1,
                mode: "exact".into(),
                height: Some(40.0),
            },
            true,
        )
        .unwrap();
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    document.place_caret(after, 0);
    document.paste_rich(&clip).unwrap();

    // Each pasted row sets what the copied one did.
    let rows = |order: usize| {
        document
            .word_document()
            .table_by_order(order)
            .unwrap()
            .rows
            .iter()
            .map(|row| {
                (
                    row.is_header,
                    row.cant_split,
                    row.height.map(|height| height.round()),
                    row.height_rule,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(1), rows(0));
    assert!(rows(0)[0].0 && rows(0)[0].1);
    assert_eq!(rows(0)[1].2, Some(40.0));
    let xml = document_xml(&document.export_docx().unwrap());
    assert_eq!(xml.matches("<w:tblHeader").count(), 2, "{xml}");
    assert_eq!(xml.matches("w:hRule=\"exact\"").count(), 2, "{xml}");
    // One undo takes the whole table back.
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
}

#[test]
fn rows_pasted_in_a_table_keep_their_own_heights() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::HeadlessCommand;
    let mut document = with_table(2, 2);
    fill(&mut document, 0, &[&["a", "b"], &["c", "d"]]);
    // The first row is exactly 40pt; the second is as tall as its text.
    document
        .apply_command(
            HeadlessCommand::SetTableRowHeightProperties {
                table_order_index: 0,
                row: 0,
                mode: "exact".into(),
                height: Some(40.0),
            },
            true,
        )
        .unwrap();
    let first = cell_paragraph(&document, 0, 0, 0);
    let second_row = cell_paragraph(&document, 0, 1, 0);
    document.select((first, 0), (second_row, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    let heights = |document: &EditableDocument| {
        document
            .word_document()
            .table_by_order(0)
            .unwrap()
            .rows
            .iter()
            .map(|row| {
                (row.height_rule != Some(jade_docx_editor::document::TableRowHeightRule::Auto))
                    .then_some(row.height.map(f32::round))
                    .flatten()
            })
            .collect::<Vec<_>>()
    };

    // Pasted above the second row, the new row is 40pt as the copied one
    // was, not as tall as its text like the row it went in above.
    document.place_caret(cell_paragraph(&document, 0, 1, 0), 0);
    document.paste_rich(&clip).unwrap();
    assert_eq!(heights(&document), [Some(40.0), Some(40.0), None]);
    // And pasted above the 40pt row, the copy of the second row is not.
    let lower = cell_paragraph(&document, 0, 2, 0);
    document
        .select((lower, 0), (document.paragraph_count() - 1, 0))
        .unwrap();
    let clip = document.copy_rich().unwrap();
    document.place_caret(cell_paragraph(&document, 0, 0, 0), 0);
    document.paste_rich(&clip).unwrap();
    assert_eq!(heights(&document), [None, Some(40.0), Some(40.0), None]);
}

#[test]
fn a_pasted_table_keeps_its_borders_alignment_and_cell_formatting() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::{HeadlessBorderEdge, HeadlessCommand};
    let mut document = with_table(2, 2);
    fill(&mut document, 0, &[&["a", "b"], &["c", "d"]]);
    let edge = || {
        Some(HeadlessBorderEdge {
            style: Some("double".into()),
            width: Some(1.5),
            color: Some("#FF0000".into()),
        })
    };
    document
        .apply_commands(
            vec![
                HeadlessCommand::SetTableAlignment {
                    table_order_index: 0,
                    alignment: "center".into(),
                },
                HeadlessCommand::SetTableCellVerticalAlignment {
                    table_order_index: 0,
                    row: 0,
                    column: 0,
                    alignment: "center".into(),
                },
                HeadlessCommand::SetTableCellTextDirection {
                    table_order_index: 0,
                    row: 0,
                    column: 1,
                    direction: Some("tbRl".into()),
                },
                HeadlessCommand::SetTableCellBorders {
                    table_order_index: 0,
                    row: 1,
                    column: 0,
                    top: edge(),
                    bottom: None,
                    left: None,
                    right: None,
                    tl2br: edge(),
                    tr2bl: None,
                },
                HeadlessCommand::SetTableBorders {
                    table_order_index: 0,
                    top: edge(),
                    bottom: edge(),
                    left: None,
                    right: None,
                    inside_h: None,
                    inside_v: None,
                },
            ],
            true,
        )
        .unwrap();
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    let tables = |xml: &str| {
        xml.match_indices("<w:tbl>")
            .map(|(at, _)| {
                let table = &xml[at..];
                table[..table.find("</w:tbl>").unwrap()].to_string()
            })
            .collect::<Vec<_>>()
    };
    let formatting = [
        "<w:jc w:val=\"center\"",
        "<w:vAlign w:val=\"center\"",
        "<w:textDirection",
        "<w:tl2br",
        "<w:tblBorders",
    ];

    // Pasted in the same document and in another, the copy is formatted as
    // the table was, cell by cell, and holds its own text.
    document.place_caret(after, 0);
    document.paste_rich(&clip).unwrap();
    assert_eq!(cell_texts(&document, 1), [["a", "b"], ["c", "d"]]);
    let cell = |document: &EditableDocument, order: usize| {
        document.word_document().table_by_order(order).unwrap().rows[0].cells[0]
            .vertical_align
            .clone()
    };
    assert_eq!(cell(&document, 1).as_deref(), Some("center"));
    let saved = tables(&document_xml(&document.export_docx().unwrap()));
    for mark in formatting {
        assert!(saved[1].contains(mark), "{mark}: {}", saved[1]);
    }
    let mut other = blank();
    other.place_caret(0, 0);
    other.paste_rich(&clip).unwrap();
    assert_eq!(cell(&other, 0).as_deref(), Some("center"));
    let reopened = open_bytes(&other.export_docx().unwrap());
    assert_eq!(cell(&reopened, 0).as_deref(), Some("center"));
    assert_eq!(cell_texts(&reopened, 0), [["a", "b"], ["c", "d"]]);

    // One undo takes the whole table back.
    document.undo().unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
}

#[test]
fn merged_padding_does_not_move_a_note_or_the_paste_caret() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::HeadlessBorderEdge;
    use jade_web::word::headless::{HeadlessCommand, HeadlessTableCellSelection};
    use jade_web::word::picture::WordPicture;
    for multi_paragraph in [false, true] {
        for copies in [1, 2] {
            let mut document = with_table(1, 3);
            fill(&mut document, 0, &[&["L", "", "R"]]);
            document
                .apply_command(
                    HeadlessCommand::MergeTableCells {
                        table_order_index: 0,
                        selected_cells: (0..2)
                            .map(|column| HeadlessTableCellSelection {
                                row: 0,
                                column,
                                column_span: 1,
                            })
                            .collect(),
                    },
                    false,
                )
                .unwrap();
            if multi_paragraph {
                document.place_caret(0, 1);
                document.insert_paragraph_break().unwrap();
                document.insert_text("B").unwrap();
                WordFormatting::set_alignment(&mut document, "right").unwrap();
            }
            let extra = usize::from(multi_paragraph);
            let right = cell_paragraph(&document, 0, 0, 2);
            document
                .apply_command(
                    HeadlessCommand::InsertFootnote {
                        paragraph: right,
                        offset: 0,
                        text: "note".into(),
                    },
                    false,
                )
                .unwrap();
            let edge = Some(HeadlessBorderEdge {
                style: Some("double".into()),
                width: Some(1.5),
                color: Some("#FF0000".into()),
            });
            document
                .apply_commands(
                    vec![
                        HeadlessCommand::SetTableAlignment {
                            table_order_index: 0,
                            alignment: "center".into(),
                        },
                        HeadlessCommand::SetTableBorders {
                            table_order_index: 0,
                            top: edge.clone(),
                            bottom: edge,
                            left: None,
                            right: None,
                            inside_h: None,
                            inside_v: None,
                        },
                        HeadlessCommand::SetTableCellShading {
                            table_order_index: 0,
                            row: 0,
                            column: 2,
                            color: Some("#00FF00".into()),
                        },
                        HeadlessCommand::SetTableCellVerticalAlignment {
                            table_order_index: 0,
                            row: 0,
                            column: 2,
                            alignment: "center".into(),
                        },
                        HeadlessCommand::SetTableRowProperties {
                            table_order_index: 0,
                            row: 0,
                            repeat_header: Some(true),
                            cant_split: Some(true),
                        },
                        HeadlessCommand::SetTableRowHeightProperties {
                            table_order_index: 0,
                            row: 0,
                            mode: "atLeast".into(),
                            height: Some(40.0),
                        },
                    ],
                    false,
                )
                .unwrap();
            let mut png = Vec::new();
            image::RgbaImage::from_pixel(2, 1, image::Rgba([255, 0, 0, 255]))
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            document.place_caret(
                right,
                document.word_document().rendered_paragraph_len(right),
            );
            document.insert_picture(png).unwrap();
            let after = (0..document.paragraph_count())
                .rfind(|&p| {
                    document
                        .word_document()
                        .is_top_level_body_paragraph_surface(p)
                })
                .unwrap();
            document.select((0, 0), (after, 0)).unwrap();
            let mut clip = document.copy_rich().unwrap();
            assert_eq!(clip.notes.len(), 1);
            assert_eq!(clip.pictures.len(), 1);
            assert!(clip.tables[0].format.is_some());
            assert_eq!(
                clip.plain_text(),
                if multi_paragraph {
                    "L\nB\nR\n"
                } else {
                    "L\nR\n"
                }
            );
            assert!(!document.selected_text().unwrap().contains("note"));
            let body_index = |document: &EditableDocument, surface| match document
                .word_document()
                .paragraph_story(surface)
                .unwrap()
            {
                jade_docx_editor::word_document::ParagraphStory::Body { index } => index,
                other => panic!("expected body paragraph, got {other:?}"),
            };
            let body_start = body_index(&document, after);
            if copies == 2 {
                let second = clip.clone();
                let offset = clip.paragraphs.len();
                clip.paragraphs.extend(second.paragraphs);
                clip.tables
                    .extend(second.tables.into_iter().map(|mut table| {
                        table.first += offset;
                        table
                    }));
                clip.pictures
                    .extend(second.pictures.into_iter().map(|mut picture| {
                        picture.paragraph += offset;
                        picture
                    }));
                clip.notes.extend(second.notes.into_iter().map(|mut note| {
                    note.paragraph += offset;
                    note
                }));
            }

            document.place_caret(after, 0);
            let caret = document.paste_rich(&clip).unwrap();
            assert_eq!(
                body_index(&document, caret.0),
                body_start + (3 + extra) * copies - 1
            );
            assert_eq!(caret.1, 0);
            assert!(document.cell_id_for_paragraph(caret.0).is_none());
            let right = cell_paragraph(&document, 1, 0, 2);
            assert_eq!(right, after + 1 + extra);
            assert_eq!(
                document.word_document().reference_marker_spans(right).len(),
                1
            );
            assert_eq!(document.paragraph_text(right).as_deref(), Some("R"));
            if copies == 2 {
                let later_right = cell_paragraph(&document, 2, 0, 2);
                assert_eq!(
                    body_index(&document, later_right),
                    body_start + 4 + 2 * extra
                );
                assert_eq!(
                    document
                        .word_document()
                        .reference_marker_spans(later_right)
                        .len(),
                    1
                );
                assert_eq!(document.paragraph_text(later_right).as_deref(), Some("R"));
            }
            let saved = document.export_docx().unwrap();
            let reopened = open_bytes(&saved);
            assert_eq!(
                reopened.word_document().reference_marker_spans(right).len(),
                1
            );
            for checked in [&document, &reopened] {
                for order in 1..=copies {
                    let table = checked.word_document().table_by_order(order).unwrap();
                    assert_eq!(table.rows[0].cells.len(), 2);
                    assert!(table.rows[0].is_header && table.rows[0].cant_split);
                    assert_eq!(table.rows[0].height, Some(40.0));
                    assert_eq!(
                        table.rows[0].cells[1].vertical_align.as_deref(),
                        Some("center")
                    );
                    let right = cell_paragraph(checked, order, 0, 2);
                    assert_eq!(
                        checked
                            .word_document()
                            .paragraph(right)
                            .unwrap()
                            .runs
                            .iter()
                            .filter(|r| r.image.is_some())
                            .count(),
                        1
                    );
                    if multi_paragraph {
                        let left = cell_paragraph(checked, order, 0, 0);
                        assert_eq!(
                            checked
                                .word_document()
                                .paragraph(left + 1)
                                .unwrap()
                                .properties
                                .as_ref()
                                .unwrap()
                                .alignment
                                .as_deref(),
                            Some("right")
                        );
                    }
                }
            }
            let xml = document_xml(&saved);
            assert_eq!(xml.matches("<w:tblBorders").count(), copies + 1);
            assert_eq!(xml.matches("w:fill=\"00FF00\"").count(), copies + 1);
            document.undo().unwrap();
            assert!(document.word_document().table_by_order(1).is_none());
        }
    }
}

#[test]
fn fields_in_a_second_cell_paragraph_never_replace_the_next_cell() {
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::headless::{HeadlessCommand, HeadlessFieldKind};
    for inside_text in [false, true] {
        let mut document = with_table(1, 2);
        fill(&mut document, 0, &[&["A", "XYZ"]]);
        document
            .apply_command(
                HeadlessCommand::InsertParagraphBreak {
                    paragraph: 0,
                    offset: 1,
                    count: 1,
                },
                false,
            )
            .unwrap();
        document
            .apply_command(
                HeadlessCommand::InsertField {
                    start_paragraph: 1,
                    start_offset: 0,
                    end_paragraph: 1,
                    end_offset: 0,
                    kind: HeadlessFieldKind::Page,
                    cached_text: "999".into(),
                },
                false,
            )
            .unwrap();
        let after = document.paragraph_count() - 1;
        document.select((0, 0), (after, 0)).unwrap();
        let clip = document.copy_rich().unwrap();
        assert_eq!(clip.fields.len(), 1);
        document.place_caret(after, 0);
        if inside_text {
            document.insert_text("leftRIGHT").unwrap();
            document.place_caret(after, 4);
        }
        document.paste_rich(&clip).unwrap();
        let left = cell_paragraph(&document, 1, 0, 0);
        let right = cell_paragraph(&document, 1, 0, 1);
        assert_eq!(document.paragraph_text(right).as_deref(), Some("XYZ"));
        assert!(document
            .word_document()
            .paragraph(left + 1)
            .unwrap()
            .runs
            .iter()
            .any(|r| r.field.is_some()));
        assert_eq!(document.paragraph_text(left).as_deref(), Some("A"));
        if inside_text {
            assert_eq!(document.paragraph_text(after).as_deref(), Some("left"));
            assert_eq!(document.paragraph_text(right + 1).as_deref(), Some("RIGHT"));
        }
        let reopened = open_bytes(&document.export_docx().unwrap());
        assert_eq!(reopened.paragraph_text(right).as_deref(), Some("XYZ"));
        assert!(reopened
            .word_document()
            .paragraph(left + 1)
            .unwrap()
            .runs
            .iter()
            .any(|r| r.field.is_some()));
    }
}

#[test]
fn keep_text_only_turns_table_cells_into_ordinary_paragraphs() {
    use jade_web::word::clipboard::{PasteMode, WordClipboard};
    let mut document = with_table(1, 2);
    fill(&mut document, 0, &[&["A", "B"]]);
    let after = document.paragraph_count() - 1;
    document.select((0, 0), (after, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.tables.len(), 1);
    document.place_caret(after, 0);
    document.paste_as(&clip, PasteMode::TextOnly).unwrap();
    assert!(document.word_document().table_by_order(1).is_none());
    assert_eq!(document.paragraph_text(after).as_deref(), Some("A"));
    assert_eq!(document.paragraph_text(after + 1).as_deref(), Some("B"));
}

#[test]
fn keep_text_only_in_a_table_does_not_insert_rows() {
    use jade_web::word::clipboard::{PasteMode, WordClipboard};
    let mut document = with_table(2, 2);
    fill(&mut document, 0, &[&["A", "B"], &["C", "D"]]);
    let second_row = cell_paragraph(&document, 0, 1, 0);
    document.select((0, 0), (second_row, 0)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.tables.len(), 1);
    let target = cell_paragraph(&document, 0, 1, 1);
    document.place_caret(target, 0);
    document.paste_as(&clip, PasteMode::TextOnly).unwrap();
    assert_eq!(
        document
            .word_document()
            .table_by_order(0)
            .unwrap()
            .rows
            .len(),
        2
    );
    assert!(document.word_document().table_by_order(1).is_none());
    assert_eq!(cell_texts(&document, 0), [["A", "B"], ["C", "A\nB\nD"]]);
    document.undo().unwrap();
    assert_eq!(cell_texts(&document, 0), [["A", "B"], ["C", "D"]]);
}

#[test]
fn links_and_ruby_follow_the_second_cell_paragraph_in_body_pastes() {
    use jade_web::word::clipboard::{ClipLink, ClipRuby, ClipTable, RichClip, WordClipboard};
    use jade_web::word::headless::HeadlessRuby;
    for ruby in [false, true] {
        for inside_text in [false, true] {
            let mut document = blank();
            document.place_caret(0, 0);
            if inside_text {
                document.insert_text("leftRIGHT").unwrap();
                document.place_caret(0, 4);
            }
            let mut clip = RichClip::from_text("A\nB\nXYZ");
            clip.tables.push(ClipTable {
                first: 0,
                cells: 3,
                columns: 2,
                cell_paragraphs: vec![2, 1],
                ..Default::default()
            });
            if ruby {
                clip.rubies.push(ClipRuby {
                    paragraph: 1,
                    offset: 0,
                    len: 1,
                    ruby: HeadlessRuby {
                        text: "bee".into(),
                        align: None,
                        size: None,
                        raise: None,
                        font: None,
                    },
                });
            } else {
                clip.links.push(ClipLink {
                    paragraph: 1,
                    offset: 0,
                    len: 1,
                    address: "https://example.com/kept".into(),
                    tooltip: None,
                });
            }
            document.paste_rich(&clip).unwrap();
            let left = cell_paragraph(&document, 0, 0, 0);
            let right = cell_paragraph(&document, 0, 0, 1);
            assert_eq!(document.paragraph_text(right).as_deref(), Some("XYZ"));
            let runs = &document.word_document().paragraph(left + 1).unwrap().runs;
            if ruby {
                assert!(runs.iter().any(|r| r
                    .properties
                    .as_ref()
                    .and_then(|p| p.ruby.as_ref())
                    .is_some_and(|r| r.text == "bee")));
            } else {
                assert!(runs.iter().any(|r| r.hyperlink.is_some()));
            }
            let right = document.word_document().paragraph(right).unwrap();
            assert!(right.runs.iter().all(|r| r.hyperlink.is_none()
                && r.properties
                    .as_ref()
                    .and_then(|p| p.ruby.as_ref())
                    .is_none()));
        }
    }
}

#[test]
fn merged_clip_places_its_note_in_r_and_caret_in_the_following_body_paragraph() {
    use jade_docx_editor::word_document::ParagraphStory;
    use jade_web::word::clipboard::{ClipNote, ClipTable, RichClip, WordClipboard};
    let mut document = blank();
    document.place_caret(0, 0);
    let mut clip = RichClip::from_text("L\nR");
    clip.tables.push(ClipTable {
        first: 0,
        cells: 2,
        columns: 3,
        cell_spans: vec![2, 1],
        merges: vec![(0, 0, 2, 1)],
        ..Default::default()
    });
    clip.notes.push(ClipNote {
        paragraph: 1,
        offset: 0,
        footnote: true,
        text: "note".into(),
        ..Default::default()
    });
    let caret = document.paste_rich(&clip).unwrap();
    let right = cell_paragraph(&document, 0, 0, 2);
    assert_eq!(right, 1);
    assert_eq!(
        document.word_document().reference_marker_spans(right).len(),
        1
    );
    assert_eq!(document.paragraph_text(right).as_deref(), Some("R"));
    // Paragraph surfaces also contain the new footnote's own story. The
    // caret is the second body paragraph after P, not that footnote surface.
    assert_eq!(
        document.word_document().paragraph_story(caret.0),
        Some(ParagraphStory::Body { index: 2 })
    );
    assert_eq!(caret.1, 0);
}

#[test]
fn later_cell_paragraphs_keep_their_individual_formats() {
    use jade_web::word::clipboard::WordClipboard;
    let mut source = with_table(1, 2);
    fill(&mut source, 0, &[&["A", "XYZ"]]);
    let left = cell_paragraph(&source, 0, 0, 0);
    source.place_caret(left, 1);
    source.insert_paragraph_break().unwrap();
    source.insert_text("B").unwrap();
    WordFormatting::set_alignment(&mut source, "right").unwrap();
    source.insert_paragraph_break().unwrap();
    source.insert_text("C").unwrap();
    WordFormatting::set_alignment(&mut source, "center").unwrap();
    let original: Vec<_> = (1..=2)
        .map(|i| {
            source
                .word_document()
                .paragraph(left + i)
                .unwrap()
                .properties
                .as_ref()
                .and_then(|p| p.alignment.clone())
        })
        .collect();
    source
        .select((left, 0), (source.paragraph_count() - 1, 0))
        .unwrap();
    let clip = source.copy_rich().unwrap();
    let mut target = blank();
    target.place_caret(0, 0);
    target.paste_rich(&clip).unwrap();
    let pasted = cell_paragraph(&target, 0, 0, 0);
    let formats: Vec<_> = (1..=2)
        .map(|i| {
            target
                .word_document()
                .paragraph(pasted + i)
                .unwrap()
                .properties
                .as_ref()
                .and_then(|p| p.alignment.clone())
        })
        .collect();
    assert_eq!(formats, original);
    assert_eq!(cell_texts(&target, 0), vec![vec!["A\nB\nC", "XYZ"]]);
    let reopened = open_bytes(&target.export_docx().unwrap());
    let formats: Vec<_> = (1..=2)
        .map(|i| {
            reopened
                .word_document()
                .paragraph(pasted + i)
                .unwrap()
                .properties
                .as_ref()
                .and_then(|p| p.alignment.clone())
        })
        .collect();
    assert_eq!(formats, original);
    target.undo().unwrap();
    assert!(target.word_document().table_by_order(0).is_none());
}
