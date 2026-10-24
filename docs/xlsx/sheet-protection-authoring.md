# Worksheet protection, cell locks and hidden contents

Worksheet tabs now expose **Sheet Protection…** in both native and browser
shells. The shared password dialog supports optional passwords, confirmation,
masked inputs, wrong-password feedback and fifteen permission toggles. The
cell context menu also exposes cell-lock and hidden-formula toggles. A sheet
dialog remains bound to the sheet on which it opened; changing the model's
selected sheet cannot silently retarget its Apply action. Cancel writes nothing.

## Package and authentication behavior

The preserving writer edits the selected worksheet part, using its actual
part name from the reader/headless model. It inserts `sheetProtection` after
cell data/calculation properties and before later worksheet elements. Strict,
Transitional and namespace aliases are supported. Unknown attributes,
extension children, foreign protection records and other package parts are
retained. Authenticated removal clears sheet/object/scenario locks and their
password attributes while retaining other permission and opaque metadata.
An already configured verifier cannot be replaced without removal first;
passwordless/no-op removal does not invent credentials or consume history.

New password verifiers use the shared SHA-512/100,000-iteration primitive and
fresh platform randomness described in [workbook authoring](protection-authoring.md).
Legacy and imported ISO verifiers use the same tested verification policy.
Incomplete, malformed or unsupported modern metadata never falls back to a
matching legacy verifier. Wrong authentication leaves source, values, selection
and undo/redo unchanged. The low-level writer accepts an explicit 16-byte salt;
the shared model obtains a secure random salt and propagates randomness failure.

Malformed XML Schema booleans and duplicate owned sheet-protection records
are rejected. The reader keeps its existing namespace/MCE projection policy;
nested/foreign protection cannot replace the selected standard sheet record.
The preserving writer refuses protection inside a root-level MCE
`AlternateContent`, rather than inserting a competing editable record. General
MCE protection authoring remains open.

## Cell formats and UI behavior

`FormatChange` now carries optional `locked` and `formula_hidden` fields.
Formatting appends/reuses an xf, preserves other protection/foreign attributes
and children, and writes protection before `extLst` with `applyProtection=1`.
Changing one property retains the other effective property. Cell formats
without a protection child can inherit their named-style protection; an
explicit cell protection record retains its stored values. Named styles with
`applyProtection=0` do not apply their protection. This follows the distinction
in Microsoft's [xf implementation notes](https://learn.microsoft.com/el-gr/openspecs/office_standards/ms-oe376/59922f8b-0edc-4e93-a822-9f22254aec46);
Office application inheritance/update oracles are still required.

When the sheet is protected, hidden cell contents are blank in the formula
bar/edit initializer. Calculated grid values and stored formulas remain in the
workbook. This applies to numbers/text as well as formulas, and resolves cell,
row and column formats for blank cells too. Unprotect/undo/redo restores the
corresponding visibility. The [cell protection specification](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.protection?view=openxml-3.0.1)
defines that display behavior; this is editing protection, not encryption.

The [sheet-protection attributes](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.sheetprotection?view=openxml-3.0.1)
include selection, formatting, insertion/deletion, hyperlinks, sorting,
autofilter, pivots, objects and scenarios. The reader exposes all fifteen
permissions with XML defaults; new UI protection permits selecting cells and
initially disallows the other operations. Existing headless guards enforce
permissions for supported operations. Stored pivot/scenario permissions do not
provide the missing pivot/scenario feature implementations.

Selection checks include default styles, individual cells, row and column
bands. They cover ordinary central movement, pointer/header presses and drags,
Select All and named/internal-link range assignments in both shells. Sparse
bands avoid enumerating a whole worksheet selection; restricted vertical arrows
jump over absent forbidden rows. Manual native/browser interaction, merged-cell
selection and broader platform/IME/clipboard oracles remain required.

## Verification and remaining work

The initial reader probe passed its ownership control but reproduced accepting
`sheet="yes"` as unprotected. The corrected probe rejects malformed flags and
duplicates. Reader cases also cover aliases, defaults, inherited/explicit cell
protection and foreign style children. Writer/headless cases cover strong and
passwordless authoring, independent imported SHA-256, legacy precedence,
object-only credentials, preserving XML, unsupported/MCE rejection, permission
guards and history. Shared-model cases cover dialog state, sheet identity,
hidden formula/value persistence, row/column defaults, sparse selection and
history. Final gates passed 1,184 cases across 172 library/integration/doc
targets in eleven XLSX packages, 19 OPC unit cases and 97 cases across twenty
named tiny-skia model targets. Both native production/test and browser
production compilation passed. These scopes exclude other model targets and
the Skia-only print/Thai cases. Source hashes and exact gate scope are in
[sheet-protection-verification.json](sheet-protection-verification.json);
counts replace the preceding checkpoint rather than adding to it.

The subsequent [editable-range follow-up](editable-ranges.md) adds range
authoring/authentication, structural reconciliation and session unlocks.
The [object-protection follow-up](object-protection.md) adds independent
worksheet object guards. Automatic edit-triggered prompts, account ACLs, individual
DrawingML/VML object locks, scenario editing, encryption, additional digest/locale behavior, all merged/compound
selection paths and Office save/repair/rendering oracles remain open. The complete [ibisOffice audit](ibis-parity.md) remains
active, including pivots, slicers, queries, advanced charts and remaining
functions. Reference implementation behavior was inspected; no implementation
code was incorporated.
