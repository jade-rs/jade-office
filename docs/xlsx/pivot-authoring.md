# Editing saved worksheet pivots

The later [creation/deletion checkpoint](pivot-lifecycle.md) adds both operations,
a common source/destination panel and displayed-value static chart snapshots.
This page's counts and source hashes describe the earlier authoring checkpoint.


Saved worksheet pivots now have a common editor in the native and browser
context menus. Select a cell inside a supported report and choose Edit pivot
table. This closes part of the authoring gap in the
[pinned ibisOffice comparison](ibis-parity.md); the implementation is independent
and incorporates no reference implementation code.

The editor changes the source range or declared name/table, report origin,
row/column/filter fields and their order, Values axis, report layout, value
aggregations/captions, member selections, field captions/sort/subtotals, grand
totals, column fitting and named styles. Standard custom styles and 84 normative
preset names are available. Source changes currently require the same headers,
field count and order. An explicit item refresh populates actual cache members
from the current source while retaining pending editor choices; apply a changed
source first. Item refresh is its own history step.

Writer `pivot_refresh::update_pivot`, headless `update_pivot[_with_layout]` and
model `update_pivot_named` prepare a preserving patch and rebuild every report
sharing the cache. Cache/report/chart writes, fitting and recalculation commit
as one edit. Invalid fields, occupied destinations, incompatible sources,
protection failures and unsupported chart transformations preserve the original
package and redo history. The editor refuses stale configuration and reused
cache indices, including after Undo or a remote replacement.

Measure insertion/reordering/removal maps old measure identities explicitly so
existing chart series retain their IDs, colors and combination-chart lane.
New measures receive new identities. Dimension keys use canonical field/member
pairs. Full point-to-series formatting translation after changing axis topology
is not established. Report moves clear only old report/filter cells; actual
filter labels, values and geometry are written and collision-checked.

The preserving XML writer omits empty optional containers whose schemas require
at least one entry. It refuses removal that would discard unknown container
metadata. Existing entry metadata survives understood edits. Required table-to-
cache relationships are added for unambiguous legacy files; contradictory,
ambiguous or external explicit links refuse editing. The reader continues to
show repairable files with absent legacy links. Valid Strict chart percentage
values such as `75%` now produce the same geometry as Transitional numeric values.

## Verification

The [machine-readable checkpoint](pivot-authoring-verification.json) records
frozen source hashes, complete target results and the scope of every oracle.
Package and model totals replace earlier checkpoints and are not additive.

| Check | Result |
|---|---|
| Reader/writer/calc/headless/layout/render | 1,230 passed /177 targets |
| API/CLI/MCP/diff | 182 passed /15 targets |
| Standalone native renderer | 14 passed /4 targets |
| Combined eleven XLSX packages | 1,426 passed /196 targets |
| Shared OPC suite | 38 passed /8 targets |
| Shared model, named tiny-skia targets | 160 passed /28 targets |
| Actual native production/tests and wasm production compilation | Passed |

New cases exercise saved values and charts, real TinySkia style-removal pixels,
manual cell-format preservation, multiple filter geometry, source growth,
collisions, stale dialogs, member-choice refresh, malformed quoted sheet names,
reopen and Undo/Redo. Application compilation is distinct from a pointer/keyboard
runtime oracle. The model selection omits the print case requiring both renderer
features and one Skia-only Thai case. The calculation engine and function
registry are unchanged; previous 2,246 passes/eight existing ignored cases and
507 registrations remain carried evidence.

## Independent schema and SDK checks

The original corpus generator produces 20 actual packages: original plus nine
editing/repair scenarios, each in Transitional and Strict. It asserts literal
saved chart values and one actual chart in every output. The Strict seed has
real Strict namespaces and valid percentage lexical values. It is a synthetic
fixture, not an Office-produced or Office-repaired file.

Unmodified official ECMA-376 Part 1/4 schemas check all 180 selected XML
components. There are 160 raw XSD passes and 20 separately recorded `xml:space`
deviations on worksheet text, with no unexpected failures. Raw nonzero XSD exits
and diagnostics remain in the report. Excel uses `xml:space` as described in the
[Microsoft implementation note](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/82ad64a7-401b-4f34-9319-3600b16b2791);
workbooks and schemas are not rewritten to suppress those errors. This component
check does not validate OPC relationships or all semantic constraints.

The standalone QA project runs the actual Open XML SDK 3.5.1 validator with
Office2021 constraints, read-only. All ten Transitional packages have zero
errors. The full probe also retains ten Strict errors: the SDK rejects valid
`gapWidth="150%"` as UInt16. The unmodified Strict XSD accepts that lexical form.
These are recorded SDK probe limitations, not a Strict SDK pass. No valid
percentage is stripped or converted to make that probe pass. The missing
required cache relationship found by the SDK was corrected in the original
fixture generator and in legacy save repair, with regression coverage.

Reproduce from the repository root with extracted official schemas:

```sh
cargo run --locked --manifest-path jade-core/Cargo.toml \
  -p jade-xlsx-headless-editor --example pivot_authoring_corpus -- /tmp/pivot-corpus
python3 scripts/xlsx/validate_pivot_authoring_corpus.py \
  --corpus /tmp/pivot-corpus --transitional-schema /path/to/transitional \
  --strict-schema /path/to/strict --allow-office-xml-space \
  --report /tmp/pivot-xsd-report.json
dotnet restore scripts/xlsx/openxml-validator/OpenXmlValidator.csproj --locked-mode
dotnet run --no-restore --project scripts/xlsx/openxml-validator -- \
  /tmp/pivot-corpus /tmp/pivot-sdk-report.json
```

The final command intentionally returns nonzero for the recorded Strict probe
errors; run against the byte-identical Transitional subset for its zero-error
gate. The .NET 10 QA project and its pinned NuGet lock are optional verification
tools; production Cargo manifests, locks and dependencies are unchanged in this
milestone. Official schemas, SDK binaries and generated corpus remain outside
the repository. Archive/package hashes are retained in the proof.

Creation/deletion, arbitrary source field remapping, calculated/grouped fields,
show-as and advanced value/top-N/dynamic filters, unsupported compact/mixed
layouts, all grouped/page styles, automatic load refresh, OLAP, slicers and
timelines remain open. Full native/browser interaction, Office refresh/save/
repair/visual equivalence, all chart producers/options, general chart MCE/schema
hardening and the other audit gaps remain unproven. Full ibisOffice parity is
not complete.
