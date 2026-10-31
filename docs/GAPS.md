# Gap inventory

What an everyday Office user would reach for and does not find yet, taken
from the code on 2026-10-20 (`docs/COMPATIBILITY.md` lists what works). Each
row says how often a user hits it, and whether jade-core's engine already
does it, so only the shells lack a button or key (**engine: yes**), or the
engine needs work first (**engine: no**).

**Blocker** marks what is required before a release: every High row, and
the Format Cells dialog, conditional-format editing and chart editing. Table
cell merge/split and SmartArt resizing were required and are done. Every
PowerPoint blocker is done, marked **Done** with the pull request that did
it; copy and paste across slides needed the engine (jade-rs/jade-core#111).
Every Word blocker is done the same way; tracked changes typed a key at a
time needed the engine (jade-rs/jade-core#108).
Excel's printing, CSV, Paste Special, Find and Replace and the native Number
Format dropdown are done, marked the same way; the engine gave them a number
under Text shown as General and transposed formulas (jade-rs/jade-core#118,
#119). Thai tone marks
(#56), Korean (#57) and Arabic (#58) are tracked as issues, not blockers.

## Excel

| | Gap | Hit | Engine |
|---|---|---|---|
| **Done** (#71) | Format Cells dialog (⌘1): number, alignment, font, border, fill tabs | High | partly — `format_at`/`set_format` cover font, fill, number format, every alignment, wrap, indent, rotation, shrink-to-fit, borders and protection flags; not pattern or gradient fills, super/subscript |
| **Done** (#75) | Conditional formats: create, edit, delete rules | High | no writer; display also misses formula rules, top 10, above average, duplicates, dates, `cellIs` against a cell, icon sets |
| **Done** (#89) | Charts: select, move, resize, edit type, data, titles, axes, legend | High | reads and draws bar/line/area/pie/doughnut/scatter; no chart authoring or anchor move/resize |
| **Done** (#70) | Hidden sheets show as tabs | High (fidelity) | sheets carry visibility; the shells don't filter |
| **Done** (#72) | Supported cell and worksheet edits respect imported sheet protection | High (fidelity) | protection read/enforced; broader protection authoring and workbook differences are tracked below |
| **Done** (#95) | Print and export to PDF | High | no — jade-office-model paginates; Skia records the PDF |
| **Done** (#94) | CSV import and export; Save As writes xlsx whatever the name | High | no — jade-office-model reads and writes it |
| **Done** (#93) | Paste Special (values, formats, transpose) | High | yes (`transpose_formula`, jade-core#119) |
| **Done** (#92) | Find and Replace (only Find, no options) | High | yes (`find`) |
| **Done** (#91) | Number-format dropdown in the native app (the browser has one) | High | yes |
| Partial (2026-11-01) | Find All in both find bars (the matches listed with their sheet, cell, value and formula, the count in the status bar, a row clicked selects its cell); Look in: Values, and Find and Replace looking only within a selection of several cells, the active cell moving through it as in Excel; Look in: Notes remains | Medium | yes (`find`) |
| Partial (2026-11-01) | Paste Special operations (None, Add, Subtract, Multiply, Divide) in both shells' dialog, as Excel combines them; column widths, Paste Link and Paste Options after a paste remain | Medium | partly |
| Partial (2026-11-01) | CSV: `.tsv` opened and saved tab-separated, a CSV whose first line has semicolons (or tabs) and no commas split at those, UTF-16 *Unicode Text* read by its BOM besides UTF-8 and Shift_JIS; the Text Import Wizard (choosing delimiters and column formats, leading zeros kept as text) remains | Medium | — |
| Done in both shells (2026-10-31) | Format Painter: a brush on the sheet tool strip and 書式のコピー/貼り付け on the right-click menu, the next selection taking the formats; a double-click on the brush keeps painting until Escape | Medium | yes (`copy_format`) |
| Partial (2026-11-01) | Clear Formats (書式のクリア) and Clear All (すべてクリア) on the right-click menu in both shells, each one undo step; notes and row/column defaults are left by both | Medium | yes (`clear_formats`) |
| | Font family, vertical alignment, indent controls; custom/theme colours | Medium | yes for most |
| Partial (2026-11-01) | Sort dialog (並べ替え… on the right-click menu in both shells: the selection or the list round the active cell, a header row guessed as Excel does and shown by name, any number of levels each with its column and A to Z / Z to A, case sensitivity, one undo step); left-to-right sorting and sorting by color remain | Medium | yes (`SortRequest`) |
| Partial (2026-11-01) | Filter: ticked values, Top 10 and Bottom 10, and Reapply on the filter button in both shells; custom conditions, dates and a chosen N remain | Medium | yes (`AutoFilterChange`) |
| Partial (2026-10-23) | Data validation: invalid-data circles, validation-only paste, general MCE, advanced coercion and Office oracle coverage | Medium | standard/x14/quoted-list reader and preserving edits, source alternatives, formula bounds and alert choices implemented; see [validation](xlsx/validation.md) |
| Partial (2026-10-23) | Named ranges: scope-aware create/update/delete and calculation; name-box definition/navigation in both shells; Name Manager dialog remains | Medium | yes — see `docs/xlsx/ibis-parity.md` |
| Partial (2026-10-23) | Insert/delete cells shifting down/right/up/left in both context menus; x14 coordinate/ID reconciliation added; keyboard choice dialog and broader imported extension coverage remain | Medium | yes for standard paths — see [cell shifts](xlsx/cell-shifts.md) |
| Partial (2026-11-01) | Go To (F5, ⌃G) in both shells: the name box opened on the active cell for a cell, range or name to go to, one that does not exist said so and never defined; Go To Special (blanks, constants, formulas…) remains, needing selections of several areas | Medium | partly |
| Partial (2026-10-23) | Hyperlinks: insert/edit/remove, display text and ScreenTips in both shells (⌘K); relative file opening/base-address and Office oracle coverage remain | Medium | yes — see [hyperlinks](xlsx/hyperlinks.md) |
| Partial (2026-11-01) | Function AutoComplete in both shells (the functions the word before the caret starts listed under the cell, the arrows to pick, Tab or a click to put one in with its parenthesis); Insert Function and argument tips remain | Medium | yes (`supported_function_names`) |
| Partial (2026-10-24) | Formula compatibility: ECMA.CEILING, SINGLE, ANCHORARRAY and saved-result GETPIVOTDATA call paths added; five function-name differences remain against pinned ibisOffice, with broader semantic/oracle coverage still open | Medium | see [compatibility calls](xlsx/compatibility-functions.md) and the [full audit](xlsx/ibis-parity.md) |
| Partial (2026-10-24) | Workbook structure guards plus protect/unprotect, SHA/legacy password authentication, preserving save/history and shared native/browser dialog added; worksheet protect/unprotect, cell lock/hidden contents, permission and selection controls added; standard/x14 editable-range CRUD, authentication, structural reconciliation and session unlocks added; independent worksheet object guards and reader/cached permission queries added; individual imported DrawingML/VML object/text locks and preserving updates added; automatic typed-cell and bulk paste/clear/fill/move range password prompts added with deferred credentials and atomic grant/command commit; navigation replay, flag authoring/selection feedback, general drawing ownership/MCE, account ACLs, scenario editing, encryption and broader application evidence remain | High (fidelity) | see [workbook protection](xlsx/workbook-protection.md) and [authoring](xlsx/protection-authoring.md) and [sheet protection](xlsx/sheet-protection-authoring.md) and [editable ranges](xlsx/editable-ranges.md) and [object protection](xlsx/object-protection.md) and [individual locks](xlsx/individual-object-protection.md) and [range prompts](xlsx/range-unlock-prompts.md) |
| Partial (2026-10-24) | Compound clipboard/fill: failed tiled paste and active-entry fill restore source/model/history/redo/clipboard/session/layout state; cut sources guarded; sparse full-axis copying retained; bulk paste/clear/fill/move authentication now uses actual write coverage and atomic grants/command commit; general compound operations remain | High (fidelity) | see [compound edit atomicity](xlsx/bulk-edit-atomicity.md) and [bulk authentication](xlsx/bulk-range-authentication.md) |
| Partial (2026-10-24) | Cell/shared-string XML: common namespace/hierarchy and Choice/Fallback selection for values, rich runs and phonetic keys; owned edits/caches and compatible existing alternatives added; general MCE, alternate topology and application corpus evidence remain | High (fidelity) | see [cell content](xlsx/cell-content.md) |
| Partial (2026-10-24) | Pivots: typed catalog, GETPIVOTDATA, structural guards, atomic selected/all refresh, dependency scheduling, standard relationship ownership and named styles added. Refresh now fits report/filter column widths with backend metrics, shared-column maxima, opt-out/protection and one-step rollback/history. Local classic PivotChart synchronization and worksheet/source name identity are now covered; see [PivotCharts](xlsx/pivot-charts.md). A common native/browser editor now changes existing report source/layout/measures/members/pages/field options/styles atomically, with actual saved-value/pixel/history and scoped official-XSD/SDK checks; see [authoring](xlsx/pivot-authoring.md). Both shells now create/delete supported reports, including new-sheet transactions, shared cache ownership and static snapshots from displayed chart data; see [creation/deletion](xlsx/pivot-lifecycle.md). Empty/no-value design, reused-cache creation, arbitrary source field remapping, advanced/calculated/show-as/grouping authoring, complete legacy fitting/page/group/styles/charts/controls/external identity, load refresh, OLAP, slicers/timelines and full Office oracles remain | Medium | see [refresh fitting](xlsx/pivot-autofit.md), [styles/ownership](xlsx/pivot-styles.md), [dependencies](xlsx/pivot-refresh-order.md) and [full audit](xlsx/ibis-parity.md) |
| Done in jade-core (2026-10-31) | Fill continuing month and weekday names (Jan…, 月曜…) | Medium | yes — Excel's built-in lists (days, months, 1月…12月, quarters, 睦月…, 十二支, 十干) go round, in the source's capitals and step |
| Partial (2026-11-01) | Hide/unhide sheets in both shells; Tab Color (Excel's standard colours and No Color) on the sheet tab's menu in both shells; sheet copy remains | Medium | partly |
| Partial (2026-10-31) | Row/column grouping (outline): Group, Ungroup, Hide and Show Detail on selected rows or columns from the right-click menu in both shells, and the outline gutter — a button per level, a bracket per group, its box `−`/`+` on the summary — drawn and clicked; read and written as Excel does; Auto Outline and Clear Outline, and Data ▸ Subtotal from the right-click menu through its dialog (the column whose runs are totalled, eleven functions, the columns totalled, replacing current subtotals, a page break between groups) | Medium | yes |
| Partial (2026-10-31) | Remove duplicates (重複の削除 on the right-click menu through its dialog: the selection or the list round the active cell, the columns compared ticked by name with Select All and Unselect All, text without regard to case, a header row guessed as Excel does and toggled, Excel's count in the status bar) and text to columns (区切り位置: tab, semicolon, comma and space delimiters, consecutive ones as one, quotes, a preview, each piece taken as typed; fixed width with the breaks guessed as Excel does and set on a ruler over the sample); column data formats and flash fill remain | Medium | partly |
| | Trace precedents, error checking, show formulas | Medium | no |
| Partial (2026-10-23) | Excel tables: create/resize/style/totals/calculated columns/entry expansion/conversion added; 990 checks in the six selected implementation crates and both shell builds passed; explicit ownership, preserving namespace-aware table shifts and header insertion added; dialog runtime, broader extension cases and custom styles remain | Low–Medium | see [table authoring](xlsx/tables.md) and [verification](xlsx/verification.md) |
| Partial (2026-11-01) | Page setup: margins, orientation, paper, scaling, print area/titles can be written through engine/model; a Page Setup dialog on the right-click menu in both shells (orientation, six papers, a scale or fitting to pages, the Normal/Wide/Narrow margins); custom margins, centring and break controls remain | Medium | yes — see `docs/xlsx/ibis-parity.md` |
| | Insert pictures and shapes | Low–Medium | no |
| Done in both shells (2026-10-31) | Calculation mode and F9: 計算方法: 手動 / 自動 on the right-click menu (saved with the workbook, automatic recalculating at once) and 再計算実行 / F9 | Low | yes |
| | Threaded comments, multi-sheet selection, Merge Across, split view, cell styles gallery | Low | no |

## Word

| | Gap | Hit | Engine |
|---|---|---|---|
| **Done** (#78) | New blank document, new from template | High | no |
| **Done** (#78) | Save as / export PDF (native prints only; the browser has neither) | High | yes (`export_pdf`) |
| **Done** (#82) | Copy and paste keeping formatting (plain text today, even within a document) | High | partly |
| **Done** (#82) | Insert picture (and moving it in the browser; wrap options) | High | yes (`insert_image`) |
| **Done** (#79) | Page setup: margins, orientation, size, columns, section breaks | High | yes |
| **Done** (#79) | Headers and footers: edit; page numbers | High | yes (`set_header_footer_text`) |
| **Done** (#81) | Comments: type the text, reply, resolve, delete, pane; none in the browser | High | yes |
| **Done** (#81) | Tracked changes: accept or reject one, next and previous; none in the browser | High | partly |
| **Done** (#76) | Find and Replace in the browser; options (case, whole word) in both | High | yes |
| **Done** (#74) | Native keys: Home/End, word and line jumps, word delete, Tab (the browser has them) | High | yes |
| | Headers and footers: first-page and odd/even variants, a story of more than one line, a header per section | Medium | partly (`SetHeaderFooterVariants`) |
| | Page setup per section (the commands act on the default one) | Medium | partly |
| | Paste with formatting from other apps (HTML, RTF); a copy keeps its formatting within the app only | Medium | no |
| | PDF in the browser is a picture of each page: its text cannot be selected or searched | Medium | partly (`jade-docx-pdf-renderer`) |
| **Done** (#108) | Shift+Enter line break | Medium | partly |
| **Done** (#108) | Format Painter: one click paints once, a double-click until Escape; ⌘⇧C / ⌘⇧V | Medium | yes |
| **Done** (#108) | Space before/after, paragraph dialog (first-line/hanging indent, keep with next) | Medium | yes (`set_paragraph_spacing`) |
| | Fonts beyond a fixed list; custom colours; Font dialog | Medium | partly |
| | Modify and create styles; character styles | Medium | partly |
| | List types, bullet glyphs, number formats, restart numbering | Medium | partly |
| **Done** (#108) | Hyperlinks: insert, edit (⌘K), follow (⌘-click) | Medium | yes |
| | Page Break (native runs Blank Page), ⌘Enter | Medium | yes |
| **Done** (#108) | Footnotes and endnotes: insert (⌥⌘F, ⌥⌘D) | Medium | yes |
| **Done** (#108) | Table of contents: update; none in the browser | Medium | yes |
| | Table column resizing by drag, AutoFit, table properties | Medium | yes |
| | Word count (done on word-editing-gaps: status bar, Japanese counted as Word counts it); spelling | Medium | no |
| | Save failures only printed to the console (native) | Medium | — |
| | Shapes, text boxes, symbols, equations, fields, bookmarks | Low–Medium | yes for most |
| | Japanese word selection on double-click; drag-and-drop text | Low | no |
| | Navigation pane, outline/draft views | Low | no |

## PowerPoint

| | Gap | Hit | Engine |
|---|---|---|---|
| **Done** (#73) | Slides: new, duplicate, delete, reorder | High | yes |
| **Done** (#73) | Layouts; hide a slide (hidden slides still play) | High | yes |
| **Done** (#77) | Text: caret movement, selection, typing in place (native types at the end only) | High | yes (`apply_text_input`) |
| **Done** (#80) | Character formatting: font, size, bold, italic, colour | High | yes (`format_text_range`) |
| **Done** (#80) | Paragraph formatting: alignment, bullets, numbering, spacing | High | yes (`format_paragraphs`) |
| **Done** (#84) | Insert text box, shape, picture | High | yes |
| **Done** (#86) | Copy, paste, duplicate shapes; select several | High | yes (`copy_shapes`, jade-core#111) |
| **Done** (#85) | Speaker notes: edit | High | yes (`set_slide_notes`) |
| | Paste into text in the native app; copy and cut slide text | Medium | — |
| | Cut shapes; paste pictures and shapes from other apps | Medium | partly |
| | Table cells typed in a box over the cell, not in place | Medium | partly |
| | Notes pane: caret put by a click, selection, formatting | Low | yes |
| | Fill and outline; rotate and flip; z-order; group | Medium | yes |
| | Ungroup, align and distribute, effects | Low–Medium | no |
| | Table styles, borders, shading | Medium | no |
| | Chart data and type | Medium | no |
| | Slide master, themes, backgrounds | Medium | no |
| | Transitions and animations (not read, played or edited) | Medium | no |
| | Slide show: presenter view, notes, timer, jump to slide | Medium | — |
| | Comments | Medium | no |
| | Find and Replace for slides | Medium | partly |
| | Export PDF and images; print in the browser | Medium | partly (`Deck::pdf`) |
| | Slide size, sections | Low | no |
| | Outline and slide sorter views | Low | — |
