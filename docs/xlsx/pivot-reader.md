# Declared pivot tables and shared caches

This is the historical reader checkpoint. The later [standard ownership and
styles correction](pivot-styles.md) fixes its worksheet declaration assumption:
CT_Worksheet has no `pivotTableParts`; actual pivot ownership comes from typed
worksheet relationships. Later GETPIVOTDATA/refresh/structural work also
supersedes the pending-function statements below.


The reader now exposes `Workbook.pivots`: worksheet-owned pivot definitions,
workbook-declared shared caches and typed cached records, with independent
metadata issues. Previously these parts were retained without an interpreted
pivot model. This supplies reader/context foundations; GETPIVOTDATA calculation,
pivot refresh, rebuilding/filtering/authoring and full structural reconciliation
remain active work.

The current view resolves `pivotCaches` through workbook relationships and
pivot tables directly through each worksheet's typed relationships. Cache IDs are independent
of vector position and relationship IDs. Exact internal relationship types,
owner-scoped duplicate detection, strict/transitional namespaces and qualified
relationship-ID aliases are checked. Package-relative and absolute targets resolve
without escaping the package or fetching external sources. Relationships are
indexed once per owner. Foreign/extension wrappers do not promote lookalike
children into the standard view; general MCE interpretation remains open.

Cache values retain text, finite numbers, booleans, errors, missing values, ISO
date spelling and shared indices. Unused and unknown entries keep their positions;
removing them would silently change later record references. Definitions retain
source fields and grouped metadata, locations, pivot fields/items, row/column
axes and compressed item descriptors, page/data fields, aggregation/show-as/base/
format metadata, filter conditions, style attributes and unsupported feature markers.
Dates are not prematurely converted using a guessed epoch.

A real regression also reproduced valid numeric boundary whitespace incorrectly
losing cache identity and record counts. Those paths now use consistent numeric
[normalization](https://www.w3.org/TR/xmlschema-2/#rf-whiteSpace). The first frozen core graph had passed165 targets but lacked that
case; a new red test failed, the source was corrected and every final graph rerun.
String fields, relationship ownership and enum guards were not loosened.

Declared counts are checked against actual children and never drive allocation.
Record field counts, shared indices and definition/record counts are checked.
Unsupported/malformed optional pivot metadata is reported while original XML and
ordinary cells remain available. Existing package/worksheet ownership guards
still apply: a duplicate worksheet relationship ID can reject the whole malformed
input before the optional pivot catalog is read. This stricter existing guard
was retained after a draft regression incorrectly expected a local diagnostic.

Metadata trees and streamed records have explicit depth/node/actual-value limits;
workbook-wide interpreted values, fields and table/declaration counts are also
bounded. Beyond those limits the original parts remain preserved rather than
being interpreted as different data. Limits and exact source hashes are recorded
in [pivot-reader-verification.json](pivot-reader-verification.json). These are
bounded reader views, not a large-production-workbook equivalence claim.

The empty catalog is omitted from serialization for existing nonpivot consumers.
Cache/table payloads are shared by `Arc`, so complete edit checkpoints retain the
same immutable payload instead of duplicating cache records. Ordinary edits,
save/reopen, Undo/Redo and refused compound editing preserve pivot/cache XML bytes
and saved report values. Editing source data does not pretend to refresh a saved
pivot. Existing unsupported-formula cache policy still preserves GETPIVOTDATA;
there is no placeholder registration or claimed new evaluation path.

## Evidence and next calculation work

Seventeen reader cases cover typed/shared records, unused/unknown item positions,
strict/aliased/prefixed namespaces, owned relationships, duplicates, grouping and
filter metadata, foreign wrappers, malformed ranges/counts/indices/parts, external
source descriptions and actual declaration/depth bounds. Four headless cases
cover byte-preserving edits/save/history, source edits without false refresh,
checkpoint payload identity and unsupported formula caches. Two shared-model
cases exercise saved report/history and returned-error rollback.

The original deterministic
[fixture generator](../../scripts/xlsx/generate_pivot_fixture.py) creates source
East10+5 and West20+7, saved report15/27/42, date values, a historical unused shared
item and declared relationship/content-type parts. Regeneration is byte-identical.
This generated fixture is not an Excel schema/repair/save/render oracle. Pinned
ibisOffice pivot reader/writer boundaries and GETPIVOTDATA routing were inspected
for behavior; no reference implementation code was incorporated.

| Final check | Result |
|---|---|
| Six implementation packages, library/integration/doc targets | 1,078 passed, 165 targets |
| API/CLI/MCP/diff and standalone native renderer | 194 passed, 19 targets |
| Combined eleven XLSX packages | 1,272 passed, 184 targets |
| Complete shared OPC unit suite | 22 passed |
| Shared model, twenty-seven tiny-skia `*_like_excel` targets | 144 passed |
| Actual native production/test compilation | Passed, Rust 1.93.1 |
| Actual wasm32 production compilation | Passed, Rust 1.99.0 |

Counts replace the prior checkpoint. Earlier unchanged-engine runtime/patch/
function-inventory evidence is carried rather than rerun. Model Skia-only print/
Thai and other feature/unit combinations remain excluded. Actual keyboard/pointer/
paint and Excel/reference producer/save-repair/calculation oracles remain open.

Microsoft defines GETPIVOTDATA as retrieving
[visible PivotTable data](https://support.microsoft.com/en-us/excel/functions/getpivotdata-function),
including field captions/root names, optional field/item pairs and typed values.
The next calculation step must map the saved row/column/page descriptors and
visible result cells into engine context, then implement real registry/parser/
evaluation/arity behavior with cross-sheet, filter, subtotal, date/error and
save/history tests. Summing arbitrary source rows or returning a registry stub
would not establish that behavior.

Derived/grouped/OLAP/calculated record semantics, all producer layouts, pivot
refresh/layout/filter/authoring, structural/clipboard ownership, slicers/timelines,
queries and the remaining [full audit](ibis-parity.md) are incomplete. This reader
foundation does not establish full ibisOffice parity.
