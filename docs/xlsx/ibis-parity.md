# XLSX parity with ibisOffice

Audit started 2026-10-23. Reference: [ibisOffice
6e004f71ed43ea73c1a6cb0f0496483c107a0485](https://github.com/ibisjp/ibis-office/tree/6e004f71ed43ea73c1a6cb0f0496483c107a0485).
Jade baseline: jade-office `e97451d`, jade-core `e3a5929`.

This is an implementation comparison, not an Excel certification. The
reference's feature catalogue has 933 XLSX entries, including individual
functions and audit tasks. Its `Completed` labels do not establish that every
operation renders, calculates and round-trips correctly. Source inspection,
Jade regression tests and Office oracle captures are separate evidence.
No ibis implementation code is incorporated into Jade.

## Findings and order of work

Paths under `ibis-office-app/src/xlsx` and `ibis-pptx/src` below are relative
to the reference's `crates/`. Jade engine paths are under `jade-core/crates`.
“Preserved” means the original package part is retained, not that Jade can
interpret or edit its contents. “Partial” explicitly does not claim parity.

| Area | Reference implementation | Jade at the baseline | Current gap / acceptance requirement |
|---|---|---|---|
| Package read, cells, shared formulas, 1900/1904 | `ibis-pptx/src/xlsx.rs`, `save/xlsx.rs` | Reader/writer, shared formula expansion and date epoch tests | Added common namespace/hierarchy-aware standard cell/shared-string/rich/phonetic reading, Choice/Fallback selection, owned cell/cache writes and existing compatible-representation updates; see [cell content](cell-content.md). General MCE, alternate grid/shared-formula topology, other feature families, full schema validation and production/application corpus evidence remain |
| Formula registration | `ibis-calc/src/functions` | 503 registered names | Added ECMA.CEILING and complete call/prefix/registry paths for SINGLE/ANCHORARRAY: 507 versus the reference’s 512, including saved-result GETPIVOTDATA. Five names remain; generated inventory/difference and semantic limits are in [compatibility-functions.md](compatibility-functions.md) |
| Formula semantics, arrays, names | `ibis-calc/src/eval`, `formula`, `recalc.rs` | IronCalc adapter, spills, scoped range names | Added named constants/expressions and cycle termination; relative names, named LAMBDAs and array-producing names need broader oracle coverage |
| External links and service functions | `xlsx/external_links.rs`, `ibis-pptx/src/xlsx_external.rs` | Original parts and unsupported formula caches preserved | External workbook manager, refresh, linked data types, service/provider integration missing |
| Typed input, IME, furigana, cell rich text | `xlsx/cell_edit.rs`, `phonetic.rs` | Typed input/IME/PHONETIC; rich text read and drawn | Rich-text run authoring and phonetic editing missing |
| Clipboard and Paste Special | `xlsx/clipboard.rs`, `paste_special_dialog.rs` | Values/formulas/formats/transpose/skip blanks | Added returned-error rollback for tiled text/internal-copy paste and active-entry range fill, complete edit checkpoints, guarded cut sources and sparse full-axis permissions; see [compound edit atomicity](bulk-edit-atomicity.md). Arithmetic operations, column widths, validation-only paste, links, external rich clipboard remain. Added shared native/browser bulk editable-range authentication for paste, clear, all fill variants and move/copy, with deferred credentials, exact write coverage and atomic command/grant commit; see [bulk authentication](bulk-range-authentication.md) |
| Clear, Find, Go To | `xlsx/find.rs`, `goto_special.rs` | Clear contents, Find/Replace | Clear formats/all, result list, values/notes/selection scopes, Go To Special remain |
| Fill and input assistance | `xlsx/fill.rs`, `series.rs`, `flash_fill.rs`, `autocomplete.rs` | Numeric series, copy/fill handle | Date/text series, flash fill, formula suggestions/argument help remain |
| Names and scope | `xlsx/names.rs`, `ibis-pptx/src/save/xlsx.rs` | Read-only definitions | Added scoped create/update/delete/recalculation and name-box create/navigation; Name Manager dialog remains |
| Cell formatting and styles | `xlsx/custom_format.rs`, `cell_styles.rs`, `format_more.rs` | Number/font/fill/alignment/wrap/borders editor | Rotation, vertical text, shrink-to-fit, indent, pattern/gradient fills, named styles/theme authoring remain |
| Conditional formatting | `xlsx/conditional*.rs`, `ibis-pptx/src/save/xlsx_sizes.rs` | Wide read/render support; basic rule creation | The package rewrite now follows owned standard/x14 conditional formulas and formula thresholds. Full rule editing, precedence, stop-if-true, custom icons, advanced data-bar authoring and whole-axis anchor/namespace cases remain |
| Sheet tabs | `xlsx/sheet_ui.rs`, `save/xlsx_structure.rs` | Add/delete/rename/move; visibility/tab colour read | Added hide/unhide in both shells and tab-colour writer/model; sheet copy and tab-colour controls remain |
| Rows/columns, cell shifts, outlines | `xlsx/cells_dialog.rs`, `headers.rs`, `subtotal_dialog.rs` | Whole-axis insertion/deletion, hiding/sizing/fit | Added cell-range insertion/deletion, structural table-header insertion, shared-model APIs and both context menus; 1,106 tests across the six selected implementation crates passed. x14 rule/validation/sparkline coordinate and identity reconciliation is implemented; keyboard choice dialog, other extension families, grouping and subtotal authoring remain; see [cell-shifts.md](cell-shifts.md) |
| Sheet/window views | `xlsx/split_view.rs`, `custom_views.rs`, `arrange_windows.rs` | Saved freeze panes/gridlines; application zoom | Persisted freeze/view changes, split views, custom views and workbook windows remain |
| Tables | `xlsx/table_edit.rs`, `table_design.rs`, `save/xlsx_structure.rs` | Read/render, structured-reference calculation, header rename | Added create/resize/style/header/totals/calculated-column/entry expansion/conversion writers, model and dialogs. The regression across the six selected implementation crates (1,106 tests) and both shell builds passed. Explicit ownership, duplicate targets and table namespace/template hierarchy are handled. Added table-width/height band shifts, structural header insertion, calculated-row fill and saved-template rebasing. Custom-style authoring, dedicated table commands, clipboard/filter/spill/extension edge cases, dialog runtime and Office oracles remain. See [tables.md](tables.md) |
| Sort/filter | `xlsx/sort_edit.rs`, `filter.rs`, `advanced_filter.rs` | Engine multi-key/custom/date/rank filters; basic UI | Advanced/custom sort/filter UI, colour/icon criteria, custom lists and advanced filters remain |
| Validation | `xlsx/validation.rs`, `save/xlsx_sizes.rs` | Read; limited UI enforcement | Added preserving writer, all standard rule types, relative/formula bounds, names/dynamic lists, input/error messages, alert choices, native/browser editing and history. x14 reading, indexed preserving edits, shared input enforcement, quoted-list/source-alternative read/write and dropdown literal handling added. Whole-axis edits now retain x14 rules and rebase deleted anchors while isolating opaque XML. The package rewrite used by partial shifts and table changes now scopes formula ownership and x14 table context; invalid-data circles, validation-only paste, general MCE, advanced coercion and Office oracle coverage remain; see [validation.md](validation.md) |
| Hyperlinks and notes | `xlsx/links.rs`, `notes.rs` | Link following, note authoring | Added URL/file/internal/named-link authoring, display text/ScreenTips, range removal, native/browser dialogs and history. Relative file opening/base-address handling and Office round-trip/oracle cases remain; see [hyperlinks.md](hyperlinks.md) |
| Charts | `xlsx/chart_edit.rs`, `chart_insert.rs`, `chart_template.rs` | Basic authoring; classic/combo/radar/bubble rendering | Extended/3-D/stock/surface/statistical charts, full axes/series/labels/effects controls and chart sheets/templates remain |
| Sparklines | `xlsx/sparkline_dialog.rs`, `save/xlsx_sizes.rs` | Read/render | Added namespace/hierarchy-aware reading, CDATA/entity leaves and valid single-cell destinations, with cross-sheet edits/save/history cases; see [sparkline reading](sparkline-reading.md). Create/edit/group/style/axis controls, authoring writer, date-axis rendering, general MCE and Office oracles remain |
| Shapes, pictures and SmartArt | `xlsx/shape_edit.rs`, `ibis-pptx/src/save/xlsx_shapes.rs` | Drawing parts retained; basic draw/move/resize/delete/text | Insertion, geometry/effects/crop, grouping, SmartArt authoring, in-cell/linked images remain |
| Ink and form controls | `xlsx/ink.rs`, `control*.rs` | Parts preserved | Read/render/edit support remains |
| Pivot tables/charts | `xlsx/pivot*.rs`, `ibis-pptx/src/xlsx_pivot.rs`, `save/xlsx_pivot.rs` | Parts preserved | Added typed catalog, GETPIVOTDATA, structural movement, selected/all refresh and observed dependency scheduling. Standard relationship ownership reads 22 normative reports; named custom/84 normative styles reach both shells and raster/history tests. Refresh now saves report/filter column fitting using backend metrics, shared-column maxima and opt-out/protected widths in one atomic edit; see [autofit](pivot-autofit.md). Added locally corroborated classic PivotChart refresh, stable point/series styles, reference preservation and report-tab rename; see [PivotCharts](pivot-charts.md). Added preserving existing-pivot source/layout/measure/member/page/style editing and a common native/browser editor; see [authoring](pivot-authoring.md). Source headers/order/count must remain unchanged. Added atomic creation/deletion, new/existing destinations, a common creation panel and displayed-value static chart snapshots; see [creation/deletion](pivot-lifecycle.md). Blank/no-value design, reused-cache creation, source field remapping, advanced/calculated/grouped/show-as authoring, complete legacy fitting/page/group/styles/charts/controls/external identity, load refresh, OLAP and broad Office oracles remain |
| Slicers and timelines | `xlsx/slicer*.rs`, `timeline*.rs`, `save/xlsx_slicer.rs` | Parts preserved | Model/read/render, filtering, connections and authoring remain |
| Queries and XML maps | `xlsx/query*.rs`, `xml_maps.rs` | Parts preserved | Query operations, source connections/refresh, model authoring and XML maps remain |
| What-if and analysis | `xlsx/goal_seek.rs`, `solver_panel.rs`, `scenario_manager.rs`, `data_table.rs`, `analysis_panel.rs`, `forecast_sheet.rs` | Formula calculation only | Goal Seek, Solver, scenarios, data tables, analysis/forecast tools remain |
| Protection and encryption | `xlsx/protection.rs`, `password.rs` | Sheet protection read and enforced on supported edits | Added imported workbook structure protection in reader/writer/model and both shell command guards, with atomic refusals and preserving saves; see [workbook protection](workbook-protection.md). Added workbook structure protect/unprotect, SHA/legacy password authentication, preserving metadata/history and a shared native/browser dialog; see [protection authoring](protection-authoring.md). Added worksheet protect/unprotect with optional SHA/legacy authentication, cell lock/hidden-content authoring, fifteen permissions and shared native/browser controls; see [sheet protection](sheet-protection-authoring.md). Added standard/x14 editable-range reading, preserving CRUD/structural shifts, password authentication, unsaved sheet-local grants and a shared manager; see [editable ranges](editable-ranges.md). Added independent worksheet object guards for notes, shapes, pictures and charts plus reader/cached queries for content/object/scenario permissions; see [object protection](object-protection.md). Added individual imported DrawingML/VML position/text locks, owned identities/selected references and preserving drawing/note updates; see [individual object protection](individual-object-protection.md). Added automatic typed single-cell range-password prompts through the shared dialog and both editors; see [range prompts](range-unlock-prompts.md). Added automatic bulk-operation authentication with deferred proofs and atomic command/grant commit; see [bulk authentication](bulk-range-authentication.md). Pending navigation replay, individual flag-authoring/selection feedback, general drawing ownership/MCE, account ACLs, scenario editing, broader selection/application oracles, window/revision guards, additional digest/locale coverage and encrypted XLSX flow remain |
| Collaboration and review | `xlsx/threaded_comments.rs`, `spelling_panel.rs` | Legacy notes | Threaded comments and spelling UI remain |
| Page setup and print | `xlsx/page_layout.rs`, `header_footer*.rs`, `print.rs` | Read settings, paginate/PDF, header/footer writer | Added paper/orientation/margins/scaling/page order/print titles/print area writers and model; Page Setup UI, break authoring, headings/gridlines/centering and workbook-wide print remain |
| File formats and recovery | `xlsx/import_text.rs`, `repair.rs`, `backup_save.rs`, `ibis-pptx/src/xls*.rs`, `ods*.rs` | XLSX/CSV/PDF; opaque parts preserved | XLS/XLSB/ODS, text import options, macro/template conversion, recovery/repair UI remain |
| Accessibility, properties, settings | `xlsx/accessibility_panel.rs`, `properties_panel.rs`, `compatibility_panel.rs` | Basic shell/file handling | Workbook accessibility checker, object alt-text editing, properties/custom properties and compatibility/settings UI remain |

## Regression gates added

Table-authoring checks and current storage-related verification limits are in
[tables.md](tables.md); UI presence is not counted as a passed runtime check.


- `jade-xlsx-headless-editor/tests/defined_names.rs`: scoped definitions,
  constants/expressions, forward dependencies, cycles, invalid inputs,
  prefixed XML, metadata preservation, save/reopen and Undo/Redo.
- `tests/manages_sheets.rs`: deleting a referenced sheet now propagates
  `#REF!` through a named formula instead of retaining its old numeric cache.
- `tests/sheet_properties.rs`: hide/unhide/VeryHidden, last-visible-sheet
  protection, saved active tab, tab colours and preserving sheet properties.
- `tests/page_setup.rs`: print settings, quoted sheet names, print titles,
  scoped print area, printer attributes, unrelated parts, invalid inputs and
  history.
- `jade-xlsx-headless-editor/tests/hyperlinks.rs`: external/internal links,
  shared relationships, partial replacement/removal, strict/aliased XML,
  unknown metadata, atomic text/style changes, protection and array spills.
- `jade-office-model/tests/links_like_excel.rs`: named destinations, dialog
  text/ScreenTips, numeric/formula type preservation and history.
- `jade-xlsx-headless-editor/tests/data_validations.rs`: all standard rule types,
  namespace aliases, foreign metadata, partial clearing, overlap replacement,
  protection, save/reopen and history.
- `jade-xlsx-headless-editor/tests/table_ownership.rs` and
  `jade-xlsx-writer/tests/table_namespaces.rs`: explicit worksheet ownership,
  external/wrong-type/foreign links, Strict aliases and ordering, orphan part
  preservation, numeric header-position edits, duplicate target shifts/deletion,
  owned template/formula hierarchy, filter/sort metadata, local namespace
  declaration scope and maximum column IDs.
- `jade-xlsx-headless-editor/tests/extended_validations.rs`: imported x14 lists
  and relative bounds, storage-preserving indexed edits, standard append index,
  cross-format overlap clearing/replacement, Strict/default namespaces,
  multiple containers, unmodeled siblings, missing-formula schema ordering and
  foreign wrapper/attribute exclusion.
- `jade-xlsx-headless-editor/tests/compatibility_functions.rs`: ECMA rounding,
  typed/imported SINGLE and ANCHORARRAY, scalar/array/cross-sheet behavior,
  arities/prefixes, stale caches, spills/cycles, lambda shadowing, broadcast
  errors/resource bounds and FORMULATEXT. A direct engine test checks a later
  spill anchor; the shared typing target checks calls and rejected-entry history.
- `jade-xlsx-headless-editor/tests/formula_ownership.rs`: cross-sheet validation
  sources, table rename/column context/A1 conversion, implicit cell addresses,
  owned conditional thresholds and sparkline/date-axis sources. Writer units
  separately cover cell source alternatives, opaque leaves, CDATA/comments and
  invalid XML output; see [formula-ownership.md](formula-ownership.md).
- `jade-xlsx-reader/tests/sparkline_ownership.rs`: foreign/misplaced groups,
  wrapped settings, source/destination ownership, Strict/default namespaces,
  CDATA/entities, invalid/out-of-grid destinations and one-cell ranges.
  The headless ownership target adds cross-sheet edits, exact foreign XML,
  save/reopen and model/package history with those interpreted groups.
- Reader `sheet_protection`, headless `object_protection` and model
  `object_protection_like_excel`: independent flags, supported operation guards,
  selected MCE ownership, cross-sheet identity, authentication and cache/history
  refresh; see [object-protection.md](object-protection.md).
- Reader/writer `protected_ranges`, headless `editable_ranges`, OPC range
  vectors and model `editable_ranges_like_excel`: standard/x14 identity,
  namespace ownership, strong-verifier precedence, preserving CRUD/shifts,
  session grants and revocation, batch atomicity, stale dialogs and history;
  see [editable-ranges.md](editable-ranges.md).
- Reader `sheet_protection`, writer `sheet_protection_authoring` and model
  `sheet_protection_like_excel`: malformed flags, defaults and aliases,
  authenticated preserving edits/history, cell lock/hidden formats, row/column
  inheritance, dialog sheet identity and sparse selection; see
  [sheet-protection-authoring.md](sheet-protection-authoring.md).
- Shared OPC protection tests and writer `workbook_protection_authoring`:
  independent ISO/legacy vectors, Unicode/code pages, limits, modern-verifier
  precedence, correct/wrong passwords, preserving revisions/foreign XML,
  relocated parts and authoring/save/history. Four added shared-model cases
  exercise masked confirmation, cancellation, fresh platform salts and redo;
  see [protection-authoring.md](protection-authoring.md).
- Reader/writer `workbook_protection` targets and shared-model
  `workbook_protection_like_excel`: imported flags, direct namespace ownership,
  all existing sheet structural commands, hidden-sheet unhide, no-op bytes,
  ordinary cell edits and atomic save/history. API/CLI/MCP/diff suites and
  the separate native-render suite are also exercised; see
  [workbook-protection.md](workbook-protection.md) for exact scope and limits.
- `cell_content` targets in the reader, writer and headless editor: common
  value/run/phonetic selection, shared-string indices, implicit coordinates,
  Strict/local namespaces, quoted attributes, nested/empty alternatives,
  selected/compatible source edits and caches, stored-value eligibility,
  local namespace declarations, exact foreign XML and save/history. The
  alternate-grid scenario compares both understood capability selections;
  this is separate from an Office application oracle.
- `jade-xlsx-headless-editor/tests/validation_structure.rs`: deleted row/column
  anchors, removed first union members, absolute deleted references, qualified
  cross-sheet sources, x14/formula and coverage alternatives, unknown XML,
  grid-edge clipping, CDATA/no-op bytes, saved files, input results and history.
  The model validation target also checks standard/x14 input after row deletion.
- `jade-xlsx-headless-editor/tests/quoted_validations.rs`: quoted CSV values,
  source Choice/Fallback selection, Strict/x14 aliases, preserving source
  edits, malformed/atomic authoring, literal structural preservation, x14
  formula-leaf cardinality and relative alternative anchors. Reader codec
  checks cover exact values and malformed fields. Shared-model cases cover
  dialog CSV, formula-like literal selection, boolean/date coercion and history.
- `jade-xlsx-calc/tests/validation.rs`: candidate-aware custom formulas,
  relative bounds, dates, named lists on hidden sheets and dynamic sources.
- `jade-office-model/tests/validation_like_excel.rs`: dialog settings,
  messages, explicit Warning/Information acceptance, relative rebasing,
  matching-rule changes and multi-range history.
- `jade-xlsx-writer/tests/cell_shift_extensions.rs`: paired and standalone
  x14 conditional formatting, shared priority ordering, aggregate populations,
  standard CFVO formulas, x14 validation formulas/counts/identities, sparkline
  source/display deletion, cross-sheet sources, Strict/default namespaces and
  foreign lookalikes. `headless-editor/tests/x14_bars.rs` also checks unchanged
  signed bar geometry, save/reopen and exact history after a partial shift.
- `jade-office-model/tests/names_like_excel.rs`: name-box range definitions,
  cross-sheet navigation, scope shadowing, save/reopen, and visible tabs
  through Undo/Redo.

New metadata operations keep the existing package and edit the affected XML
only. No Excel application round-trip or pixel oracle has yet been run for
these additions. API/model availability is not counted as a completed UI.
The commands, counts and limitations of the checks performed are recorded in
[verification.md](verification.md).

## Reproducing the function comparison

Generate the Jade registration list from the actual engine:

```sh
cargo run --manifest-path jade-core/Cargo.toml \
  -p jade-xlsx-headless-editor --example function_inventory > /tmp/jade-functions.json
python3 scripts/xlsx/compare_functions.py --jade /tmp/jade-functions.json
```

The reference inventory was obtained by calling
`ibis_calc::functions::implemented()` at the pinned revision, sorting and
deduplicating the result. `ANCHORARRAY` and `SINGLE` are now registered names
with parser/operator representations in Jade. Their earlier inventory absence
did not mean all saved spill/intersection formulas were unsupported; the
[compatibility-call cases](compatibility-functions.md) distinguish the actual
call, metadata and evaluation fixes. Registration alone
does not demonstrate numerical accuracy, pivot/image provider integration or
network-backed evaluation.

The goal remains open until the remaining areas above have implementations,
UI paths and appropriate regression/oracle evidence. Do not replace this with
a blanket parity percentage based on function or catalogue entry counts.


The [bulk editable-range authentication checkpoint](bulk-range-authentication.md)
passes 1,251 XLSX cases across 182 targets, 22 OPC unit cases and 142 shared-model
cases across twenty-six named tiny-skia targets, plus both application compiler
graphs. This is a scoped regression result; full reference/Excel equivalence is
not established.


The [pivot/cache reader checkpoint](pivot-reader.md) passes 1,272 XLSX cases
across 184 targets, 22 OPC unit cases and 144 shared-model cases across twenty-seven
named tiny-skia targets, plus both actual application compiler graphs. This earlier
reader checkpoint preceded GETPIVOTDATA evaluation and the later explicit-refresh work.


The [saved-pivot query checkpoint](getpivotdata.md) passes 1,300 XLSX
cases across 186 targets, 22 OPC cases, 146 shared-model cases across 27 named
tiny-skia targets and 2,242 actual engine cases (eight existing ignored), plus
both actual application compiler graphs. The registry now has 507 of the pinned
reference's 512 names. Full ibisOffice/Excel parity remains incomplete.


The [explicit pivot refresh checkpoint](pivot-refresh.md) passes 1,334
XLSX cases across 188 targets, 22 OPC cases, 148 shared-model cases across 28
named tiny-skia targets and 2,242 engine cases (eight existing ignored), plus
both actual application compiler graphs. Counts replace earlier checkpoints.
The function registry remains 507 of 512; full ibisOffice parity remains open.


The [refresh-all checkpoint](pivot-refresh-all.md) passes 1,338 XLSX
cases across 188 targets, 22 OPC cases, 150 shared-model cases across 28 named
tiny-skia targets and 2,242 engine cases (eight existing ignored), plus both
actual application compiler graphs. Registry507 and the pinned engine patch
remain unchanged and were re-executed/replayed. Declaration order is explicit;
general dependency scheduling and full ibisOffice parity remain open.


The [dependency-order checkpoint](pivot-refresh-order.md) passes 1,350
XLSX cases across 189 targets, 22 OPC cases, 151 shared-model cases across 28
named tiny-skia targets and 2,246 engine cases (eight existing ignored), plus
both actual application compiler graphs. The regenerated engine patch replays
489 files; the actual registry remains 507. Runtime-observed references and
resource limits do not establish a complete dependency or full parity oracle.

## Current pivot style and standard ownership checkpoint

The [style checkpoint](pivot-styles.md) and its
[machine-readable evidence](pivot-styles-verification.json) replace prior totals:
1,371 XLSX cases /191 targets, 22 OPC cases and 152 shared-model cases /28 named
tiny-skia targets passed, plus both actual application compiler graphs. Engine
code is unchanged: prior2,246 passed/eight ignored and registry507 are retained
as historical runtime evidence; the pinned489-file patch was replayed again.
The unmodified normative workbook exposed a real ownership error: CT_Worksheet
has no pivotTableParts list. Actual relationship ownership now yields22 tables.
Earlier synthetic-marker tests did not establish standard ownership. Named
style/refresh raster regressions do not establish full Office or ibis parity.

## Current pivot column-fitting checkpoint

The [fitting checkpoint](pivot-autofit.md) and
[machine-readable evidence](pivot-autofit-verification.json) replace earlier
counts: 1,385 XLSX cases /193 targets, 22 OPC cases and 153 shared-model cases
/28 named tiny-skia targets passed, plus both actual application compiler
graphs. The same pinned 489-file engine patch was replayed; historical engine
2,246 passed/eight existing ignored and registry 507 are carried unchanged.
This adds actual host-measured refresh widths and page-filter geometry, with
atomic history/refusal cases. Full Office width/producer/application oracles,
PivotChart bindings and other audit gaps remain.

## PivotChart refresh and sheet identity follow-up

The [PivotChart checkpoint](pivot-charts.md) adds typed standard chart-space
bindings, local report projection and atomic category/series reconciliation.
Stable member colors, combination-plot roles, reference-wrapper metadata,
report-tab renames and literal/quoted sheet identity survive refresh and
history. Actual TinySkia refreshed pixels and native Skia saved pixels pass.

Final replacement totals are 1,409 XLSX cases /195 targets, 154 model cases /28
named tiny-skia targets and 38 OPC cases, plus both actual app compiler graphs.
The engine/registry remain unchanged; prior 2,246 passes/eight ignored and 507
registrations are carried evidence, while the 489-file patch was replayed again.
The byte-only API cannot independently establish filename identity. Advanced
PivotCharts, field controls, complete external/producer/Office oracles, pivot
authoring and all other audit gaps remain. See the
[proof](pivot-charts-verification.json); full parity is not claimed.

## Saved pivot authoring and independent validation

The [authoring checkpoint](pivot-authoring.md) adds a common native/browser
editor for existing worksheet pivots: source, origin, field layout, values,
members, pages, field options and styles. Report/cache/chart/fitting/calculation
changes share one commit. Stable original measure identities survive insertion,
reordering and removal. Actual page geometry, absent legacy cache-link repair,
explicit-link consistency, cached-member refresh and stale dialog refusal are
covered. Source field headers/order/count must currently remain unchanged.

Replacement totals are 1,426 XLSX cases /196 targets, 160 model cases /28 named
tiny-skia targets, 38 OPC cases and both actual application compiler graphs.
Twenty actual Transitional/Strict outputs also have literal saved-value checks.
Official unmodified schemas check180 components:160 raw passes,20 documented
Office xml:space deviations and zero unexpected failures. The real SDK validates
ten Transitional packages with zero errors. Ten valid-Strict percentage lexical
errors remain in the SDK probe; full Strict SDK validation is not claimed.
Engine2246/eight ignored and registry507 are carried unchanged evidence.

Creation/deletion, arbitrary source field remapping, advanced/calculated/grouped
and show-as authoring, full chart/style/Office/interaction oracles and all other
audit gaps remain. Detailed hashes and exclusions are in the
[proof](pivot-authoring-verification.json); full parity remains incomplete.

## Pivot creation, deletion and displayed chart snapshots

The [creation/deletion checkpoint](pivot-lifecycle.md) adds both context-menu
operations and a common source/destination dialog. A new worksheet, cache graph,
report, fitting and recalculation commit once. Creation currently suggests an
initial summary with an independent cache; it is not an empty field-selection
placeholder. Removal preserves other shared reports/caches and manual formats,
collects only unreferenced cache parts, and snapshots supported bound charts
from the worksheet values currently displayed, including stale/missing caches.
Raw saved formula display values are not mislabeled as calculation-verified.

Replacement totals are1,448 XLSX cases /197 targets,164 named tiny-skia model
cases /28 targets,38 OPC cases and both actual application compiler graphs.
Actual static-chart pixels survive deletion and history from a stale-cache input.
Thirty-four original Transitional/Strict outputs assert literal data and graph
state. Official unmodified schemas check308 components:282 raw passes,26
separately retained Office xml:space deviations and zero unexpected failures.
Actual SDK validation passes17 Transitional packages with zero errors;17 valid-
Strict percentage errors remain explicit SDK probe limitations.

Blank/no-value design, reused-cache creation, arbitrary field remapping, hidden/
mixed-format/static-chart reconciliation, format deltas, advanced pivots,
Office/widget oracles and the remaining audit gaps stay open. Engine2246/eight
ignored, registry507 and the historical489-file patch replay are carried
unchanged evidence. Details are in the [proof](pivot-lifecycle-verification.json).
