# Independent worksheet object protection

`sheetProtection` can protect worksheet contents, objects and scenarios
independently. Excel's [Protect parameters](https://learn.microsoft.com/en-us/office/vba/api/excel.worksheet.protect)
distinguish those operations. Previously, `sheet="0"` discarded the record from
Jade's cell-oriented `Sheet::protection`, and every object guard checked that
optional cell state. An imported objects-only lock therefore allowed editing.
The first runtime control reproduced a forbidden note being written successfully.

## Current behavior

The reader's new `sheet_protection::read` returns a `ProtectionState` containing
the independent content flag and all stored operation permissions. It uses the
same worksheet projection as ordinary cell reading: Transitional, Strict,
unbound legacy XML, namespace aliases, direct ownership and supported
Choice/Fallback selection. Foreign and nested lookalikes are ignored. Selected
duplicate records and malformed booleans are rejected. `Sheet::protection`
continues to mean cell protection, so an objects-only record does not suddenly
lock cells, selection, formatting or worksheet structure.

The headless `worksheet_protection` query returns the current selected state.
Private worksheet state caches it together with editable-range metadata;
ordinary edits, structural changes, authenticated protection changes, sheet
rename, undo/redo and reopening refresh or restore that cache. Object gestures
do not repeatedly unzip/project the whole worksheet just to check permission.
This read-only query also exposes scenario permissions without providing the
missing scenario editing feature. The existing authoring query remains separate
and retains its explicit MCE mutation refusal.

All supported object-writing commands now check the independent object lock:

- Create/replace/delete a legacy note.
- Move/resize/delete shapes, charts and pictures.
- Change shape text.
- Insert a chart or change its title, type, legend or data.

These checks run before a write. Refusals leave source, model, dirty state,
layout revision and undo/redo intact. A lock on one worksheet does not affect
another. `objects="0"` permits these commands even when cells are protected;
scenario-only protection does not lock objects or cells. Object-only protection
continues to allow ordinary cell edits. Existing shared native/browser commands
use these headless paths. The existing sheet-protection dialog already recognizes
objects/scenarios-only configurations and authenticates their removal; the new
model tests exercise that flow and its history.

## Evidence and remaining work

Two added reader cases cover independent flags, defaults, namespaces, ownership,
MCE selection, malformed flags and duplicates. Six headless cases cover every
guarded command family, three picture anchor kinds, independent allowed
operations, unchanged rejected writes, authentication, history, reopen and
cross-sheet identity. Two shared-model cases cover object/note commands without
dirtying and the existing masked password dialog, failed/correct authentication,
Undo and retained Redo. Final gates passed 1,214 cases across 176 targets in eleven XLSX packages,
21 OPC unit cases and 106 cases across twenty-two named tiny-skia model targets.
Native production/test and browser production compilation passed. Other model
targets and Skia-only print/Thai cases remain excluded. Gate scope and hashes
are in
[object-protection-verification.json](object-protection-verification.json).

The current object policy is sheet-wide. Individual DrawingML
[`clientData/fLocksWithSheet`](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.spreadsheet.clientdata?view=openxml-3.0.1),
VML object locks and text-lock flags are not yet used to permit particular
objects. Excel's [Shape.Locked](https://learn.microsoft.com/en-us/office/vba/api/excel.shape.locked)
documents that distinction; unlocked individual objects currently remain too
restricted when the sheet's object lock is on. General DrawingML namespace/MCE
ownership, object selection/gesture feedback and native/browser interaction
oracles also remain open. No scenario editor or Office application oracle was
added. Automatic editable-range authentication prompts, encryption, other
protection gaps and the wider [ibisOffice audit](ibis-parity.md) remain active.

The later [individual-object milestone](individual-object-protection.md) supersedes
the sheet-wide overrestriction described here for supported direct DrawingML
objects and VML notes. Broader ownership/MCE and selection feedback remain open.
