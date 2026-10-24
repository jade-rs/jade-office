# Workbook structure protection and broader verification

Imported `workbookProtection/@lockStructure` now prevents sheet creation,
deletion, rename, reorder and visibility changes in the preserving writer.
The headless editor and shared model use that writer, so callers cannot bypass
the guard through another supported sheet command. Native and browser menu
handlers also refuse these commands before changing selection, scroll position
or opening the rename editor, with a localized workbook-protection message.

The [Microsoft workbook protection guidance](https://support.microsoft.com/en-us/excel/protect-a-workbook)
lists these sheet operations as restricted by structure protection. The
[Open XML workbookProtection definition](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.workbookprotection?view=openxml-3.0.1)
places the record under the workbook and distinguishes structure, windows,
revision and password attributes. The pinned ibisOffice protection module
also exposes a structure guard and separate protection/unprotection panels;
see the [reference comparison](ibis-parity.md). Jade's changes were implemented
independently; reference implementation code was not incorporated.

## Behavior

The reader exposes `Workbook::structure_protected` with a serde default of
false for older serialized models. It follows a direct workbook child in the
Transitional or Strict SpreadsheetML namespace, including local aliases and
legacy namespace-free fixtures. Foreign elements, nested lookalikes and
qualified foreign attributes cannot supply the flag. XML Schema booleans
`true`, `1`, `false` and `0` are accepted with XML whitespace; malformed owned
flags produce a read error rather than silently unlocking the structure.
Duplicate owned records are invalid schema input, but a later false flag
does not erase an earlier true one.

A password attribute by itself, `lockWindows` or `lockRevision` does not lock
sheet structure. Cells and worksheet rows/columns remain editable subject to
the separate worksheet/cell protections. Existing tab-color editing remains
available. No-op rename, move and visibility writes preserve the original
bytes and add no history. A refused change leaves workbook bytes, sheet
order, selected sheet and undo/redo history intact. Password metadata is
retained as opaque source XML, including legacy and modern attribute forms;
that guard checkpoint did not validate or authenticate a password. Subsequent
[workbook authoring](protection-authoring.md) adds authenticated removal and
optional-password structure protection through the model and both shells.

## Evidence and remaining differences

Before the guard, the writer/headless target reproduced eight failures and
two passing controls. The final focused reader/writer targets contain fifteen
cases, including strict/aliased namespaces, malformed flags, hidden-sheet
unhide, all supported structural commands, no-op bytes, normal cell editing,
save/reopen, tab color and atomic undo/redo. Three shared-model cases exercise
imported protection, selected-sheet/history preservation and unlocked controls.
The final counts and exact test/build scope are recorded in
[workbook-protection-verification.json](workbook-protection-verification.json).

Verification was also expanded beyond the six principal implementation
crates to the API, CLI, MCP and diff package suites and the separate native
renderer. The diff suite's seven attributed public workbooks, exact mutation
snapshots and stress cases passed. These are difference-analysis regressions;
they are not a spreadsheet application oracle or a new Excel certification.
The headless round-trip corpus separately already includes these public
workbooks. Native-render tests exercise actual Skia pixels for viewports,
charts, pictures, conditional formatting and filter buttons.

Workbook protect/unprotect and authentication are now covered by the
[subsequent authoring milestone](protection-authoring.md). The
[worksheet follow-up](sheet-protection-authoring.md) adds sheet authentication,
cell locks/hidden contents and permission controls. Editable ranges/session
unlocks, independent objects/scenarios-only guards, workbook-window/
revision guards, encryption and
native/browser runtime captures remain open. General MCE selection around
workbook-protection records and the existing workbook structural writers'
broader namespace ownership also need separate coverage. Menus are guarded;
the original guard milestone did not include protect/unprotect authoring.
The newer dialog still needs manual runtime/complete menu availability evidence. The broader ibisOffice goal remains active.
