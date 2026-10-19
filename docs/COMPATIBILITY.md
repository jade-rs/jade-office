# Office compatibility

What jade office does as Word, Excel and PowerPoint do, how each claim is
checked, and what is still short of a release. Every claim here has a test
behind it; a gap without one is listed as a gap.

Engine: jade-core at the pin in this repository (`git ls-tree HEAD jade-core`).

## How it is checked

Each oracle is a list of cases written down as the Office application behaves.
A fix starts as a case that fails.

| Oracle | What a case is | Size | Where |
|---|---|---|---|
| Excel functions | a formula and what Excel computes, Japanese functions and HYPERLINK/ENCODEURL/DBCS/PHONETIC included; 497 of Excel's 518 functions | 304 cases | jade-core `crates/jade-xlsx-headless-editor/tests/excel_conformance.rs` |
| Excel number formats | a value, a format code and what Excel displays (en-US and `[$-411]`) | 62 cases | jade-core `crates/jade-xlsx-layout/tests/excel_number_format_conformance.rs` |
| Word round trip | every fixture opened, edited, saved and reopened with every paragraph and part intact | 165 documents | jade-core `crates/jade-docx-editor/tests/round_trip_corpus.rs` |
| Excel round trip | the same for every cell's value, formula and style | 29 workbooks | jade-core `crates/jade-xlsx-headless-editor/tests/round_trip_corpus.rs` |
| PowerPoint round trip | the same for every shape's text and position, notes and media; tables and charts move and resize, SmartArt moves | 117 decks, 30 of them real decks from Apache POI's test data (PowerPoint, LibreOffice, published talks) | jade-core `crates/jade-pptx-writer/tests/round_trip_corpus.rs` |
| Slides like PowerPoint | in decks made in PowerPoint: a double-click in a table types into the cell under it (a merged cell where it starts), Enter into its first cell; a table resizes with its columns and rows scaled to the frame, a chart with its frame; SmartArt moves and keeps its size; every deck opens and every slide paints | 6 + 1 tests, and a scenario test in the native shell | `crates/jade-office-model/tests/decks_like_powerpoint.rs`, `tests/opens_every_fixture.rs`, `crates/jade-gpui-app/src/deck.rs`; jade-core `crates/jade-pptx-writer/tests/tables_and_charts.rs` |
| Rich slide objects | a table, a chart, SmartArt and a video keep every part through an edit and save, and draw the same | 2 tests | jade-core `crates/jade-pptx-writer/tests/rich_objects_round_trip.rs` |
| Dynamic arrays | a spilling formula fills its cells, saves as Excel saves one (`t="array" ref`, `cm`, `xl/metadata.xml`), grows and shrinks with its inputs, reads `#SPILL!` when blocked; a file's spills survive recalculation | 6 tests | jade-core `crates/jade-xlsx-calc/tests/recalculates.rs`, `crates/jade-xlsx-writer/tests/writes_spills.rs`, `crates/jade-xlsx-headless-editor/tests/dynamic_arrays.rs`, `crates/jade-office-model/tests/dynamic_arrays_like_excel.rs` |
| Excel tables | structured references (`Table1[Amount]`, `[#Totals]`) recalculate when the table is edited | 2 tests | jade-core `crates/jade-xlsx-calc/tests/recalculates.rs`, `crates/jade-office-model/tests/tables_like_excel.rs` |
| Word layout | pages compared against Word's own rendering | per fixture | jade-core CI `layout-compare` |
| Typing like Excel | text typed into a cell and the value, format and display Excel makes of it; open parentheses closed; a misspelt function | 25 inputs + cases | `crates/jade-office-model/tests/typing_like_excel.rs` |
| Clipboard like Excel | Excel's clipboard text in and out, quoted multi-line cells, CR LF; a copied formula pastes adjusted, a cut moves the cells on paste and references follow | 5 tests | `crates/jade-office-model/tests/clipboard_like_excel.rs` |
| Working a sheet like Excel | selection (cells, headers, Shift, merges, hidden rows), ⌘-arrows, Enter/Tab round a range, ⌘D/⌘R/⌘Enter, the fill handle and its double-click, copies that move formulas, tiled pastes, cuts and drags that move, the Home tab's formats and decimals, Merge & Center, sorts with a header, filters, data validation, hyperlinks, notes, rows and columns inserted, deleted, hidden and resized, sheets added, renamed, moved and deleted, AutoSum, Find, F4 | 25 + 5 tests, and scenario tests in both shells | `crates/jade-office-model/src/sheet.rs`, `src/grid.rs` |
| Japanese functions | LENB, MIDB, FINDB, REPLACEB, ASC, JIS and the rest, typed into a cell; PHONETIC reading the furigana Excel saved (ヤマダタロウ for 山田太郎), and text typed over a name reading as that text | 11 cases + 1 test | `crates/jade-office-model/tests/japanese_functions_like_excel.rs`; jade-core `crates/jade-xlsx-calc/tests/phonetic.rs`, `crates/jade-xlsx-headless-editor/tests/phonetic_readings.rs` |
| Files people have | every fixture opens; damaged, protected, macro-enabled and template files say what they are and save as what they are | 3 suites | `crates/jade-office-model/tests/`, `crates/jade-web/tests/` |
| Editing Word in the browser | caret (on the line clicked, at a wrap too), typing, IME, selection, arrows, Home/End, clipboard, undo, save | 13 tests | `crates/jade-web/tests/edits_documents.rs` |

## What works

| | Word | Excel | PowerPoint |
|---|---|---|---|
| Open and show | pages as laid out by the engine; headers, footers, tables, pictures, text boxes, comments, tracked changes | cells, number formats (incl. Japanese eras), merged cells, conditional formats, charts, filters, frozen panes | slides, shapes, pictures, charts, notes, slide show |
| Edit | caret, typing (IMEs included), Backspace/Delete/Enter, selection by drag, Shift-click, Shift+arrows, ⌘A; Home/End/arrows; copy, cut, paste (lines become paragraphs); undo/redo | select by click, drag (scrolling past the edge), Shift-click, row and column headers and the corner, ⌘A (the data, then the sheet), Shift/Ctrl+Space; ⌘-arrows to the ends of the data, ⌘Home/⌘End, Page Up/Down, Home; arrows over merged cells and past hidden rows; Enter and Tab round a range, Enter back to where Tab started; the name box shows the active cell and goes to a typed reference; type as in Excel (`10%`, `1,234`, `¥1,000`, `2024/1/15`, `'0123`), arrows leaving the cell, F2 to edit in place, ⌥Enter for a line break, arrows and clicks pointing at cells while a formula is typed, F4 for `$`, AutoSum (⌘⇧T); formulas over 497 functions, errors as Excel shows them (`#NAME?` for a misspelt function, in its dependents too); ⌘Enter, ⌘D, ⌘R and the fill handle (series, and double-click down the data); copy and paste with formulas moved and formats carried, tiled over a larger selection, cut moving the cells, Excel's clipboard text in and out; clear; ⌘B/⌘I/⌘U/⌘⇧X, Ctrl+Shift+1–6 number formats, ⌃; and ⌘; for the date and time; Merge & Center; sort A–Z/Z–A keeping a header; insert and delete rows and columns (⌘+/⌘-, menu), resize them by their header edges and fit by double-click; add, rename, move and delete sheets; hide and unhide rows and columns; drag a selection by its border to move it (⌥ copies); a Home-tab strip (font styles and size, alignment, wrap, merge, number formats, decimals, fill and font colours, borders, filters); filter dropdowns (sort, clear, tick values); data validation (list dropdowns, refused entries with the rule's message); hyperlinks followed on a click; notes written, edited, deleted and shown on hover; Find (⌘F); a right-click menu; undo/redo | select a shape (click or Tab), type into it or into a table's cell, move it, resize it by its corners (native app; tables and charts too), delete it; undo/redo |
| Native only | ribbon formatting, find and replace, tables, comments, change tracking, PDF export, print | | |
| Save | the file that was opened, in place and atomically; `.docm`/`.dotx`/`.xlsm`/`.pptm` stay what they are | the same; a formula jade cannot evaluate keeps Excel's value and asks Excel to recalculate on open | the same |

The browser build runs the same engine as the native app (on a pure-Rust
painter) and edits through the same cores.

## Short of a release

1. **SmartArt moves but does not resize**, since its saved drawing would
   not follow; **the browser build resizes no shape yet** (the native app
   does, tables and charts included). A table's rows and columns cannot be
   added or removed, and text typed into a cell takes the cell's first
   run's formatting.
2. **21 of Excel's 518 functions are missing**, nearly all ones no offline
   engine can compute: CUBE\*, CALL, REGISTER.ID, RTD, WEBSERVICE,
   FILTERXML, IMAGE, EUROCONVERT, GETPIVOTDATA, GROUPBY, PIVOTBY; and
   BAHTTEXT. A file's own formulas keep Excel's cached value; typed fresh,
   they show `#NAME?`.
3. **The browser clipboard** is checked with dispatched paste events; the
   embedded test browser cannot read the system clipboard.
4. **Formula entry is refused as Excel refuses it** — a trailing operator,
   an open string, an unmatched parenthesis, or a function given the wrong
   number of arguments — with the editor kept open; `TODAY(1)` is accepted,
   since the engine reads a time zone there.
5. **Freeze Panes cannot be set or removed.** Frozen panes in a file are
   shown and scroll as in Excel, but the engine has no writer for the
   sheet view yet.
6. **No Format Cells dialog (⌘1), conditional-format editing, charts or
   pivot tables are edited**; they open, draw and save as they were.

## Running the oracles

```sh
cargo test --manifest-path crates/jade-office-model/Cargo.toml --features skia,tiny-skia
cargo test --manifest-path crates/jade-web/Cargo.toml
cargo test --manifest-path crates/jade-gpui-app/Cargo.toml
cd jade-core && cargo test -p jade-xlsx-headless-editor -p jade-xlsx-layout -p jade-docx-editor -p jade-pptx-writer
```
