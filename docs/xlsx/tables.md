# Table authoring work

This work addresses the table gaps in the [ibis comparison](ibis-parity.md).
It is in progress. The checks below do not establish full table/Excel parity.

## Implemented paths

The writer and headless editor now create tables and edit their name, size,
style, header/total rows, filter buttons, calculated columns and per-column
totals. Table names are checked against all tables and defined names in the
workbook. Names are case insensitive, and table IDs, column IDs, package part
names and relationship IDs are allocated without replacing existing objects.

Creation normalizes headings to unique text names and writes the matching
header cells, definition, content-type override and worksheet relationship in
one transaction. Blank/duplicate headings receive generated/suffixed names.
A selection without headings gains a new heading row through the
[cell-shift writer](cell-shifts.md). Cells below the selection move too,
including occupied cells; notes, links, raw rich text and structural formula
references follow. A header-only selection
gains an empty data row. Tables cannot overlap another table, merges or array
blocks, and table authoring is refused on protected sheets.

Design options cover all 60 built-in table styles and existing custom styles,
first/last columns, row/column stripes, headers, totals and filter buttons.
Totals support Sum, Average, Count, Count Numbers, Maximum, Minimum, Standard
Deviation, Variance, labels and custom formulas. Calculated-column formulas
are stored in column metadata and filled into worksheet data cells with
relative references moved per row. New data rows inherit recorded calculated
formulas without replacing existing exceptions. Structural row/cell insertion
now fills new rows too; insertion/deletion of the first data/header row rebases
the saved formula template by stable column ID. Literal, blank and formula
exceptions in retained rows remain untouched. Direct cell entry and batched
cell edits can grow tables through consecutive adjacent rows/columns and fill
a newly entered formula into an otherwise empty column. Formatting-only edits
and protected worksheets do not trigger this behavior.

Table and header renames rewrite structured references in formula parts and
names, while retaining string literals and external workbook references.
Implicit current-table references use the cell/table context. Calculation
projects static structured references (including `[@Column]`) to their
current A1 equivalent without changing the saved formula text.

Conversion to a normal range removes the worksheet table reference, its
unneeded relationship/definition/type override, and rewrites structured
references to fixed/current-row A1 references. Table styling is materialized
in cell formats with existing direct formatting retained. Converting after
editing dialog settings is one history operation. Unknown root/column/style
attributes, unrelated extension children and stable column IDs are retained
when editing a definition.

These behaviors follow the basic [structured reference
rules](https://support.microsoft.com/en-us/excel/using-structured-references-with-excel-tables)
and [table design options](https://support.microsoft.com/en-us/excel/format-an-excel-table).
The column-name length limit is specified by [Microsoft's OOXML
notes](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/b639d6d3-bc4d-4269-b6da-bb35fc1b49a0).

New-row filling follows the documented
[calculated-column behavior](https://support.microsoft.com/en-us/excel/use-calculated-columns-in-a-table-in-excel-for-the-web).
Retained exceptions are preserved, consistent with Microsoft's description of
[calculated-column exceptions](https://support.microsoft.com/en-us/excel/detect-formula-errors-in-excel).
The saved A1 formula is treated as a template at the first data row, as in the
existing Jade authoring path. Its structural origin handling is covered by
regression tests; imported origins, relative dependencies outside a moved band
and grid-edge cases still need Excel application captures.

Both shells have a Create / Design Table context-menu entry and Command/Ctrl+T.
The dialog exposes range/name, style selection/options, header/total/filter
flags, calculated-column formulas and column totals, and conversion to a normal
range. Column drafts are retained while selecting a different column. The
browser production build passed for WASM after rebuilding with one job. The native
production build and all native test code also compile. The preceding six shared
table/cell-shift scenarios passed runtime testing, including calculated-row
fill and cache refresh through model history. The native table scenario and manual interaction in both
shells remain unverified.

## Table ownership and namespace preservation

A worksheet's direct `tableParts` collection now determines its tables;
relationship-only definitions are retained as opaque parts and cannot coerce
ordinary header-position edits into text or participate in table expansion,
filtering and structural moves. Linked IDs must resolve to internal table
relationships. Both Transitional and Strict element/relationship namespaces
and aliases are recognized; the existing undeclared conventional `r:id`
tolerance remains. Explicitly foreign IDs/elements are not links. Referenced
targets are deduplicated in worksheet order, and each is structurally shifted
once. Deleting a table removes its same-sheet relationship aliases too.

The preserving geometry writer edits only direct owned table columns,
filter/sort children and formula metadata. Foreign namesakes and nested
extension formula lookalikes remain untouched. Calculated/total formula
rewriting uses the recognized column and its stable ID, including CDATA and
entity text. Inserted columns use a namespace binding available from their
container, rather than a sibling's local declaration. Sparse imported column
IDs that include the unsigned maximum no longer overflow on insertion.

This follows Microsoft's [tableParts ownership description](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.tableparts?view=openxml-3.0.1)
and [table package structure](https://github.com/OfficeDev/open-xml-docs/blob/main/docs/spreadsheet/working-with-tables.md).
These are format requirements; an Excel application oracle remains separate.

## Evidence and remaining work

Fourteen table-authoring integration tests passed after rebuilding on alternate
storage. They cover creation/save/reopen/history, no-header data moves,
duplicate headings, name/formula following, all eight built-in totals,
custom totals, calculated columns, expansion, conversion, header/filter flags,
strict namespace aliases/foreign metadata, direct total-cell metadata
updates/constant preservation, snapshot row expansion/formula fill, pasted
header reference following, cut/pasted totals and protected/invalid operations.
The calculated-template checkpoint passed 842 tests across 136 library,
integration and documentation targets, including structural header insertion,
calculated-row fill and template-origin rebasing.
Its machine-readable results are in
[table-template-verification.json](table-template-verification.json);
[cell-shift-verification.json](cell-shift-verification.json) and
[table-verification.json](table-verification.json) record earlier checkpoints. The shared model and both
shells compile. The current seven shared table/cell-shift scenarios passed runtime testing;
the native test targets passed type checking. Native runtime and manual
interaction in both shells remain unverified. See the current [verification record](verification.md)
for the checks and their storage limits.

Work still required before claiming parity:

- Run the new native scenarios and exercise both dialogs after storage
  is available; the full XLSX regression, seven shared-model table scenarios and
  browser compilation have passed.
- Header insertion now uses the cell-shift engine and follows standard
  metadata and structural references. x14/extension identities and other
  imported edge cases still need reconciliation; see [cell-shifts.md](cell-shifts.md).
- Resizing currently keeps the first row and overlapping columns. Relocating
  a totals row into occupied cells is refused; the general cell context commands now support full table-width/height band
  shifts, while dedicated table-row/column controls and relocation remain.
- Internal snapshot pastes now follow header renames, expand adjacent tables,
  fill calculated columns and update totals metadata. Audit remaining fill and
  Paste Special paths, mixed calculated-column exceptions and formulas left
  outside a shrunken table. Moving/copying an entire table as an object remains.
- Enforce dynamic-array spill constraints inside tables and add Office cases
  for implicit intersections, indirect references, unions of selectors,
  removed-column references and lexical formula scopes.
- Add custom table-style authoring, table-part selection commands and the full
  contextual ribbon. Converted table colours currently become resolved RGB;
  indexed/pattern/theme-dependent style cases need more preservation coverage.
- The authoring cell limit is one million cells for dense calculated-column
  writes/style materialization. Sparse large-table cases need further work.
- Revalidate sort/filter state on size changes and conversion, query-linked
  tables, chart caches, and all saved package references to a renamed table.
- Capture real Excel save/repair, calculation and visual comparisons. None was
  performed for this addition.

The table-ownership checkpoint passed 869 tests in all 140 core targets and
15 shared-model scenarios (seven table/cell-shift, eight validation), including
ordinary numeric edits with a stale table relationship and exact history.
See [table-ownership-verification.json](table-ownership-verification.json) and
[verification.md](verification.md) for the final source hashes and compilation
scope. Counts supersede earlier checkpoints rather than adding to them.


## Formula-source ownership during table edits

Package-wide table rename, column rename/deletion and conversion now route
worksheet sources through the ownership visitor. Imported x14 validation
supplies its first coverage cell as table/current-row context, and cells with
omitted coordinates use their inferred address. Thus unqualified current-row
bounds follow a column rename and become row-specific A1 operands on
conversion. Foreign wrappers, unknown source choices and missing-range records
are retained. Supported formula leaves preserve comments when their text
changes and retain untouched CDATA bytes. See
[formula-ownership.md](formula-ownership.md) for paths, evidence and the broader
reader, sheet-operation and application-oracle limits.


The current formula-ownership checkpoint passes 905 core tests across all 143
targets and 20 shared-model scenarios (thirteen validation, seven tables), with
native production/test and browser wasm compilation. See
[formula-ownership-verification.json](formula-ownership-verification.json)
for target results, frozen hashes and evidence limits. Manual UI and Excel
application oracles remain outstanding.
