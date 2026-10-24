# Compound paste and fill atomicity

Tiled text/internal-copy paste and Ctrl/Command+Enter range fill now restore the
entire editing state when a later stage refuses the command. Previously the
origin/active cell could remain changed even though the command returned an
error, with no shared-model history or dirty-state update. The original runtime
probes reproduced both failures: A1 changed from 1 to 42 before protected A2
refused the remaining write.

A separate runtime probe showed a cut could clear a locked source when its
destination was editable. Headless cut-paste now checks both source and
destination permissions, including cross-sheet sources. Copying locked source
cells remains a reading operation. The existing dense clipboard limit is checked
before permission work for dense rectangles. Complete axes retain their sparse
writer path; permissions use style/default/authenticated-range bands and explicit
cell exceptions, including blanks, instead of visiting billions of cells.

The pinned ibisOffice [clipboard implementation](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/clipboard.rs)
was inspected for its paste permission gate and batched document-command paths.
No reference implementation code was incorporated. Source inspection is separate
from the actual Jade regressions above and does not establish reference UI parity.

## Editing checkpoint

An opaque `EditCheckpoint` captures the complete headless in-memory state:
source and interpreted model, worksheet permission cache, unsaved range grants,
clipboard origin identity, automatic row-height base, calculation/source time
zones, filter-row state and both history branches. It can only be restored to
the open workbook that created it. Restoration replaces that state directly;
it does not reopen the package, recalculate or reconstruct grants/history.

Immutable history entries are held by `Arc`, so taking a checkpoint shares their
payloads instead of copying up to one hundred workbook snapshots. Current
source/model state is copied once; a pending automatic row-height base can also
require a source copy. Frozen internal clipboard bytes use `Arc<[u8]>`, keeping
clipboard checkpoints cheap while retaining their original data and identity.
This is a correctness mechanism with bounded existing operation limits, not a
large-production-workload performance certification.

The shared model wraps `paste_over` and `fill_with` in scoped rollback. On a
returned error it restores the engine, active sheet/layout/extent, dirty and
layout revision, grouped undo/redo, internal clipboard and pending paint-change
notifications. These command paths do not mutate painter caches. Invalid paste
rectangles and fill selections are rejected before writing. Successful commands
retain their existing recalculation, formatting and one-action undo grouping.
The native and browser shells use these same shared-model methods.

## Evidence and limits

The original two corruption probes fail against the preceding implementation
and pass after this change. A third probe reproduced an incorrectly accepted
protected-source cut. Focused headless cases cover exact source/model and redo
restoration, clipboard identity, rejection of a different workbook, history
retention across the one-hundred-entry eviction limit, unsaved grants, same-sheet
and cross-sheet cut guards and huge ranges. Shared-model cases cover refused
text/internal-copy paste, a retryable refused cut, preserved existing redo,
percentage formats in a successful grouped fill, saved/reopened successful paste,
late size refusal, invalid selections and unchanged paint notifications.
An initial full core regression exposed a blanket size-limit regression in four
existing whole-axis clipboard/dimension cases. Those assertions remain unchanged;
the corrected dense-only bound and sparse permission checks retain that support.
Additional cases cover a far locked-cell exception on an otherwise unlocked
whole sheet and a single-cell password gap between allowed column bands.
Final gates passed 1,247 XLSX cases across 181 targets, 22 OPC unit cases and 126
shared-model cases across twenty-five named tiny-skia targets. Native production/
test and browser production checks passed. Final source hashes and gate scope are in
[bulk-edit-atomicity-verification.json](bulk-edit-atomicity-verification.json).

This rollback contract covers returned failures in these supported compound
paths. It does not claim recovery from process termination, panic or memory
allocation failure, nor general transactions for every editor operation.
The later [bulk-authentication milestone](bulk-range-authentication.md) adds
automatic prompts for supported paste/clear/fill/move commands. Interrupted-navigation
replay remains open. Clipboard/filter/spill/table/schema edge cases, account identity,
full model/feature matrices and actual Excel/native/browser interaction oracles
remain part of the wider [ibisOffice audit](ibis-parity.md).
