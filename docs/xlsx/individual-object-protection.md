# Individual drawing and note protection

Imported DrawingML objects now use their own `clientData/fLocksWithSheet` for
movement/deletion and `sp/fLocksText` for shape-text edits when worksheet object
protection is active. The corresponding VML note properties are `Locked` and
`LockText`. Object position and text flags are interpreted separately; allowing
text edits does not allow movement/deletion. New objects still require the
worksheet's object permission. When that permission permits editing objects,
the individual flags do not restrict supported commands.

The [ECMA-376 part 1/4 schemas](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
give the DrawingML flags a default of true. The
[VML Locked](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.vml.spreadsheet.locked?view=openxml-3.0.1)
and [LockText](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.vml.spreadsheet.locktext?view=openxml-3.0.1)
definitions also default to locked and treat an empty element as true. VML uses
the [True/False/blank enumeration](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.vml.spreadsheet.booleanentrywithblankvalues?view=openxml-3.0.1),
which differs from XML Schema boolean spelling. Four object/text combinations
are tested through the editing APIs. Excel/native/browser interaction oracles
have not been performed; physical selection and text-edit gestures remain a
separate verification requirement.

## Ownership and references

The shared reader module `object_protection` supplies owned records and original
byte spans to the headless editor and writers. It recognizes direct standard,
Strict and unbound legacy drawing anchors, their matching nonvisual property
containers/IDs and direct client data. Foreign/nested lookalikes do not unlock
an object. Duplicate supported IDs/client data and malformed lock flags cannot
supply an editable record. Existing model identity must agree with the selected
owned record before a headless edit proceeds. General group/member identity,
DrawingML MCE and full drawing-reader ownership remain open.

Worksheet drawing and legacy-drawing references follow the existing selected
namespace/MCE policy. A relationship-only projection skips the grid subtrees
and does not construct cell/span or drawing trees for the worksheet. The reader
and writer now follow the worksheet's selected `r:id`, including attribute
namespace aliases, rather than picking the first drawing relationship. Reference projection also
accepts standard chart/dialog-sheet roots, retaining import of the existing
32-workbook public corpus. This does not add chart-sheet editing UI. Chart
edits additionally verify the owned graphic/chart relationship against the
model's chart part. External, foreign, nested, wrong-type and ambiguous
relationships cannot supply the selected object.

Note edits use the selected VML relationship and the unique owned comments
relationship. An interpreted relationship view ignores foreign/nested records
and rejects duplicate owned IDs or invalid target modes, while preserving
writers retain the original relationship XML. Unused drawing/VML parts and
opaque relationships are not edited. This hardening is scoped to these XLSX
paths; it does not establish complete package ownership for every feature.

## Preserving writes and lifecycle

Moving a drawing replaces only owned position elements and the anchor kind.
Anchor attributes, local namespace declarations, unknown siblings, object
content and client data survive conversion from one-cell/absolute anchors.
Deletion removes the uniquely selected owned anchor. Shape-text writes replace
owned paragraphs, retain foreign siblings and inherit the first owned
paragraph/run formatting, including namespace declarations scoped to the old
paragraph. Empty-line font properties and significant whitespace are retained.

Changing note text leaves an existing VML drawing byte-for-byte unchanged.
Previously it replaced the note box, losing custom geometry and lock settings.
New note shapes and shape types declare their namespaces locally, including
when expanding an empty VML root or encountering conflicting prefix bindings.
Owned note identities use direct Row/Column leaves; entities and CDATA are
handled, and malformed/duplicate metadata does not grant an edit.

Comments now share the ordinary shared-string namespace/MCE, rich-text, CDATA,
entity, phonetic-value and XString interpretation. Authors and comments are
scoped to their standard containers. Note updates preserve root/target metadata
and inherit the first owned run's font. Supported text-level MCE alternatives
are updated together, each keeping its own font and wrapper; unsupported
branches remain opaque. Read-only import also selects MCE authors and records.
Authoring MCE-wrapped root/list/author containers is explicitly refused rather
than appending a competing representation. Literal XString-looking text, controls
and author names are encoded correctly. Self-closing author/list/text containers
can be expanded. Deleting the last note retains an empty comments part when
opaque comments or package registry metadata would otherwise be discarded.

Refusals leave source, model and history unchanged. Individual flags remain in
the package through save/reopen, ordinary undo/redo, structural note movement
and worksheet unprotect/reprotect. Existing shared model commands allow moving
or deleting unlocked drawings and editing unlocked note text; failed creation
of a new protected object does not dirty the document or change layout revision.

## Verification and remaining work

The original headless control reproduced refusal of an individually unlocked
drawing. Focused reader/writer/headless/model checks cover the flags, preserved
XML, namespace ownership, selected/unused parts, note geometry, font scope,
literal XStrings and lifecycle behavior. Final gates passed 1,237 XLSX cases across 179 targets, 21 OPC unit cases and
109 cases across twenty-three named tiny-skia model targets. Native production/
test and browser production checks passed. Exact final gate scope and source
hashes are recorded in
[individual-object-protection-verification.json](individual-object-protection-verification.json).

Individual lock-property authoring controls, complete locked-object selection
and gesture feedback, group/member editing and general DrawingML/VML ownership
or MCE remain open. Full schema validation, malformed/unsupported record repair,
large-file performance and real Office save/repair/rendering evidence are not
proven. This does not add form controls, a scenario editor, automatic range
authentication prompts or encrypted XLSX support. The wider
[ibisOffice parity audit](ibis-parity.md) remains active.
