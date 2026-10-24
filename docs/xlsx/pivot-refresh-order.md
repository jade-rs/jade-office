# Pivot refresh dependencies

Refresh-all now chooses cache order from observed source dependencies rather
than relying on saved report order. A reverse-declared consumer that previously
kept an old total of 27 now receives 117 after its producer refreshes. Source
and report updates still commit as one Undo/Redo step, with full rollback on
failure.

## Dependency evidence

The calculation engine offers optional, model-owned cell/range tracing.
Ordinary calculations allocate no trace. The adapter requests it only for
dependency-aware refresh and receives references from the same evaluation that
supplies source values. Named expressions, dynamic references, structured
references, GETPIVOTDATA and spill-output/anchor reads follow their normal
calculation paths. Formatting formulas are excluded. Range rectangles include
empty cells; contained cell reads compact into an existing rectangle rather
than creating a dependency for every coordinate.

Source bounds use the refresh writer's existing resolver for direct worksheet
ranges, scoped/global name aliases and tables. The scheduler walks observed
references transitively through sparse formula cells and matches them against
report footprints. A cache is ready only after its observed predecessors.
Unrelated ready caches retain saved-order tie breaking.

The graph is rebuilt after each staged refresh. Completion timestamps invalidate
an earlier consumer if a predecessor was refreshed later or became pending.
For example, a source reading a formerly empty cell can acquire a dependency
when another report grows into it; the consumer then refreshes again. Shared
caches remain deduplicated as targets, though newly discovered dependencies can
require another refresh of an invalidated target.

Cycles, including dynamically introduced cycles, refuse the entire batch.
An incomplete trace or exhausted work/retry budget also refuses without changing
bytes, model state or history. Office/reference behavior for cyclic cache sources
still requires oracle coverage. Independent volatile sources do not require
numeric value convergence. The final traced calculation already supplies the
committed formula caches, so there is no extra volatile evaluation between graph
validation and commit.

## Limits and reference comparison

This is an observed, conservative dependency graph. Reference-only operands
can overapproximate value dependencies. Unread branches are revisited if later
calculation activates them; this is not an all-branches static dependency oracle
or Excel calculation-chain serialization. Volatile formulas can produce new
samples on later calculations; refresh consumes a source snapshot rather than
claiming permanent equality between a pivot cache and volatile source cells.

The trace retains at most one million distinct edges per evaluation after
rectangle compaction. A batch allows twenty million dependency work steps and
at most `min(N*(N+1), 16384)` cache refreshes for N distinct targets. These are
resource constraints, not Excel compatibility claims. Cross-workbook providers,
general asynchronous data connections and query Refresh All remain unimplemented.

The pinned [ibisOffice refresh command](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/pivot.rs)
was inspected for behavior only; no AGPL implementation was copied. Jade's
earlier saved-order batch is documented in [refresh all](pivot-refresh-all.md).
Full pivot styles/autofit, PivotChart synchronization, automatic load refresh,
creation/deletion, source/layout/filter authoring, grouping/calculated/show-as,
OLAP, slicers and timelines still need work.

## Verification

Current replacement results: **1,350 XLSX cases across 189 targets** (1,156 /170
in the six implementation packages and 194 /19 in API/CLI/MCP/diff/native
renderer), 22 OPC cases, 151 shared-model cases /28 named tiny-skia targets, and
2,246 actual engine cases with eight existing ignored. Actual native production/
test and browser production compiler graphs passed locked/offline.

New coverage comprises nine headless cases, three calculation cases, one shared
model case and four engine cases. Reverse order and missing cycle detection were
first reproduced as failures. Tests then cover dynamic/named/structured/spill
dependencies, direct report and named/table sources, empty-cell footprint growth,
dynamic cycles, volatile sources, trace isolation/budgets/compaction and shared
model refusal. The refresh targets now contain 38 headless and five model cases.

The final snapshot covers 315 scoped source/build/corpus/provenance hashes and
28 model target hashes; 196 scoped changed/new Rust files pass formatting. The
regenerated engine patch replays 489 source/build files against the hash-pinned
original 480-file inventory. The actual registry remains 507 of the pinned
reference's 512 names. Detailed evidence is in
[pivot-refresh-order-verification.json](pivot-refresh-order-verification.json).

Counts replace earlier checkpoints. Full producer/Excel repair/save/pixel and
native/browser pointer/keyboard oracles, Skia-only cases and the entire model
feature/unit matrix remain outside this selection. Full ibisOffice parity
remains an active goal.
