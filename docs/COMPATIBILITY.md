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
| Excel functions | a formula and what Excel computes, Japanese functions and HYPERLINK/ENCODEURL/DBCS/PHONETIC/BAHTTEXT included; 498 of Excel's 518 functions | 318 cases | jade-core `crates/jade-xlsx-headless-editor/tests/excel_conformance.rs` |
| Excel number formats | a value, a format code and what Excel displays (en-US and `[$-411]`) | 62 cases | jade-core `crates/jade-xlsx-layout/tests/excel_number_format_conformance.rs` |
| Word round trip | every fixture opened, edited, saved and reopened with every paragraph and part intact | 165 documents | jade-core `crates/jade-docx-editor/tests/round_trip_corpus.rs` |
| Excel round trip | the same for every cell's value, formula and style | 29 workbooks | jade-core `crates/jade-xlsx-headless-editor/tests/round_trip_corpus.rs` |
| PowerPoint round trip | the same for every shape's text and position, notes and media; tables, charts and SmartArt move and resize | 117 decks, 30 of them real decks from Apache POI's test data (PowerPoint, LibreOffice, published talks) | jade-core `crates/jade-pptx-writer/tests/round_trip_corpus.rs` |
| Slides like PowerPoint | in decks made in PowerPoint: a double-click in a table types into the cell under it (a merged cell where it starts), Enter into its first cell, Tab and ⇧Tab from cell to cell (a merge one stop) with the cell's text selected, so typing replaces it, and Tab in the last cell adds a row; rows and columns added beside a cell and removed, empty and formatted as their neighbour, the frame growing and shrinking by them, a change that would cut a merge refused; cells selected with Shift+click merge into one, their text as its paragraphs, and a merged cell splits back; a table resizes with its columns and rows scaled to the frame, a chart with its frame; SmartArt resizes with its drawing scaled to the frame; a corner drag resizes from the opposite corner and a body drag moves, each one undoable step, in both shells; slides added after the current one on its layout, duplicated after it, moved, deleted (never the last), put on another layout and hidden, each one undoable step, a hidden slide skipped by the slide show; text typed in place at the caret keeps the formatting of the run it goes into, a selection made with ⇧ is replaced by what is typed, a click puts the caret where it lands, Up goes to the line above, an IME composes at the caret, a run of typing is one step of Undo; a selection made bold, italic, larger, in another font and colour, centred, bulleted, numbered and spaced, the characters round it keeping theirs, and saved so; with nothing selected, what is typed next takes the formatting picked; a selected shape formatted as a whole; a text box drawn and typed into, shapes dropped at their usual size, pictures placed in the middle at their own size or shrunk to fit, a file that is not a picture refused; every deck opens and every slide paints | 29 + 1 tests, and 9 scenario tests in the native shell | `crates/jade-office-model/tests/decks_like_powerpoint.rs`, `tests/opens_every_fixture.rs`, `crates/jade-gpui-app/src/deck.rs`; jade-core `crates/jade-pptx-writer/tests/tables_and_charts.rs` |
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
| By hand, in the deployed browser build | a slide table: a double-click types into a cell, Tab selects the next cell's text, a row inserted below, Shift+click selects cells that merge and split back, undo restoring each; a corner drag resizing a table and a SmartArt diagram (its shapes scaled inside), undo keeping the selection; a sheet: BAHTTEXT and PHONETIC typed in, ⌘C/⌘V through the system clipboard pasting once | checked on 2026-10-19 and 2026-10-20 | https://jade-rs.github.io/jade-office/ |

## What works

| | Word | Excel | PowerPoint |
|---|---|---|---|
| Open and show | pages as laid out by the engine; headers, footers, tables, pictures, text boxes, comments, tracked changes | cells, number formats (incl. Japanese eras), merged cells, conditional formats, charts, filters, frozen panes | slides, shapes, pictures, charts, notes, slide show (hidden slides skipped) |
| Edit | caret, typing (IMEs included), Backspace/Delete/Enter, selection by drag, Shift-click, Shift+arrows, ⌘A; Home/End/arrows; copy, cut, paste (lines become paragraphs); undo/redo | select by click, drag (scrolling past the edge), Shift-click, row and column headers and the corner, ⌘A (the data, then the sheet), Shift/Ctrl+Space; ⌘-arrows to the ends of the data, ⌘Home/⌘End, Page Up/Down, Home; arrows over merged cells and past hidden rows; Enter and Tab round a range, Enter back to where Tab started; the name box shows the active cell and goes to a typed reference; type as in Excel (`10%`, `1,234`, `¥1,000`, `2024/1/15`, `'0123`), arrows leaving the cell, F2 to edit in place, ⌥Enter for a line break, arrows and clicks pointing at cells while a formula is typed, F4 for `$`, AutoSum (⌘⇧T); formulas over 498 functions, errors as Excel shows them (`#NAME?` for a misspelt function, in its dependents too); ⌘Enter, ⌘D, ⌘R and the fill handle (series, and double-click down the data); copy and paste with formulas moved and formats carried, tiled over a larger selection, cut moving the cells, Excel's clipboard text in and out; clear; ⌘B/⌘I/⌘U/⌘⇧X, Ctrl+Shift+1–6 number formats, ⌃; and ⌘; for the date and time; Merge & Center; sort A–Z/Z–A keeping a header; insert and delete rows and columns (⌘+/⌘-, menu), resize them by their header edges and fit by double-click; add, rename, move and delete sheets; hide and unhide rows and columns; drag a selection by its border to move it (⌥ copies); a Home-tab strip (font styles and size, alignment, wrap, merge, number formats, decimals, fill and font colours, borders, filters, conditional formatting); conditional formats as Excel's menu makes them — highlight rules (greater or less than, between, equal to, text that contains, duplicates) and top/bottom rules asking for their value, data bars and colour scales — cleared from the selection or the sheet and listed and deleted in a Rules Manager; filter dropdowns (sort, clear, tick values); data validation (list dropdowns, refused entries with the rule's message); hyperlinks followed on a click; a protected sheet refusing changes to its locked cells, formatting and rows and columns unless its protection allows them; notes written, edited, deleted and shown on hover; Find (⌘F); a right-click menu; undo/redo | add a slide after the one shown (⌘⇧N, or ⌘M in the browser), duplicate it (⌘D), delete it (Delete in the slide list), move it by dragging its thumbnail, put it on another layout, hide it (struck through and dimmed in the list, skipped by the slide show); select a shape (click or Tab) and type into it where its text lies — the caret put by a click, moved by the arrows (⌥ by words, ⌘ to the ends of the line and the text, Home/End), the selection made by ⇧ and a drag, a word by a double-click, ⌘A, Backspace and Delete, Enter for a paragraph and ⇧Enter for a line break, the runs keeping their font, size and colour, an IME composing at the caret, a run of typing one step of Undo — or into a table's cell; format the selection, or with the shape selected all its text, from a format strip (font, size and ⌘⇧>/⌘⇧<, bold ⌘B, italic ⌘I, underline ⌘U, colour, alignment ⌘L/⌘E/⌘R/⌘J, bullets, numbering, line spacing and space after), with nothing selected for what is typed next; insert a text box or one of six shapes by dragging it out on the slide (a click drops it at its usual size), and a PNG or JPEG picture, placed in the middle of the slide and shrunk to fit (Tab between cells; rows and columns added and deleted, and cells Shift+clicked across merged and split, from the toolbar), move it (drag or arrow keys), resize it by its corner handles from the opposite corner (tables, charts and SmartArt too), delete it; undo/redo |
| Native only | ribbon formatting, find and replace, tables, comments, change tracking, PDF export, print | | |
| Save | the file that was opened, in place and atomically; `.docm`/`.dotx`/`.xlsm`/`.pptm` stay what they are | the same; a formula jade cannot evaluate keeps Excel's value and asks Excel to recalculate on open | the same |

The browser build runs the same engine as the native app (on a pure-Rust
painter) and edits through the same cores.

## Short of a release

1. **Text typed into a table cell takes the cell's first run's
   formatting**; a SmartArt diagram resized keeps its text size, where
   PowerPoint may fit it to the new shapes.
2. **20 of Excel's 518 functions are missing**: CUBE\*, CALL, REGISTER.ID,
   RTD, WEBSERVICE, FILTERXML and IMAGE need a server, a network or a
   program outside the workbook; EUROCONVERT is an add-in's; GETPIVOTDATA,
   GROUPBY and PIVOTBY need pivot tables, which are not computed. A file's
   own formulas keep Excel's cached value; typed fresh, they show `#NAME?`.
   BAHTTEXT's เอ็ด after the millions (21000001 → ยี่สิบเอ็ดล้านเอ็ด) follows
   a reported Excel result, not one checked here.
3. **The browser clipboard** is checked with dispatched paste events, and
   by hand: ⌘C and ⌘V between cells, through the system clipboard, paste
   once. Where the page may not write to the clipboard (an embedding
   page without clipboard access), ⌘V pastes the workbook's own copy, as
   its menu's Paste does — but a clipboard still holding something older
   pastes that, as the browser hands it over.
4. **Japanese, Latin and Thai text only are drawn**: both shells lay out
   and paint documents, sheets and slides with the bundled Noto Sans JP
   and Noto Sans Thai, each character falling back to a face that has it,
   so Korean, Arabic, Hebrew, Devanagari and other scripts show as boxes.
   The values are right, and save and copy as they are. A Thai tone
   mark over an upper vowel (the ่ of ยี่) is drawn on the vowel rather
   than above it: the painters place glyphs without OpenType mark
   positioning.
5. **Formula entry is refused as Excel refuses it** — a trailing operator,
   an open string, an unmatched parenthesis, or a function given the wrong
   number of arguments — with the editor kept open; `TODAY(1)` is accepted,
   since the engine reads a time zone there.
6. **Freeze Panes cannot be set or removed.** Frozen panes in a file are
   shown and scroll as in Excel, but the engine has no writer for the
   sheet view yet.
7. **No Format Cells dialog (⌘1), and charts and pivot tables are not
   edited**; they open, draw and save as they were.

## Running the oracles

```sh
cargo test --manifest-path crates/jade-office-model/Cargo.toml --features skia,tiny-skia
cargo test --manifest-path crates/jade-web/Cargo.toml
cargo test --manifest-path crates/jade-gpui-app/Cargo.toml
cd jade-core && cargo test -p jade-xlsx-headless-editor -p jade-xlsx-layout -p jade-docx-editor -p jade-pptx-writer
```
