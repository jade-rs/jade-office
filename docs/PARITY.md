# Parity with a full Office suite

The current XLSX coverage audit, with implementation gaps and regression gates,
is in [xlsx/coverage.md](xlsx/coverage.md).
The current generated function inventory is 507 names. This measures
registration, not complete calculation or application compatibility; current results and limits are in
[xlsx/verification.md](xlsx/verification.md). The latest [pivot creation/deletion](xlsx/pivot-lifecycle.md) adds a common
creation dialog, atomic new-sheet/report/cache graphs and static chart snapshots
of currently displayed values, with independent schema and SDK checks;
full parity remains open. The
counts below describe the earlier baseline.

The goal set on 2026-10-21: jade office opens, draws, edits and saves Word,
Excel and PowerPoint files as a complete office suite does, measured
feature by feature against what Word, Excel and PowerPoint do — Word first.

`docs/GAPS.md` lists what everyday users hit first; this file tracks the
distance to a whole suite, and how it is measured.

## Current acceptance status (2026-11-07)

Word parity remains **open**. The historical counts below describe feature
inventory, not proof of equivalent behavior. A row being implemented does not
establish parity across the native and browser shells, saved files, rendering,
and Word-authored inputs. The uncovered rows in [word/coverage.md](word/coverage.md)
and the differences in [word/web-vs-desktop.md](word/web-vs-desktop.md) still need
verification and fixes.

The browser now warns before navigation with unsaved edits in any editor tab.
Its recovery store waits for transaction commit and retries a failed database
open. `scripts/test-web-storage.cjs` checks commit, abort, retry and warning
lifecycle; this is a storage regression gate, not an end-to-end Word parity gate.
Recovery snapshots now reopen as unsaved and remain stored until saved or
explicitly discarded. Native recovery and opening-clock regressions pass:
DATE/TIME fields receive the browser's local clock from the first layout in
the body, headers and footers while preserving saved results and edit history.
The browser shell also passes its WASM compile check. These checks cover the
specific behaviors above; they do not establish full Word parity.
Recent opened/saved files now survive a reload through a bounded IndexedDB
store (eight names). The storage regression additionally checks v1 migration,
concurrent writes, replacement, clock rollback and atomic eviction rollback.
These are browser-local copies; original-file paths and same-name identity
remain limitations described in the browser comparison.

Dirty browser tabs now ask before closing, with download, discard and cancel
choices. Download leaves the tab open; explicit discard removes its recovery
copy. Word format/protection failures retain the unsaved state, and Compare
uses a snapshot that does not acknowledge saving. The save regressions cover
failure state/undo preservation and successful RTF output reopening. Release-build browser interaction checks pass for Cancel, Escape, download,
discard, clean close and retaining the confirmation target while another tab
opens/closes. The downloaded DOCX contains the edits, and reopening recent files
after page reload restores the saved version rather than later discarded edits.
Recovery-store deletion and rejected cell edits still need browser coverage.

The macOS browser build now selects LLVM’s archiver when available and no
archiver override is set. Apple’s archiver produced an empty `ring` WASM archive,
causing undefined-symbol errors; rebuilding with LLVM produces a working release
bundle. README documents the toolchain requirement and clearing stale archives.

Browser appearance now offers System, Light and Dark in the View tab, with a
separate dark-pages preference. Both preferences persist in localStorage; the
title-bar toggle saves its choice too. Release-build browser checks confirm
dark chrome with white paper, light chrome, returning to System, and persistence
after reload. Existing document text and page thumbnails follow the paper
choice. The WASM compile and release build pass. The native
`dark_paper_turns_the_pages_over_and_back` regression passes, checking identical
DOCX bytes and restored light-paper pixels after switching back. Live OS appearance changes and
storage-denied browser sessions have not been exercised in the browser.

The browser screen-reader mirror now belongs to the active Word tab and caches
document entity identity. Both its text box and live region are removed when
no Word document is active. WASM compile/release builds and browser checks pass
for editing, tab switching, leaving Word for Excel, closing the last Word tab,
and reopening the same filename. The comparison actions now dispatch through
the originating window. Release-browser checks confirm the two-document layout,
active-tab/screen-reader ownership when clicking either pane, and clearing
comparison when a paired document closes with synchronous scrolling disabled.
Synchronous/independent scrolling and resetting the partner position also pass
browser interaction checks. Actual VoiceOver speech remains unverified.
Word now uses the editor/pane dimensions rather than the whole browser window.
Oversized vertical pages align from the left so their start remains reachable.
The rebuilt release passes browser checks for visible line starts at 100% and
page-width fitting within one comparison pane.

Browser print preview now paints actual sheet content using the same paper
composition as PDF printing. Snapshot pagination covers selection printing,
mixed section sizes and print-time TOC updates without editing the open
document. Seven print and ten story regressions pass, including byte-identical
PDFs made from preview sheets and matching open header/footer selection images.
Release-browser checks cover visible text, page navigation, two-up layout,
white print paper with dark editor pages, reopening the panel, and no-selection
errors. Physical printing and comparison with Word rendering remain open.

## Where jade stood on 2026-10-21

Each feature was classified from jade's code:
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

Word moved from 70 to 165 of its 506 items being full.

- **Fonts**: files are drawn in their own fonts — the faces Office ships and
  the system's, found by any of their names (游明朝, ＭＳ Ｐゴシック, Calibri,
  Helvetica Neue…) — where every file used to be drawn in Noto Sans JP.
  Italic is drawn, bold and italic are synthesised for families without
  them, and a Japanese character after a Latin family keeps the run's weight.
- **Saving**: a part added without a content type gets one, and a sheet left
  with rows out of order by another writer is put in order — both made
  Office repair the file.
- **Word editing**: Shift+Enter, hyperlinks (⌘K, ⌘-click), footnotes and
  endnotes, TOC update, word count, the Paragraph dialog, Format Painter;
  the Font dialog, underline styles, character spacing, effects, ruby,
  emphasis marks, enclosed and combined characters, 縦中横, 体裁 options,
  symbols, date and time fields, paste options, change case; the table
  tabs and Table Properties (draw, convert, sort, formulas, AutoFit, borders,
  floating tables); Page Setup per section (文字数と行数, margins, paper,
  columns, line numbers, borders, colour, cover pages) and headers and
  footers edited in place with first-page, odd/even and linked variants.
- **Excel drawing**: formula conditional formats and icon sets, chart labels,
  axes, trendlines, error bars, combos, radar and bubble, table styles,
  shapes, rich text, sparklines, tab colours.
- **PowerPoint drawing**: shadows, glow, reflection, soft edges, chart labels
  and combos, all built-in table styles, picture bullets, arched WordArt;
  autofit drawn at its recorded scale, as PowerPoint opens a deck.

## Batch 2

Word: fields (a Field dialog, update, toggle codes, lock and unlink, every
field type evaluated with Word's switches and 和暦), paragraphs and lists
(tabs dialog and ruler, borders and shading, drop caps, picture bullets,
numbering formats, multilevel and list styles), styles (character and
linked styles, new/modify/update, Styles pane, themes, style sets,
templates, Quick Parts), and references (notes options and conversion,
bookmarks, cross-references, captions, citations and bibliography, TOC
options, table of figures, index and table of authorities, the navigation
pane with collapsible headings, Go To). Rows full: 350 of 506, counting a
few dialogs that are native-only for now (listed in
`docs/word/web-vs-desktop.md`).

## Word reference documents

Per-area detail behind the Word numbers above, in `docs/word/`:

- [coverage.md](word/coverage.md) — the test that covers each Word feature,
  and the features no test covers.
- [enumerations.md](word/enumerations.md) — border, underline, highlight,
  shape, text-effect, emphasis and number-format values one by one.
- [compatibility-modes.md](word/compatibility-modes.md) — compatibility modes,
  legacy layout options and Word for Mac vs Windows, with jade's handling.
- [fields.md](word/fields.md) — every field type and switch: parsed,
  evaluated, insertable.
- [japanese-typesetting.md](word/japanese-typesetting.md) — each Japanese
  typesetting option against its element, JIS X 4051 rule, code and tests.
- [web-vs-desktop.md](word/web-vs-desktop.md) — what the browser build does
  differently from the native app, why, and what is left to port.
- [ribbon-map.md](word/ribbon-map.md) — Word's ribbon tabs, context tabs,
  dialogs and shortcuts (Mac and Windows) mapped to jade's two shells.

## Batch 4: every Word item

With files and printing, the Draw tab, the View tab and editing aids,
review and protection, and the fidelity fixes, all 506 Word features
are drawn and editable in the native app. Still to do, and
tracked: the browser shell's share of a few of them (views, Word Options,
AutoRecover, the picture/shape format tab — `docs/word/web-vs-desktop.md`),
and checks against files Word itself wrote for open passwords and digital
signatures.
