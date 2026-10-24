# Explicit pivot refresh

Jade can rebuild supported imported worksheet pivots from current source cells.
The native and browser context menus expose **Refresh PivotTable** inside a
report; both call the same shared-model command. Source edits alone retain the
saved report. Refresh is explicit and commits the rebuilt cache, every report
sharing it, dependent calculations and history as one undoable operation.

## Behavior implemented

- Worksheet ranges, scoped/global defined-name aliases and named tables resolve
  to current source rows. Table totals are excluded. Source headers must still
  match the existing cache fields; adding/remapping fields requires authoring.
- Existing shared-item indices, including unused members, stay stable. New
  typed members append, cache records rebuild, and missing records parts receive
  owned relationships and content types. Cache refresh/save flags update.
- Row/column axes, selected page items, manual hidden-member filters, label
  ordering, grand totals, default subtotals, multiple measures and compact row
  hierarchy rebuild. Measures on rows work in tabular layouts; compact values
  on rows and mixed compact/tabular layouts are refused.
- All eleven standard aggregate kinds are implemented: sum, count, countNums,
  average, min, max, product, var, varp, stdDev and stdDevp. Typed errors and
  blanks follow aggregate-specific behavior. Unsupported derived/show-as,
  grouped, calculated, external and advanced-filter layouts are refused.
- Date members retain typed indices across equivalent ISO spellings and both
  workbook epochs. Numeric source styles determine dates; General remains
  numeric. Fractional dates, the fictional 1900 leap day, number formats and
  inferred saved measure captions are covered. New member captions preserve
  tabs, line feeds and literal OOXML escape sequences.
- Existing cell styles are retained; measure/date formats and compact-row
  indentation are materialized. This does not reapply the complete pivot style,
  resize columns or reproduce every new-member banding/theme rule.

The writer requires explicit proof of successfully evaluated source formula
cells. Headless refresh stages recalculation and verifies formulas only when
that calculation completes without skips. An unverified formula source is
refused; literal sources can refresh even when unrelated formulas are skipped.
Returned errors restore workbook bytes, calculation state, history and grants.

Before committing, all reports sharing the cache are planned together. Report
collisions with occupied cells, another pivot, source data, tables, merges and
array formulas are refused. Existing sparse report footprints are cleared by
stored cells rather than expanding a potentially enormous empty rectangle.
Source/output/cache/edit budgets and aggregate-work limits bound extra work.
These are implementation limits, not Excel compatibility claims.

Owned transitional/strict namespace paths, qualified prefixes, foreign
lookalikes and opaque retained extensions are handled conservatively. Unknown
metadata in rebuilt entries causes refusal rather than silent deletion.
Generated output is reread to catch new catalog inconsistencies; that check
does not establish schema validity or Excel repair-free round-tripping.

## Reference comparison and remaining work

The pinned [ibisOffice pivot command implementation](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/pivot.rs)
was inspected for behavior only. No AGPL implementation was copied. Its refresh
path also reapplies pivot styles, autofits and synchronizes PivotCharts. Jade's
ordinary chart-cache update is not PivotChart binding synchronization.

Creation/deletion, source/layout/filter editing and automatic
refresh-on-load execution, grouping, calculated fields/items, show-as values,
advanced filters, OLAP/cube contexts, slicers and timelines remain open.
General MCE/extension/producer coverage and page-filter areas outside the
report location also need further work. [GETPIVOTDATA](getpivotdata.md) remains
a visible-report query; refreshed results now flow through that existing path.

## Verification

The replacement checkpoint passes **1,334 XLSX cases across 188 targets**:
1,140 across 169 targets in the six implementation packages, plus 194 across
19 API/CLI/MCP/diff/native-render targets. It also passes 22 OPC cases,
148 shared-model cases across 28 named tiny-skia targets, and 2,242 engine
cases with eight existing ignored. Actual native production/test and browser
production compiler graphs pass under the recorded locked/offline profiles.

New targeted coverage includes 25 headless refresh cases, eight writer cases,
two shared-model cases and one calendar inverse case. Red regressions first
reproduced General/date misclassification, equivalent-date index loss and
missing measure-caption lookup, then verified their fixes. All graphs ran
against the final production sources. A late cross-sheet protection test
changed only that test file; its full 25-case target was rerun and the source
snapshot updated. The checkpoint covers 312 source/build/corpus/provenance
hashes, with 193 scoped changed/new Rust files checked for formatting. Counts replace previous
checkpoints and must not be added together.

Exact target results, hashes, exclusions and evidence are in
[pivot-refresh-verification.json](pivot-refresh-verification.json). No Excel
repair/save/pixel oracle, broad producer corpus or native/browser pointer and
keyboard exercise was performed. Skia-only model cases and the full model
feature/unit matrix remain outside this selection. Full ibisOffice parity is
still an active goal.


The later [refresh-all follow-up](pivot-refresh-all.md) adds atomic workbook-wide
pivot refresh, once per shared cache, from either shell. The verification counts
above describe the selected-refresh checkpoint; use the follow-up for current
source hashes and regression results. Saved declaration order does not establish
general dependency-graph freshness.
