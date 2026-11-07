# XLSX coverage

Audit started 2026-10-23 against Excel behaviour. Jade baseline: jade-office
`e97451d`, jade-core `e3a5929`.

This is a coverage audit, not an Excel certification. Jade regression tests,
generated-workbook checks and Office oracle captures are separate evidence;
UI presence alone is not counted as a passed runtime check.

## Findings and order of work

Jade engine paths are under `jade-core/crates`. “Preserved” means the original
package part is retained, not that Jade can interpret or edit its contents.
“Partial” explicitly does not claim parity.

| Area | Jade at the baseline | Current gap / acceptance requirement |
|---|---|---|
| Package read, cells, shared formulas, 1900/1904 | Reader/writer, shared formula expansion and date epoch tests | Added common namespace/hierarchy-aware standard cell/shared-string/rich/phonetic reading, Choice/Fallback selection, owned cell/cache writes and existing compatible-representation updates; see [cell content](cell-content.md). General MCE, alternate grid/shared-formula topology, other feature families, full schema validation and production/application corpus evidence remain |
| Formula registration | 503 registered names | Added ECMA.CEILING and complete call/prefix/registry paths for SINGLE/ANCHORARRAY: 507 registered names, including saved-result GETPIVOTDATA. The Excel functions still unregistered, the generated inventory and semantic limits are in [compatibility-functions.md](compatibility-functions.md) |
| Formula semantics, arrays, names | IronCalc adapter, spills, scoped range names | Added named constants/expressions and cycle termination; relative names, named LAMBDAs and array-producing names need broader oracle coverage |
| External links and service functions | Original parts and unsupported formula caches preserved | External workbook manager, refresh, linked data types, service/provider integration missing |
| Typed input, IME, furigana, cell rich text | Typed input/IME/PHONETIC; rich text read and drawn | Rich-text run authoring and phonetic editing missing |
| Clipboard and Paste Special | Values/formulas/formats/transpose/skip blanks | Added returned-error rollback for tiled text/internal-copy paste and active-entry range fill, complete edit checkpoints, guarded cut sources and sparse full-axis permissions; see [compound edit atomicity](bulk-edit-atomicity.md). Arithmetic operations, column widths, validation-only paste, links, external rich clipboard remain. Added shared native/browser bulk editable-range authentication for paste, clear, all fill variants and move/copy, with deferred credentials, exact write coverage and atomic command/grant commit; see [bulk authentication](bulk-range-authentication.md) |
| Clear, Find, Go To | Clear contents, Find/Replace | Clear formats/all, result list, values/notes/selection scopes, Go To Special remain |
| Fill and input assistance | Numeric series, copy/fill handle | Date/text series, flash fill, formula suggestions/argument help remain |
| Names and scope | Read-only definitions | Added scoped create/update/delete/recalculation and name-box create/navigation; Name Manager dialog remains |
| Cell formatting and styles | Number/font/fill/alignment/wrap/borders editor | Rotation, vertical text, shrink-to-fit, indent, pattern/gradient fills, named styles/theme authoring remain |
| Conditional formatting | Wide read/render support; basic rule creation | The package rewrite now follows owned standard/x14 conditional formulas and formula thresholds. Full rule editing, precedence, stop-if-true, custom icons, advanced data-bar authoring and whole-axis anchor/namespace cases remain |
| Sheet tabs | Add/delete/rename/move; visibility/tab colour read | Added hide/unhide in both shells and tab-colour writer/model; sheet copy and tab-colour controls remain |
| Rows/columns, cell shifts, outlines | Whole-axis insertion/deletion, hiding/sizing/fit | Added cell-range insertion/deletion, structural table-header insertion, shared-model APIs and both context menus; 1,106 tests across the six selected implementation crates passed. x14 rule/validation/sparkline coordinate and identity reconciliation is implemented; keyboard choice dialog, other extension families, grouping and subtotal authoring remain; see [cell-shifts.md](cell-shifts.md) |
| Sheet/window views | Saved freeze panes/gridlines; application zoom | Persisted freeze/view changes, split views, custom views and workbook windows remain |
| Tables | Read/render, structured-reference calculation, header rename | Added create/resize/style/header/totals/calculated-column/entry expansion/conversion writers, model and dialogs. The regression across the six selected implementation crates (1,106 tests) and both shell builds passed. Explicit ownership, duplicate targets and table namespace/template hierarchy are handled. Added table-width/height band shifts, structural header insertion, calculated-row fill and saved-template rebasing. Custom-style authoring, dedicated table commands, clipboard/filter/spill/extension edge cases, dialog runtime and Office oracles remain. See [tables.md](tables.md) |
| Sort/filter | Engine multi-key/custom/date/rank filters; basic UI | Advanced/custom sort/filter UI, colour/icon criteria, custom lists and advanced filters remain |
| Validation | Read; limited UI enforcement | Added preserving writer, all standard rule types, relative/formula bounds, names/dynamic lists, input/error messages, alert choices, native/browser editing and history. x14 reading, indexed preserving edits, shared input enforcement, quoted-list/source-alternative read/write and dropdown literal handling added. Whole-axis edits now retain x14 rules and rebase deleted anchors while isolating opaque XML. The package rewrite used by partial shifts and table changes now scopes formula ownership and x14 table context; invalid-data circles, validation-only paste, general MCE, advanced coercion and Office oracle coverage remain; see [validation.md](validation.md) |
| Hyperlinks and notes | Link following, note authoring | Added URL/file/internal/named-link authoring, display text/ScreenTips, range removal, native/browser dialogs and history. Relative file opening/base-address handling and Office round-trip/oracle cases remain; see [hyperlinks.md](hyperlinks.md) |
| Charts | Basic authoring; classic/combo/radar/bubble rendering | Extended/3-D/stock/surface/statistical charts, full axes/series/labels/effects controls and chart sheets/templates remain |
| Sparklines | Read/render | Added namespace/hierarchy-aware reading, CDATA/entity leaves and valid single-cell destinations, with cross-sheet edits/save/history cases; see [sparkline reading](sparkline-reading.md). Create/edit/group/style/axis controls, authoring writer, date-axis rendering, general MCE and Office oracles remain |
| Shapes, pictures and SmartArt | Drawing parts retained; basic draw/move/resize/delete/text | Insertion, geometry/effects/crop, grouping, SmartArt authoring, in-cell/linked images remain |
| Ink and form controls | Parts preserved | Read/render/edit support remains |
| Pivot tables/charts | Parts preserved | Added typed catalog, GETPIVOTDATA, structural movement, selected/all refresh and observed dependency scheduling. Standard relationship ownership reads 22 normative reports; named custom/84 normative styles reach both shells and raster/history tests. Refresh now saves report/filter column fitting using backend metrics, shared-column maxima and opt-out/protected widths in one atomic edit; see [autofit](pivot-autofit.md). Added locally corroborated classic PivotChart refresh, stable point/series styles, reference preservation and report-tab rename; see [PivotCharts](pivot-charts.md). Added preserving existing-pivot source/layout/measure/member/page/style editing and a common native/browser editor; see [authoring](pivot-authoring.md). Source headers/order/count must remain unchanged. Added atomic creation/deletion, new/existing destinations, a common creation panel and displayed-value static chart snapshots; see [creation/deletion](pivot-lifecycle.md). Blank/no-value design, reused-cache creation, source field remapping, advanced/calculated/grouped/show-as authoring, complete legacy fitting/page/group/styles/charts/controls/external identity, load refresh, OLAP and broad Office oracles remain |
| Slicers and timelines | Parts preserved | Model/read/render, filtering, connections and authoring remain |
| Queries and XML maps | Parts preserved | Query operations, source connections/refresh, model authoring and XML maps remain |
| What-if and analysis | Formula calculation only | Goal Seek, Solver, scenarios, data tables, analysis/forecast tools remain |
| Protection and encryption | Sheet protection read and enforced on supported edits | Added imported workbook structure protection in reader/writer/model and both shell command guards, with atomic refusals and preserving saves; see [workbook protection](workbook-protection.md). Added workbook structure protect/unprotect, SHA/legacy password authentication, preserving metadata/history and a shared native/browser dialog; see [protection authoring](protection-authoring.md). Added worksheet protect/unprotect with optional SHA/legacy authentication, cell lock/hidden-content authoring, fifteen permissions and shared native/browser controls; see [sheet protection](sheet-protection-authoring.md). Added standard/x14 editable-range reading, preserving CRUD/structural shifts, password authentication, unsaved sheet-local grants and a shared manager; see [editable ranges](editable-ranges.md). Added independent worksheet object guards for notes, shapes, pictures and charts plus reader/cached queries for content/object/scenario permissions; see [object protection](object-protection.md). Added individual imported DrawingML/VML position/text locks, owned identities/selected references and preserving drawing/note updates; see [individual object protection](individual-object-protection.md). Added automatic typed single-cell range-password prompts through the shared dialog and both editors; see [range prompts](range-unlock-prompts.md). Added automatic bulk-operation authentication with deferred proofs and atomic command/grant commit; see [bulk authentication](bulk-range-authentication.md). Pending navigation replay, individual flag-authoring/selection feedback, general drawing ownership/MCE, account ACLs, scenario editing, broader selection/application oracles, window/revision guards, additional digest/locale coverage and encrypted XLSX flow remain |
| Collaboration and review | Legacy notes | Threaded comments and spelling UI remain |
| Page setup and print | Read settings, paginate/PDF, header/footer writer | Added paper/orientation/margins/scaling/page order/print titles/print area writers and model; Page Setup UI, break authoring, headings/gridlines/centering and workbook-wide print remain |
| File formats and recovery | XLSX/CSV/PDF; opaque parts preserved | XLS/XLSB/ODS, text import options, macro/template conversion, recovery/repair UI remain |
| Accessibility, properties, settings | Basic shell/file handling | Workbook accessibility checker, object alt-text editing, properties/custom properties and compatibility/settings UI remain |

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

## Reproducing the function inventory

Generate the Jade registration list from the actual engine:

```sh
cargo run --manifest-path jade-core/Cargo.toml \
  -p jade-xlsx-headless-editor --example function_inventory > /tmp/jade-functions.json
```

The output is sorted and deduplicated. `ANCHORARRAY` and `SINGLE` are now registered names
with parser/operator representations in Jade. Their earlier inventory absence
did not mean all saved spill/intersection formulas were unsupported; the
[compatibility-call cases](compatibility-functions.md) distinguish the actual
call, metadata and evaluation fixes. Registration alone
does not demonstrate numerical accuracy, pivot/image provider integration or
network-backed evaluation.

The goal remains open until the remaining areas above have implementations,
UI paths and appropriate regression/oracle evidence. Do not replace this with
a blanket parity percentage based on function or feature counts.


The [bulk editable-range authentication checkpoint](bulk-range-authentication.md)
passes 1,251 XLSX cases across 182 targets, 22 OPC unit cases and 142 shared-model
cases across twenty-six named tiny-skia targets, plus both application compiler
graphs. This is a scoped regression result; full Excel equivalence is
not established.


The [pivot/cache reader checkpoint](pivot-reader.md) passes 1,272 XLSX cases
across 184 targets, 22 OPC unit cases and 144 shared-model cases across twenty-seven
named tiny-skia targets, plus both actual application compiler graphs. This earlier
reader checkpoint preceded GETPIVOTDATA evaluation and the later explicit-refresh work.


The [saved-pivot query checkpoint](getpivotdata.md) passes 1,300 XLSX
cases across 186 targets, 22 OPC cases, 146 shared-model cases across 27 named
tiny-skia targets and 2,242 actual engine cases (eight existing ignored), plus
both actual application compiler graphs. The registry now has 507 names. Full Excel parity remains incomplete.


The [explicit pivot refresh checkpoint](pivot-refresh.md) passes 1,334
XLSX cases across 188 targets, 22 OPC cases, 148 shared-model cases across 28
named tiny-skia targets and 2,242 engine cases (eight existing ignored), plus
both actual application compiler graphs. Counts replace earlier checkpoints.
The function registry remains 507; full Excel parity remains open.


The [refresh-all checkpoint](pivot-refresh-all.md) passes 1,338 XLSX
cases across 188 targets, 22 OPC cases, 150 shared-model cases across 28 named
tiny-skia targets and 2,242 engine cases (eight existing ignored), plus both
actual application compiler graphs. Registry507 and the pinned engine patch
remain unchanged and were re-executed/replayed. Declaration order is explicit;
general dependency scheduling and full Excel parity remain open.


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
style/refresh raster regressions do not establish full Office or Excel parity.

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

## Formula identity, literal text and display formats (2026-11-07)

This checkpoint fixes scoped round-trip and calculation differences:

- LET/LAMBDA variable prefixes respect lexical scope, sheet/table references,
  escaped table headers, Unicode and scientific numeric literals.
- Shared/inline rich-text escapes decode per text run. Literal escape-looking
  strings and whitespace in cell/chart number formats survive writing again.
- CSV separator directives, BOM decoding, signed numeric inputs, elapsed time
  and AM/PM entry have maintained model regression coverage.
- Standard error constants remain constants through entry, Paste Special,
  saving and history. ISFORMULA distinguishes them from formulas. Unsupported
  formulas held at their cached values retain source text for ISFORMULA and
  FORMULATEXT, including restoration after what-if trials.
- ISFORMULA returns #VALUE! for non-reference arguments. Number formats handle
  escaped layout characters, repeated percent scaling, case-insensitive elapsed
  tokens and conditional negative fractions. Private-use text cannot collide
  with the internal repeat delimiter, including when real repeats are present.

The latest complete selected package runs pass 218 calculation cases /9 targets
and 210 layout cases /31 targets. The selected model CSV, Paste Special and
scenario run passes 30 cases /3 targets; these counts describe separate runs,
not a fresh execution of every XLSX or application test. Maintained reader,
writer and editor regressions cover the other changes. Native Excel UI
comparison was unavailable because the computer-use runtime failed during
initialization; no application round-trip is claimed. Existing audit gaps,
unsupported function semantics and broader Office oracles remain open.

The time-format follow-up fixes independent whole-second rounding in elapsed
formats with fractional seconds: 3735.8 seconds under `[ss].00` now displays
3735.80 rather than 3736.80. Minute, hour and day boundaries share the same
rounding result. Lowercase `am/pm` and `a/p` display tokens retain their case.
Typed/CSV short meridiem suffixes (`9:00 p`, `12:00 a`) become numeric times,
with calculation and XLSX save/reopen coverage. The selected format run passes
43 cases, the conformance/property runs pass five cases, and the latest complete
CSV model target passes 12 cases. These are focused follow-up runs, not a new
full-package total. Primary expectations are Microsoft's
[number-format guidelines](https://support.microsoft.com/en-us/excel/review-guidelines-for-customizing-a-number-format)
and [manual data-entry instructions](https://support.microsoft.com/en-us/excel/enter-data-manually-in-worksheet-cells).

The information-function follow-up removes i64 saturation from ISODD/ISEVEN:
large even numbers such as 1E20 and 1E100 no longer become odd. Negative values,
fractional truncation and zero retain the documented behavior. TYPE now returns
64 for array constants (including a single element or error-bearing arrays) and
calculated arrays, alongside its existing scalar type results. Both differences
were reproduced by failing workbook regressions before their fixes. All 71
calculation cases /9 targets pass after both changes; this does not constitute
native Excel comparison or verify compound/LAMBDA type semantics. Primary
expectations are Microsoft's [ISODD](https://support.microsoft.com/en-us/excel/functions/isodd-function),
[ISEVEN](https://support.microsoft.com/en-us/excel/functions/iseven-function) and
[TYPE](https://support.microsoft.com/en-au/excel/functions/type-function) descriptions.

The IS-predicate follow-up adds element-wise array/range handling for ISNUMBER,
ISTEXT, ISNONTEXT, ISLOGICAL, ISBLANK, ISERROR, ISERR and ISNA. Static shape
analysis now recognizes their spill results. Numeric-looking strings stay text,
empty-string formulas stay nonblank, and errors are inspected without being
propagated or confused with text. Mixed arrays, two-dimensional blank/value
ranges and FILTER composition have literal expected-value regressions. A headless
editor test checks source edits, dependent spills/aggregation, undo/redo and both
cached and recalculated XLSX reopen paths.

At this checkpoint, the standalone engine library passes 2,301 tests with nine
existing ignored tests; all 73 calculation package cases /9 targets and the new
editor lifecycle case pass. The tiny-skia model library type check also passes.
Three-dimensional ranges remain unsupported, and predicate range materialization
uses the engine's existing one-million-element matrix limit, which is not an
Excel limit. Additional conversion cases, other information-function array cases, native
application oracles and the remaining audit gaps are still open. Predicate type
expectations follow Microsoft's [IS function documentation](https://support.microsoft.com/en-us/excel/functions/is-functions);
these regression runs do not claim full Excel equivalence.

The N/T follow-up shares the guarded scalar/array/range mapping operation with
the IS predicates. N preserves numbers and errors, converts booleans to 1/0 and
text/blanks to zero; T preserves text and errors, returning empty text for other
values. Their array elements spill independently, and calculated arrays compose
with IFERROR, SUM and TEXTJOIN. TEXTJOIN now consumes data arrays in row-major
order, applies ignore_empty to empty strings as well as blank cells, and checks
its 32,767 UTF-16-unit result bound before allocating the joined result. Its
252-text-argument bound is enforced at evaluation.

The current standalone engine run again passes 2,301 cases with nine existing
ignored cases; the complete calculation package run passes 75 cases /9 targets.
Two editor lifecycle cases cover source type changes, spilled values, dependent
aggregates, undo/redo and cached/recalculated save/reopen. These checks do not
cover every TEXTJOIN case: Unicode boundary behavior still requires follow-up, alongside the existing
3-D/matrix-size and Office-oracle limitations. Primary value-conversion and
text-join expectations are Microsoft's [N](https://support.microsoft.com/en-us/excel/functions/n-function),
[T](https://support.microsoft.com/en-gb/excel/functions/t-function) and
[TEXTJOIN](https://support.microsoft.com/en-us/excel/functions/textjoin-function) descriptions.

The delimiter follow-up accepts TEXTJOIN delimiter arrays and ranges instead of
coercing them to a scalar or an unimplemented error. Delimiters cycle across all
text arguments after empty values are omitted; a blank delimiter remains an
empty separator. Numeric/logical delimiters convert to text, and delimiter
errors propagate. The argument signature consumes delimiter vectors and the
length check counts only the delimiters actually used, including partial cycles.
Regression expectations include the Microsoft city/state/postcode example's
comma/comma/comma/semicolon pattern, vertical and calculated delimiter arrays,
empty values, errors and length boundaries.

The latest standalone engine run passes 2,301 cases with nine existing ignored
cases; all 76 calculation package cases /9 targets and three editor lifecycle
cases pass. Delimiter-cell edits, blank separators, undo/redo and cached versus
recalculated XLSX reopen are verified. The existing delimiter-range matrix/3-D
limits, Unicode boundaries and native Office
oracles remain explicit follow-up areas; full TEXTJOIN or Excel parity is not
claimed. Primary delimiter behavior is illustrated in Microsoft's
[TEXTJOIN example 3](https://support.microsoft.com/en-us/excel/functions/textjoin-function).

The sparse-range follow-up retains the logical size of TEXTJOIN source ranges,
including blank tails in whole rows/columns. The old used-dimension truncation
silently discarded separators when ignore_empty was false. The replacement
visits stored cells in row/column order and accounts for intervening blank runs
without materializing a string for each empty cell. Delimiter cycles and output
length use wide, saturating counts; the bounded output is constructed only while
it fits the text limit. Source formulas are evaluated before the sparse snapshot
so newly materialized spill members are included.

Literal regressions cover empty rows, full columns, a large rectangle extending
to the worksheet boundary, blank-run delimiter cycling across arguments, length
errors and dynamic spill sources. The selected editor lifecycle test changes an
empty cell to text and verifies whole-row/column aggregates through undo/redo and
both cached/recalculated save/reopen. The current engine run passes 2,301 cases
with nine existing ignored cases; all 78 calculation package cases /9 targets
and four editor lifecycle cases pass. These are targeted compatibility results;
delimiter range limits, Unicode boundary captures, omitted arguments and broader
native Excel comparison remain open.

The next information-function follow-up applies ERROR.TYPE, ISODD and ISEVEN
element by element over arrays/ranges and exposes their spill shapes to static
analysis. ERROR.TYPE returns each recognized error code and #N/A for each
nonerror element. Parity reuses the scalar numeric casts and boolean rejection
per element, retains source errors, truncates fractions and preserves the prior
large-number fix. The shared mapper now allows model-aware scalar conversions
without separately evaluating the source expression for each output value.

A composition regression also exposed spill references discarding a valid array
whenever its first element was an error. Successfully produced arrays now retain
their reference shape, including single-cell error arrays. Failed formulas,
empty FILTER results, blocked spills and cycle guards continue to propagate
errors. Evaluation-local array identity is reset between calculation passes;
failed results do not retain a successful array's status.

The current standalone engine run passes 2,301 tests with nine existing ignored
tests, the full calculation package run passes 81 cases /9 targets, and five
editor lifecycle cases pass. Tests include standard error-code arrays, calculated
errors, mixed-type parity arrays, error-first `#` references, failed-array
transitions, source edits, history and cached/recalculated XLSX reopen. Modern
unimplemented error classes, broader CSE behavior and native Excel captures
remain outside this evidence. Primary scalar expectations are Microsoft's
[ERROR.TYPE](https://support.microsoft.com/en-us/excel/functions/error-type-function),
[ISODD](https://support.microsoft.com/en-us/excel/functions/isodd-function) and
[ISEVEN](https://support.microsoft.com/en-us/excel/functions/iseven-function) documentation.

The FORMULATEXT protection follow-up returns #N/A when its source formula is
hidden by active worksheet protection. A hidden-format flag on an unprotected
sheet still allows formula text, and visible formulas on a protected sheet
remain readable. The existing top-left behavior for multi-cell FORMULATEXT
references follows the scalar documentation; this change does not infer new
ISFORMULA/FORMULATEXT array semantics from the general dynamic-array list.

The host's effective format lookup includes source-sheet protection, explicit
cell styles and inherited row/column formats. It is available even when a named
formula hides the FORMULATEXT call from the worksheet formula text. Cached
unsupported formula identity and ordinary calculation values remain intact.
Protecting/unprotecting a sheet now refreshes dependent formulas under the
existing automatic-calculation policy, in the same edit/history step.

The current engine run passes 2,301 tests with nine existing ignored tests; all
83 calculation package cases /9 targets and 12 selected editor cases /3 targets
(hidden formulas, protected sheets and edit checkpoints) pass. The hidden-formula
case additionally verifies reprotection, cross-sheet and named queries, inherited
format precedence, undo/redo and cached/recalculated save/reopen. This remains
scoped regression evidence; array-reference semantics, wider protection/Office
oracles and other audit gaps stay open. The primary expected #N/A behavior is in
Microsoft's [FORMULATEXT remarks](https://support.microsoft.com/en-us/excel/functions/formulatext-function).

The NUMBERVALUE follow-up implements the documented scalar conversion rules:
empty text becomes zero; spaces inside the text are ignored; separator arguments
use their first Unicode character; repeated decimals and group separators after
the decimal are rejected. Only the configured decimal character is accepted.
Repeated trailing percent signs scale independently, including when separated
by spaces. Invalid numeric spellings such as NaN/inf and nonfinite parse results
return #VALUE! instead of entering the workbook as ordinary numbers.

Omitted separator arguments use the engine's current locale rather than fixed
English separators, including omitted argument slots. Standalone German/French
engine cases verify these defaults; this does not add an application locale
selector. Workbook cases cover explicit European/Japanese separator characters,
first-character conflicts, empty text, spaces, percent suffixes and invalid
grouping positions. An editor case checks text/separator edits, dependencies,
undo/redo and cached/recalculated XLSX save/reopen.

The latest engine library run passes 2,302 cases with nine existing ignored
cases; the full calculation package run passes 84 cases /9 targets, and the new
number-conversion editor case passes. Array arguments, unusual separators,
overflow/underflow and Unicode-space boundary comparisons with native Excel
remain open; these tests do not certify all numeric parsing. Primary scalar
expectations follow Microsoft's [NUMBERVALUE documentation](https://support.microsoft.com/en-us/excel/functions/numbervalue-function).

The VALUE follow-up lifts its scalar conversions over arrays and ranges, with
static spill-shape recognition. Numbers, currency text and source errors keep
their types per element; invalid text/booleans return element errors. Blank
cells become zero while formula-produced empty strings remain conversion errors.
Mixed-array IFERROR/SUM composition and error-first spill references are covered.

VALUE also accepts common time constants and date/time combinations, which the
previous number/date-only parser rejected. Date prefixes use the existing locale
date parser and workbook epoch; tests cover both 1900 and 1904 systems, ISO date/
time text, AM/PM boundaries and the official time-difference example. The locale's
currency symbol joins the existing accepted currency spellings; a standalone
en-GB engine case verifies pound conversion. No locale catalog or application
locale selector is added.

The latest engine run passes 2,303 tests with nine existing ignored tests; the
complete calculation package run passes 86 cases /9 targets, and both numeric-
conversion editor lifecycle cases pass. Source edits, spilled errors/values,
dependent aggregation, history and cached/recalculated save/reopen are verified.
Unusual time/elapsed boundaries, wider locale/calendar cases, empty inline-string
import semantics and native Office captures remain outside this evidence. The
primary scalar conversion and time examples are in Microsoft's
[VALUE documentation](https://support.microsoft.com/en-us/excel/functions/value-function).

VALUETOTEXT now accepts its optional concise/strict format, quotes and escapes
text in strict mode, converts errors to text, and maps value arrays/ranges to
spilled text results. Invalid format values and oversized UTF-16 output return
VALUE errors. Expectations follow Microsoft's
[VALUETOTEXT documentation](https://support.microsoft.com/en-us/excel/functions/valuetotext-function).

Calculation result extraction now consults the engine cell type instead of
guessing errors from their spelling. Literal formula results such as "#N/A"
remain text in ordinary results, spills, what-if probes and validation sources;
actual errors retain their error type. The editor lifecycle case verifies
strict/concise conversion, dependencies, edits, history and cached/recalculated
save/reopen.

This checkpoint passes 2,303 engine tests (nine existing ignored), all 91
calculation cases /9 targets, the new text-conversion editor case, and the
application model library check with tiny-skia. Format-array broadcasting,
precise numeric/scientific text representation, blank-cell boundaries and native
Excel comparisons remain open; these checks do not establish complete parity.

ARRAYTOTEXT now builds range/array output incrementally and rejects results
above 32,767 UTF-16 units, including delimiters, quotes and braces. Large blank
ranges stop when the output cannot fit instead of first allocating the entire
range matrix. Fractional format arguments are rejected rather than silently
rounded to concise/strict mode. Tests cover row delimiters, embedded errors,
quote escaping, omitted format, large SEQUENCE output and full-column ranges.

Array constant strings now decode doubled formula quotes when parsed and
re-escape them when serialized. Their stored values therefore match runtime
arrays without a second decoding step when BYROW/BYCOL pass arrays to a LAMBDA.
Direct spills, scalar LEN through INDEX, VALUETOTEXT, ARRAYTOTEXT and editor
save/reopen exercise this representation. LEN applied directly to an array
was still unsupported at that checkpoint; the follow-up below addresses it.

This follow-up passes 2,303 engine tests (nine existing ignored), all 93
calculation cases /9 targets and the expanded text-conversion editor lifecycle
case. Numeric text precision, scalar strict-array edge cases, blank semantics,
locale delimiters and native Excel comparisons still need broader evidence.

LEN now maps over arrays and ranges and reports their spill shape. Regression
cases cover two-dimensional output, quote-decoded text, spaces, numbers,
Booleans, empty cells, formula-empty strings and per-element errors, including
an error-first spill consumed by IFERROR/SUM. The editor lifecycle case checks
source edits, dependent totals and cached/recalculated save/reopen. This
checkpoint passes all 95 calculation cases /9 targets, the editor lifecycle
case and 2,303 engine tests (nine existing ignored).

The existing Unicode scalar count is retained, including separate combining
marks and variation selectors. This matches the surrogate-pair counting rule
of Excel Compatibility Version 2, but Jade does not yet select text-function
behavior from a workbook's compatibility version. Version 1 counts surrogate
pairs differently; no legacy-workbook parity is claimed. See Microsoft's
[LEN documentation](https://support.microsoft.com/en-us/excel/functions/len-function)
and [Compatibility Versions](https://support.microsoft.com/en-gb/excel/compatibility-versions).

REPT now checks its output in UTF-16 units before narrowing the repeat count or
multiplying lengths. Japanese text no longer hits a UTF-8-byte-based early limit,
and large counts cannot overflow the previous i32 multiplication. Fractional
positive counts truncate, zero returns empty text, and oversized output returns
VALUE. Text and count arrays/ranges broadcast with per-element errors; numeric
text and Boolean counts retain scalar coercion. The shared text broadcaster
also preserves one-element arrays for spill references and checks range/output
matrix dimensions before allocation.

The follow-up passes all 97 calculation cases /9 targets and 2,303 engine tests
(nine existing ignored). The expanded editor case verifies REPT range output,
dependent LEN/SUM, source edits and cached/recalculated save/reopen; the tiny-skia
application model library check passes. These are regression checks, not native
Office captures: extreme empty-text counts, error/broadcast precedence, broader
locale coercion and resource limits still need oracle coverage. The documented
scalar rules follow Microsoft's
[REPT documentation](https://support.microsoft.com/en-us/excel/functions/rept-function).

TRIM now maps over arrays and ranges with static spill-shape recognition instead
of returning an unsupported-array error. Its existing ASCII-space-only rule is
retained: tabs, line breaks, NBSP and full-width spaces remain text. Cases cover
two-dimensional arrays, per-element errors, empty cells/formula-empty strings,
numeric/Boolean conversion, literal error-looking text, one-element error spills
and LEN/SUM composition. The editor case verifies range results, source edits,
NBSP/blank output and cached/recalculated save/reopen.

This checkpoint passes all 99 calculation cases /9 targets, 2,303 engine tests
(nine existing ignored), the expanded text-conversion editor case and the
tiny-skia application model library check. Broad native Office captures and
the previously listed locale, Unicode-version and array edge cases remain
open. The space rule follows Microsoft's
[TRIM documentation](https://support.microsoft.com/en-us/excel/functions/trim-function).

CLEAN and UNICODE now map over arrays/ranges with static spill-shape recognition.
CLEAN removes ASCII code points 0–31 while retaining higher control characters
and NBSP; UNICODE returns each text's first scalar code point, with VALUE for
empty text and preserved source errors. Tests cover numeric/Boolean coercion,
Japanese text, emoji, empty cells/formula-empty strings, one-element error
spills, IFERROR/SUM and CLEAN/TRIM composition.

The expanded editor case saves all ASCII controls 0–31 in source text and
verifies CLEAN/TRIM/UNICODE results after cached/recalculated reopen. Source
edits and undo/redo update the dependent code points. All 101 calculation cases
/9 targets, 2,303 engine tests (nine existing ignored), the editor lifecycle
case and the tiny-skia application model library check pass. Invalid UTF-16
surrogate import and native Office array captures remain unverified; these
checks do not establish complete text-function parity. Scalar expectations
follow Microsoft's [CLEAN documentation](https://support.microsoft.com/en-us/excel/functions/clean-function)
and [UNICODE documentation](https://support.microsoft.com/en-us/excel/functions/unicode-function).

CHAR and UNICHAR now map over arrays/ranges and expose their spill shape.
UNICHAR distinguishes partial surrogate code points (NA) from zero or values
outside the Unicode range (VALUE), as specified by Microsoft's
[UNICHAR documentation](https://support.microsoft.com/en-us/excel/functions/unichar-function).
Numeric bounds are checked before narrowing to u32. Tests cover both surrogate
halves and their adjacent valid code points, the highest Unicode code point,
large values, fractional input, source errors and one-element error spills.

All 103 calculation cases /9 targets and 2,303 engine tests (nine existing
ignored) pass. The expanded editor case verifies range constructors, dependent
UNICODE results, source edits/undo and cached/recalculated save/reopen; the
tiny-skia application model library check passes. CHAR retains the existing
Windows-1252 code page: Japanese/other platform code-page selection, fractional
boundary oracle captures and wider native Office array comparisons remain open.

EXACT now broadcasts scalar/array/range comparisons and exposes the result's
spill shape. Case and spaces remain significant, while existing scalar text
coercion and the numeric/numeric 15-digit comparison are retained. Tests cover
row/column broadcasting, empty cells/formula-empty strings, literal error text,
per-element errors and one-element error spills. Microsoft's
[EXACT documentation](https://support.microsoft.com/en-au/excel/functions/exact-function)
and [case-sensitive array comparison example](https://support.microsoft.com/en-us/excel/check-if-a-cell-contains-text-case-sensitive)
provide the case/array expectations.

The integration tests exposed two additional unary-operation gaps: minus and
percent now coerce arrays/ranges element by element, and parsing no longer
cancels pairs of minus operators. Such cancellation lost the numeric coercion
in --EXACT(...) and --{TRUE,FALSE,TRUE}, producing incorrect totals. Tests cover
text/Boolean coercion, invalid text, error shapes and SUM composition. The editor
case verifies comparison results and coerced totals after source edits and
cached/recalculated save/reopen.

All 106 calculation cases /9 targets, 2,303 engine tests (nine existing ignored),
the expanded editor case and the tiny-skia application model library check pass.
Broader numeric-text formatting, incompatible shape/error precedence and native
Office captures remain open; no complete comparison/arithmetic parity is claimed.

PROPER now maps over arrays/ranges with static spill-shape recognition. Its
existing letter/nonletter word-boundary rule is retained, including digits,
hyphens and apostrophes; whitespace is preserved. PROPER, LOWER and UPPER now
share the guarded scalar-value mapper, preserving blank/error values and
one-element arrays consistently. Tests cover case conversion, Japanese/Latin
mixed text, Boolean/numeric conversion, formula-empty/blank cells, literal error
text and EXACT/SUM composition. The boundary expectation follows Microsoft's
[text cleanup reference](https://support.microsoft.com/en-us/excel/top-ten-ways-to-clean-your-data).

All 108 calculation cases /9 targets and 2,303 engine tests (nine existing
ignored) pass. The editor case verifies PROPER/LOWER dependencies after source
edits and cached/recalculated save/reopen, including the distinction between
actual errors and case-converted error-looking text. The tiny-skia application
model library check passes. Locale-specific Unicode casing, combining-mark
word boundaries, case-expansion length limits and native Office captures still
need broader evidence; no complete international casing parity is claimed.

REPLACE now broadcasts all four scalar/array/range arguments and reports its
spill shape. Per-element numeric/text coercion and errors are preserved. Position
and length are validated before narrowing and clamped to the actual source
extent, with saturating addition; output above 32,767 UTF-16 units returns VALUE.
Tests cover mixed row/column arguments, insertion, removal, numeric inputs,
fractional counts, large positions/lengths, Unicode scalar positions and error
spills. Basic replacements and Version 2 surrogate handling follow Microsoft's
[REPLACE documentation](https://support.microsoft.com/en-gb/excel/functions/replace-function).

All 110 calculation cases /9 targets and 2,303 engine tests (nine existing
ignored) pass. The editor case verifies range replacement, dependent edits to
the start position, undo and cached/recalculated save/reopen, and the tiny-skia
application model library check passes. Workbook-selected legacy UTF-16 position
semantics, extreme-position native oracle captures and broader incompatible
array/error precedence remain open.

SUBSTITUTE now broadcasts its text, search, replacement and optional occurrence
arguments. The minimum arity is three, preventing the previous out-of-bounds
access for two arguments; an engine regression verifies an arity error rather
than a panic. Nth-occurrence and all-occurrence replacement use consistent
nonoverlapping matches. Bounded output assembly checks UTF-16 units before
appending, avoiding excessive allocation when many matches expand the text.
Tests cover repeated/overlapping candidates, case sensitivity, empty search,
large occurrence values, invalid occurrences, array error spills and cell limits.
The scalar occurrence rule follows Microsoft's
[SUBSTITUTE documentation](https://support.microsoft.com/en-us/excel/functions/substitute-function).

All 112 calculation cases /9 targets and 2,304 engine tests (nine existing
ignored) pass. The editor case verifies range results, edits to the selected
occurrence, undo and cached/recalculated save/reopen. The tiny-skia application
model library check passes. Explicit omitted-slot coercion, extreme occurrence
native captures and broader incompatible-shape/error precedence remain open.

FIND now broadcasts its search text, target text and optional start position,
with static spill-shape recognition. It preserves case-sensitive literal
matching, including literal wildcard characters, and returns per-element VALUE
for missing matches/invalid starts while retaining source errors. Start bounds
use Unicode scalar positions rather than UTF-8 bytes; the existing Version 2-
like position counting is retained. Tests cover row/column combinations,
numeric coercion, fractional/large starts, Japanese/emoji positions, error-first
spills and IFERROR/SUM. Scalar search/start rules follow Microsoft's
[FIND/SEARCH error guidance](https://support.microsoft.com/en-us/excel/how-to-correct-a-value-error-in-find-findb-and-search-searchb-functions).

All 114 calculation cases /9 targets, 2,304 engine tests (nine existing ignored),
the expanded editor lifecycle case and the tiny-skia application model library
check pass. Range search results and dependent totals follow source edits and
cached/recalculated save/reopen. Empty-target/empty-search boundaries, omitted
slots, legacy workbook position semantics and native Office captures remain open.

SEARCH now shares FIND's guarded argument broadcasting and spill-shape analysis.
Case-insensitive regex matching runs against the original text rather than a
lowercased copy, preventing position shifts when Unicode lowercasing expands a
character. Wildcards match line breaks as well as other characters; tilde-escaped
question marks, stars and tildes remain literal. Tests cover row/column arguments,
start arrays, missing matches, source errors, Japanese/emoji positions and the
original position after an expanding lowercase character. Scalar expectations
follow Microsoft's [SEARCH documentation](https://support.microsoft.com/en-us/excel/functions/search-function).

All 116 calculation cases /9 targets, 2,304 engine tests (nine existing ignored),
the expanded editor case and the tiny-skia application model library check pass.
The editor checks range searches, dependent totals and a newline wildcard after
source edits and cached/recalculated save/reopen. Locale-specific case folding,
large-array search performance, empty-target boundaries, legacy position modes
and native Office captures remain open; full Unicode search parity is unproven.

SEARCH reuses compiled wildcard patterns within a formula evaluation, with at
most eight cached patterns and no persistent cross-evaluation cache. Wildcard
pattern translation is separated from compilation so SEARCH compiles once with
its case/newline options instead of compiling twice. Other wildcard callers
retain their previous regex options and translation. Original-text character
offsets and errors are unchanged.

An identical regression test sums searches over 10,000 repeated text values and
a two-row/1,000-column combination. On this environment's debug build, its
reported test execution time changed from 21.00 seconds to 0.05 seconds,
excluding compilation; this is one local measurement, not a general performance
guarantee. Another case exercises more distinct patterns than the cache retains,
including missing matches and offsets after emoji.

All 118 calculation cases /9 targets, 2,304 engine tests (nine existing ignored),
the editor lifecycle case and the tiny-skia application model library check
pass. Larger production workbooks, diverse-pattern performance, locale-specific
case folding and the previously listed native Office/position boundaries still
need broader coverage.

NUMBERVALUE now broadcasts text, decimal-separator and group-separator arrays
and ranges with static spill-shape recognition. Its scalar parser is shared
unchanged: omitted separator slots use the current locale, only the first
separator character is used, empty text becomes zero, percent suffixes scale
the value and invalid text/separators yield per-element errors. Cases include
mixed European/English separators, Unicode separators, invalid/errored separator
elements, omitted slots, error-first spills and IFERROR/SUM. Scalar expectations
follow Microsoft's [NUMBERVALUE documentation](https://support.microsoft.com/en-us/excel/functions/numbervalue-function).

All 120 calculation cases /9 targets, 2,304 engine tests (nine existing ignored),
all three numeric-conversion editor cases and the tiny-skia application model
library check pass. The new editor case verifies per-row separator ranges,
blank/formula-empty sources, source errors, separator edits, undo/redo and
cached/recalculated save/reopen. Wider locale/Unicode-space parsing, extreme
number boundaries and native Office array captures remain open.

FIXED/DOLLAR no longer narrow the integer part or fractional digits to u64.
Their shared formatter rounds a 15-significant-digit decimal representation
directly, avoiding integer saturation, floating-point scaling overflow and
binary residual digits in high-precision output. Decimal counts truncate toward
zero, positive counts are bounded at 127 and very negative counts safely round
to zero. Tests include 1E20/1E308, tiny values, 127 places, carries such as 9.995,
negative decimal positions and out-of-range counts. The precision/decimal rules
follow Microsoft's [FIXED documentation](https://support.microsoft.com/en-us/excel/functions/fixed-function).

All 122 calculation cases /9 targets and 2,304 engine tests (nine existing
ignored) pass. All four numeric-conversion editor cases pass, including large
number output, decimal edits, undo/redo, invalid-count recovery and cached/
recalculated save/reopen. The tiny-skia application model library check passes.
Regional currency/grouping conventions, negative-zero and
extreme rounding boundaries still need broader native Excel evidence; DOLLAR
retains its existing dollar-sign/parenthesized-negative presentation.

FIXED and DOLLAR now broadcast numbers and decimal-place arguments; FIXED also
broadcasts no_commas. Static analysis preserves their spill shape. The shared
formatter retains scalar rounding, grouping, sign and currency behavior, with
per-element coercion and errors rather than discarding a whole array. Cases
cover row/column combinations, grouping flags, numeric text/Booleans, invalid
decimals/flags, empty argument slots and one-element error spills.

All 124 calculation cases /9 targets, 2,304 engine tests (nine existing ignored),
all four numeric-conversion editor cases and the tiny-skia application model
library check pass. The expanded editor case verifies per-row decimal ranges,
negative output, error elements, decimal edits/undo and cached/recalculated
save/reopen. Regional currency patterns, negative zero, scalar-error broadcasting
precedence and broader native Excel array captures remain open.

Shared text argument handling now retains ordinary scalar errors as values
during broadcasting instead of returning before determining the output shape.
An array combined with a scalar error therefore keeps its dimensions and
per-element errors, including spill references consumed by ROWS/COLUMNS and
IFERROR. Circular-reference errors still terminate evaluation; a dedicated engine
case verifies they do not become successful error arrays.

Cases cover LEFT, REPT, EXACT, FIXED, DOLLAR, REPLACE, SUBSTITUTE, NUMBERVALUE,
FIND and SEARCH, with two-dimensional all-error arrays. All 126 calculation
cases /9 targets, 2,305 engine tests (nine existing ignored), both text-conversion
editor cases and the tiny-skia application model library check pass. The new
editor case verifies transitions from values to errors, history and cached/
recalculated save/reopen while retaining the spill extent. Mixed-error precedence,
invalid shapes and native Excel captures still need broader verification.

TEXT now broadcasts both value and format arguments through the shared guarded
text mapper. Boolean results are text rather than Boolean values, and finite
numeric text is converted before formatting (for example, "12" with "000"
becomes "012"). Nonnumeric text remains text, including NaN/inf spellings;
blank cells format as zero. Per-element value/format errors and one-element
error spills are retained. Output above the UTF-16 cell limit returns VALUE.
The format-to-text expectation follows Microsoft's
[TEXT documentation](https://support.microsoft.com/en-us/excel/functions/text-function).

All 128 calculation cases /9 targets, 2,305 engine tests (nine existing ignored)
and all five numeric-conversion editor cases pass. The application model library
check with tiny-skia also passes. The new editor case verifies per-row formats,
numeric-text and Boolean types, format edits/errors, undo/redo and cached/
recalculated save/reopen. Text-section formatting, empty-string/date coercion,
full locale/epoch formatting and native Office array captures remain open.

CONCAT now shares TEXTJOIN's bounded output and sparse-range traversal with an
empty delimiter. It concatenates arrays in row order, retains scalar coercion
and source errors, and enforces one to 253 arguments plus the 32,767 UTF-16-unit
cell limit. Full-column/large blank ranges no longer require visiting every
unused cell. Source formulas are evaluated before a fresh sparse snapshot so
new spill values are included. The behavior follows Microsoft's
[CONCAT documentation](https://support.microsoft.com/en-gb/excel/functions/concat-function).

All 130 calculation cases /9 targets and 2,306 engine tests (nine existing
ignored) pass. Cases cover array order, errors, Japanese/emoji output boundaries,
253 arguments, huge blank rectangles and full-column inputs with a spill plus
the last worksheet row. All three text-conversion editor cases and the tiny-skia
application model library check pass. The new editor case verifies spill-size
edits, error insertion/recovery, undo/redo and cached/recalculated save/reopen.
Numeric text precision, mixed error/length precedence, broader application
workbooks and native Office captures remain open.

CONCATENATE now broadcasts scalar/array/range arguments and preserves the result
shape instead of rejecting array values. It joins corresponding elements rather
than flattening all arguments as CONCAT does, retaining coercion, per-element
errors and one-element error spills. Argument count is bounded at 255 and output
cannot exceed the 32,767 UTF-16-unit cell limit. The existing engine test now
checks a blocked spill, an unblocked spill and explicit @ intersection.

All 132 calculation cases /9 targets, 2,306 engine tests (nine existing ignored),
all three text-conversion editor cases and the tiny-skia application model
library check pass. The expanded editor case verifies spill-size changes,
dependent row counts, history and cached/recalculated save/reopen. Microsoft's
[CONCATENATE documentation](https://support.microsoft.com/en-gb/excel/functions/concatenate-function)
also states an 8,192-character total; that legacy limit's application versus
formula/cell limits still needs native Excel captures. No complete legacy/CSE,
long-output or incompatible-array parity is claimed by these checks.

ASC and DBCS/JIS now use the shared guarded text mapper for arrays/ranges.
ASC's static result follows its argument shape; DBCS/JIS already advertised an
array shape but their scalar-only evaluator is now aligned with it. Existing
Japanese width mappings are retained, including voiced/semi-voiced kana pairs.
The shared unary-transform path checks expanded output against the UTF-16 cell
limit; ASC can expand one voiced full-width kana into two half-width characters.
The conversion direction follows Microsoft's
[ASC documentation](https://support.microsoft.com/en-us/excel/functions/asc-function)
and [DBCS documentation](https://support.microsoft.com/en-us/excel/functions/dbcs-function).

All 134 calculation cases /9 targets, 2,306 engine tests (nine existing ignored),
all four text-conversion editor cases and the tiny-skia application model library
check pass. Cases include mixed Japanese/Latin/emoji text, numeric/Boolean
coercion, errors, one-element spills, expansion limits and DBCS/JIS equivalence.
The new editor case verifies kana edits, history and cached/recalculated
save/reopen, including blank/formula-empty sources and source errors. Locale
selection, decomposed sound marks, special punctuation and native Office width/
case-expansion boundaries remain open; no mapping-table completeness is claimed.

CODE now maps over arrays/ranges with static spill-shape recognition. It retains
the existing Windows-1252 lookup, scalar text coercion, VALUE for empty/unmapped
text and original source errors. Cases cover ASCII, euro/Latin characters,
numeric/Boolean coercion, CODE/CHAR array round trips, aggregation and a
one-element error spill. All 135 calculation cases /9 targets, 2,306 engine tests
(nine existing ignored), all four text-conversion editor cases and the tiny-skia
application model library check pass. The expanded editor case checks CODE range
results after source edits/undo and cached/recalculated save/reopen. Japanese/
other platform code pages and native Office array captures remain open.

LEFT/RIGHT/MID start and length elements now use normal numeric argument
coercion, accepting numeric text and Boolean values instead of rejecting both
types. Invalid text, nonfinite spellings, negative lengths and invalid MID starts
remain element errors. Existing fractional truncation, error propagation and
Unicode position counting are retained. The scalar extent rule follows Microsoft's
[LEFT documentation](https://support.microsoft.com/en-us/excel/functions/left-function).

All 136 calculation cases /9 targets, 2,306 engine tests (nine existing ignored),
all five text-conversion editor cases and the tiny-skia application model library
check pass. The new editor case exercises text/Boolean position ranges, invalid
elements, dependent lengths, source edits/undo/redo and cached/recalculated
save/reopen. Broader locale coercion, legacy Unicode position modes and native
Office boundary captures remain open.

TEXT's engine formatter now parses one to three fractional-second zero
placeholders after seconds, including elapsed seconds. It rounds once at the
requested precision before deriving fractions, seconds, minutes and the date,
so 59.9996 seconds and midnight carry consistently. Minute/month classification
now uses adjacent date/time fields: mm:ss is minutes, while a month in
hh:mm yyyy-mm-dd remains a month. Scalar format expectations follow Microsoft's
[custom number format guidelines](https://support.microsoft.com/en-us/excel/review-guidelines-for-customizing-a-number-format).

All 137 calculation cases /9 targets, 2,306 engine tests (nine existing ignored),
all six numeric-conversion editor cases and the tiny-skia application model
library check pass. Both 1900/1904 systems cover tenths/hundredths/milliseconds,
minute/day carries, 24-hour-plus elapsed times and mixed calendar/time formats.
The new editor case verifies value/format edits, undo/redo and cached/recalculated
save/reopen for both systems. Locale decimal separators, mixed precision,
unusual literal placement, negative/extreme times and wider native Office
captures remain open; precision beyond three fractional places is unsupported.

TEXT's formatter now accepts A/P and a/p and retains the requested AM/PM token
spelling/case instead of always emitting uppercase AM/PM. The lexer/parser carry
both period labels to rendering; the selected label uses the rounded clock,
including fractional-second carries at noon and midnight. Expectations follow
Microsoft's [custom format period codes](https://support.microsoft.com/en-us/excel/review-guidelines-for-customizing-a-number-format).

All 138 calculation cases /9 targets, 2,306 engine tests (nine existing ignored),
all six numeric-conversion editor cases and the tiny-skia application model
library check pass. Cases cover all four conventional tokens, array values,
midnight/noon boundaries and rounded transitions. The expanded editor case
verifies lowercase/short labels after edits/history and cached/recalculated
save/reopen in both date systems. Localized period labels, unusual mixed-case
tokens and broader native Office captures remain open.

General scientific formatting now lets decimal formatting carry into the
exponent, so a rounded coefficient of ten is normalized (for example, 10E+11
becomes 1E+12). It no longer divides by a separately calculated power of ten.
Existing notation thresholds and six-significant-digit scientific precision
are retained pending native evidence for TEXT/General's width-dependent rules.
The width-dependent General display model is described in Microsoft's
[number format remarks](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.numberingformat).

All 139 calculation cases /9 targets, 2,306 engine tests (nine existing ignored),
all seven numeric-conversion editor cases and the tiny-skia application model
library check pass. Cases include positive/negative carries, very large values,
ordinary exponential output, small values and zero. The new editor case verifies
source edits, history and cached/recalculated save/reopen. Full General precision,
width/notation thresholds and extreme/subnormal native Excel captures remain open.

Multi-section TEXT number formats now select numeric sections by their parsed
conditions instead of always routing by positive/negative/zero. The first matching
condition wins; an unconditional numeric section supplies the fallback. Conditional
sections retain the signed input, including negative values routed to the second
section. Ordinary sign-based formats continue to format the second section using
the magnitude. This follows Microsoft's documented
[conditional section example](https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/excel/format-cells-settings).

The selected calculation package passes all 140 cases /9 targets, the engine
passes 2,306 tests (nine existing ignored), and all eight numeric-conversion
editor cases pass. The new cases cover strict/inclusive
thresholds, equality, positive/negative/zero boundaries, array output and ordinary
negative-section regression. A numeric-conversion editor case covers source and
format edits, undo/redo, and cached/recalculated save/reopen. Single-section
conditions retain their legacy behavior pending native evidence. Conditions on
date/General/fraction sections, the not-equal operator, invalid combinations and
the exact TEXT behavior when no condition/fallback matches remain open; the
current multi-section implementation returns VALUE for the last case. This is
not a claim of complete custom-format parity.

The follow-up condition-format pass adds the `<>` comparison to the formatter
lexer and section selector, including negative/exponential thresholds. Date/time
and General parse parts now retain condition metadata instead of dropping it.
They participate in the same multi-section condition selection as numeric parts;
for example, `[<1]hh:mm;0.00` renders 0.5 as `12:00` and 2 as `2.00`, while
`[>=1]General;0.00` renders 0.5 as `0.50`. Calculation regressions cover both
workbook date systems, matching/fallback branches and array output. The selected
calculation package passes all 142 cases /9 targets; all 2,307 engine tests
(nine existing ignored) and all eight numeric-conversion editor cases pass.
Formatting and whitespace checks also pass. The editor regression also
checks not-equal source edits/history and saved cached/recalculated results, then
switches among conditional clock/numeric and General/numeric sections.

This resolves the previous checkpoint's not-equal and date/General condition gaps
for the exercised multi-section cases. Single-section conditions, conditional
fractions, invalid combinations, no-match native TEXT behavior, and broader native
Office captures remain open. General color metadata and precise General display
rules are separate remaining concerns.

Conditional fraction formats now use the same comparison/fallback selector as
token-based numeric/date/General formats. The fraction path recognizes condition
and color prefixes, preserves the signed value for explicit conditions, and
delegates to the selected scalar section when a mixed format chooses a decimal,
date or General section. Ordinary second-section negative-number magnitude
handling is retained. Its raw section splitter now respects backslash escapes,
underscore spacing and fill-character escapes when scanning semicolons.

Calculation regressions cover variable/fixed denominators, mixed decimal/fraction
branches, date/General fallback branches, negative conditional fractions, ordinary
negative sections, escaped semicolons and array output. All 143 selected
calculation cases /9 targets, 2,308 engine tests (nine existing ignored), and all
nine numeric-conversion editor cases pass; formatting/whitespace checks pass.
An engine regression checks both the selected fraction text and its color; an
editor regression exercises source/format edits, history and saved cached versus
recalculated values. Supported simple fraction patterns remain limited: richer
literal/currency/locale affixes, parentheses, extreme values, denominator precision
and broader native Excel captures still need work. Single-section conditions and
no-match native TEXT behavior also remain open.

Number-format parsing now retains empty trailing sections, including an entirely
empty format. Previously `TEXT(0,"0;;")` returned `0`, because the zero section
was dropped and the format became two sections. It now returns an empty string;
`0;` likewise suppresses negative inputs. The shared raw section scanner keeps
quoted, backslash-escaped, spacing and fill semicolons out of the section count.
Explicit empty conditional fallback sections and mixed fraction/empty formats
also work, while a fifth section is rejected rather than disappearing at EOF.
This follows Microsoft's documented
[empty sections for hiding values](https://support.microsoft.com/en-us/excel/number-format-codes-in-excel-for-mac).

The regression was observed failing before the fix and passes afterward. The
selected calculation package passes all 144 cases /9 targets, 2,309 engine tests
(nine existing ignored), and all ten numeric-conversion editor cases. Formatting
and whitespace checks pass. The new cases cover
empty formats, leading/trailing/zero sections, General/conditional/fraction
combinations, invalid section counts and array output. Parser-level regressions
check section counts around quoted/escaped/spacing/fill semicolons, and the editor
case checks hidden/visible source edits, format changes, undo/redo and saving an
empty computed string. This does not resolve the remaining text-section rendering,
single-condition or native no-match semantics gaps.

Numeric TEXT formatting no longer drops the negative sign at the exact display
unit or for negative values that round to a nonzero display below that unit.
The observed regression `TEXT(-1,"0")` returned `1` before the fix; it now returns
`-1`. Sign emission uses the displayed integer/fraction digits, preserving the
existing unsigned-zero policy pending native captures of values rounded to zero.
It also works when the format starts with a decimal point: `.00` is parsed as a
numeric decimal pattern, and its omitted integer placeholder does not discard
actual nonzero integer digits (for example, -12.34 still renders as `-12.34`).

The selected calculation package passes all 145 cases /9 targets, all 2,309
engine tests (nine existing ignored), and all eleven numeric-conversion editor
cases; formatting/whitespace checks pass. New cases cover
integer/decimal precision boundaries, rounding into a nonzero display, leading
decimal patterns, percent scaling, explicit conditions, ordinary negative
parenthesis sections, true zero and array output. The new editor case covers
source/format edits, history and cached/recalculated save/reopen. Native rounded
negative-zero policy, optional decimal placeholders and broader custom-format
precision/rounding captures remain open.

Decimal placeholders now share decimal-separator emission, including `?` when it
outputs a padding space. The failing regression `TEXT(1,"0.??")` previously
returned `1` followed by two spaces with no decimal separator; it now returns
`1.` followed by two spaces. Optional `#` digits still suppress insignificant
zeros, mandatory `0` digits remain, and the existing explicit decimal point for
an all-optional decimal pattern is retained. Microsoft's
[custom-format guidelines](https://support.microsoft.com/en-us/excel/review-guidelines-for-customizing-a-number-format)
describe the separate digit, spacing and decimal-point roles.

All 146 selected calculation cases /9 targets, 2,310 engine tests (nine existing
ignored), and all twelve numeric-conversion editor cases pass; formatting and
whitespace checks pass. Regressions cover whole,
partial and full fractional values, `0.##`, `0.??`, `0.0?`, percent scaling,
negative values and arrays. An engine regression covers English/German/French
decimal symbols, and an editor case exercises source/format edits, history and
cached/recalculated save/reopen with significant trailing padding spaces. Mixed
mandatory digits after optional placeholders, literal placement, full precision
and broader native captures remain open.

Custom scientific TEXT formats now round the coefficient and normalize its
exponent together, instead of independently scaling by a logarithm-derived
power of ten. The observed `TEXT(999.9,"0.00E+00")` regression returned `9.00E+02`
before the fix and now returns `1.00E+03`. This also corrects the exponent sign
for coefficients whose exponent is zero and avoids duplicate digits when an
exponent needs more digits than its placeholders (for example, E+308). The
scientific path no longer performs an unrelated initial fixed-point precision
rounding. Microsoft's [TEXT documentation](https://support.microsoft.com/en-au/excel/functions/text-function)
describes the normalized scientific coefficient.

All 147 selected calculation cases /9 targets, 2,311 engine tests (nine existing
ignored), and all thirteen numeric-conversion editor cases pass; formatting and
whitespace checks pass. Cases cover positive and
negative carries, small-value carries, zero, exponent zero, E+ versus E- format
codes, very large/small values and arrays. Engine locale regressions exercise
English/German/French separators; an editor case checks source/format edits,
undo/redo and cached/recalculated save/reopen at a three-digit exponent. Decimal
half-way rounding (including the existing ignored scientific rounding case),
multi-integer-placeholder engineering notation, subnormal/native captures and
the full custom-format precision rules remain open.

TEXT now supports exponent grouping by multiple integer placeholders, including
the built-in engineering pattern `##0.0E+0`. It retains the coefficient's required
decimal places within each exponent group, uses Euclidean grouping for negative
exponents, and carries into the next group when the rounded coefficient reaches
the group's limit. The observed `TEXT(12200000,"#0.0E+0")` failure returned
`1.2E+7`; it now returns `12.2E+6`, matching Microsoft's
[custom-format example](https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/excel/format-cells-settings).
The existing layout formatter already supports the same built-in engineering
pattern; this pass brings the exercised TEXT cases into agreement with it.

All 148 selected calculation cases /9 targets, 2,312 engine tests (nine existing
ignored), all fourteen numeric-conversion editor cases and the targeted built-in
layout engineering regression pass; formatting/whitespace checks pass. Cases include two/three
integer placeholders, large and small inputs, negative exponents, carries within
and across groups, negative values, zero and arrays. Engine locale regressions
cover English/German/French decimal symbols, and the editor case exercises source
and format edits, undo/redo, and cached/recalculated save/reopen. Half-way rounding,
very large placeholder counts, unusual mandatory/optional-placeholder combinations
and broader native engineering-format captures remain open.

TEXT's fixed/scientific/engineering numeric formats now share the decimal-digit
rounding primitive previously used by FIXED/DOLLAR. Midpoints round away from
zero using the existing 15-significant-digit decimal policy, instead of Rust's
binary half-even formatting. Fixed-point integer/fraction digits come from the
same rounded decimal string, so fractional carries update the integer part and
large finite outputs are not narrowed through an integer or reparsed float.
Examples now include 1.25 → 1.3, -1.475 → -1.48, 9.95 → 10.0 and
0.9996 → 1.000. The scientific midpoint regression for 12345 under `0.###E+00`
now passes and is enabled rather than ignored. Microsoft's
[rounding examples](https://support.microsoft.com/en-us/excel/functions/round-function)
document the decimal midpoint convention.

All 149 selected calculation cases /9 targets, 2,313 engine tests (eight remaining
ignored), and all fifteen numeric-conversion editor cases pass; formatting and
whitespace checks pass. Cases cover positive/negative midpoints, fixed/scientific/
engineering formats, integer carries, arrays, consistency with FIXED, and an exact
large fixed-point output at 1E308. The editor case verifies source/format edits,
history and cached/recalculated save/reopen. This resolves the exercised midpoint
gap from the preceding checkpoints; exhaustive native precision/rounding captures,
rounded negative-zero semantics, unusual placeholder combinations and scaling at
the finite-range limits remain open.

The ignored-format audit exposed missing thousands separators when a format has
more leading `#` placeholders than actual integer digits. Group positions now
use the actual digit position rather than advancing through suppressed leading
placeholders: `TEXT(1234.5,"###,##0.00")` returns `1,234.50` instead of `1234.50`.
Regression cases also exercise mandatory zero padding, negative values, arrays,
and thousands/millions scaling. The original failing calculation is retained as
a passing regression.

Four formatter tests are enabled again: simple fractions, fixed eighths, grouped
leading hashes, and German millions scaling. The German test's format was
corrected from one scaling comma to two, consistent with its name and expected
million-scaled values; a single comma correctly scales by a thousand. See
Microsoft's [grouping and scaling guidelines](https://support.microsoft.com/en-us/excel/review-guidelines-for-customizing-a-number-format).
The ignored two-digit-denominator fraction test still disagrees on trailing
padding, so it remains pending native evidence rather than changing whitespace
behavior merely to satisfy the old expectation.

All 150 selected calculation cases /9 targets, 2,317 engine tests (four
remaining ignored), and all sixteen numeric-conversion editor cases pass;
formatting/whitespace checks pass. The editor regression covers source/format edits,
history, zero padding and cached/recalculated save/reopen with grouped/scaled
TEXT output. Rich padding/grouping combinations, the remaining fraction padding
disagreement and broader native custom-format captures remain open.

Numeric decimal-point tokens now render at their own positions instead of
waiting for the next fractional digit. The observed `TEXT(1.25,"0.")` failure
returned `1`; it now returns `1.`. This also keeps quoted/escaped literals after
the separator rather than moving the separator past them, while preserving the
actual integer digits in leading-decimal formats such as `.00`. Optional-digit
and padding behavior from the earlier checkpoint is retained. Microsoft's
[custom-format guidelines](https://support.microsoft.com/en-us/excel/review-guidelines-for-customizing-a-number-format)
describe the explicit decimal-point and literal-character roles.

All 151 selected calculation cases /9 targets, 2,318 engine tests (four
remaining ignored), and all seventeen numeric-conversion editor cases pass;
formatting/whitespace checks pass. Cases include terminal decimal points, rounding and
negative signs, interleaved quoted/escaped literals, percentage scaling,
leading-decimal formats, quoted literal periods, padding and arrays. Engine
regressions cover English/German/French decimal symbols; the editor case covers
source/format edits, history and cached/recalculated save/reopen. Leading decimal
points separated from the first placeholder by a literal still need parser-context
work, alongside richer literal/padding patterns and broader native captures.

Leading-decimal classification now looks through quoted/escaped literals and
non-consuming formatting directives to find a numeric placeholder in the same
section. The observed `TEXT(1.23,".""x""00")` failure returned `.x01`; it now
returns `1.x23`. The lookahead inspects a lexer clone, keeping the literals in
the parser's actual token stream, and stops at date/time fields, section boundaries,
EOF or invalid tokens. Quoted digits remain literals, and a later section cannot
turn an earlier literal period into a numeric decimal point.

All 152 selected calculation cases /9 targets, 2,318 engine tests (four remaining
ignored), and all eighteen numeric-conversion editor cases pass; formatting and
whitespace checks pass. Regressions exercise quoted
and escaped literals, escaped semicolons, interleaved spaces, optional padding,
negative values, arrays and literal-only sections. Date/time prefixes and
fractional seconds are checked in both workbook date systems. Existing locale
regressions now include this leading-decimal context, and the editor case checks
source/format edits, undo/redo, numeric and time-prefix results, and cached versus
recalculated save/reopen. This resolves the preceding checkpoint's exercised
leading-decimal parser-context gap. Exotic directive placement, mixed date/numeric
patterns and broader native format captures remain open.

DATEVALUE and TIMEVALUE now convert ranges/arrays element by element using the
shared guarded scalar mapper. DATEVALUE previously returned NIMPL for arrays;
TIMEVALUE also rejected array/range inputs. Both retain array shape (including a
one-element array), localize bad-text/source errors to their elements, and expose
their output shape to static ROWS/COLUMNS/TYPE consumers. DATEVALUE's numeric,
Boolean and empty scalar inputs now follow its date-text parsing path and return
VALUE instead of passing through a serial/floor or 0/1. The existing text date/time
parsers and workbook date-system context provide the converted values. Primary
references are Microsoft's [DATEVALUE](https://support.microsoft.com/en-us/excel/functions/datevalue-function)
and [TIMEVALUE](https://support.microsoft.com/en-us/excel/functions/timevalue-function)
contracts.

All 153 selected calculation cases /9 targets and 2,318 engine tests (four
remaining ignored) pass. Eighteen numeric-conversion editor cases and the new
date/time-conversion editor case pass, as does the tiny-skia application model
library check; formatting/whitespace checks pass. The new editor case exercises
both date systems, range conversion with an error-first spill, blank and source
errors, scalar shape queries, source edits, undo/redo, and cached/recalculated
save/reopen before and after repairing the first input. The error-cache checkpoint
was rerun after its additional assertions were added. Locale/calendar parsing,
omitted-year rules, unusual/non-text coercions, normalization edge cases and
broader native dynamic-array captures remain open.

TIME now broadcasts hour/minute/second inputs across ranges and arrays, including
singleton-axis expansion and per-element missing-position/coercion/source errors.
Scalar errors retain the array shape supplied by other components. A shared
typed-argument mapper normalizes guarded ranges, preserves one-element arrays,
checks the broadcast product before allocation and stops circular operands; the
unary mapper uses the same range normalization. TIME's static shape inference
now follows its input shapes. Components with nonfinite values or integral values
above 32767 return NUM, matching Microsoft's documented
[TIME input limit](https://support.microsoft.com/en-us/excel/functions/time-function).
Two existing stress expectations that accepted million/i32-max components were
corrected to NUM; valid-bound stress values remain numeric.

All 154 selected calculation cases /9 targets, 2,319 engine tests (four remaining
ignored), and 25 selected editor cases across numeric/date-time/text conversion
pass; formatting/whitespace checks pass. Regressions cover outer-axis broadcasting,
mismatched dimensions, error-first and scalar-error arrays, Boolean/numeric-text
coercion, one-element shape and inclusive component limits. An engine case checks
the existing one-million-element output resource guard. The new editor case covers
range expansion, source changes, undo/redo, and cached/recalculated save/reopen
with a NUM-first spill and valid following rows. Existing floor/negative-component
compensation behavior remains pending native evidence; negative fractions, lower
component limits, exceptional numeric text and broader Office captures remain open.

DATE now uses the shared typed-argument broadcaster introduced for TIME. Its
previous dedicated array branch returned early for scalar errors/coercion
failures, collapsing an array-shaped call such as `DATE(#N/A,{1,2},1)` into a
single error. It now preserves the two-element result and applies scalar source
or coercion errors to each element. Existing numeric coercion, floor policy,
month/day normalization and workbook date-system construction are retained;
nonfinite components and out-of-range years return NUM. The constructor uses
Microsoft's documented [DATE year and component rules](https://support.microsoft.com/en-us/excel/functions/date-function).
Range and broadcast-product guards now also cover DATE's allocation path.

All 155 selected calculation cases /9 targets, 2,319 engine tests (four remaining
ignored), and 26 selected editor cases across numeric/date-time/text conversion
pass; formatting/whitespace checks pass. Regressions exercise scalar errors,
invalid text, outer-axis expansion, missing positions, year bounds, heterogeneous
arrays and one-element arrays in both date systems. The engine allocation guard
case now includes DATE as well as TIME. The editor case checks error-first spill
metadata, row/column queries, source corrections, undo/redo, growth from one row
to two, and cached/recalculated save/reopen with DIV/0 in the first row and valid
dates in the second. The one-million-element resource ceiling remains an engine
policy rather than an Excel compatibility claim; native negative/fractional
component details, exceptional text coercion and broader captures remain open.

EDATE and EOMONTH now use the shared typed-argument broadcaster and checked
calendar-month arithmetic. The new pre-fix calculation batch reproduced a
`NaiveDate + Months` panic from very large month counts. These inputs now return
NUM without unwinding, including offsets beyond Chrono's representable dates.
Both lower and upper result serial bounds are checked; EDATE no longer returns
dates beyond the workbook's maximum. Month counts retain truncation toward zero,
EDATE retains day-clamping, and EOMONTH retains its last-day calculation and
existing Boolean-month coercion policy. EDATE shape inference now follows its
arguments. Primary contracts are Microsoft's [EDATE](https://support.microsoft.com/en-us/excel/functions/edate-function)
and [EOMONTH](https://support.microsoft.com/en-us/excel/functions/eomonth-function)
documentation.

All 156 selected calculation cases /9 targets, 2,319 engine tests (four remaining
ignored), and 27 selected editor cases across numeric/date-time/text conversion
pass; formatting/whitespace checks pass. Regressions cover scalar/array/outer-axis
inputs, source/coercion errors, one-element shape, leap-year clamping, fractional
negative offsets, the year-9999 edge and huge positive/negative month counts in
both date systems. The engine product-size guard case includes both functions.
The new editor case verifies edited month/start-date ranges, history, a NUM-first
spill alongside valid dates and NA elements, and cached/recalculated save/reopen.
Serial zero/fictitious-1900-leap-day behavior, uncommon Boolean/text coercions,
early-date boundaries and broader native captures remain open.

DAYS now broadcasts its end/start arguments and distinguishes numeric serials
from date text. Numeric serials retain fractional time during subtraction;
date text follows the existing DATEVALUE parser and loses its time component.
The observed `DAYS(2.75,1.25)` regression returned 1 before the fix and now returns
1.5. Unparseable text returns VALUE, invalid numeric date ranges return NUM, and
numeric text without a date representation is no longer treated as a serial.
The final valid date can retain its time fraction. This follows Microsoft's
[DAYS numeric-versus-text contract](https://support.microsoft.com/en-us/excel/functions/days-function).
Static shape inference now follows the argument shapes as well.

All 157 selected calculation cases /9 targets, 2,319 engine tests (four remaining
ignored), and 28 selected editor cases across numeric/date-time/text conversion
pass; formatting/whitespace checks pass. Regressions cover positive/negative
fractional differences, text time removal, mixed numeric/text arguments, the
documented 42-day example, outer-axis arrays, scalar and element errors,
one-element arrays and final-date bounds in both date systems. The engine
broadcast-product guard includes DAYS. The editor case checks range edits,
history, an error-first spill with a valid fractional result in a later row,
and cached/recalculated save/reopen. Locale/date-text parsing inherits the
DATEVALUE parser's remaining limits; uncommon coercions, extreme boundaries
and broader native captures remain open.

2026-11-07 date-text month-name checkpoint: DATEVALUE/DAYS no longer truncate
uppercase OCT/OCTOBER at T or discard the date after its first space. Clock
suffixes and ISO T separators are removed separately from date tokens. Tests
cover day-first and month-first English names, comma-separated year, uppercase
hyphenated names, clock suffixes, ISO timestamps and invalid year text.
Engine: 2,320 passed / 4 existing ignored (`tmp/xlsx-date-spaces-engine.log`).
Calc: 157 passed across 9 targets (`tmp/xlsx-date-spaces-calc.log`). Regional
date order, localized month names, omitted years and native Excel captures
remain open; this checkpoint does not claim locale parity.

2026-11-07 regional date-text checkpoint: DATEVALUE, DAYS, DATEDIF text arguments
and conditional-format date bounds now pass the model locale to the shared
date parser. Numeric dates follow the regional short-date day/month order;
ISO year-first dates retain month/day order. Dot-separated dates and localized
full/abbreviated month names are recognized, including Unicode case matching.
Regression cases cover English versus German/French ambiguous dates, German
dot dates, uppercase MÄRZ/FÉVRIER, ISO timestamps, and invalid swapped numeric
months. Two leap/month-end DATEDIF fixtures now use ISO dates instead of relying
on day-first input under the English locale; their arithmetic assertions remain.
Reference: https://support.microsoft.com/en-us/excel/how-to-correct-a-value-error-in-the-datevalue-function
(regional settings govern date-text interpretation).
Engine: 2,321 passed / 4 existing ignored (`tmp/xlsx-date-locale-engine.log`).
Calc: 157 passed across 9 targets (`tmp/xlsx-date-locale-calc.log`). Selected
editor conversion/history/save-reopen suites: 28 passed
(`tmp/xlsx-date-locale-editor.log`). Formatting and both diff checks pass.
Omitted years, complete locale-specific punctuation and acceptance rules,
additional regional date orders and native captures remain unverified.

2026-11-07 DATEVALUE epoch-bound checkpoint: date-text conversion now rejects
serials outside the active workbook date system instead of returning negative
serials for pre-epoch dates. The shared parser retains 1900-02-29 as serial 60
only in the 1900 system. Calc regressions cover 1899-12-31, year 0000, year
10000, the final 9999-12-31 date, 1900/1904 starting dates, the fictional leap
day, mixed valid/error arrays and DAYS text coercion in both date systems.
The editor regression checks an error-first DATEVALUE range spill, boundary
correction and undo, then cached and recalculated save/reopen for both epochs.
Reference: https://support.microsoft.com/en-us/excel/functions/datevalue-function
(the default-system date-text range is 1900-01-01 through 9999-12-31).
Engine: 2,321 passed / 4 existing ignored (`tmp/xlsx-date-bounds-engine.log`).
Calc: 158 passed across 9 targets (`tmp/xlsx-date-bounds-calc.log`). Selected
editor conversion/history/save-reopen suites: 29 passed
(`tmp/xlsx-date-bounds-editor.log`). Rust formatting and both diff checks pass.
Native confirmation of 1904 zero-date acceptance and broader localized/date
input edge cases remains open; these tests establish the shared epoch policy.

2026-11-07 engine-patch replay checkpoint: the previously saved `jade.patch`
failed replay comparison against the current implementation. Regenerated it
from the SHA-256-pinned official ironcalc_base 0.8.3 archive, including the
recent formatter, text, broadcasting and date changes. Fixed the replay
script's Windows portability: canonical UTF-8/LF comparison excludes checkout
CRLF differences, explicit UTF-8/LF output prevents platform conversion, and
standard no-final-newline markers preserve meaningful EOF differences. Binary
upstream files remain byte-identical. Three script regression tests pass,
including actual patch application for EOF changes, creation and deletion.
Independent non-update replay passes for 500 source/build files, with all
480 upstream inventory hashes checked. Patch SHA-256:
`c54f34def635fce4cf3faa2f03a4ab46e213fb113d58b4e7b937da422e1e8774`.
Evidence: `tmp/xlsx-engine-patch-before.log`,
`tmp/xlsx-engine-patch-update.log`, `tmp/xlsx-engine-patch-replay.log`,
`tmp/xlsx-engine-patch-tests.log`. Both repository diff checks pass. No engine
source behavior changed in this checkpoint; prior runtime test results retain
their stated scope, and native Excel comparisons remain incomplete.

2026-11-07 localized TIMEVALUE date-prefix checkpoint: if the existing time
patterns do not match, the parser now validates a date prefix with the shared
locale-aware DATEVALUE parser and parses the following clock separately.
This supports space-separated full month names, uppercase localized month
names and German dot dates, with AM/PM clock values. Invalid calendar dates,
invalid clock hours and arbitrary prefix text still produce VALUE errors.
The engine case covers English/German/French prefixes and the fictional 1900
leap day. Calc checks scalar/array results, local errors and one-element array
shape in both epochs. The editor's date/time range lifecycle case now includes
month-name prefixes through correction, history and cached/recalculated reopen.
Reference: https://support.microsoft.com/en-us/excel/functions/timevalue-function
(a time may include date information, which is omitted from the result).
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-localized-time-engine.log`).
Calc: 159 passed across 9 targets (`tmp/xlsx-localized-time-calc.log`). Selected
editor conversion suites: 29 passed (`tmp/xlsx-localized-time-editor.log`).
Canonical patch regeneration/replay checks all 500 source/build files and 480
upstream inventory entries (`tmp/xlsx-localized-time-patch.log`); patch SHA-256
is `ea11b2ba9a461c268e7268b19159dc4b9c28a6b03817b53ce14603108cddcacf`.
Fractional-second acceptance, complete regional time syntax and native captures
remain unverified. Existing time normalization and primary parse paths remain.

2026-11-07 WEEKNUM broadcasting checkpoint: runtime evaluation and static
shape analysis now lift both the date and optional return-type arguments over
ranges/arrays. The shared typed broadcaster preserves singleton axes,
one-element arrays, element errors, mismatched-dimension NA results and the
existing allocation limit. Nonfinite numeric operands and unsupported numeric
return types produce NUM errors; the previous VALUE results for return types
0, 3 and -1 were corrected against the documented return-type contract.
Reference: https://support.microsoft.com/en-us/excel/functions/weeknum-function
Calc regressions cover outer-axis date/type combinations, ISO week-year
boundaries, invalid types, scalar/element errors, mismatched dimensions and the
documented 2012-03-09 examples in both workbook epochs. The editor case checks
range formulas, error-first spills, correction, undo/redo and cached/recalculated
save/reopen. The engine allocation-product regression now includes WEEKNUM.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-weeknum-engine.log`).
Calc: 160 passed across 9 targets (`tmp/xlsx-weeknum-calc.log`). Selected editor
conversion suites: 30 passed (`tmp/xlsx-weeknum-editor.log`). Canonical patch
replay verifies 500 source/build files and 480 upstream hashes
(`tmp/xlsx-weeknum-patch.log`); SHA-256:
`c32b33212f193332108d93807e3f9b0c04961cd984f05dba4e6bdfafeb684451`.
Early-1900 week arithmetic, exceptional coercions and broader native captures
remain open. This does not establish full date-function or Excel parity.

2026-11-07 WEEKDAY broadcasting checkpoint: both the date and optional
numbering-mode argument now use the shared typed broadcaster, preserving
outer axes, scalar/element errors, missing-position NA results and one-element
array shape. Nonfinite operands return NUM. Unsupported numeric mode 0 now
returns NUM instead of VALUE, as specified for unsupported return types.
Reference: https://support.microsoft.com/en-us/excel/functions/weekday-function
Calc covers all supported numbering modes, Monday/Sunday boundaries, scalar
and element errors, mismatched axes and the documented 2008-02-14 example in
both workbook epochs. The combined WEEKNUM/WEEKDAY editor regression checks
range modes through edits, undo/redo, an error-first spill and valid later
element, cached reopen and recalculated reopen. The engine allocation-product
test includes WEEKDAY. Existing early-1900 weekday arithmetic is unchanged
and still requires native reference captures.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-weekday-engine.log`). Calc:
161 passed across 9 targets (`tmp/xlsx-weekday-calc.log`). Selected editor
conversion suites: 30 passed (`tmp/xlsx-weekday-editor.log`). Formatting and
both diff checks pass. Canonical patch replay verifies 500 source/build files
and 480 upstream hashes (`tmp/xlsx-weekday-patch.log`); patch SHA-256:
`ec8241d96ba68f9af23c0540a8e427ed13576a5780638613eb654269e3dc4b90`.

2026-11-07 DAYS360 broadcasting checkpoint: both date arguments and the
optional method now use the shared typed broadcaster. Static shape analysis
matches runtime array results. Date components retain the existing serial
conversion and US/European month-end arithmetic; boolean method coercion uses
the existing typed cast. Nonfinite date values return NUM. Calc cases cover
outer-axis leap-February/month-end dates and methods, logical/numeric/text
method coercion, scalar/element errors, mismatched rows and one-element arrays
in both epochs, plus documented scalar examples.
Reference: https://support.microsoft.com/en-us/excel/functions/days360-function
The editor case checks a method range, error-first spill with a valid later
element, correction, undo/redo, cached reopen and recalculated reopen. The
engine broadcast-product allocation guard now includes DAYS360.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-days360-engine.log`). Calc:
162 passed across 9 targets (`tmp/xlsx-days360-calc.log`). Selected editor
conversion suites: 31 passed (`tmp/xlsx-days360-editor.log`). Rust formatting
and both diff checks pass. Canonical patch replay verifies 500 source/build
files and 480 upstream hashes (`tmp/xlsx-days360-patch.log`); patch SHA-256:
`c29484287e2089c4be7ad82b8af9ecf32d089966fe014a3d1f37465657f895a3`.
Broader month-end/native reference captures and exceptional coercion cases
remain open; this checkpoint establishes array support, not full parity.

2026-11-07 DATEDIF broadcasting checkpoint: start date, end date and unit now
use the shared typed broadcaster, with matching static shape analysis. Scalar
date conversion was separated from expression evaluation so array elements
retain the same locale-aware date-text, boolean and empty-value paths; nonfinite
numeric dates now return NUM. Existing date-difference algorithms and invalid
unit handling remain unchanged. Calc covers all six units over outer axes,
invalid text and source errors, reversed date intervals, mismatched rows,
one-element arrays and the documented YD example in both workbook epochs.
Reference: https://support.microsoft.com/en-au/excel/functions/datedif-function
The editor regression checks date/unit ranges, an error-first spill, correction,
undo/redo, a unit change and cached/recalculated save/reopen. The engine's
broadcast allocation-product guard includes DATEDIF.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-datedif-engine.log`). Calc:
163 passed across 9 targets (`tmp/xlsx-datedif-calc.log`). Selected editor
conversion suites: 32 passed (`tmp/xlsx-datedif-editor.log`). Formatting and
both diff checks pass. Canonical patch replay verifies 500 source/build files
and 480 upstream hashes (`tmp/xlsx-datedif-patch.log`); patch SHA-256:
`eac0964fdfb8121655573a1bc73e172f95ef2c9737ba31c25c119aec64b2864c`.
Native MD/YD leap/month-end cases, invalid-unit error semantics and exceptional
coercions remain open; array regressions do not establish complete parity.

2026-11-07 YEARFRAC broadcasting checkpoint: start/end dates and optional
basis now use the shared typed broadcaster, with matching static shape
analysis. Basis 4 evaluates DAYS360 on each already-converted scalar serial,
instead of re-evaluating the original operand expressions. Nonfinite date or
basis numbers return NUM. Existing basis arithmetic otherwise remains.
Reference: https://support.microsoft.com/en-us/excel/yearfrac-function
Calc checks all five bases against day-count ratios over outer axes, invalid
bases, scalar/element errors, mismatched rows, one-element shape and the
documented Actual/Actual example in both epochs. Editor checks a basis range,
error-first spill with a valid later element, edits, undo/redo and cached and
recalculated reopen. The engine allocation-product guard includes YEARFRAC.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-yearfrac-engine.log`). Calc:
164 passed across 9 targets (`tmp/xlsx-yearfrac-calc.log`). Selected editor
conversion suites: 33 passed (`tmp/xlsx-yearfrac-editor.log`). Formatting and
both diff checks pass. Canonical patch replay verifies 500 source/build files
and 480 upstream hashes (`tmp/xlsx-yearfrac-patch.log`); patch SHA-256:
`e3af63fe2d128b64ae0e782ae68d703d4b16f2fef7b058f654614baeea5c49ba`.
Reversed Actual/Actual intervals, early serials, February basis-0 native
captures and exceptional coercions remain open. A ROUND(array, digits) probe
returned N/IMPL during this validation; that separate array-support gap remains.
YEARFRAC numerical verification uses explicit day-count ratios without ROUND.

2026-11-07 rounding-array checkpoint: ROUND, ROUNDUP and ROUNDDOWN now lift
both number and digits arguments through the shared typed broadcaster. Their
existing 15-significant-digit scalar rounding and digit truncation remain.
Calc covers positive/negative values, outer axes, negative digit positions,
midpoints, coercions, scalar/element errors, mismatched rows, one-element
shape and ROUND applied to YEARFRAC's five-basis array. This closes the
ROUND(array, digits) N/IMPL gap recorded in the preceding checkpoint.
References: https://support.microsoft.com/en-us/excel/functions/round-function
and the corresponding ROUNDUP/ROUNDDOWN documentation (away/toward zero).
The editor case checks all three range formulas, an error-first spill with a
valid later result, number/digit edits, undo/redo and cached/recalculated reopen.
All three functions are included in the broadcast-product allocation guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-round-array-engine.log`).
Calc: 165 passed across 9 targets (`tmp/xlsx-round-array-calc.log`). Selected
editor conversion suites: 34 passed (`tmp/xlsx-round-array-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-round-array-patch.log`);
patch SHA-256: `c2ec72a715fa5ffe36b811fee503c2fc867a9b6140e60c0cb0b4f0b20b95aaa1`.
Extreme-scale overflow/underflow, exceptional numeric operands and native
boundary captures remain open; scalar arithmetic was not redesigned here.

2026-11-07 decimal-rounding scale checkpoint: ROUND/ROUNDUP/ROUNDDOWN now
share bounded 15-significant-digit decimal rounding rather than multiplying
by `10^digits`. The decimal coefficient is rounded according to the requested
nearest/away/toward-zero policy and converted once to a numeric result. Large
positive digit counts return the preserved significant value; coarse positions
return zero or the next quantum as appropriate. Nonfinite operands or an
unrepresentable rounded result return NUM instead of an internal NaN/infinity.
Calc verifies finite 1E300 values, rounded large and tiny magnitudes, very large
positive/negative digit counts, zero, overflow and positive/negative midpoints.
The three-function editor lifecycle regression now includes 1E300 as a later
valid spill element, checked after cached and recalculated save/reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-round-scale-engine.log`).
Calc: 166 passed across 9 targets (`tmp/xlsx-round-scale-calc.log`). Selected
editor conversion suites: 34 passed (`tmp/xlsx-round-scale-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-round-scale-patch.log`);
patch SHA-256: `6e1529e1ee724cbb968832c182017fcf8ffbf4f7b8ed8eb64b907b754be3dc8f`.
The regression expectations exercise decimal rounding semantics and finite
result preservation; native Excel captures at extreme magnitudes, denormal
boundaries and exceptional coercion cases remain open.

2026-11-07 TRUNC checkpoint: number and optional digits now use the shared
typed broadcaster and bounded decimal toward-zero helper. Removed the old
shortcut that returned the input unchanged whenever digits exceeded +/-15;
coarse negative positions now truncate instead of bypassing the calculation.
Finite large/tiny values no longer depend on an overflowing decimal scale.
Reference: https://support.microsoft.com/en-us/excel/functions/trunc-function
Calc covers optional/default digits, positive/negative numbers and positions,
outer axes, source/coercion/scalar errors, missing rows, one-element shape,
digits +/-1E9, 1E300 and a tiny decimal magnitude. The rounding editor lifecycle
case now includes a TRUNC range and verifies error-first spills and valid
negative/large later elements after history and cached/recalculated reopen.
The engine allocation-product guard includes TRUNC.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-trunc-engine.log`). Calc:
167 passed across 9 targets (`tmp/xlsx-trunc-calc.log`). Selected editor
conversion suites: 34 passed (`tmp/xlsx-trunc-editor.log`). Formatting and
both diff checks pass. Canonical patch replay verifies 500 source/build files
and 480 upstream hashes (`tmp/xlsx-trunc-patch.log`); patch SHA-256:
`48e02b3b41a13f6ddad0083c20f527551bfacba63eb7fb294aa2358fb3f390e6`.
Native high-precision/denormal boundary captures and exceptional coercion cases
remain open; broader Excel parity is not established by these checks.

2026-11-07 MOD/QUOTIENT broadcasting checkpoint: both operands now use the
shared typed broadcaster. MOD retains its divisor-sign remainder formula;
QUOTIENT retains toward-zero division and its existing rejection of booleans.
Zero denominators remain DIV errors local to each element. Nonfinite inputs
or arithmetic results now return NUM instead of a nonfinite numeric result.
References: https://support.microsoft.com/en-us/excel/functions/mod-function
and https://support.microsoft.com/en-gb/excel/functions/quotient-function
Calc checks outer axes, all sign combinations, zero-divisor columns, source
errors, scalar errors, mismatched rows, numeric-text/boolean coercion,
one-element shape, documented scalar examples and quotient overflow. Editor
checks both operand ranges through denominator correction, undo/redo and an
error-first spill with valid later elements after cached/recalculated reopen.
Both functions are covered by the engine allocation-product guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-mod-quotient-engine.log`).
Calc: 168 passed across 9 targets (`tmp/xlsx-mod-quotient-calc.log`). Selected
editor conversion suites: 35 passed (`tmp/xlsx-mod-quotient-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-mod-quotient-patch.log`);
patch SHA-256: `92a3817a2fe3c8018ea430fcf598cc1d452103b25190bf720fec47c3f3ce6299`.
Floating-point remainder precision, very large ratio native captures and
exceptional coercion cases remain open; scalar remainder arithmetic is retained.

2026-11-07 MROUND broadcasting checkpoint: number and multiple now use the
shared typed broadcaster, preserving singleton axes, missing-position NA,
one-element shape and local errors. Existing explicit-empty/boolean coercion,
zero-multiple and sign checks remain. Nonfinite arithmetic results return NUM
instead of an infinite numeric result. Existing decimal midpoint arithmetic
is unchanged; Excel documents limitations for decimal-multiple midpoints.
Reference: https://support.microsoft.com/en-us/excel/functions/mround-function
Calc covers positive/negative outer axes, sign-error columns, zero multiples,
source/scalar errors, mismatched rows, numeric-text/boolean inputs, one-element
shape, documented examples and the finite-result guard. Editor checks operand
ranges, correction, undo/redo and an error-first spill with a valid negative
later element after cached/recalculated save/reopen. The engine allocation-
product guard includes MROUND.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-mround-engine.log`). Calc:
169 passed across 9 targets (`tmp/xlsx-mround-calc.log`). Selected editor
conversion suites: 36 passed (`tmp/xlsx-mround-editor.log`). Formatting and
both diff checks pass. Canonical patch replay verifies 500 source/build files
and 480 upstream hashes (`tmp/xlsx-mround-patch.log`); patch SHA-256:
`18275a6192bc395436a5d6c988e48c55fb88b5f2de0a6a9070a0c1d3aba8c64d`.
Native decimal-midpoint, extreme-ratio and exceptional-coercion captures remain
open; these array checks do not establish full MROUND or Excel parity.

2026-11-07 FLOOR/CEILING broadcasting checkpoint: both functions now share
typed argument broadcasting. FLOOR gains number/significance arrays; CEILING's
previous bespoke numeric-array path no longer collapses a scalar operand error
before broadcasting. Existing legacy sign rules, distinct zero-significance
behavior and 15-digit ratio precision remain. FLOOR now guards nonfinite
results as CEILING already did. Removed an unused precision import.
References: https://support.microsoft.com/en-us/excel/functions/ceiling-function
and https://support.microsoft.com/en-au/excel/functions/floor-function
Calc checks outer axes, signs, zero significance, scalar/source errors,
mismatched rows, one-element shape and documented decimal examples. The
combined multiple-rounding editor case now verifies FLOOR and CEILING range
formulas through correction/history and cached/recalculated save/reopen, with
an error-first spill and a valid negative later element. Both functions are
covered by the engine allocation-product guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-floor-ceiling-engine.log`).
Calc: 170 passed across 9 targets (`tmp/xlsx-floor-ceiling-calc.log`). Selected
editor conversion suites: 36 passed (`tmp/xlsx-floor-ceiling-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-floor-ceiling-patch.log`);
patch SHA-256: `546766c4f7ea7b50f604f2c7f3ce449a47c47e8f576edfcde1687d4f6a31c7ec`.
Extreme-ratio precision, exceptional coercions and broader native captures
remain open. MATH/PRECISE/ISO variants are separate implementations to audit.

2026-11-07 modern multiple-rounding checkpoint: CEILING.MATH, FLOOR.MATH,
CEILING.PRECISE, FLOOR.PRECISE and ISO.CEILING now share typed broadcasting.
Default significance 1, absolute significance, MATH-only optional mode and
negative-number direction reversal remain. Arity restrictions stay distinct
for MATH versus PRECISE/ISO. Nonfinite operands/results produce NUM.
References: https://support.microsoft.com/en-us/excel/functions/ceiling-math-function
https://support.microsoft.com/en-us/excel/functions/floor-math-function
https://support.microsoft.com/en-us/excel/functions/floor-precise-function
https://support.microsoft.com/en-us/excel/functions/iso-ceiling-function
Calc checks number/mode outer axes, positive/negative/zero significance,
omitted arguments, mismatched mode rows, one-element shape and scalar error
broadcasting for all five functions. Editor checks all five range formulas,
mode edits, undo/redo and error-first spills with valid negative later elements
after cached and recalculated save/reopen. The engine allocation-product
guard includes all five functions.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-modern-round-engine.log`).
Calc: 171 passed across 9 targets (`tmp/xlsx-modern-round-calc.log`). Selected
editor conversion suites: 37 passed (`tmp/xlsx-modern-round-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-modern-round-patch.log`);
patch SHA-256: `9e47936afc545ed217f966ca6034d3834ce98f72cfe6320a2b26806b901bbb87`.
Extreme-ratio precision, exceptional coercions and broader native captures
remain open; existing ratio arithmetic is retained.

2026-11-07 POWER/LOG broadcasting checkpoint: both functions now use the
shared typed broadcaster for all arguments; omitted LOG base remains 10.
Domain errors stay local to array elements. POWER overflow now returns NUM
instead of DIV, with an explicit zero-to-negative-power DIV branch. Existing
zero-to-zero and negative fractional-power errors remain. Nonfinite operands
and LOG results are guarded rather than stored as numeric NaN/infinity.
References: https://support.microsoft.com/en-us/excel/functions/power-function
https://support.microsoft.com/en-us/excel/functions/log-function
https://support.microsoft.com/en-us/excel/how-to-correct-a-num-error
Calc covers outer axes, signs, integer/reciprocal/fractional exponents, LOG
base domains/default, scalar errors, missing rows, one-element shape, overflow
and zero-power boundaries. Editor checks both range formulas through source
correction, exponent edits, undo/redo and error-first spills with valid later
elements after cached/recalculated save/reopen. Both functions are included
in the engine allocation-product guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-power-log-engine.log`).
Calc: 172 passed across 9 targets (`tmp/xlsx-power-log-calc.log`). Selected
editor conversion suites: 38 passed (`tmp/xlsx-power-log-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-power-log-patch.log`);
patch SHA-256: `4d7e8ca4833889f464c55a7d627de81367de77d386136ff78de947f9589c038c`.
Native floating-point/underflow boundaries, exceptional coercions and complete
POWER-versus-exponent-operator comparisons remain open.

2026-11-07 combination checkpoint: COMBIN/COMBINA now use shared typed
broadcasting and symmetric binomial evaluation. The reduced iteration count
is min(k,n-k) for COMBIN and min(k,n-1) for COMBINA. Special constant/identity
cases finish immediately; nonfinite arguments and overflowing products return
NUM instead of overflowing or attempting an enormous usize-sized loop.
For a symmetric binomial, 1,024 reduced selections imply a lower bound of
2^1024, outside finite f64 range, so such cases can be rejected directly.
References: https://support.microsoft.com/en-us/excel/functions/combin-function
and https://support.microsoft.com/en-us/excel/functions/combina-function
Calc covers outer axes, errors, mismatched rows, one-element shape, positive
fractional arguments, documented examples, zero selections, large equal
COMBIN arguments, COMBINA with one/two item types and huge selections, and
overflowing combinations. Editor checks both range formulas through source
correction, selection edits, history and error-first spills with valid later
elements after cached/recalculated save/reopen. Both functions are included
in the engine allocation-product guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-combinations-engine.log`).
Calc: 173 passed across 9 targets (`tmp/xlsx-combinations-calc.log`). Selected
editor conversion suites: 39 passed (`tmp/xlsx-combinations-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-combinations-patch.log`);
patch SHA-256: `10c2cfeb1543f004c565bced2314001e198329c4b27081c39cf4b9d7918589fe`.
Large finite integer precision, exceptional coercions and native argument-bound
captures remain open; existing zero-item conventions are retained.

2026-11-07 combination integer-precision checkpoint: floating product probes
showed 373 inexact results among tested n<=100 cases whose exact integer result
fits 2^53, including C(11,5)=461.99999999999994 and a four-unit error for
C(58,23). For exactly representable integer operands up to 2^53, the shared
combination evaluator now retains a u128 recurrence. GCD reduction cancels the
denominator before checked multiplication, avoiding intermediate overflow;
only results beyond u128 capacity transition to floating arithmetic. COMBINA
also forms its integer total before conversion, and k=1 returns n directly.
Calc regressions check small counterexamples, symmetry, C(58,23), the equivalent
repeated-selection count, C(100,50), the k=1 boundary, arrays and a downstream
MOD(result,1) check. Editor save/reopen checks exact 462 and 3003 later spill
elements alongside an error-first range and history changes.
Engine: 2,322 passed / 4 existing ignored
(`tmp/xlsx-combination-integers-engine.log`). Calc: 174 passed across 9 targets
(`tmp/xlsx-combination-integers-calc.log`). Selected editor conversion suites:
39 passed (`tmp/xlsx-combination-integers-editor.log`). Formatting and both
diff checks pass. Canonical patch replay verifies 500 source/build files and
480 upstream hashes (`tmp/xlsx-combination-integers-patch.log`); patch SHA-256:
`4736819d98a9319672296a337041fafa7f84be80ada415df4cca0156a5f13767`.
Reference values are mathematical integer counts; complete native Excel
high-magnitude rounding, exceptional coercions and float-fallback boundaries
still require captures and are not established by these regressions.

2026-11-07 factorial bounds checkpoint: FACT/FACTDOUBLE now use the shared
typed unary mapper and validate finite, nonnegative truncated operands before
their product loops. FACT stops at 170 and FACTDOUBLE at 300, the last finite
integer results for these products; larger values return NUM immediately.
This removes unbounded loops for huge inputs and avoids numeric infinity.
FACTDOUBLE's prior negative-one extension was removed in accordance with the
documented negative-input NUM contract.
References: https://support.microsoft.com/en-us/excel/functions/fact-function
and https://support.microsoft.com/en-us/excel/functions/factdouble-function
Calc covers documented examples, zero, positive fractional truncation, negative
inputs, overflow and 1E100 inputs, source errors, one-element shape, coercions
and finite maximum boundaries. Editor checks both range formulas through
overflow correction, source edits, undo/redo and error-first spills with valid
later elements after cached/recalculated save/reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-factorials-engine.log`).
Calc: 175 passed across 9 targets (`tmp/xlsx-factorials-calc.log`). Selected
editor conversion suites: 40 passed (`tmp/xlsx-factorials-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-factorials-patch.log`);
patch SHA-256: `ebb9fe4b3129e28deff86e2c71f935c0995c9dff066d3adef249ffe584e8b19e`.
Native boundary/negative-input captures, large finite factorial precision and
exceptional coercions remain open; full Excel parity is not established.

2026-11-07 unary finite-result checkpoint: the single-number function macro
now delegates to the existing shared numeric unary mapper and rejects
nonfinite inputs/results with NUM. This covers 32 registered implementations
including EXP, trigonometric/inverse and hyperbolic functions, avoiding numeric
NaN/infinity from overflow or undefined real results. Existing explicit error
codes and numeric coercions remain; the mapper retains the legacy full-column
evaluation path, verified by the existing SUM(SIN(A:A)) engine regression.
References: https://support.microsoft.com/en-us/excel/how-to-correct-a-num-error
https://support.microsoft.com/en-us/excel/functions/acoth-function
https://support.microsoft.com/en-us/excel/functions/exp-function
Calc checks EXP/SINH/COSH overflow, ASIN/ACOS/ACOSH/ATANH/ACOTH domains,
source errors, one-element shape and coercions. Editor checks EXP/ASIN ranges
through correction, source edits, undo/redo and error-first spills with valid
later elements after cached and recalculated save/reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-unary-finite-engine.log`).
Calc: 176 passed across 9 targets (`tmp/xlsx-unary-finite-calc.log`). Selected
editor conversion suites: 41 passed (`tmp/xlsx-unary-finite-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-unary-finite-patch.log`);
patch SHA-256: `f6cbe8500374b8443f534214762bd7c01a16949c1a10e6fe8df0d32758a62068`.
Function-specific numeric caps, extreme precision/underflow, complete full-axis
semantics and broader native captures remain open; scalar formulas are retained.

2026-11-07 ATAN2 checkpoint: both coordinates now use the shared typed
broadcaster. Origin DIV errors stay local to elements, nonfinite coordinates
return NUM, and an angle of -pi is normalized to pi to match the documented
principal interval, including signed-zero y on the negative x-axis.
Reference: https://support.microsoft.com/en-us/excel/functions/atan2-function
Calc covers quadrant/axis outer axes through DEGREES, origin/source/scalar
errors, mismatched rows, one-element shape, coercions, signed-zero boundary
and large finite coordinates. Editor checks coordinate ranges through origin
correction, axis changes, undo/redo and an error-first spill with a valid later
element after cached/recalculated save/reopen. ATAN2 is included in the engine
broadcast allocation-product guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-atan2-engine.log`). Calc:
177 passed across 9 targets (`tmp/xlsx-atan2-calc.log`). Selected editor
conversion suites: 42 passed (`tmp/xlsx-atan2-editor.log`). Formatting and
both diff checks pass. Canonical patch replay verifies 500 source/build files
and 480 upstream hashes (`tmp/xlsx-atan2-patch.log`); patch SHA-256:
`c085cd9269b54f95049a5a0094f9c7796aed3c4e7c52d6c5f520bae4aefd7ded`.
Native extreme-coordinate/near-axis precision and exceptional coercion
captures remain open; complete trigonometric parity is not established.

2026-11-07 permutation checkpoint: PERMUT/PERMUTATIONA now use typed
broadcasting and matching static shape analysis. Removed saturating i64
conversion of large numeric operands. PERMUT uses a bounded descending product,
with checked u128 multiplication for exactly representable integer operands
and a finite-result guard. A selection count above 170 implies a result at
least 171!, so it returns NUM without an enormous loop. Zero selections no
longer compute an overflowing signed-integer n+1 endpoint. PERMUTATIONA guards
power overflow and rejects zero items with positive selections; PERMUT rejects
zero item counts according to its documented constraints.
References: https://support.microsoft.com/en-gb/excel/functions/permut-function
and https://support.microsoft.com/en-us/excel/functions/permutationa-function
Calc covers outer axes, errors, missing rows, one-element shape, truncation,
documented examples, zero boundaries, large finite n/short selections, huge
selection counts and power overflow. Editor checks both operand ranges through
source correction, selection edits, history and error-first spills with valid
later elements after cached/recalculated save/reopen. Both functions are
included in the engine broadcast-product allocation guard.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-permutations-engine.log`).
Calc: 178 passed across 9 targets (`tmp/xlsx-permutations-calc.log`). Selected
editor conversion suites: 43 passed (`tmp/xlsx-permutations-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-permutations-patch.log`);
patch SHA-256: `b30fb2b68f469d441e4ccc195ab932730bbff75fd2b39f852d0d5f5201b5f08a`.
Native high-magnitude precision, exceptional coercions and complete boundary
captures remain open; repeated zero-to-zero retains the prior value of one.

2026-11-07 LARGE/SMALL checkpoint: ranks now lift over arrays/ranges while
the data vector is evaluated and sorted once per formula. Static output shape
comes from the rank argument only. Numeric single-cell data references are
accepted, data-array/source errors keep their original codes and broadcast over
rank results, and invalid/nonfinite ranks are guarded before indexing. Array
text/boolean data remain excluded from the numeric dataset; existing direct
scalar coercion and rank truncation/boolean policies are retained.
Reference: https://support.microsoft.com/en-us/excel/functions/large-function
Calc checks rank rows/columns, duplicate data values, invalid bounds, source
and scalar errors, numeric-only datasets, empty numeric data, one-element
shape, huge ranks and single-cell numeric references. Editor checks both rank
range formulas through rank correction, data changes, history and error-first
spills with valid later elements after cached/recalculated save/reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-large-small-engine.log`).
Calc: 179 passed across 9 targets (`tmp/xlsx-large-small-calc.log`). Selected
editor conversion suites: 44 passed (`tmp/xlsx-large-small-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-large-small-patch.log`);
patch SHA-256: `2c3d5aff0bbab340b52d665c88e8df4be48a2b0097f715468634ba38b1cf2a28`.
Native fractional/boolean-rank coercion, complete full-axis performance and
broader native captures remain open; rank-policy parity is not established.

2026-11-07 percentile/quartile checkpoint: PERCENTILE.INC/EXC and QUARTILE.INC/
EXC now lift only their parameter argument over ranges/arrays while collecting
and sorting the data once. Legacy PERCENTILE/QUARTILE aliases share this path;
all six static output shapes follow the parameter. Existing inclusive/exclusive
interpolation and quartile conversion policies remain. Shared data collection
now preserves source/array error codes, rejects 3D ranges and nonfinite numbers;
parameter/result finiteness is checked before indexing or storing results.
References: https://support.microsoft.com/en-us/excel/functions/percentile-inc-function
https://support.microsoft.com/en-us/excel/functions/percentile-exc-function
https://support.microsoft.com/en-us/excel/functions/quartile-inc-function
https://support.microsoft.com/en-us/excel/functions/quartile-exc-function
Calc covers inclusive/exclusive endpoints/interpolation, parameter rows/columns,
invalid bounds, aliases, data errors, one-element shape and the documented
30th-percentile example. Editor checks all four modern range formulas through
parameter correction, edits, undo/redo and error-first spills with valid later
elements after cached and recalculated save/reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-percentile-quartile-engine.log`).
Calc: 180 passed across 9 targets (`tmp/xlsx-percentile-quartile-calc.log`).
Selected editor conversion suites: 45 passed (`tmp/xlsx-percentile-quartile-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-percentile-quartile-patch.log`);
patch SHA-256: `3e0ed4f3ea83ecc3846e096b230ef8980eee9d9f2e4aab5100ce782184e2e420`.
Extreme interpolation precision/overflow, exceptional coercions, full-axis
performance and broader native captures remain open.

2026-11-07 percentile interpolation checkpoint: a regression probe reproduced
PERCENTILE.INC({-9E307,9E307},0) returning NUM instead of its finite minimum
(`tmp/xlsx-percentile-interpolation-before.log`). Shared percentile/quartile
interpolation now returns endpoints directly and uses weighted endpoints for
opposite signs, avoiding overflow of high-low. Same-sign interpolation retains
the previous expression. Exclusive virtual ranks are clamped to valid endpoint
positions after domain validation to absorb boundary rounding noise.
Calc covers inclusive endpoints, exclusive endpoint positions, midpoints,
quartiles and legacy aliases, an array of normalized interpolation values and
a same-sign high-magnitude interval. Editor checks large opposite-sign data,
source correction/undo and all four modern functions after cached/recalculated
save/reopen, retaining an invalid first parameter and valid later result.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-percentile-interpolation-engine.log`).
Calc: 181 passed across 9 targets (`tmp/xlsx-percentile-interpolation-calc.log`).
Selected editor conversion suites: 45 passed (`tmp/xlsx-percentile-interpolation-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-percentile-interpolation-patch.log`);
patch SHA-256: `065ba3eea0d376cc53c3672aa25cf351bed9dcbfb62008ce26fef32cd631eed2`.
These expectations follow finite interpolation and endpoint semantics; native
Excel extreme-magnitude/rounding captures and exceptional coercions remain open.

2026-11-07 PERCENTRANK checkpoint: INC/EXC and the legacy alias now broadcast
value/significance operands while collecting and sorting data once. Static
output shape excludes the data vector. Shared bounded decimal truncation
replaces `10^significance` scaling, so huge valid significance does not create
NaN. Opposite-sign inverse interpolation normalizes endpoints before forming
their difference. Empty numeric data now returns the documented NUM error
instead of NA; existing duplicate positioning and singleton-INC policy remain.
References: https://support.microsoft.com/en-us/excel/functions/percentrank-inc-function
https://support.microsoft.com/en-us/excel/functions/percentrank-exc-function
Calc checks value/significance outer axes, default precision, aliases,
duplicates, out-of-range values, invalid significance, source errors, empty
data, one-element shape, huge significance and opposite-sign extreme data.
Editor checks both range formulas through value/precision edits, undo/redo and
error-first spills with valid later elements after cached/recalculated reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-percentrank-engine.log`).
Calc: 182 passed across 9 targets (`tmp/xlsx-percentrank-calc.log`). Selected
editor conversion suites: 46 passed (`tmp/xlsx-percentrank-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-percentrank-patch.log`);
patch SHA-256: `7193f5436078000ced5cd3ee1df44f64fe812bcb43237106ba7ab7614160485e`.
Native high-significance rounding, duplicate/singleton boundaries, exceptional
coercions and broader captures remain open; precision follows the shared
15-significant-digit decimal policy rather than a complete native oracle.


2026-11-07 RANK checkpoint: RANK.EQ, RANK.AVG and legacy RANK now broadcast
number/order operands while sorting the data vector once. Static shape excludes
the data vector; singleton numeric references are accepted and source errors
retain their original error kind. Binary searches locate equal-value bounds for
both ordinary and averaged ties. Existing exceptional coercion policies remain.
Calc checks ascending/descending outer axes, ties, absent numbers, legacy alias,
array/source errors, singleton shape, ignored array text/booleans and numeric
cell references. Editor checks range inputs through corrections, order edits,
undo/redo and error-first spills after cached/recalculated save/reopen.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-rank-engine.log`).
Calc: 183 passed across 9 targets (`tmp/xlsx-rank-calc.log`). Selected editor
conversion suites: 47 passed (`tmp/xlsx-rank-editor.log`). Formatting and both
diff checks pass. Canonical patch replay verifies 500 source/build files and
480 upstream hashes (`tmp/xlsx-rank-patch.log`); patch SHA-256:
`e5cd784c6cbf321635f1cba35f9b6f10a906b73f9fa49aae54cc5af7d9a03b63`.
Native Excel captures, exceptional coercions, error precedence and broader
workbook/UI compatibility remain open; these checks do not establish full parity.


2026-11-07 YEARFRAC reversed-date checkpoint: chronological endpoints are now
normalized before day-count calculations. Previously, reversed multi-year
Actual/Actual intervals could divide by zero and export NUM, while US month-end
adjustments could differ by argument order. The regression failed before the
change (`tmp/xlsx-yearfrac-reversed-before.log`) and now passes. This follows
Apache POI's common endpoint-normalization policy, rather than claiming a new
native Excel capture:
https://github.com/apache/poi/blob/trunk/poi/src/main/java/org/apache/poi/ss/formula/atp/YearFracCalculator.java
https://support.microsoft.com/en-us/excel/functions/yearfrac-function
Calc covers reversed one-/multi-year intervals, February-to-March month-ends,
all five bases, array axes, invalid basis and both date systems. Editor covers
reversed range formulas with an error-first spill through date edits, undo/redo
and cached/recalculated save/reopen in both date systems.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-yearfrac-reversed-engine.log`).
Calc: 184 passed across 9 targets (`tmp/xlsx-yearfrac-reversed-calc.log`). Selected
editor conversion suites: 48 passed (`tmp/xlsx-yearfrac-reversed-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-yearfrac-reversed-patch.log`);
patch SHA-256: `d91a08efd41e3d5fecf321a122ba34bad51d0fe1d421e5fd589a8d4d0afa9b7a`.
Native Excel reversed-date/month-end captures, early-1900 semantics and broader
workbook/UI compatibility remain open.


2026-11-07 time-component checkpoint: HOUR, MINUTE and SECOND now evaluate
range/array arguments element-wise and report matching static array shapes.
The prior scalar-only path returned NIMPL for array inputs, reproduced before
this change (`tmp/xlsx-time-components-before.log`). Typed mapping preserves
individual source errors, singleton arrays and numeric coercion; negative or
non-finite numeric times return NUM. Existing fractional-second extraction and
rounding expressions are retained, pending broader native boundary captures.
Calc covers matrices, whole-day offsets, mixed errors, Boolean/numeric-text
coercions, singleton shape and components of TIME(13,14,15). Editor checks range
inputs in both date systems through source corrections, undo/redo and cached/
recalculated save/reopen, including an error-first spill with valid later values.
Engine: 2,322 passed / 4 existing ignored (`tmp/xlsx-time-components-engine.log`).
Calc: 185 passed across 9 targets (`tmp/xlsx-time-components-calc.log`). Selected
editor conversion suites: 49 passed (`tmp/xlsx-time-components-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-time-components-patch.log`);
patch SHA-256: `6c1a80a38f3001640fb5143a7a122a3de4c0af54ee9471960f285d336fee2245`.
Native Excel precision/coercion boundaries, time-string acceptance and broader
workbook/UI compatibility remain open; these selected suites do not prove parity.

2026-11-07 WORKDAY bounds checkpoint: WORKDAY and WORKDAY.INTL replace the
unbounded per-day walk with binary search within the active date epoch.
Whole-week counting plus at most six residual weekdays excludes working
holidays exactly once. Huge offsets and unavailable result dates return NUM
instead of overflowing chrono or walking beyond the supported epoch. A
pre-change regression reproduced a date-overflow panic
(`tmp/xlsx-workday-bounds-before.log`). Scalar coercion policies remain;
array operands are still an open task.
Reference: https://support.microsoft.com/en-gb/excel/functions/workday-intl-function
A daily-count oracle checks all 127 usable weekend masks, seven start weekdays
and offsets -30..30 with holidays. Calc covers maximum dates, huge signed
offsets, fractional truncation, duplicate holidays, no-weekend masks and a
100,000-business-day interval in both date systems. Editor covers offset
correction, undo/redo and cached/recalculated save/reopen.
Engine: 2,323 passed / 4 existing ignored (`tmp/xlsx-workday-bounds-engine.log`).
Calc: 186 passed across 9 targets (`tmp/xlsx-workday-bounds-calc.log`). Selected
editor conversion suites: 50 passed (`tmp/xlsx-workday-bounds-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-workday-bounds-patch.log`);
patch SHA-256: `4e14fe1f810e6aaebed88cc72dc0378659adee61ffd27016b0942ed2b6fd6994`.
Early-1900 weekday semantics, native Excel captures, exceptional coercions,
array inputs and broader workbook/UI compatibility remain open.

2026-11-07 WORKDAY array checkpoint: WORKDAY/WORKDAY.INTL now broadcast start,
offset and optional weekend operands using typed element-wise evaluation.
Holiday lists are collected once and remain vectors in argument signatures;
they do not influence static result shape. Weekend scalar parsing is shared
with existing NETWORKDAYS.INTL logic. Original source errors propagate per
element, with circular holiday dependencies remaining fatal.
The pre-change array regression returned NIMPL (`tmp/xlsx-workday-array-before.log`).
Calc covers outer axes, duplicate/shared holidays, numeric/string weekend
patterns, invalid/all-weekend patterns, mixed errors and singleton shape in
both date systems. Editor covers range inputs and holiday/weekend edits,
undo/redo, and error-first matrix spills after cached/recalculated reopen.
Both functions also retain the shared oversized-result allocation guard.
Engine: 2,323 passed / 4 existing ignored (`tmp/xlsx-workday-array-engine.log`).
Calc: 187 passed across 9 targets (`tmp/xlsx-workday-array-calc.log`). Selected
editor conversion suites: 51 passed (`tmp/xlsx-workday-array-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-workday-array-patch.log`);
patch SHA-256: `2569184f98e04c50f1840613c44d34d68e1acb92befa0336a3d58934e0aaee3b`.
Native captures, exceptional coercion/error precedence, early-1900 weekdays,
NETWORKDAYS array support and broader workbook/UI compatibility remain open.

2026-11-07 NETWORKDAYS array checkpoint: NETWORKDAYS/NETWORKDAYS.INTL now
broadcast endpoint/weekend operands and retain holiday lists as shared vectors.
Static result shape excludes holidays; typed mapping preserves individual
errors and singleton arrays. Both endpoints are validated before counting.
Full-week counting replaces the per-day scan, with at most six residual days
and a bounded early-1900 prefix preserving the existing serial-60 mapping.
Duplicate holidays are removed and only working holidays within the inclusive
interval are subtracted; reversed dates retain the negative sign.
The pre-change array regression returned NIMPL (`tmp/xlsx-networkdays-array-before.log`).
An independent daily-serial oracle checks all 128 weekend masks, signed spans,
early-1900 boundaries and both date systems. Calc covers outer axes, shared
holidays, numeric/string and all-weekend masks, reversed endpoints, invalid
dates/weekends, source errors and singleton shape. Editor covers range inputs,
holiday/weekend edits, undo/redo and error-first spills after cached/recalculated
save/reopen. Both functions retain the oversized-result allocation guard.
Engine: 2,324 passed / 4 existing ignored (`tmp/xlsx-networkdays-array-engine.log`).
Calc: 188 passed across 9 targets (`tmp/xlsx-networkdays-array-calc.log`). Selected
editor conversion suites: 52 passed (`tmp/xlsx-networkdays-array-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-networkdays-array-patch.log`);
patch SHA-256: `9dcfc00afd883921504bc815083af6421b402400c8e662ff8efe9235e0430b5f`.
Native Excel captures, early-1900 weekday parity, exceptional coercions/error
precedence and broader workbook/UI compatibility remain open.

2026-11-07 empty-holiday checkpoint: the shared holiday collector accepts
explicitly omitted arguments, empty cell references and empty array elements
as empty lists. Previously, an explicitly omitted optional holiday argument
returned VALUE (`tmp/xlsx-holiday-empty-before.log`). A shared numeric date
validator rejects non-finite values before integer conversion, including NaN
that could otherwise become serial zero in the 1904 epoch.
Reference: https://support.microsoft.com/en-us/excel/functions/workday-function
Calc covers explicit omission and empty references across all four business-day
functions, including array operands and both date systems. Engine checks
non-finite holiday numbers and an array with an empty element. Editor checks
empty references becoming actual holidays, undo/redo and cached/recalculated
save/reopen, including formulas with explicitly omitted trailing arguments.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-holiday-empty-engine.log`).
Calc: 189 passed across 9 targets (`tmp/xlsx-holiday-empty-calc.log`). Selected
editor conversion suites: 53 passed (`tmp/xlsx-holiday-empty-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-holiday-empty-patch.log`);
patch SHA-256: `b8a2c56b46146cdaf33415dfe10f2ea0a3660ecc7f08605972ba866288053d5a`.
Native Excel captures of blank/exceptional coercions, early-1900 weekday parity
and broader workbook/UI compatibility remain open.

2026-11-07 STANDARDIZE checkpoint: value, mean and standard-deviation operands
now broadcast with matching static array shape. Typed mapping retains the
existing no-Boolean coercion policy and propagates source errors per element.
Non-finite parameters/results and nonpositive deviations return NUM; the
existing arithmetic expression is retained rather than changing extreme-value
rounding without a native oracle. Oversized output is checked before allocation.
Reference: https://support.microsoft.com/en-us/excel/functions/standardize-function
The pre-change array regression returned NIMPL (`tmp/xlsx-standardize-before.log`).
Calc covers outer axes, deviation arrays, scalar/array errors, numeric text,
Boolean rejection, singleton shape and the documented scalar example. Editor
covers range inputs through deviation/mean edits, undo/redo and error-first
matrix spills after cached/recalculated save/reopen.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-standardize-engine.log`).
Calc: 190 passed across 9 targets (`tmp/xlsx-standardize-calc.log`). Selected
editor conversion suites: 54 passed (`tmp/xlsx-standardize-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-standardize-patch.log`);
patch SHA-256: `a90af6d93a06e9627b0491c377e60d4762ca44ac6a9b904d6bb8d4017e9c80df`.
Native coercion/precision captures and broader workbook/UI compatibility remain
open; these selected suites do not establish complete Excel parity.

2026-11-07 Fisher array checkpoint: FISHER/FISHERINV now map typed range/array
operands element-wise with corresponding static shapes. Source errors remain
per-element; invalid transform domains and non-finite numbers return NUM.
Existing transform arithmetic and tanh inverse are retained to avoid changing
precision policy without native captures.
Reference: https://learn.microsoft.com/en-us/office/vba/api/excel.worksheetfunction.fisher
The pre-change array regression returned NIMPL (`tmp/xlsx-fisher-array-before.log`).
Calc covers matrices, domain endpoints, mixed source errors, existing Boolean/
numeric-text coercions, singleton shapes and transform/inverse composition.
Editor checks source corrections, undo/redo and error-first range results after
cached/recalculated save/reopen, retaining valid later elements and spill shape.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-fisher-array-engine.log`).
Calc: 191 passed across 9 targets (`tmp/xlsx-fisher-array-calc.log`). Selected
editor conversion suites: 55 passed (`tmp/xlsx-fisher-array-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-fisher-array-patch.log`);
patch SHA-256: `b7d6262d3d82bda351669117f077629fb6112c2638f53f1a2fadb799b81be5e8`.
Native coercion/precision captures, small-value cancellation and broader
workbook/UI compatibility remain open; no complete Excel parity is claimed.

2026-11-07 PHI/GAUSS checkpoint: both functions now map typed range/array
operands element-wise and report corresponding static result shapes. Source
errors propagate per element, and non-finite numeric parameters return NUM.
Existing PHI Boolean rejection and GAUSS numeric coercion are retained. GAUSS
constructs the standard normal distribution once per call; both existing
numeric expressions remain unchanged pending native precision captures.
The pre-change array regression returned NIMPL (`tmp/xlsx-phi-gauss-before.log`).
Calc covers matrices, positive/negative values, mixed errors, numeric text and
Boolean policies, singleton arrays and extreme finite tails. Editor checks
range corrections, undo/redo and error-first results after cached/recalculated
save/reopen, retaining valid later values and spill shapes.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-phi-gauss-engine.log`).
Calc: 192 passed across 9 targets (`tmp/xlsx-phi-gauss-calc.log`). Selected
editor conversion suites: 56 passed (`tmp/xlsx-phi-gauss-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-phi-gauss-patch.log`);
patch SHA-256: `2d666d507b7d7e632ac0e5bfcb937a07f1efa42cce1259e07d7f3d8ce87aa470`.
Native coercion/precision captures, GAUSS cancellation near zero and broader
workbook/UI compatibility remain open; no complete Excel parity is claimed.

2026-11-07 error-function checkpoint: ERF, ERF.PRECISE, ERFC and ERFC.PRECISE
now use typed broadcasting, matching their existing static shape analysis.
ERF supports independent lower/upper array axes. Existing no-Boolean coercion
and numeric expressions remain; source errors propagate per element and
non-finite parameters/results return NUM. Function-specific arity is retained.
The pre-change array regression returned NIMPL (`tmp/xlsx-erf-array-before.log`).
Calc covers all four variants, matrices/outer bounds, mixed errors, Boolean/
numeric-text coercions and singleton shapes. Editor covers range bounds and
PRECISE/complement outputs through corrections, undo/redo and error-first
spills after cached/recalculated save/reopen. The oversized-result guard is
also checked for ERF's two independent axes.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-erf-array-engine.log`).
Calc: 193 passed across 9 targets (`tmp/xlsx-erf-array-calc.log`). Selected
editor conversion suites: 57 passed (`tmp/xlsx-erf-array-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-erf-array-patch.log`);
patch SHA-256: `adc2b932cc5b3d095d3d122b6f60b4e70dde3fe7b9fe1067c28883c47624dbe1`.
Native coercion/precision captures, narrow-interval cancellation and broader
workbook/UI compatibility remain open; no complete Excel parity is claimed.

2026-11-07 DELTA/GESTEP checkpoint: both comparison operands now broadcast
with matching static array shapes. The shared typed evaluator retains default
zero for omitted second operands, no-Boolean coercion and the existing
16-digit comparison rounding. Source errors propagate per element; non-finite
numeric parameters return NUM. Oversized outer-axis results are guarded.
The pre-change array regression returned NIMPL (`tmp/xlsx-delta-gestep-before.log`).
Calc covers outer axes, equality/threshold boundaries, default and explicitly
omitted operands, mixed errors, numeric-text/Boolean policy and singleton shape.
Editor checks range operands through corrections, undo/redo and error-first
matrix spills after cached/recalculated save/reopen.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-delta-gestep-engine.log`).
Calc: 194 passed across 9 targets (`tmp/xlsx-delta-gestep-calc.log`). Selected
editor conversion suites: 58 passed (`tmp/xlsx-delta-gestep-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-delta-gestep-patch.log`);
patch SHA-256: `9aac8839449743354eb5d224d6b509384f3b8c0df159c2bc3bbde71dcda96273`.
Native coercion/comparison precision captures and broader workbook/UI
compatibility remain open; no complete Excel parity is claimed.

2026-11-07 bit-operation checkpoint: BITAND/BITOR/BITXOR/BITLSHIFT/BITRSHIFT
now broadcast typed operands, matching their existing static shapes. Shared
validation enforces finite nonnegative 48-bit integer operands and bounded
shift counts, preserving existing numeric coercion and count truncation.
Shifts use u128 intermediates before checking the 48-bit result limit; this
fixes silent i64 wraparound. Before the change, BITLSHIFT(140737488355328,17)
returned zero instead of NUM (`tmp/xlsx-bit-array-before.log`). Negative
BITRSHIFT counts exercising the same left-shift path are covered too.
Reference: https://support.microsoft.com/en-au/excel/functions/bitlshift-function
Calc covers all five functions, outer axes, shift directions, 47/48/53/54-bit
boundaries, invalid operands, source errors and singleton shapes. Editor checks
all five range formulas through operand/count edits, undo/redo and error-first
spills after cached/recalculated save/reopen, including the overflow regression.
Oversized outer-axis allocation guards are checked for all five functions.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-bit-array-engine.log`).
Calc: 195 passed across 9 targets (`tmp/xlsx-bit-array-calc.log`). Selected
editor conversion suites: 59 passed (`tmp/xlsx-bit-array-editor.log`).
Formatting and both diff checks pass. Canonical patch replay verifies 500
source/build files and 480 upstream hashes (`tmp/xlsx-bit-array-patch.log`);
patch SHA-256: `f2c6561d1a67b0a14c5bff1dc06e469028e180c4eb3e63a939d22d92d80faa6e`.
Native exceptional-coercion/fractional-count captures and broader workbook/UI
compatibility remain open; no complete Excel parity is claimed.

2026-11-07 BASE checkpoint: number, radix and optional padding now broadcast.
Truncated finite arguments are validated before integer casts or allocation:
number is nonnegative and below 2^53, radix is 2..36, padding is 0..255.
Previously BASE(1,2,256) returned 256 characters instead of NUM; huge padding
could request unbounded memory (`tmp/xlsx-base-before.log`). The conversion
now needs at most 53 digits and 255 output characters per element.
Reference: https://support.microsoft.com/en-us/excel/functions/base-function
Calc covers bounds, huge padding, the computed 2^53-1 boundary, outer radix/
padding axes, truncation, errors and singleton shape. Editor checks range
parameters, undo/redo and error-first spills after cached/recalculated reopen.
Engine: 2,325 passed / 4 existing ignored (`tmp/xlsx-base-engine.log`).
Calc: 196 passed across 9 targets (`tmp/xlsx-base-calc.log`). Selected editor
conversion suites: 60 passed (`tmp/xlsx-base-editor.log`). Canonical patch
replay verifies 500 source/build files and 480 upstream hashes
(`tmp/xlsx-base-patch.log`); patch SHA-256:
`7394a29ad1f28b32d734da6de7f364a363046588e7c6ec04876ad0dbdee22ad2`.
DECIMAL arrays/range limits, native coercion/precision captures and broader
workbook/UI compatibility remain open; complete Excel parity is not claimed.

2026-11-07 engineering-conversion checkpoint: DECIMAL now parses up to 255
radix digits with exact bounded integer limbs and a single nearest-even
binary64 rounding step. Valid unsigned values beyond i64 are accepted;
excessive length and unrepresentable results return NUM. Signed text and
invalid-text error policies are retained pending native captures. All twelve
BIN2/DEC2/HEX2/OCT2 conversions and BESSELI/J/K/Y now broadcast typed operands,
preserving per-element errors, singleton shapes and existing scalar policies.
Bessel numerical routines and order truncation are unchanged. Oversized
outer-axis results are guarded before output allocation.

Calc: 199 passed across 9 targets (`tmp/xlsx-bessel-array-calc.log`). Selected
editor conversion suites: 63 passed (`tmp/xlsx-bessel-array-editor.log`),
including corrections, undo/redo, cached/recalculated reopen and saved number
versus text types. Engine: 2,328 passed / 4 existing ignored
(`tmp/xlsx-bessel-array-engine.log`), including 4,760 exact u128 comparison
cases spanning radices 2..36 and separate wider-value, rounding-tie, length
and overflow cases. Strict engine Clippy, Rust 1.99 wasm32, formatting and
diff checks pass. Canonical replay verifies 500 source/build files and 480
upstream hashes (`tmp/xlsx-bessel-array-patch.log`); patch SHA-256:
`ece8b44508e9dc0e43c1f740cad70ac7c2fe775fe2ad1ca4d43c93aece73df5d`.
This supersedes the preceding DECIMAL array/range gap. Native exceptional
coercion, high-magnitude precision, very-large Bessel orders, remaining complex
functions and broader workbook/UI compatibility still need work.

2026-11-07 complex-function checkpoint: COMPLEX and the five component functions
IMABS/IMREAL/IMAGINARY/IMARGUMENT/IMCONJUGATE now broadcast typed operands.
The eighteen remaining scalar complex operations also broadcast, including
IMDIV/IMSUB/IMPOWER outer axes. Shared complex-number casts retain existing
parsing, suffixes, scalar coercion, numerical routines and string formatting;
source/domain errors remain per element and singleton arrays retain shape.
Existing IMSUM/IMPRODUCT aggregation behavior is unchanged. Oversized COMPLEX
and binary complex results use the existing allocation guard.

The new array regressions fail with NIMPL before each fix. Calc: 201 passed
across 9 targets (`tmp/xlsx-complex-transcendentals-calc.log`). Selected editor
conversion suites: 65 passed (`tmp/xlsx-complex-transcendentals-editor.log`),
covering all 24 changed functions through range edits, undo/redo and cached/
recalculated reopen with saved text versus number types. Engine: 2,328 passed /
4 existing ignored (`tmp/xlsx-complex-transcendentals-engine.log`). Strict
native Clippy, Rust 1.99 wasm32, formatting and diff checks pass. Canonical
replay verifies 500 source/build files and 480 upstream hashes
(`tmp/xlsx-complex-transcendentals-patch.log`); patch SHA-256:
`cedc1a740bd536484646aa2bdb71e25ef1327c512e749b6df2d989c40c1fd457`.
The earlier six-function checkpoint passed 200 calc / 64 selected editor
cases (`tmp/xlsx-complex-calc.log`, `tmp/xlsx-complex-editor.log`). Native
exceptional coercion, extreme-magnitude numerical behavior, aggregate complex
semantics and broader application oracles remain open.

2026-11-07 complex aggregate arity checkpoint: IMSUM/IMPRODUCT now accept
1..255 scalar arguments in runtime and static signatures, replacing the old
exactly-two restriction. Existing scalar casts, suffix checks, formatting and
left-to-right error order are retained. References:
https://support.microsoft.com/en-us/excel/functions/imsum-function and
https://support.microsoft.com/en-gb/excel/functions/improduct-function .
Calc: 202 passed /9 targets; selected editor: 66 passed; engine: 2,329 passed /
4 existing ignored (`tmp/xlsx-complex-variadic-{calc,editor,engine}.log`).
Tests include zero/one/255/256 arguments and reference edits/history/save.
Range/array aggregation, real-only suffix interactions, exceptional coercion
and extreme-value arithmetic remain open.

2026-11-07 CONVERT checkpoint: Celsius conversion used a 273.5 Kelvin offset
instead of 273.15. CONVERT(68,"F","C") now returns 20 instead of 19.65;
Celsius identity and negative-temperature round trips are corrected too.
Reference: https://support.microsoft.com/en-us/excel/functions/convert-function .
CONVERT also broadcasts values and both unit operands with matching static
shapes, per-element source errors, unknown/incompatible-unit failures and
singleton shapes. Existing unit tables and scalar conversion policies are
retained; tables are built once per call and reused across output elements.
Oversized outer-axis results are guarded before output allocation.

The scalar temperature regression and array NIMPL regression were reproduced
before their fixes (`tmp/xlsx-convert-{celsius,array}-before.log`). Calc:
204 passed /9 targets (`tmp/xlsx-convert-array-calc.log`); selected editor:
68 passed (`tmp/xlsx-convert-array-editor.log`), including value/unit edits,
undo/redo and error-first matrices through cached/recalculated reopen. Engine:
2,329 passed /4 existing ignored (`tmp/xlsx-convert-array-engine.log`). Strict
native Clippy, Rust 1.99 wasm32, formatting and diff checks pass. Canonical
replay verifies 500 source/build files and 480 upstream hashes
(`tmp/xlsx-convert-array-patch.log`); patch SHA-256:
`b016b70cfffb6f525d33afd3bbf23aaabc49ab0af4553841aa7f78a6072812e7`.
The preceding scalar-temperature checkpoint passed 203 calc /67 selected
editor cases. Native exceptional coercion, full unit-family accuracy, complex
range aggregates, broader numerical captures and workbook/UI gaps remain open.

2026-11-07 fractional-dollar conversion checkpoint: DOLLARDE/DOLLARFR now
broadcast values and denominators with matching static shapes. Existing scalar
conversion, denominator truncation, negative/zero errors, no-Boolean coercion
and source-error precedence are retained. Oversized outer-axis outputs are
guarded. References:
https://support.microsoft.com/en-gb/excel/functions/dollarde-function and
https://support.microsoft.com/en-us/excel/functions/dollarfr-function .
The pre-change array regression returns NIMPL (`tmp/xlsx-dollar-array-before.log`).
Calc: 205 passed /9 targets; selected editor: 69 passed; engine: 2,329 passed /
4 existing ignored (`tmp/xlsx-dollar-array-{calc,editor,engine}.log`). Editor
coverage includes operand/denominator edits, undo/redo and error-first matrices
through cached/recalculated reopen. Strict native Clippy, Rust 1.99 wasm32,
formatting and diff checks pass. Canonical replay verifies 500 source/build
files and 480 upstream hashes (`tmp/xlsx-dollar-array-patch.log`); patch SHA-256:
`19f4cec6b289529f57593d73ac548f73d44861cb43165a6981f6481bf074a706`.
Native exceptional coercion, high-magnitude precision, other financial-function
arrays and the broader workbook/UI/application-oracle gaps remain open.

The engineering integration checkpoint `da321d0e` includes these Excel changes and the
concurrent core main changes (including Word #519 and #525), and resolves the
submodule conflict with office #270. Excel production and test sources match
the `601122d1` checkpoint used for the 205-case calculation and 69-case selected
editor runs above. The editor profiling example additionally gains Windows CPU
time support from core main. The vendored engine and canonical replay inputs
remain identical to the previously verified `286c4c5b` integration.


2026-11-08 formula-source, numeric-criteria and financial checkpoint (core #531):
FORMULATEXT retains parsed source spacing and structured references through
parsed caches, kept recalculation, what-if restoration, edits and saved reopen.
Numeric conditional-aggregation criteria trim ASCII spaces for numeric parsing
only, preserving literal text and wildcard spacing. SLN, SYD, EFFECT, NOMINAL,
RRI and PDURATION broadcast typed operands while retaining scalar domains and
Boolean policies. The complete selected calculator run passes 209 cases across
9 targets; selected editor conversion/protection suites pass 73 cases across
4 targets; the engine passes 2,331 with 4 existing ignored. Strict calculator
Clippy, Rust 1.99 wasm32, format/diff checks and canonical replay (500 files /
480 upstream hashes) pass. Canonical patch SHA-256:
`8bcfdf44ac3f46b735e7dce4cc2e96a867c5915436e578e96ed40249131c531e`.

At this checkpoint, 35 of 39 formula anchors match stored results in Microsoft's
published counting workbook, up from 32; no formulas are skipped. This is a
published saved-result reference, not a fresh native Excel application capture.
The external binary is an ignored local audit input and is not redistributed.
Three legacy CSE brace cases and one legacy-array/cache ambiguity remain here;
subsequent array-identity and fixed-range follow-ups are tracked separately.
Unicode criterion spaces, broader financial functions, numerical coercion and
precision, workbook/UI behavior and native application validation remain open.


2026-11-08 legacy-array follow-ups (core #532, #534, #535, #537 and #538):
XLDAPR reuse follows the actual future-metadata record and explicit dynamic
flag, with namespace/parent ownership and ambiguity checks. Reader arrays keep
Legacy/Dynamic/Unknown identity. Legacy calculation uses the declared CSE area:
excess results are truncated, scalar results repeat, and missing array values
are #N/A. Held and over-budget arrays retain their caches and fixed ranges.
The eager CSE calculation budget is 1,048,576 cells; larger declarations remain
reported as unsupported calculation, with sparse source identity preserved.
Partial value edits to multi-cell legacy arrays are refused atomically; whole
replacement/deletion, per-cell formatting and single-cell arrays remain allowed.

FORMULATEXT and ISFORMULA resolve legacy members to the original anchor without
relative translation or dense area expansion, including held and huge arrays.
Temporary literal probes restore that identity. Cell/range views expose the
original member formula while retaining the member's own value and freshness.
Dynamic and unknown output cells are kept distinct. Formula search/replace and
structural CSE operations still require group-aware compatibility work.

The complete calculator checkpoint at `fc9c076c` passes 218 cases /9 targets;
selected editor suites pass 84 cases /6 targets; the engine passes 2,332 with
4 existing ignored. The cell-view checkpoint `c69fba64` additionally passes
29 reader library and 6 selected legacy editor cases. Strict native Clippy,
Rust 1.99 wasm32 and formatting/diff checks pass. Canonical replay verifies
500 files /480 upstream hashes; engine patch SHA-256:
`d7d867f97447ccf9171f81f718850a0d05ad0b994b07cdc26f17cb568e45560d`.

The shared sheet model now supplies separate formula-bar and editable text:
a selected legacy member displays `{=SEQUENCE(D1, 1)}`, while editing starts
with `=SEQUENCE(D1, 1)`. Native and web non-editing formula bars use that display
getter. Hidden protected members remain blank. All 6 typing-model cases pass,
including the new legacy selection/input/source-protection/save/reopen case.
The pre-change runtime test shows the missing CSE display braces. Strict
TinySkia model library/typing-test Clippy and Rust 1.99 TinySkia model wasm32
checks pass; local debug symbols were disabled to fit the build environment.
These checks exercise the shared model, not a fresh native Excel GUI session.

A fresh saved-reference comparison on the combined UI/core test content matches
38 of 39 formula anchors in the published Microsoft counting workbook, with
zero skipped formulas (`tmp/xlsx-cse-ui-final-sample.log`). The remaining
ordinary-formula VALUE-versus-37 evaluation-mode case is unresolved; no cached result is
forced to make the comparison match. Broader financial/coercion/precision,
UI/rendering, structural operations and native-application oracles remain open.

The app integration pins published core main `5db65c85308476741bfd9c5cf3a96b64d4a9bc0e`
after #538, preserving the concurrent Word changes. Its complete source tree
matches the local combined core used for the model and saved-reference checks
above. Final-pin canonical replay passes 500 source/build files and 480 upstream
hashes (`tmp/xlsx-cse-ui-final-patch.log`). The committed LF patch SHA-256 is
`d7d867f97447ccf9171f81f718850a0d05ad0b994b07cdc26f17cb568e45560d`;
the Windows CRLF checkout reports
`3e4f19d0bef151884c76865aaf033cc2d09fae65aca28111d67c862cf49d8dc1`
and matches that committed patch after CRLF normalization. Structural-editor
and clipboard follow-ups after #538 are separate from this integration pin.

2026-11-08 array editing and metadata integration (core #541, #542, #544, #546):
The app now pins published core main `b8f9b95075940c42892da17c0bfb2f0cc362fc5d`.
Row/column insertion cannot split a proven legacy array or clip its unstored
tail beyond the worksheet edge; partial deletion is refused atomically.
Whole-area movement/deletion remains allowed. Both direct writer entry points
and the editor use the same sparse bounds check, and partial-array errors are
classified as caller input errors.

Whole legacy-array clipboard copies and moves preserve their fixed ranges,
including overlapping moves, cross-sheet moves, historical copied snapshots
and dependent CSE formula rewrites after a cut. Partial destination writes and
partial source cuts are refused before writing. Dynamic/unknown metadata and
partial-source copying remain separate clipboard compatibility work.

New dynamic spills can append the required type, future record and cell record
to an existing metadata part without renumbering existing entries. Empty and
qualified containers, strict namespaces and unrelated/foreign content are
covered. Existing proven dynamic links are reused unchanged. Dangling t/v/cm
references, duplicate containers and inconsistent counts are not silently
activated by appending; unsupported malformed metadata retains the pre-existing
fallback and remains a compatibility limit.

The combined Excel checkpoint `5cd7397a` passes the selected legacy-array,
spill-writer and structural-writer suites plus the shared typing-model suite;
detailed counts are recorded in the integration PR. Final pin `b8f9b950`
also preserves the concurrent Windows spacing core #543 and app #273 changes.
Its reader/calculator/writer/editor and vendored engine sources match that
Excel checkpoint. Final-pin canonical replay passes 500 source/build
files and 480 upstream hashes (`tmp/xlsx-array-edit-integration-patch.log`). The
committed LF engine patch remains `d7d867f97447ccf9171f81f718850a0d05ad0b994b07cdc26f17cb568e45560d`
(Windows CRLF checkout hash `3e4f19d0bef151884c76865aaf033cc2d09fae65aca28111d67c862cf49d8dc1`).
The formula-evaluation-mode follow-up core #549 is separate from this pin.
The remaining counting-sample cell is persisted as an ordinary formula rather
than a CSE declaration; its IIE/AE distinction is tracked in that follow-up.
Fresh native Excel application validation and broader table/clipboard,
coercion/precision and rendering gaps remain open.

2026-11-08 ordinary and array evaluation integration (core #549):
The app pins published core main `677f7601a60a5711241c3fc829aef5f2e45cad0c`.
Imported ordinary formulas apply implicit intersection at scalar argument
boundaries. Proven fixed legacy arrays retain their declared CSE area, and
dynamic arrays retain array evaluation. Newly entered modern formulas persist
their array identity when required, including aggregates that return one value.
For example, an ordinary `SUM(LEN(A1:A3))` outside the input rows returns
`#VALUE!`, while a fixed CSE formula or a newly entered array-evaluated aggregate
returns the sum of the three lengths. Displayed original source remains intact.

The published Microsoft counting workbook comparison at this core checkpoint
matches all 39 saved formula anchors, with zero skipped formulas. This resolves
the earlier 38/39 ordinary-formula mismatch without forcing cached results.
It compares Microsoft's stored workbook answers, not a fresh native Excel run.
The final pin's reader, calculator and vendored engine sources match #549's
verified head. Canonical patch replay verifies 500 source/build files and
480 upstream hashes. The committed LF engine patch SHA-256 is
`810ef161e2a891527c93bfa30be02cf2842cf0c7231048ee746989df47c13c60`.

Clipboard mode propagation (#550), metadata preservation (#551), spill XML
ownership (#552), named-formula intersection (#554) and table spill bounds
(#555) are separate follow-ups and are not included in this app pin. Fresh
native Excel application validation, broader tables, unknown/composite
metadata, precision/coercion and rendering compatibility remain open.
