# Cell insertion and deletion

This is part of the [ibisOffice comparison](ibis-parity.md), not a claim of
complete XLSX or Excel parity. The reference exposes four choices in
[`cells_dialog.rs`](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/cells_dialog.rs)
and applies them in `ibis-doc-model/src/sheet_command.rs`. No reference
implementation code was copied. The implementation uses Jade's own structural
geometry, formula scanner and preserving XML edits.

## Implemented

`jade_xlsx_reader::CellShift` describes a rectangle, an axis and insertion or
deletion. Rows means down/up within its columns; columns means right/left
within its rows. Entire-axis selections delegate to the existing whole-axis
writer. `shift_cells_formula` moves absolute and relative A1 references into
the affected sheet. References whose perpendicular span crosses the strip
retain their boundaries, matching the reference implementation's rule.
Strings, structured selectors, external workbook references and 3-D sheet
qualifiers are not mistaken for local cells. Deleted references become
`#REF!`; retained ranges shorten or grow.

The writer and headless editor apply the operation as one history edit:

- Sparse cells move with their raw rich-text/extension content. Effective
  cell formats are carried across different row/column defaults. Row heights,
  visibility and column widths stay on their axes during a partial shift.
- Shared formula groups expand before references change, including groups on
  other sheets. Defined names, cell formulas, standard rule formulas, table
  formulas and chart sources follow their sheet context.
- Hyperlink ranges and internal destinations follow; fragments in another
  workbook do not. Notes and their VML positions follow their cells.
- Drawing markers follow their strip, respecting absolute anchors and the
  move-only `editAs` option. Shapes wholly removed by a two-marker deletion
  are removed.
- Tables covered across their full width/height move, grow or shrink, retaining
  column IDs. Header replacements and inserted columns stay synchronized.
  Recorded calculated columns fill newly inserted rows, and their saved
  formula origin follows first-row/header changes. Existing literal, blank
  and formula exceptions remain untouched. Whole-axis edits use the same
  calculation-origin/fill logic. Removed column/table references are rewritten,
  and unused table parts go.
- Standard and x14 validation and relative conditional-format coverage split
  where movement changes its translation. Each piece gets the appropriate
  relative formula anchor; inserted interior cells inherit a rule from the
  preceding cell. Aggregate rules (data bars, colour scales, icon sets,
  ranks, averages and duplicate/unique tests) retain one population over the
  mapped union, rather than changing thresholds by cloning each fragment.
- Paired x14 data bars retain their legacy link ID and use the same mapped
  coverage. Standalone extended priorities share the standard priority order;
  paired records without a priority remain without one. Cloned revision UIDs
  and standalone rule IDs are renewed without colliding with package IDs.
  Formula-valued standard CFVO attributes follow structural references too.
- x14 sparkline display cells move or disappear with their cells, without
  creating a new sparkline in an inserted blank. Source and date-axis formulas
  follow their worksheet context, including sources on another sheet. Empty
  sparkline groups and known extension containers are removed. Namespace
  aliases and Strict worksheets are supported; foreign lookalikes and opaque
  extension siblings remain preserved.
- Whole array/merged blocks can move. Operations that split their protected
  geometry, split a table, change an array's shape, discard stored edge cells
  or move locked sheet content are refused atomically.

Creating a table without headings uses this structural operation to insert its
new heading row. Occupied cells below the selection move down too; source
rich text, notes, links and absolute/dependent references follow. The prior
rectangle-only value copy and empty-row requirement were removed for this path.

Both shells offer the four operations in the cell context menu through the
shared `Workbook::shift_cells` model API. Long menus are bounded to the visible
viewport and scroll. The dedicated four-choice keyboard dialog and contextual
table-row/column shortcuts are not implemented yet.

The operation follows Microsoft's descriptions of
[inserting cells](https://support.microsoft.com/en-us/excel/insert-one-or-more-rows-columns-or-cells-in-excel-for-mac)
and [range insertion](https://learn.microsoft.com/en-us/office/vba/api/excel.range.insert).
Their descriptions are not an application oracle for all imported edge cases.

The extension edits follow Microsoft's [CT_CfRule](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/025ea6e4-ad42-43ea-a016-16f4e4688ac8),
[CT_Cfvo](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/15094f93-f587-4f4e-be74-1fbdc93da768),
[CT_DataValidation](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/89029dfc-1ca8-4ff9-afe0-46f9454d09c6)
and [CT_Sparkline](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/6b28a993-e0fd-451d-860e-35097c6baa77)
records. In particular, an explicit x14 priority makes its ID independent of a
legacy link; an absent priority denotes paired extension information.

## Verification and remaining work

The extended-rule coordinate checkpoint passed **851 tests in all 137 targets**.
The cell-shift target has twenty headless cases, with additional reader
coverage for geometry, structural references and structured-name translation.
Eight writer extension cases cover relative anchors in both axes, counts,
identity renewal, aggregate unions, mixed standard/extended priorities,
paired deletion, standalone ID precedence, sparkline locations/source formulas,
Strict/default namespaces and foreign lookalikes. The ninth new case verifies
signed data-bar geometry and extended properties after partial insertion,
save/reopen and exact history.

Existing coverage includes all directions, shared formulas, inherited formats,
rich text, names, notes, links, arrays, merges, drawing markers, table columns
and structural header insertion over occupied data. See
[x14-verification.json](x14-verification.json) for this checkpoint and
[verification.md](verification.md) for compilation results and runtime limits.
The preceding table-ownership checkpoint passed 869 core tests across all 140
targets and seven shared-model table/cell-shift scenarios, plus eight shared
validation scenarios. See [table-ownership-verification.json](table-ownership-verification.json)
for that snapshot; the earlier calculated-template model checkpoint is
[recorded separately](table-template-verification.json).

The preceding quoted-validation checkpoint passed 882 core tests in 141 targets
and 18 shared-model scenarios (seven table/cell-shift, eleven validation).
Partial shifts now rebase formula alternatives at each split anchor while
retaining quoted literal values. See
[quoted-validation-verification.json](quoted-validation-verification.json).

Further work before declaring this area complete:

- Cover general worksheet/container-level MCE, other imported extension
  families and conditional visual activation predicates. Whole-axis
  conditional-formatting anchors/namespaces need a separate audit. Standard and x14
  input-rule reading, indexed preserving edits, quoted literal sources,
  source-level alternatives and shared model enforcement are implemented; see [validation.md](validation.md).
- Cover what-if data-table input attributes, form controls and other cell-bound
  objects not represented by the current reader, query-linked tables and
  partial-strip sort/filter state.
- Broaden table import/ownership oracles and audit the remaining worksheet,
  drawing and formula namespace contexts. Direct tableParts ownership, table
  geometry/filter/sort namespace isolation, formula-template hierarchy,
  duplicate relationship shifts and imported maximum column IDs are now
  handled; see [tables.md](tables.md). Calculated row fill and template-origin
  rebasing are implemented; grid-edge/imported formula cases still need Excel
  application captures.
- Capture Excel cases for crossing/3-D references, inserted-cell formatting,
  merge deletions, print-area boundaries and drawing anchors at strip edges.
- Audit remaining byte stability and file growth: no-op child edits now retain
  original XML bytes, including untouched ordinary sheets. Shared-formula
  expansion and moved detached namespace bindings can still normalize cell
  headers or materialize declarations.
  The current authoring limit is one million stored/materialized cells.
- Run the new native command scenario and exercise both context menus. Real
  Excel save/repair, calculation and visual comparisons remain outstanding.

Whole-axis input-rule edits now retain x14 containers, rebase a deleted first
anchor/union member, clip grid-edge coverage and isolate unknown source XML.
The preceding whole-axis checkpoint passed 895 core tests across 142 targets and 19
shared-model scenarios, with both shell compilation checks. See
[validation-structure-verification.json](validation-structure-verification.json)
and [validation.md](validation.md) for target results, rule semantics and
evidence limits.

The broader parity goal remains open, including the other areas in the audit.


The worksheet package formula pass used by partial shifts now isolates
interpreted sources on other worksheets, including standard/x14 validation,
conditional formulas/thresholds and sparkline/date-axis sources. Foreign XML
and unsupported source choices remain opaque; current-row table context also
follows implicit cell coordinates and x14 validation coverage. See
[formula-ownership.md](formula-ownership.md). Whole-axis rule/sparkline passes,
sheet rename/delete and reader ownership still need separate coverage.


The current formula-ownership checkpoint passes 905 core tests across all 143
targets and 20 shared-model scenarios (thirteen validation, seven tables), with
native production/test and browser wasm compilation. See
[formula-ownership-verification.json](formula-ownership-verification.json)
for target results, frozen hashes and evidence limits. Manual UI and Excel
application oracles remain outstanding.
