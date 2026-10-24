# Scoped sparkline reading

The reader now follows the worksheet extension hierarchy and the resolved
x14/xm namespaces when interpreting sparklines. A foreign or misplaced
`sparklineGroup`, colour, formula or display reference cannot replace a live
group or one of its values. Prefix aliases, local default namespaces and
Strict worksheet namespaces are supported. Existing namespace-free worksheet
fixtures remain supported with namespaced x14/xm extensions.

Group colours come from direct x14 children. Individual sparklines belong to
the group's direct x14 `sparklines` collection; their formula and display
reference come from direct xm children. CDATA, text, numeric/predefined entity
references, comments and processing instructions no longer truncate a text
leaf. Foreign nested text is excluded.

The display reference must resolve to one cell inside the worksheet bounds.
Missing, malformed, multi-cell and out-of-grid references are omitted from the
interpreted model instead of silently placing a chart at A1. A1 itself remains
a valid destination, as do absolute references and one-cell ranges such as
`D1:D1`. Raw package preservation and interpreting a record are
separate operations.

The [MS-XLSX CT_Sparkline definition](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/6b28a993-e0fd-451d-860e-35097c6baa77)
requires the destination to identify one cell. The
[group definition](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/331c1ecb-e49d-4cd0-843d-3ae189dc8a88)
and [individual collection definition](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/b52ae477-f11f-45c1-8327-2089fb9da4c4)
describe the child hierarchy. These descriptions guided the reader boundary;
the regression tests are separate from application oracle evidence.

## Reproduced cases

The first six reader cases produced one pass and five failures with the old
parser: foreign/misplaced groups were visible, wrapped colours overwrote the
real colour, foreign formulas/references replaced the real source/destination,
CDATA disappeared, invalid destinations defaulted to A1 and nested groups
replaced the active group. All six passed after the ownership/text changes.
The Strict/default-namespace case passed before and after.
An additional case reproduced dropping one-cell ranges after the initial
scoping change. Resolving a range and checking that both endpoints identify
the same cell fixes that case without accepting multi-cell destinations.
All seven reader cases pass with the final decoder.

The headless integration exercises both Transitional and Strict packages,
owned CDATA formula/display text, exact foreign-XML preservation, cross-sheet
cell shifts, save/reopen and exact package/model Undo/Redo. Existing conditional
format, sparkline, table and input-rule ownership cases run alongside it.

## Limits

This is a reader correction, not sparkline create/edit/group/style/axis UI.
It does not implement a general MCE Choice/Fallback processor, date-axis
rendering, every source-formula grammar, external-workbook refresh or full
schema validation. Existing behavior omits data-less individual sparklines
from the model; the source package remains available. No Excel repair/save,
calculation or pixel oracle was run for this milestone. The subsequent
[cell-content pass](cell-content.md) addresses standard cell, shared-string,
rich-run and phonetic ownership/selection. Whole-axis and sheet-rename/delete
passes, other extension families and the
broader [parity audit](ibis-parity.md) remain open.

## Final checkpoint

The final frozen sources passed 926 core tests across all 145 targets and 25
shared-model tests. Native production/test and browser wasm compilation passed.
The seven new reader cases and the new headless save/history case are included
in the core total. The prior engine source and its validation are unchanged.
Results, 120 file hashes, interruption/retry details and remaining evidence are
recorded in [sparkline-reading-verification.json](sparkline-reading-verification.json).
