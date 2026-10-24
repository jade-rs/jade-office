# Refresh all pivot tables

This records the earlier declaration-order checkpoint. Current
[observed dependency scheduling](pivot-refresh-order.md) supersedes its ordering
limitations and supplies the latest source hashes and regression results.


Native and browser context menus offer **Refresh All PivotTables** whenever
the workbook contains interpreted pivot reports. The command works from any
worksheet or selected cell and calls the same shared-model/headless operation.
It refreshes pivot tables, not external data connections or queries.

Every distinct shared cache is refreshed once. Its reports all rebuild through
the existing [explicit refresh writer](pivot-refresh.md). Sources are calculated
before each cache is consumed, so a later source can use an earlier refreshed
report, directly or through formulas. Intermediate packages stay inside a
complete edit checkpoint without intermediate history entries. Successful
updates commit as one Undo/Redo step, including dependent formula results.

A later collision, protected report owner, unsupported source/layout or
unverified source formula refuses the entire operation. Original bytes,
calculation state and both history branches are restored. No reports is a true
no-op; an uncertain catalog is refused rather than silently omitting reports.

## Comparison and limits

The pinned [ibisOffice command](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/pivot.rs)
updates saved-order reports on a working copy and commits compound sheet
commands. Its style/autofit and PivotChart synchronization still exceed Jade's
current implementation. Source was inspected for behavior only; no AGPL
implementation was copied.

At this checkpoint Jade processed caches in saved report declaration order, deduplicated by cache
ID. This is not dependency-graph scheduling: backward cross-cache dependencies
and cycles are not guaranteed fresh in one pass. Dependency ordering and
freshness validation, including volatile sources across calculation passes,
remain open. The command does not execute automatic refresh-on-load or the
general workbook data-connection Refresh All operation.

All existing refresh restrictions remain: grouped/calculated/derived/OLAP
layouts, advanced filters, unsupported compact layouts and unknown replaced
metadata are refused. Creation/deletion and source/layout/filter editing,
full styles/autofit, PivotCharts, slicers and timelines still need work.

## Verification

Replacement results are **1,338 XLSX cases across 188 targets**, comprising
1,144 across 169 targets in the six implementation packages and 194 across
19 API/CLI/MCP/diff/native-render targets. Also passed: 22 OPC cases,
150 shared-model cases across 28 named tiny-skia targets, and 2,242 engine
cases with eight existing ignored. Actual native production/test and browser
production compiler graphs passed with locked/offline dependencies.

Four new headless cases exercise cache deduplication, separate sources with
intermediate formula calculation, later collision/unsupported-formula rollback
with a preserved redo branch, and no-op/uncertain-catalog behavior. Two shared
model cases exercise invocation away from a report, save/history/revision
updates and refusal without model changes. The complete refresh targets now
contain 29 headless and four model cases. Independent Rust fixtures create the
second cache, records part and source; they contain no reference workbook data.

The final snapshot contains 312 scoped source/build/corpus/provenance hashes
and 28 model target hashes. Eight files changed this milestone. All selected
graphs ran against that snapshot; 193 scoped changed/new Rust files pass
formatting. Registry507 and the 488-file engine patch remain unchanged, and
their actual inventory, regression suite and patch replay were rerun.

Exact evidence is in
[pivot-refresh-all-verification.json](pivot-refresh-all-verification.json).
Counts replace earlier checkpoints. No broad producer/Excel repair/save/pixel
oracle or native/browser pointer/keyboard exercise was performed. The existing
Skia-only and full model feature/unit exclusions remain. Full ibisOffice parity
is an active goal.
