# Worksheet formula ownership during package edits

This milestone scopes the package formula rewrite used by partial cell shifts,
table renames/column changes/deletion and conversion to normal ranges. The
previous shared pass accepted any familiar formula name in several known
namespaces, even inside foreign wrappers or unsupported source choices.
It also lacked the coverage anchor for x14 validation and the inferred address
of cells with omitted coordinates.

## Implemented source paths

A worksheet visitor now requires the correct namespace and parent path.
Transitional/Strict main namespaces and aliases are supported. Foreign/nested
lookalikes are preserved. Namespace scope is captured as the streaming reader
reaches each cell, avoiding a scan from the worksheet start for every cell.

| Source | Ownership and calculation context |
|---|---|
| Cell formula | Direct main-namespace `sheetData/row/c/f`; inferred row/cell positions when their indices are omitted |
| Validation formula | Reader-interpreted standard/x14 records, physical container/index, first coverage cell; supported formula sources and fallback alternatives |
| Conditional formula | Direct worksheet conditional formats or x14 conditional-format collections in direct worksheet extensions; direct `cfRule` formula leaves |
| Conditional threshold | Direct data-bar/color-scale/icon-set `cfvo`; only `type="formula"`, standard `val` or extended `xm:f` |
| Sparkline source/date axis | Direct x14 sparkline groups in worksheet extensions; group source or direct sparkline source, with the display cell as its row context |

The visitor consumes recognized feature subtrees once. Missing-range validation
siblings, foreign containers/wrappers and unsupported Choice branches are not
formula operands. Literal list strings remain literal. Supported sources and
fallbacks update together. A table-relative source such as `[@Qty]` in an
x14 validation inside a table now follows a header rename. During conversion,
`H3:H4` with `[@Qty]` becomes the table's single relative-row A1 operand for
row 3; a separate rule on H4 uses row 4. The existing one-step history and
saved-file flow include these changes.

Text-only formula leaves are decoded before rewriting. A leaf with element
children stays opaque, rather than losing those children. Comments and
processing instructions remain when text changes; unchanged leaves retain
CDATA, quoting and original XML bytes. Invalid replacement XML characters are
refused before a rewritten worksheet is returned.

The source paths follow the [cell formula model](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.cellformula?view=openxml-3.0.1)
and Microsoft's [extended conditional rule definition](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/025ea6e4-ad42-43ea-a016-16f4e4688ac8).
These describe saved-file structure; they do not prove application behavior.
No external implementation code was copied.

## Scope and remaining evidence

The seven integration cases cover cross-sheet standard/x14 validation updates,
opaque source preservation, table rename, unqualified x14 column rename,
implicit cell addresses, current-row A1 conversion, standard/Strict conditional
formulas and thresholds, and sparkline/date-axis sources. Three writer unit
cases separately check source-alternative ownership, nontext/CDATA/comment
preservation and invalid replacement text. Cell-level MCE in the writer unit
is a serialization check; the later [cell-content cases](cell-content.md)
separately check reader selection and compatible cell/cache updates. The
shared-model case covers x14 table-relative input
bounds through header rename, rejected/accepted input, save/reopen and history.

General MCE around worksheet feature containers remains outside this visitor.
Whole-axis cell/conditional/sparkline processing and sheet rename/delete still
have separate formula passes and need their own ownership audit. Other package
parts, including workbook names and charts, retain their existing rewrite
paths and need broader namespace/ownership cases. The subsequent
[sparkline reader correction](sparkline-reading.md) scopes group/data/colour
ownership and text leaves, including an eighth integration case in this
target. The subsequent [standard cell-content pass](cell-content.md) covers
cell/shared-string/run/phonetic ownership and selection. General MCE and
other feature families still need their own audit; correct preservation by
this writer does not establish correct interpretation in every reader.

This is part of the ongoing [ibisOffice comparison](ibis-parity.md). Manual
native interaction, Excel save/repair/calculation/pixel oracles and the broader
feature gaps remain open.


The current formula-ownership checkpoint passes 905 core tests across all 143
targets and 20 shared-model scenarios (thirteen validation, seven tables), with
native production/test and browser wasm compilation. See
[formula-ownership-verification.json](formula-ownership-verification.json)
for target results, frozen hashes and evidence limits. Manual UI and Excel
application oracles remain outstanding.
