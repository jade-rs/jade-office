//! The ribbon's commands on the browser build's Word document — the ones
//! ported from jade-ide's style menu and table tools, which the native app
//! runs too: toggles that read back and turn off, formatting on the word at
//! a bare caret, the style gallery, the list buttons, and the table tools
//! around the caret, each undoable.

use std::path::PathBuf;

use jade_web::editing::{
    style_gallery, CharacterFormat, EditableDocument, ListKind, ParagraphStyle, TableBorders,
    TableCommand, WordFormatting, WordSession,
};

fn contract() -> EditableDocument {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    let bytes = std::fs::read(core.join("contract.docx")).unwrap();
    EditableDocument::open(&bytes, 1.0, fonts, measurer).unwrap()
}

fn in_table(document: &EditableDocument, paragraph: usize) -> bool {
    document.cell_id_for_paragraph(paragraph).is_some()
}

/// A body paragraph outside any table whose text starts with a word.
fn body_paragraph(document: &EditableDocument) -> usize {
    (0..document.paragraph_count())
        .find(|&paragraph| {
            !in_table(document, paragraph)
                && document.paragraph_text(paragraph).is_some_and(|text| {
                    text.starts_with(|ch: char| ch.is_alphanumeric()) && text.chars().count() >= 4
                })
        })
        .expect("a paragraph with text")
}

fn select_start(document: &mut EditableDocument, paragraph: usize, chars: usize) {
    let text = document.paragraph_text(paragraph).unwrap();
    let end = text
        .char_indices()
        .nth(chars)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    document.select((paragraph, 0), (paragraph, end)).unwrap();
}

/// Bold goes on, reads back as current, keeps the selection, and the same
/// button takes it off again; undo steps through both.
#[test]
fn bold_toggles_on_the_selection() {
    let mut document = contract();
    let paragraph = body_paragraph(&document);
    select_start(&mut document, paragraph, 3);
    let was_bold = document.format_state().bold;

    document.toggle_format(CharacterFormat::Bold).unwrap();
    assert_eq!(document.format_state().bold, !was_bold);
    assert!(document.selection().is_some(), "the selection survives");
    assert!(document.is_dirty());

    document.toggle_format(CharacterFormat::Bold).unwrap();
    assert_eq!(document.format_state().bold, was_bold);
    document.undo().unwrap();
    assert_eq!(document.format_state().bold, !was_bold);

    // A swatch's bare hex and Word's 自動 both go through.
    document.set_font_color("C00000").unwrap();
    document.set_font_color("auto").unwrap();
}

/// With a bare caret in a word, a size applies to that word, as in Word.
#[test]
fn a_caret_in_a_word_formats_the_word() {
    let mut document = contract();
    let paragraph = body_paragraph(&document);
    let second = document
        .paragraph_text(paragraph)
        .unwrap()
        .char_indices()
        .nth(1)
        .unwrap()
        .0;
    document.place_caret(paragraph, second);
    WordFormatting::set_font_size(&mut document, 30.0).unwrap();
    assert_eq!(
        document.run_style_at((paragraph, 0)).unwrap().font_size,
        Some(30.0)
    );
}

/// A heading from the gallery lands on the paragraph and reads back as the
/// active entry; 標準 takes it off.
#[test]
fn the_gallery_applies_a_heading_and_normal() {
    let mut document = contract();
    let paragraph = body_paragraph(&document);
    document.place_caret(paragraph, 0);
    let styles = document.paragraph_styles();
    let gallery = style_gallery(&styles);
    let default = styles
        .into_iter()
        .find(|style| style.is_default)
        .map(|style| style.id);

    document
        .set_paragraph_style(gallery[1].id.as_deref())
        .unwrap();
    let state = document.format_state();
    assert!(gallery[1].is_active(state.style_id.as_deref(), default.as_deref()));
    assert!(!gallery[0].is_active(state.style_id.as_deref(), default.as_deref()));

    document.set_paragraph_style(None).unwrap();
    let state = document.format_state();
    assert!(gallery[0].is_active(state.style_id.as_deref(), default.as_deref()));
}

#[test]
fn the_gallery_drops_document_styles_that_are_presets() {
    let style = |id: &str, name: &str, is_default| ParagraphStyle {
        id: id.to_string(),
        name: name.to_string(),
        is_default,
    };
    let gallery = style_gallery(&[
        style("a", "Normal", true),
        style("1", "heading 1", false),
        style("Title", "Title", false),
        style("Heading4", "heading 4", false),
        style("Quote", "Quote", false),
    ]);
    let ids: Vec<Option<&str>> = gallery.iter().map(|entry| entry.id.as_deref()).collect();
    assert_eq!(
        ids,
        [
            None,
            Some("Heading1"),
            Some("Heading2"),
            Some("Heading3"),
            Some("Heading4"),
            Some("Quote"),
            Some("Title"),
        ]
    );
    assert!(
        gallery[0].is_active(Some("a"), Some("a")),
        "the default is 標準"
    );
    assert!(
        gallery[1].is_active(Some("1"), Some("a")),
        "Japanese Word's heading id"
    );
}

/// The bullet button makes a bulleted list and, pressed again, plain
/// paragraphs; the number button does the same with numbers.
#[test]
fn list_buttons_toggle() {
    let mut document = contract();
    let paragraph = body_paragraph(&document);
    document.place_caret(paragraph, 0);
    document.toggle_list(true).unwrap();
    assert_eq!(document.format_state().list, Some(ListKind::Bullet));
    document.toggle_list(false).unwrap();
    assert_eq!(document.format_state().list, Some(ListKind::Decimal));
    document.toggle_list(false).unwrap();
    assert_eq!(document.format_state().list, None);
}

/// A table goes in with the caret in its first cell; rows and columns are
/// inserted and deleted around the caret, cells merge and unmerge, and the
/// table goes again.
#[test]
fn table_tools_edit_the_caret_table() {
    let mut document = contract();
    let paragraph = body_paragraph(&document);
    let paragraphs = document.paragraph_count();
    document.place_caret(paragraph, 0);
    let caret = WordFormatting::insert_table(&mut document, 2, 3).unwrap();
    document.place_caret(caret.0, caret.1);
    let cell = document
        .caret_table_cell()
        .expect("the caret is in the table");
    assert_eq!(
        (cell.row, cell.column, cell.rows, cell.columns),
        (0, 0, 2, 3)
    );

    let caret = document
        .table_command(TableCommand::InsertRowAbove)
        .unwrap();
    document.place_caret(caret.0, caret.1);
    let cell = document.caret_table_cell().unwrap();
    assert_eq!((cell.row, cell.rows), (1, 3), "the caret's row moved down");

    let caret = document
        .table_command(TableCommand::InsertColumnRight)
        .unwrap();
    document.place_caret(caret.0, caret.1);
    let cell = document.caret_table_cell().unwrap();
    assert_eq!((cell.column, cell.columns), (0, 4));

    let caret = document.table_command(TableCommand::DeleteRow).unwrap();
    document.place_caret(caret.0, caret.1);
    assert_eq!(document.caret_table_cell().unwrap().rows, 2);

    // Merge the caret's cell with the one to its right.
    let here = document.caret_table_cell().unwrap();
    let right = document
        .cell_layouts()
        .iter()
        .find(|layout| {
            document.table_order_for(layout.table_id) == Some(here.table_order_index)
                && layout.row == here.row
                && layout.column == here.column + 1
        })
        .and_then(|layout| layout.paragraphs.first().copied())
        .unwrap();
    document.select(caret, (right, 0)).unwrap();
    assert!(document.caret_table_cell().unwrap().can_merge);
    let caret = document.table_command(TableCommand::MergeCells).unwrap();
    document.place_caret(caret.0, caret.1);
    assert!(document.caret_table_cell().unwrap().can_unmerge);
    let caret = document.table_command(TableCommand::UnmergeCells).unwrap();
    document.place_caret(caret.0, caret.1);
    assert!(!document.caret_table_cell().unwrap().can_unmerge);

    document
        .table_command(TableCommand::StylePreset("blueBands".to_string()))
        .unwrap();
    document
        .table_command(TableCommand::Borders(TableBorders::None))
        .unwrap();
    document
        .table_command(TableCommand::CellShading(Some("D9EAF7".to_string())))
        .unwrap();
    document
        .table_command(TableCommand::Align("center".to_string()))
        .unwrap();

    let caret = document.table_command(TableCommand::DeleteTable).unwrap();
    document.place_caret(caret.0, caret.1);
    assert!(document.caret_table_cell().is_none());
    assert_eq!(document.paragraph_count(), paragraphs);
    assert!(!document.export_docx().unwrap().is_empty());
}
