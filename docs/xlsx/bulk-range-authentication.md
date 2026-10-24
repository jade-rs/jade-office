# Bulk editable-range password prompts

Paste, clear, Ctrl/Command+Enter range entry, directional fill, series fill and
range move/copy now use a shared automatic editable-range password dialog in the
native and browser shells. The original guarded command is attempted first; a
prompt appears only when supported passwords can cover all required locked cells.
Unsupported or uncovered cells retain their guarded refusal.

The shared command describes actual writes. Copy/fill source cells are read-only;
cut/move source cells require permission as well as their destination. Clear
checks stored nonempty values/formulas rather than blank cells in its selection.
Ragged TSV excludes unwritten holes in its initial block; tiled output includes
all actual destinations. Whole-axis clear stays sparse. Frozen internal clipboard
geometry is used, and same-named ranges on different worksheets have distinct
part identities and sheet-qualified labels.

Passwords are verified without adding live session grants. The dialog removes
already-covered alternatives and asks for the remaining required ranges. Wrong
passwords and cancellation leave workbook bytes, dirty/layout state, undo/redo,
clipboard and live grants unchanged. An original Ctrl/Command+Enter entry remains
in its editor while the prompt is pending, and its active cell/text are checked
before continuation.

After complete coverage, all proofs are revalidated and the grants plus original
command commit inside one editing checkpoint. A returned error rolls back both
new grants and the command. Pending work is bound to its open workbook, sheet
part/layout revision, source/authentication generation and clipboard revision;
intervening edits/history, unlock/relock, worksheet navigation or clipboard
replacement invalidate it. No historical whole-workbook snapshot is retained
across human password entry. Password plaintext is absent from deferred proofs.

Input validation keeps the command's existing behavior: typed active-cell entry
checks its rule; copied/filled destinations follow Excel's documented bulk
semantics. Microsoft explicitly states that validation messages do not appear for
[copied or filled data](https://support.microsoft.com/en-us/excel/more-on-data-validation).
A draft regression incorrectly assumed otherwise and was corrected after checking
that source. A separate invalid-formula-XML write tests real final-writer failure
and complete grant/source/history rollback; no existing validation assertion or
implementation check was relaxed.

## Verification

| Final check | Result |
|---|---|
| Six implementation packages, library/integration/doc targets | 1,057 passed, 163 targets |
| API/CLI/MCP/diff and standalone native renderer | 194 passed, 19 targets |
| Combined eleven XLSX packages | 1,251 passed, 182 targets |
| Complete shared OPC unit suite | 22 passed |
| Shared model, twenty-six tiny-skia `*_like_excel` targets | 142 passed |
| Actual native production/test compilation | Passed, Rust 1.93.1 |
| Actual wasm32 production compilation | Passed, Rust 1.99.0 |

Four new headless cases cover pure credential staging, coverage, origin/history/
relock staleness, all-or-nothing proof application and checkpoint restoration.
Sixteen new model cases exercise the exact shared shell attempt/dialog/retry path:
multi-password/tiled text/internal-copy paste, cancellation and existing redo,
clear/full axes, ragged TSV, read-only sources, all fill variants, original-entry
binding, returned writer failure, range move/copy, cross-sheet cuts with equal
range names, clipboard/navigation/replacement staleness, overlapping alternatives,
live grant changes and uncovered/invalid requests. Both shells use the same path
for their menus, shortcuts and pointer commands.

Counts replace preceding checkpoint counts. Exact source hashes, targets and
compiler evidence are in
[bulk-range-authentication-verification.json](bulk-range-authentication-verification.json).
The model run excludes the established Skia-only print/Thai cases and other
model/unit/feature combinations. Earlier engine runtime, patch-replay and function
inventory evidence is carried with unchanged source hashes, not rerun here.

Actual keyboard/pointer/paint interactions, Excel/reference application captures,
merged/compound selections, broader clipboard/spill/table/schema behavior,
pending-navigation replay and prompts for other command families remain
unverified or incomplete. Panic/process/allocation-failure recovery and large
production performance are outside this returned-error contract. The full
[ibisOffice parity goal](ibis-parity.md), including pivots, slicers, queries,
advanced charts, six missing function registrations and encryption, remains active.
