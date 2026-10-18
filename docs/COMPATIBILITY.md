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
| Excel functions | a formula and what Excel computes, Japanese functions included | 296 cases | jade-core `crates/jade-xlsx-headless-editor/tests/excel_conformance.rs` |
| Excel number formats | a value, a format code and what Excel displays (en-US and `[$-411]`) | 62 cases | jade-core `crates/jade-xlsx-layout/tests/excel_number_format_conformance.rs` |
| Word round trip | every fixture opened, edited, saved and reopened with every paragraph and part intact | 165 documents | jade-core `crates/jade-docx-editor/tests/round_trip_corpus.rs` |
| Excel round trip | the same for every cell's value, formula and style | 29 workbooks | jade-core `crates/jade-xlsx-headless-editor/tests/round_trip_corpus.rs` |
| PowerPoint round trip | the same for every shape's text and position, notes and media | 87 decks | jade-core `crates/jade-pptx-writer/tests/round_trip_corpus.rs` |
| Word layout | pages compared against Word's own rendering | per fixture | jade-core CI `layout-compare` |
| Typing like Excel | text typed into a cell and the value, format and display Excel makes of it; a misspelt function | 22 inputs + cases | `crates/jade-office-model/tests/typing_like_excel.rs` |
| Clipboard like Excel | Excel's clipboard text in and out, quoted multi-line cells, CR LF | 2 tests | `crates/jade-office-model/tests/clipboard_like_excel.rs` |
| Japanese functions | LENB, MIDB, FINDB, REPLACEB, ASC, JIS and the rest, typed into a cell | 11 cases | `crates/jade-office-model/tests/japanese_functions_like_excel.rs` |
| Files people have | every fixture opens; damaged, protected, macro-enabled and template files say what they are and save as what they are | 3 suites | `crates/jade-office-model/tests/`, `crates/jade-web/tests/` |
| Editing Word in the browser | caret, typing, IME, selection, arrows, clipboard, undo, save | 12 tests | `crates/jade-web/tests/edits_documents.rs` |

## What works

| | Word | Excel | PowerPoint |
|---|---|---|---|
| Open and show | pages as laid out by the engine; headers, footers, tables, pictures, text boxes, comments, tracked changes | cells, number formats (incl. Japanese eras), merged cells, conditional formats, charts, filters, frozen panes | slides, shapes, pictures, charts, notes, slide show |
| Edit | caret, typing (IMEs included), Backspace/Delete/Enter, selection by drag, Shift-click, Shift+arrows, ⌘A; Home/End/arrows; copy, cut, paste (lines become paragraphs); undo/redo | type as in Excel (`10%`, `1,234`, `¥1,000`, `2024/1/15`, `'0123`), formulas over 498 functions, errors as Excel shows them (`#NAME?` for a misspelt function, in its dependents too); copy, cut, paste in Excel's clipboard text; clear; undo/redo | select a shape (click or Tab), type into it, move it, delete it; undo/redo |
| Native only | ribbon formatting, find and replace, tables, comments, change tracking, PDF export, print | | |
| Save | the file that was opened, in place and atomically; `.docm`/`.dotx`/`.xlsm`/`.pptm` stay what they are | the same; a formula jade cannot evaluate keeps Excel's value and asks Excel to recalculate on open | the same |

The browser build runs the same engine as the native app (on a pure-Rust
painter) and edits through the same cores.

## Short of a release

1. **PowerPoint's evidence is narrow.** 87 decks round-trip, but 79 of them
   are colour-transform variants; a release needs real decks with tables,
   SmartArt, charts, placeholders and media.
2. **A caret at a wrapped line's start is drawn at the end of the line
   above.** Positions carry no affinity in the renderer; keyboard motions
   keep to the right line, but the caret is drawn on the other one (both
   shells).
3. **A formula Excel would refuse is accepted.** `=SUM(` is stored and the
   cell shows empty; Excel refuses it with a message.
4. **Cut clears at once.** Excel moves the cells when they are pasted.
5. **Functions the engine lacks** keep Excel's cached value in a saved file;
   typed fresh, they show `#NAME?` until Excel recalculates the file.
6. **The browser clipboard** is checked with dispatched paste events; the
   embedded test browser cannot read the system clipboard.

In flight: jade-core#35 (the round-trip corpus sizes above; on main the
corpora are 137 documents, 22 workbooks and 4 decks).

## Running the oracles

```sh
cargo test --manifest-path crates/jade-office-model/Cargo.toml --features skia,tiny-skia
cargo test --manifest-path crates/jade-web/Cargo.toml
cargo test --manifest-path crates/jade-gpui-app/Cargo.toml
cd jade-core && cargo test -p jade-xlsx-headless-editor -p jade-xlsx-layout -p jade-docx-editor -p jade-pptx-writer
```
