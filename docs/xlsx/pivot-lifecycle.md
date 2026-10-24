# Creating and deleting worksheet pivots

Both application context menus now create a supported worksheet pivot from a
range, declared name or table. The dialog chooses its name and an existing or
new destination worksheet. A new worksheet and its report form one history
edit; failure leaves neither behind. The application selects the created report
and opens its field editor. Hidden worksheets cannot become the dialog's active
destination.

The initial report suggests the first numeric, non-date value field (Sum), or
Count when no such field exists, and the first other field for rows. This is an
initial summary, not Excel's empty field-selection placeholder. The existing
editor changes fields, aggregations, members, layout and style afterwards.
Creation currently builds an independent cache. Empty/no-value layouts, creation
with a reused cache and arbitrary source field remapping remain work to do.

The preserving writer creates actual cache definition/records/report parts,
content types, workbook cache declarations and all required relationships.
Part names avoid existing parts, relationship parts and content-type reservations.
Cache IDs also reserve retained main-namespace declarations, including other
markup branches, without promoting them into interpreted caches. Only real,
staged cache members are used for initial selections. Occupied origins, source
intersections, other reports, tables, merges, arrays and protection are checked.

Delete PivotTable removes the report and its owned body/filter values, retaining
manual cell formats. Other reports and their shared cache/records stay intact.
An unused cache is collected only after checking incoming package references;
opaque dependencies, potential URI aliases and uncertain ownership retain it.
Related locally corroborated charts become static charts using the saved worksheet values currently displayed by the renderer. Stale,
empty or missing chart caches are rebuilt first. Numeric category types,
uniform source formats, colors, IDs and plot roles remain.
Ordinary charts still follow their worksheet cells. Dependent GETPIVOTDATA calls
recalculate to a reference error when their report disappears.

Conversion retains namespace bindings and reads the result back to verify cached
data and series identity. Cached chart titles use ST_Xstring, whereas DrawingML
text is plain XML string data: literal escape text, Unicode and carriage returns
are preserved, while an unrepresentable XML control refuses the entire deletion.
Nonempty pivot format deltas, hidden visible-only points and mixed numeric
category formats need reconciliation and currently refuse rather than changing
the displayed result. Ambiguous sources, opaque source conversion and slicer/
timeline bindings also remain explicit unsupported cases. Byte-only workbook
filename identity and complete external alias recognition are still unproven.

## Verified checkpoint

| Check | Result |
|---|---|
| Reader/writer/calc/headless/layout/render | 1,252 passed /178 targets |
| API/CLI/MCP/diff | 182 passed /15 targets |
| Standalone native renderer | 14 passed /4 targets |
| Combined eleven XLSX packages | 1,448 passed /197 targets |
| Shared OPC suite | 38 passed /8 targets |
| Shared model, named tiny-skia targets | 164 passed /28 targets |
| Actual native production/tests and wasm production compilation | Passed |

Twenty-two headless cases cover real creation, formula sources, occupied origins,
new-sheet history, initial member selection, ID reservation, deletion, shared/
opaque cache ownership, legacy markers/manual formats, protection, chart
conversion, title strings and refusal/redo. Four new model cases cover creation
and text focus, hidden destinations, and actual unchanged chart color pixels
through deletion and Undo/Redo. Production inputs stayed unchanged during the final gates after the title-control
and displayed-snapshot corrections. A test-only stale-cache precondition was
changed to use the public reader, then all 28 model targets were rerun. The
final source record covers 364 files.
All 237 scoped dirty/new Rust files pass formatting. Totals replace earlier
checkpoints and are not additive. Print requiring both renderer features and one
Skia-only Thai model case remain outside the tiny-skia selection.

## Independent validation

The existing authoring corpus (20 packages) and new creation/deletion corpus
(14 packages) assert literal saved values and typed graph/chart state in both
Transitional and Strict formats. The new cases include creation on an existing
sheet, creation with a filter, creation on a new sheet, removal of an independent
created cache, removal of the last report, a linked chart title, and stale saved caches.

Unmodified official ECMA-376 Part 1/4 schemas check all 308 selected XML
components: 282 raw XSD passes, 26 separately retained Office `xml:space`
deviations, and zero unexpected failures. The existing
[Microsoft implementation note](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/82ad64a7-401b-4f34-9319-3600b16b2791)
explains Excel's use of that attribute. Raw nonzero exits remain; neither schemas
nor saved packages are adapted to suppress errors.

The actual Open XML SDK 3.5.1 Office2021 validator reports zero errors for all
17 Transitional packages. Its full Strict probe retains 17 known UInt16 errors
on valid `gapWidth="150%"`; the unmodified Strict chart XSD accepts those values.
This is not a Strict SDK pass. These oracle counts are separate from package
unit/integration totals and do not establish Office application equivalence.

Reproduce the new corpus from the repository root:

```sh
cargo run --locked --manifest-path jade-core/Cargo.toml \
  -p jade-xlsx-headless-editor --example pivot_lifecycle_corpus -- /tmp/pivot-lifecycle
python3 scripts/xlsx/validate_pivot_authoring_corpus.py --profile lifecycle \
  --corpus /tmp/pivot-lifecycle --transitional-schema /path/to/transitional \
  --strict-schema /path/to/strict --allow-office-xml-space \
  --report /tmp/pivot-lifecycle-xsd.json
```

The optional [SDK QA project](../../scripts/xlsx/openxml-validator/OpenXmlValidator.csproj)
and usage in the [authoring notes](pivot-authoring.md) apply to this corpus too.
Production and QA dependencies/lockfiles are unchanged in this milestone.
Schemas, validator binaries and generated workbooks remain outside the repository.
The engine/registry and prior 489-file patch replay are carried unchanged evidence:
2,246 engine passes/eight existing ignored cases and 507 registrations.

The official [deletion guide](https://support.microsoft.com/en-us/excel/delete-a-pivottable)
describes removing a report from a worksheet; the current implementation adds
that operation to both Jade context menus. The independent original corpus and
preserving implementation incorporate no reference implementation code.
Detailed hashes, target results and limits are in
[pivot-lifecycle-verification.json](pivot-lifecycle-verification.json).
Full ibisOffice parity, complete Office refresh/save/repair/visual validation,
all native/browser widget interactions, blank/advanced pivot layouts, source
schema remapping, grouped/calculated/show-as fields, full PivotChart conversion,
slicers/timelines and the other audit gaps remain open.
