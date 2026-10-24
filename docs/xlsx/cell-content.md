# Cell content and source ownership

Cell values/formulas, inline/shared rich runs and phonetic readings now consume
the same namespace- and hierarchy-aware standard content view. The view is
temporary: it is never used to regenerate the saved worksheet or shared-string
part. Writers use original byte locations instead.

The view recognizes Transitional, Strict and the existing namespace-free test
documents. Foreign and misplaced rows, cells, values, formulas, string items,
runs, fonts and sheet settings cannot impersonate the grid. Only unqualified
schema attributes supply cell/row/column/font settings. Attribute values are
decoded and escaped when constructing the view, including quotes in a
single-quoted source attribute. Text, CDATA, entity references and significant
spaces are retained; nested foreign text is excluded.

Empty shared-string items retain their indices in the value, rich-run and
phonetic pools. Omitted cell coordinates are inferred from the selected grid
and supplied consistently to those readers. Phonetic guides remain separate
from the displayed value.

## Alternate content and editing

The first understood Choice supplies interpreted content; Fallback is selected
when no Choice is understood. Every required prefix must resolve to a supported
namespace, including declarations local to a Choice. Empty selected Choices
remain selected, and nested alternatives are handled. The namespace support
list is shared with the validation source selector. This implements these
content-selection cases, not every MCE rule.

The [Microsoft OOXML primer](https://download.microsoft.com/download/e/1/4/e14fb96f-83b8-4a2a-84db-7fa8acbe061a/Office%20Open%20XML%20Part%203%20-%20Primer.pdf)
describes first-match Choice/Fallback selection in section 8.3.4.2.1. The
[shared-string documentation](https://learn.microsoft.com/en-us/office/open-xml/spreadsheet/working-with-the-shared-string-table)
describes the indexed string pool and rich text representation. These primary
descriptions are separate from the regression/application evidence.

Cell patching, shared-formula detachment and safe row/cell ordering now follow
owned source tags. Value/style edits also update existing cells in other
understood representations, while unknown Choices and unrelated foreign XML
remain opaque. Namespace declarations local to a replaced cell or expanded
shared-formula tag survive. Ordering stops before rearranging opaque siblings.

Formula-cache refresh follows the selected formula's logical cell and updates
its understood formula representations. Existing value leaves stay in their
original branches. Missing caches use the preferred formula or a selected
empty/nested value branch, retaining namespace bindings even when prefixes are
locally rebound. A regression checks that making an unknown Choice understood
does not expose a duplicate cache in the tested layouts. Formula text and
opaque branches remain intact. An inactive formula cannot turn a selected
stored value into a formula cache. Separate alternate grids
with equivalent formulas are checked under both capability selections, before
and after recalculation, edit, save and history. This is a simulated selection
comparison, not a second spreadsheet application's result.

Cache refresh and alternate-cell editing retain detailed spans only for the
requested references; source-location scans do not construct another XML copy.
An auxiliary unoptimized-build probe generated 100,000 numeric cells, with one
formula cache to update (4,677,912 XML bytes). Peak resident memory decreased
from 45,809,664 to 16,941,056 bytes after filtering and the final cache
corrections. The same standalone harness linked the actual writer
and its exact reader dependency before and after. This is one synthetic
measurement, not a production-corpus performance guarantee.

## Regression evidence

The first nine reader cases all failed before the change. Four writer cases
then all failed against the scoped reader. A cache-refresh case reproduced a
stale real-cell cache while a foreign lookalike changed; a headless scenario
reproduced the same error during open-for-editing. Additional cases caught
attribute-quote damage, stale known Fallback values and an inactive formula
mistakenly treated as the selected stored cell's cache.

The final focused targets contain twelve reader, twelve writer and three
headless cases. They include Strict/default/local namespaces, selected and
unknown branches, nested/empty choices, implicit indices, shared strings,
CDATA/entities, run fonts, phonetic keys, formula caches, local namespace
declarations, shared-formula isolation, effective cache cardinality across
capability selections, local cache namespaces, exact foreign XML, save/reopen and
model/package Undo/Redo.

The existing prefix-generation fixture used the arbitrary `urn:sheet`
namespace. It now uses the actual SpreadsheetML namespace while keeping its
prefix assertions; a separate negative case rejects editing a foreign
worksheet root. A new headless fixture initially treated the model's saved
phonetic String as the detailed reading struct; its field access was corrected.
Neither fixture correction is a production compatibility fix.

## Remaining work

This is not general MCE conformance or full XML schema validation.
MustUnderstand, ProcessContent, preservation directives and alternate-content
wrappers around other feature families need separate work. Alternate records
with different grid structure, shared-formula groups spanning representations,
and synchronization of unknown representations need further cases. Cache
refresh uses the selected calculation result; it does not independently
evaluate every alternative formula.

The other structural/sheet-rename/package-part writers retain their existing
ownership passes. Those passes, rich-text/phonetic authoring, broader imported
records, performance on large production corpora, manual native dialogs and
Excel save/repair/calculation/pixel oracles remain open. The broader
[ibisOffice comparison](ibis-parity.md) is still active. No external
implementation code was copied.

## Final checkpoint

The final frozen sources passed 953 tests across all 148 targets of the six
selected implementation crates, plus 25 shared-model tests. Native production/
test and browser wasm compilation passed. Results, 129 file hashes, source
provenance, the selected/excluded package lists and evidence limits are in
[cell-content-verification.json](cell-content-verification.json). The separate
API/CLI/MCP/diff/native-render test suites remain outside this checkpoint.
