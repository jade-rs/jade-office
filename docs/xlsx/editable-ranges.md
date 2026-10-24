# Editable ranges and session authentication

Both native and browser worksheet/cell menus expose **Editable Ranges…**.
With sheet protection off, the shared manager creates, renames, resizes and
deletes ranges. Existing credentials stay intact until the explicit password
change option is selected; confirming an empty replacement removes them.
New passwords require confirmation and use SHA-512, 100,000 iterations and a
fresh 16-byte platform-random salt. Randomness failure is returned to the
caller. Range references can contain multiple areas or complete rows/columns.

With sheet protection on, the manager authenticates individual named ranges
and can relock all authenticated ranges on the current sheet. Authentication
and relocking do not change source bytes, dirty state or undo/redo history.
Grants are live-session state and disappear when the document is reopened.
The manager pins the worksheet part, document revision and protection state;
a stale action cannot silently apply to a different sheet or revision.
Cancel writes nothing. Password inputs are masked and cleared after use.

## Reading and preserving writes

The reader recognizes direct standard `protectedRanges/protectedRange`
elements in Transitional and Strict worksheets, including namespace aliases.
It also recognizes x14 records under the specified worksheet extension URI
`{FC87AEE6-9EDD-4A0A-B7FB-166176984837}`, with one direct `xm:sqref` leaf.
Foreign, nested and wrong-extension lookalikes cannot grant edit permission.
Invalid, reversed, out-of-grid or external-sheet references do not become
passwordless exceptions. Unsupported MCE representations are refused for
range permissions and mutation; general MCE range authoring remains open.

Standard and x14 records with the same sheet-local name are grouped only
when their coverage is geometrically equivalent. Repeated records in the
same family or conflicting coverage are ambiguous and cannot unlock cells.
When modern credentials exist in either representation, passwordless or
legacy attributes in the other cannot weaken them. Malformed or unsupported
modern credentials are errors. Equivalent unions are compared by sparse
rectangle subtraction rather than expanding complete columns into cells.

The preserving writer edits the worksheet's actual part name. It updates
both representations together and retains credentials unless replacement
was explicitly requested. It preserves unknown attributes, descriptors,
foreign children, unrelated extension records and other package parts.
Deletion prunes empty plain owned containers and their now-empty owned
extension wrappers, while retaining opaque content. Semantically unchanged
name/coverage updates with retained credentials return the original bytes.
Definitions can be authored or deleted only while sheet protection is off.

Whole-row/column insertion and deletion and partial cell shifts reconcile
both representations through the same range mapper. Partial movement can
split coverage; deleted or clipped-away coverage removes the corresponding
definitions. Generic reference walkers skip protected-range subtrees so that
opaque extension payloads are not accidentally interpreted as local ranges.
MCE mutation is refused instead of writing competing range definitions.

## Authentication and editing

Locked cells are editable when covered by any valid passwordless range or
an authenticated range. Overlapping ranges with different names use the
union of permitted coverage; batches still reject atomically if any required
locked cell is forbidden. Normal unlocked cell styles continue to work.
Per-range grants do not remove worksheet protection or change its other
operation permissions, selection restrictions or hidden-content behavior.

The standard permits passwordless ranges, including zero legacy verifiers,
regardless of security descriptors. Account/ACL identity authentication is
not implemented. Descriptors are preserved, and password-bearing ranges
still require authentication. See the [standard range definition](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.protectedrange?view=openxml-3.0.1).

Range ISO authentication separately supports the Office interpretation that
hashes the legacy verifier as a raw UTF-16 unit, as well as hashing the direct
password. The complete modern verifier is checked in both cases: a matching
legacy attribute alone never bypasses it. Existing workbook/sheet verification
policy is unchanged. Independently generated SHA-256 vectors cover an ASCII
password, a CP932 password and a legacy value in the unpaired-surrogate range.
See [MS-XLSX CT_ProtectedRange](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/85f5567f-2599-41ad-ae26-8cfab23ce754)
and the [extension mapping](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/07d607af-5618-4ca2-b683-6a78dc0d9627).

Grants are scoped to worksheet part, logical name, credential/descriptor
metadata and sheet-protection metadata. Ordinary cell undo/redo retains
valid grants; structural movement follows the same logical range. Sheet
rename keeps part identity. Deleting the range or sheet, changing credentials
or removing sheet protection revokes grants. Undo of a deleted definition
does not resurrect its old authentication. Equal names on different sheets
never share grants.

## Evidence and remaining work

The initial headless control reproduced a locked cell being refused despite
a valid passwordless range. The corrected implementation passes that control.
New cases cover reader ownership/coverage, preserving writer CRUD and shifts,
authentication/session history, shared dialog behavior and crypto vectors.
Final gates passed 1,206 cases across 175 targets in eleven XLSX packages,
21 OPC unit cases and 104 cases across twenty-one named tiny-skia model targets.
Native production/test and browser production compilation passed. Other model
targets and Skia-only print/Thai cases are excluded. Gate scope and source hashes
are recorded in
[editable-ranges-verification.json](editable-ranges-verification.json).

Automatic authentication prompts triggered by attempted edits remain open;
authentication currently uses the manager. A pending invalid cell edit may
need to be cancelled before opening it. Full account/ACL authentication,
general MCE, individual DrawingML/VML object locks, scenario editing, all
merged/compound selection paths, large-file performance and broader
digest/locale behavior still require work. Native/browser interaction and
Excel save/repair/rendering oracles were not performed. The wider
[ibisOffice audit](ibis-parity.md) remains active, including pivots, slicers,
queries, advanced charts, encryption and the six remaining function
registrations. Reference behavior was inspected; implementation code was
not incorporated.

The subsequent [object-protection follow-up](object-protection.md) adds
independent worksheet object guards and caches selected permission metadata.

The later [range-prompt milestone](range-unlock-prompts.md) adds automatic
password prompts for ordinary typed single-cell commits in both editors.
Bulk-operation prompts and pending-navigation replay remain open.
