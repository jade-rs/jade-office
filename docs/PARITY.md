# Parity with a full Office suite

The goal set on 2026-10-21: jade office opens, draws, edits and saves Word,
Excel and PowerPoint files as a complete office suite does, measured
against a reference list of some 1,800 features an Office user reaches
for — Word's 506 first.

`docs/GAPS.md` lists what everyday users hit first; this file tracks the
distance to a whole suite, and how it is measured.

## Where jade stood on 2026-10-21

Each item of the yardstick was classified from jade's code:
**full** (drawn and editable as Office does), **render** (read and drawn,
no way to make or change it), **partial**, **roundtrip** (kept on save, not
drawn), **missing**.

| | items | full | render | partial | roundtrip | missing |
|---|---|---|---|---|---|---|
| Word | 506 | 70 | 165 | 123 | 40 | 108 |
| Excel | 933 | 576 | 43 | 79 | 94 | 141 |
| PowerPoint | 367 | 24 | 73 | 38 | 97 | 135 |

Excel's count is mostly worksheet functions (497 of 507 present). The shape
of the gap differs by application:

- **Word**: the engine reads and draws far more than the shells let anyone
  make or change — Japanese typesetting, sections, notes, fields, tracked
  changes. Most of the distance is editing surface: paragraph dialog, tabs
  and ruler, hyperlinks, notes, header/footer variants, styles, spelling.
- **Excel**: calculation is close to complete. What a received workbook
  shows is not: conditional formats by formula and icon sets, table styles,
  combo charts and data labels, shapes' own fills, sparklines, rich text in
  cells. No `.xls`/`.xlsb`.
- **PowerPoint**: the most distance. Decks draw, but shapes, tables, charts,
  backgrounds and themes cannot be made or changed beyond moving, resizing
  and typing; effects beyond an outer shadow, chart data labels, animation
  and transitions are not drawn or played.

## How it is measured

Compatibility is checked against Office itself, on this Mac (Word, Excel and
PowerPoint 16), not by eye:

1. **Drawing**: each corpus file is printed to PDF by the Office app and by
   jade (`crates/jade-office-model/examples/oracle_pdf.rs`,
   `docx_to_png --real-fonts`, `pptx_to_png --real-fonts`), and the pages are
   compared pixel by pixel; Word layout also against Word's own line traces
   (`jade-core/tests/word-oracle`).
2. **Saving**: each corpus file is saved by jade untouched and after an edit
   (`examples/resave.rs`, `examples/resave_docx.rs`), checked statically for
   what Office repairs (parts without a content type, broken relationships,
   rows and cells out of order, schema order in Word's properties), then
   opened in Office, which must not offer to repair it.

## Done in this push

- Files are drawn in their own fonts: the faces Office ships and the
  system's, found by any of their names (游明朝, ＭＳ Ｐゴシック, Calibri,
  Helvetica Neue…), where every file used to be drawn in Noto Sans JP.
  Italic is drawn (it was ignored natively), bold and italic are synthesised
  for families without them, and a Japanese character after a Latin family
  keeps the run's weight.
- Saving: a part added without a content type gets one, and a sheet left
  with rows out of order by another writer is put in order — both made Office
  repair the file.
