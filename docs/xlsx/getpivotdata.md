# Saved pivot queries and structural editing

GETPIVOTDATA now has real registration, reference evaluation and visible-result
lookup. Its host context belongs to one calculation model. The adapter uses
the declared pivot/cache catalog rather than treating ordinary worksheet data
as a pivot, and reads the saved report cells rather than summing source/cache
records. Editing source data retains the saved report until an explicit
[pivot refresh](pivot-refresh.md) rebuilds supported caches and reports.

## Supported behavior

- Canonical `GETPIVOTDATA(data_field,pivot_reference,field,item,...)`, including
  cell references for value arguments and single-cell/range defined names for
  the pivot anchor. Normal formula errors, dependencies and cycles propagate.
- Display captions and source aliases, with primary displayed measure captions
  taking precedence over another measure's source alias. Numeric, boolean,
  text, blank and ISO-cache-date criteria remain typed; date serials follow
  the workbook's 1900/1904 epoch, including time fractions.
- Separate row/column criteria, compressed item prefixes, visible grand totals
  and saved subtotal lines, multiple measures and selected report filter items.
  Report-filter cache indices are explicitly mapped to pivot-field item indices;
  unused shared cache values cannot shift the meaning of an item.
- Scalar/singleton/array broadcasting, spill results and per-element errors.
  Input/output cell budgets bound the additional GETPIVOTDATA work. Resource
  limits are implementation constraints, not Excel equivalence claims.
- Owned workbook and worksheet declarations inside selected MCE Choice/Fallback
  branches, at container and item depth. Unsupported branches and foreign
  parents do not supply declarations. This does not implement general MCE for
  every pivot/cache child or other OOXML feature.

Unknown or ambiguous visible layout produces a calculation artifact so the
adapter preserves the imported formula cache and reports a skip. A genuine
invalid/no-pivot/invisible-item query returns REF instead. Duplicate table
ownership retains uncertainty on the owning worksheet even after ambiguous
tables are removed from the catalog.

## Editing and preservation

Whole-report movement caused by row/column changes and covered cell-strip
shifts updates the owned pivot definition's location. Source worksheet ranges
follow compatible structural changes; the cache requests refresh on load while
its original records and displayed summary remain unchanged. Source-sheet
rename edits only the owned cacheSource/worksheetSource element, preserving
foreign lookalikes and inherited namespace aliases. Ordinary edits outside a
pivot keep the original opaque parts.

Partial/resizing/removal edits to a saved report, nonrectangular source shifts,
and complete removal of its source are refused atomically until corresponding
pivot rebuilding is available. Value edits/pastes into the report are refused;
formatting and copying its values out are available. Cut-moving a report or a
range source is refused pending source/report relationship reconciliation.
When a cache source cannot be interpreted, structural edits, source renames
and cut moves are refused before commit so omitted caches cannot retain a stale
source. These refusals prevent inconsistent metadata; they do not establish complete
Excel editing parity. Row/column relocation, typed queries, saved values,
Save/Reopen and Undo/Redo are exercised through both headless and shared models.

## Verification

The earlier query/structure checkpoint is recorded in
[getpivotdata-verification.json](getpivotdata-verification.json). Current frozen
source and application compiler results include the later
[dependency-order checkpoint](pivot-refresh-order-verification.json).
The generated [function inventory](jade-function-inventory.json) and
[difference](function-diff.json) are produced by executing the actual engine
inventory example and comparing it with the pinned reference inventory.
Tests include:

- `jade-core/crates/jade-xlsx-calc/tests/pivot_calculation.rs`;
- `jade-core/crates/jade-xlsx-headless-editor/tests/pivot_structure.rs` and
  `pivot_preservation.rs`;
- `jade-core/crates/jade-xlsx-reader/tests/pivots.rs`;
- `crates/jade-office-model/tests/pivots_like_excel.rs`;
- independent model context, invalid host coordinates and cycle tests in
  `jade-core/third_party/ironcalc-base/src/pivot.rs`.

The fixture is independently generated, with deliberate unused shared-cache
indices and stale formula-cache values in calculation tests. It is a regression
fixture, not an Office-produced schema/repair/save/render oracle. Vendored
engine changes remain recorded against the hash-pinned official upstream crate
by `scripts/xlsx/replay_engine_patch.py`; no ibis implementation code is copied.

## Remaining work

Legacy two-argument reference/descriptor syntax, complete OLAP/member naming,
unsupported cache refresh variants, pivot creation/layout/filter/group/calculated authoring,
report deletion/cut-move, source invalidation and broader MCE/extension semantics
remain open. A reference containing multiple pivots currently prefers the last
saved declaration; an actual creation-order/Office oracle is still needed.
Other function languages, cross-workbook references, custom subtotal descriptor
syntax, general cache-derived semantics and broader application captures also
remain. Slicers, timelines and pivot charts require their own implementation.
The overall [ibisOffice parity goal](ibis-parity.md) remains active.

Primary behavior references: [Microsoft GETPIVOTDATA](https://support.microsoft.com/en-us/excel/functions/getpivotdata-function),
[compressed RowItems](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.rowitems?view=openxml-3.0.1),
[PageField item indices](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.pagefield?view=openxml-3.0.1),
and [cache definitions](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.pivotcachedefinition?view=openxml-3.0.1).
The pinned reference's [GETPIVOTDATA wrapper](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-calc/src/functions/lookup.rs)
was inspected for argument forms and callback/context requirements. Source
inspection, generated regression results and Office oracles are separate evidence.
